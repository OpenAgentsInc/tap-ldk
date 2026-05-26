use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use lightning::{
    ln::{
        taproot_asset::{
            SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION, TaprootAssetChannelDescriptor,
            TaprootAssetCloseAllocation, TaprootAssetCloseAllocationError,
            TaprootAssetCloseAllocationExpectation, prepare_cooperative_close_asset_allocation,
            validate_cooperative_close_asset_allocation,
        },
        types::ChannelId,
    },
    types::features::{ChannelTypeFeatures, InitFeatures},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{AssetAmount, AssetError, AssetLeaf, Bytes32, CompressedKey, derive_hash_sum_root},
    asset_channel_funding::AssetChannelFundingError,
    asset_commitment::{AssetCommitmentChannelState, AssetCommitmentError, AssetCommitmentStore},
    asset_htlc::{AssetHtlcError, AssetHtlcStore},
    asset_payment::{
        NativeAssetPaymentError, NativeAssetPaymentRequest, NativeAssetPaymentStore,
        send_native_asset_payment,
    },
    proof::{ProofError, ProofFile, VerificationScope},
    rfq_invoice::RfqInvoiceError,
    rfq_quote_store::RfqQuoteStore,
    wallet::{ImportOutcome, WalletError, WalletState},
};

pub const NATIVE_ASSET_CLOSE_STORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAssetCloseStatus {
    CooperativeClosed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeForceCloseStatus {
    Deferred,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeSweepRecoveryStatus {
    NotAttempted,
    Failed,
    Recovered,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseOwnerSide {
    Local,
    Remote,
}

impl CloseOwnerSide {
    fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetCloseStore {
    pub version: u32,
    pub closes: BTreeMap<String, NativeAssetClose>,
}

impl Default for NativeAssetCloseStore {
    fn default() -> Self {
        Self {
            version: NATIVE_ASSET_CLOSE_STORE_SCHEMA_VERSION,
            closes: BTreeMap::new(),
        }
    }
}

impl NativeAssetCloseStore {
    pub fn record_close(
        &mut self,
        close: NativeAssetClose,
    ) -> Result<NativeAssetClose, NativeAssetCloseError> {
        if self.closes.contains_key(&close.close_id) {
            return Err(NativeAssetCloseError::DuplicateClose(close.close_id));
        }
        let mut next = self.clone();
        next.closes.insert(close.close_id.clone(), close.clone());
        next.validate()?;
        *self = next;
        Ok(close)
    }

    pub fn inspect_close(&self, close_id: &str) -> Result<NativeAssetClose, NativeAssetCloseError> {
        self.closes
            .get(close_id)
            .cloned()
            .ok_or_else(|| NativeAssetCloseError::UnknownClose(close_id.to_owned()))
    }

    pub fn validate(&self) -> Result<(), NativeAssetCloseError> {
        if self.version != NATIVE_ASSET_CLOSE_STORE_SCHEMA_VERSION {
            return Err(NativeAssetCloseError::UnsupportedVersion(self.version));
        }
        for (close_id, close) in &self.closes {
            if close_id != &close.close_id {
                return Err(NativeAssetCloseError::StorageInvariant(format!(
                    "close map key {close_id} does not match close_id {}",
                    close.close_id
                )));
            }
            close.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetClose {
    pub close_id: String,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_amount: u64,
    pub remote_amount: u64,
    pub total_amount: u64,
    pub proof_root_hash: Bytes32,
    pub proof_root_sum: u64,
    pub local_script_key: CompressedKey,
    pub remote_script_key: CompressedKey,
    pub local_proof_tlv_hex: String,
    pub remote_proof_tlv_hex: String,
    pub local_proof_digest: Bytes32,
    pub remote_proof_digest: Bytes32,
    pub ldk_close_allocation_digest: Bytes32,
    pub status: NativeAssetCloseStatus,
    pub force_close_status: NativeForceCloseStatus,
    pub sweep_recovery_status: NativeSweepRecoveryStatus,
    pub close_digest: Bytes32,
}

impl NativeAssetClose {
    pub fn local_proof(&self) -> Result<ProofFile, NativeAssetCloseError> {
        decode_close_proof(&self.local_proof_tlv_hex)
    }

    pub fn remote_proof(&self) -> Result<ProofFile, NativeAssetCloseError> {
        decode_close_proof(&self.remote_proof_tlv_hex)
    }

    fn validate(&self) -> Result<(), NativeAssetCloseError> {
        if self
            .local_amount
            .checked_add(self.remote_amount)
            .ok_or(NativeAssetCloseError::BalanceOverflow)?
            != self.total_amount
        {
            return Err(NativeAssetCloseError::BalanceNotConserved {
                local_amount: self.local_amount,
                remote_amount: self.remote_amount,
                total_amount: self.total_amount,
            });
        }
        if self.status != NativeAssetCloseStatus::CooperativeClosed {
            return Err(NativeAssetCloseError::StorageInvariant(
                "close is not cooperatively closed".to_owned(),
            ));
        }
        if self.force_close_status != NativeForceCloseStatus::Deferred
            && self.sweep_recovery_status == NativeSweepRecoveryStatus::Failed
        {
            return Err(NativeAssetCloseError::FailedSweepReportedRecovered);
        }
        let local_proof = self.local_proof()?;
        let remote_proof = self.remote_proof()?;
        validate_close_proof(self, CloseOwnerSide::Local, &local_proof)?;
        validate_close_proof(self, CloseOwnerSide::Remote, &remote_proof)?;
        validate_ldk_close_allocation(self, self.commitment_number)?;
        if self.close_digest
            != close_digest(
                &self.channel_id,
                self.asset_id,
                self.commitment_number,
                self.local_amount,
                self.remote_amount,
                &self.local_proof_tlv_hex,
                &self.remote_proof_tlv_hex,
                self.ldk_close_allocation_digest,
            )
        {
            return Err(NativeAssetCloseError::StorageInvariant(
                "cooperative close digest mismatch".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetCloseSmokeReport {
    pub close_id: String,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_amount: u64,
    pub remote_amount: u64,
    pub total_amount: u64,
    pub ldk_close_allocation_digest: Bytes32,
    pub local_proof_import_status: String,
    pub remote_proof_import_status: String,
    pub local_proof_tlv_hex: String,
    pub remote_proof_tlv_hex: String,
    pub local_wallet_balance: u64,
    pub remote_wallet_balance: u64,
    pub restart_after_close_matches: bool,
    pub obsolete_proof_rejected: bool,
    pub force_close_status: NativeForceCloseStatus,
    pub failed_sweep_not_reported_recovered: bool,
}

pub fn cooperative_close(
    store: &AssetCommitmentStore,
    channel_id: &str,
    local_script_key: CompressedKey,
    remote_script_key: CompressedKey,
) -> Result<NativeAssetClose, NativeAssetCloseError> {
    let state = store.channel_state(channel_id)?;
    close_from_state(
        &state,
        local_script_key,
        remote_script_key,
        state.latest_commitment_number,
    )
}

pub fn run_native_asset_close_smoke() -> Result<NativeAssetCloseSmokeReport, NativeAssetCloseError>
{
    let (commitment_store, state) = initialized_settled_commitment_store()?;
    let local_script_key = close_script_key(2)?;
    let remote_script_key = close_script_key(3)?;
    let close = cooperative_close(
        &commitment_store,
        &state.channel_id,
        local_script_key,
        remote_script_key,
    )?;
    let mut close_store = NativeAssetCloseStore::default();
    let close = close_store.record_close(close)?;
    let close_store = roundtrip(&close_store)?;
    let recovered_close = close_store.inspect_close(&close.close_id)?;

    let mut local_wallet = WalletState::default();
    let local_import =
        local_wallet.import_encoded_proof(&decode_hex(&close.local_proof_tlv_hex)?)?;
    let mut remote_wallet = WalletState::default();
    let remote_import =
        remote_wallet.import_encoded_proof(&decode_hex(&close.remote_proof_tlv_hex)?)?;
    let local_wallet = roundtrip(&local_wallet)?;
    let remote_wallet = roundtrip(&remote_wallet)?;
    let local_wallet_balance = wallet_balance(&local_wallet, close.asset_id)?;
    let remote_wallet_balance = wallet_balance(&remote_wallet, close.asset_id)?;

    let stale_state = stale_commitment_view(&state)?;
    let stale_local_proof = build_close_proof(
        &stale_state,
        CloseOwnerSide::Local,
        close.local_script_key,
        stale_state.local_balance,
    )?;
    let obsolete_proof_rejected =
        validate_close_proof(&recovered_close, CloseOwnerSide::Local, &stale_local_proof).is_err();

    let failed_sweep_not_reported_recovered =
        !matches!(failed_sweep_gate(), NativeSweepRecoveryStatus::Recovered);

    commitment_store.validate()?;
    let restart_after_close_matches = recovered_close == close
        && local_wallet_balance == close.local_amount
        && remote_wallet_balance == close.remote_amount;

    Ok(NativeAssetCloseSmokeReport {
        close_id: close.close_id,
        channel_id: close.channel_id,
        asset_id: close.asset_id,
        commitment_number: close.commitment_number,
        local_amount: close.local_amount,
        remote_amount: close.remote_amount,
        total_amount: close.total_amount,
        ldk_close_allocation_digest: close.ldk_close_allocation_digest,
        local_proof_import_status: import_status(local_import).to_owned(),
        remote_proof_import_status: import_status(remote_import).to_owned(),
        local_proof_tlv_hex: close.local_proof_tlv_hex,
        remote_proof_tlv_hex: close.remote_proof_tlv_hex,
        local_wallet_balance,
        remote_wallet_balance,
        restart_after_close_matches,
        obsolete_proof_rejected,
        force_close_status: close.force_close_status,
        failed_sweep_not_reported_recovered,
    })
}

fn close_from_state(
    state: &AssetCommitmentChannelState,
    local_script_key: CompressedKey,
    remote_script_key: CompressedKey,
    latest_safe_commitment_number: u64,
) -> Result<NativeAssetClose, NativeAssetCloseError> {
    let local_proof = build_close_proof(
        state,
        CloseOwnerSide::Local,
        local_script_key,
        state.local_balance,
    )?;
    let remote_proof = build_close_proof(
        state,
        CloseOwnerSide::Remote,
        remote_script_key,
        state.remote_balance,
    )?;
    let local_proof_tlv_hex = encode_hex(&local_proof.encode()?);
    let remote_proof_tlv_hex = encode_hex(&remote_proof.encode()?);
    let local_proof_digest = proof_handoff_digest(CloseOwnerSide::Local, &local_proof_tlv_hex);
    let remote_proof_digest = proof_handoff_digest(CloseOwnerSide::Remote, &remote_proof_tlv_hex);
    let ldk_allocation = TaprootAssetCloseAllocation::new(
        parse_channel_id(&state.channel_id)?,
        state.asset_id.0,
        state.latest_commitment_number,
        state.local_balance,
        state.remote_balance,
        state.monitor_blob.proof_root_hash.0,
        state.monitor_blob.proof_root_sum,
        local_proof_digest.0,
        remote_proof_digest.0,
    )
    .map_err(NativeAssetCloseError::LdkCloseAllocation)?;
    let close_id = close_id(state);
    let close_digest = close_digest(
        &state.channel_id,
        state.asset_id,
        state.latest_commitment_number,
        state.local_balance,
        state.remote_balance,
        &local_proof_tlv_hex,
        &remote_proof_tlv_hex,
        Bytes32(ldk_allocation.allocation_digest),
    );

    let close = NativeAssetClose {
        close_id,
        channel_id: state.channel_id.clone(),
        asset_id: state.asset_id,
        commitment_number: state.latest_commitment_number,
        local_amount: state.local_balance,
        remote_amount: state.remote_balance,
        total_amount: state.total_amount,
        proof_root_hash: state.monitor_blob.proof_root_hash,
        proof_root_sum: state.monitor_blob.proof_root_sum,
        local_script_key,
        remote_script_key,
        local_proof_tlv_hex,
        remote_proof_tlv_hex,
        local_proof_digest,
        remote_proof_digest,
        ldk_close_allocation_digest: Bytes32(ldk_allocation.allocation_digest),
        status: NativeAssetCloseStatus::CooperativeClosed,
        force_close_status: NativeForceCloseStatus::Deferred,
        sweep_recovery_status: NativeSweepRecoveryStatus::NotAttempted,
        close_digest,
    };
    validate_ldk_close_allocation(&close, latest_safe_commitment_number)?;
    close.validate()?;
    Ok(close)
}

fn build_close_proof(
    state: &AssetCommitmentChannelState,
    owner: CloseOwnerSide,
    script_key: CompressedKey,
    amount: u64,
) -> Result<ProofFile, NativeAssetCloseError> {
    if amount == 0 {
        return Err(NativeAssetCloseError::ZeroCloseAmount(owner));
    }
    let asset_amount = AssetAmount::new(amount);
    let tap_asset_root = derive_hash_sum_root(&[AssetLeaf {
        asset_id: state.asset_id,
        script_key,
        amount: asset_amount,
    }])?;
    Ok(ProofFile {
        version: 0,
        asset_id: state.asset_id,
        genesis_outpoint: close_genesis_outpoint(state.asset_id),
        anchor_outpoint: close_anchor_outpoint(
            &state.channel_id,
            owner,
            state.latest_commitment_number,
            state.asset_id,
            amount,
            script_key,
        ),
        amount: asset_amount,
        script_key,
        tap_asset_root,
        verification_scope: VerificationScope::BoundedAnchorOnly,
    })
}

fn validate_close_proof(
    close: &NativeAssetClose,
    owner: CloseOwnerSide,
    proof: &ProofFile,
) -> Result<(), NativeAssetCloseError> {
    proof.verify_bounded_anchor()?;
    let (expected_amount, expected_script_key) = match owner {
        CloseOwnerSide::Local => (close.local_amount, close.local_script_key),
        CloseOwnerSide::Remote => (close.remote_amount, close.remote_script_key),
    };
    let expected_anchor = close_anchor_outpoint(
        &close.channel_id,
        owner,
        close.commitment_number,
        close.asset_id,
        expected_amount,
        expected_script_key,
    );
    if proof.asset_id != close.asset_id
        || proof.amount.value() != expected_amount
        || proof.script_key != expected_script_key
        || proof.anchor_outpoint != expected_anchor
        || proof.genesis_outpoint != close_genesis_outpoint(close.asset_id)
    {
        return Err(NativeAssetCloseError::ProofDoesNotMatchClose(owner));
    }
    Ok(())
}

fn validate_ldk_close_allocation(
    close: &NativeAssetClose,
    latest_safe_commitment_number: u64,
) -> Result<(), NativeAssetCloseError> {
    let allocation = TaprootAssetCloseAllocation::new(
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
    .map_err(NativeAssetCloseError::LdkCloseAllocation)?;
    let expected = TaprootAssetCloseAllocationExpectation {
        channel_id: parse_channel_id(&close.channel_id)?,
        asset_id: close.asset_id.0,
        latest_commitment_number: latest_safe_commitment_number,
        local_amount: close.local_amount,
        remote_amount: close.remote_amount,
        proof_root_hash: close.proof_root_hash.0,
        proof_root_sum: close.proof_root_sum,
    };
    validate_cooperative_close_asset_allocation(Some(&allocation), &expected)
        .map_err(NativeAssetCloseError::LdkCloseAllocation)?;
    let mut features = InitFeatures::empty();
    features.set_static_remote_key_optional();
    features.set_channel_type_optional();
    features.set_taproot_asset_channel_optional();
    let descriptor = TaprootAssetChannelDescriptor::new(
        close.asset_id.0,
        SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
    )
    .map_err(|_| {
        NativeAssetCloseError::LdkCloseAllocation(TaprootAssetCloseAllocationError::AssetIdMismatch)
    })?;
    prepare_cooperative_close_asset_allocation(
        &features,
        &features,
        &ChannelTypeFeatures::taproot_asset_single_asset(),
        descriptor,
        allocation,
    )
    .map_err(NativeAssetCloseError::LdkCloseAllocation)?;
    if close.ldk_close_allocation_digest.0 != allocation.allocation_digest {
        return Err(NativeAssetCloseError::LdkCloseAllocation(
            TaprootAssetCloseAllocationError::AllocationDigestMismatch,
        ));
    }
    Ok(())
}

fn initialized_settled_commitment_store()
-> Result<(AssetCommitmentStore, AssetCommitmentChannelState), NativeAssetCloseError> {
    let (channel_store, report) = crate::asset_channel_funding::run_asset_channel_funding_smoke()?;
    let funded = channel_store
        .channels
        .get(&report.channel_id)
        .ok_or_else(|| AssetCommitmentError::UnknownChannel(report.channel_id.clone()))?;
    let mut commitment_store = AssetCommitmentStore::default();
    let state = commitment_store.initialize_channel(funded)?;
    let mut htlc_store = AssetHtlcStore::default();
    let mut payment_store = NativeAssetPaymentStore::default();
    let mut rfq_store = RfqQuoteStore::default();
    let request = NativeAssetPaymentRequest {
        channel_id: state.channel_id.clone(),
        sender_peer: "alice".to_owned(),
        receiver_peer: "bob".to_owned(),
        asset_id: state.asset_id,
        asset_amount: 125,
        rfq_id: Bytes32([101; 32]),
        invoice_context: Bytes32([102; 32]),
        payment_hash: Bytes32([103; 32]),
        asset_nonce: Bytes32([104; 32]),
        now_unix_seconds: 1_000,
    };
    send_native_asset_payment(
        &mut commitment_store,
        &mut htlc_store,
        &mut payment_store,
        &mut rfq_store,
        request,
    )?;
    let state = commitment_store.channel_state(&state.channel_id)?;
    Ok((commitment_store, state))
}

fn stale_commitment_view(
    latest: &AssetCommitmentChannelState,
) -> Result<AssetCommitmentChannelState, NativeAssetCloseError> {
    let mut stale = latest.clone();
    stale.latest_commitment_number = latest
        .latest_commitment_number
        .checked_sub(1)
        .ok_or(NativeAssetCloseError::MissingStaleCommitment)?;
    stale.local_balance = latest
        .local_balance
        .checked_add(125)
        .ok_or(NativeAssetCloseError::BalanceOverflow)?;
    stale.remote_balance = latest
        .remote_balance
        .checked_sub(125)
        .ok_or(NativeAssetCloseError::BalanceOverflow)?;
    Ok(stale)
}

fn failed_sweep_gate() -> NativeSweepRecoveryStatus {
    NativeSweepRecoveryStatus::Failed
}

fn close_script_key(seed: u8) -> Result<CompressedKey, NativeAssetCloseError> {
    let prefix = if seed % 2 == 0 { "02" } else { "03" };
    format!("{prefix}{:064}", seed)
        .parse::<CompressedKey>()
        .map_err(NativeAssetCloseError::Asset)
}

fn wallet_balance(wallet: &WalletState, asset_id: Bytes32) -> Result<u64, NativeAssetCloseError> {
    Ok(wallet
        .balances()?
        .into_iter()
        .find(|balance| balance.asset_id == asset_id.to_hex())
        .map(|balance| balance.spendable)
        .unwrap_or(0))
}

fn import_status(outcome: ImportOutcome) -> &'static str {
    match outcome {
        ImportOutcome::Imported { .. } => "imported",
        ImportOutcome::AlreadyPresent { .. } => "already_present",
    }
}

fn decode_close_proof(hex: &str) -> Result<ProofFile, NativeAssetCloseError> {
    let encoded = decode_hex(hex)?;
    ProofFile::decode(&encoded).map_err(NativeAssetCloseError::Proof)
}

fn close_id(state: &AssetCommitmentChannelState) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-asset-close-id:v1");
    hasher.update((state.channel_id.len() as u64).to_be_bytes());
    hasher.update(state.channel_id.as_bytes());
    hasher.update(state.asset_id.0);
    hasher.update(state.latest_commitment_number.to_be_bytes());
    hasher.update(state.local_balance.to_be_bytes());
    hasher.update(state.remote_balance.to_be_bytes());
    Bytes32(hasher.finalize().into()).to_hex()
}

fn close_digest(
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    local_amount: u64,
    remote_amount: u64,
    local_proof_tlv_hex: &str,
    remote_proof_tlv_hex: &str,
    ldk_close_allocation_digest: Bytes32,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-asset-close-digest:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(local_amount.to_be_bytes());
    hasher.update(remote_amount.to_be_bytes());
    hasher.update(ldk_close_allocation_digest.0);
    hasher.update((local_proof_tlv_hex.len() as u64).to_be_bytes());
    hasher.update(local_proof_tlv_hex.as_bytes());
    hasher.update((remote_proof_tlv_hex.len() as u64).to_be_bytes());
    hasher.update(remote_proof_tlv_hex.as_bytes());
    Bytes32(hasher.finalize().into())
}

fn proof_handoff_digest(owner: CloseOwnerSide, proof_tlv_hex: &str) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-close-proof-handoff:v1");
    hasher.update(owner.as_str().as_bytes());
    hasher.update((proof_tlv_hex.len() as u64).to_be_bytes());
    hasher.update(proof_tlv_hex.as_bytes());
    Bytes32(hasher.finalize().into())
}

fn parse_channel_id(channel_id: &str) -> Result<ChannelId, NativeAssetCloseError> {
    let bytes = Bytes32::from_str(channel_id).map_err(NativeAssetCloseError::Asset)?;
    Ok(ChannelId::from_bytes(bytes.0))
}

fn close_genesis_outpoint(asset_id: Bytes32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-close-genesis:v1");
    hasher.update(asset_id.0);
    format!("{}:0", Bytes32(hasher.finalize().into()).to_hex())
}

fn close_anchor_outpoint(
    channel_id: &str,
    owner: CloseOwnerSide,
    commitment_number: u64,
    asset_id: Bytes32,
    amount: u64,
    script_key: CompressedKey,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-close-anchor:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(owner.as_str().as_bytes());
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(asset_id.0);
    hasher.update(amount.to_be_bytes());
    hasher.update(script_key.0);
    format!(
        "{}:{}",
        Bytes32(hasher.finalize().into()).to_hex(),
        owner as u8
    )
}

fn roundtrip<T>(value: &T) -> Result<T, NativeAssetCloseError>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    Ok(serde_json::from_slice(&serde_json::to_vec_pretty(value)?)?)
}

fn encode_hex(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(CHARS[(byte >> 4) as usize] as char);
        out.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, NativeAssetCloseError> {
    if hex.len() % 2 != 0 {
        return Err(NativeAssetCloseError::InvalidHexLength);
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks(2) {
        let high = decode_hex_nibble(pair[0])?;
        let low = decode_hex_nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, NativeAssetCloseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        other => Err(NativeAssetCloseError::InvalidHexByte(
            (other as char).to_string(),
        )),
    }
}

#[derive(Debug)]
pub enum NativeAssetCloseError {
    Json(serde_json::Error),
    Asset(AssetError),
    Proof(ProofError),
    Wallet(WalletError),
    Funding(AssetChannelFundingError),
    Commitment(AssetCommitmentError),
    Payment(NativeAssetPaymentError),
    Htlc(AssetHtlcError),
    Rfq(RfqInvoiceError),
    LdkCloseAllocation(TaprootAssetCloseAllocationError),
    UnsupportedVersion(u32),
    DuplicateClose(String),
    UnknownClose(String),
    MissingStaleCommitment,
    ZeroCloseAmount(CloseOwnerSide),
    ProofDoesNotMatchClose(CloseOwnerSide),
    FailedSweepReportedRecovered,
    InvalidHexLength,
    InvalidHexByte(String),
    BalanceOverflow,
    BalanceNotConserved {
        local_amount: u64,
        remote_amount: u64,
        total_amount: u64,
    },
    StorageInvariant(String),
}

impl fmt::Display for NativeAssetCloseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "native asset close JSON error: {err}"),
            Self::Asset(err) => write!(f, "native asset close asset error: {err}"),
            Self::Proof(err) => write!(f, "native asset close proof error: {err}"),
            Self::Wallet(err) => write!(f, "native asset close wallet error: {err}"),
            Self::Funding(err) => write!(f, "native asset close funding error: {err}"),
            Self::Commitment(err) => write!(f, "native asset close commitment error: {err}"),
            Self::Payment(err) => write!(f, "native asset close payment error: {err}"),
            Self::Htlc(err) => write!(f, "native asset close HTLC error: {err}"),
            Self::Rfq(err) => write!(f, "native asset close RFQ error: {err}"),
            Self::LdkCloseAllocation(err) => {
                write!(f, "LDK asset close allocation rejected close: {err:?}")
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported native asset close version {version}")
            }
            Self::DuplicateClose(close_id) => write!(f, "duplicate native asset close {close_id}"),
            Self::UnknownClose(close_id) => write!(f, "unknown native asset close {close_id}"),
            Self::MissingStaleCommitment => write!(f, "missing stale commitment view"),
            Self::ZeroCloseAmount(owner) => {
                write!(f, "native asset close {owner:?} amount is zero")
            }
            Self::ProofDoesNotMatchClose(owner) => {
                write!(f, "native asset close proof does not match {owner:?} owner")
            }
            Self::FailedSweepReportedRecovered => {
                write!(f, "native asset close failed sweep reported as recovered")
            }
            Self::InvalidHexLength => write!(f, "native asset close hex value has odd length"),
            Self::InvalidHexByte(value) => {
                write!(f, "invalid native asset close hex byte: {value}")
            }
            Self::BalanceOverflow => write!(f, "native asset close balance overflowed"),
            Self::BalanceNotConserved {
                local_amount,
                remote_amount,
                total_amount,
            } => write!(
                f,
                "native asset close local={local_amount} remote={remote_amount} do not conserve total={total_amount}"
            ),
            Self::StorageInvariant(message) => {
                write!(f, "native asset close storage invariant failed: {message}")
            }
        }
    }
}

impl Error for NativeAssetCloseError {}

impl From<serde_json::Error> for NativeAssetCloseError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<AssetError> for NativeAssetCloseError {
    fn from(err: AssetError) -> Self {
        Self::Asset(err)
    }
}

impl From<ProofError> for NativeAssetCloseError {
    fn from(err: ProofError) -> Self {
        Self::Proof(err)
    }
}

impl From<WalletError> for NativeAssetCloseError {
    fn from(err: WalletError) -> Self {
        Self::Wallet(err)
    }
}

impl From<AssetChannelFundingError> for NativeAssetCloseError {
    fn from(err: AssetChannelFundingError) -> Self {
        Self::Funding(err)
    }
}

impl From<AssetCommitmentError> for NativeAssetCloseError {
    fn from(err: AssetCommitmentError) -> Self {
        Self::Commitment(err)
    }
}

impl From<NativeAssetPaymentError> for NativeAssetCloseError {
    fn from(err: NativeAssetPaymentError) -> Self {
        Self::Payment(err)
    }
}

impl From<AssetHtlcError> for NativeAssetCloseError {
    fn from(err: AssetHtlcError) -> Self {
        Self::Htlc(err)
    }
}

impl From<RfqInvoiceError> for NativeAssetCloseError {
    fn from(err: RfqInvoiceError) -> Self {
        Self::Rfq(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooperative_close_smoke_exports_and_imports_latest_proofs() {
        let report = run_native_asset_close_smoke().expect("close smoke passes");
        assert_eq!(report.local_amount, 575);
        assert_eq!(report.remote_amount, 425);
        assert_eq!(report.total_amount, 1_000);
        assert_eq!(report.local_wallet_balance, 575);
        assert_eq!(report.remote_wallet_balance, 425);
        assert_eq!(report.local_proof_import_status, "imported");
        assert_eq!(report.remote_proof_import_status, "imported");
        assert_ne!(report.ldk_close_allocation_digest, Bytes32([0; 32]));
        assert!(report.restart_after_close_matches);
        assert!(report.obsolete_proof_rejected);
        assert_eq!(report.force_close_status, NativeForceCloseStatus::Deferred);
        assert!(report.failed_sweep_not_reported_recovered);
    }

    #[test]
    fn cooperative_close_uses_latest_commitment_view() {
        let (commitment_store, state) =
            initialized_settled_commitment_store().expect("settled state");
        let close = cooperative_close(
            &commitment_store,
            &state.channel_id,
            close_script_key(2).expect("local key"),
            close_script_key(3).expect("remote key"),
        )
        .expect("close succeeds");
        assert_eq!(close.commitment_number, state.latest_commitment_number);
        assert_eq!(close.local_amount, state.local_balance);
        assert_eq!(close.remote_amount, state.remote_balance);
    }

    #[test]
    fn stale_close_allocation_fails_before_close_is_built() {
        let (_commitment_store, state) =
            initialized_settled_commitment_store().expect("settled state");
        let stale = stale_commitment_view(&state).expect("stale state");
        assert!(matches!(
            close_from_state(
                &stale,
                close_script_key(2).expect("local key"),
                close_script_key(3).expect("remote key"),
                state.latest_commitment_number,
            ),
            Err(NativeAssetCloseError::LdkCloseAllocation(
                TaprootAssetCloseAllocationError::CommitmentNumberMismatch
            ))
        ));
    }

    #[test]
    fn missing_or_tampered_close_allocation_fails_validation() {
        let (commitment_store, state) =
            initialized_settled_commitment_store().expect("settled state");
        let mut close = cooperative_close(
            &commitment_store,
            &state.channel_id,
            close_script_key(2).expect("local key"),
            close_script_key(3).expect("remote key"),
        )
        .expect("close succeeds");
        close.ldk_close_allocation_digest = Bytes32([0; 32]);
        assert!(matches!(
            close.validate(),
            Err(NativeAssetCloseError::LdkCloseAllocation(
                TaprootAssetCloseAllocationError::AllocationDigestMismatch
            ))
        ));

        let mut close = cooperative_close(
            &commitment_store,
            &state.channel_id,
            close_script_key(2).expect("local key"),
            close_script_key(3).expect("remote key"),
        )
        .expect("close succeeds");
        close.proof_root_hash = Bytes32([99; 32]);
        assert!(matches!(
            close.validate(),
            Err(NativeAssetCloseError::LdkCloseAllocation(
                TaprootAssetCloseAllocationError::AllocationDigestMismatch
            ))
        ));
    }

    #[test]
    fn failed_sweep_gate_is_not_reported_recovered() {
        assert_eq!(failed_sweep_gate(), NativeSweepRecoveryStatus::Failed);
        assert!(!matches!(
            failed_sweep_gate(),
            NativeSweepRecoveryStatus::Recovered
        ));
    }
}
