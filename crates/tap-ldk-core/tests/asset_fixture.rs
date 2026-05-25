use std::{fs, path::Path, str::FromStr};

use serde_json::Value;
use tap_ldk_core::asset::{
    AssetAmount, AssetError, AssetLeaf, AssetType, Bytes32, CompressedKey, Genesis,
    validate_split_conservation,
};

#[test]
fn asset_identity_fixture_derives_expected_asset_id() {
    let fixture = load_fixture();
    let genesis = parse_genesis(&fixture["genesis"]);

    assert_eq!(
        genesis.asset_id().to_hex(),
        required_str(&fixture, "expected_asset_id")
    );
}

#[test]
fn split_fixture_conserves_amount_and_root_sum() {
    let fixture = load_fixture();
    let asset_id =
        Bytes32::from_str(required_str(&fixture, "expected_asset_id")).expect("asset id parses");
    let split = &fixture["split"];
    let leaves = split["outputs"]
        .as_array()
        .expect("split outputs must be an array")
        .iter()
        .map(|output| AssetLeaf {
            asset_id,
            script_key: CompressedKey::from_str(required_str(output, "script_key"))
                .expect("script key parses"),
            amount: AssetAmount::new(required_u64(output, "amount")),
        })
        .collect::<Vec<_>>();

    let root = validate_split_conservation(
        AssetAmount::new(required_u64(split, "input_amount")),
        &leaves,
    )
    .expect("split conserves amount");

    assert_eq!(
        root.hash.to_hex(),
        required_str(split, "expected_root_hash")
    );
    assert_eq!(root.sum.value(), required_u64(split, "expected_root_sum"));
}

#[test]
fn wrong_split_sum_fails_closed() {
    let fixture = load_fixture();
    let asset_id =
        Bytes32::from_str(required_str(&fixture, "expected_asset_id")).expect("asset id parses");
    let script_key = CompressedKey::from_str(
        "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
    )
    .expect("script key parses");
    let leaves = [
        AssetLeaf {
            asset_id,
            script_key,
            amount: AssetAmount::new(750000),
        },
        AssetLeaf {
            asset_id,
            script_key,
            amount: AssetAmount::new(249999),
        },
    ];

    assert_eq!(
        validate_split_conservation(AssetAmount::new(1000000), &leaves),
        Err(AssetError::AmountNotConserved {
            input: 1000000,
            output: 999999
        })
    );
}

fn parse_genesis(value: &Value) -> Genesis {
    Genesis {
        first_prev_out: required_str(value, "first_prev_out").to_owned(),
        tag: Bytes32::from_str(required_str(value, "tag")).expect("tag parses"),
        meta_hash: Bytes32::from_str(required_str(value, "meta_hash")).expect("meta hash parses"),
        output_index: required_u64(value, "output_index") as u32,
        asset_type: match required_str(value, "asset_type") {
            "normal" => AssetType::Normal,
            "collectible" => AssetType::Collectible,
            other => panic!("unknown asset_type: {other}"),
        },
    }
}

fn load_fixture() -> Value {
    let path = repo_root().join("fixtures/synthetic/asset_identity.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_str(&raw).expect("asset fixture is valid JSON")
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
