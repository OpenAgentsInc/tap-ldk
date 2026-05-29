use std::{error::Error, fmt, str::FromStr};

use lightning::ln::{
    taproot_asset::{
        TAPROOT_ASSET_RECOVERY_SPEND_COMMITMENT, TaprootAssetChannelStateError,
        TaprootAssetCloseAllocation, TaprootAssetCloseAllocationError,
        TaprootAssetProofOwnershipError, TaprootAssetProofOwnershipState,
    },
    types::ChannelId,
};
use serde::{Deserialize, Serialize};

use crate::{
    asset::{AssetError, Bytes32, CompressedKey},
    asset_channel_funding::{
        AssetChannelFundingError, build_ldk_asset_channel_state, run_asset_channel_funding_smoke,
    },
    asset_channel_negotiation::{NegotiatedChannelType, NegotiationError, run_negotiation_smoke},
    asset_close::{NativeAssetCloseError, NativeAssetCloseStore, cooperative_close},
    asset_commitment::{AssetCommitmentError, AssetCommitmentStore},
    asset_htlc::{AssetHtlcError, AssetHtlcStatus, AssetHtlcStore},
    asset_payment::{
        NativeAssetPaymentError, NativeAssetPaymentRequest, NativeAssetPaymentStatus,
        NativeAssetPaymentStore, send_native_asset_payment,
    },
    asset_peer_message::{AssetPeerMessageError, run_peer_message_smoke},
    ldk_baseline::{BaselineBtcSmokeState, BaselineLdkError},
    ldk_fork::OPENAGENTS_RUST_LIGHTNING_REV,
    rfq_quote_store::RfqQuoteStore,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimpleTaprootAssetChannelIntegrationReport {
    pub rust_lightning_rev: String,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub negotiated_simple_taproot_asset_channel: bool,
    pub proof_exchange_separate_from_open_channel: bool,
    pub proof_message_count: usize,
    pub funding_hook_approved: bool,
    pub initial_monitor_aux_persisted: bool,
    pub missing_monitor_update_rejected: bool,
    pub ldk_state_advanced_with_monitor_aux: bool,
    pub payment_id: String,
    pub htlc_id: String,
    pub payment_settled: bool,
    pub sender_balance_before: u64,
    pub receiver_balance_before: u64,
    pub sender_balance_after: u64,
    pub receiver_balance_after: u64,
    pub latest_commitment_number: u64,
    pub commitment_proof_history_replayed: bool,
    pub restart_reestablish_survived: bool,
    pub cooperative_close_exported: bool,
    pub cooperative_close_allocation_validated_by_ldk: bool,
    pub cooperative_close_preserved_latest_asset_allocation: bool,
    pub cooperative_close_restart_preserved_asset_allocation: bool,
    pub force_close_proof_ownership_validated_by_ldk: bool,
    pub btc_only_baseline_unaffected: bool,
}

pub fn run_simple_taproot_asset_channel_integration_smoke()
-> Result<SimpleTaprootAssetChannelIntegrationReport, SimpleTaprootAssetChannelIntegrationError> {
    let (channel_store, funding_report) = run_asset_channel_funding_smoke()?;
    let funded = channel_store
        .channels
        .get(&funding_report.channel_id)
        .cloned()
        .ok_or_else(|| {
            AssetChannelFundingError::UnknownChannel(funding_report.channel_id.clone())
        })?;
    let negotiation = run_negotiation_smoke(funding_report.asset_id)?;
    let negotiated_simple_taproot_asset_channel = matches!(
        negotiation.asset_channel,
        NegotiatedChannelType::SingleAsset { .. }
    ) && negotiation
        .fork_implicit_asset_upgrade_rejected;
    let peer_message = run_peer_message_smoke(&negotiation.asset_channel, funding_report.asset_id)?;
    let proof_exchange_separate_from_open_channel =
        peer_message.message_count > 0 && peer_message.premature_message_rejected;

    let mut ldk_state = build_ldk_asset_channel_state(&funded)?;
    let mut commitment_store = AssetCommitmentStore::default();
    let initial_state = commitment_store.initialize_channel(&funded)?;
    let initial_monitor_update = initial_state.build_ldk_monitor_update()?;
    ldk_state.require_current_monitor_aux_blob(
        initial_monitor_update.taproot_asset_aux_blobs().next(),
        initial_state.monitor_blob.state_digest.0,
    )?;

    let mut htlc_store = AssetHtlcStore::default();
    let mut payment_store = NativeAssetPaymentStore::default();
    let mut receiver_quote_store = RfqQuoteStore::default();
    let request = NativeAssetPaymentRequest {
        channel_id: initial_state.channel_id.clone(),
        sender_peer: "alice".to_owned(),
        receiver_peer: "bob".to_owned(),
        asset_id: initial_state.asset_id,
        asset_amount: 125,
        rfq_id: Bytes32([121; 32]),
        invoice_context: Bytes32([122; 32]),
        payment_hash: Bytes32([123; 32]),
        asset_nonce: Bytes32([124; 32]),
        now_unix_seconds: 1_000,
    };
    let sender_balance_before = initial_state.local_balance;
    let receiver_balance_before = initial_state.remote_balance;
    let payment = send_native_asset_payment(
        &mut commitment_store,
        &mut htlc_store,
        &mut payment_store,
        &mut receiver_quote_store,
        request.clone(),
    )?;
    let htlc_id = payment.htlc_id.clone().ok_or_else(|| {
        SimpleTaprootAssetChannelIntegrationError::Invariant(
            "settled payment is missing HTLC ID".to_owned(),
        )
    })?;
    let htlc = htlc_store
        .htlcs
        .get(&htlc_id)
        .ok_or_else(|| AssetHtlcError::UnknownHtlc(htlc_id.clone()))?;
    let latest_state = commitment_store.channel_state(&initial_state.channel_id)?;
    let missing_monitor_update_rejected = {
        let mut missing_monitor_state = build_ldk_asset_channel_state(&funded)?;
        matches!(
            missing_monitor_state.apply_commitment_update(
                latest_state.latest_commitment_number,
                request.asset_amount,
                0,
                latest_state.monitor_blob.state_digest.0,
                None,
            ),
            Err(TaprootAssetChannelStateError::Monitor(_))
        )
    };
    let latest_monitor_update = latest_state.build_ldk_monitor_update()?;
    ldk_state.apply_commitment_update(
        latest_state.latest_commitment_number,
        request.asset_amount,
        0,
        latest_state.monitor_blob.state_digest.0,
        latest_monitor_update.taproot_asset_aux_blobs().next(),
    )?;
    let ldk_state_advanced_with_monitor_aux = ldk_state.local_balance == latest_state.local_balance
        && ldk_state.remote_balance == latest_state.remote_balance
        && ldk_state.latest_commitment_number == latest_state.latest_commitment_number;
    let latest_commitment = latest_state
        .commitments
        .get(&latest_state.latest_commitment_number)
        .ok_or_else(|| {
            SimpleTaprootAssetChannelIntegrationError::Invariant(format!(
                "missing asset commitment {}",
                latest_state.latest_commitment_number
            ))
        })?;
    let commitment_proof_history_replayed = latest_state.latest_proof_history_output_id
        == latest_commitment.proof_history_output_id
        && latest_state.latest_proof_history_transition_id
            == latest_commitment.proof_history_transition_id;

    let close = cooperative_close(
        &commitment_store,
        &initial_state.channel_id,
        close_script_key(2)?,
        close_script_key(3)?,
    )?;
    let mut close_store = NativeAssetCloseStore::default();
    let close = close_store.record_close(close)?;
    let close_allocation = TaprootAssetCloseAllocation::new(
        parse_channel_id(&close.channel_id)?,
        close.asset_id.0,
        close.commitment_number,
        close.local_amount,
        close.remote_amount,
        close.proof_root_hash.0,
        close.proof_root_sum,
        close.local_proof_digest.0,
        close.remote_proof_digest.0,
    )
    .map_err(SimpleTaprootAssetChannelIntegrationError::LdkCloseAllocation)?;
    ldk_state.validate_cooperative_close(Some(&close_allocation))?;
    let cooperative_close_allocation_validated_by_ldk = ldk_state.closed;
    let cooperative_close_preserved_latest_asset_allocation = close.commitment_number
        == latest_state.latest_commitment_number
        && close.local_amount == latest_state.local_balance
        && close.remote_amount == latest_state.remote_balance
        && close.total_amount == latest_state.total_amount
        && close.proof_root_hash == latest_state.monitor_blob.proof_root_hash
        && close.proof_root_sum == latest_state.monitor_blob.proof_root_sum;
    let recovered_close_store = roundtrip(&close_store)?;
    let recovered_close = recovered_close_store.inspect_close(&close.close_id)?;
    let cooperative_close_restart_preserved_asset_allocation =
        recovered_close == close && cooperative_close_preserved_latest_asset_allocation;

    let proof_ownership = TaprootAssetProofOwnershipState::new(
        parse_channel_id(&close.channel_id)?,
        close.asset_id.0,
        close.commitment_number,
        TAPROOT_ASSET_RECOVERY_SPEND_COMMITMENT,
        true,
        true,
        close.proof_root_hash.0,
        close.proof_root_sum,
        close.local_proof_digest.0,
        close.close_digest.0,
    )
    .map_err(SimpleTaprootAssetChannelIntegrationError::LdkProofOwnership)?;
    ldk_state.validate_proof_ownership(
        Some(&proof_ownership),
        TAPROOT_ASSET_RECOVERY_SPEND_COMMITMENT,
    )?;

    let restart_reestablish_survived = roundtrip(&channel_store)?.validate().is_ok()
        && roundtrip(&commitment_store)?.validate().is_ok()
        && roundtrip(&htlc_store)?.validate().is_ok()
        && roundtrip(&payment_store)?.validate().is_ok()
        && recovered_close_store.validate().is_ok();
    let btc_only_baseline = BaselineBtcSmokeState::run_btc_only_smoke()?;
    let btc_only_baseline_unaffected =
        !btc_only_baseline.asset_channel_features_enabled && btc_only_baseline.validate().is_ok();

    Ok(SimpleTaprootAssetChannelIntegrationReport {
        rust_lightning_rev: OPENAGENTS_RUST_LIGHTNING_REV.to_owned(),
        channel_id: latest_state.channel_id.clone(),
        asset_id: latest_state.asset_id,
        negotiated_simple_taproot_asset_channel,
        proof_exchange_separate_from_open_channel,
        proof_message_count: peer_message.message_count,
        funding_hook_approved: funding_report.fork_funding_hook_approved,
        initial_monitor_aux_persisted: initial_state.monitor_blob.ldk_aux_blob_digest.is_some(),
        missing_monitor_update_rejected,
        ldk_state_advanced_with_monitor_aux,
        payment_id: payment.payment_id,
        htlc_id,
        payment_settled: payment.status == NativeAssetPaymentStatus::Settled
            && htlc.status == AssetHtlcStatus::Settled,
        sender_balance_before,
        receiver_balance_before,
        sender_balance_after: latest_state.local_balance,
        receiver_balance_after: latest_state.remote_balance,
        latest_commitment_number: latest_state.latest_commitment_number,
        commitment_proof_history_replayed,
        restart_reestablish_survived,
        cooperative_close_exported: !close.local_proof_tlv_hex.is_empty()
            && !close.remote_proof_tlv_hex.is_empty(),
        cooperative_close_allocation_validated_by_ldk,
        cooperative_close_preserved_latest_asset_allocation,
        cooperative_close_restart_preserved_asset_allocation,
        force_close_proof_ownership_validated_by_ldk: true,
        btc_only_baseline_unaffected,
    })
}

#[derive(Debug)]
pub enum SimpleTaprootAssetChannelIntegrationError {
    Asset(AssetError),
    Funding(AssetChannelFundingError),
    Negotiation(NegotiationError),
    PeerMessage(AssetPeerMessageError),
    Commitment(AssetCommitmentError),
    Payment(NativeAssetPaymentError),
    Htlc(AssetHtlcError),
    Close(NativeAssetCloseError),
    Baseline(BaselineLdkError),
    LdkChannelState(TaprootAssetChannelStateError),
    LdkCloseAllocation(TaprootAssetCloseAllocationError),
    LdkProofOwnership(TaprootAssetProofOwnershipError),
    Json(serde_json::Error),
    Invariant(String),
}

impl fmt::Display for SimpleTaprootAssetChannelIntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(err) => write!(f, "simple-taproot asset channel asset error: {err}"),
            Self::Funding(err) => write!(f, "simple-taproot asset channel funding error: {err}"),
            Self::Negotiation(err) => {
                write!(f, "simple-taproot asset channel negotiation error: {err}")
            }
            Self::PeerMessage(err) => {
                write!(f, "simple-taproot asset channel peer message error: {err}")
            }
            Self::Commitment(err) => {
                write!(f, "simple-taproot asset channel commitment error: {err}")
            }
            Self::Payment(err) => write!(f, "simple-taproot asset channel payment error: {err}"),
            Self::Htlc(err) => write!(f, "simple-taproot asset channel HTLC error: {err}"),
            Self::Close(err) => write!(f, "simple-taproot asset channel close error: {err}"),
            Self::Baseline(err) => {
                write!(f, "simple-taproot asset channel BTC baseline error: {err}")
            }
            Self::LdkChannelState(err) => {
                write!(f, "LDK simple-taproot asset channel state error: {err:?}")
            }
            Self::LdkCloseAllocation(err) => {
                write!(f, "LDK asset close allocation error: {err:?}")
            }
            Self::LdkProofOwnership(err) => {
                write!(f, "LDK asset proof ownership error: {err:?}")
            }
            Self::Json(err) => write!(f, "simple-taproot asset channel JSON error: {err}"),
            Self::Invariant(message) => {
                write!(
                    f,
                    "simple-taproot asset channel invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for SimpleTaprootAssetChannelIntegrationError {}

impl From<AssetError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: AssetError) -> Self {
        Self::Asset(err)
    }
}

impl From<AssetChannelFundingError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: AssetChannelFundingError) -> Self {
        Self::Funding(err)
    }
}

