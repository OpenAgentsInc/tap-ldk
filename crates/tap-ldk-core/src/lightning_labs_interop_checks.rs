use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    asset::Bytes32,
    lightning_labs_blob::{
        LightningLabsBlobError, LightningLabsHtlcBlob, decode_htlc_blob_hexdump,
    },
    lightning_labs_funding::{
        FundingInteropGap, LightningLabsFundingInteropError, LightningLabsFundingInteropReport,
        run_lightning_labs_funding_interop_fixture_smoke,
    },
    lightning_labs_payment::{
        LightningLabsIncomingPaymentReport, LightningLabsOutgoingPaymentError,
        LightningLabsOutgoingPaymentReport, run_lightning_labs_incoming_payment_smoke,
        run_lightning_labs_outgoing_payment_smoke,
    },
    lightning_labs_rfq::{
        LIGHTNING_LABS_RFQ_ACCEPT_TYPE, LIGHTNING_LABS_RFQ_REJECT_TYPE,
        LIGHTNING_LABS_RFQ_REQUEST_TYPE, LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT,
    },
    simple_taproot_asset_channel::{
        SimpleTaprootAssetChannelIntegrationError, SimpleTaprootAssetChannelIntegrationReport,
        run_simple_taproot_asset_channel_integration_smoke,
    },
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
    pub htlc_blob_digest: Bytes32,
    pub htlc_rfq_id: Option<Bytes32>,
    pub htlc_available_rfq_ids: usize,
    pub proof_file_digest: Bytes32,
    pub outgoing_payment_id: String,
    pub incoming_payment_id: String,
    pub simple_taproot_rust_lightning_rev: String,
    pub simple_taproot_channel_id: String,
    pub simple_taproot_lifecycle_passed: bool,
    pub close_and_recovery_vectors_passed: bool,
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
            || self.simple_taproot_rust_lightning_rev.trim().is_empty()
            || self.simple_taproot_channel_id.trim().is_empty()
        {
            return Err(LightningLabsInteropCheckError::StorageInvariant(
                "interop check identity fields cannot be empty".to_owned(),
            ));
        }
        if self.htlc_blob_digest == Bytes32::ZERO || self.proof_file_digest == Bytes32::ZERO {
            return Err(LightningLabsInteropCheckError::StorageInvariant(
                "interop fixture digests cannot be zero".to_owned(),
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
    htlc_hexdump: &str,
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
    let htlc = decode_htlc_blob_hexdump(htlc_hexdump)?;
    let proof = decode_fixture_hex(proof_file_hex, single_proof_hex)?;
    let simple_taproot = run_simple_taproot_asset_channel_integration_smoke()?;

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
    record_htlc_checks(&mut checks, &mut mismatches, &htlc, tapchannel_fixture_dir);
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
    record_rfq_vector_checks(
        &mut checks,
        &mut mismatches,
        &outgoing,
        &incoming,
        &report_artifact("rfq_vectors"),
    );
    record_simple_taproot_checks(
        &mut checks,
        &mut mismatches,
        &simple_taproot,
        &report_artifact("simple_taproot_asset_channel"),
    );
    checks.push(LightningLabsInteropCheck {
        name: "live Lightning Labs cooperative close remains documented gap".to_owned(),
        status: LightningLabsInteropCheckStatus::DocumentedGap,
        side: "both".to_owned(),
        field: "litd_closechannel_post_close_asset_balances".to_owned(),
        expected:
            "litd CloseChannel with native post-close Taproot Asset proof and balance observation"
                .to_owned(),
        actual: "fixture-boundary-only".to_owned(),
        artifact_path: report_artifact("litd_cooperative_close.documented_gap"),
        detail: "Lightning Labs exposes LND CloseChannel for asset channels through taproot-assets AuxChanCloser/ShutdownBlob/FinalizeClose, and tap-ldk exposes a litd close command; the live harness still needs native post-close proof and balance observation before claiming live close completion."
            .to_owned(),
    });

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
        htlc_blob_digest: htlc.raw_digest,
        htlc_rfq_id: htlc.rfq_id,
        htlc_available_rfq_ids: htlc.available_rfq_ids.len(),
        proof_file_digest: proof.proof_file.raw_digest,
        outgoing_payment_id: outgoing.payment_id,
        incoming_payment_id: incoming.payment_id,
        simple_taproot_rust_lightning_rev: simple_taproot.rust_lightning_rev.clone(),
        simple_taproot_channel_id: simple_taproot.channel_id.clone(),
        simple_taproot_lifecycle_passed: simple_taproot_lifecycle_passed(&simple_taproot),
        close_and_recovery_vectors_passed: close_and_recovery_vectors_passed(&simple_taproot),
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

fn record_htlc_checks(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    htlc: &LightningLabsHtlcBlob,
    tapchannel_fixture_dir: &str,
) {
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "HTLC fixture carries RFQ metadata".to_owned(),
            side: "lightning_labs",
            field: "htlc.rfq_id",
            expected: "present".to_owned(),
            actual: if htlc.rfq_id.is_some() {
                "present".to_owned()
            } else {
                "missing".to_owned()
            },
            artifact_path: format!("{tapchannel_fixture_dir}/htlc-blob.hexdump"),
            detail: "Lightning Labs HTLC metadata vector exposes the RFQ binding needed by asset payments",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "HTLC fixture digest is nonzero".to_owned(),
            side: "lightning_labs",
            field: "htlc.raw_digest",
            expected: "nonzero".to_owned(),
            actual: nonzero_digest_label(htlc.raw_digest),
            artifact_path: format!("{tapchannel_fixture_dir}/htlc-blob.hexdump"),
            detail: "HTLC fixture bytes are decoded as a stable compatibility vector",
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

fn record_rfq_vector_checks(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    outgoing: &LightningLabsOutgoingPaymentReport,
    incoming: &LightningLabsIncomingPaymentReport,
    artifact_path: &str,
) {
    for (direction, request, accept, reject) in [
        (
            "outgoing",
            outgoing.request_message_type,
            outgoing.accept_message_type,
            outgoing.reject_message_type,
        ),
        (
            "incoming",
            incoming.request_message_type,
            incoming.accept_message_type,
            incoming.reject_message_type,
        ),
    ] {
        record_comparison(
            checks,
            mismatches,
            ComparisonInput {
                name: format!("{direction} RFQ request type matches Lightning Labs"),
                side: "both",
                field: "rfq.request_type",
                expected: LIGHTNING_LABS_RFQ_REQUEST_TYPE.to_string(),
                actual: request.to_string(),
                artifact_path: artifact_path.to_owned(),
                detail: "RFQ request messages use the Lightning Labs custom message type",
            },
        );
        record_comparison(
            checks,
            mismatches,
            ComparisonInput {
                name: format!("{direction} RFQ accept type matches Lightning Labs"),
                side: "both",
                field: "rfq.accept_type",
                expected: LIGHTNING_LABS_RFQ_ACCEPT_TYPE.to_string(),
                actual: accept.to_string(),
                artifact_path: artifact_path.to_owned(),
                detail: "RFQ accept messages use the Lightning Labs custom message type",
            },
        );
        record_comparison(
            checks,
            mismatches,
            ComparisonInput {
                name: format!("{direction} RFQ reject type matches Lightning Labs"),
                side: "both",
                field: "rfq.reject_type",
                expected: LIGHTNING_LABS_RFQ_REJECT_TYPE.to_string(),
                actual: reject.to_string(),
                artifact_path: artifact_path.to_owned(),
                detail: "RFQ reject messages use the Lightning Labs custom message type",
            },
        );
    }
}

fn record_simple_taproot_checks(
    checks: &mut Vec<LightningLabsInteropCheck>,
    mismatches: &mut Vec<LightningLabsInteropMismatch>,
    simple_taproot: &SimpleTaprootAssetChannelIntegrationReport,
    artifact_path: &str,
) {
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "simple-taproot asset-channel lifecycle passes".to_owned(),
            side: "tap-ldk",
            field: "simple_taproot_lifecycle_passed",
            expected: "true".to_owned(),
            actual: simple_taproot_lifecycle_passed(simple_taproot).to_string(),
            artifact_path: artifact_path.to_owned(),
            detail: "Lightning Labs vectors are checked alongside the fork-backed simple-taproot asset-channel lifecycle",
        },
    );
    record_comparison(
        checks,
        mismatches,
        ComparisonInput {
            name: "simple-taproot close and recovery vectors pass".to_owned(),
            side: "tap-ldk",
            field: "close_and_recovery_vectors_passed",
            expected: "true".to_owned(),
            actual: close_and_recovery_vectors_passed(simple_taproot).to_string(),
            artifact_path: artifact_path.to_owned(),
            detail: "cooperative close proof export, latest-allocation preservation, restart recovery, and force-close proof-ownership recovery are exercised through the rust-lightning fork state",
        },
    );
}

fn simple_taproot_lifecycle_passed(report: &SimpleTaprootAssetChannelIntegrationReport) -> bool {
    report.negotiated_simple_taproot_asset_channel
        && report.proof_exchange_separate_from_open_channel
        && report.funding_hook_approved
        && report.initial_monitor_aux_persisted
        && report.missing_monitor_update_rejected
        && report.ldk_state_advanced_with_monitor_aux
        && report.payment_settled
        && report.restart_reestablish_survived
        && report.btc_only_baseline_unaffected
}

fn close_and_recovery_vectors_passed(report: &SimpleTaprootAssetChannelIntegrationReport) -> bool {
    report.cooperative_close_exported
        && report.cooperative_close_allocation_validated_by_ldk
        && report.cooperative_close_preserved_latest_asset_allocation
        && report.cooperative_close_restart_preserved_asset_allocation
        && report.force_close_proof_ownership_validated_by_ldk
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
    Blob(LightningLabsBlobError),
    Funding(LightningLabsFundingInteropError),
    Payment(LightningLabsOutgoingPaymentError),
    Proof(TapdProofError),
    SimpleTaproot(SimpleTaprootAssetChannelIntegrationError),
    AmountOverflow,
    StorageInvariant(String),
}

impl fmt::Display for LightningLabsInteropCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "Lightning Labs interop check JSON error: {err}"),
            Self::Blob(err) => write!(f, "Lightning Labs interop blob check error: {err}"),
            Self::Funding(err) => write!(f, "Lightning Labs interop funding check error: {err}"),
            Self::Payment(err) => write!(f, "Lightning Labs interop payment check error: {err}"),
            Self::Proof(err) => write!(f, "Lightning Labs interop proof check error: {err}"),
            Self::SimpleTaproot(err) => write!(
                f,
                "Lightning Labs simple-taproot asset-channel check error: {err}"
            ),
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

impl From<LightningLabsBlobError> for LightningLabsInteropCheckError {
    fn from(err: LightningLabsBlobError) -> Self {
        Self::Blob(err)
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

impl From<SimpleTaprootAssetChannelIntegrationError> for LightningLabsInteropCheckError {
    fn from(err: SimpleTaprootAssetChannelIntegrationError) -> Self {
        Self::SimpleTaproot(err)
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
    const HTLC_HEXDUMP: &str =
        include_str!("../../../fixtures/lightning-labs/tapchannelmsg/testdata/htlc-blob.hexdump");
    const PROOF_FILE_HEX: &str =
        include_str!("../../../fixtures/lightning-labs/proof/testdata/proof-file.hex");
    const SINGLE_PROOF_HEX: &str =
        include_str!("../../../fixtures/lightning-labs/proof/testdata/proof.hex");

    #[test]
    fn interop_check_smoke_covers_funding_payments_proofs_and_restart() {
        let report = run_lightning_labs_interop_check_smoke(
            FUNDING_HEXDUMP,
            HTLC_HEXDUMP,
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
        assert!(report.simple_taproot_lifecycle_passed);
        assert!(report.close_and_recovery_vectors_passed);
        assert!(report.htlc_rfq_id.is_some());
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
        assert!(report.checks.iter().any(|check| {
            check.name == "HTLC fixture carries RFQ metadata"
                && check.status == LightningLabsInteropCheckStatus::Passed
        }));
        assert!(report.checks.iter().any(|check| {
            check.name == "simple-taproot close and recovery vectors pass"
                && check.status == LightningLabsInteropCheckStatus::Passed
        }));
        assert!(report.checks.iter().any(|check| {
            check.name == "live Lightning Labs cooperative close remains documented gap"
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
