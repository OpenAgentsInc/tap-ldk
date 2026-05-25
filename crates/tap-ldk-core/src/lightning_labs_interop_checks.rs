use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    asset::Bytes32,
    lightning_labs_funding::{
        FundingInteropGap, LightningLabsFundingInteropError, LightningLabsFundingInteropReport,
        run_lightning_labs_funding_interop_fixture_smoke,
    },
    lightning_labs_payment::{
        LightningLabsIncomingPaymentReport, LightningLabsOutgoingPaymentError,
        LightningLabsOutgoingPaymentReport, run_lightning_labs_incoming_payment_smoke,
        run_lightning_labs_outgoing_payment_smoke,
    },
    lightning_labs_rfq::LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT,
    tapd_proof::{TapdProofError, TapdProofFixtureReport, decode_fixture_hex},
};

pub const LIGHTNING_LABS_INTEROP_CHECK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsInteropCheckStatus {
    Passed,
    DocumentedGap,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsInteropMismatch {
    pub side: String,
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub artifact_path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsInteropCheck {
    pub name: String,
    pub status: LightningLabsInteropCheckStatus,
    pub side: String,
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub artifact_path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsInteropCheckReport {
    pub version: u32,
    pub source_commit: String,
    pub funding_interop_id: String,
    pub asset_id: Bytes32,
    pub funding_total_amount: u64,
    pub funding_proof_digest: Bytes32,
    pub proof_file_digest: Bytes32,
    pub outgoing_payment_id: String,
    pub incoming_payment_id: String,
    pub all_automated_checks_passed: bool,
    pub live_daemon_gaps_remaining: bool,
    pub checks: Vec<LightningLabsInteropCheck>,
    pub mismatches: Vec<LightningLabsInteropMismatch>,
    pub documented_gaps: Vec<FundingInteropGap>,
}

impl LightningLabsInteropCheckReport {
    fn validate(&self) -> Result<(), LightningLabsInteropCheckError> {
        if self.version != LIGHTNING_LABS_INTEROP_CHECK_SCHEMA_VERSION {
            return Err(LightningLabsInteropCheckError::StorageInvariant(format!(
                "unsupported interop check schema version {}",
                self.version
            )));
        }
        if self.source_commit.trim().is_empty()
            || self.funding_interop_id.trim().is_empty()
            || self.outgoing_payment_id.trim().is_empty()
            || self.incoming_payment_id.trim().is_empty()
        {
            return Err(LightningLabsInteropCheckError::StorageInvariant(
                "interop check identity fields cannot be empty".to_owned(),
            ));
        }
        let failed_count = self
            .checks
            .iter()
            .filter(|check| check.status == LightningLabsInteropCheckStatus::Failed)
            .count();
        if self.all_automated_checks_passed != (failed_count == 0 && self.mismatches.is_empty()) {
            return Err(LightningLabsInteropCheckError::StorageInvariant(
                "automated check pass flag does not match failed diagnostics".to_owned(),
            ));
        }
        if self.live_daemon_gaps_remaining
            != self
                .checks
                .iter()
                .any(|check| check.status == LightningLabsInteropCheckStatus::DocumentedGap)
        {
            return Err(LightningLabsInteropCheckError::StorageInvariant(
                "live daemon gap flag does not match documented gap checks".to_owned(),
            ));
        }
        for mismatch in &self.mismatches {
            for (field, value) in [
                ("side", mismatch.side.as_str()),
                ("field", mismatch.field.as_str()),
                ("expected", mismatch.expected.as_str()),
                ("actual", mismatch.actual.as_str()),
                ("artifact_path", mismatch.artifact_path.as_str()),
                ("detail", mismatch.detail.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(LightningLabsInteropCheckError::StorageInvariant(format!(
                        "mismatch {field} cannot be empty"
                    )));
                }
            }
        }
        Ok(())
    }
}

pub fn run_lightning_labs_interop_check_smoke(
    funding_hexdump: &str,
    commitment_hexdump: &str,
    proof_file_hex: &str,
    single_proof_hex: &str,
    tapchannel_fixture_dir: &str,
    proof_fixture_dir: &str,
    report_path: &str,
) -> Result<LightningLabsInteropCheckReport, LightningLabsInteropCheckError> {
    let (funding_store, funding) =
        run_lightning_labs_funding_interop_fixture_smoke(funding_hexdump, commitment_hexdump)?;
    let (outgoing_store, outgoing) =
        run_lightning_labs_outgoing_payment_smoke(funding_hexdump, commitment_hexdump)?;
    let (incoming_store, incoming) =
        run_lightning_labs_incoming_payment_smoke(funding_hexdump, commitment_hexdump)?;
    let proof = decode_fixture_hex(proof_file_hex, single_proof_hex)?;

    let mut checks = Vec::new();
    let mut mismatches = Vec::new();
    let tapchannel_artifact = |file: &str| format!("{tapchannel_fixture_dir}/{file}");
    let report_artifact = |fragment: &str| format!("{report_path}#{fragment}");

    record_comparison(
        &mut checks,
        &mut mismatches,
        ComparisonInput {
            name: "funding balances conserve asset amount".to_owned(),
            side: "both",
            field: "local_balance + remote_balance",
            expected: funding.funding_total_amount.to_string(),
            actual: funding
                .local_balance
                .checked_add(funding.remote_balance)
                .ok_or(LightningLabsInteropCheckError::AmountOverflow)?
                .to_string(),
            artifact_path: tapchannel_artifact("commitment-blob.hexdump"),
            detail: "decoded Lightning Labs funding and commitment fixture balances agree",
        },
    );
    record_comparison(
        &mut checks,
        &mut mismatches,
        ComparisonInput {
            name: "funding store survives restart".to_owned(),
            side: "tap-ldk",
            field: "funding_store_roundtrip",
            expected: "true".to_owned(),
            actual: store_roundtrip_matches(&funding_store)?.to_string(),
            artifact_path: report_artifact("funding_store"),
            detail: "fixture-backed funding interop state serializes and reloads unchanged",
        },
    );
    record_comparison(
        &mut checks,
        &mut mismatches,
        ComparisonInput {
            name: "funding proof material is present".to_owned(),
            side: "lightning_labs",
            field: "funding_asset_proof_digest",
            expected: "nonzero".to_owned(),
            actual: nonzero_digest_label(funding.funding_asset_proof_digest),
            artifact_path: tapchannel_artifact("funding-blob.hexdump"),
            detail: "funding fixture exposes proof material digest needed for follow-up proof binding",
        },
    );
    record_proof_checks(&mut checks, &mut mismatches, &proof, proof_fixture_dir);
    record_payment_checks(
        &mut checks,
        &mut mismatches,
        &funding,
        &outgoing,
        store_roundtrip_matches(&outgoing_store)?,
        &report_artifact("outgoing_payment"),
        "outgoing",
    )?;
    record_payment_checks(
        &mut checks,
        &mut mismatches,
        &funding,
        &incoming,
        store_roundtrip_matches(&incoming_store)?,
        &report_artifact("incoming_payment"),
        "incoming",
    )?;

    checks.push(LightningLabsInteropCheck {
        name: "outgoing live receiver balance remains documented gap".to_owned(),
        status: LightningLabsInteropCheckStatus::DocumentedGap,
        side: "lightning_labs".to_owned(),
        field: "observed_receiver_balance_after".to_owned(),
        expected: outgoing
            .expected_lightning_labs_receiver_balance_after
            .to_string(),
        actual: "unobserved".to_owned(),
        artifact_path: report_artifact("outgoing_payment.documented_gap"),
        detail: outgoing.documented_gap.reason.clone(),
    });
    checks.push(LightningLabsInteropCheck {
        name: "incoming live receiver balance remains documented gap".to_owned(),
        status: LightningLabsInteropCheckStatus::DocumentedGap,
        side: "tap-ldk".to_owned(),
        field: "observed_receiver_balance_after".to_owned(),
        expected: incoming.expected_tap_ldk_receiver_balance_after.to_string(),
        actual: "unobserved".to_owned(),
        artifact_path: report_artifact("incoming_payment.documented_gap"),
        detail: incoming.documented_gap.reason.clone(),
    });

    let all_automated_checks_passed = mismatches.is_empty()
        && checks
            .iter()
            .all(|check| check.status != LightningLabsInteropCheckStatus::Failed);
    let live_daemon_gaps_remaining = checks
        .iter()
        .any(|check| check.status == LightningLabsInteropCheckStatus::DocumentedGap);
    let report = LightningLabsInteropCheckReport {
        version: LIGHTNING_LABS_INTEROP_CHECK_SCHEMA_VERSION,
        source_commit: LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT.to_owned(),
        funding_interop_id: funding.interop_id.clone(),
        asset_id: funding.asset_id,
        funding_total_amount: funding.funding_total_amount,
        funding_proof_digest: funding.funding_asset_proof_digest,
        proof_file_digest: proof.proof_file.raw_digest,
        outgoing_payment_id: outgoing.payment_id,
        incoming_payment_id: incoming.payment_id,
        all_automated_checks_passed,
        live_daemon_gaps_remaining,
        checks,
        mismatches,
        documented_gaps: vec![
            funding.documented_gap,
            outgoing.documented_gap,
            incoming.documented_gap,
        ],
    };
    report.validate()?;
    Ok(report)
}

fn record_proof_checks(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    proof: &TapdProofFixtureReport,
    proof_fixture_dir: &str,
) {
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "tapd proof file has proof material".to_owned(),
            side: "lightning_labs",
            field: "proof_count",
            expected: "nonzero".to_owned(),
            actual: if proof.proof_file.proof_count > 0 {
                "nonzero".to_owned()
            } else {
                "zero".to_owned()
            },
            artifact_path: format!("{proof_fixture_dir}/proof-file.hex"),
            detail: "TAPF fixture decodes with chained checksums and at least one proof",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "wrapped single proof remains compatible".to_owned(),
            side: "tap-ldk",
            field: "wrapped_single_proof_count",
            expected: "1".to_owned(),
            actual: proof.wrapped_single_proof_file.proof_count.to_string(),
            artifact_path: format!("{proof_fixture_dir}/proof.hex"),
            detail: "single TAPP proof can be wrapped as a TAPF proof file for Lightning Labs tooling",
        },
    );
}

fn record_payment_checks(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    funding: &LightningLabsFundingInteropReport,
    payment: &impl PaymentCheckView,
    store_restart_matches: bool,
    artifact_path: &str,
    direction: &'static str,
) -> Result<(), LightningLabsInteropCheckError> {
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: format!("{direction} payment asset id matches funding"),
            side: "both",
            field: "asset_id",
            expected: funding.asset_id.to_hex(),
            actual: payment.asset_id().to_hex(),
            artifact_path: artifact_path.to_owned(),
            detail: "payment artifact uses the same asset ID as the fixture-backed funding state",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: format!("{direction} payment balance delta conserves total"),
            side: "both",
            field: "expected_balance_conserved",
            expected: "true".to_owned(),
            actual: payment.expected_balance_conserved().to_string(),
            artifact_path: artifact_path.to_owned(),
            detail: "expected sender and receiver balance deltas preserve total channel asset amount",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: format!("{direction} payment store survives restart"),
            side: "tap-ldk",
            field: "payment_store_roundtrip",
            expected: "true".to_owned(),
            actual: store_restart_matches.to_string(),
            artifact_path: artifact_path.to_owned(),
            detail: "payment interop state serializes and reloads unchanged",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: format!("{direction} payment starts from funding balances"),
            side: "both",
            field: "balance_before",
            expected: payment.expected_balance_before(funding),
            actual: payment.actual_balance_before(),
            artifact_path: artifact_path.to_owned(),
            detail: "payment before-balances line up with the fixture-backed channel allocation",
        },
    );
    if !payment.metadata_rejections_passed() {
        record_comparison(
            checks,
            mismatches,
            ComparisonInput {
                name: format!("{direction} metadata rejection checks"),
                side: "tap-ldk",
                field: "metadata_rejections",
                expected: "true".to_owned(),
                actual: "false".to_owned(),
                artifact_path: artifact_path.to_owned(),
                detail: "wrong, stale, malformed, or replayed metadata must fail closed",
            },
        );
    }
    Ok(())
}

