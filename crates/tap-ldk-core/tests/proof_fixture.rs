use std::{fs, path::Path, str::FromStr};

use serde_json::Value;
use tap_ldk_core::{
    asset::{AssetAmount, AssetType, Bytes32, CompressedKey, RootHashSum},
    proof::{ProofError, ProofFile, ProofNetwork, ProofValidationContext, VerificationScope},
};

#[test]
fn proof_fixture_round_trips_and_verifies() {
    let proof = load_valid_proof();
    let encoded = proof.encode().expect("proof encodes");
    let decoded = ProofFile::decode(&encoded).expect("proof decodes");

    assert_eq!(decoded, proof);
    decoded
        .verify_semantic_ancestry(&ProofValidationContext::default())
        .expect("proof verifies");
}

#[test]
fn invalid_proof_fixtures_fail_closed() {
    let invalid = load_json("fixtures/synthetic/proof_anchor_invalid.json");
    let cases = invalid["cases"]
        .as_array()
        .expect("invalid cases must be an array");

    for case in cases {
        let name = required_str(case, "name");
        let expected_error = required_str(case, "error");
        let mut proof = load_valid_proof();
        apply_mutation(&mut proof, &case["mutate"]);
        let err = proof
            .verify_semantic_ancestry(&ProofValidationContext::default())
            .unwrap_err();

        assert_eq!(
            classify_error(&err),
            expected_error,
            "unexpected proof error for case {name}: {err}"
        );
    }
}

fn load_valid_proof() -> ProofFile {
    let value = load_json("fixtures/synthetic/proof_anchor_valid.json");
    let root = &value["tap_asset_root"];

    ProofFile {
        version: 0,
        asset_id: Bytes32::from_str(required_str(&value, "asset_id")).expect("asset id parses"),
        genesis_outpoint: required_str(&value, "genesis_outpoint").to_owned(),
        anchor_outpoint: required_str(&value, "anchor_outpoint").to_owned(),
        amount: AssetAmount::new(required_u64(&value, "amount")),
        script_key: CompressedKey::from_str(required_str(&value, "script_key"))
            .expect("script key parses"),
        tap_asset_root: RootHashSum {
            hash: Bytes32::from_str(required_str(root, "hash")).expect("root hash parses"),
            sum: AssetAmount::new(required_u64(root, "sum")),
        },
        verification_scope: VerificationScope::from_str(required_str(&value, "verification_scope"))
            .expect("scope parses"),
        network: ProofNetwork::from_str(required_str(&value, "network")).expect("network parses"),
        asset_type: AssetType::from_u8(required_u64(&value, "asset_type") as u8)
            .expect("asset type parses"),
    }
}

fn apply_mutation(proof: &mut ProofFile, mutation: &Value) {
    if let Some(asset_id) = mutation.get("asset_id").and_then(Value::as_str) {
        proof.asset_id = Bytes32::from_str(asset_id).expect("mutated asset id parses");
    }
    if let Some(sum) = mutation.get("tap_asset_root_sum").and_then(Value::as_u64) {
        proof.tap_asset_root.sum = AssetAmount::new(sum);
    }
    if let Some(hash) = mutation.get("tap_asset_root_hash").and_then(Value::as_str) {
        proof.tap_asset_root.hash = Bytes32::from_str(hash).expect("mutated root hash parses");
    }
    if let Some(anchor) = mutation.get("anchor_outpoint").and_then(Value::as_str) {
        proof.anchor_outpoint = anchor.to_owned();
    }
    if let Some(genesis) = mutation.get("genesis_outpoint").and_then(Value::as_str) {
        proof.genesis_outpoint = genesis.to_owned();
    }
    if let Some(scope) = mutation.get("verification_scope").and_then(Value::as_str) {
        proof.verification_scope = VerificationScope::from_str(scope).expect("scope parses");
    }
}

fn classify_error(err: &ProofError) -> &'static str {
    match err {
        ProofError::ZeroAssetId => "zero_asset_id",
        ProofError::RootSumMismatch { .. } => "root_sum_mismatch",
        ProofError::UnsupportedScope(_) => "unsupported_scope",
        ProofError::ZeroAmount => "zero_amount",
        ProofError::UnsupportedVersion(_) => "unsupported_version",
        ProofError::MalformedOutpoint(_) => "malformed_outpoint",
        ProofError::CommitmentRootMismatch { .. } => "commitment_root_mismatch",
        ProofError::BrokenAncestry(_) => "broken_ancestry",
        ProofError::StaleProof { .. } => "stale_proof",
        ProofError::WrongAsset { .. } => "wrong_asset",
        ProofError::WrongOwner { .. } => "wrong_owner",
        ProofError::WrongAmount { .. } => "wrong_amount",
        ProofError::WrongNetwork { .. } => "wrong_network",
        ProofError::WrongAssetType { .. } => "wrong_asset_type",
        ProofError::UnsupportedNetwork(_) => "unsupported_network",
        ProofError::MissingTapdProofSummary => "missing_tapd_proof_summary",
        ProofError::StaleTapdProof { .. } => "stale_tapd_proof",
        ProofError::Tlv(_) => "tlv",
        ProofError::Asset(_) => "asset",
        ProofError::MissingField(_) => "missing_field",
        ProofError::InvalidFieldLength { .. } => "invalid_field_length",
        ProofError::InvalidUtf8(_) => "invalid_utf8",
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
