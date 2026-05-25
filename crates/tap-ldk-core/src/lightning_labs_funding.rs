use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    lightning_labs_blob::{
        AssetOutputSummary, LightningLabsBlobError, LightningLabsCommitmentBlob,
        LightningLabsFundingBlob, decode_commitment_blob_hexdump, decode_funding_blob_hexdump,
    },
    regtest::LightningLabsCounterpartyConfig,
};

pub const LIGHTNING_LABS_FUNDING_INTEROP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingInteropStore {
    pub version: u32,
    pub metadata: LightningLabsFundingInteropMetadata,
    pub states: BTreeMap<String, LightningLabsFundingInteropState>,
}

impl Default for LightningLabsFundingInteropStore {
    fn default() -> Self {
        Self {
            version: LIGHTNING_LABS_FUNDING_INTEROP_SCHEMA_VERSION,
            metadata: LightningLabsFundingInteropMetadata::default(),
            states: BTreeMap::new(),
        }
    }
}

impl LightningLabsFundingInteropStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LightningLabsFundingInteropError> {
        let raw =
            fs::read_to_string(path.as_ref()).map_err(LightningLabsFundingInteropError::Io)?;
        let store =
            serde_json::from_str::<Self>(&raw).map_err(LightningLabsFundingInteropError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn save_atomic(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), LightningLabsFundingInteropError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(LightningLabsFundingInteropError::Io)?;
            }
        }

        let raw =
            serde_json::to_vec_pretty(self).map_err(LightningLabsFundingInteropError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(LightningLabsFundingInteropError::Io)?;
        fs::rename(&temp_path, path).map_err(LightningLabsFundingInteropError::Io)?;
        Ok(())
    }

    pub fn insert_state(
        &mut self,
        state: LightningLabsFundingInteropState,
    ) -> Result<(), LightningLabsFundingInteropError> {
        if self.states.contains_key(&state.interop_id) {
            return Err(LightningLabsFundingInteropError::DuplicateInteropState(
                state.interop_id,
            ));
        }

        let mut next = self.clone();
        next.states.insert(state.interop_id.clone(), state);
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), LightningLabsFundingInteropError> {
        if self.version != LIGHTNING_LABS_FUNDING_INTEROP_SCHEMA_VERSION {
            return Err(LightningLabsFundingInteropError::UnsupportedVersion(
                self.version,
            ));
        }
        self.metadata.validate()?;

        for (interop_id, state) in &self.states {
            if interop_id != &state.interop_id {
                return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
                    "interop map key {interop_id} does not match state id {}",
                    state.interop_id
                )));
            }
            state.validate()?;
            if state.binding_id() != *interop_id {
                return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
                    "interop state {interop_id} binding hash does not match fields"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingInteropMetadata {
    pub implementation: String,
    pub schema: String,
    pub target: LightningLabsFundingTarget,
}

impl Default for LightningLabsFundingInteropMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk Lightning Labs funding interop".to_owned(),
            schema: "fixture-backed-funding-interop-v1".to_owned(),
            target: LightningLabsFundingTarget::default(),
        }
    }
}

impl LightningLabsFundingInteropMetadata {
    fn validate(&self) -> Result<(), LightningLabsFundingInteropError> {
        for (field, value) in [
            ("implementation", self.implementation.as_str()),
            ("schema", self.schema.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
                    "metadata {field} is empty"
                )));
            }
        }
        self.target.validate()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingTarget {
    pub bitcoind_image: String,
    pub lnd_image: String,
    pub tapd_image: String,
}

impl Default for LightningLabsFundingTarget {
    fn default() -> Self {
        let config = LightningLabsCounterpartyConfig::default();
        Self {
            bitcoind_image: config.bitcoind_image,
            lnd_image: config.lnd_image,
            tapd_image: config.tapd_image,
        }
    }
}