trait PaymentCheckView {
    fn asset_id(&self) -> Bytes32;
    fn expected_balance_conserved(&self) -> bool;
    fn metadata_rejections_passed(&self) -> bool;
    fn expected_balance_before(&self, funding: &LightningLabsFundingInteropReport) -> String;
    fn actual_balance_before(&self) -> String;
}

impl PaymentCheckView for LightningLabsOutgoingPaymentReport {
    fn asset_id(&self) -> Bytes32 {
        self.asset_id
    }

    fn expected_balance_conserved(&self) -> bool {
        self.expected_balance_conserved
    }

    fn metadata_rejections_passed(&self) -> bool {
        self.quote_replay_rejected && self.wrong_asset_rejected
    }

    fn expected_balance_before(&self, funding: &LightningLabsFundingInteropReport) -> String {
        format!(
            "tap-ldk:{};lightning_labs:{}",
            funding.local_balance, funding.remote_balance
        )
    }

    fn actual_balance_before(&self) -> String {
        format!(
            "tap-ldk:{};lightning_labs:{}",
            self.sender_balance_before, self.lightning_labs_receiver_balance_before
        )
    }
}

impl PaymentCheckView for LightningLabsIncomingPaymentReport {
    fn asset_id(&self) -> Bytes32 {
        self.asset_id
    }

