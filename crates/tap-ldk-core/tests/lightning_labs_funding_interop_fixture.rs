use std::{
    fs,
    path::{Path, PathBuf},
};

use tap_ldk_core::lightning_labs_funding::{
    LightningLabsFundingInteropError, LightningLabsFundingInteropStatus,
    LightningLabsFundingInteropStore, run_lightning_labs_funding_interop_fixture_smoke,
};

const FIXTURE_DIR: &str = "fixtures/lightning-labs/tapchannelmsg/testdata";
const REPO_ROOT_FROM_CORE: &str = "../..";

#[test]
fn lightning_labs_funding_fixture_maps_asset_balance_and_gap_state() {
    let funding = fixture("funding-blob.hexdump");
    let commitment = fixture("commitment-blob.hexdump");

    let (store, report) = run_lightning_labs_funding_interop_fixture_smoke(&funding, &commitment)
        .expect("funding interop fixture maps");

    assert_eq!(
        report.status,
        LightningLabsFundingInteropStatus::StoppedAtDocumentedGap
    );
    assert_eq!(
        report.asset_id.to_hex(),
        "5bbcbdf00f8e1065384efef9286646ca3b9765458df9a22baa1b1bd3bb75bf71"
    );
    assert_eq!(report.funding_total_amount, 100_000_000_000);
    assert_eq!(report.local_balance, 56_700_021_068);
    assert_eq!(report.remote_balance, 43_299_978_932);
    assert!(report.balance_comparison.balances_match);
    assert_eq!(store.states.len(), 1);
    assert!(report.documented_gap.field.contains("funding_outpoint"));
}

#[test]
fn lightning_labs_funding_fixture_state_survives_restart() {
    let path = temp_store_path();
    let funding = fixture("funding-blob.hexdump");
    let commitment = fixture("commitment-blob.hexdump");
    let (store, report) = run_lightning_labs_funding_interop_fixture_smoke(&funding, &commitment)
        .expect("funding interop fixture maps");

    store.save_atomic(&path).expect("store saves");
    let loaded = LightningLabsFundingInteropStore::load(&path).expect("store reloads");

    let state = loaded
        .states
        .get(&report.interop_id)
        .expect("interop state persists");
    assert_eq!(state.local_balance, report.local_balance);
    assert_eq!(state.remote_balance, report.remote_balance);
    assert_eq!(state.funding_total_amount, report.funding_total_amount);
    fs::remove_file(path).ok();
}

#[test]
fn lightning_labs_funding_fixture_rejects_mismatched_commitment() {
    let funding = fixture("funding-blob.hexdump");
    let commitment = fixture("funding-blob.hexdump");

    assert!(matches!(
        run_lightning_labs_funding_interop_fixture_smoke(&funding, &commitment),
        Err(LightningLabsFundingInteropError::Blob(_))
    ));
}

fn fixture(file_name: &str) -> String {
    fs::read_to_string(fixture_path(file_name))
        .unwrap_or_else(|err| panic!("failed to read fixture {file_name}: {err}"))
}

fn fixture_path(file_name: &str) -> PathBuf {
    repo_root().join(FIXTURE_DIR).join(file_name)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(REPO_ROOT_FROM_CORE)
}

fn temp_store_path() -> PathBuf {
    repo_root().join(format!(
        "target/lightning_labs_funding_interop_{}_{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos()
    ))
}