impl From<NegotiationError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: NegotiationError) -> Self {
        Self::Negotiation(err)
    }
}

impl From<AssetPeerMessageError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: AssetPeerMessageError) -> Self {
        Self::PeerMessage(err)
    }
}

impl From<AssetCommitmentError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: AssetCommitmentError) -> Self {
        Self::Commitment(err)
    }
}

impl From<NativeAssetPaymentError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: NativeAssetPaymentError) -> Self {
        Self::Payment(err)
    }
}

impl From<AssetHtlcError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: AssetHtlcError) -> Self {
        Self::Htlc(err)
    }
}

impl From<NativeAssetCloseError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: NativeAssetCloseError) -> Self {
        Self::Close(err)
    }
}

impl From<BaselineLdkError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: BaselineLdkError) -> Self {
        Self::Baseline(err)
    }
}

impl From<TaprootAssetChannelStateError> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: TaprootAssetChannelStateError) -> Self {
        Self::LdkChannelState(err)
    }
}

impl From<serde_json::Error> for SimpleTaprootAssetChannelIntegrationError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

fn parse_channel_id(
    channel_id: &str,
) -> Result<ChannelId, SimpleTaprootAssetChannelIntegrationError> {
    let bytes = Bytes32::from_str(channel_id)?;
    Ok(ChannelId::from_bytes(bytes.0))
}

