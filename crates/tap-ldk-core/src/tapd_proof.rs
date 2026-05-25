use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    tlv::{
        TlvError, TlvRecord, decode_big_size, decode_stream, encode_big_size,
        reject_unknown_required,
    },
};

pub const TAPD_PROOF_MAGIC: &[u8; 4] = b"TAPP";
pub const TAPD_PROOF_FILE_MAGIC: &[u8; 4] = b"TAPF";
pub const TAPD_PROOF_FILE_VERSION: u32 = 0;
pub const LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT: &str = "743db21da57b5fdecf5daca9a925f0261ca94e40";

const TYPE_VERSION: u64 = 0;
const TYPE_PREV_OUT: u64 = 2;
const TYPE_BLOCK_HEADER: u64 = 4;
const TYPE_ANCHOR_TX: u64 = 6;
const TYPE_TX_MERKLE_PROOF: u64 = 8;
const TYPE_ASSET_LEAF: u64 = 10;
const TYPE_INCLUSION_PROOF: u64 = 12;
const TYPE_EXCLUSION_PROOFS: u64 = 13;
const TYPE_SPLIT_ROOT_PROOF: u64 = 15;
const TYPE_META_REVEAL: u64 = 17;
const TYPE_ADDITIONAL_INPUTS: u64 = 19;
const TYPE_CHALLENGE_WITNESS: u64 = 21;
const TYPE_BLOCK_HEIGHT: u64 = 22;
const TYPE_GENESIS_REVEAL: u64 = 23;
const TYPE_GROUP_KEY_REVEAL: u64 = 25;
const TYPE_ALT_LEAVES: u64 = 27;

const KNOWN_PROOF_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_PREV_OUT,
    TYPE_BLOCK_HEADER,
    TYPE_ANCHOR_TX,
    TYPE_TX_MERKLE_PROOF,
    TYPE_ASSET_LEAF,
    TYPE_INCLUSION_PROOF,
    TYPE_EXCLUSION_PROOFS,
    TYPE_SPLIT_ROOT_PROOF,
    TYPE_META_REVEAL,
    TYPE_ADDITIONAL_INPUTS,
    TYPE_CHALLENGE_WITNESS,
    TYPE_BLOCK_HEIGHT,
    TYPE_GENESIS_REVEAL,
    TYPE_GROUP_KEY_REVEAL,
    TYPE_ALT_LEAVES,
];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TapdProofFileSummary {
    pub source_commit: String,
    pub version: u32,
    pub proof_count: usize,
    pub raw_len: usize,
    pub raw_digest: Bytes32,
    pub final_chain_checksum: Bytes32,
    pub proofs: Vec<TapdProofSummary>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TapdProofSummary {
    pub proof_index: usize,
    pub raw_len: usize,
    pub raw_digest: Bytes32,
    pub chain_checksum: Bytes32,
    pub record_count: usize,
    pub transition_version: Option<u32>,
    pub block_height: Option<u32>,
    pub optional_unknown_record_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TapdProofFixtureReport {
    pub proof_file: TapdProofFileSummary,
    pub single_proof: TapdProofSummary,
    pub wrapped_single_proof_file: TapdProofFileSummary,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TapdProofError {
    EmptyHex,
    InvalidHexLength,
    InvalidHexByte(String),
    InvalidMagic {
        expected: &'static str,
        actual: String,
    },
    UnsupportedFileVersion(u32),
    EmptyProofFile,
    Tlv(TlvError),
    Truncated {
        field: &'static str,
        expected: usize,
        remaining: usize,
    },
    TrailingBytes(usize),
    InvalidChecksum {
        proof_index: usize,
        expected: Bytes32,
        actual: Bytes32,
    },
    InvalidRecordLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for TapdProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHex => write!(f, "tapd proof hex contained no bytes"),
            Self::InvalidHexLength => write!(f, "tapd proof hex has odd length"),
            Self::InvalidHexByte(value) => write!(f, "invalid tapd proof hex byte: {value}"),
            Self::InvalidMagic { expected, actual } => {
                write!(
                    f,
                    "invalid tapd proof magic: expected {expected}, got {actual}"
                )
            }
            Self::UnsupportedFileVersion(version) => {
                write!(f, "unsupported tapd proof file version {version}")
            }
            Self::EmptyProofFile => write!(f, "tapd proof file contains no proofs"),
            Self::Tlv(err) => write!(f, "tapd proof TLV error: {err}"),
            Self::Truncated {
                field,
                expected,
                remaining,
            } => write!(
                f,
                "truncated tapd proof {field}: expected {expected} bytes, got {remaining}"
            ),
            Self::TrailingBytes(remaining) => {
                write!(f, "tapd proof file has {remaining} trailing bytes")
            }
            Self::InvalidChecksum {
                proof_index,
                expected,
                actual,
            } => write!(
                f,
                "invalid tapd proof checksum at index {proof_index}: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::InvalidRecordLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "invalid tapd proof record {field} length: expected {expected}, got {actual}"
            ),
        }
    }
}

impl Error for TapdProofError {}

pub fn decode_hex_text(raw: &str) -> Result<Vec<u8>, TapdProofError> {
    let compact = raw
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.is_empty() {
        return Err(TapdProofError::EmptyHex);
    }
    if compact.len() % 2 != 0 {
        return Err(TapdProofError::InvalidHexLength);
    }

    compact
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let value = std::str::from_utf8(chunk)
                .expect("hex input is str")
                .to_owned();
            u8::from_str_radix(&value, 16).map_err(|_| TapdProofError::InvalidHexByte(value))
        })
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(CHARS[(byte >> 4) as usize] as char);
        out.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn decode_tapd_proof_file(bytes: &[u8]) -> Result<TapdProofFileSummary, TapdProofError> {
    let mut cursor = bytes;
    expect_magic(&mut cursor, TAPD_PROOF_FILE_MAGIC, "TAPF")?;
    let version = take_array::<4>(&mut cursor, "proof file version").map(u32::from_be_bytes)?;
    if version != TAPD_PROOF_FILE_VERSION {
        return Err(TapdProofError::UnsupportedFileVersion(version));
    }

    let proof_count = read_big_size(&mut cursor, "proof count")? as usize;
    if proof_count == 0 {
        return Err(TapdProofError::EmptyProofFile);
    }

    let mut previous_checksum = Bytes32::ZERO;
    let mut proofs = Vec::with_capacity(proof_count);
    for proof_index in 0..proof_count {
        let proof_len = read_big_size(&mut cursor, "proof length")? as usize;
        let proof_bytes = take_bytes(&mut cursor, proof_len, "proof bytes")?;
        let actual_checksum = Bytes32(take_array::<32>(&mut cursor, "proof checksum")?);
        let expected_checksum = chained_checksum(previous_checksum, proof_bytes);
        if actual_checksum != expected_checksum {
            return Err(TapdProofError::InvalidChecksum {
                proof_index,
                expected: expected_checksum,
                actual: actual_checksum,
            });
        }

        proofs.push(decode_single_proof_at(
            proof_bytes,
            proof_index,
            actual_checksum,
        )?);
        previous_checksum = actual_checksum;
    }

    if !cursor.is_empty() {
        return Err(TapdProofError::TrailingBytes(cursor.len()));
    }

    Ok(TapdProofFileSummary {
        source_commit: LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT.to_owned(),
        version,
        proof_count,
        raw_len: bytes.len(),
        raw_digest: digest(bytes),
        final_chain_checksum: previous_checksum,
        proofs,
    })
}

