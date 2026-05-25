use std::{
    fs,
    path::{Path, PathBuf},
};

use tap_ldk_core::{
    asset::Bytes32,
    lightning_labs_blob::{
        LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT, LightningLabsBlobError, TYPE_COMMITMENT_AUX_LEAVES,
        TYPE_COMMITMENT_INCOMING_HTLCS, TYPE_COMMITMENT_LOCAL_ASSETS,
        TYPE_COMMITMENT_OUTGOING_HTLCS, TYPE_COMMITMENT_REMOTE_ASSETS, TYPE_HTLC_RFQ_ID,
        decode_commitment_blob, decode_commitment_blob_hexdump, decode_fixture_hexdumps,
        decode_funding_blob, decode_funding_blob_hexdump, decode_htlc_blob,
        decode_htlc_blob_hexdump, extract_hexdump_bytes,
    },
    tlv::{TlvError, TlvRecord, encode_big_size, encode_stream},
};

const REPO_ROOT_FROM_CORE: &str = "../..";
const FIXTURE_DIR: &str = "fixtures/lightning-labs/tapchannelmsg/testdata";

#[test]
fn lightning_labs_blob_fixtures_decode_to_native_field_maps() {
    let funding_raw = fixture("funding-blob.hexdump");
    let htlc_raw = fixture("htlc-blob.hexdump");
    let commitment_raw = fixture("commitment-blob.hexdump");

    let report =
        decode_fixture_hexdumps(&funding_raw, &htlc_raw, &commitment_raw).expect("fixtures decode");

    assert_eq!(report.source_commit, LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT);

    assert_eq!(report.funding.raw_len, 1683);
    assert_eq!(
        report.funding.raw_digest.to_hex(),
        "cf806f27c40a227d6f4ca0c1e5f12906d216f11160829f189c94634f604d1d96"
    );
    assert_eq!(report.funding.decimal_display, 6);
    assert_eq!(report.funding.funded_assets.output_count, 1);
    assert_eq!(report.funding.funded_assets.total_amount, 100_000_000_000);
    assert_eq!(
        report.funding.funded_assets.outputs[0].asset_id.to_hex(),
        "5bbcbdf00f8e1065384efef9286646ca3b9765458df9a22baa1b1bd3bb75bf71"
    );
    assert_eq!(report.funding.funded_assets.outputs[0].proof_len, 1624);

    assert_eq!(report.htlc.raw_len, 45);
    assert_eq!(
        report.htlc.rfq_id.expect("RFQ id present").to_hex(),
        "cbe41e5c1bbe711d9edf3245c6d8484cc5a339fa3082a400f550ebe846373a3d"
    );
    assert_eq!(report.htlc.balances.balance_count, 0);
    assert_eq!(report.htlc.optional_unknown_records.len(), 1);
    assert_eq!(report.htlc.optional_unknown_records[0].type_id, 106_823);

    assert_eq!(report.commitment.raw_len, 3642);
    assert_eq!(report.commitment.local_assets.output_count, 1);
    assert_eq!(report.commitment.remote_assets.output_count, 1);
    assert_eq!(report.commitment.local_assets.total_amount, 56_700_021_068);
    assert_eq!(report.commitment.remote_assets.total_amount, 43_299_978_932);
    assert_eq!(
        report.commitment.local_assets.total_amount + report.commitment.remote_assets.total_amount,
        report.funding.funded_assets.total_amount
    );
    assert_eq!(report.commitment.outgoing_htlcs.htlc_count, 0);
    assert_eq!(report.commitment.incoming_htlcs.htlc_count, 0);
    assert_eq!(
        report
            .commitment
            .aux_leaves
            .local_leaf
            .as_ref()
            .unwrap()
            .script_len,
        73
    );
    assert_eq!(
        report
            .commitment
            .aux_leaves
            .remote_leaf
            .as_ref()
            .unwrap()
            .script_len,
        73
    );
}

#[test]
fn lightning_labs_blob_decoders_reject_truncation() {
    let fixture_cases: [(&str, fn(&[u8]) -> Result<(), LightningLabsBlobError>); 2] = [
        ("funding-blob.hexdump", |bytes| {
            decode_funding_blob(bytes).map(|_| ())
        }),
        ("commitment-blob.hexdump", |bytes| {
            decode_commitment_blob(bytes).map(|_| ())
        }),
    ];

    for (file_name, decoder) in fixture_cases {
        let bytes = extract_hexdump_bytes(&fixture(file_name)).expect("fixture extracts");
        for len in 0..bytes.len() {
            assert!(
                decoder(&bytes[..len]).is_err(),
                "{file_name} prefix of {len} bytes unexpectedly decoded"
            );
        }
    }

    let htlc = extract_hexdump_bytes(&fixture("htlc-blob.hexdump")).expect("fixture extracts");
    for len in 0..38 {
        assert!(
            decode_htlc_blob(&htlc[..len]).is_err(),
            "HTLC RFQ record prefix of {len} bytes unexpectedly decoded"
        );
    }
    for len in 39..htlc.len() {
        assert!(
            decode_htlc_blob(&htlc[..len]).is_err(),
            "HTLC optional record prefix of {len} bytes unexpectedly decoded"
        );
    }
}

#[test]
fn lightning_labs_blob_decoders_reject_non_canonical_tlv() {
    let non_canonical_type = [0xfd, 0x00, 0xfc, 0x00];

    assert!(matches!(
        decode_htlc_blob(&non_canonical_type),
        Err(LightningLabsBlobError::Tlv(TlvError::NonCanonicalBigSize {
            value: 0xfc,
            minimum: 0xfd
        }))
    ));
}

