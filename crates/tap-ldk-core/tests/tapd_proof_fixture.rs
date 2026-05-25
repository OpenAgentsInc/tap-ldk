use std::{
    fs,
    path::{Path, PathBuf},
};

use tap_ldk_core::tapd_proof::{
    TapdProofError, decode_hex_text, decode_tapd_proof_file, decode_tapd_single_proof,
    wrap_single_proof_as_tapd_file,
};

const FIXTURE_DIR: &str = "fixtures/lightning-labs/proof/testdata";
const REPO_ROOT_FROM_CORE: &str = "../..";

#[test]
fn lightning_labs_proof_file_fixture_decodes_and_validates_chain() {
    let proof_file = decode_hex_fixture("proof-file.hex");
    let summary = decode_tapd_proof_file(&proof_file).expect("tapd proof file decodes");

    assert_eq!(summary.version, 0);
    assert_eq!(summary.proof_count, 3);
    assert_eq!(summary.proofs.len(), 3);
    assert_eq!(summary.raw_len, proof_file.len());
    assert_eq!(
        summary.final_chain_checksum,
        summary.proofs.last().expect("proof exists").chain_checksum
    );

    for proof in &summary.proofs {
        assert!(proof.raw_len > 0);
        assert!(proof.record_count > 0);
        assert_eq!(proof.transition_version, Some(0));
    }
}

#[test]
fn lightning_labs_single_proof_fixture_wraps_as_tapd_file() {
    let single_proof = decode_hex_fixture("proof.hex");
    let single_summary = decode_tapd_single_proof(&single_proof).expect("single proof decodes");
    let wrapped = wrap_single_proof_as_tapd_file(&single_proof).expect("single proof wraps");
    let wrapped_summary = decode_tapd_proof_file(&wrapped).expect("wrapped proof decodes");

    assert_eq!(wrapped_summary.proof_count, 1);
    assert_eq!(
        wrapped_summary.proofs[0].raw_digest,
        single_summary.raw_digest
    );
    assert_eq!(
        wrapped_summary.final_chain_checksum,
        wrapped_summary.proofs[0].chain_checksum
    );
}

#[test]
fn lightning_labs_proof_file_fixture_fails_closed_on_corruption() {
    let mut proof_file = decode_hex_fixture("proof-file.hex");
    let last = proof_file.last_mut().expect("proof file has checksum");
    *last ^= 1;

    assert!(matches!(
        decode_tapd_proof_file(&proof_file),
        Err(TapdProofError::InvalidChecksum { .. })
    ));

    let truncated = &proof_file[..proof_file.len() - 40];
    assert!(matches!(
        decode_tapd_proof_file(truncated),
        Err(TapdProofError::Truncated { .. })
    ));
}

fn decode_hex_fixture(file_name: &str) -> Vec<u8> {
    let raw = fs::read_to_string(fixture_path(file_name))
        .unwrap_or_else(|err| panic!("failed to read fixture {file_name}: {err}"));
    decode_hex_text(&raw)
        .unwrap_or_else(|err| panic!("failed to decode fixture {file_name}: {err}"))
}

fn fixture_path(file_name: &str) -> PathBuf {
    repo_root().join(FIXTURE_DIR).join(file_name)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(REPO_ROOT_FROM_CORE)
}
