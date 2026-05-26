use tap_ldk_core::lightning_labs_interop_checks::{
    LightningLabsInteropCheckStatus, run_lightning_labs_interop_check_smoke,
};

const FUNDING_HEXDUMP: &str =
    include_str!("../../../fixtures/lightning-labs/tapchannelmsg/testdata/funding-blob.hexdump");
const HTLC_HEXDUMP: &str =
    include_str!("../../../fixtures/lightning-labs/tapchannelmsg/testdata/htlc-blob.hexdump");
const COMMITMENT_HEXDUMP: &str =
    include_str!("../../../fixtures/lightning-labs/tapchannelmsg/testdata/commitment-blob.hexdump");
const PROOF_FILE_HEX: &str =
    include_str!("../../../fixtures/lightning-labs/proof/testdata/proof-file.hex");
const SINGLE_PROOF_HEX: &str =
    include_str!("../../../fixtures/lightning-labs/proof/testdata/proof.hex");

#[test]
fn lightning_labs_vectors_cover_simple_taproot_asset_channel_path() {
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
    .expect("Lightning Labs interop vectors pass");

    assert!(report.all_automated_checks_passed);
    assert!(report.live_daemon_gaps_remaining);
    assert!(report.htlc_rfq_id.is_some());
    assert!(report.simple_taproot_lifecycle_passed);
    assert!(report.close_and_recovery_vectors_passed);
    assert!(report.checks.iter().any(|check| check.name
        == "outgoing RFQ request type matches Lightning Labs"
        && check.status == LightningLabsInteropCheckStatus::Passed));
    assert!(report.checks.iter().any(|check| check.name
        == "incoming live receiver balance remains documented gap"
        && check.status == LightningLabsInteropCheckStatus::DocumentedGap));
}