    fn expected_balance_conserved(&self) -> bool {
        self.expected_balance_conserved
    }

    fn metadata_rejections_passed(&self) -> bool {
        self.final_hop_validated
            && self.stale_htlc_rejected
            && self.wrong_amount_rejected
            && self.malformed_htlc_rejected
            && self.quote_replay_rejected
    }

    fn expected_balance_before(&self, funding: &LightningLabsFundingInteropReport) -> String {
        format!(
            "tap-ldk:{};lightning_labs:{}",
            funding.local_balance, funding.remote_balance
        )
    }

    fn actual_balance_before(&self) -> String {
        format!(
            "tap-ldk:{};lightning_labs:{}",
            self.tap_ldk_receiver_balance_before, self.lightning_labs_sender_balance_before
        )
    }
}

struct ComparisonInput<'a> {
    name: String,
    side: &'a str,
    field: &'a str,
    expected: String,
    actual: String,
    artifact_path: String,
    detail: &'a str,
}

fn record_comparison(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    input: ComparisonInput<'_>,
) {
    let status = if input.expected == input.actual {
        LightningLabsInteropCheckStatus::Passed
    } else {
        let mismatch = LightningLabsInteropMismatch {
            side: input.side.to_owned(),
            field: input.field.to_owned(),
            expected: input.expected.clone(),
            actual: input.actual.clone(),
            artifact_path: input.artifact_path.clone(),
            detail: input.detail.to_owned(),
        };
        mismatches.push(mismatch);
        LightningLabsInteropCheckStatus::Failed
    };
    checks.push(LightningLabsInteropCheck {
        name: input.name,
        status,
        side: input.side.to_owned(),
        field: input.field.to_owned(),
        expected: input.expected,
        actual: input.actual,
        artifact_path: input.artifact_path,
        detail: input.detail.to_owned(),
    });
}

