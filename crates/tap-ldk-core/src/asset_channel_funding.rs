use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use lightning::{
    bitcoin::{
        hash_types::Txid,
        secp256k1::{PublicKey, Secp256k1, SecretKey},
    },
    chain::transaction::OutPoint,
    ln::{
        taproot_asset::{
            self, TaprootAssetChannelNegotiationError, TaprootAssetFundingAllocation,
            TaprootAssetFundingError, TaprootAssetFundingExpectations, TaprootAssetFundingOutput,
            TaprootAssetFundingProofMaterial,
            TaprootAssetFundingRequest as LdkTaprootAssetFundingRequest,
        },
        types::ChannelId,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{
        AssetAmount, AssetError, AssetLeaf, Bytes32, CompressedKey, RootHashSum,
        derive_hash_sum_root,
    },
    proof::{ProofError, ProofFile},
    wallet::{RegtestIssueRequest, WalletError, WalletState},
};

pub const ASSET_CHANNEL_STORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelStore {
    pub version: u32,
    pub metadata: AssetChannelStoreMetadata,
    pub channels: BTreeMap<String, StoredAssetChannel>,
    pub spent_funding_proofs: BTreeMap<String, String>,
}

impl Default for AssetChannelStore {
    fn default() -> Self {
        Self {
            version: ASSET_CHANNEL_STORE_SCHEMA_VERSION,
            metadata: AssetChannelStoreMetadata::default(),
            channels: BTreeMap::new(),
            spent_funding_proofs: BTreeMap::new(),
        }
    }
}

impl AssetChannelStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AssetChannelFundingError> {
        let raw = fs::read_to_string(path.as_ref()).map_err(AssetChannelFundingError::Io)?;
        let store = serde_json::from_str::<Self>(&raw).map_err(AssetChannelFundingError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), AssetChannelFundingError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(AssetChannelFundingError::Io)?;
            }
        }

        let raw = serde_json::to_vec_pretty(self).map_err(AssetChannelFundingError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(AssetChannelFundingError::Io)?;
        fs::rename(&temp_path, path).map_err(AssetChannelFundingError::Io)?;
        Ok(())
    }

    pub fn fund_channel(
        &mut self,
        request: AssetChannelFundingRequest,
    ) -> Result<StoredAssetChannel, AssetChannelFundingError> {
        self.fund_channel_with_fork_commitment_override(request, None)
    }

    fn fund_channel_with_fork_commitment_override(
        &mut self,
        request: AssetChannelFundingRequest,
        output_commitment_override: Option<Bytes32>,
    ) -> Result<StoredAssetChannel, AssetChannelFundingError> {
        validate_funding_request(&request)?;

        let local_inputs = validate_inputs(
            &request.asset_id,
            ChannelSide::Local,
            &request.local_inputs,
            &self.spent_funding_proofs,
        )?;
        let remote_inputs = validate_inputs(
            &request.asset_id,
            ChannelSide::Remote,
            &request.remote_inputs,
            &self.spent_funding_proofs,
        )?;
        let genesis_outpoint = shared_genesis(&local_inputs, &remote_inputs)?;
        let local_balance = sum_inputs(&local_inputs)?;
        let remote_balance = sum_inputs(&remote_inputs)?;
        let total_amount = local_balance
            .checked_add(remote_balance)
            .map_err(AssetChannelFundingError::Asset)?;
        let funding_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: request.asset_id,
            script_key: request.funding_script_key,
            amount: total_amount,
        }])
        .map_err(AssetChannelFundingError::Asset)?;

        if let Some(expected) = request.expected_funding_root {
            if expected != funding_root {
                return Err(AssetChannelFundingError::FundingRootMismatch {
                    expected: StoredRootHashSum::from(expected),
                    actual: StoredRootHashSum::from(funding_root),
                });
            }
        }

        let input_proofs = local_inputs
            .iter()
            .chain(remote_inputs.iter())
            .map(|input| input.proof_id.clone())
            .collect::<Vec<_>>();
        let channel_id = derive_channel_id(
            &request.local_peer,
            &request.remote_peer,
            request.asset_id,
            &request.funding_outpoint,
            request.funding_script_key,
            local_balance.value(),
            remote_balance.value(),
            &input_proofs,
        );
        if self.channels.contains_key(&channel_id) {
            return Err(AssetChannelFundingError::DuplicateChannel(channel_id));
        }

        validate_with_ldk_funding_hook(
            &request,
            &channel_id,
            &genesis_outpoint,
            funding_root,
            local_balance.value(),
            remote_balance.value(),
            input_proofs.len(),
            output_commitment_override,
        )?;

        let monitor = AssetChannelMonitorBlob::new(
            &channel_id,
            request.asset_id,
            local_balance.value(),
            remote_balance.value(),
            funding_root,
        );
        let channel = StoredAssetChannel {
            channel_id: channel_id.clone(),
            local_peer: request.local_peer,
            remote_peer: request.remote_peer,
            asset_id: request.asset_id,
            genesis_outpoint,
            funding_outpoint: request.funding_outpoint,
            funding_script_key: request.funding_script_key,
            funding_tap_asset_root: StoredRootHashSum::from(funding_root),
            local_balance: local_balance.value(),
            remote_balance: remote_balance.value(),
            total_amount: total_amount.value(),
            local_input_proof_ids: local_inputs
                .iter()
                .map(|input| input.proof_id.clone())
                .collect(),
            remote_input_proof_ids: remote_inputs
                .iter()
                .map(|input| input.proof_id.clone())
                .collect(),
            status: AssetChannelFundingStatus::Funded,
            monitor,
        };

        let mut next = self.clone();
        for proof_id in &input_proofs {
            next.spent_funding_proofs
                .insert(proof_id.clone(), channel_id.clone());
        }
        next.channels.insert(channel_id, channel.clone());
        next.validate()?;
        *self = next;

        Ok(channel)
    }

    pub fn channel_balances(
        &self,
        channel_id: &str,
    ) -> Result<AssetChannelBalances, AssetChannelFundingError> {
        let channel = self
            .channels
            .get(channel_id)
            .ok_or_else(|| AssetChannelFundingError::UnknownChannel(channel_id.to_owned()))?;
        Ok(AssetChannelBalances {
            channel_id: channel.channel_id.clone(),
            asset_id: channel.asset_id,
            local_balance: channel.local_balance,
            remote_balance: channel.remote_balance,
            total_amount: channel.total_amount,
        })
    }

    pub fn validate(&self) -> Result<(), AssetChannelFundingError> {
        if self.version != ASSET_CHANNEL_STORE_SCHEMA_VERSION {
            return Err(AssetChannelFundingError::UnsupportedVersion(self.version));
        }

        let mut spent = BTreeMap::<String, String>::new();
        for (channel_id, channel) in &self.channels {
            if channel_id != &channel.channel_id {
                return Err(AssetChannelFundingError::StorageInvariant(format!(
                    "channel map key {channel_id} does not match channel_id {}",
                    channel.channel_id
                )));
            }
            channel.validate_fields()?;
            if channel.binding_id() != *channel_id {
                return Err(AssetChannelFundingError::StorageInvariant(format!(
                    "channel {channel_id} binding hash does not match stored fields"
                )));
            }
            if !channel.monitor.persisted {
                return Err(AssetChannelFundingError::MonitorNotPersisted(
                    channel_id.clone(),
                ));
            }
            if channel.monitor.blob_digest != channel.expected_monitor_digest() {
                return Err(AssetChannelFundingError::StorageInvariant(format!(
                    "channel {channel_id} monitor digest does not match stored fields"
                )));
            }

            for proof_id in channel
                .local_input_proof_ids
                .iter()
                .chain(channel.remote_input_proof_ids.iter())
            {
                if spent.insert(proof_id.clone(), channel_id.clone()).is_some() {
                    return Err(AssetChannelFundingError::StorageInvariant(format!(
                        "funding proof {proof_id} is assigned to multiple channels"
                    )));
                }
            }
        }

        if spent != self.spent_funding_proofs {
            return Err(AssetChannelFundingError::StorageInvariant(
                "spent funding proof index does not match channels".to_owned(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelStoreMetadata {
    pub implementation: String,
    pub schema: String,
}

impl Default for AssetChannelStoreMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk experimental asset channel store".to_owned(),
            schema: "bounded-regtest-asset-channel-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetChannelFundingRequest {
    pub local_peer: String,
    pub remote_peer: String,
    pub asset_id: Bytes32,
    pub funding_outpoint: String,
    pub funding_script_key: CompressedKey,
    pub local_inputs: Vec<ProofFile>,
    pub remote_inputs: Vec<ProofFile>,
    pub expected_funding_root: Option<RootHashSum>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredAssetChannel {
    pub channel_id: String,
    pub local_peer: String,
    pub remote_peer: String,
    pub asset_id: Bytes32,
    pub genesis_outpoint: String,
    pub funding_outpoint: String,
    pub funding_script_key: CompressedKey,
    pub funding_tap_asset_root: StoredRootHashSum,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_amount: u64,
    pub local_input_proof_ids: Vec<String>,
    pub remote_input_proof_ids: Vec<String>,
    pub status: AssetChannelFundingStatus,
    pub monitor: AssetChannelMonitorBlob,
}

impl StoredAssetChannel {
    pub fn binding_id(&self) -> String {
        derive_channel_id(
            &self.local_peer,
            &self.remote_peer,
            self.asset_id,
            &self.funding_outpoint,
            self.funding_script_key,
            self.local_balance,
            self.remote_balance,
            &self
                .local_input_proof_ids
                .iter()
                .chain(self.remote_input_proof_ids.iter())
                .cloned()
                .collect::<Vec<_>>(),
        )
    }

    fn validate_fields(&self) -> Result<(), AssetChannelFundingError> {
        if self.local_peer.trim().is_empty() || self.remote_peer.trim().is_empty() {
            return Err(AssetChannelFundingError::EmptyPeer);
        }
        if self.funding_outpoint.trim().is_empty() {
            return Err(AssetChannelFundingError::MalformedFundingOutpoint);
        }
        if self
            .local_balance
            .checked_add(self.remote_balance)
            .ok_or(AssetChannelFundingError::AmountOverflow)?
            != self.total_amount
        {
            return Err(AssetChannelFundingError::BalanceMismatch {
                local_balance: self.local_balance,
                remote_balance: self.remote_balance,
                total_amount: self.total_amount,
            });
        }
        if self.funding_tap_asset_root.sum != self.total_amount {
            return Err(AssetChannelFundingError::FundingRootSumMismatch {
                root_sum: self.funding_tap_asset_root.sum,
                total_amount: self.total_amount,
            });
        }
        let expected_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: self.asset_id,
            script_key: self.funding_script_key,
            amount: AssetAmount::new(self.total_amount),
        }])
        .map_err(AssetChannelFundingError::Asset)?;
        if self.funding_tap_asset_root != StoredRootHashSum::from(expected_root) {
            return Err(AssetChannelFundingError::FundingRootMismatch {
                expected: StoredRootHashSum::from(expected_root),
                actual: self.funding_tap_asset_root.clone(),
            });
        }
        if self.local_input_proof_ids.is_empty() && self.remote_input_proof_ids.is_empty() {
            return Err(AssetChannelFundingError::MissingFundingProofs);
        }
        Ok(())
    }

    fn expected_monitor_digest(&self) -> Bytes32 {
        AssetChannelMonitorBlob::digest_for(
            &self.channel_id,
            self.asset_id,
            self.local_balance,
            self.remote_balance,
            RootHashSum {
                hash: self.funding_tap_asset_root.hash,
                sum: AssetAmount::new(self.funding_tap_asset_root.sum),
            },
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredRootHashSum {
    pub hash: Bytes32,
    pub sum: u64,
}

impl From<RootHashSum> for StoredRootHashSum {
    fn from(root: RootHashSum) -> Self {
        Self {
            hash: root.hash,
            sum: root.sum.value(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetChannelFundingStatus {
    Funded,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelMonitorBlob {
    pub schema_version: u32,
    pub commitment_number: u64,
    pub persisted: bool,
    pub blob_digest: Bytes32,
}

impl AssetChannelMonitorBlob {
    fn new(
        channel_id: &str,
        asset_id: Bytes32,
        local_balance: u64,
        remote_balance: u64,
        funding_root: RootHashSum,
    ) -> Self {
        Self {
            schema_version: ASSET_CHANNEL_STORE_SCHEMA_VERSION,
            commitment_number: 0,
            persisted: true,
            blob_digest: Self::digest_for(
                channel_id,
                asset_id,
                local_balance,
                remote_balance,
                funding_root,
            ),
        }
    }

    fn digest_for(
        channel_id: &str,
        asset_id: Bytes32,
        local_balance: u64,
        remote_balance: u64,
        funding_root: RootHashSum,
    ) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:asset-channel-monitor:v1");
        hasher.update((channel_id.len() as u64).to_be_bytes());
        hasher.update(channel_id.as_bytes());
        hasher.update(asset_id.0);
        hasher.update(local_balance.to_be_bytes());
        hasher.update(remote_balance.to_be_bytes());
        hasher.update(funding_root.hash.0);
        hasher.update(funding_root.sum.value().to_be_bytes());
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelBalances {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelFundingSmokeReport {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_amount: u64,
    pub fork_funding_hook_approved: bool,
    pub monitor_persisted: bool,
}

pub fn run_asset_channel_funding_smoke()
-> Result<(AssetChannelStore, AssetChannelFundingSmokeReport), AssetChannelFundingError> {
    let local_script_key = "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"
        .parse::<CompressedKey>()
        .map_err(AssetChannelFundingError::Asset)?;
    let remote_script_key = "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"
        .parse::<CompressedKey>()
        .map_err(AssetChannelFundingError::Asset)?;
    let funding_script_key = "02bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        .parse::<CompressedKey>()
        .map_err(AssetChannelFundingError::Asset)?;
    let local_proof = issue_openusd_proof(700, local_script_key)?;
    let remote_proof = issue_openusd_proof(300, remote_script_key)?;
    let asset_id = local_proof.asset_id;
    let mut store = AssetChannelStore::default();
    let channel = store.fund_channel(AssetChannelFundingRequest {
        local_peer: "alice".to_owned(),
        remote_peer: "bob".to_owned(),
        asset_id,
        funding_outpoint: "1111111111111111111111111111111111111111111111111111111111111111:0"
            .to_owned(),
        funding_script_key,
        local_inputs: vec![local_proof],
        remote_inputs: vec![remote_proof],
        expected_funding_root: None,
    })?;

    Ok((
        store,
        AssetChannelFundingSmokeReport {
            channel_id: channel.channel_id,
            asset_id: channel.asset_id,
            local_balance: channel.local_balance,
            remote_balance: channel.remote_balance,
            total_amount: channel.total_amount,
            fork_funding_hook_approved: true,
            monitor_persisted: channel.monitor.persisted,
        },
    ))
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ValidatedFundingInput {
    proof_id: String,
    proof: ProofFile,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ChannelSide {
    Local,
    Remote,
}

#[derive(Debug)]
pub enum AssetChannelFundingError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Wallet(WalletError),
    Proof(ProofError),
    Asset(AssetError),
    UnsupportedVersion(u32),
    EmptyPeer,
    MissingFundingProofs,
    MalformedFundingOutpoint,
    UnknownChannel(String),
    DuplicateChannel(String),
    AmountOverflow,
    AssetIdMismatch {
        expected: Bytes32,
        actual: Bytes32,
    },
    GenesisMismatch,
    FundingRootMismatch {
        expected: StoredRootHashSum,
        actual: StoredRootHashSum,
    },
    FundingRootSumMismatch {
        root_sum: u64,
        total_amount: u64,
    },
    BalanceMismatch {
        local_balance: u64,
        remote_balance: u64,
        total_amount: u64,
    },
    SpentFundingProof {
        proof_id: String,
        channel_id: String,
    },
    LdkChannelDescriptor(TaprootAssetChannelNegotiationError),
    LdkFundingHook(TaprootAssetFundingError),
    MonitorNotPersisted(String),
    StorageInvariant(String),
}

impl fmt::Display for AssetChannelFundingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "asset-channel funding I/O error: {err}"),
            Self::Json(err) => write!(f, "asset-channel funding JSON error: {err}"),
            Self::Wallet(err) => write!(f, "asset-channel funding wallet error: {err}"),
            Self::Proof(err) => write!(f, "asset-channel funding proof error: {err}"),
            Self::Asset(err) => write!(f, "asset-channel funding asset error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported asset-channel store schema version {version}"
                )
            }
            Self::EmptyPeer => write!(f, "asset-channel peer IDs cannot be empty"),
            Self::MissingFundingProofs => write!(f, "asset-channel funding requires proof input"),
            Self::MalformedFundingOutpoint => {
                write!(f, "asset-channel funding outpoint cannot be empty")
            }
            Self::UnknownChannel(channel_id) => write!(f, "unknown asset channel {channel_id}"),
            Self::DuplicateChannel(channel_id) => write!(f, "duplicate asset channel {channel_id}"),
            Self::AmountOverflow => write!(f, "asset-channel funding amount overflowed"),
            Self::AssetIdMismatch { expected, actual } => write!(
                f,
                "asset-channel funding asset mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::GenesisMismatch => write!(f, "asset-channel funding inputs have mixed genesis"),
            Self::FundingRootMismatch { .. } => {
                write!(f, "asset-channel funding root does not match expected root")
            }
            Self::FundingRootSumMismatch {
                root_sum,
                total_amount,
            } => write!(
                f,
                "asset-channel funding root sum {root_sum} does not match total {total_amount}"
            ),
            Self::BalanceMismatch {
                local_balance,
                remote_balance,
                total_amount,
            } => write!(
                f,
                "asset-channel balances local={local_balance} remote={remote_balance} do not sum to total={total_amount}"
            ),
            Self::SpentFundingProof {
                proof_id,
                channel_id,
            } => write!(
                f,
                "asset-channel funding proof {proof_id} was already used by channel {channel_id}"
            ),
            Self::LdkChannelDescriptor(err) => {
                write!(f, "LDK asset-channel descriptor rejected funding: {err:?}")
            }
            Self::LdkFundingHook(err) => {
                write!(
                    f,
                    "LDK asset-channel funding hook rejected funding: {err:?}"
                )
            }
            Self::MonitorNotPersisted(channel_id) => {
                write!(f, "asset-channel monitor for {channel_id} is not persisted")
            }
            Self::StorageInvariant(message) => {
                write!(f, "asset-channel storage invariant failed: {message}")
            }
        }
    }
}

impl Error for AssetChannelFundingError {}

fn validate_funding_request(
    request: &AssetChannelFundingRequest,
) -> Result<(), AssetChannelFundingError> {
    if request.local_peer.trim().is_empty() || request.remote_peer.trim().is_empty() {
        return Err(AssetChannelFundingError::EmptyPeer);
    }
    if request.funding_outpoint.trim().is_empty() {
        return Err(AssetChannelFundingError::MalformedFundingOutpoint);
    }
    if request.local_inputs.is_empty() && request.remote_inputs.is_empty() {
        return Err(AssetChannelFundingError::MissingFundingProofs);
    }
    Ok(())
}

fn validate_with_ldk_funding_hook(
    request: &AssetChannelFundingRequest,
    channel_id: &str,
    genesis_outpoint: &str,
    funding_root: RootHashSum,
    local_balance: u64,
    remote_balance: u64,
    proof_count: usize,
    output_commitment_override: Option<Bytes32>,
) -> Result<(), AssetChannelFundingError> {
    let descriptor = taproot_asset::TaprootAssetChannelDescriptor::new(
        request.asset_id.0,
        taproot_asset::SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
    )
    .map_err(AssetChannelFundingError::LdkChannelDescriptor)?;
    let funding_outpoint = parse_ldk_outpoint(&request.funding_outpoint)?;
    let local_peer_id = derive_peer_public_key(&request.local_peer)?;
    let remote_peer_id = derive_peer_public_key(&request.remote_peer)?;
    let genesis_id = derive_genesis_id(genesis_outpoint);
    let expected_output_commitment = derive_output_commitment(
        &request.funding_outpoint,
        request.funding_script_key,
        funding_root,
    );
    let actual_output_commitment = output_commitment_override.unwrap_or(expected_output_commitment);
    let proof_count =
        u16::try_from(proof_count).map_err(|_| AssetChannelFundingError::AmountOverflow)?;

    let ldk_request = LdkTaprootAssetFundingRequest {
        pending_channel_id: derive_pending_channel_id(channel_id),
        descriptor,
        funding_outpoint,
        local_peer_id,
        remote_peer_id,
        proof_material: TaprootAssetFundingProofMaterial {
            asset_id: request.asset_id.0,
            genesis_id: genesis_id.0,
            group_key: None,
            proof_root_hash: funding_root.hash.0,
            proof_root_sum: funding_root.sum.value(),
            complete_fragment_count: proof_count,
            expected_fragment_count: proof_count,
        },
        funding_output: TaprootAssetFundingOutput {
            outpoint: funding_outpoint,
            asset_id: request.asset_id.0,
            taproot_asset_root_hash: funding_root.hash.0,
            taproot_asset_root_sum: funding_root.sum.value(),
            output_commitment: actual_output_commitment.0,
        },
        expectations: TaprootAssetFundingExpectations {
            asset_id: request.asset_id.0,
            genesis_id: genesis_id.0,
            group_key: None,
            proof_root_hash: funding_root.hash.0,
            output_commitment: expected_output_commitment.0,
            total_amount: local_balance
                .checked_add(remote_balance)
                .ok_or(AssetChannelFundingError::AmountOverflow)?,
        },
        allocation: TaprootAssetFundingAllocation {
            local_amount: local_balance,
            remote_amount: remote_balance,
        },
    };

    taproot_asset::validate_asset_channel_funding(&ldk_request)
        .map(|_| ())
        .map_err(AssetChannelFundingError::LdkFundingHook)
}

fn validate_inputs(
    asset_id: &Bytes32,
    _side: ChannelSide,
    proofs: &[ProofFile],
    spent_funding_proofs: &BTreeMap<String, String>,
) -> Result<Vec<ValidatedFundingInput>, AssetChannelFundingError> {
    let mut seen = BTreeSet::<String>::new();
    let mut validated = Vec::with_capacity(proofs.len());
    for proof in proofs {
        proof
            .verify_bounded_anchor()
            .map_err(AssetChannelFundingError::Proof)?;
        if proof.asset_id != *asset_id {
            return Err(AssetChannelFundingError::AssetIdMismatch {
                expected: *asset_id,
                actual: proof.asset_id,
            });
        }
        let proof_id = funding_proof_id(proof);
        if !seen.insert(proof_id.clone()) {
            return Err(AssetChannelFundingError::SpentFundingProof {
                proof_id,
                channel_id: "current-request".to_owned(),
            });
        }
        if let Some(channel_id) = spent_funding_proofs.get(&proof_id) {
            return Err(AssetChannelFundingError::SpentFundingProof {
                proof_id,
                channel_id: channel_id.clone(),
            });
        }
        validated.push(ValidatedFundingInput {
            proof_id,
            proof: proof.clone(),
        });
    }
    Ok(validated)
}

fn shared_genesis(
    local_inputs: &[ValidatedFundingInput],
    remote_inputs: &[ValidatedFundingInput],
) -> Result<String, AssetChannelFundingError> {
    let mut inputs = local_inputs.iter().chain(remote_inputs.iter());
    let Some(first) = inputs.next() else {
        return Err(AssetChannelFundingError::MissingFundingProofs);
    };
    let genesis = first.proof.genesis_outpoint.clone();
    if inputs.any(|input| input.proof.genesis_outpoint != genesis) {
        return Err(AssetChannelFundingError::GenesisMismatch);
    }
    Ok(genesis)
}

fn sum_inputs(inputs: &[ValidatedFundingInput]) -> Result<AssetAmount, AssetChannelFundingError> {
    let mut amount = AssetAmount::ZERO;
    for input in inputs {
        amount = amount
            .checked_add(input.proof.amount)
            .map_err(AssetChannelFundingError::Asset)?;
    }
    Ok(amount)
}

fn issue_openusd_proof(
    amount: u64,
    script_key: CompressedKey,
) -> Result<ProofFile, AssetChannelFundingError> {
    let mut wallet = WalletState::default();
    let outcome = wallet
        .issue_regtest_asset(RegtestIssueRequest::openusd(
            AssetAmount::new(amount),
            script_key,
        ))
        .map_err(AssetChannelFundingError::Wallet)?;
    let encoded = wallet
        .export_encoded_proof(&outcome.proof_id)
        .map_err(AssetChannelFundingError::Wallet)?;
    ProofFile::decode(&encoded).map_err(AssetChannelFundingError::Proof)
}

fn funding_proof_id(proof: &ProofFile) -> String {
    format!("{}:{}", proof.asset_id.to_hex(), proof.anchor_outpoint)
}

fn parse_ldk_outpoint(funding_outpoint: &str) -> Result<OutPoint, AssetChannelFundingError> {
    let (txid, index) = funding_outpoint
        .rsplit_once(':')
        .ok_or(AssetChannelFundingError::MalformedFundingOutpoint)?;
    let txid =
        Txid::from_str(txid).map_err(|_| AssetChannelFundingError::MalformedFundingOutpoint)?;
    let index = index
        .parse::<u16>()
        .map_err(|_| AssetChannelFundingError::MalformedFundingOutpoint)?;
    Ok(OutPoint { txid, index })
}

fn derive_peer_public_key(peer: &str) -> Result<PublicKey, AssetChannelFundingError> {
    let secp_ctx = Secp256k1::new();
    let mut candidate = digest_domain(b"tap-ldk:ldk-peer-identity:v1", &[peer.as_bytes()]).0;
    for counter in 0u8..=u8::MAX {
        candidate[31] = candidate[31].wrapping_add(counter);
        if let Ok(secret_key) = SecretKey::from_slice(&candidate) {
            return Ok(PublicKey::from_secret_key(&secp_ctx, &secret_key));
        }
    }
    Err(AssetChannelFundingError::StorageInvariant(format!(
        "could not derive LDK peer identity for {peer}"
    )))
}

fn derive_genesis_id(genesis_outpoint: &str) -> Bytes32 {
    digest_domain(
        b"tap-ldk:asset-channel-genesis-id:v1",
        &[genesis_outpoint.as_bytes()],
    )
}

fn derive_output_commitment(
    funding_outpoint: &str,
    funding_script_key: CompressedKey,
    funding_root: RootHashSum,
) -> Bytes32 {
    digest_domain(
        b"tap-ldk:asset-channel-funding-output:v1",
        &[
            funding_outpoint.as_bytes(),
            &funding_script_key.0,
            &funding_root.hash.0,
            &funding_root.sum.value().to_be_bytes(),
        ],
    )
}

fn derive_pending_channel_id(channel_id: &str) -> ChannelId {
    ChannelId::from_bytes(
        digest_domain(
            b"tap-ldk:asset-channel-pending-channel-id:v1",
            &[channel_id.as_bytes()],
        )
        .0,
    )
}

fn digest_domain(domain: &[u8], parts: &[&[u8]]) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    Bytes32(hasher.finalize().into())
}

fn derive_channel_id(
    local_peer: &str,
    remote_peer: &str,
    asset_id: Bytes32,
    funding_outpoint: &str,
    funding_script_key: CompressedKey,
    local_balance: u64,
    remote_balance: u64,
    input_proofs: &[String],
) -> String {
    let mut proofs = input_proofs.to_vec();
    proofs.sort();

    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:asset-channel-id:v1");
    hasher.update((local_peer.len() as u64).to_be_bytes());
    hasher.update(local_peer.as_bytes());
    hasher.update((remote_peer.len() as u64).to_be_bytes());
    hasher.update(remote_peer.as_bytes());
    hasher.update(asset_id.0);
    hasher.update((funding_outpoint.len() as u64).to_be_bytes());
    hasher.update(funding_outpoint.as_bytes());
    hasher.update(funding_script_key.0);
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    for proof_id in proofs {
        hasher.update((proof_id.len() as u64).to_be_bytes());
        hasher.update(proof_id.as_bytes());
    }
    Bytes32(hasher.finalize().into()).to_hex()
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "asset-channels.json".to_owned());
    path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn local_script_key() -> CompressedKey {
        "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"
            .parse()
            .expect("script key parses")
    }

    fn remote_script_key() -> CompressedKey {
        "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"
            .parse()
            .expect("script key parses")
    }

    fn funding_script_key() -> CompressedKey {
        "02bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            .parse()
            .expect("script key parses")
    }

    fn request(
        local_inputs: Vec<ProofFile>,
        remote_inputs: Vec<ProofFile>,
    ) -> AssetChannelFundingRequest {
        let asset_id = local_inputs
            .first()
            .or_else(|| remote_inputs.first())
            .expect("at least one input")
            .asset_id;
        AssetChannelFundingRequest {
            local_peer: "alice".to_owned(),
            remote_peer: "bob".to_owned(),
            asset_id,
            funding_outpoint: "1111111111111111111111111111111111111111111111111111111111111111:0"
                .to_owned(),
            funding_script_key: funding_script_key(),
            local_inputs,
            remote_inputs,
            expected_funding_root: None,
        }
    }

    #[test]
    fn funding_conserves_same_asset_inputs_and_persists_restart() {
        let path = temp_store_path("restart");
        let local = issue_openusd_proof(700, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut store = AssetChannelStore::default();
        let channel = store
            .fund_channel(request(vec![local], vec![remote]))
            .expect("channel funds");

        assert_eq!(channel.local_balance, 700);
        assert_eq!(channel.remote_balance, 300);
        assert_eq!(channel.total_amount, 1_000);
        assert!(channel.monitor.persisted);

        store.save_atomic(&path).expect("store saves");
        let loaded = AssetChannelStore::load(&path).expect("store loads");
        assert_eq!(
            loaded
                .channel_balances(&channel.channel_id)
                .expect("balances load")
                .total_amount,
            1_000
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn multi_input_same_asset_funding_merges_inputs() {
        let local_one = issue_openusd_proof(400, local_script_key()).expect("local proof");
        let local_two = issue_openusd_proof(300, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut store = AssetChannelStore::default();
        let channel = store
            .fund_channel(request(vec![local_one, local_two], vec![remote]))
            .expect("channel funds");

        assert_eq!(channel.local_balance, 700);
        assert_eq!(channel.remote_balance, 300);
        assert_eq!(channel.total_amount, 1_000);
        assert_eq!(channel.local_input_proof_ids.len(), 2);
    }

    #[test]
    fn wrong_asset_incomplete_and_spent_proofs_fail_closed() {
        let local = issue_openusd_proof(700, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut wrong_asset_request = request(vec![local.clone()], vec![remote.clone()]);
        wrong_asset_request.asset_id = Bytes32([99; 32]);
        let mut store = AssetChannelStore::default();
        assert!(matches!(
            store.fund_channel(wrong_asset_request),
            Err(AssetChannelFundingError::AssetIdMismatch { .. })
        ));
        assert!(store.channels.is_empty());

        assert!(matches!(
            store.fund_channel(AssetChannelFundingRequest {
                local_peer: "alice".to_owned(),
                remote_peer: "bob".to_owned(),
                asset_id: local.asset_id,
                funding_outpoint: "outpoint".to_owned(),
                funding_script_key: funding_script_key(),
                local_inputs: Vec::new(),
                remote_inputs: Vec::new(),
                expected_funding_root: None,
            }),
            Err(AssetChannelFundingError::MissingFundingProofs)
        ));

        let channel = store
            .fund_channel(request(vec![local.clone()], vec![remote]))
            .expect("channel funds");
        assert!(matches!(
            store.fund_channel(request(vec![local], Vec::new())),
            Err(AssetChannelFundingError::SpentFundingProof { channel_id, .. })
                if channel_id == channel.channel_id
        ));
    }

    #[test]
    fn wrong_expected_funding_root_fails_before_state_advances() {
        let local = issue_openusd_proof(700, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut request = request(vec![local], vec![remote]);
        request.expected_funding_root = Some(RootHashSum {
            hash: Bytes32([42; 32]),
            sum: AssetAmount::new(1_000),
        });
        let mut store = AssetChannelStore::default();

        assert!(matches!(
            store.fund_channel(request),
            Err(AssetChannelFundingError::FundingRootMismatch { .. })
        ));
        assert!(store.channels.is_empty());
    }

    #[test]
    fn tampered_stored_funding_root_fails_validation() {
        let local = issue_openusd_proof(700, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut store = AssetChannelStore::default();
        let channel = store
            .fund_channel(request(vec![local], vec![remote]))
            .expect("channel funds");
        store
            .channels
            .get_mut(&channel.channel_id)
            .expect("channel exists")
            .funding_tap_asset_root
            .hash = Bytes32([42; 32]);

        assert!(matches!(
            store.validate(),
            Err(AssetChannelFundingError::FundingRootMismatch { .. })
        ));
    }

    #[test]
    fn funding_smoke_builds_persisted_monitor_state() {
        let (_store, report) = run_asset_channel_funding_smoke().expect("smoke passes");
        assert_eq!(report.local_balance, 700);
        assert_eq!(report.remote_balance, 300);
        assert_eq!(report.total_amount, 1_000);
        assert!(report.fork_funding_hook_approved);
        assert!(report.monitor_persisted);
    }

    #[test]
    fn ldk_funding_hook_failure_fails_before_state_advances() {
        let local = issue_openusd_proof(700, local_script_key()).expect("local proof");
        let remote = issue_openusd_proof(300, remote_script_key()).expect("remote proof");
        let mut store = AssetChannelStore::default();
        let err = store
            .fund_channel_with_fork_commitment_override(
                request(vec![local], vec![remote]),
                Some(Bytes32([99; 32])),
            )
            .expect_err("LDK hook rejects mismatched output commitment");

        assert!(matches!(
            err,
            AssetChannelFundingError::LdkFundingHook(
                TaprootAssetFundingError::OutputCommitmentMismatch
            )
        ));
        assert!(store.channels.is_empty());
        assert!(store.spent_funding_proofs.is_empty());
    }

    fn temp_store_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tap_ldk_asset_channel_{name}_{}_{}.json",
            std::process::id(),
            nanos
        ))
    }
}
