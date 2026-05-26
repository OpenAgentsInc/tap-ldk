use std::{error::Error, fmt, str::FromStr};

use lightning::{
    ln::{
        taproot_asset::{
            SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
            TAPROOT_ASSET_RECOVERY_SPEND_COMMITMENT, TAPROOT_ASSET_RECOVERY_SPEND_FINAL_SWEEP,
            TAPROOT_ASSET_RECOVERY_SPEND_SECOND_LEVEL_HTLC, TaprootAssetChannelDescriptor,
            TaprootAssetProofOwnershipError, TaprootAssetProofOwnershipExpectation,
            TaprootAssetProofOwnershipState, prepare_asset_proof_ownership_recovery,
            validate_asset_proof_ownership_recovery,
        },
        types::ChannelId,
    },
    types::features::{ChannelTypeFeatures, InitFeatures},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{AssetError, Bytes32},
    asset_channel_funding::{AssetChannelFundingError, run_asset_channel_funding_smoke},
    asset_commitment::{
        AssetCommitmentChannelState, AssetCommitmentError, AssetCommitmentStore,
        build_commitment_update,
    },
    asset_htlc::{
        AssetHtlcCustomRecords, AssetHtlcDecode, AssetHtlcError, AssetHtlcStatus, AssetHtlcStore,
        decode_custom_records, validate_final_hop,
    },
    asset_payment::{
        NativeAssetPaymentError, NativeAssetPaymentRequest, NativeAssetPaymentStatus,
        NativeAssetPaymentStore, send_native_asset_payment,
    },
    asset_peer_message::AssetPeerMessage,
    ldk_baseline::BaselineBtcSmokeState,
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoice, QuoteBoundInvoiceRequest, RfqInvoiceError,
        bind_quote_to_invoice, pay_quote_bound_invoice, receive_native_rfq_request,
    },
    rfq_quote_store::{RfqHtlcAuthorization, RfqQuoteError, RfqQuoteStatus, RfqQuoteStore},
};

