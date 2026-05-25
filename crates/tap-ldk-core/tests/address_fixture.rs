use std::{fs, path::Path, str::FromStr};

use serde_json::Value;
use tap_ldk_core::{
    address::{TapAddress, TapHrp},
    asset::{AssetAmount, Bytes32, CompressedKey},
};

#[test]
fn imported_address_vectors_encode_and_decode() {
    let fixture = load_json("fixtures/tap-bips/address_tlv_encoding_generated.json");
    let cases = fixture["valid_test_cases"]
        .as_array()
        .expect("valid_test_cases must be an array");

    for case in cases.iter().take(6) {
        let address = parse_address(&case["address"]);
        let expected = required_str(case, "expected");

        assert_eq!(address.encode().expect("address encodes"), expected);
        assert_eq!(
            TapAddress::decode(expected).expect("address decodes"),
            address
        );
    }
}

#[test]
fn imported_address_error_vectors_fail_closed() {
    let fixture = load_json("fixtures/tap-bips/address_tlv_encoding_error_cases.json");
    let cases = fixture["error_test_cases"]
        .as_array()
        .expect("error_test_cases must be an array");

    for case in cases.iter().take(8) {
        assert!(
            try_parse_address(&case["address"]).is_err(),
            "error case unexpectedly parsed: {}",
            required_str(case, "error")
        );
    }
}

fn parse_address(value: &Value) -> TapAddress {
    try_parse_address(value).expect("fixture address parses")
}

fn try_parse_address(value: &Value) -> Result<TapAddress, String> {
    let group_key = optional_str(value, "group_key")
        .filter(|key| !key.is_empty())
        .map(CompressedKey::from_str)
        .transpose()
        .map_err(|err| err.to_string())?;

    let tapscript_sibling = optional_str(value, "tapscript_sibling")
        .map(decode_hex)
        .transpose()?
        .unwrap_or_default();

    let address = TapAddress {
        hrp: TapHrp::from_str(required_str_result(value, "chain_params_hrp")?)
            .map_err(|err| err.to_string())?,
        version: required_u64_result(value, "asset_version")? as u8,
        asset_id: Bytes32::from_str(required_str_result(value, "asset_id")?)
            .map_err(|err| err.to_string())?,
        group_key,
        script_key: CompressedKey::from_str(required_str_result(value, "script_key")?)
            .map_err(|err| err.to_string())?,
        internal_key: CompressedKey::from_str(required_str_result(value, "internal_key")?)
            .map_err(|err| err.to_string())?,
        tapscript_sibling,
        amount: AssetAmount::new(required_u64_result(value, "amount")?),
        proof_courier_addr: required_str_result(value, "proof_courier_addr")?.to_owned(),
    };

    address.validate().map_err(|err| err.to_string())?;
    Ok(address)
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

fn optional_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn required_str<'a>(value: &'a Value, field: &str) -> &'a str {
    required_str_result(value, field).unwrap_or_else(|err| panic!("{err}"))
}

fn required_str_result<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field: {field}"))
}

fn required_u64_result(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing u64 field: {field}"))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("hex length must be even".to_owned());
    }

    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).expect("hex is utf8");
            u8::from_str_radix(text, 16).map_err(|err| err.to_string())
        })
        .collect()
}