pub fn decode_tapd_single_proof(bytes: &[u8]) -> Result<TapdProofSummary, TapdProofError> {
    decode_single_proof_at(bytes, 0, chained_checksum(Bytes32::ZERO, bytes))
}

pub fn wrap_single_proof_as_tapd_file(bytes: &[u8]) -> Result<Vec<u8>, TapdProofError> {
    decode_tapd_single_proof(bytes)?;

    let checksum = chained_checksum(Bytes32::ZERO, bytes);
    let mut out = Vec::with_capacity(TAPD_PROOF_FILE_MAGIC.len() + 4 + 9 + bytes.len() + 32);
    out.extend_from_slice(TAPD_PROOF_FILE_MAGIC);
    out.extend_from_slice(&TAPD_PROOF_FILE_VERSION.to_be_bytes());
    encode_big_size(1, &mut out);
    encode_big_size(bytes.len() as u64, &mut out);
    out.extend_from_slice(bytes);
    out.extend_from_slice(&checksum.0);
    Ok(out)
}

pub fn decode_fixture_hex(
    proof_file_hex: &str,
    single_proof_hex: &str,
) -> Result<TapdProofFixtureReport, TapdProofError> {
    let proof_file = decode_hex_text(proof_file_hex)?;
    let single_proof = decode_hex_text(single_proof_hex)?;
    let wrapped = wrap_single_proof_as_tapd_file(&single_proof)?;

    Ok(TapdProofFixtureReport {
        proof_file: decode_tapd_proof_file(&proof_file)?,
        single_proof: decode_tapd_single_proof(&single_proof)?,
        wrapped_single_proof_file: decode_tapd_proof_file(&wrapped)?,
    })
}

fn decode_single_proof_at(
    bytes: &[u8],
    proof_index: usize,
    chain_checksum: Bytes32,
) -> Result<TapdProofSummary, TapdProofError> {
    let mut cursor = bytes;
    expect_magic(&mut cursor, TAPD_PROOF_MAGIC, "TAPP")?;
    let records = decode_stream(cursor).map_err(TapdProofError::Tlv)?;
    reject_unknown_required(&records, KNOWN_PROOF_TYPES).map_err(TapdProofError::Tlv)?;

    Ok(TapdProofSummary {
        proof_index,
        raw_len: bytes.len(),
        raw_digest: digest(bytes),
        chain_checksum,
        record_count: records.len(),
        transition_version: parse_optional_u32(&records, TYPE_VERSION, "version")?,
        block_height: parse_optional_u32(&records, TYPE_BLOCK_HEIGHT, "block_height")?,
        optional_unknown_record_count: records
            .iter()
            .filter(|record| {
                record.type_id % 2 == 1 && !KNOWN_PROOF_TYPES.contains(&record.type_id)
            })
            .count(),
    })
}