fn store_roundtrip_matches<T>(store: &T) -> Result<bool, LightningLabsInteropCheckError>
where
    T: Serialize + for<'de> Deserialize<'de> + Eq,
{
    let raw = serde_json::to_vec_pretty(store)?;
    let decoded = serde_json::from_slice::<T>(&raw)?;
    Ok(decoded == *store)
}

fn nonzero_digest_label(digest: Bytes32) -> String {
    if digest == Bytes32::ZERO {
        "zero".to_owned()
    } else {
        "nonzero".to_owned()
    }
}

#[derive(Debug)]
pub enum LightningLabsInteropCheckError {
    Json(serde_json::Error),
    Funding(LightningLabsFundingInteropError),
    Payment(LightningLabsOutgoingPaymentError),
    Proof(TapdProofError),
    AmountOverflow,
    StorageInvariant(String),
}

impl fmt::Display for LightningLabsInteropCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "Lightning Labs interop check JSON error: {err}"),
            Self::Funding(err) => write!(f, "Lightning Labs interop funding check error: {err}"),
            Self::Payment(err) => write!(f, "Lightning Labs interop payment check error: {err}"),
            Self::Proof(err) => write!(f, "Lightning Labs interop proof check error: {err}"),
            Self::AmountOverflow => write!(f, "Lightning Labs interop check amount overflow"),
            Self::StorageInvariant(message) => {
                write!(
                    f,
                    "Lightning Labs interop check storage invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for LightningLabsInteropCheckError {}

impl From<serde_json::Error> for LightningLabsInteropCheckError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<LightningLabsFundingInteropError> for LightningLabsInteropCheckError {
    fn from(err: LightningLabsFundingInteropError) -> Self {
        Self::Funding(err)
    }
}

impl From<LightningLabsOutgoingPaymentError> for LightningLabsInteropCheckError {
    fn from(err: LightningLabsOutgoingPaymentError) -> Self {
        Self::Payment(err)
    }
}

impl From<TapdProofError> for LightningLabsInteropCheckError {
    fn from(err: TapdProofError) -> Self {
        Self::Proof(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUNDING_HEXDUMP: &str = include_str!(
        "../../../fixtures/lightning-labs/tapchannelmsg/testdata/funding-blob.hexdump"
    );
    const COMMITMENT_HEXDUMP: &str = include_str!(
        "../../../fixtures/lightning-labs/tapchannelmsg/testdata/commitment-blob.hexdump"
    );
    const PROOF_FILE_HEX: &str =
        include_str!("../../../fixtures/lightning-labs/proof/testdata/proof-file.hex");
    const SINGLE_PROOF_HEX: &str =
        include_str!("../../../fixtures/lightning-labs/proof/testdata/proof.hex");

    #[test]
    fn interop_check_smoke_covers_funding_payments_proofs_and_restart() {
        let report = run_lightning_labs_interop_check_smoke(
            FUNDING_HEXDUMP,
            COMMITMENT_HEXDUMP,
            PROOF_FILE_HEX,
            SINGLE_PROOF_HEX,
            "fixtures/lightning-labs/tapchannelmsg/testdata",
            "fixtures/lightning-labs/proof/testdata",
            "target/lightning-labs-interop-checks.json",
        )
        .expect("interop checks pass");

        assert!(report.all_automated_checks_passed);
        assert!(report.live_daemon_gaps_remaining);
        assert!(report.mismatches.is_empty());
        assert_eq!(report.documented_gaps.len(), 3);
        assert!(report.checks.iter().any(|check| {
            check.name == "funding balances conserve asset amount"
                && check.status == LightningLabsInteropCheckStatus::Passed
        }));
        assert!(report.checks.iter().any(|check| {
            check.name == "outgoing live receiver balance remains documented gap"
                && check.status == LightningLabsInteropCheckStatus::DocumentedGap
        }));
        assert!(report.checks.iter().any(|check| {
            check.name == "incoming live receiver balance remains documented gap"
                && check.status == LightningLabsInteropCheckStatus::DocumentedGap
        }));
    }

    #[test]
    fn mismatch_diagnostics_include_required_fields() {
        let mut checks = Vec::new();
        let mut mismatches = Vec::new();
        record_comparison(
            &mut checks,
            &mut mismatches,
            ComparisonInput {
                name: "intentional mismatch".to_owned(),
                side: "lightning_labs",
                field: "receiver_balance_after",
                expected: "10".to_owned(),
                actual: "9".to_owned(),
                artifact_path: "target/mismatch.json".to_owned(),
                detail: "test mismatch",
            },
        );

        assert_eq!(checks[0].status, LightningLabsInteropCheckStatus::Failed);
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].side, "lightning_labs");
        assert_eq!(mismatches[0].field, "receiver_balance_after");
        assert_eq!(mismatches[0].expected, "10");
        assert_eq!(mismatches[0].actual, "9");
        assert_eq!(mismatches[0].artifact_path, "target/mismatch.json");
    }
}
