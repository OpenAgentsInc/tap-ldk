use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_close::{
        NativeAssetCloseError, NativeAssetCloseSmokeReport, run_native_asset_close_smoke,
    },
    asset_recovery::{
        NativeAssetProofRecoveryReport, NativeAssetRecoveryError, NativeAssetRecoveryMatrixReport,
        run_native_asset_recovery_matrix_smoke,
    },
    proof::ProofAnchorState,
};

pub const ONCHAIN_LIFECYCLE_REPORT_SCHEMA_VERSION: u32 = 1;
pub const ONCHAIN_LIFECYCLE_CHAIN_OBSERVATION_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnchainLifecycleEventKind {
    CooperativeCloseLocal,
    CooperativeCloseRemote,
    UnilateralCommitment,
    SecondLevelHtlcSuccess,
    SecondLevelHtlcTimeout,
    FinalSweep,
    FailedSweep,
    BtcOnlySweepRefusal,
    StaleProofOwnershipRefusal,
    MissingProofOwnershipRefusal,
    RestartRecovery,
}

impl OnchainLifecycleEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CooperativeCloseLocal => "cooperative_close_local",
            Self::CooperativeCloseRemote => "cooperative_close_remote",
            Self::UnilateralCommitment => "unilateral_commitment",
            Self::SecondLevelHtlcSuccess => "second_level_htlc_success",
            Self::SecondLevelHtlcTimeout => "second_level_htlc_timeout",
            Self::FinalSweep => "final_sweep",
            Self::FailedSweep => "failed_sweep",
            Self::BtcOnlySweepRefusal => "btc_only_sweep_refusal",
            Self::StaleProofOwnershipRefusal => "stale_proof_ownership_refusal",
            Self::MissingProofOwnershipRefusal => "missing_proof_ownership_refusal",
            Self::RestartRecovery => "restart_recovery",
        }
    }

    fn required_status(self) -> OnchainLifecycleEventStatus {
        match self {
            Self::CooperativeCloseLocal | Self::CooperativeCloseRemote => {
                OnchainLifecycleEventStatus::ProofExported
            }
            Self::UnilateralCommitment
            | Self::SecondLevelHtlcSuccess
            | Self::SecondLevelHtlcTimeout
            | Self::FinalSweep => OnchainLifecycleEventStatus::AssetProofRecovered,
            Self::FailedSweep
            | Self::BtcOnlySweepRefusal
            | Self::StaleProofOwnershipRefusal
            | Self::MissingProofOwnershipRefusal => OnchainLifecycleEventStatus::Refused,
            Self::RestartRecovery => OnchainLifecycleEventStatus::Restarted,
        }
    }

    fn requires_proof_history(self) -> bool {
        matches!(
            self,
            Self::CooperativeCloseLocal
                | Self::CooperativeCloseRemote
                | Self::UnilateralCommitment
                | Self::SecondLevelHtlcSuccess
                | Self::SecondLevelHtlcTimeout
                | Self::FinalSweep
                | Self::RestartRecovery
        )
    }

    fn requires_proof_handoff(self) -> bool {
        matches!(
            self,
            Self::CooperativeCloseLocal
                | Self::CooperativeCloseRemote
                | Self::UnilateralCommitment
                | Self::SecondLevelHtlcSuccess
                | Self::SecondLevelHtlcTimeout
                | Self::FinalSweep
        )
    }

    fn requires_sweep_output(self) -> bool {
        matches!(
            self,
            Self::UnilateralCommitment
                | Self::SecondLevelHtlcSuccess
                | Self::SecondLevelHtlcTimeout
                | Self::FinalSweep
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnchainLifecycleEventStatus {
    ProofExported,
    AssetProofRecovered,
    Refused,
    Restarted,
}

impl OnchainLifecycleEventStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProofExported => "proof_exported",
            Self::AssetProofRecovered => "asset_proof_recovered",
            Self::Refused => "refused",
            Self::Restarted => "restarted",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnchainLifecycleObservationSource {
    ChainWatcher,
    Sweeper,
    WalletMonitor,
}

impl OnchainLifecycleObservationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChainWatcher => "chain_watcher",
            Self::Sweeper => "sweeper",
            Self::WalletMonitor => "wallet_monitor",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnchainLifecycleObservationKind {
    CooperativeCloseAnchor,
    UnilateralCommitmentAnchor,
    SecondLevelHtlcAnchor,
    FinalSweepAnchor,
    FailedSweep,
    BtcOnlySweepRefusal,
    StaleProofOwnershipAnchor,
    MissingProofOwnershipRefusal,
    RestartEvidence,
}

impl OnchainLifecycleObservationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CooperativeCloseAnchor => "cooperative_close_anchor",
            Self::UnilateralCommitmentAnchor => "unilateral_commitment_anchor",
            Self::SecondLevelHtlcAnchor => "second_level_htlc_anchor",
            Self::FinalSweepAnchor => "final_sweep_anchor",
            Self::FailedSweep => "failed_sweep",
            Self::BtcOnlySweepRefusal => "btc_only_sweep_refusal",
            Self::StaleProofOwnershipAnchor => "stale_proof_ownership_anchor",
            Self::MissingProofOwnershipRefusal => "missing_proof_ownership_refusal",
            Self::RestartEvidence => "restart_evidence",
        }
    }

    fn required_source(self) -> OnchainLifecycleObservationSource {
        match self {
            Self::CooperativeCloseAnchor
            | Self::UnilateralCommitmentAnchor
            | Self::SecondLevelHtlcAnchor
            | Self::StaleProofOwnershipAnchor => OnchainLifecycleObservationSource::ChainWatcher,
            Self::FinalSweepAnchor | Self::FailedSweep | Self::BtcOnlySweepRefusal => {
                OnchainLifecycleObservationSource::Sweeper
            }
            Self::MissingProofOwnershipRefusal | Self::RestartEvidence => {
                OnchainLifecycleObservationSource::WalletMonitor
            }
        }
    }

    fn required_status(self) -> OnchainLifecycleEventStatus {
        match self {
            Self::CooperativeCloseAnchor => OnchainLifecycleEventStatus::ProofExported,
            Self::UnilateralCommitmentAnchor
            | Self::SecondLevelHtlcAnchor
            | Self::FinalSweepAnchor => OnchainLifecycleEventStatus::AssetProofRecovered,
            Self::FailedSweep
            | Self::BtcOnlySweepRefusal
            | Self::StaleProofOwnershipAnchor
            | Self::MissingProofOwnershipRefusal => OnchainLifecycleEventStatus::Refused,
            Self::RestartEvidence => OnchainLifecycleEventStatus::Restarted,
        }
    }

    fn matches_lifecycle_kind(self, kind: OnchainLifecycleEventKind) -> bool {
        match self {
            Self::CooperativeCloseAnchor => matches!(
                kind,
                OnchainLifecycleEventKind::CooperativeCloseLocal
                    | OnchainLifecycleEventKind::CooperativeCloseRemote
            ),
            Self::UnilateralCommitmentAnchor => {
                kind == OnchainLifecycleEventKind::UnilateralCommitment
            }
            Self::SecondLevelHtlcAnchor => matches!(
                kind,
                OnchainLifecycleEventKind::SecondLevelHtlcSuccess
                    | OnchainLifecycleEventKind::SecondLevelHtlcTimeout
            ),
            Self::FinalSweepAnchor => kind == OnchainLifecycleEventKind::FinalSweep,
            Self::FailedSweep => kind == OnchainLifecycleEventKind::FailedSweep,
            Self::BtcOnlySweepRefusal => kind == OnchainLifecycleEventKind::BtcOnlySweepRefusal,
            Self::StaleProofOwnershipAnchor => {
                kind == OnchainLifecycleEventKind::StaleProofOwnershipRefusal
            }
            Self::MissingProofOwnershipRefusal => {
                kind == OnchainLifecycleEventKind::MissingProofOwnershipRefusal
            }
            Self::RestartEvidence => kind == OnchainLifecycleEventKind::RestartRecovery,
        }
    }

    fn requires_anchor_outpoint(self) -> bool {
        matches!(
            self,
            Self::CooperativeCloseAnchor
                | Self::UnilateralCommitmentAnchor
                | Self::SecondLevelHtlcAnchor
                | Self::FinalSweepAnchor
                | Self::StaleProofOwnershipAnchor
        )
    }

    fn requires_sweep_output(self) -> bool {
        matches!(
            self,
            Self::FinalSweepAnchor | Self::FailedSweep | Self::BtcOnlySweepRefusal
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OnchainLifecycleObservation {
    pub observation_id: String,
    pub lifecycle_event_id: String,
    pub lifecycle_event_kind: OnchainLifecycleEventKind,
    pub lifecycle_event_status: OnchainLifecycleEventStatus,
    pub source: OnchainLifecycleObservationSource,
    pub kind: OnchainLifecycleObservationKind,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub amount: u64,
    pub anchor_state: ProofAnchorState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_outpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_output_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_evidence_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_evidence_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
    pub observation_digest: Bytes32,
}

impl OnchainLifecycleObservation {
    pub fn new(
        kind: OnchainLifecycleObservationKind,
        source: OnchainLifecycleObservationSource,
        lifecycle_event: &OnchainLifecycleEvent,
        anchor_state: ProofAnchorState,
    ) -> Self {
        let mut observation = Self {
            observation_id: observation_id_for(kind, source, lifecycle_event),
            lifecycle_event_id: lifecycle_event.event_id.clone(),
            lifecycle_event_kind: lifecycle_event.kind,
            lifecycle_event_status: lifecycle_event.status,
            source,
            kind,
            channel_id: lifecycle_event.channel_id.clone(),
            asset_id: lifecycle_event.asset_id,
            amount: lifecycle_event.amount,
            anchor_state,
            height: None,
            anchor_outpoint: None,
            sweep_output_digest: None,
            wallet_evidence_digest: None,
            monitor_evidence_digest: None,
            refusal_reason: None,
            observation_digest: Bytes32::ZERO,
        };
        observation.observation_digest = observation.digest();
        observation
    }

    pub fn with_height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self.refresh_digest()
    }

    pub fn with_anchor_outpoint(mut self, anchor_outpoint: String) -> Self {
        self.anchor_outpoint = Some(anchor_outpoint);
        self.refresh_digest()
    }

    pub fn with_sweep_output(mut self, digest: Bytes32) -> Self {
        self.sweep_output_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_wallet_evidence(mut self, digest: Bytes32) -> Self {
        self.wallet_evidence_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_monitor_evidence(mut self, digest: Bytes32) -> Self {
        self.monitor_evidence_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_refusal_reason(mut self, reason: String) -> Self {
        self.refusal_reason = Some(reason);
        self.refresh_digest()
    }

    pub fn validate(&self) -> Result<(), OnchainLifecycleError> {
        require_nonempty("observation_id", &self.observation_id)?;
        require_nonempty("lifecycle_event_id", &self.lifecycle_event_id)?;
        require_nonempty("channel_id", &self.channel_id)?;
        if self.asset_id == Bytes32::ZERO {
            return Err(OnchainLifecycleError::ObservationZeroAssetId {
                observation_id: self.observation_id.clone(),
            });
        }
        if self.amount == 0 {
            return Err(OnchainLifecycleError::ObservationZeroAmount {
                observation_id: self.observation_id.clone(),
            });
        }
        if self.source != self.kind.required_source() {
            return Err(OnchainLifecycleError::ObservationWrongSource {
                observation_id: self.observation_id.clone(),
                kind: self.kind,
                source: self.source,
            });
        }
        if !self.kind.matches_lifecycle_kind(self.lifecycle_event_kind) {
            return Err(OnchainLifecycleError::ObservationWrongLifecycleKind {
                observation_id: self.observation_id.clone(),
                kind: self.kind,
                lifecycle_event_kind: self.lifecycle_event_kind,
            });
        }
        if self.lifecycle_event_status != self.kind.required_status() {
            return Err(OnchainLifecycleError::ObservationWrongLifecycleStatus {
                observation_id: self.observation_id.clone(),
                kind: self.kind,
                lifecycle_event_status: self.lifecycle_event_status,
            });
        }
        self.validate_anchor_state()?;
        if self.kind.requires_anchor_outpoint() && empty_optional(self.anchor_outpoint.as_deref()) {
            return Err(OnchainLifecycleError::MissingObservationEvidence {
                observation_id: self.observation_id.clone(),
                field: "anchor_outpoint",
            });
        }
        if self.kind.requires_sweep_output() && self.sweep_output_digest.is_none() {
            return Err(OnchainLifecycleError::MissingObservationEvidence {
                observation_id: self.observation_id.clone(),
                field: "sweep_output_digest",
            });
        }
        if self.kind == OnchainLifecycleObservationKind::RestartEvidence
            && (self.wallet_evidence_digest.is_none() || self.monitor_evidence_digest.is_none())
        {
            return Err(OnchainLifecycleError::MissingObservationEvidence {
                observation_id: self.observation_id.clone(),
                field: "restart_wallet_and_monitor_evidence",
            });
        }
        if self.lifecycle_event_status == OnchainLifecycleEventStatus::Refused
            && empty_optional(self.refusal_reason.as_deref())
        {
            return Err(OnchainLifecycleError::MissingObservationEvidence {
                observation_id: self.observation_id.clone(),
                field: "refusal_reason",
            });
        }
        if self.lifecycle_event_status != OnchainLifecycleEventStatus::Refused
            && self.refusal_reason.is_some()
        {
            return Err(OnchainLifecycleError::UnexpectedObservationRefusalReason {
                observation_id: self.observation_id.clone(),
            });
        }
        if self.observation_digest != self.digest() {
            return Err(OnchainLifecycleError::ObservationDigestMismatch {
                observation_id: self.observation_id.clone(),
            });
        }
        Ok(())
    }

    fn validate_anchor_state(&self) -> Result<(), OnchainLifecycleError> {
        let valid = match self.kind {
            OnchainLifecycleObservationKind::CooperativeCloseAnchor
            | OnchainLifecycleObservationKind::UnilateralCommitmentAnchor
            | OnchainLifecycleObservationKind::SecondLevelHtlcAnchor
            | OnchainLifecycleObservationKind::FinalSweepAnchor
            | OnchainLifecycleObservationKind::RestartEvidence => {
                self.anchor_state == ProofAnchorState::Confirmed
            }
            OnchainLifecycleObservationKind::FailedSweep => {
                !matches!(self.anchor_state, ProofAnchorState::Confirmed)
            }
            OnchainLifecycleObservationKind::BtcOnlySweepRefusal => true,
            OnchainLifecycleObservationKind::StaleProofOwnershipAnchor => {
                matches!(
                    self.anchor_state,
                    ProofAnchorState::Stale | ProofAnchorState::Reorged
                )
            }
            OnchainLifecycleObservationKind::MissingProofOwnershipRefusal => {
                self.anchor_state == ProofAnchorState::Unknown
            }
        };
        if valid {
            Ok(())
        } else {
            Err(OnchainLifecycleError::ObservationWrongAnchorState {
                observation_id: self.observation_id.clone(),
                kind: self.kind,
                anchor_state: self.anchor_state,
            })
        }
    }

    fn refresh_digest(mut self) -> Self {
        self.observation_digest = self.digest();
        self
    }

    fn digest(&self) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:onchain-lifecycle-observation:v1");
        hash_string(&mut hasher, &self.observation_id);
        hash_string(&mut hasher, &self.lifecycle_event_id);
        hash_string(&mut hasher, self.lifecycle_event_kind.as_str());
        hash_string(&mut hasher, self.lifecycle_event_status.as_str());
        hash_string(&mut hasher, self.source.as_str());
        hash_string(&mut hasher, self.kind.as_str());
        hash_string(&mut hasher, &self.channel_id);
        hasher.update(self.asset_id.0);
        hasher.update(self.amount.to_be_bytes());
        hash_string(&mut hasher, self.anchor_state.as_str());
        hash_optional_u32(&mut hasher, self.height);
        hash_optional_string(&mut hasher, self.anchor_outpoint.as_deref());
        hash_optional_bytes32(&mut hasher, self.sweep_output_digest);
        hash_optional_bytes32(&mut hasher, self.wallet_evidence_digest);
        hash_optional_bytes32(&mut hasher, self.monitor_evidence_digest);
        hash_optional_string(&mut hasher, self.refusal_reason.as_deref());
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OnchainLifecycleEvent {
    pub event_id: String,
    pub kind: OnchainLifecycleEventKind,
    pub status: OnchainLifecycleEventStatus,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub amount: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_history_output_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_handoff_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_evidence_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_evidence_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep_output_digest: Option<Bytes32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
    pub event_digest: Bytes32,
}

impl OnchainLifecycleEvent {
    pub fn new(
        kind: OnchainLifecycleEventKind,
        channel_id: String,
        asset_id: Bytes32,
        amount: u64,
    ) -> Self {
        let mut event = Self {
            event_id: event_id_for(kind, &channel_id, asset_id, amount),
            kind,
            status: kind.required_status(),
            channel_id,
            asset_id,
            amount,
            proof_history_output_id: None,
            proof_handoff_digest: None,
            wallet_evidence_digest: None,
            monitor_evidence_digest: None,
            sweep_output_digest: None,
            refusal_reason: None,
            event_digest: Bytes32::ZERO,
        };
        event.event_digest = event.digest();
        event
    }

    pub fn with_proof_history(mut self, proof_history_output_id: String) -> Self {
        self.proof_history_output_id = Some(proof_history_output_id);
        self.refresh_digest()
    }

    pub fn with_proof_handoff(mut self, digest: Bytes32) -> Self {
        self.proof_handoff_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_wallet_evidence(mut self, digest: Bytes32) -> Self {
        self.wallet_evidence_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_monitor_evidence(mut self, digest: Bytes32) -> Self {
        self.monitor_evidence_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_sweep_output(mut self, digest: Bytes32) -> Self {
        self.sweep_output_digest = Some(digest);
        self.refresh_digest()
    }

    pub fn with_refusal_reason(mut self, reason: String) -> Self {
        self.refusal_reason = Some(reason);
        self.refresh_digest()
    }

    pub fn validate(&self) -> Result<(), OnchainLifecycleError> {
        require_nonempty("event_id", &self.event_id)?;
        require_nonempty("channel_id", &self.channel_id)?;
        if self.asset_id == Bytes32::ZERO {
            return Err(OnchainLifecycleError::ZeroAssetId {
                event_id: self.event_id.clone(),
            });
        }
        if self.amount == 0 {
            return Err(OnchainLifecycleError::ZeroAmount {
                event_id: self.event_id.clone(),
            });
        }
        if self.status != self.kind.required_status() {
            return Err(OnchainLifecycleError::WrongStatus {
                event_id: self.event_id.clone(),
                kind: self.kind,
                status: self.status,
            });
        }
        if self.kind.requires_proof_history()
            && empty_optional(self.proof_history_output_id.as_deref())
        {
            return Err(OnchainLifecycleError::MissingProofHistory {
                event_id: self.event_id.clone(),
            });
        }
        if self.kind.requires_proof_handoff() && self.proof_handoff_digest.is_none() {
            return Err(OnchainLifecycleError::MissingProofHandoff {
                event_id: self.event_id.clone(),
            });
        }
        if self.kind.requires_sweep_output() && self.sweep_output_digest.is_none() {
            return Err(OnchainLifecycleError::MissingSweepOutput {
                event_id: self.event_id.clone(),
            });
        }
        if self.kind == OnchainLifecycleEventKind::RestartRecovery
            && (self.wallet_evidence_digest.is_none() || self.monitor_evidence_digest.is_none())
        {
            return Err(OnchainLifecycleError::MissingRestartEvidence {
                event_id: self.event_id.clone(),
            });
        }
        if self.status == OnchainLifecycleEventStatus::Refused
            && empty_optional(self.refusal_reason.as_deref())
        {
            return Err(OnchainLifecycleError::MissingRefusalReason {
                event_id: self.event_id.clone(),
            });
        }
        if self.status != OnchainLifecycleEventStatus::Refused && self.refusal_reason.is_some() {
            return Err(OnchainLifecycleError::UnexpectedRefusalReason {
                event_id: self.event_id.clone(),
            });
        }
        if self.event_digest != self.digest() {
            return Err(OnchainLifecycleError::DigestMismatch {
                event_id: self.event_id.clone(),
            });
        }
        Ok(())
    }

    fn refresh_digest(mut self) -> Self {
        self.event_digest = self.digest();
        self
    }

    fn digest(&self) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:onchain-lifecycle-event:v1");
        hash_string(&mut hasher, &self.event_id);
        hash_string(&mut hasher, self.kind.as_str());
        hash_string(&mut hasher, self.status.as_str());
        hash_string(&mut hasher, &self.channel_id);
        hasher.update(self.asset_id.0);
        hasher.update(self.amount.to_be_bytes());
        hash_optional_string(&mut hasher, self.proof_history_output_id.as_deref());
        hash_optional_bytes32(&mut hasher, self.proof_handoff_digest);
        hash_optional_bytes32(&mut hasher, self.wallet_evidence_digest);
        hash_optional_bytes32(&mut hasher, self.monitor_evidence_digest);
        hash_optional_bytes32(&mut hasher, self.sweep_output_digest);
        hash_optional_string(&mut hasher, self.refusal_reason.as_deref());
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OnchainLifecycleReport {
    pub version: u32,
    pub events: Vec<OnchainLifecycleEvent>,
    pub cooperative_close_exported: bool,
    pub unilateral_recovery_explained: bool,
    pub second_level_success_explained: bool,
    pub second_level_timeout_explained: bool,
    pub final_sweep_explained: bool,
    pub failed_sweep_refused: bool,
    pub btc_only_sweep_refused: bool,
    pub restart_recovery_explained: bool,
    pub live_chain_watcher_backed: bool,
    pub production_ready: bool,
}

impl OnchainLifecycleReport {
    pub fn new(events: Vec<OnchainLifecycleEvent>) -> Result<Self, OnchainLifecycleError> {
        let report = Self {
            version: ONCHAIN_LIFECYCLE_REPORT_SCHEMA_VERSION,
            cooperative_close_exported: has_proof_export(
                &events,
                OnchainLifecycleEventKind::CooperativeCloseLocal,
            ) && has_proof_export(
                &events,
                OnchainLifecycleEventKind::CooperativeCloseRemote,
            ),
            unilateral_recovery_explained: has_recovered(
                &events,
                OnchainLifecycleEventKind::UnilateralCommitment,
            ),
            second_level_success_explained: has_recovered(
                &events,
                OnchainLifecycleEventKind::SecondLevelHtlcSuccess,
            ),
            second_level_timeout_explained: has_recovered(
                &events,
                OnchainLifecycleEventKind::SecondLevelHtlcTimeout,
            ),
            final_sweep_explained: has_recovered(&events, OnchainLifecycleEventKind::FinalSweep),
            failed_sweep_refused: has_refusal(&events, OnchainLifecycleEventKind::FailedSweep),
            btc_only_sweep_refused: has_refusal(
                &events,
                OnchainLifecycleEventKind::BtcOnlySweepRefusal,
            ),
            restart_recovery_explained: has_status(
                &events,
                OnchainLifecycleEventKind::RestartRecovery,
                OnchainLifecycleEventStatus::Restarted,
            ),
            live_chain_watcher_backed: false,
            production_ready: false,
            events,
        };
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), OnchainLifecycleError> {
        if self.version != ONCHAIN_LIFECYCLE_REPORT_SCHEMA_VERSION {
            return Err(OnchainLifecycleError::UnsupportedVersion(self.version));
        }
        if self.events.is_empty() {
            return Err(OnchainLifecycleError::EmptyReport);
        }
        if self.production_ready {
            return Err(OnchainLifecycleError::UnsupportedProductionClaim);
        }

        let mut ids = BTreeSet::<String>::new();
        for event in &self.events {
            event.validate()?;
            if !ids.insert(event.event_id.clone()) {
                return Err(OnchainLifecycleError::DuplicateEvent {
                    event_id: event.event_id.clone(),
                });
            }
        }

        self.expect_summary(
            "cooperative_close_exported",
            self.cooperative_close_exported,
            has_proof_export(
                &self.events,
                OnchainLifecycleEventKind::CooperativeCloseLocal,
            ) && has_proof_export(
                &self.events,
                OnchainLifecycleEventKind::CooperativeCloseRemote,
            ),
        )?;
        self.expect_summary(
            "unilateral_recovery_explained",
            self.unilateral_recovery_explained,
            has_recovered(
                &self.events,
                OnchainLifecycleEventKind::UnilateralCommitment,
            ),
        )?;
        self.expect_summary(
            "second_level_success_explained",
            self.second_level_success_explained,
            has_recovered(
                &self.events,
                OnchainLifecycleEventKind::SecondLevelHtlcSuccess,
            ),
        )?;
        self.expect_summary(
            "second_level_timeout_explained",
            self.second_level_timeout_explained,
            has_recovered(
                &self.events,
                OnchainLifecycleEventKind::SecondLevelHtlcTimeout,
            ),
        )?;
        self.expect_summary(
            "final_sweep_explained",
            self.final_sweep_explained,
            has_recovered(&self.events, OnchainLifecycleEventKind::FinalSweep),
        )?;
        self.expect_summary(
            "failed_sweep_refused",
            self.failed_sweep_refused,
            has_refusal(&self.events, OnchainLifecycleEventKind::FailedSweep),
        )?;
        self.expect_summary(
            "btc_only_sweep_refused",
            self.btc_only_sweep_refused,
            has_refusal(&self.events, OnchainLifecycleEventKind::BtcOnlySweepRefusal),
        )?;
        self.expect_summary(
            "restart_recovery_explained",
            self.restart_recovery_explained,
            has_status(
                &self.events,
                OnchainLifecycleEventKind::RestartRecovery,
                OnchainLifecycleEventStatus::Restarted,
            ),
        )?;

        Ok(())
    }

    fn expect_summary(
        &self,
        field: &'static str,
        claimed: bool,
        actual: bool,
    ) -> Result<(), OnchainLifecycleError> {
        if claimed != actual {
            Err(OnchainLifecycleError::SummaryMismatch { field })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct OnchainLifecycleChainObservationReport {
    pub version: u32,
    pub lifecycle_report: OnchainLifecycleReport,
    pub observations: Vec<OnchainLifecycleObservation>,
    pub all_lifecycle_events_observed: bool,
    pub confirmed_recovery_observed: bool,
    pub refusal_observations_present: bool,
    pub restart_observation_present: bool,
    pub live_chain_watcher_backed: bool,
    pub production_ready: bool,
}

impl OnchainLifecycleChainObservationReport {
    pub fn new(
        lifecycle_report: OnchainLifecycleReport,
        observations: Vec<OnchainLifecycleObservation>,
    ) -> Result<Self, OnchainLifecycleError> {
        let report = Self {
            version: ONCHAIN_LIFECYCLE_CHAIN_OBSERVATION_REPORT_SCHEMA_VERSION,
            all_lifecycle_events_observed: all_lifecycle_events_observed(
                &lifecycle_report.events,
                &observations,
            ),
            confirmed_recovery_observed: confirmed_recovery_observed(
                &lifecycle_report.events,
                &observations,
            ),
            refusal_observations_present: refusal_observations_present(
                &lifecycle_report.events,
                &observations,
            ),
            restart_observation_present: restart_observation_present(&observations),
            live_chain_watcher_backed: false,
            production_ready: false,
            lifecycle_report,
            observations,
        };
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), OnchainLifecycleError> {
        if self.version != ONCHAIN_LIFECYCLE_CHAIN_OBSERVATION_REPORT_SCHEMA_VERSION {
            return Err(OnchainLifecycleError::UnsupportedObservationReportVersion(
                self.version,
            ));
        }
        self.lifecycle_report.validate()?;
        if self.observations.is_empty() {
            return Err(OnchainLifecycleError::EmptyObservationReport);
        }
        if self.live_chain_watcher_backed {
            return Err(OnchainLifecycleError::UnsupportedLiveChainWatcherClaim);
        }
        if self.production_ready {
            return Err(OnchainLifecycleError::UnsupportedProductionClaim);
        }

        let events_by_id = self
            .lifecycle_report
            .events
            .iter()
            .map(|event| (event.event_id.clone(), event))
            .collect::<BTreeMap<_, _>>();
        let mut observation_ids = BTreeSet::<String>::new();
        for observation in &self.observations {
            observation.validate()?;
            if !observation_ids.insert(observation.observation_id.clone()) {
                return Err(OnchainLifecycleError::DuplicateObservation {
                    observation_id: observation.observation_id.clone(),
                });
            }
            let event = events_by_id
                .get(&observation.lifecycle_event_id)
                .ok_or_else(|| OnchainLifecycleError::ObservationUnknownLifecycleEvent {
                    observation_id: observation.observation_id.clone(),
                    lifecycle_event_id: observation.lifecycle_event_id.clone(),
                })?;
            if observation.lifecycle_event_kind != event.kind
                || observation.lifecycle_event_status != event.status
                || observation.channel_id != event.channel_id
                || observation.asset_id != event.asset_id
                || observation.amount != event.amount
            {
                return Err(OnchainLifecycleError::ObservationLifecycleEventMismatch {
                    observation_id: observation.observation_id.clone(),
                    lifecycle_event_id: observation.lifecycle_event_id.clone(),
                });
            }
        }

        self.expect_summary(
            "all_lifecycle_events_observed",
            self.all_lifecycle_events_observed,
            all_lifecycle_events_observed(&self.lifecycle_report.events, &self.observations),
        )?;
        self.expect_summary(
            "confirmed_recovery_observed",
            self.confirmed_recovery_observed,
            confirmed_recovery_observed(&self.lifecycle_report.events, &self.observations),
        )?;
        self.expect_summary(
            "refusal_observations_present",
            self.refusal_observations_present,
            refusal_observations_present(&self.lifecycle_report.events, &self.observations),
        )?;
        self.expect_summary(
            "restart_observation_present",
            self.restart_observation_present,
            restart_observation_present(&self.observations),
        )?;

        Ok(())
    }

    fn expect_summary(
        &self,
        field: &'static str,
        claimed: bool,
        actual: bool,
    ) -> Result<(), OnchainLifecycleError> {
        if claimed != actual {
            Err(OnchainLifecycleError::ObservationSummaryMismatch { field })
        } else {
            Ok(())
        }
    }
}

pub fn run_onchain_lifecycle_smoke() -> Result<OnchainLifecycleReport, OnchainLifecycleError> {
    let close = run_native_asset_close_smoke()?;
    let recovery = run_native_asset_recovery_matrix_smoke()?;

    require_lifecycle_evidence(
        "close_proof_history_replayed",
        close.close_proof_history_replayed,
    )?;
    require_lifecycle_evidence(
        "proof_export_history_replayed",
        close.proof_export_history_replayed,
    )?;
    require_lifecycle_evidence(
        "local_wallet_export_matches_close_output",
        close.local_wallet_export_matches_close_output,
    )?;
    require_lifecycle_evidence(
        "remote_wallet_export_matches_close_output",
        close.remote_wallet_export_matches_close_output,
    )?;
    require_lifecycle_evidence(
        "restart_after_close_matches",
        close.restart_after_close_matches,
    )?;
    require_lifecycle_evidence(
        "failed_sweep_not_reported_recovered",
        close.failed_sweep_not_reported_recovered,
    )?;
    require_lifecycle_evidence(
        "normal_btc_restart_unaffected",
        recovery.normal_btc_restart_unaffected,
    )?;
    require_lifecycle_evidence(
        "missing_proof_ownership_refused",
        recovery.missing_proof_ownership_refused,
    )?;
    require_lifecycle_evidence(
        "stale_proof_ownership_refused",
        recovery.stale_proof_ownership_refused,
    )?;
    require_lifecycle_evidence(
        "btc_sweep_without_asset_proof_refused",
        recovery.btc_sweep_without_asset_proof_refused,
    )?;

    OnchainLifecycleReport::new(vec![
        cooperative_close_event(
            OnchainLifecycleEventKind::CooperativeCloseLocal,
            &close,
            close.local_amount,
            close.local_export_proof_history_output_id.clone(),
            close.local_proof_digest,
        ),
        cooperative_close_event(
            OnchainLifecycleEventKind::CooperativeCloseRemote,
            &close,
            close.remote_amount,
            close.remote_export_proof_history_output_id.clone(),
            close.remote_proof_digest,
        ),
        recovery_event(
            OnchainLifecycleEventKind::UnilateralCommitment,
            &recovery.force_close_recovery,
        ),
        recovery_event(
            OnchainLifecycleEventKind::SecondLevelHtlcSuccess,
            &recovery.second_level_htlc_recovery,
        ),
        recovery_event(
            OnchainLifecycleEventKind::SecondLevelHtlcTimeout,
            &recovery.second_level_htlc_recovery,
        ),
        recovery_event(
            OnchainLifecycleEventKind::FinalSweep,
            &recovery.final_sweep_recovery,
        ),
        refusal_event(
            OnchainLifecycleEventKind::FailedSweep,
            &close.channel_id,
            close.asset_id,
            close.total_amount,
            "failed sweep was not reported as recovered asset ownership",
        ),
        refusal_event(
            OnchainLifecycleEventKind::BtcOnlySweepRefusal,
            &recovery.final_sweep_recovery.channel_id,
            recovery.final_sweep_recovery.asset_id,
            recovery.final_sweep_recovery.proof_root_sum,
            "BTC-only sweep was refused as asset recovery",
        ),
        refusal_event(
            OnchainLifecycleEventKind::StaleProofOwnershipRefusal,
            &recovery.force_close_recovery.channel_id,
            recovery.force_close_recovery.asset_id,
            recovery.force_close_recovery.proof_root_sum,
            "stale proof ownership state was refused",
        ),
        refusal_event(
            OnchainLifecycleEventKind::MissingProofOwnershipRefusal,
            &recovery.force_close_recovery.channel_id,
            recovery.force_close_recovery.asset_id,
            recovery.force_close_recovery.proof_root_sum,
            "missing proof ownership state was refused",
        ),
        restart_event(&close, &recovery),
    ])
}

pub fn run_chain_watcher_lifecycle_smoke()
-> Result<OnchainLifecycleChainObservationReport, OnchainLifecycleError> {
    let lifecycle_report = run_onchain_lifecycle_smoke()?;
    let observations = lifecycle_report
        .events
        .iter()
        .map(bounded_chain_observation_for_event)
        .collect::<Vec<_>>();
    OnchainLifecycleChainObservationReport::new(lifecycle_report, observations)
}

#[derive(Debug)]
pub enum OnchainLifecycleError {
    NativeClose(NativeAssetCloseError),
    NativeRecovery(NativeAssetRecoveryError),
    MissingLifecycleEvidence(&'static str),
    UnsupportedVersion(u32),
    UnsupportedObservationReportVersion(u32),
    UnsupportedProductionClaim,
    UnsupportedLiveChainWatcherClaim,
    EmptyReport,
    EmptyObservationReport,
    MissingField(&'static str),
    DuplicateEvent {
        event_id: String,
    },
    DuplicateObservation {
        observation_id: String,
    },
    ZeroAssetId {
        event_id: String,
    },
    ZeroAmount {
        event_id: String,
    },
    WrongStatus {
        event_id: String,
        kind: OnchainLifecycleEventKind,
        status: OnchainLifecycleEventStatus,
    },
    ObservationZeroAssetId {
        observation_id: String,
    },
    ObservationZeroAmount {
        observation_id: String,
    },
    ObservationWrongSource {
        observation_id: String,
        kind: OnchainLifecycleObservationKind,
        source: OnchainLifecycleObservationSource,
    },
    ObservationWrongLifecycleKind {
        observation_id: String,
        kind: OnchainLifecycleObservationKind,
        lifecycle_event_kind: OnchainLifecycleEventKind,
    },
    ObservationWrongLifecycleStatus {
        observation_id: String,
        kind: OnchainLifecycleObservationKind,
        lifecycle_event_status: OnchainLifecycleEventStatus,
    },
    ObservationWrongAnchorState {
        observation_id: String,
        kind: OnchainLifecycleObservationKind,
        anchor_state: ProofAnchorState,
    },
    MissingObservationEvidence {
        observation_id: String,
        field: &'static str,
    },
    UnexpectedObservationRefusalReason {
        observation_id: String,
    },
    ObservationDigestMismatch {
        observation_id: String,
    },
    ObservationUnknownLifecycleEvent {
        observation_id: String,
        lifecycle_event_id: String,
    },
    ObservationLifecycleEventMismatch {
        observation_id: String,
        lifecycle_event_id: String,
    },
    ObservationSummaryMismatch {
        field: &'static str,
    },
    MissingProofHistory {
        event_id: String,
    },
    MissingProofHandoff {
        event_id: String,
    },
    MissingSweepOutput {
        event_id: String,
    },
    MissingRestartEvidence {
        event_id: String,
    },
    MissingRefusalReason {
        event_id: String,
    },
    UnexpectedRefusalReason {
        event_id: String,
    },
    DigestMismatch {
        event_id: String,
    },
    SummaryMismatch {
        field: &'static str,
    },
}

impl fmt::Display for OnchainLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeClose(err) => write!(f, "native close smoke failed: {err}"),
            Self::NativeRecovery(err) => write!(f, "native recovery smoke failed: {err}"),
            Self::MissingLifecycleEvidence(field) => {
                write!(
                    f,
                    "on-chain lifecycle missing required smoke evidence {field}"
                )
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported on-chain lifecycle schema version {version}")
            }
            Self::UnsupportedObservationReportVersion(version) => write!(
                f,
                "unsupported on-chain lifecycle observation report schema version {version}"
            ),
            Self::UnsupportedProductionClaim => {
                write!(
                    f,
                    "bounded on-chain lifecycle report cannot claim production readiness"
                )
            }
            Self::UnsupportedLiveChainWatcherClaim => write!(
                f,
                "bounded on-chain lifecycle observation report cannot claim live chain watcher backing"
            ),
            Self::EmptyReport => write!(f, "on-chain lifecycle report is empty"),
            Self::EmptyObservationReport => {
                write!(f, "on-chain lifecycle observation report is empty")
            }
            Self::MissingField(field) => write!(f, "on-chain lifecycle missing {field}"),
            Self::DuplicateEvent { event_id } => {
                write!(f, "duplicate on-chain lifecycle event {event_id}")
            }
            Self::DuplicateObservation { observation_id } => {
                write!(
                    f,
                    "duplicate on-chain lifecycle observation {observation_id}"
                )
            }
            Self::ZeroAssetId { event_id } => {
                write!(f, "on-chain lifecycle event {event_id} has zero asset id")
            }
            Self::ZeroAmount { event_id } => {
                write!(f, "on-chain lifecycle event {event_id} has zero amount")
            }
            Self::WrongStatus {
                event_id,
                kind,
                status,
            } => write!(
                f,
                "on-chain lifecycle event {event_id} has status {} for kind {}",
                status.as_str(),
                kind.as_str()
            ),
            Self::ObservationZeroAssetId { observation_id } => {
                write!(
                    f,
                    "on-chain lifecycle observation {observation_id} has zero asset id"
                )
            }
            Self::ObservationZeroAmount { observation_id } => {
                write!(
                    f,
                    "on-chain lifecycle observation {observation_id} has zero amount"
                )
            }
            Self::ObservationWrongSource {
                observation_id,
                kind,
                source,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} has source {} for kind {}",
                source.as_str(),
                kind.as_str()
            ),
            Self::ObservationWrongLifecycleKind {
                observation_id,
                kind,
                lifecycle_event_kind,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} has lifecycle kind {} for observation kind {}",
                lifecycle_event_kind.as_str(),
                kind.as_str()
            ),
            Self::ObservationWrongLifecycleStatus {
                observation_id,
                kind,
                lifecycle_event_status,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} has lifecycle status {} for observation kind {}",
                lifecycle_event_status.as_str(),
                kind.as_str()
            ),
            Self::ObservationWrongAnchorState {
                observation_id,
                kind,
                anchor_state,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} has anchor state {} for kind {}",
                anchor_state.as_str(),
                kind.as_str()
            ),
            Self::MissingObservationEvidence {
                observation_id,
                field,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} is missing {field}"
            ),
            Self::UnexpectedObservationRefusalReason { observation_id } => write!(
                f,
                "on-chain lifecycle observation {observation_id} has unexpected refusal reason"
            ),
            Self::ObservationDigestMismatch { observation_id } => write!(
                f,
                "on-chain lifecycle observation {observation_id} digest mismatch"
            ),
            Self::ObservationUnknownLifecycleEvent {
                observation_id,
                lifecycle_event_id,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} references unknown lifecycle event {lifecycle_event_id}"
            ),
            Self::ObservationLifecycleEventMismatch {
                observation_id,
                lifecycle_event_id,
            } => write!(
                f,
                "on-chain lifecycle observation {observation_id} does not match lifecycle event {lifecycle_event_id}"
            ),
            Self::ObservationSummaryMismatch { field } => write!(
                f,
                "on-chain lifecycle observation summary field {field} does not match observations"
            ),
            Self::MissingProofHistory { event_id } => {
                write!(
                    f,
                    "on-chain lifecycle event {event_id} is missing proof history"
                )
            }
            Self::MissingProofHandoff { event_id } => {
                write!(
                    f,
                    "on-chain lifecycle event {event_id} is missing proof handoff"
                )
            }
            Self::MissingSweepOutput { event_id } => {
                write!(
                    f,
                    "on-chain lifecycle event {event_id} is missing sweep output"
                )
            }
            Self::MissingRestartEvidence { event_id } => {
                write!(
                    f,
                    "on-chain lifecycle event {event_id} is missing restart evidence"
                )
            }
            Self::MissingRefusalReason { event_id } => {
                write!(
                    f,
                    "on-chain lifecycle event {event_id} is missing refusal reason"
                )
            }
            Self::UnexpectedRefusalReason { event_id } => write!(
                f,
                "on-chain lifecycle event {event_id} has unexpected refusal reason"
            ),
            Self::DigestMismatch { event_id } => {
                write!(f, "on-chain lifecycle event {event_id} digest mismatch")
            }
            Self::SummaryMismatch { field } => {
                write!(
                    f,
                    "on-chain lifecycle summary field {field} does not match events"
                )
            }
        }
    }
}

impl Error for OnchainLifecycleError {}

impl From<NativeAssetCloseError> for OnchainLifecycleError {
    fn from(err: NativeAssetCloseError) -> Self {
        Self::NativeClose(err)
    }
}

impl From<NativeAssetRecoveryError> for OnchainLifecycleError {
    fn from(err: NativeAssetRecoveryError) -> Self {
        Self::NativeRecovery(err)
    }
}

fn cooperative_close_event(
    kind: OnchainLifecycleEventKind,
    report: &NativeAssetCloseSmokeReport,
    amount: u64,
    proof_history_output_id: String,
    proof_handoff_digest: Bytes32,
) -> OnchainLifecycleEvent {
    OnchainLifecycleEvent::new(kind, report.channel_id.clone(), report.asset_id, amount)
        .with_proof_history(proof_history_output_id)
        .with_proof_handoff(proof_handoff_digest)
        .with_wallet_evidence(proof_handoff_digest)
        .with_monitor_evidence(report.ldk_close_allocation_digest)
}

fn recovery_event(
    kind: OnchainLifecycleEventKind,
    report: &NativeAssetProofRecoveryReport,
) -> OnchainLifecycleEvent {
    OnchainLifecycleEvent::new(
        kind,
        report.channel_id.clone(),
        report.asset_id,
        report.proof_root_sum,
    )
    .with_proof_history(report.proof_history_output_id.clone())
    .with_proof_handoff(report.proof_handoff_digest)
    .with_monitor_evidence(report.ldk_proof_ownership_digest)
    .with_sweep_output(report.sweep_output_digest)
}

fn refusal_event(
    kind: OnchainLifecycleEventKind,
    channel_id: &str,
    asset_id: Bytes32,
    amount: u64,
    reason: &str,
) -> OnchainLifecycleEvent {
    OnchainLifecycleEvent::new(kind, channel_id.to_owned(), asset_id, amount)
        .with_refusal_reason(reason.to_owned())
}

fn restart_event(
    close: &NativeAssetCloseSmokeReport,
    recovery: &NativeAssetRecoveryMatrixReport,
) -> OnchainLifecycleEvent {
    let wallet_evidence = lifecycle_evidence_digest(
        "restart-wallet",
        &[
            close.local_proof_digest.to_hex(),
            close.remote_proof_digest.to_hex(),
            close.local_wallet_balance.to_string(),
            close.remote_wallet_balance.to_string(),
        ],
    );
    let monitor_evidence = lifecycle_evidence_digest(
        "restart-monitor",
        &[
            close.ldk_close_allocation_digest.to_hex(),
            recovery
                .force_close_recovery
                .ldk_proof_ownership_digest
                .to_hex(),
            recovery
                .force_close_recovery
                .proof_history_output_id
                .clone(),
        ],
    );

    OnchainLifecycleEvent::new(
        OnchainLifecycleEventKind::RestartRecovery,
        close.channel_id.clone(),
        close.asset_id,
        close.total_amount,
    )
    .with_proof_history(close.local_export_proof_history_output_id.clone())
    .with_wallet_evidence(wallet_evidence)
    .with_monitor_evidence(monitor_evidence)
}

fn require_lifecycle_evidence(
    field: &'static str,
    present: bool,
) -> Result<(), OnchainLifecycleError> {
    if present {
        Ok(())
    } else {
        Err(OnchainLifecycleError::MissingLifecycleEvidence(field))
    }
}

fn lifecycle_evidence_digest(tag: &str, values: &[String]) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:onchain-lifecycle-evidence:v1");
    hash_string(&mut hasher, tag);
    for value in values {
        hash_string(&mut hasher, value);
    }
    Bytes32(hasher.finalize().into())
}

fn bounded_chain_observation_for_event(
    event: &OnchainLifecycleEvent,
) -> OnchainLifecycleObservation {
    let kind = observation_kind_for_event(event.kind);
    let anchor_state = match kind {
        OnchainLifecycleObservationKind::FailedSweep
        | OnchainLifecycleObservationKind::MissingProofOwnershipRefusal => {
            ProofAnchorState::Unknown
        }
        OnchainLifecycleObservationKind::StaleProofOwnershipAnchor => ProofAnchorState::Stale,
        _ => ProofAnchorState::Confirmed,
    };
    let observation =
        OnchainLifecycleObservation::new(kind, kind.required_source(), event, anchor_state)
            .with_height(144)
            .with_wallet_evidence(event.wallet_evidence_digest.unwrap_or_else(|| {
                lifecycle_evidence_digest(
                    "wallet-observation",
                    std::slice::from_ref(&event.event_id),
                )
            }))
            .with_monitor_evidence(event.monitor_evidence_digest.unwrap_or_else(|| {
                lifecycle_evidence_digest(
                    "monitor-observation",
                    std::slice::from_ref(&event.event_id),
                )
            }));
    let observation = if kind.requires_anchor_outpoint() {
        observation.with_anchor_outpoint(format!("{}:0", event.event_id))
    } else {
        observation
    };
    let observation = if kind.requires_sweep_output() {
        observation.with_sweep_output(event.sweep_output_digest.unwrap_or_else(|| {
            lifecycle_evidence_digest("sweep-observation", std::slice::from_ref(&event.event_id))
        }))
    } else {
        observation
    };
    if event.status == OnchainLifecycleEventStatus::Refused {
        observation.with_refusal_reason(
            event
                .refusal_reason
                .clone()
                .unwrap_or_else(|| format!("{} refused", event.kind.as_str())),
        )
    } else {
        observation
    }
}

fn observation_kind_for_event(kind: OnchainLifecycleEventKind) -> OnchainLifecycleObservationKind {
    match kind {
        OnchainLifecycleEventKind::CooperativeCloseLocal
        | OnchainLifecycleEventKind::CooperativeCloseRemote => {
            OnchainLifecycleObservationKind::CooperativeCloseAnchor
        }
        OnchainLifecycleEventKind::UnilateralCommitment => {
            OnchainLifecycleObservationKind::UnilateralCommitmentAnchor
        }
        OnchainLifecycleEventKind::SecondLevelHtlcSuccess
        | OnchainLifecycleEventKind::SecondLevelHtlcTimeout => {
            OnchainLifecycleObservationKind::SecondLevelHtlcAnchor
        }
        OnchainLifecycleEventKind::FinalSweep => OnchainLifecycleObservationKind::FinalSweepAnchor,
        OnchainLifecycleEventKind::FailedSweep => OnchainLifecycleObservationKind::FailedSweep,
        OnchainLifecycleEventKind::BtcOnlySweepRefusal => {
            OnchainLifecycleObservationKind::BtcOnlySweepRefusal
        }
        OnchainLifecycleEventKind::StaleProofOwnershipRefusal => {
            OnchainLifecycleObservationKind::StaleProofOwnershipAnchor
        }
        OnchainLifecycleEventKind::MissingProofOwnershipRefusal => {
            OnchainLifecycleObservationKind::MissingProofOwnershipRefusal
        }
        OnchainLifecycleEventKind::RestartRecovery => {
            OnchainLifecycleObservationKind::RestartEvidence
        }
    }
}

fn all_lifecycle_events_observed(
    events: &[OnchainLifecycleEvent],
    observations: &[OnchainLifecycleObservation],
) -> bool {
    events.iter().all(|event| {
        observations
            .iter()
            .any(|observation| observation.lifecycle_event_id == event.event_id)
    })
}

fn confirmed_recovery_observed(
    events: &[OnchainLifecycleEvent],
    observations: &[OnchainLifecycleObservation],
) -> bool {
    events
        .iter()
        .filter(|event| event.status != OnchainLifecycleEventStatus::Refused)
        .all(|event| {
            observations.iter().any(|observation| {
                observation.lifecycle_event_id == event.event_id
                    && observation.anchor_state == ProofAnchorState::Confirmed
            })
        })
}

fn refusal_observations_present(
    events: &[OnchainLifecycleEvent],
    observations: &[OnchainLifecycleObservation],
) -> bool {
    events
        .iter()
        .filter(|event| event.status == OnchainLifecycleEventStatus::Refused)
        .all(|event| {
            observations.iter().any(|observation| {
                observation.lifecycle_event_id == event.event_id
                    && observation.lifecycle_event_status == OnchainLifecycleEventStatus::Refused
                    && !empty_optional(observation.refusal_reason.as_deref())
            })
        })
}

fn restart_observation_present(observations: &[OnchainLifecycleObservation]) -> bool {
    observations.iter().any(|observation| {
        observation.kind == OnchainLifecycleObservationKind::RestartEvidence
            && observation.anchor_state == ProofAnchorState::Confirmed
            && observation.wallet_evidence_digest.is_some()
            && observation.monitor_evidence_digest.is_some()
    })
}

fn has_proof_export(events: &[OnchainLifecycleEvent], kind: OnchainLifecycleEventKind) -> bool {
    has_status(events, kind, OnchainLifecycleEventStatus::ProofExported)
}

fn has_recovered(events: &[OnchainLifecycleEvent], kind: OnchainLifecycleEventKind) -> bool {
    has_status(
        events,
        kind,
        OnchainLifecycleEventStatus::AssetProofRecovered,
    )
}

fn has_refusal(events: &[OnchainLifecycleEvent], kind: OnchainLifecycleEventKind) -> bool {
    has_status(events, kind, OnchainLifecycleEventStatus::Refused)
}

fn has_status(
    events: &[OnchainLifecycleEvent],
    kind: OnchainLifecycleEventKind,
    status: OnchainLifecycleEventStatus,
) -> bool {
    events
        .iter()
        .any(|event| event.kind == kind && event.status == status)
}

fn event_id_for(
    kind: OnchainLifecycleEventKind,
    channel_id: &str,
    asset_id: Bytes32,
    amount: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:onchain-lifecycle-event-id:v1");
    hash_string(&mut hasher, kind.as_str());
    hash_string(&mut hasher, channel_id);
    hasher.update(asset_id.0);
    hasher.update(amount.to_be_bytes());
    let digest = Bytes32(hasher.finalize().into());
    format!("{}:{}", kind.as_str(), digest.to_hex())
}

fn observation_id_for(
    kind: OnchainLifecycleObservationKind,
    source: OnchainLifecycleObservationSource,
    lifecycle_event: &OnchainLifecycleEvent,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:onchain-lifecycle-observation-id:v1");
    hash_string(&mut hasher, kind.as_str());
    hash_string(&mut hasher, source.as_str());
    hash_string(&mut hasher, &lifecycle_event.event_id);
    hash_string(&mut hasher, lifecycle_event.kind.as_str());
    hash_string(&mut hasher, &lifecycle_event.channel_id);
    hasher.update(lifecycle_event.asset_id.0);
    hasher.update(lifecycle_event.amount.to_be_bytes());
    let digest = Bytes32(hasher.finalize().into());
    format!("{}:{}", kind.as_str(), digest.to_hex())
}

fn require_nonempty(field: &'static str, value: &str) -> Result<(), OnchainLifecycleError> {
    if value.is_empty() {
        Err(OnchainLifecycleError::MissingField(field))
    } else {
        Ok(())
    }
}

fn empty_optional(value: Option<&str>) -> bool {
    value.map(str::is_empty).unwrap_or(true)
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn hash_optional_string(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_string(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_u32(hasher: &mut Sha256, value: Option<u32>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_bytes32(hasher: &mut Sha256, value: Option<Bytes32>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.0);
        }
        None => hasher.update([0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_report_validates_required_events() {
        let report = valid_report();
        report.validate().expect("report validates");

        assert!(report.cooperative_close_exported);
        assert!(report.unilateral_recovery_explained);
        assert!(report.second_level_success_explained);
        assert!(report.second_level_timeout_explained);
        assert!(report.final_sweep_explained);
        assert!(report.failed_sweep_refused);
        assert!(report.btc_only_sweep_refused);
        assert!(report.restart_recovery_explained);
        assert!(!report.live_chain_watcher_backed);
        assert!(!report.production_ready);
    }

    #[test]
    fn lifecycle_events_fail_closed_on_wrong_statuses_and_missing_evidence() {
        let mut recovered_failed_sweep = event(OnchainLifecycleEventKind::FailedSweep);
        recovered_failed_sweep.status = OnchainLifecycleEventStatus::AssetProofRecovered;
        recovered_failed_sweep.event_digest = recovered_failed_sweep.digest();
        assert!(matches!(
            recovered_failed_sweep.validate(),
            Err(OnchainLifecycleError::WrongStatus { .. })
        ));

        let mut recovered_btc_only = event(OnchainLifecycleEventKind::BtcOnlySweepRefusal);
        recovered_btc_only.status = OnchainLifecycleEventStatus::AssetProofRecovered;
        recovered_btc_only.event_digest = recovered_btc_only.digest();
        assert!(matches!(
            recovered_btc_only.validate(),
            Err(OnchainLifecycleError::WrongStatus { .. })
        ));

        let mut missing_proof_history = event(OnchainLifecycleEventKind::FinalSweep);
        missing_proof_history.proof_history_output_id = None;
        missing_proof_history.event_digest = missing_proof_history.digest();
        assert!(matches!(
            missing_proof_history.validate(),
            Err(OnchainLifecycleError::MissingProofHistory { .. })
        ));

        let mut missing_restart = event(OnchainLifecycleEventKind::RestartRecovery);
        missing_restart.wallet_evidence_digest = None;
        missing_restart.event_digest = missing_restart.digest();
        assert!(matches!(
            missing_restart.validate(),
            Err(OnchainLifecycleError::MissingRestartEvidence { .. })
        ));
    }

    #[test]
    fn lifecycle_report_rejects_duplicate_summary_and_digest_mismatch() {
        let duplicate = event(OnchainLifecycleEventKind::CooperativeCloseLocal);
        let duplicate_id = duplicate.event_id.clone();
        let mut report = valid_report();
        report.events.push(duplicate);
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::DuplicateEvent { event_id }) if event_id == duplicate_id
        ));

        let mut report = valid_report();
        report.final_sweep_explained = false;
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::SummaryMismatch {
                field: "final_sweep_explained"
            })
        ));

        let mut report = valid_report();
        report.events[0].event_digest = Bytes32([77; 32]);
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::DigestMismatch { .. })
        ));

        let mut report = valid_report();
        report.production_ready = true;
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::UnsupportedProductionClaim)
        ));
    }

    #[test]
    fn onchain_lifecycle_smoke_covers_close_recovery_and_refusals() {
        let report = run_onchain_lifecycle_smoke().expect("lifecycle smoke passes");
        report.validate().expect("lifecycle report validates");

        assert!(report.cooperative_close_exported);
        assert!(report.unilateral_recovery_explained);
        assert!(report.second_level_success_explained);
        assert!(report.second_level_timeout_explained);
        assert!(report.final_sweep_explained);
        assert!(report.failed_sweep_refused);
        assert!(report.btc_only_sweep_refused);
        assert!(report.restart_recovery_explained);
        assert!(!report.live_chain_watcher_backed);
        assert!(!report.production_ready);

        let kinds = report
            .events
            .iter()
            .map(|event| event.kind)
            .collect::<BTreeSet<_>>();
        for kind in [
            OnchainLifecycleEventKind::CooperativeCloseLocal,
            OnchainLifecycleEventKind::CooperativeCloseRemote,
            OnchainLifecycleEventKind::UnilateralCommitment,
            OnchainLifecycleEventKind::SecondLevelHtlcSuccess,
            OnchainLifecycleEventKind::SecondLevelHtlcTimeout,
            OnchainLifecycleEventKind::FinalSweep,
            OnchainLifecycleEventKind::FailedSweep,
            OnchainLifecycleEventKind::BtcOnlySweepRefusal,
            OnchainLifecycleEventKind::StaleProofOwnershipRefusal,
            OnchainLifecycleEventKind::MissingProofOwnershipRefusal,
            OnchainLifecycleEventKind::RestartRecovery,
        ] {
            assert!(kinds.contains(&kind), "missing {}", kind.as_str());
        }
    }

    #[test]
    fn chain_observations_validate_required_lifecycle_classes() {
        let observations = valid_observations();

        for observation in &observations {
            observation.validate().expect("observation validates");
        }

        let observed_kinds = observations
            .iter()
            .map(|observation| observation.kind)
            .collect::<BTreeSet<_>>();
        for kind in [
            OnchainLifecycleObservationKind::CooperativeCloseAnchor,
            OnchainLifecycleObservationKind::UnilateralCommitmentAnchor,
            OnchainLifecycleObservationKind::SecondLevelHtlcAnchor,
            OnchainLifecycleObservationKind::FinalSweepAnchor,
            OnchainLifecycleObservationKind::FailedSweep,
            OnchainLifecycleObservationKind::BtcOnlySweepRefusal,
            OnchainLifecycleObservationKind::StaleProofOwnershipAnchor,
            OnchainLifecycleObservationKind::MissingProofOwnershipRefusal,
            OnchainLifecycleObservationKind::RestartEvidence,
        ] {
            assert!(observed_kinds.contains(&kind), "missing {}", kind.as_str());
        }
    }

    #[test]
    fn chain_observations_fail_closed_on_bad_claims_and_missing_evidence() {
        let mut recovered_failed_sweep = event(OnchainLifecycleEventKind::FailedSweep);
        recovered_failed_sweep.status = OnchainLifecycleEventStatus::AssetProofRecovered;
        recovered_failed_sweep.event_digest = recovered_failed_sweep.digest();
        let failed_sweep_observation = observation(
            &recovered_failed_sweep,
            OnchainLifecycleObservationKind::FailedSweep,
        );
        assert!(matches!(
            failed_sweep_observation.validate(),
            Err(OnchainLifecycleError::ObservationWrongLifecycleStatus { .. })
        ));

        let mut recovered_btc_only = event(OnchainLifecycleEventKind::BtcOnlySweepRefusal);
        recovered_btc_only.status = OnchainLifecycleEventStatus::AssetProofRecovered;
        recovered_btc_only.event_digest = recovered_btc_only.digest();
        let btc_only_observation = observation(
            &recovered_btc_only,
            OnchainLifecycleObservationKind::BtcOnlySweepRefusal,
        );
        assert!(matches!(
            btc_only_observation.validate(),
            Err(OnchainLifecycleError::ObservationWrongLifecycleStatus { .. })
        ));

        let mut reorged_recovery = observation(
            &event(OnchainLifecycleEventKind::UnilateralCommitment),
            OnchainLifecycleObservationKind::UnilateralCommitmentAnchor,
        );
        reorged_recovery.anchor_state = ProofAnchorState::Reorged;
        reorged_recovery.observation_digest = reorged_recovery.digest();
        assert!(matches!(
            reorged_recovery.validate(),
            Err(OnchainLifecycleError::ObservationWrongAnchorState { .. })
        ));

        let mut missing_event_id = observation(
            &event(OnchainLifecycleEventKind::CooperativeCloseLocal),
            OnchainLifecycleObservationKind::CooperativeCloseAnchor,
        );
        missing_event_id.lifecycle_event_id.clear();
        missing_event_id.observation_digest = missing_event_id.digest();
        assert!(matches!(
            missing_event_id.validate(),
            Err(OnchainLifecycleError::MissingField("lifecycle_event_id"))
        ));

        let mut missing_anchor = observation(
            &event(OnchainLifecycleEventKind::FinalSweep),
            OnchainLifecycleObservationKind::FinalSweepAnchor,
        );
        missing_anchor.anchor_outpoint = None;
        missing_anchor.observation_digest = missing_anchor.digest();
        assert!(matches!(
            missing_anchor.validate(),
            Err(OnchainLifecycleError::MissingObservationEvidence {
                field: "anchor_outpoint",
                ..
            })
        ));

        let mut missing_restart = observation(
            &event(OnchainLifecycleEventKind::RestartRecovery),
            OnchainLifecycleObservationKind::RestartEvidence,
        );
        missing_restart.wallet_evidence_digest = None;
        missing_restart.observation_digest = missing_restart.digest();
        assert!(matches!(
            missing_restart.validate(),
            Err(OnchainLifecycleError::MissingObservationEvidence {
                field: "restart_wallet_and_monitor_evidence",
                ..
            })
        ));

        let mut tampered = observation(
            &event(OnchainLifecycleEventKind::CooperativeCloseRemote),
            OnchainLifecycleObservationKind::CooperativeCloseAnchor,
        );
        tampered.observation_digest = Bytes32([99; 32]);
        assert!(matches!(
            tampered.validate(),
            Err(OnchainLifecycleError::ObservationDigestMismatch { .. })
        ));
    }

    #[test]
    fn chain_observation_smoke_validates_and_covers_lifecycle_events() {
        let report = run_chain_watcher_lifecycle_smoke().expect("chain observation smoke passes");
        report
            .validate()
            .expect("chain observation report validates");

        assert!(report.all_lifecycle_events_observed);
        assert!(report.confirmed_recovery_observed);
        assert!(report.refusal_observations_present);
        assert!(report.restart_observation_present);
        assert!(!report.live_chain_watcher_backed);
        assert!(!report.production_ready);
        assert_eq!(
            report.lifecycle_report.events.len(),
            report.observations.len()
        );
    }

    #[test]
    fn chain_observation_report_rejects_unknown_events_duplicates_and_false_claims() {
        let lifecycle_report = valid_report();
        let observations = valid_observations();
        let mut report =
            OnchainLifecycleChainObservationReport::new(lifecycle_report, observations)
                .expect("report builds");

        report.live_chain_watcher_backed = true;
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::UnsupportedLiveChainWatcherClaim)
        ));

        let mut report =
            OnchainLifecycleChainObservationReport::new(valid_report(), valid_observations())
                .expect("report builds");
        report.observations[0].lifecycle_event_id = "unknown-event".to_owned();
        report.observations[0].observation_digest = report.observations[0].digest();
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::ObservationUnknownLifecycleEvent { .. })
        ));

        let mut report =
            OnchainLifecycleChainObservationReport::new(valid_report(), valid_observations())
                .expect("report builds");
        report.observations.push(report.observations[0].clone());
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::DuplicateObservation { .. })
        ));

        let lifecycle_report = valid_report();
        let observations = valid_observations();
        let mut report =
            OnchainLifecycleChainObservationReport::new(lifecycle_report, observations)
                .expect("report builds");
        report.all_lifecycle_events_observed = false;
        assert!(matches!(
            report.validate(),
            Err(OnchainLifecycleError::ObservationSummaryMismatch {
                field: "all_lifecycle_events_observed"
            })
        ));
    }

    fn valid_report() -> OnchainLifecycleReport {
        OnchainLifecycleReport::new(vec![
            event(OnchainLifecycleEventKind::CooperativeCloseLocal),
            event(OnchainLifecycleEventKind::CooperativeCloseRemote),
            event(OnchainLifecycleEventKind::UnilateralCommitment),
            event(OnchainLifecycleEventKind::SecondLevelHtlcSuccess),
            event(OnchainLifecycleEventKind::SecondLevelHtlcTimeout),
            event(OnchainLifecycleEventKind::FinalSweep),
            event(OnchainLifecycleEventKind::FailedSweep),
            event(OnchainLifecycleEventKind::BtcOnlySweepRefusal),
            event(OnchainLifecycleEventKind::StaleProofOwnershipRefusal),
            event(OnchainLifecycleEventKind::MissingProofOwnershipRefusal),
            event(OnchainLifecycleEventKind::RestartRecovery),
        ])
        .expect("valid report")
    }

    fn valid_observations() -> Vec<OnchainLifecycleObservation> {
        valid_report()
            .events
            .iter()
            .map(|event| {
                let kind = match event.kind {
                    OnchainLifecycleEventKind::CooperativeCloseLocal
                    | OnchainLifecycleEventKind::CooperativeCloseRemote => {
                        OnchainLifecycleObservationKind::CooperativeCloseAnchor
                    }
                    OnchainLifecycleEventKind::UnilateralCommitment => {
                        OnchainLifecycleObservationKind::UnilateralCommitmentAnchor
                    }
                    OnchainLifecycleEventKind::SecondLevelHtlcSuccess
                    | OnchainLifecycleEventKind::SecondLevelHtlcTimeout => {
                        OnchainLifecycleObservationKind::SecondLevelHtlcAnchor
                    }
                    OnchainLifecycleEventKind::FinalSweep => {
                        OnchainLifecycleObservationKind::FinalSweepAnchor
                    }
                    OnchainLifecycleEventKind::FailedSweep => {
                        OnchainLifecycleObservationKind::FailedSweep
                    }
                    OnchainLifecycleEventKind::BtcOnlySweepRefusal => {
                        OnchainLifecycleObservationKind::BtcOnlySweepRefusal
                    }
                    OnchainLifecycleEventKind::StaleProofOwnershipRefusal => {
                        OnchainLifecycleObservationKind::StaleProofOwnershipAnchor
                    }
                    OnchainLifecycleEventKind::MissingProofOwnershipRefusal => {
                        OnchainLifecycleObservationKind::MissingProofOwnershipRefusal
                    }
                    OnchainLifecycleEventKind::RestartRecovery => {
                        OnchainLifecycleObservationKind::RestartEvidence
                    }
                };
                observation(event, kind)
            })
            .collect()
    }

    fn observation(
        event: &OnchainLifecycleEvent,
        kind: OnchainLifecycleObservationKind,
    ) -> OnchainLifecycleObservation {
        let anchor_state = match kind {
            OnchainLifecycleObservationKind::FailedSweep => ProofAnchorState::Unknown,
            OnchainLifecycleObservationKind::StaleProofOwnershipAnchor => ProofAnchorState::Stale,
            OnchainLifecycleObservationKind::MissingProofOwnershipRefusal => {
                ProofAnchorState::Unknown
            }
            _ => ProofAnchorState::Confirmed,
        };
        let source = kind.required_source();
        let observation = OnchainLifecycleObservation::new(kind, source, event, anchor_state)
            .with_height(144)
            .with_wallet_evidence(Bytes32([21; 32]))
            .with_monitor_evidence(Bytes32([22; 32]));
        let observation = if kind.requires_anchor_outpoint() {
            observation.with_anchor_outpoint(format!("anchor:{}:0", event.event_id))
        } else {
            observation
        };
        let observation = if kind.requires_sweep_output() {
            observation.with_sweep_output(Bytes32([23; 32]))
        } else {
            observation
        };
        if event.status == OnchainLifecycleEventStatus::Refused {
            observation.with_refusal_reason(format!("{} refused", kind.as_str()))
        } else {
            observation
        }
    }

    fn event(kind: OnchainLifecycleEventKind) -> OnchainLifecycleEvent {
        let asset_id = Bytes32([1; 32]);
        let event = OnchainLifecycleEvent::new(kind, "channel-1".to_owned(), asset_id, 1_000);
        match kind {
            OnchainLifecycleEventKind::CooperativeCloseLocal
            | OnchainLifecycleEventKind::CooperativeCloseRemote => event
                .with_proof_history(format!("proof-history:{}", kind.as_str()))
                .with_proof_handoff(Bytes32([2; 32]))
                .with_wallet_evidence(Bytes32([3; 32])),
            OnchainLifecycleEventKind::UnilateralCommitment
            | OnchainLifecycleEventKind::SecondLevelHtlcSuccess
            | OnchainLifecycleEventKind::SecondLevelHtlcTimeout
            | OnchainLifecycleEventKind::FinalSweep => event
                .with_proof_history(format!("proof-history:{}", kind.as_str()))
                .with_proof_handoff(Bytes32([4; 32]))
                .with_monitor_evidence(Bytes32([5; 32]))
                .with_sweep_output(Bytes32([6; 32])),
            OnchainLifecycleEventKind::FailedSweep
            | OnchainLifecycleEventKind::BtcOnlySweepRefusal
            | OnchainLifecycleEventKind::StaleProofOwnershipRefusal
            | OnchainLifecycleEventKind::MissingProofOwnershipRefusal => {
                event.with_refusal_reason(format!("{} refused", kind.as_str()))
            }
            OnchainLifecycleEventKind::RestartRecovery => event
                .with_proof_history("proof-history:restart".to_owned())
                .with_wallet_evidence(Bytes32([7; 32]))
                .with_monitor_evidence(Bytes32([8; 32])),
        }
    }
}