impl LightningLabsFundingTarget {
    fn validate(&self) -> Result<(), LightningLabsFundingInteropError> {
        for (field, value) in [
            ("bitcoind_image", self.bitcoind_image.as_str()),
            ("lnd_image", self.lnd_image.as_str()),
            ("tapd_image", self.tapd_image.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
                    "target {field} is empty"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingInteropState {
    pub interop_id: String,
    pub target: LightningLabsFundingTarget,
    pub status: LightningLabsFundingInteropStatus,
    pub asset_id: Bytes32,
    pub funding_total_amount: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub funding_blob_digest: Bytes32,
    pub commitment_blob_digest: Bytes32,
    pub funding_asset_output_digest: Bytes32,
    pub funding_asset_proof_digest: Bytes32,
    pub local_commitment_output_digest: Bytes32,
    pub remote_commitment_output_digest: Bytes32,
    pub balance_comparison: FundingBalanceComparison,
    pub documented_gap: FundingInteropGap,
}

impl LightningLabsFundingInteropState {
    fn validate(&self) -> Result<(), LightningLabsFundingInteropError> {
        if self.interop_id.trim().is_empty() {
            return Err(LightningLabsFundingInteropError::StorageInvariant(
                "interop id is empty".to_owned(),
            ));
        }
        self.target.validate()?;
        if self.asset_id == Bytes32::ZERO {
            return Err(LightningLabsFundingInteropError::StorageInvariant(
                "asset id is zero".to_owned(),
            ));
        }
        if self.funding_total_amount == 0 {
            return Err(LightningLabsFundingInteropError::MissingAssetOutput);
        }
        if self
            .local_balance
            .checked_add(self.remote_balance)
            .ok_or(LightningLabsFundingInteropError::AmountOverflow)?
            != self.funding_total_amount
        {
            return Err(LightningLabsFundingInteropError::BalanceMismatch {
                funding_total: self.funding_total_amount,
                local_balance: self.local_balance,
                remote_balance: self.remote_balance,
            });
        }
        if !self.balance_comparison.balances_match {
            return Err(LightningLabsFundingInteropError::StorageInvariant(
                "stored balance comparison must match fixture balances".to_owned(),
            ));
        }
        self.documented_gap.validate()?;

        Ok(())
    }

    fn binding_id(&self) -> String {
        derive_interop_id(
            self.asset_id,
            self.funding_total_amount,
            self.local_balance,
            self.remote_balance,
            self.funding_blob_digest,
            self.commitment_blob_digest,
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsFundingInteropStatus {
    StoppedAtDocumentedGap,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FundingBalanceComparison {
    pub funding_total: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub balances_match: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FundingInteropGap {
    pub field: String,
    pub reason: String,
    pub next_step: String,
}

impl FundingInteropGap {
    fn validate(&self) -> Result<(), LightningLabsFundingInteropError> {
        for (field, value) in [
            ("field", self.field.as_str()),
            ("reason", self.reason.as_str()),
            ("next_step", self.next_step.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
                    "documented gap {field} is empty"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingInteropReport {
    pub interop_id: String,
    pub target: LightningLabsFundingTarget,
    pub status: LightningLabsFundingInteropStatus,
    pub asset_id: Bytes32,
    pub funding_total_amount: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub funding_blob_digest: Bytes32,
    pub commitment_blob_digest: Bytes32,
    pub funding_asset_output_digest: Bytes32,
    pub funding_asset_proof_digest: Bytes32,
    pub balance_comparison: FundingBalanceComparison,
    pub documented_gap: FundingInteropGap,
}

#[derive(Debug)]
pub enum LightningLabsFundingInteropError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Blob(LightningLabsBlobError),
    UnsupportedVersion(u32),
    MissingAssetOutput,
    MissingCommitmentOutput(&'static str),
    AssetIdMismatch {
        field: &'static str,
        expected: Bytes32,
        actual: Bytes32,
    },
    BalanceMismatch {
        funding_total: u64,
        local_balance: u64,
        remote_balance: u64,
    },
    AmountOverflow,
    DuplicateInteropState(String),
    StorageInvariant(String),
}

impl fmt::Display for LightningLabsFundingInteropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Lightning Labs funding interop I/O error: {err}"),
            Self::Json(err) => write!(f, "Lightning Labs funding interop JSON error: {err}"),
            Self::Blob(err) => write!(f, "Lightning Labs funding interop blob error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported Lightning Labs funding interop schema version {version}"
                )
            }
            Self::MissingAssetOutput => {
                write!(f, "Lightning Labs funding blob has no asset output")
            }
            Self::MissingCommitmentOutput(side) => {
                write!(
                    f,
                    "Lightning Labs commitment blob has no {side} asset output"
                )
            }
            Self::AssetIdMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "Lightning Labs {field} asset id mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::BalanceMismatch {
                funding_total,
                local_balance,
                remote_balance,
            } => write!(
                f,
                "Lightning Labs funding total {funding_total} does not match local={local_balance} remote={remote_balance}"
            ),
            Self::AmountOverflow => write!(f, "Lightning Labs funding amount overflowed"),
            Self::DuplicateInteropState(interop_id) => {
                write!(
                    f,
                    "duplicate Lightning Labs funding interop state {interop_id}"
                )
            }
            Self::StorageInvariant(message) => {
                write!(
                    f,
                    "Lightning Labs funding interop invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for LightningLabsFundingInteropError {}

pub fn run_lightning_labs_funding_interop_fixture_smoke(
    funding_hexdump: &str,
    commitment_hexdump: &str,
) -> Result<
    (
        LightningLabsFundingInteropStore,
        LightningLabsFundingInteropReport,
    ),
    LightningLabsFundingInteropError,
> {
    let funding = decode_funding_blob_hexdump(funding_hexdump)
        .map_err(LightningLabsFundingInteropError::Blob)?;
    let commitment = decode_commitment_blob_hexdump(commitment_hexdump)
        .map_err(LightningLabsFundingInteropError::Blob)?;
    let state = build_state_from_fixture(&funding, &commitment)?;
    let report = report_from_state(&state);
    let mut store = LightningLabsFundingInteropStore::default();
    store.insert_state(state)?;

    Ok((store, report))
}

fn build_state_from_fixture(
    funding: &LightningLabsFundingBlob,
    commitment: &LightningLabsCommitmentBlob,
) -> Result<LightningLabsFundingInteropState, LightningLabsFundingInteropError> {
    let funding_output = single_output(
        funding.funded_assets.outputs.first(),
        "funding",
        LightningLabsFundingInteropError::MissingAssetOutput,
    )?;
    let local_output = single_output(
        commitment.local_assets.outputs.first(),
        "commitment.local",
        LightningLabsFundingInteropError::MissingCommitmentOutput("local"),
    )?;
    let remote_output = single_output(
        commitment.remote_assets.outputs.first(),
        "commitment.remote",
        LightningLabsFundingInteropError::MissingCommitmentOutput("remote"),
    )?;

    require_asset_id(
        "commitment.local",
        funding_output.asset_id,
        local_output.asset_id,
    )?;
    require_asset_id(
        "commitment.remote",
        funding_output.asset_id,
        remote_output.asset_id,
    )?;

    let local_balance = commitment.local_assets.total_amount;
    let remote_balance = commitment.remote_assets.total_amount;
    let funding_total = funding.funded_assets.total_amount;
    let balances_sum = local_balance
        .checked_add(remote_balance)
        .ok_or(LightningLabsFundingInteropError::AmountOverflow)?;
    if balances_sum != funding_total {
        return Err(LightningLabsFundingInteropError::BalanceMismatch {
            funding_total,
            local_balance,
            remote_balance,
        });
    }

    let interop_id = derive_interop_id(
        funding_output.asset_id,
        funding_total,
        local_balance,
        remote_balance,
        funding.raw_digest,
        commitment.raw_digest,
    );

    Ok(LightningLabsFundingInteropState {
        interop_id,
        target: LightningLabsFundingTarget::default(),
        status: LightningLabsFundingInteropStatus::StoppedAtDocumentedGap,
        asset_id: funding_output.asset_id,
        funding_total_amount: funding_total,
        local_balance,
        remote_balance,
        funding_blob_digest: funding.raw_digest,
        commitment_blob_digest: commitment.raw_digest,
        funding_asset_output_digest: funding_output.output_digest,
        funding_asset_proof_digest: funding_output.proof_digest,
        local_commitment_output_digest: local_output.output_digest,
        remote_commitment_output_digest: remote_output.output_digest,
        balance_comparison: FundingBalanceComparison {
            funding_total,
            local_balance,
            remote_balance,
            balances_match: true,
        },
        documented_gap: FundingInteropGap {
            field: "live_funding_outpoint_and_proof_mapping".to_owned(),
            reason: "static tapchannelmsg fixtures prove the Lightning Labs funding asset ID, total, allocation, and proof/output digests, but they do not by themselves perform the live LND/tapd custom-channel funding handshake or bind a freshly generated funding outpoint to a fully verified proof chain".to_owned(),
            next_step: "drive the headless or Polar-backed Lightning Labs counterparty through channel funding and compare the live funding outpoint, proof file, funding blob, and initial balances against this fixture-backed state".to_owned(),
        },
    })
}

fn report_from_state(
    state: &LightningLabsFundingInteropState,
) -> LightningLabsFundingInteropReport {
    LightningLabsFundingInteropReport {
        interop_id: state.interop_id.clone(),
        target: state.target.clone(),
        status: state.status,
        asset_id: state.asset_id,
        funding_total_amount: state.funding_total_amount,
        local_balance: state.local_balance,
        remote_balance: state.remote_balance,
        funding_blob_digest: state.funding_blob_digest,
        commitment_blob_digest: state.commitment_blob_digest,
        funding_asset_output_digest: state.funding_asset_output_digest,
        funding_asset_proof_digest: state.funding_asset_proof_digest,
        balance_comparison: state.balance_comparison.clone(),
        documented_gap: state.documented_gap.clone(),
    }
}

fn single_output<'a>(
    output: Option<&'a AssetOutputSummary>,
    field: &'static str,
    err: LightningLabsFundingInteropError,
) -> Result<&'a AssetOutputSummary, LightningLabsFundingInteropError> {
    let output = output.ok_or(err)?;
    if output.amount == 0 || output.proof_len == 0 || output.proof_digest == Bytes32::ZERO {
        return Err(LightningLabsFundingInteropError::StorageInvariant(format!(
            "{field} asset output is missing amount or proof material"
        )));
    }

    Ok(output)
}

fn require_asset_id(
    field: &'static str,
    expected: Bytes32,
    actual: Bytes32,
) -> Result<(), LightningLabsFundingInteropError> {
    if actual != expected {
        return Err(LightningLabsFundingInteropError::AssetIdMismatch {
            field,
            expected,
            actual,
        });
    }

    Ok(())
}

fn derive_interop_id(
    asset_id: Bytes32,
    funding_total: u64,
    local_balance: u64,
    remote_balance: u64,
    funding_blob_digest: Bytes32,
    commitment_blob_digest: Bytes32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:lightning-labs-funding-interop:v1");
    hasher.update(asset_id.0);
    hasher.update(funding_total.to_be_bytes());
    hasher.update(local_balance.to_be_bytes());
    hasher.update(remote_balance.to_be_bytes());
    hasher.update(funding_blob_digest.0);
    hasher.update(commitment_blob_digest.0);
    Bytes32(hasher.finalize().into()).to_hex()
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "lightning-labs-funding-interop.json".to_owned());
    path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightning_labs_blob::{
        AssetOutputListSummary, AuxLeavesSummary, HtlcAssetOutputSummary,
        LightningLabsCommitmentBlob, LightningLabsFundingBlob,
    };

    #[test]
    fn fixture_state_rejects_balance_mismatch_and_wrong_asset() {
        let mut funding = synthetic_funding();
        let commitment = synthetic_commitment();
        funding.funded_assets.total_amount += 1;
        assert!(matches!(
            build_state_from_fixture(&funding, &commitment),
            Err(LightningLabsFundingInteropError::BalanceMismatch { .. })
        ));

        let funding = synthetic_funding();
        let mut commitment = synthetic_commitment();
        commitment.remote_assets.outputs[0].asset_id = Bytes32([9; 32]);
        assert!(matches!(
            build_state_from_fixture(&funding, &commitment),
            Err(LightningLabsFundingInteropError::AssetIdMismatch {
                field: "commitment.remote",
                ..
            })
        ));
    }

    fn synthetic_funding() -> LightningLabsFundingBlob {
        LightningLabsFundingBlob {
            raw_len: 1,
            raw_digest: Bytes32([1; 32]),
            decimal_display: 6,
            group_key: None,
            funded_assets: AssetOutputListSummary {
                output_count: 1,
                total_amount: 100,
                value_len: 1,
                value_digest: Bytes32([2; 32]),
                outputs: vec![asset_output(Bytes32([3; 32]), 100, Bytes32([4; 32]))],
            },
        }
    }

    fn synthetic_commitment() -> LightningLabsCommitmentBlob {
        LightningLabsCommitmentBlob {
            raw_len: 1,
            raw_digest: Bytes32([5; 32]),
            local_assets: AssetOutputListSummary {
                output_count: 1,
                total_amount: 60,
                value_len: 1,
                value_digest: Bytes32([6; 32]),
                outputs: vec![asset_output(Bytes32([3; 32]), 60, Bytes32([7; 32]))],
            },
            remote_assets: AssetOutputListSummary {
                output_count: 1,
                total_amount: 40,
                value_len: 1,
                value_digest: Bytes32([8; 32]),
                outputs: vec![asset_output(Bytes32([3; 32]), 40, Bytes32([9; 32]))],
            },
            outgoing_htlcs: empty_htlcs(Bytes32([10; 32])),
            incoming_htlcs: empty_htlcs(Bytes32([11; 32])),
            aux_leaves: AuxLeavesSummary {
                value_len: 1,
                value_digest: Bytes32([12; 32]),
                local_leaf: None,
                remote_leaf: None,
                outgoing_htlc_leaf_count: 0,
                incoming_htlc_leaf_count: 0,
            },
            stxo: None,
        }
    }

    fn asset_output(asset_id: Bytes32, amount: u64, digest: Bytes32) -> AssetOutputSummary {
        AssetOutputSummary {
            asset_id,
            amount,
            proof_len: 1,
            proof_digest: digest,
            output_len: 1,
            output_digest: digest,
        }
    }

    fn empty_htlcs(value_digest: Bytes32) -> HtlcAssetOutputSummary {
        HtlcAssetOutputSummary {
            htlc_count: 0,
            total_amount: 0,
            value_len: 1,
            value_digest,
            entries: Vec::new(),
        }
    }
}