fn close_script_key(seed: u8) -> Result<CompressedKey, SimpleTaprootAssetChannelIntegrationError> {
    let prefix = if seed % 2 == 0 { "02" } else { "03" };
    format!("{prefix}{:064}", seed)
        .parse::<CompressedKey>()
        .map_err(SimpleTaprootAssetChannelIntegrationError::Asset)
}

fn roundtrip<T>(value: &T) -> Result<T, SimpleTaprootAssetChannelIntegrationError>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_slice(&serde_json::to_vec_pretty(value)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_taproot_asset_channel_integration_smoke_runs() {
        let report = run_simple_taproot_asset_channel_integration_smoke()
            .expect("simple-taproot asset channel integration smoke runs");

        assert!(report.negotiated_simple_taproot_asset_channel);
        assert!(report.proof_exchange_separate_from_open_channel);
        assert!(report.funding_hook_approved);
        assert!(report.initial_monitor_aux_persisted);
        assert!(report.missing_monitor_update_rejected);
        assert!(report.ldk_state_advanced_with_monitor_aux);
        assert!(report.payment_settled);
        assert!(report.commitment_proof_history_replayed);
        assert!(report.restart_reestablish_survived);
        assert!(report.cooperative_close_exported);
        assert!(report.cooperative_close_allocation_validated_by_ldk);
        assert!(report.cooperative_close_preserved_latest_asset_allocation);
        assert!(report.cooperative_close_restart_preserved_asset_allocation);
        assert!(report.force_close_proof_ownership_validated_by_ldk);
        assert!(report.btc_only_baseline_unaffected);
        assert_eq!(
            report.sender_balance_after,
            report.sender_balance_before - 125
        );
        assert_eq!(
            report.receiver_balance_after,
            report.receiver_balance_before + 125
        );
    }
}
