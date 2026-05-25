use std::{fs, path::Path};

use serde_json::Value;
use tap_ldk_core::tlv::{TlvError, TlvRecord, decode_stream, encode_stream};

#[test]
fn valid_tlv_fixture_streams_decode_and_round_trip() {
    let fixture = load_fixture();
    let streams = fixture["valid_streams"]
        .as_array()
        .expect("valid_streams must be an array");

    for stream in streams {
        let hex = required_str(stream, "hex");
        let bytes = decode_hex(hex);
        let decoded = decode_stream(&bytes).expect("valid TLV stream decodes");
        let expected_records = stream["records"]
            .as_array()
            .expect("records must be an array")
            .iter()
            .map(|record| {
                TlvRecord::new(
                    required_u64(record, "type_id"),
                    decode_hex(required_str(record, "value")),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(decoded, expected_records);
        assert_eq!(encode_stream(&decoded).expect("stream re-encodes"), bytes);
    }
}

#[test]
fn invalid_tlv_fixture_streams_fail_closed() {
    let fixture = load_fixture();
    let streams = fixture["invalid_streams"]
        .as_array()
        .expect("invalid_streams must be an array");

    for stream in streams {
        let name = required_str(stream, "name");
        let expected_error = required_str(stream, "error");
        let bytes = decode_hex(required_str(stream, "hex"));
        let err = decode_stream(&bytes).unwrap_err();
        let actual_error = classify_error(&err);

        assert_eq!(
            actual_error, expected_error,
            "unexpected error classification for fixture {name}: {err}"
        );
    }
}

fn load_fixture() -> Value {
    let path = repo_root().join("fixtures/synthetic/tlv_codec.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_str(&raw).expect("tlv fixture is valid JSON")
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn classify_error(err: &TlvError) -> &'static str {
    match err {
        TlvError::TruncatedBigSize => "truncated_big_size",
        TlvError::NonCanonicalBigSize { .. } => "non_canonical_big_size",
        TlvError::TruncatedRecord { .. } => "truncated_record",
        TlvError::DuplicateType(_) => "duplicate_type",
        TlvError::OutOfOrder { .. } => "out_of_order",
        TlvError::UnknownRequiredType(_) => "unknown_required_type",
    }
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

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "hex length must be even");
    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).expect("hex is utf8");
            u8::from_str_radix(text, 16).expect("hex byte decodes")
        })
        .collect()
}