fn expect_magic(
    cursor: &mut &[u8],
    expected: &[u8; 4],
    expected_name: &'static str,
) -> Result<(), TapdProofError> {
    let actual = take_array::<4>(cursor, "magic")?;
    if &actual != expected {
        return Err(TapdProofError::InvalidMagic {
            expected: expected_name,
            actual: String::from_utf8_lossy(&actual).into_owned(),
        });
    }

    Ok(())
}

fn parse_optional_u32(
    records: &[TlvRecord],
    type_id: u64,
    field: &'static str,
) -> Result<Option<u32>, TapdProofError> {
    let Some(record) = records.iter().find(|record| record.type_id == type_id) else {
        return Ok(None);
    };
    if record.value.len() != 4 {
        return Err(TapdProofError::InvalidRecordLength {
            field,
            expected: 4,
            actual: record.value.len(),
        });
    }

    Ok(Some(u32::from_be_bytes(
        record.value.as_slice().try_into().expect("length checked"),
    )))
}

fn read_big_size(cursor: &mut &[u8], field: &'static str) -> Result<u64, TapdProofError> {
    decode_big_size(cursor).map_err(|err| match err {
        TlvError::TruncatedBigSize => TapdProofError::Truncated {
            field,
            expected: 1,
            remaining: 0,
        },
        other => TapdProofError::Tlv(other),
    })
}

fn take_array<const N: usize>(
    cursor: &mut &[u8],
    field: &'static str,
) -> Result<[u8; N], TapdProofError> {
    if cursor.len() < N {
        return Err(TapdProofError::Truncated {
            field,
            expected: N,
            remaining: cursor.len(),
        });
    }

    let (head, tail) = cursor.split_at(N);
    *cursor = tail;
    Ok(head.try_into().expect("slice length is checked"))
}

fn take_bytes<'a>(
    cursor: &mut &'a [u8],
    len: usize,
    field: &'static str,
) -> Result<&'a [u8], TapdProofError> {
    if cursor.len() < len {
        return Err(TapdProofError::Truncated {
            field,
            expected: len,
            remaining: cursor.len(),
        });
    }

    let (head, tail) = cursor.split_at(len);
    *cursor = tail;
    Ok(head)
}

fn chained_checksum(previous_checksum: Bytes32, proof: &[u8]) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(previous_checksum.0);
    hasher.update(proof);
    Bytes32(hasher.finalize().into())
}

fn digest(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tlv::TlvRecord;

    #[test]
    fn wraps_single_proof_with_valid_chain_checksum() {
        let proof = fake_single_proof();
        let file = wrap_single_proof_as_tapd_file(&proof).expect("proof wraps");
        let summary = decode_tapd_proof_file(&file).expect("proof file decodes");

        assert_eq!(summary.version, TAPD_PROOF_FILE_VERSION);
        assert_eq!(summary.proof_count, 1);
        assert_eq!(summary.proofs[0].raw_digest, digest(&proof));
        assert_eq!(
            summary.final_chain_checksum,
            summary.proofs[0].chain_checksum
        );
    }

    #[test]
    fn proof_file_rejects_bad_checksum_and_unknown_required_record() {
        let proof = fake_single_proof();
        let mut file = wrap_single_proof_as_tapd_file(&proof).expect("proof wraps");
        let last = file.last_mut().expect("file has checksum");
        *last ^= 1;
        assert!(matches!(
            decode_tapd_proof_file(&file),
            Err(TapdProofError::InvalidChecksum { .. })
        ));

        let records = vec![
            TlvRecord::new(TYPE_VERSION, 0_u32.to_be_bytes()),
            TlvRecord::new(100, []),
        ];
        let mut bad = Vec::from(TAPD_PROOF_MAGIC.as_slice());
        bad.extend_from_slice(&crate::tlv::encode_stream(&records).expect("records encode"));
        assert!(matches!(
            decode_tapd_single_proof(&bad),
            Err(TapdProofError::Tlv(TlvError::UnknownRequiredType(100)))
        ));
    }

    #[test]
    fn hex_text_accepts_whitespace_and_rejects_bad_bytes() {
        assert_eq!(
            decode_hex_text("54 41\n50 50").expect("hex decodes"),
            b"TAPP"
        );
        assert!(matches!(
            decode_hex_text("zz"),
            Err(TapdProofError::InvalidHexByte(value)) if value == "zz"
        ));
    }

    fn fake_single_proof() -> Vec<u8> {
        let records = vec![
            TlvRecord::new(TYPE_VERSION, 0_u32.to_be_bytes()),
            TlvRecord::new(TYPE_BLOCK_HEIGHT, 144_u32.to_be_bytes()),
        ];
        let mut proof = Vec::from(TAPD_PROOF_MAGIC.as_slice());
        proof.extend_from_slice(&crate::tlv::encode_stream(&records).expect("records encode"));
        proof
    }
}
