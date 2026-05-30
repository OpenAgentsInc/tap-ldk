use std::{collections::BTreeSet, error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::asset::Bytes32;

pub const ONCHAIN_LIFECYCLE_REPORT_SCHEMA_VERSION: u32 = 1;

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
        let mut report = Self {
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

    pub fn validate(&mut self) -> Result<(), OnchainLifecycleError> {
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

#[derive(Debug)]
pub enum OnchainLifecycleError {
    UnsupportedVersion(u32),
    UnsupportedProductionClaim,
    EmptyReport,
    MissingField(&'static str),
    DuplicateEvent {
        event_id: String,
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
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported on-chain lifecycle schema version {version}")
            }
            Self::UnsupportedProductionClaim => {
                write!(
                    f,
                    "bounded on-chain lifecycle report cannot claim production readiness"
                )
            }
            Self::EmptyReport => write!(f, "on-chain lifecycle report is empty"),
            Self::MissingField(field) => write!(f, "on-chain lifecycle missing {field}"),
            Self::DuplicateEvent { event_id } => {
                write!(f, "duplicate on-chain lifecycle event {event_id}")
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
        let mut report = valid_report();
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
