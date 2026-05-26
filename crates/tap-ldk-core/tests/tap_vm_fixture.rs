use std::{fs, path::Path};

use tap_ldk_core::tap_vm::{TapVmFixture, TapVmTransitionKind};

#[test]
fn tap_bip_vm_generated_valid_vectors_pass() {
    let fixture = load_fixture("fixtures/tap-bips/vm_validation_generated.json");
    assert_eq!(fixture.valid_test_cases.len(), 10);

    let mut issuance_count = 0;
    let mut transfer_count = 0;
    let mut split_count = 0;
    let mut script_witness_count = 0;

    for case in &fixture.valid_test_cases {
        let summary = case
            .validate()
            .unwrap_or_else(|err| panic!("valid TAP VM case failed: {}: {err}", case.comment));
        match summary.transition_kind {
            TapVmTransitionKind::Issuance => issuance_count += 1,
            TapVmTransitionKind::Transfer => transfer_count += 1,
            _ => panic!("unexpected transition kind in fixture"),
        }
        if summary.split_root_sum.is_some() {
            split_count += 1;
        }
        script_witness_count += summary.script_witnesses_checked;
        if summary.transition_kind != TapVmTransitionKind::Issuance {
            assert_eq!(summary.input_amount, summary.output_amount);
        }
    }

    assert_eq!(issuance_count, 2);
    assert_eq!(transfer_count, 8);
    assert_eq!(split_count, 6);
    assert!(script_witness_count >= 8);
}

#[test]
fn tap_bip_vm_generated_error_vectors_fail_closed() {
    let fixture = load_fixture("fixtures/tap-bips/vm_validation_generated_error_cases.json");
    assert_eq!(fixture.error_test_cases.len(), 7);

    for case in &fixture.error_test_cases {
        assert!(
            case.validate().is_err(),
            "invalid TAP VM case unexpectedly passed: {}",
            case.comment
        );
    }
}

fn load_fixture(relative_path: &str) -> TapVmFixture {
    let path = repo_root().join(relative_path);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    TapVmFixture::from_json_str(&raw).expect("fixture decodes")
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