#[test]
fn lightning_labs_blob_decoders_reject_unsupported_and_semantically_wrong_fields() {
    let funding =
        decode_funding_blob_hexdump(&fixture("funding-blob.hexdump")).expect("funding decodes");
    assert_eq!(funding.decimal_display, 6);

    let htlc_with_unknown_even = encode_stream(&[
        TlvRecord::new(TYPE_HTLC_RFQ_ID, [3_u8; 32]),
        TlvRecord::new(65_542, [0]),
    ])
    .expect("HTLC encodes");
    assert!(matches!(
        decode_htlc_blob(&htlc_with_unknown_even),
        Err(LightningLabsBlobError::UnsupportedRecord {
            blob: "htlc",
            type_id: 65_542
        })
    ));

    let bad_decimal_display = encode_stream(&[
        TlvRecord::new(0, encode_one_empty_asset_output_list()),
        TlvRecord::new(1, [1, 2]),
    ])
    .expect("funding encodes");
    assert!(matches!(
        decode_funding_blob(&bad_decimal_display),
        Err(LightningLabsBlobError::InvalidFieldLength {
            field: "funding.decimal_display",
            expected: 1,
            actual: 2
        })
    ));

    let commitment_missing_aux = encode_stream(&[
        TlvRecord::new(TYPE_COMMITMENT_LOCAL_ASSETS, empty_asset_output_list()),
        TlvRecord::new(TYPE_COMMITMENT_REMOTE_ASSETS, empty_asset_output_list()),
        TlvRecord::new(TYPE_COMMITMENT_OUTGOING_HTLCS, empty_htlc_asset_output()),
        TlvRecord::new(TYPE_COMMITMENT_INCOMING_HTLCS, empty_htlc_asset_output()),
    ])
    .expect("commitment encodes");
    assert!(matches!(
        decode_commitment_blob(&commitment_missing_aux),
        Err(LightningLabsBlobError::MissingRecord {
            blob: "commitment",
            type_id: TYPE_COMMITMENT_AUX_LEAVES
        })
    ));
}

#[test]
fn lightning_labs_blob_decoders_reject_wrong_inner_lengths() {
    let bad_rfq_id =
        encode_stream(&[TlvRecord::new(TYPE_HTLC_RFQ_ID, [1_u8; 31])]).expect("bad HTLC encodes");
    assert!(matches!(
        decode_htlc_blob(&bad_rfq_id),
        Err(LightningLabsBlobError::InvalidFieldLength {
            field: "htlc.rfq_id",
            expected: 32,
            actual: 31
        })
    ));

    let bad_leaf = vec![0xc0, 2, 1, 0x51];
    let aux = encode_stream(&[
        TlvRecord::new(0, bad_leaf.clone()),
        TlvRecord::new(2, empty_htlc_aux_leaf_map()),
        TlvRecord::new(3, empty_htlc_aux_leaf_map()),
    ])
    .expect("aux encodes");
    let mut aux_inline = Vec::new();
    encode_big_size(aux.len() as u64, &mut aux_inline);
    aux_inline.extend_from_slice(&aux);

    let commitment_bad_aux = encode_stream(&[
        TlvRecord::new(TYPE_COMMITMENT_LOCAL_ASSETS, empty_asset_output_list()),
        TlvRecord::new(TYPE_COMMITMENT_REMOTE_ASSETS, empty_asset_output_list()),
        TlvRecord::new(TYPE_COMMITMENT_OUTGOING_HTLCS, empty_htlc_asset_output()),
        TlvRecord::new(TYPE_COMMITMENT_INCOMING_HTLCS, empty_htlc_asset_output()),
        TlvRecord::new(TYPE_COMMITMENT_AUX_LEAVES, aux_inline),
    ])
    .expect("commitment encodes");
    assert!(matches!(
        decode_commitment_blob(&commitment_bad_aux),
        Err(LightningLabsBlobError::Semantic {
            field: "commitment.aux_leaves.local_leaf",
            reason: "declared script length does not match inline script length"
        })
    ));
}

#[test]
fn individual_hexdump_decoders_match_combined_report() {
    let funding =
        decode_funding_blob_hexdump(&fixture("funding-blob.hexdump")).expect("funding decodes");
    let htlc = decode_htlc_blob_hexdump(&fixture("htlc-blob.hexdump")).expect("htlc decodes");
    let commitment = decode_commitment_blob_hexdump(&fixture("commitment-blob.hexdump"))
        .expect("commitment decodes");

    assert_eq!(funding.raw_len, 1683);
    assert_eq!(htlc.raw_len, 45);
    assert_eq!(commitment.raw_len, 3642);
}

fn empty_asset_output_list() -> Vec<u8> {
    vec![0]
}

fn empty_htlc_asset_output() -> Vec<u8> {
    vec![0]
}

fn empty_htlc_aux_leaf_map() -> Vec<u8> {
    vec![0]
}

fn encode_one_empty_asset_output_list() -> Vec<u8> {
    let output = encode_stream(&[
        TlvRecord::new(0, Bytes32::ZERO.0),
        TlvRecord::new(1, 0_u64.to_be_bytes()),
        TlvRecord::new(2, []),
    ])
    .expect("asset output encodes");
    let mut encoded = vec![1];
    encode_big_size(output.len() as u64, &mut encoded);
    encoded.extend_from_slice(&output);
    encoded
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
