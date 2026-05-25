use std::{fs, path::Path};

use serde_json::Value;
use tap_ldk_core::{
    asset::AssetAmount,
    virtual_psbt::{SigningDomain, VirtualPacketSummary},
};

#[test]
fn imported_virtual_psbt_vectors_validate_summary() {
    let fixture = load_json("fixtures/tap-bips/psbt_encoding_generated.json");
    let cases = fixture["valid_test_cases"]
        .as_array()
        .expect("valid_test_cases must be an array");

    for case in cases.iter().take(3) {
        let packet = &case["packet"];
        let summary = parse_summary(packet);

        summary
            .validate()
            .expect("virtual packet summary validates");
        assert!(summary.canonical_summary().starts_with("tap-vpsbt:v"));
        assert!(
            required_str(case, "expected").len() > 8,
            "fixture expected base64 PSBT must be populated"
        );
    }
}

#[test]
fn asset_signing_domain_is_separate_from_btc_domain() {
    assert_ne!(
        SigningDomain::TaprootAssets.nonce_context(),
        SigningDomain::Bitcoin.nonce_context()
    );
}

fn parse_summary(packet: &Value) -> VirtualPacketSummary {
    let inputs = packet["inputs"]
        .as_array()
        .expect("inputs must be an array");
    let outputs = packet["outputs"]
        .as_array()
        .expect("outputs must be an array");
    let total_output_amount = outputs
        .iter()
        .map(|output| AssetAmount::new(required_u64(output, "amount")))
        .try_fold(AssetAmount::ZERO, AssetAmount::checked_add)
        .expect("output amount does not overflow");

    VirtualPacketSummary {
        version: required_u64(packet, "version") as u8,
        chain_params_hrp: required_str(packet, "chain_params_hrp").to_owned(),
        input_count: inputs.len(),
        output_count: outputs.len(),
        total_output_amount,
        signing_domain: SigningDomain::TaprootAssets,
    }
}

fn load_json(relative_path: &str) -> Value {
    let path = repo_root().join(relative_path);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_str(&raw).expect("fixture is valid JSON")
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn required_str<'a>(value: &'a Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing string field: {field}"))
}

fn required_u64(value: &Value, field: &str) -> u64 {
    value
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 field: {field}"))
}