pub const NATIVE_ASSET_RECOVERY_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAssetRecoveryStage {
    Funding,
    QuoteAccepted,
    HtlcAdded,
    CommitmentSigned,
    Settled,
    ClosePrepared,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetClosePreparation {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_balance: u64,
    pub close_tx_digest: Bytes32,
    pub prepared: bool,
}

impl NativeAssetClosePreparation {
    pub fn new(state: &AssetCommitmentChannelState) -> Self {
        let close_tx_digest = close_tx_digest(state);
        Self {
            channel_id: state.channel_id.clone(),
            asset_id: state.asset_id,
            commitment_number: state.latest_commitment_number,
            local_balance: state.local_balance,
            remote_balance: state.remote_balance,
            total_balance: state.total_amount,
            close_tx_digest,
            prepared: true,
        }
    }

    fn validate(&self) -> Result<(), NativeAssetRecoveryError> {
        if !self.prepared {
            return Err(NativeAssetRecoveryError::StorageInvariant(
                "close preparation marker is not prepared".to_owned(),
            ));
        }
        if self
            .local_balance
            .checked_add(self.remote_balance)
            .ok_or(NativeAssetRecoveryError::BalanceOverflow)?
            != self.total_balance
        {
            return Err(NativeAssetRecoveryError::BalanceNotConserved {
                local_balance: self.local_balance,
                remote_balance: self.remote_balance,
                total_balance: self.total_balance,
            });
        }
        let expected = close_tx_digest_from_parts(
            &self.channel_id,
            self.asset_id,
            self.commitment_number,
            self.local_balance,
            self.remote_balance,
        );
        if self.close_tx_digest != expected {
            return Err(NativeAssetRecoveryError::StorageInvariant(
                "close preparation digest mismatch".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetRecoveryCheckpoint {
    pub version: u32,
    pub stage: NativeAssetRecoveryStage,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_balance: u64,
    pub quote_id: Option<String>,
    pub htlc_id: Option<String>,
    pub payment_id: Option<String>,
    pub close_preparation: Option<NativeAssetClosePreparation>,
    pub checkpoint_digest: Bytes32,
}

impl NativeAssetRecoveryCheckpoint {
    fn new(
        stage: NativeAssetRecoveryStage,
        state: &AssetCommitmentChannelState,
        quote_id: Option<String>,
        htlc_id: Option<String>,
        payment_id: Option<String>,
        close_preparation: Option<NativeAssetClosePreparation>,
    ) -> Result<Self, NativeAssetRecoveryError> {
        let mut checkpoint = Self {
            version: NATIVE_ASSET_RECOVERY_CHECKPOINT_SCHEMA_VERSION,
            stage,
            channel_id: state.channel_id.clone(),
            asset_id: state.asset_id,
            commitment_number: state.latest_commitment_number,
            local_balance: state.local_balance,
            remote_balance: state.remote_balance,
            total_balance: state.total_amount,
            quote_id,
            htlc_id,
            payment_id,
            close_preparation,
            checkpoint_digest: Bytes32([0; 32]),
        };
        checkpoint.checkpoint_digest = checkpoint.digest();
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    fn validate(&self) -> Result<(), NativeAssetRecoveryError> {
        if self.version != NATIVE_ASSET_RECOVERY_CHECKPOINT_SCHEMA_VERSION {
            return Err(NativeAssetRecoveryError::UnsupportedVersion(self.version));
        }
        if self
            .local_balance
            .checked_add(self.remote_balance)
            .ok_or(NativeAssetRecoveryError::BalanceOverflow)?
            != self.total_balance
        {
            return Err(NativeAssetRecoveryError::BalanceNotConserved {
                local_balance: self.local_balance,
                remote_balance: self.remote_balance,
                total_balance: self.total_balance,
            });
        }
        if self.checkpoint_digest != self.digest() {
            return Err(NativeAssetRecoveryError::StorageInvariant(
                "recovery checkpoint digest mismatch".to_owned(),
            ));
        }
        if self.stage == NativeAssetRecoveryStage::ClosePrepared {
            self.close_preparation
                .as_ref()
                .ok_or(NativeAssetRecoveryError::MissingClosePreparation)?
                .validate()?;
        }
        Ok(())
    }

    fn digest(&self) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:native-asset-recovery-checkpoint:v1");
        hasher.update([self.stage as u8]);
        hasher.update((self.channel_id.len() as u64).to_be_bytes());
        hasher.update(self.channel_id.as_bytes());
        hasher.update(self.asset_id.0);
        hasher.update(self.commitment_number.to_be_bytes());
        hasher.update(self.local_balance.to_be_bytes());
        hasher.update(self.remote_balance.to_be_bytes());
        hasher.update(self.total_balance.to_be_bytes());
        hash_optional_string(&mut hasher, self.quote_id.as_deref());
        hash_optional_string(&mut hasher, self.htlc_id.as_deref());
        hash_optional_string(&mut hasher, self.payment_id.as_deref());
        if let Some(close_preparation) = &self.close_preparation {
            hasher.update([1]);
            hasher.update(close_preparation.close_tx_digest.0);
        } else {
            hasher.update([0]);
        }
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetRecoveryStageReport {
    pub stage: NativeAssetRecoveryStage,
    pub recovered: bool,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_balance: u64,
    pub quote_status: Option<RfqQuoteStatus>,
    pub htlc_status: Option<AssetHtlcStatus>,
    pub payment_status: Option<NativeAssetPaymentStatus>,
    pub close_prepared: bool,
    pub refusal_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAssetProofRecoverySpendKind {
    Commitment,
    SecondLevelHtlc,
    FinalSweep,
}

impl NativeAssetProofRecoverySpendKind {
    fn as_ldk_spend_kind(self) -> u8 {
        match self {
            Self::Commitment => TAPROOT_ASSET_RECOVERY_SPEND_COMMITMENT,
            Self::SecondLevelHtlc => TAPROOT_ASSET_RECOVERY_SPEND_SECOND_LEVEL_HTLC,
            Self::FinalSweep => TAPROOT_ASSET_RECOVERY_SPEND_FINAL_SWEEP,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Commitment => "commitment",
            Self::SecondLevelHtlc => "second_level_htlc",
            Self::FinalSweep => "final_sweep",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAssetProofRecoveryStatus {
    AssetProofRecovered,
    BtcOnlyRefused,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetProofRecoveryReport {
    pub spend_kind: NativeAssetProofRecoverySpendKind,
    pub status: NativeAssetProofRecoveryStatus,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub btc_recovered: bool,
    pub asset_proof_recovered: bool,
    pub proof_root_hash: Bytes32,
    pub proof_root_sum: u64,
    pub proof_handoff_digest: Bytes32,
    pub sweep_output_digest: Bytes32,
    pub ldk_proof_ownership_digest: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetRecoveryMatrixReport {
    pub stages: Vec<NativeAssetRecoveryStageReport>,
    pub stale_checkpoint_refused: bool,
    pub normal_btc_restart_unaffected: bool,
    pub force_close_recovery: NativeAssetProofRecoveryReport,
    pub second_level_htlc_recovery: NativeAssetProofRecoveryReport,
    pub final_sweep_recovery: NativeAssetProofRecoveryReport,
    pub missing_proof_ownership_refused: bool,
    pub stale_proof_ownership_refused: bool,
    pub btc_sweep_without_asset_proof_refused: bool,
    pub btc_sweep_without_asset_proof_status: NativeAssetProofRecoveryStatus,
}

pub fn recover_native_asset_checkpoint(
    checkpoint: &NativeAssetRecoveryCheckpoint,
    commitment_store: &AssetCommitmentStore,
    rfq_store: Option<&RfqQuoteStore>,
    htlc_store: Option<&AssetHtlcStore>,
    payment_store: Option<&NativeAssetPaymentStore>,
) -> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    checkpoint.validate()?;
    let state = commitment_store.channel_state(&checkpoint.channel_id)?;
    if state.latest_commitment_number != checkpoint.commitment_number {
        return Err(NativeAssetRecoveryError::StaleCheckpoint {
            expected: state.latest_commitment_number,
            actual: checkpoint.commitment_number,
        });
    }
    if state.asset_id != checkpoint.asset_id
        || state.local_balance != checkpoint.local_balance
        || state.remote_balance != checkpoint.remote_balance
        || state.total_amount != checkpoint.total_balance
    {
        return Err(NativeAssetRecoveryError::RecoveredStateMismatch);
    }

    let quote_status = match (&checkpoint.quote_id, rfq_store) {
        (Some(quote_id), Some(store)) => Some(store.inspect_quote(quote_id)?.status),
        (Some(_), None) => return Err(NativeAssetRecoveryError::MissingRfqStore),
        (None, _) => None,
    };
    let htlc_status = match (&checkpoint.htlc_id, htlc_store) {
        (Some(htlc_id), Some(store)) => Some(
            store
                .htlcs
                .get(htlc_id)
                .ok_or_else(|| AssetHtlcError::UnknownHtlc(htlc_id.clone()))?
                .status,
        ),
        (Some(_), None) => return Err(NativeAssetRecoveryError::MissingHtlcStore),
        (None, _) => None,
    };
    let payment_status = match (&checkpoint.payment_id, payment_store) {
        (Some(payment_id), Some(store)) => Some(store.inspect_payment(payment_id)?.status),
        (Some(_), None) => return Err(NativeAssetRecoveryError::MissingPaymentStore),
        (None, _) => None,
    };

    Ok(NativeAssetRecoveryStageReport {
        stage: checkpoint.stage,
        recovered: true,
        channel_id: checkpoint.channel_id.clone(),
        asset_id: checkpoint.asset_id,
        commitment_number: checkpoint.commitment_number,
        local_balance: checkpoint.local_balance,
        remote_balance: checkpoint.remote_balance,
        total_balance: checkpoint.total_balance,
        quote_status,
        htlc_status,
        payment_status,
        close_prepared: checkpoint.close_preparation.is_some(),
        refusal_reason: None,
    })
}

pub fn run_native_asset_recovery_matrix_smoke()
-> Result<NativeAssetRecoveryMatrixReport, NativeAssetRecoveryError> {
    let mut stages = Vec::new();
    stages.push(recover_after_funding()?);
    stages.push(recover_after_quote_acceptance()?);
    stages.push(recover_after_htlc_add()?);
    stages.push(recover_after_commitment_sign()?);
    let (settled_report, settled_store, settled_checkpoint) = recover_after_settlement()?;
    stages.push(settled_report);
    stages.push(recover_after_close_preparation()?);

    let mut stale = settled_checkpoint.clone();
    stale.commitment_number = stale.commitment_number.saturating_sub(1);
    stale.checkpoint_digest = stale.digest();
    let stale_checkpoint_refused = matches!(
        recover_native_asset_checkpoint(&stale, &settled_store, None, None, None),
        Err(NativeAssetRecoveryError::StaleCheckpoint { .. })
    );

    let normal_btc_restart_unaffected = normal_btc_restart_round_trips()?;
    let settled_state = settled_asset_state(&settled_store)?;
    let force_close_recovery = recover_asset_proof_ownership(
        &settled_state,
        NativeAssetProofRecoverySpendKind::Commitment,
    )?;
    let second_level_htlc_recovery = recover_asset_proof_ownership(
        &settled_state,
        NativeAssetProofRecoverySpendKind::SecondLevelHtlc,
    )?;
    let final_sweep_recovery = recover_asset_proof_ownership(
        &settled_state,
        NativeAssetProofRecoverySpendKind::FinalSweep,
    )?;
    let missing_proof_ownership_refused = missing_proof_ownership_refused(
        &settled_state,
        NativeAssetProofRecoverySpendKind::Commitment,
    )?;
    let stale_proof_ownership_refused = stale_proof_ownership_refused(
        &settled_state,
        NativeAssetProofRecoverySpendKind::Commitment,
    )?;
    let btc_sweep_without_asset_proof_refused = btc_only_sweep_refused_as_asset_recovery(
        &settled_state,
        NativeAssetProofRecoverySpendKind::FinalSweep,
    )?;
    let btc_sweep_without_asset_proof_status = if btc_sweep_without_asset_proof_refused {
        NativeAssetProofRecoveryStatus::BtcOnlyRefused
    } else {
        NativeAssetProofRecoveryStatus::AssetProofRecovered
    };

    Ok(NativeAssetRecoveryMatrixReport {
        stages,
        stale_checkpoint_refused,
        normal_btc_restart_unaffected,
        force_close_recovery,
        second_level_htlc_recovery,
        final_sweep_recovery,
        missing_proof_ownership_refused,
        stale_proof_ownership_refused,
        btc_sweep_without_asset_proof_refused,
        btc_sweep_without_asset_proof_status,
    })
}

fn recover_after_funding() -> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    let (_channel_store, commitment_store, state) = initialized_commitment_store()?;
    let commitment_store = roundtrip(&commitment_store)?;
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::Funding,
        &state,
        None,
        None,
        None,
        None,
    )?;
    recover_native_asset_checkpoint(&checkpoint, &commitment_store, None, None, None)
}

fn recover_after_quote_acceptance()
-> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    let (_channel_store, commitment_store, state) = initialized_commitment_store()?;
    let mut rfq_store = RfqQuoteStore::default();
    let accept = accept_quote(
        &mut rfq_store,
        state.asset_id,
        50,
        Bytes32([51; 32]),
        Bytes32([52; 32]),
    )?;
    let rfq_store = roundtrip(&rfq_store)?;
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::QuoteAccepted,
        &state,
        Some(accept.quote_id),
        None,
        None,
        None,
    )?;
    recover_native_asset_checkpoint(&checkpoint, &commitment_store, Some(&rfq_store), None, None)
}

fn recover_after_htlc_add() -> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    let (_channel_store, commitment_store, state) = initialized_commitment_store()?;
    let mut rfq_store = RfqQuoteStore::default();
    let prepared = prepare_recovery_htlc(&mut rfq_store, state.asset_id, 60, 1_000)?;
    let validation = validate_final_hop(
        &prepared.records,
        &prepared.invoice,
        &prepared.authorization,
        1_004,
    )?;
    let mut htlc_store = AssetHtlcStore::default();
    let offered = htlc_store.add_htlc(&state.channel_id, validation)?;
    let htlc_store = roundtrip(&htlc_store)?;
    let rfq_store = roundtrip(&rfq_store)?;
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::HtlcAdded,
        &state,
        Some(prepared.invoice.quote_id),
        Some(offered.htlc_id),
        None,
        None,
    )?;
    recover_native_asset_checkpoint(
        &checkpoint,
        &commitment_store,
        Some(&rfq_store),
        Some(&htlc_store),
        None,
    )
}

fn recover_after_commitment_sign()
-> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    let (_channel_store, mut commitment_store, state) = initialized_commitment_store()?;
    let mut rfq_store = RfqQuoteStore::default();
    let prepared = prepare_recovery_htlc(&mut rfq_store, state.asset_id, 70, 1_000)?;
    let validation = validate_final_hop(
        &prepared.records,
        &prepared.invoice,
        &prepared.authorization,
        1_004,
    )?;
    let mut htlc_store = AssetHtlcStore::default();
    let offered = htlc_store.add_htlc(&state.channel_id, validation)?;
    let update =
        build_commitment_update(&state, prepared.records.asset_amount, 0, Bytes32([74; 32]))?;
    let snapshot = commitment_store.apply_update(update)?;
    let state = commitment_store.channel_state(&snapshot.channel_id)?;
    let commitment_store = roundtrip(&commitment_store)?;
    let htlc_store = roundtrip(&htlc_store)?;
    let rfq_store = roundtrip(&rfq_store)?;
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::CommitmentSigned,
        &state,
        Some(prepared.invoice.quote_id),
        Some(offered.htlc_id),
        None,
        None,
    )?;
    recover_native_asset_checkpoint(
        &checkpoint,
        &commitment_store,
        Some(&rfq_store),
        Some(&htlc_store),
        None,
    )
}

fn recover_after_settlement() -> Result<
    (
        NativeAssetRecoveryStageReport,
        AssetCommitmentStore,
        NativeAssetRecoveryCheckpoint,
    ),
    NativeAssetRecoveryError,
> {
    let (_channel_store, mut commitment_store, state) = initialized_commitment_store()?;
    let mut htlc_store = AssetHtlcStore::default();
    let mut payment_store = NativeAssetPaymentStore::default();
    let mut rfq_store = RfqQuoteStore::default();
    let request = NativeAssetPaymentRequest {
        channel_id: state.channel_id.clone(),
        sender_peer: "alice".to_owned(),
        receiver_peer: "bob".to_owned(),
        asset_id: state.asset_id,
        asset_amount: 125,
        rfq_id: Bytes32([81; 32]),
        invoice_context: Bytes32([82; 32]),
        payment_hash: Bytes32([83; 32]),
        asset_nonce: Bytes32([84; 32]),
        now_unix_seconds: 1_000,
    };
    let payment = send_native_asset_payment(
        &mut commitment_store,
        &mut htlc_store,
        &mut payment_store,
        &mut rfq_store,
        request,
    )?;
    let state = commitment_store.channel_state(&state.channel_id)?;
    let commitment_store = roundtrip(&commitment_store)?;
    let htlc_store = roundtrip(&htlc_store)?;
    let payment_store = roundtrip(&payment_store)?;
    let rfq_store = roundtrip(&rfq_store)?;
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::Settled,
        &state,
        Some(payment.quote_id),
        payment.htlc_id,
        Some(payment.payment_id),
        None,
    )?;
    let report = recover_native_asset_checkpoint(
        &checkpoint,
        &commitment_store,
        Some(&rfq_store),
        Some(&htlc_store),
        Some(&payment_store),
    )?;
    Ok((report, commitment_store, checkpoint))
}

fn recover_after_close_preparation()
-> Result<NativeAssetRecoveryStageReport, NativeAssetRecoveryError> {
    let (_settled_report, commitment_store, _settled_checkpoint) = recover_after_settlement()?;
    let state = commitment_store
        .channels
        .values()
        .next()
        .cloned()
        .ok_or_else(|| AssetCommitmentError::UnknownChannel("missing channel".to_owned()))?;
    let close_preparation = NativeAssetClosePreparation::new(&state);
    let checkpoint = NativeAssetRecoveryCheckpoint::new(
        NativeAssetRecoveryStage::ClosePrepared,
        &state,
        None,
        None,
        None,
        Some(close_preparation),
    )?;
    recover_native_asset_checkpoint(&checkpoint, &commitment_store, None, None, None)
}

fn recover_asset_proof_ownership(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Result<NativeAssetProofRecoveryReport, NativeAssetRecoveryError> {
    let ldk_state = build_ldk_proof_ownership_state(state, spend_kind, true, true)?;
    let expected = proof_ownership_expectation(state, spend_kind);
    validate_asset_proof_ownership_recovery(Some(&ldk_state), &expected)
        .map_err(NativeAssetRecoveryError::LdkProofOwnership)?;
    let prepared = prepare_asset_proof_ownership_recovery(
        &asset_features(),
        &asset_features(),
        &ChannelTypeFeatures::taproot_asset_single_asset(),
        TaprootAssetChannelDescriptor::new(
            state.asset_id.0,
            SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
        )
        .map_err(|_| {
            NativeAssetRecoveryError::LdkProofOwnership(
                TaprootAssetProofOwnershipError::AssetIdMismatch,
            )
        })?,
        ldk_state,
    )
    .map_err(NativeAssetRecoveryError::LdkProofOwnership)?;

    Ok(NativeAssetProofRecoveryReport {
        spend_kind,
        status: NativeAssetProofRecoveryStatus::AssetProofRecovered,
        channel_id: state.channel_id.clone(),
        asset_id: state.asset_id,
        commitment_number: state.latest_commitment_number,
        btc_recovered: prepared.btc_recovered,
        asset_proof_recovered: prepared.asset_proof_recovered,
        proof_root_hash: state.monitor_blob.proof_root_hash,
        proof_root_sum: state.monitor_blob.proof_root_sum,
        proof_handoff_digest: Bytes32(prepared.proof_handoff_digest),
        sweep_output_digest: Bytes32(prepared.sweep_output_digest),
        ldk_proof_ownership_digest: Bytes32(prepared.ownership_digest),
    })
}

fn missing_proof_ownership_refused(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Result<bool, NativeAssetRecoveryError> {
    let expected = proof_ownership_expectation(state, spend_kind);
    Ok(matches!(
        validate_asset_proof_ownership_recovery(None, &expected),
        Err(TaprootAssetProofOwnershipError::MissingProofOwnership)
    ))
}

fn stale_proof_ownership_refused(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Result<bool, NativeAssetRecoveryError> {
    let mut stale = build_ldk_proof_ownership_state(state, spend_kind, true, true)?;
    stale.commitment_number = stale.commitment_number.saturating_sub(1);
    stale.ownership_digest = stale.digest();
    let expected = proof_ownership_expectation(state, spend_kind);
    Ok(matches!(
        validate_asset_proof_ownership_recovery(Some(&stale), &expected),
        Err(TaprootAssetProofOwnershipError::CommitmentNumberMismatch)
    ))
}

fn btc_only_sweep_refused_as_asset_recovery(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Result<bool, NativeAssetRecoveryError> {
    Ok(matches!(
        build_ldk_proof_ownership_state(state, spend_kind, true, false),
        Err(NativeAssetRecoveryError::LdkProofOwnership(
            TaprootAssetProofOwnershipError::PartialRecovery
        ))
    ))
}

fn build_ldk_proof_ownership_state(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
    btc_recovered: bool,
    asset_proof_recovered: bool,
) -> Result<TaprootAssetProofOwnershipState, NativeAssetRecoveryError> {
    TaprootAssetProofOwnershipState::new(
        parse_channel_id(&state.channel_id)?,
        state.asset_id.0,
        state.latest_commitment_number,
        spend_kind.as_ldk_spend_kind(),
        btc_recovered,
        asset_proof_recovered,
        state.monitor_blob.proof_root_hash.0,
        state.monitor_blob.proof_root_sum,
        recovery_proof_handoff_digest(state, spend_kind).0,
        recovery_sweep_output_digest(state, spend_kind).0,
    )
    .map_err(NativeAssetRecoveryError::LdkProofOwnership)
}

fn proof_ownership_expectation(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> TaprootAssetProofOwnershipExpectation {
    TaprootAssetProofOwnershipExpectation {
        channel_id: parse_channel_id(&state.channel_id)
            .expect("smoke channel IDs are fixed 32-byte hex values"),
        asset_id: state.asset_id.0,
        latest_commitment_number: state.latest_commitment_number,
        spend_kind: spend_kind.as_ldk_spend_kind(),
        proof_root_hash: state.monitor_blob.proof_root_hash.0,
        proof_root_sum: state.monitor_blob.proof_root_sum,
        require_asset_proof: true,
    }
}

fn settled_asset_state(
    commitment_store: &AssetCommitmentStore,
) -> Result<AssetCommitmentChannelState, NativeAssetRecoveryError> {
    commitment_store
        .channels
        .values()
        .next()
        .cloned()
        .ok_or_else(|| AssetCommitmentError::UnknownChannel("missing channel".to_owned()).into())
}

fn asset_features() -> InitFeatures {
    let mut features = InitFeatures::empty();
    features.set_static_remote_key_optional();
    features.set_channel_type_optional();
    features.set_simple_taproot_staging_optional();
    features.set_taproot_asset_channel_optional();
    features
}

fn parse_channel_id(channel_id: &str) -> Result<ChannelId, NativeAssetRecoveryError> {
    let bytes = Bytes32::from_str(channel_id).map_err(NativeAssetRecoveryError::Asset)?;
    Ok(ChannelId::from_bytes(bytes.0))
}

fn recovery_proof_handoff_digest(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-force-close-proof-handoff:v1");
    hasher.update(spend_kind.as_str().as_bytes());
    hasher.update((state.channel_id.len() as u64).to_be_bytes());
    hasher.update(state.channel_id.as_bytes());
    hasher.update(state.asset_id.0);
    hasher.update(state.latest_commitment_number.to_be_bytes());
    hasher.update(state.monitor_blob.proof_root_hash.0);
    hasher.update(state.monitor_blob.proof_root_sum.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn recovery_sweep_output_digest(
    state: &AssetCommitmentChannelState,
    spend_kind: NativeAssetProofRecoverySpendKind,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-force-close-sweep-output:v1");
    hasher.update(spend_kind.as_str().as_bytes());
    hasher.update((state.channel_id.len() as u64).to_be_bytes());
    hasher.update(state.channel_id.as_bytes());
    hasher.update(state.asset_id.0);
    hasher.update(state.latest_commitment_number.to_be_bytes());
    hasher.update(state.local_balance.to_be_bytes());
    hasher.update(state.remote_balance.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PreparedRecoveryHtlc {
    invoice: QuoteBoundInvoice,
    authorization: RfqHtlcAuthorization,
    records: AssetHtlcCustomRecords,
}

fn prepare_recovery_htlc(
    rfq_store: &mut RfqQuoteStore,
    asset_id: Bytes32,
    seed: u8,
    now_unix_seconds: u64,
) -> Result<PreparedRecoveryHtlc, NativeAssetRecoveryError> {
    let accept = accept_quote(
        rfq_store,
        asset_id,
        25,
        Bytes32([seed; 32]),
        Bytes32([seed + 1; 32]),
    )?;
    let invoice = bind_quote_to_invoice(
        &accept,
        QuoteBoundInvoiceRequest {
            invoice: format!("lnbcrt1recovery{}", seed),
            payment_hash: Bytes32([seed + 2; 32]),
            peer: "alice".to_owned(),
            asset_id,
            asset_amount: accept.asset_amount,
            btc_msat: accept.btc_msat,
            invoice_context: accept.invoice_context,
            invoice_expiry_unix_seconds: now_unix_seconds + 60,
            now_unix_seconds: now_unix_seconds + 1,
        },
    )?;
    let payment = pay_quote_bound_invoice(rfq_store, invoice, now_unix_seconds + 2)?;
    let records =
        AssetHtlcCustomRecords::from_authorization(&payment.invoice, &payment.authorization)?;
    let encoded = records.encode_tlv()?;
    let records = match decode_custom_records(&encoded)? {
        AssetHtlcDecode::Asset(records) => records,
        AssetHtlcDecode::BtcOnly => return Err(NativeAssetRecoveryError::MissingAssetHtlc),
    };
    Ok(PreparedRecoveryHtlc {
        invoice: payment.invoice,
        authorization: payment.authorization,
        records,
    })
}

fn accept_quote(
    rfq_store: &mut RfqQuoteStore,
    asset_id: Bytes32,
    asset_amount: u64,
    rfq_id: Bytes32,
    invoice_context: Bytes32,
) -> Result<crate::rfq_quote_store::StoredRfqQuote, NativeAssetRecoveryError> {
    let message = AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount,
        invoice_context,
    };
    Ok(receive_native_rfq_request(
        rfq_store,
        "alice",
        &message,
        1_000,
        NativeRfqPolicy::default(),
    )?
    .quote)
}

fn initialized_commitment_store() -> Result<
    (
        crate::asset_channel_funding::AssetChannelStore,
        AssetCommitmentStore,
        AssetCommitmentChannelState,
    ),
    NativeAssetRecoveryError,
> {
    let (channel_store, report) = run_asset_channel_funding_smoke()?;
    let funded = channel_store
        .channels
        .get(&report.channel_id)
        .ok_or_else(|| AssetCommitmentError::UnknownChannel(report.channel_id.clone()))?;
    let mut commitment_store = AssetCommitmentStore::default();
    let state = commitment_store.initialize_channel(funded)?;
    Ok((channel_store, commitment_store, state))
}

fn roundtrip<T>(value: &T) -> Result<T, NativeAssetRecoveryError>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_slice(&serde_json::to_vec_pretty(value)?)?)
}

fn normal_btc_restart_round_trips() -> Result<bool, NativeAssetRecoveryError> {
    let state = BaselineBtcSmokeState::run_btc_only_smoke()?;
    let loaded =
        serde_json::from_slice::<BaselineBtcSmokeState>(&serde_json::to_vec_pretty(&state)?)?;
    Ok(loaded.payment == state.payment && loaded.bob.restart_count == state.bob.restart_count)
}

fn close_tx_digest(state: &AssetCommitmentChannelState) -> Bytes32 {
    close_tx_digest_from_parts(
        &state.channel_id,
        state.asset_id,
        state.latest_commitment_number,
        state.local_balance,
        state.remote_balance,
    )
}

fn close_tx_digest_from_parts(
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    local_balance: u64,
    remote_balance: u64,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-close-preparation:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn hash_optional_string(hasher: &mut Sha256, value: Option<&str>) {
    if let Some(value) = value {
        hasher.update([1]);
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    } else {
        hasher.update([0]);
    }
}

#[derive(Debug)]
pub enum NativeAssetRecoveryError {
    Json(serde_json::Error),
    Asset(AssetError),
    Funding(AssetChannelFundingError),
    Commitment(AssetCommitmentError),
    LdkProofOwnership(TaprootAssetProofOwnershipError),
    Rfq(RfqInvoiceError),
    RfqStore(RfqQuoteError),
    Htlc(AssetHtlcError),
    Payment(NativeAssetPaymentError),
    Baseline(crate::ldk_baseline::BaselineLdkError),
    UnsupportedVersion(u32),
    MissingRfqStore,
    MissingHtlcStore,
    MissingPaymentStore,
    MissingAssetHtlc,
    MissingClosePreparation,
    StaleCheckpoint {
        expected: u64,
        actual: u64,
    },
    RecoveredStateMismatch,
    BalanceOverflow,
    BalanceNotConserved {
        local_balance: u64,
        remote_balance: u64,
        total_balance: u64,
    },
    StorageInvariant(String),
}

impl fmt::Display for NativeAssetRecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "native asset recovery JSON error: {err}"),
            Self::Asset(err) => write!(f, "native asset recovery asset error: {err}"),
            Self::Funding(err) => write!(f, "native asset recovery funding error: {err}"),
            Self::Commitment(err) => write!(f, "native asset recovery commitment error: {err}"),
            Self::LdkProofOwnership(err) => {
                write!(
                    f,
                    "LDK asset proof ownership recovery rejected state: {err:?}"
                )
            }
            Self::Rfq(err) => write!(f, "native asset recovery RFQ error: {err}"),
            Self::RfqStore(err) => write!(f, "native asset recovery RFQ store error: {err}"),
            Self::Htlc(err) => write!(f, "native asset recovery HTLC error: {err}"),
            Self::Payment(err) => write!(f, "native asset recovery payment error: {err}"),
            Self::Baseline(err) => write!(f, "native asset recovery BTC baseline error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported native asset recovery checkpoint version {version}"
                )
            }
            Self::MissingRfqStore => write!(f, "native asset recovery missing RFQ store"),
            Self::MissingHtlcStore => write!(f, "native asset recovery missing HTLC store"),
            Self::MissingPaymentStore => write!(f, "native asset recovery missing payment store"),
            Self::MissingAssetHtlc => write!(f, "native asset recovery missing asset HTLC"),
            Self::MissingClosePreparation => {
                write!(f, "native asset recovery missing close preparation marker")
            }
            Self::StaleCheckpoint { expected, actual } => write!(
                f,
                "native asset recovery stale checkpoint: expected commitment {expected}, got {actual}"
            ),
            Self::RecoveredStateMismatch => {
                write!(f, "native asset recovery state does not match checkpoint")
            }
            Self::BalanceOverflow => write!(f, "native asset recovery balance overflowed"),
            Self::BalanceNotConserved {
                local_balance,
                remote_balance,
                total_balance,
            } => write!(
                f,
                "native asset recovery balances local={local_balance} remote={remote_balance} do not conserve total={total_balance}"
            ),
            Self::StorageInvariant(message) => {
                write!(
                    f,
                    "native asset recovery storage invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for NativeAssetRecoveryError {}

impl From<serde_json::Error> for NativeAssetRecoveryError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<AssetError> for NativeAssetRecoveryError {
    fn from(err: AssetError) -> Self {
        Self::Asset(err)
    }
}

impl From<AssetChannelFundingError> for NativeAssetRecoveryError {
    fn from(err: AssetChannelFundingError) -> Self {
        Self::Funding(err)
    }
}

impl From<AssetCommitmentError> for NativeAssetRecoveryError {
    fn from(err: AssetCommitmentError) -> Self {
        Self::Commitment(err)
    }
}

impl From<RfqInvoiceError> for NativeAssetRecoveryError {
    fn from(err: RfqInvoiceError) -> Self {
        Self::Rfq(err)
    }
}

impl From<RfqQuoteError> for NativeAssetRecoveryError {
    fn from(err: RfqQuoteError) -> Self {
        Self::RfqStore(err)
    }
}

impl From<AssetHtlcError> for NativeAssetRecoveryError {
    fn from(err: AssetHtlcError) -> Self {
        Self::Htlc(err)
    }
}

impl From<NativeAssetPaymentError> for NativeAssetRecoveryError {
    fn from(err: NativeAssetPaymentError) -> Self {
        Self::Payment(err)
    }
}

impl From<crate::ldk_baseline::BaselineLdkError> for NativeAssetRecoveryError {
    fn from(err: crate::ldk_baseline::BaselineLdkError) -> Self {
        Self::Baseline(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        path::PathBuf,
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn recovery_matrix_covers_all_native_boundaries() {
        let report = run_native_asset_recovery_matrix_smoke().expect("recovery matrix passes");
        assert_eq!(report.stages.len(), 6);
        assert!(report.stages.iter().all(|stage| stage.recovered));
        assert!(report.stale_checkpoint_refused);
        assert!(report.normal_btc_restart_unaffected);
        assert_eq!(
            report.force_close_recovery.status,
            NativeAssetProofRecoveryStatus::AssetProofRecovered
        );
        assert_eq!(
            report.second_level_htlc_recovery.status,
            NativeAssetProofRecoveryStatus::AssetProofRecovered
        );
        assert_eq!(
            report.final_sweep_recovery.status,
            NativeAssetProofRecoveryStatus::AssetProofRecovered
        );
        assert!(report.force_close_recovery.btc_recovered);
        assert!(report.force_close_recovery.asset_proof_recovered);
        assert!(report.missing_proof_ownership_refused);
        assert!(report.stale_proof_ownership_refused);
        assert!(report.btc_sweep_without_asset_proof_refused);
        assert_eq!(
            report.btc_sweep_without_asset_proof_status,
            NativeAssetProofRecoveryStatus::BtcOnlyRefused
        );
        assert!(
            report
                .stages
                .iter()
                .any(|stage| stage.stage == NativeAssetRecoveryStage::HtlcAdded
                    && stage.htlc_status == Some(AssetHtlcStatus::Offered))
        );
        assert!(
            report
                .stages
                .iter()
                .any(|stage| stage.stage == NativeAssetRecoveryStage::Settled
                    && stage.payment_status == Some(NativeAssetPaymentStatus::Settled))
        );
        assert!(report.stages.iter().any(|stage| stage.stage
            == NativeAssetRecoveryStage::ClosePrepared
            && stage.close_prepared));
    }

    #[test]
    fn stale_checkpoint_is_refused() {
        let (_report, commitment_store, checkpoint) =
            recover_after_settlement().expect("settlement recovers");
        let mut stale = checkpoint;
        stale.commitment_number = 0;
        stale.local_balance = 700;
        stale.remote_balance = 300;
        stale.checkpoint_digest = stale.digest();
        assert!(matches!(
            recover_native_asset_checkpoint(&stale, &commitment_store, None, None, None),
            Err(NativeAssetRecoveryError::StaleCheckpoint { .. })
        ));
    }

    #[test]
    fn htlc_and_payment_stores_persist_recovery_markers() {
        let (_report, _commitment_store, _checkpoint) =
            recover_after_settlement().expect("settlement recovers");
        let (_channel_store, commitment_store, state) =
            initialized_commitment_store().expect("state initializes");
        let mut rfq_store = RfqQuoteStore::default();
        let prepared =
            prepare_recovery_htlc(&mut rfq_store, state.asset_id, 90, 1_000).expect("prepared");
        let validation = validate_final_hop(
            &prepared.records,
            &prepared.invoice,
            &prepared.authorization,
            1_004,
        )
        .expect("valid");
        let mut htlc_store = AssetHtlcStore::default();
        let offered = htlc_store
            .add_htlc(&state.channel_id, validation)
            .expect("offered");
        let htlc_path = temp_path("htlc-store");
        htlc_store.save_atomic(&htlc_path).expect("htlc saves");
        let htlc_store = AssetHtlcStore::load(&htlc_path).expect("htlc loads");
        let _ = fs::remove_file(&htlc_path);
        let htlc_store = roundtrip(&htlc_store).expect("htlc roundtrip");
        assert_eq!(
            htlc_store
                .htlcs
                .get(&offered.htlc_id)
                .map(|htlc| htlc.status),
            Some(AssetHtlcStatus::Offered)
        );
        let checkpoint = NativeAssetRecoveryCheckpoint::new(
            NativeAssetRecoveryStage::HtlcAdded,
            &commitment_store
                .channel_state(&state.channel_id)
                .expect("state loads"),
            Some(prepared.invoice.quote_id),
            Some(offered.htlc_id),
            None,
            None,
        )
        .expect("checkpoint");
        assert!(
            recover_native_asset_checkpoint(
                &checkpoint,
                &commitment_store,
                Some(&rfq_store),
                Some(&htlc_store),
                None,
            )
            .is_ok()
        );

        let (_channel_store, mut commitment_store, state) =
            initialized_commitment_store().expect("state initializes");
        let mut htlc_store = AssetHtlcStore::default();
        let mut payment_store = NativeAssetPaymentStore::default();
        let mut rfq_store = RfqQuoteStore::default();
        let request = NativeAssetPaymentRequest {
            channel_id: state.channel_id,
            sender_peer: "alice".to_owned(),
            receiver_peer: "bob".to_owned(),
            asset_id: state.asset_id,
            asset_amount: 125,
            rfq_id: Bytes32([91; 32]),
            invoice_context: Bytes32([92; 32]),
            payment_hash: Bytes32([93; 32]),
            asset_nonce: Bytes32([94; 32]),
            now_unix_seconds: 1_000,
        };
        let payment = send_native_asset_payment(
            &mut commitment_store,
            &mut htlc_store,
            &mut payment_store,
            &mut rfq_store,
            request,
        )
        .expect("payment settles");
        let payment_path = temp_path("payment-store");
        payment_store
            .save_atomic(&payment_path)
            .expect("payment store saves");
        let loaded_payment_store =
            NativeAssetPaymentStore::load(&payment_path).expect("payment store loads");
        let _ = fs::remove_file(&payment_path);
        assert_eq!(
            loaded_payment_store
                .inspect_payment(&payment.payment_id)
                .expect("payment loads")
                .status,
            NativeAssetPaymentStatus::Settled
        );
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        env::temp_dir().join(format!("tap-ldk-{name}-{}-{nanos}.json", process::id()))
    }
}
