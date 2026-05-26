use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use lightning::{
    chain::channelmonitor::ChannelMonitorUpdate,
    ln::{
        taproot_asset::{
            TaprootAssetMonitorAuxBlob, TaprootAssetMonitorAuxBlobError,
            TaprootAssetMonitorAuxBlobExpectation,
        },
        types::ChannelId,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{AssetError, Bytes32, CompressedKey},
    asset_channel_funding::{
        AssetChannelFundingError, StoredAssetChannel, StoredRootHashSum,
        run_asset_channel_funding_smoke,
    },
};

pub const ASSET_COMMITMENT_STORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentStore {
    pub version: u32,
    pub metadata: AssetCommitmentStoreMetadata,
    pub channels: BTreeMap<String, AssetCommitmentChannelState>,
}

impl Default for AssetCommitmentStore {
    fn default() -> Self {
        Self {
            version: ASSET_COMMITMENT_STORE_SCHEMA_VERSION,
            metadata: AssetCommitmentStoreMetadata::default(),
            channels: BTreeMap::new(),
        }
    }
}

impl AssetCommitmentStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AssetCommitmentError> {
        let raw = fs::read_to_string(path.as_ref()).map_err(AssetCommitmentError::Io)?;
        let store = serde_json::from_str::<Self>(&raw).map_err(AssetCommitmentError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), AssetCommitmentError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(AssetCommitmentError::Io)?;
            }
        }

        let raw = serde_json::to_vec_pretty(self).map_err(AssetCommitmentError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(AssetCommitmentError::Io)?;
        fs::rename(&temp_path, path).map_err(AssetCommitmentError::Io)?;
        Ok(())
    }

    pub fn initialize_channel(
        &mut self,
        channel: &StoredAssetChannel,
    ) -> Result<AssetCommitmentChannelState, AssetCommitmentError> {
        if self.channels.contains_key(&channel.channel_id) {
            return Err(AssetCommitmentError::DuplicateChannel(
                channel.channel_id.clone(),
            ));
        }

        let state = AssetCommitmentChannelState::from_funded_channel(channel)?;
        let mut next = self.clone();
        next.channels
            .insert(channel.channel_id.clone(), state.clone());
        next.validate()?;
        *self = next;
        Ok(state)
    }

    pub fn apply_update(
        &mut self,
        request: AssetCommitmentUpdateRequest,
    ) -> Result<AssetCommitmentSnapshot, AssetCommitmentError> {
        let current = self
            .channels
            .get(&request.channel_id)
            .cloned()
            .ok_or_else(|| AssetCommitmentError::UnknownChannel(request.channel_id.clone()))?;
        let next_snapshot = current.validate_update(&request)?;

        let mut next_state = current;
        next_state.revoked_commitment_numbers.insert(
            next_state.latest_commitment_number,
            next_snapshot.commitment_number,
        );
        next_state.latest_commitment_number = next_snapshot.commitment_number;
        next_state.local_balance = next_snapshot.local_balance;
        next_state.remote_balance = next_snapshot.remote_balance;
        next_state
            .used_asset_nonces
            .insert(request.asset_nonce, next_snapshot.commitment_number);
        next_state.commitments.insert(
            next_snapshot.commitment_number,
            StoredAssetCommitment {
                commitment_number: next_snapshot.commitment_number,
                local_balance: next_snapshot.local_balance,
                remote_balance: next_snapshot.remote_balance,
                state_digest: next_snapshot.state_digest,
                virtual_tx_id: request.virtual_tx_id,
                witness_digest: request.witness_digest,
                asset_nonce: request.asset_nonce,
                asset_signature: request.asset_signature,
            },
        );
        let funding_root = next_state.monitor_blob.funding_root();
        next_state.monitor_blob = AssetCommitmentMonitorBlob::new(
            &next_state.channel_id,
            next_state.asset_id,
            next_snapshot.commitment_number,
            next_snapshot.local_balance,
            next_snapshot.remote_balance,
            next_snapshot.state_digest,
            &funding_root,
            request.asset_nonce,
            request.asset_signature,
        )?;

        let mut next = self.clone();
        next.channels
            .insert(next_state.channel_id.clone(), next_state);
        next.validate()?;
        *self = next;

        Ok(next_snapshot)
    }

    pub fn channel_state(
        &self,
        channel_id: &str,
    ) -> Result<AssetCommitmentChannelState, AssetCommitmentError> {
        self.channels
            .get(channel_id)
            .cloned()
            .ok_or_else(|| AssetCommitmentError::UnknownChannel(channel_id.to_owned()))
    }

    pub fn validate(&self) -> Result<(), AssetCommitmentError> {
        if self.version != ASSET_COMMITMENT_STORE_SCHEMA_VERSION {
            return Err(AssetCommitmentError::UnsupportedVersion(self.version));
        }

        for (channel_id, state) in &self.channels {
            if channel_id != &state.channel_id {
                return Err(AssetCommitmentError::StorageInvariant(format!(
                    "commitment map key {channel_id} does not match channel_id {}",
                    state.channel_id
                )));
            }
            state.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentStoreMetadata {
    pub implementation: String,
    pub schema: String,
}

impl Default for AssetCommitmentStoreMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk experimental asset commitment store".to_owned(),
            schema: "bounded-regtest-asset-commitment-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentChannelState {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub total_amount: u64,
    pub latest_commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub commitments: BTreeMap<u64, StoredAssetCommitment>,
    pub revoked_commitment_numbers: BTreeMap<u64, u64>,
    pub used_asset_nonces: BTreeMap<Bytes32, u64>,
    pub asset_signing_key: CompressedKey,
    pub monitor_blob: AssetCommitmentMonitorBlob,
}

impl AssetCommitmentChannelState {
    fn from_funded_channel(channel: &StoredAssetChannel) -> Result<Self, AssetCommitmentError> {
        let state_digest = commitment_state_digest(
            &channel.channel_id,
            channel.asset_id,
            channel.monitor.commitment_number,
            channel.local_balance,
            channel.remote_balance,
        );
        let monitor_blob = AssetCommitmentMonitorBlob::new(
            &channel.channel_id,
            channel.asset_id,
            channel.monitor.commitment_number,
            channel.local_balance,
            channel.remote_balance,
            state_digest,
            &channel.funding_tap_asset_root,
            monitor_context_digest(
                b"tap-ldk:asset-commitment-funding-nonce:v1",
                &channel.channel_id,
                channel.asset_id,
                channel.monitor.commitment_number,
                state_digest,
            ),
            monitor_context_digest(
                b"tap-ldk:asset-commitment-funding-signature:v1",
                &channel.channel_id,
                channel.asset_id,
                channel.monitor.commitment_number,
                state_digest,
            ),
        )?;
        Ok(Self {
            channel_id: channel.channel_id.clone(),
            asset_id: channel.asset_id,
            total_amount: channel.total_amount,
            latest_commitment_number: channel.monitor.commitment_number,
            local_balance: channel.local_balance,
            remote_balance: channel.remote_balance,
            commitments: BTreeMap::new(),
            revoked_commitment_numbers: BTreeMap::new(),
            used_asset_nonces: BTreeMap::new(),
            asset_signing_key: channel.funding_script_key,
            monitor_blob,
        })
    }

    fn validate_update(
        &self,
        request: &AssetCommitmentUpdateRequest,
    ) -> Result<AssetCommitmentSnapshot, AssetCommitmentError> {
        if request.channel_id != self.channel_id {
            return Err(AssetCommitmentError::UnknownChannel(
                request.channel_id.clone(),
            ));
        }
        let expected_commitment_number = self
            .latest_commitment_number
            .checked_add(1)
            .ok_or(AssetCommitmentError::CommitmentNumberOverflow)?;
        if request.next_commitment_number != expected_commitment_number {
            return Err(AssetCommitmentError::StaleCommitmentNumber {
                expected: expected_commitment_number,
                actual: request.next_commitment_number,
            });
        }
        if self.used_asset_nonces.contains_key(&request.asset_nonce) {
            return Err(AssetCommitmentError::NonceReuse(request.asset_nonce));
        }

        let local_after_send = self
            .local_balance
            .checked_sub(request.local_to_remote)
            .ok_or(AssetCommitmentError::BalanceUnderflow {
                side: BalanceSide::Local,
                balance: self.local_balance,
                delta: request.local_to_remote,
            })?;
        let remote_after_send = self
            .remote_balance
            .checked_sub(request.remote_to_local)
            .ok_or(AssetCommitmentError::BalanceUnderflow {
                side: BalanceSide::Remote,
                balance: self.remote_balance,
                delta: request.remote_to_local,
            })?;
        let local_balance = local_after_send
            .checked_add(request.remote_to_local)
            .ok_or(AssetCommitmentError::BalanceOverflow)?;
        let remote_balance = remote_after_send
            .checked_add(request.local_to_remote)
            .ok_or(AssetCommitmentError::BalanceOverflow)?;
        if local_balance
            .checked_add(remote_balance)
            .ok_or(AssetCommitmentError::BalanceOverflow)?
            != self.total_amount
        {
            return Err(AssetCommitmentError::BalanceNotConserved {
                local_balance,
                remote_balance,
                total_amount: self.total_amount,
            });
        }

        let state_digest = commitment_state_digest(
            &self.channel_id,
            self.asset_id,
            request.next_commitment_number,
            local_balance,
            remote_balance,
        );
        let expected_signature = expected_asset_signature(
            &self.channel_id,
            self.asset_id,
            self.asset_signing_key,
            request.next_commitment_number,
            local_balance,
            remote_balance,
            request.asset_nonce,
            request.virtual_tx_id,
            request.witness_digest,
        );
        if request.asset_signature != expected_signature {
            return Err(AssetCommitmentError::InvalidSignature);
        }

        Ok(AssetCommitmentSnapshot {
            channel_id: self.channel_id.clone(),
            asset_id: self.asset_id,
            commitment_number: request.next_commitment_number,
            local_balance,
            remote_balance,
            total_amount: self.total_amount,
            state_digest,
        })
    }

    fn validate(&self) -> Result<(), AssetCommitmentError> {
        if self
            .local_balance
            .checked_add(self.remote_balance)
            .ok_or(AssetCommitmentError::BalanceOverflow)?
            != self.total_amount
        {
            return Err(AssetCommitmentError::BalanceNotConserved {
                local_balance: self.local_balance,
                remote_balance: self.remote_balance,
                total_amount: self.total_amount,
            });
        }
        if self
            .revoked_commitment_numbers
            .contains_key(&self.latest_commitment_number)
        {
            return Err(AssetCommitmentError::RevokedLatestCommitment(
                self.latest_commitment_number,
            ));
        }
        if !self.monitor_blob.persisted {
            return Err(AssetCommitmentError::MonitorNotPersisted(
                self.channel_id.clone(),
            ));
        }
        let latest_digest = commitment_state_digest(
            &self.channel_id,
            self.asset_id,
            self.latest_commitment_number,
            self.local_balance,
            self.remote_balance,
        );
        if self.monitor_blob.commitment_number != self.latest_commitment_number
            || self.monitor_blob.state_digest != latest_digest
            || self.monitor_blob.blob_digest
                != AssetCommitmentMonitorBlob::digest_for(
                    &self.channel_id,
                    self.asset_id,
                    self.latest_commitment_number,
                    self.local_balance,
                    self.remote_balance,
                    latest_digest,
                    self.monitor_blob.proof_root_hash,
                    self.monitor_blob.proof_root_sum,
                    self.monitor_blob.nonce_digest,
                    self.monitor_blob.signature_digest,
                )
        {
            return Err(AssetCommitmentError::MonitorDigestMismatch(
                self.channel_id.clone(),
            ));
        }
        self.validate_ldk_monitor_aux_blob(latest_digest)?;

        let mut nonces = BTreeSet::new();
        for (nonce, commitment_number) in &self.used_asset_nonces {
            if !nonces.insert(*nonce) {
                return Err(AssetCommitmentError::NonceReuse(*nonce));
            }
            let Some(commitment) = self.commitments.get(commitment_number) else {
                return Err(AssetCommitmentError::StorageInvariant(format!(
                    "nonce {} points to missing commitment {commitment_number}",
                    nonce.to_hex()
                )));
            };
            if commitment.asset_nonce != *nonce {
                return Err(AssetCommitmentError::StorageInvariant(format!(
                    "nonce index {} does not match commitment",
                    nonce.to_hex()
                )));
            }
        }

        for (number, commitment) in &self.commitments {
            if number != &commitment.commitment_number {
                return Err(AssetCommitmentError::StorageInvariant(format!(
                    "commitment map key {number} does not match commitment_number {}",
                    commitment.commitment_number
                )));
            }
            if commitment
                .local_balance
                .checked_add(commitment.remote_balance)
                .ok_or(AssetCommitmentError::BalanceOverflow)?
                != self.total_amount
            {
                return Err(AssetCommitmentError::BalanceNotConserved {
                    local_balance: commitment.local_balance,
                    remote_balance: commitment.remote_balance,
                    total_amount: self.total_amount,
                });
            }
            let expected_state_digest = commitment_state_digest(
                &self.channel_id,
                self.asset_id,
                commitment.commitment_number,
                commitment.local_balance,
                commitment.remote_balance,
            );
            if commitment.state_digest != expected_state_digest {
                return Err(AssetCommitmentError::StorageInvariant(format!(
                    "commitment {number} state digest mismatch"
                )));
            }
        }

        Ok(())
    }

    fn validate_ldk_monitor_aux_blob(
        &self,
        latest_digest: Bytes32,
    ) -> Result<(), AssetCommitmentError> {
        let ldk_update = self.build_ldk_monitor_update()?;
        let expected = TaprootAssetMonitorAuxBlobExpectation {
            channel_id: parse_channel_id(&self.channel_id)?,
            asset_id: self.asset_id.0,
            commitment_number: self.latest_commitment_number,
            local_balance: self.local_balance,
            remote_balance: self.remote_balance,
            state_digest: latest_digest.0,
            proof_root_hash: self.monitor_blob.proof_root_hash.0,
            proof_root_sum: self.monitor_blob.proof_root_sum,
        };
        ldk_update
            .require_taproot_asset_aux_blob(&expected)
            .map_err(AssetCommitmentError::LdkMonitorAux)?;
        Ok(())
    }

    pub fn build_ldk_monitor_update(&self) -> Result<ChannelMonitorUpdate, AssetCommitmentError> {
        let channel_id = parse_channel_id(&self.channel_id)?;
        let blob = self.monitor_blob.to_ldk_aux_blob(
            channel_id,
            self.asset_id,
            self.local_balance,
            self.remote_balance,
        )?;
        ChannelMonitorUpdate::taproot_asset_aux_update(
            self.latest_commitment_number,
            channel_id,
            blob,
        )
        .map_err(AssetCommitmentError::LdkMonitorAux)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredAssetCommitment {
    pub commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub state_digest: Bytes32,
    pub virtual_tx_id: Bytes32,
    pub witness_digest: Bytes32,
    pub asset_nonce: Bytes32,
    pub asset_signature: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentMonitorBlob {
    pub schema_version: u32,
    pub commitment_number: u64,
    pub persisted: bool,
    pub state_digest: Bytes32,
    pub proof_root_hash: Bytes32,
    pub proof_root_sum: u64,
    pub nonce_digest: Bytes32,
    pub signature_digest: Bytes32,
    pub ldk_aux_blob_digest: Option<Bytes32>,
    pub blob_digest: Bytes32,
}

impl AssetCommitmentMonitorBlob {
    fn new(
        channel_id: &str,
        asset_id: Bytes32,
        commitment_number: u64,
        local_balance: u64,
        remote_balance: u64,
        state_digest: Bytes32,
        funding_root: &StoredRootHashSum,
        nonce_digest: Bytes32,
        signature_digest: Bytes32,
    ) -> Result<Self, AssetCommitmentError> {
        let mut blob = Self {
            schema_version: ASSET_COMMITMENT_STORE_SCHEMA_VERSION,
            commitment_number,
            persisted: true,
            state_digest,
            proof_root_hash: funding_root.hash,
            proof_root_sum: funding_root.sum,
            nonce_digest,
            signature_digest,
            ldk_aux_blob_digest: None,
            blob_digest: Self::digest_for(
                channel_id,
                asset_id,
                commitment_number,
                local_balance,
                remote_balance,
                state_digest,
                funding_root.hash,
                funding_root.sum,
                nonce_digest,
                signature_digest,
            ),
        };
        let ldk_blob = blob.build_ldk_aux_blob(
            parse_channel_id(channel_id)?,
            asset_id,
            local_balance,
            remote_balance,
        )?;
        blob.ldk_aux_blob_digest = Some(Bytes32(ldk_blob.blob_digest));
        Ok(blob)
    }

    fn funding_root(&self) -> StoredRootHashSum {
        StoredRootHashSum {
            hash: self.proof_root_hash,
            sum: self.proof_root_sum,
        }
    }

    fn to_ldk_aux_blob(
        &self,
        channel_id: ChannelId,
        asset_id: Bytes32,
        local_balance: u64,
        remote_balance: u64,
    ) -> Result<TaprootAssetMonitorAuxBlob, AssetCommitmentError> {
        let ldk_blob =
            self.build_ldk_aux_blob(channel_id, asset_id, local_balance, remote_balance)?;
        let expected = self
            .ldk_aux_blob_digest
            .ok_or(AssetCommitmentError::LdkMonitorAux(
                TaprootAssetMonitorAuxBlobError::MissingAssetBlob,
            ))?;
        if expected.0 != ldk_blob.blob_digest {
            return Err(AssetCommitmentError::LdkMonitorAux(
                TaprootAssetMonitorAuxBlobError::BlobDigestMismatch,
            ));
        }
        Ok(ldk_blob)
    }

    fn build_ldk_aux_blob(
        &self,
        channel_id: ChannelId,
        asset_id: Bytes32,
        local_balance: u64,
        remote_balance: u64,
    ) -> Result<TaprootAssetMonitorAuxBlob, AssetCommitmentError> {
        TaprootAssetMonitorAuxBlob::new(
            channel_id,
            asset_id.0,
            self.commitment_number,
            local_balance,
            remote_balance,
            self.state_digest.0,
            self.proof_root_hash.0,
            self.proof_root_sum,
            self.nonce_digest.0,
            self.signature_digest.0,
        )
        .map_err(AssetCommitmentError::LdkMonitorAux)
    }

    fn digest_for(
        channel_id: &str,
        asset_id: Bytes32,
        commitment_number: u64,
        local_balance: u64,
        remote_balance: u64,
        state_digest: Bytes32,
        proof_root_hash: Bytes32,
        proof_root_sum: u64,
        nonce_digest: Bytes32,
        signature_digest: Bytes32,
    ) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:asset-commitment-monitor:v1");
        hasher.update((channel_id.len() as u64).to_be_bytes());
        hasher.update(channel_id.as_bytes());
        hasher.update(asset_id.0);
        hasher.update(commitment_number.to_be_bytes());
        hasher.update(local_balance.to_be_bytes());
        hasher.update(remote_balance.to_be_bytes());
        hasher.update(state_digest.0);
        hasher.update(proof_root_hash.0);
        hasher.update(proof_root_sum.to_be_bytes());
        hasher.update(nonce_digest.0);
        hasher.update(signature_digest.0);
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetCommitmentUpdateRequest {
    pub channel_id: String,
    pub next_commitment_number: u64,
    pub local_to_remote: u64,
    pub remote_to_local: u64,
    pub virtual_tx_id: Bytes32,
    pub witness_digest: Bytes32,
    pub asset_nonce: Bytes32,
    pub asset_signature: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentSnapshot {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_amount: u64,
    pub state_digest: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetCommitmentSmokeReport {
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub latest_commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub total_amount: u64,
    pub revoked_commitments: Vec<u64>,
    pub asset_and_btc_signatures_are_separate: bool,
    pub ldk_monitor_aux_blob_persisted: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BalanceSide {
    Local,
    Remote,
}

pub fn build_commitment_update(
    state: &AssetCommitmentChannelState,
    local_to_remote: u64,
    remote_to_local: u64,
    asset_nonce: Bytes32,
) -> Result<AssetCommitmentUpdateRequest, AssetCommitmentError> {
    let next_commitment_number = state
        .latest_commitment_number
        .checked_add(1)
        .ok_or(AssetCommitmentError::CommitmentNumberOverflow)?;
    let local_after_send = state.local_balance.checked_sub(local_to_remote).ok_or(
        AssetCommitmentError::BalanceUnderflow {
            side: BalanceSide::Local,
            balance: state.local_balance,
            delta: local_to_remote,
        },
    )?;
    let remote_after_send = state.remote_balance.checked_sub(remote_to_local).ok_or(
        AssetCommitmentError::BalanceUnderflow {
            side: BalanceSide::Remote,
            balance: state.remote_balance,
            delta: remote_to_local,
        },
    )?;
    let local_balance = local_after_send
        .checked_add(remote_to_local)
        .ok_or(AssetCommitmentError::BalanceOverflow)?;
    let remote_balance = remote_after_send
        .checked_add(local_to_remote)
        .ok_or(AssetCommitmentError::BalanceOverflow)?;
    let virtual_tx_id = virtual_tx_id(
        &state.channel_id,
        state.asset_id,
        next_commitment_number,
        local_balance,
        remote_balance,
    );
    let witness_digest = witness_digest(
        &state.channel_id,
        state.asset_id,
        next_commitment_number,
        asset_nonce,
    );
    let asset_signature = expected_asset_signature(
        &state.channel_id,
        state.asset_id,
        state.asset_signing_key,
        next_commitment_number,
        local_balance,
        remote_balance,
        asset_nonce,
        virtual_tx_id,
        witness_digest,
    );

    Ok(AssetCommitmentUpdateRequest {
        channel_id: state.channel_id.clone(),
        next_commitment_number,
        local_to_remote,
        remote_to_local,
        virtual_tx_id,
        witness_digest,
        asset_nonce,
        asset_signature,
    })
}

pub fn expected_btc_signature_for_test(
    state: &AssetCommitmentChannelState,
    request: &AssetCommitmentUpdateRequest,
) -> Result<Bytes32, AssetCommitmentError> {
    let local_after_send = state
        .local_balance
        .checked_sub(request.local_to_remote)
        .ok_or(AssetCommitmentError::BalanceUnderflow {
            side: BalanceSide::Local,
            balance: state.local_balance,
            delta: request.local_to_remote,
        })?;
    let remote_after_send = state
        .remote_balance
        .checked_sub(request.remote_to_local)
        .ok_or(AssetCommitmentError::BalanceUnderflow {
            side: BalanceSide::Remote,
            balance: state.remote_balance,
            delta: request.remote_to_local,
        })?;
    let local_balance = local_after_send
        .checked_add(request.remote_to_local)
        .ok_or(AssetCommitmentError::BalanceOverflow)?;
    let remote_balance = remote_after_send
        .checked_add(request.local_to_remote)
        .ok_or(AssetCommitmentError::BalanceOverflow)?;
    Ok(expected_signature_with_domain(
        b"tap-ldk:btc-commitment-signature:v1",
        &state.channel_id,
        state.asset_id,
        state.asset_signing_key,
        request.next_commitment_number,
        local_balance,
        remote_balance,
        request.asset_nonce,
        request.virtual_tx_id,
        request.witness_digest,
    ))
}

pub fn run_asset_commitment_smoke()
-> Result<(AssetCommitmentStore, AssetCommitmentSmokeReport), AssetCommitmentError> {
    let (funding_store, funding_report) =
        run_asset_channel_funding_smoke().map_err(AssetCommitmentError::Funding)?;
    let funded_channel = funding_store
        .channels
        .get(&funding_report.channel_id)
        .ok_or_else(|| AssetCommitmentError::UnknownChannel(funding_report.channel_id.clone()))?;
    let mut store = AssetCommitmentStore::default();
    let state = store.initialize_channel(funded_channel)?;
    let update = build_commitment_update(&state, 125, 0, Bytes32([12; 32]))?;
    let btc_signature = expected_btc_signature_for_test(&state, &update)?;
    let snapshot = store.apply_update(update.clone())?;
    let state = store.channel_state(&snapshot.channel_id)?;

    Ok((
        store,
        AssetCommitmentSmokeReport {
            channel_id: state.channel_id,
            asset_id: state.asset_id,
            latest_commitment_number: state.latest_commitment_number,
            local_balance: state.local_balance,
            remote_balance: state.remote_balance,
            total_amount: state.total_amount,
            revoked_commitments: state.revoked_commitment_numbers.keys().copied().collect(),
            asset_and_btc_signatures_are_separate: update.asset_signature != btc_signature,
            ldk_monitor_aux_blob_persisted: state.monitor_blob.ldk_aux_blob_digest.is_some(),
        },
    ))
}

#[derive(Debug)]
pub enum AssetCommitmentError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Funding(AssetChannelFundingError),
    UnsupportedVersion(u32),
    DuplicateChannel(String),
    UnknownChannel(String),
    CommitmentNumberOverflow,
    StaleCommitmentNumber {
        expected: u64,
        actual: u64,
    },
    NonceReuse(Bytes32),
    InvalidSignature,
    BalanceUnderflow {
        side: BalanceSide,
        balance: u64,
        delta: u64,
    },
    BalanceOverflow,
    BalanceNotConserved {
        local_balance: u64,
        remote_balance: u64,
        total_amount: u64,
    },
    RevokedLatestCommitment(u64),
    MonitorNotPersisted(String),
    MonitorDigestMismatch(String),
    MalformedChannelId(AssetError),
    LdkMonitorAux(TaprootAssetMonitorAuxBlobError),
    StorageInvariant(String),
}

impl fmt::Display for AssetCommitmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "asset commitment I/O error: {err}"),
            Self::Json(err) => write!(f, "asset commitment JSON error: {err}"),
            Self::Funding(err) => write!(f, "asset commitment funding error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported asset commitment schema version {version}")
            }
            Self::DuplicateChannel(channel_id) => {
                write!(f, "duplicate asset commitment channel {channel_id}")
            }
            Self::UnknownChannel(channel_id) => {
                write!(f, "unknown asset commitment channel {channel_id}")
            }
            Self::CommitmentNumberOverflow => write!(f, "asset commitment number overflowed"),
            Self::StaleCommitmentNumber { expected, actual } => write!(
                f,
                "stale asset commitment number: expected {expected}, got {actual}"
            ),
            Self::NonceReuse(nonce) => write!(f, "asset nonce reused: {}", nonce.to_hex()),
            Self::InvalidSignature => write!(f, "invalid asset commitment signature"),
            Self::BalanceUnderflow {
                side,
                balance,
                delta,
            } => write!(
                f,
                "asset commitment {side:?} balance underflow: balance={balance} delta={delta}"
            ),
            Self::BalanceOverflow => write!(f, "asset commitment balance overflowed"),
            Self::BalanceNotConserved {
                local_balance,
                remote_balance,
                total_amount,
            } => write!(
                f,
                "asset commitment balances local={local_balance} remote={remote_balance} do not conserve total={total_amount}"
            ),
            Self::RevokedLatestCommitment(commitment_number) => write!(
                f,
                "latest asset commitment {commitment_number} is marked revoked"
            ),
            Self::MonitorNotPersisted(channel_id) => {
                write!(
                    f,
                    "asset commitment monitor for {channel_id} is not persisted"
                )
            }
            Self::MonitorDigestMismatch(channel_id) => {
                write!(
                    f,
                    "asset commitment monitor digest mismatch for {channel_id}"
                )
            }
            Self::MalformedChannelId(err) => {
                write!(f, "asset commitment channel id could not map to LDK: {err}")
            }
            Self::LdkMonitorAux(err) => {
                write!(f, "LDK asset monitor aux blob rejected commitment: {err:?}")
            }
            Self::StorageInvariant(message) => {
                write!(f, "asset commitment storage invariant failed: {message}")
            }
        }
    }
}

impl Error for AssetCommitmentError {}

fn parse_channel_id(channel_id: &str) -> Result<ChannelId, AssetCommitmentError> {
    let bytes = Bytes32::from_str(channel_id).map_err(AssetCommitmentError::MalformedChannelId)?;
    Ok(ChannelId::from_bytes(bytes.0))
}

fn monitor_context_digest(
    domain: &[u8],
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    state_digest: Bytes32,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(state_digest.0);
    Bytes32(hasher.finalize().into())
}

fn commitment_state_digest(
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    local_balance: u64,
    remote_balance: u64,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:asset-commitment-state:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn virtual_tx_id(
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    local_balance: u64,
    remote_balance: u64,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:asset-virtual-tx:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn witness_digest(
    channel_id: &str,
    asset_id: Bytes32,
    commitment_number: u64,
    asset_nonce: Bytes32,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:asset-witness:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(asset_nonce.0);
    Bytes32(hasher.finalize().into())
}

fn expected_asset_signature(
    channel_id: &str,
    asset_id: Bytes32,
    signing_key: CompressedKey,
    commitment_number: u64,
    local_balance: u64,
    remote_balance: u64,
    asset_nonce: Bytes32,
    virtual_tx_id: Bytes32,
    witness_digest: Bytes32,
) -> Bytes32 {
    expected_signature_with_domain(
        b"tap-ldk:asset-commitment-signature:v1",
        channel_id,
        asset_id,
        signing_key,
        commitment_number,
        local_balance,
        remote_balance,
        asset_nonce,
        virtual_tx_id,
        witness_digest,
    )
}

#[allow(clippy::too_many_arguments)]
fn expected_signature_with_domain(
    domain: &[u8],
    channel_id: &str,
    asset_id: Bytes32,
    signing_key: CompressedKey,
    commitment_number: u64,
    local_balance: u64,
    remote_balance: u64,
    asset_nonce: Bytes32,
    virtual_tx_id: Bytes32,
    witness_digest: Bytes32,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(signing_key.0);
    hasher.update(commitment_number.to_be_bytes());
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    hasher.update(asset_nonce.0);
    hasher.update(virtual_tx_id.0);
    hasher.update(witness_digest.0);
    Bytes32(hasher.finalize().into())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "asset-commitments.json".to_owned());
    path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn funded_channel() -> StoredAssetChannel {
        let (funding_store, report) = run_asset_channel_funding_smoke().expect("funding smoke");
        funding_store
            .channels
            .get(&report.channel_id)
            .expect("channel exists")
            .clone()
    }

    fn initialized_store() -> (AssetCommitmentStore, AssetCommitmentChannelState) {
        let channel = funded_channel();
        let mut store = AssetCommitmentStore::default();
        let state = store
            .initialize_channel(&channel)
            .expect("channel initializes");
        (store, state)
    }

    #[test]
    fn update_moves_balances_and_persists_across_restart() {
        let path = temp_store_path("restart");
        let (mut store, state) = initialized_store();
        let update =
            build_commitment_update(&state, 125, 0, Bytes32([9; 32])).expect("update builds");
        let snapshot = store.apply_update(update).expect("update applies");

        assert_eq!(snapshot.commitment_number, 1);
        assert_eq!(snapshot.local_balance, 575);
        assert_eq!(snapshot.remote_balance, 425);
        assert_eq!(snapshot.total_amount, 1_000);

        store.save_atomic(&path).expect("store saves");
        let loaded = AssetCommitmentStore::load(&path).expect("store loads");
        let loaded_state = loaded
            .channel_state(&snapshot.channel_id)
            .expect("state loads");
        assert_eq!(loaded_state.latest_commitment_number, 1);
        assert_eq!(loaded_state.local_balance, 575);
        assert_eq!(loaded_state.remote_balance, 425);
        assert!(loaded_state.revoked_commitment_numbers.contains_key(&0));
        assert_ldk_monitor_aux_blob_matches(&loaded_state);
        fs::remove_file(path).ok();
    }

    #[test]
    fn stale_number_nonce_reuse_and_bad_signature_fail_closed() {
        let (mut store, state) = initialized_store();
        let mut stale =
            build_commitment_update(&state, 125, 0, Bytes32([10; 32])).expect("update builds");
        stale.next_commitment_number = 0;
        assert!(matches!(
            store.apply_update(stale),
            Err(AssetCommitmentError::StaleCommitmentNumber {
                expected: 1,
                actual: 0
            })
        ));

        let update =
            build_commitment_update(&state, 125, 0, Bytes32([10; 32])).expect("update builds");
        store
            .apply_update(update.clone())
            .expect("first update applies");
        let next_state = store
            .channel_state(&update.channel_id)
            .expect("next state exists");
        let reused_nonce = build_commitment_update(&next_state, 1, 0, Bytes32([10; 32]))
            .expect("reused nonce update builds");
        assert!(matches!(
            store.apply_update(reused_nonce),
            Err(AssetCommitmentError::NonceReuse(nonce)) if nonce == Bytes32([10; 32])
        ));

        let mut bad_signature =
            build_commitment_update(&next_state, 1, 0, Bytes32([11; 32])).expect("update builds");
        bad_signature.asset_signature = Bytes32([99; 32]);
        assert!(matches!(
            store.apply_update(bad_signature),
            Err(AssetCommitmentError::InvalidSignature)
        ));
    }

    #[test]
    fn underflow_overflow_and_tampered_monitor_fail_closed() {
        let (mut store, state) = initialized_store();
        assert!(matches!(
            build_commitment_update(&state, 701, 0, Bytes32([12; 32])),
            Err(AssetCommitmentError::BalanceUnderflow {
                side: BalanceSide::Local,
                ..
            })
        ));

        let update =
            build_commitment_update(&state, 125, 0, Bytes32([13; 32])).expect("update builds");
        store.apply_update(update).expect("update applies");
        let channel_id = state.channel_id.clone();
        store
            .channels
            .get_mut(&channel_id)
            .expect("state exists")
            .monitor_blob
            .persisted = false;
        assert!(matches!(
            store.validate(),
            Err(AssetCommitmentError::MonitorNotPersisted(id)) if id == channel_id
        ));
    }

    #[test]
    fn missing_or_tampered_ldk_monitor_aux_blob_fails_closed() {
        let (mut store, state) = initialized_store();
        let update =
            build_commitment_update(&state, 125, 0, Bytes32([15; 32])).expect("update builds");
        store.apply_update(update).expect("update applies");
        let channel_id = state.channel_id.clone();

        let mut missing = store.clone();
        missing
            .channels
            .get_mut(&channel_id)
            .expect("state exists")
            .monitor_blob
            .ldk_aux_blob_digest = None;
        assert!(matches!(
            missing.validate(),
            Err(AssetCommitmentError::LdkMonitorAux(
                TaprootAssetMonitorAuxBlobError::MissingAssetBlob
            ))
        ));

        let mut tampered = store;
        tampered
            .channels
            .get_mut(&channel_id)
            .expect("state exists")
            .monitor_blob
            .ldk_aux_blob_digest = Some(Bytes32([1; 32]));
        assert!(matches!(
            tampered.validate(),
            Err(AssetCommitmentError::LdkMonitorAux(
                TaprootAssetMonitorAuxBlobError::BlobDigestMismatch
            ))
        ));
    }

    #[test]
    fn asset_and_btc_signature_domains_are_separate() {
        let (_store, state) = initialized_store();
        let mut update =
            build_commitment_update(&state, 125, 0, Bytes32([14; 32])).expect("update builds");
        let btc_signature =
            expected_btc_signature_for_test(&state, &update).expect("btc signature builds");
        assert_ne!(update.asset_signature, btc_signature);
        update.asset_signature = btc_signature;

        let mut store = AssetCommitmentStore::default();
        store
            .channels
            .insert(state.channel_id.clone(), state.clone());
        assert!(matches!(
            store.apply_update(update),
            Err(AssetCommitmentError::InvalidSignature)
        ));
    }

    #[test]
    fn bounded_conservation_property_over_small_transfers() {
        for local_to_remote in 0..=10 {
            for remote_to_local in 0..=10 {
                let (mut store, state) = initialized_store();
                let update = build_commitment_update(
                    &state,
                    local_to_remote,
                    remote_to_local,
                    Bytes32([
                        local_to_remote as u8,
                        remote_to_local as u8,
                        42,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]),
                )
                .expect("update builds");
                let snapshot = store.apply_update(update).expect("update applies");
                assert_eq!(
                    snapshot.local_balance + snapshot.remote_balance,
                    snapshot.total_amount
                );
            }
        }
    }

    #[test]
    fn smoke_reports_commitment_update_and_revocation() {
        let (_store, report) = run_asset_commitment_smoke().expect("smoke passes");
        assert_eq!(report.latest_commitment_number, 1);
        assert_eq!(report.local_balance, 575);
        assert_eq!(report.remote_balance, 425);
        assert_eq!(report.total_amount, 1_000);
        assert_eq!(report.revoked_commitments, vec![0]);
        assert!(report.asset_and_btc_signatures_are_separate);
        assert!(report.ldk_monitor_aux_blob_persisted);
    }

    fn assert_ldk_monitor_aux_blob_matches(state: &AssetCommitmentChannelState) {
        let update = state
            .build_ldk_monitor_update()
            .expect("LDK monitor update builds");
        let expected = TaprootAssetMonitorAuxBlobExpectation {
            channel_id: parse_channel_id(&state.channel_id).expect("valid channel id"),
            asset_id: state.asset_id.0,
            commitment_number: state.latest_commitment_number,
            local_balance: state.local_balance,
            remote_balance: state.remote_balance,
            state_digest: state.monitor_blob.state_digest.0,
            proof_root_hash: state.monitor_blob.proof_root_hash.0,
            proof_root_sum: state.monitor_blob.proof_root_sum,
        };
        let blob = update
            .require_taproot_asset_aux_blob(&expected)
            .expect("LDK monitor aux blob matches commitment");
        assert_eq!(
            state.monitor_blob.ldk_aux_blob_digest,
            Some(Bytes32(blob.blob_digest))
        );
    }

    fn temp_store_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tap_ldk_asset_commitment_{name}_{}_{}.json",
            std::process::id(),
            nanos
        ))
    }
}
