use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use crate::{
    asset::{AssetAmount, AssetError, Bytes32, CompressedKey, RootHashSum},
    tlv::{TlvError, TlvRecord, decode_stream, encode_stream, reject_unknown_required},
};

const TYPE_VERSION: u64 = 1;
const TYPE_ASSET_ID: u64 = 3;
const TYPE_GENESIS_OUTPOINT: u64 = 5;
const TYPE_ANCHOR_OUTPOINT: u64 = 7;
const TYPE_AMOUNT: u64 = 9;
const TYPE_SCRIPT_KEY: u64 = 11;
const TYPE_ROOT_HASH: u64 = 13;
const TYPE_ROOT_SUM: u64 = 15;
const TYPE_VERIFICATION_SCOPE: u64 = 17;

const KNOWN_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_ASSET_ID,
    TYPE_GENESIS_OUTPOINT,
    TYPE_ANCHOR_OUTPOINT,
    TYPE_AMOUNT,
    TYPE_SCRIPT_KEY,
    TYPE_ROOT_HASH,
    TYPE_ROOT_SUM,
    TYPE_VERIFICATION_SCOPE,
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofFile {
    pub version: u8,
    pub asset_id: Bytes32,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub tap_asset_root: RootHashSum,
    pub verification_scope: VerificationScope,
}

impl ProofFile {
    pub fn encode(&self) -> Result<Vec<u8>, ProofError> {
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ASSET_ID, self.asset_id.0),
            TlvRecord::new(TYPE_GENESIS_OUTPOINT, self.genesis_outpoint.as_bytes()),
            TlvRecord::new(TYPE_ANCHOR_OUTPOINT, self.anchor_outpoint.as_bytes()),
            TlvRecord::new(TYPE_AMOUNT, self.amount.value().to_be_bytes()),
            TlvRecord::new(TYPE_SCRIPT_KEY, self.script_key.0),
            TlvRecord::new(TYPE_ROOT_HASH, self.tap_asset_root.hash.0),
            TlvRecord::new(TYPE_ROOT_SUM, self.tap_asset_root.sum.value().to_be_bytes()),
            TlvRecord::new(
                TYPE_VERIFICATION_SCOPE,
                self.verification_scope.as_str().as_bytes(),
            ),
        ];

        encode_stream(&records).map_err(ProofError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProofError> {
        let records = decode_stream(bytes).map_err(ProofError::Tlv)?;
        reject_unknown_required(&records, KNOWN_TYPES).map_err(ProofError::Tlv)?;

        let mut fields = BTreeMap::new();
        for record in records {
            fields.insert(record.type_id, record.value);
        }

        let version = parse_u8(required(&fields, TYPE_VERSION)?, "version")?;
        let asset_id = parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?;
        let genesis_outpoint = parse_string(required(&fields, TYPE_GENESIS_OUTPOINT)?)?;
        let anchor_outpoint = parse_string(required(&fields, TYPE_ANCHOR_OUTPOINT)?)?;
        let amount = AssetAmount::new(parse_u64(required(&fields, TYPE_AMOUNT)?, "amount")?);
        let script_key = parse_compressed_key(required(&fields, TYPE_SCRIPT_KEY)?)?;
        let root_hash = parse_bytes32(required(&fields, TYPE_ROOT_HASH)?)?;
        let root_sum = AssetAmount::new(parse_u64(required(&fields, TYPE_ROOT_SUM)?, "root_sum")?);
        let verification_scope = VerificationScope::from_str(&parse_string(required(
            &fields,
            TYPE_VERIFICATION_SCOPE,
        )?)?)?;

        Ok(Self {
            version,
            asset_id,
            genesis_outpoint,
            anchor_outpoint,
            amount,
            script_key,
            tap_asset_root: RootHashSum {
                hash: root_hash,
                sum: root_sum,
            },
            verification_scope,
        })
    }

    pub fn verify_bounded_anchor(&self) -> Result<(), ProofError> {
        if self.version != 0 {
            return Err(ProofError::UnsupportedVersion(self.version));
        }

        if self.asset_id == Bytes32::ZERO {
            return Err(ProofError::ZeroAssetId);
        }

        if self.amount == AssetAmount::ZERO {
            return Err(ProofError::ZeroAmount);
        }

        if self.tap_asset_root.sum != self.amount {
            return Err(ProofError::RootSumMismatch {
                amount: self.amount.value(),
                root_sum: self.tap_asset_root.sum.value(),
            });
        }

        if !self.genesis_outpoint.contains(':') {
            return Err(ProofError::MalformedOutpoint("genesis_outpoint"));
        }

        if !self.anchor_outpoint.contains(':') {
            return Err(ProofError::MalformedOutpoint("anchor_outpoint"));
        }

        if self.verification_scope != VerificationScope::BoundedAnchorOnly {
            return Err(ProofError::UnsupportedScope(
                self.verification_scope.as_str().to_owned(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VerificationScope {
    BoundedAnchorOnly,
}

impl VerificationScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BoundedAnchorOnly => "bounded-anchor-only",
        }
    }
}

impl FromStr for VerificationScope {
    type Err = ProofError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bounded-anchor-only" => Ok(Self::BoundedAnchorOnly),
            other => Err(ProofError::UnsupportedScope(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProofError {
    Tlv(TlvError),
    Asset(AssetError),
    MissingField(u64),
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidUtf8(&'static str),
    UnsupportedVersion(u8),
    UnsupportedScope(String),
    ZeroAssetId,
    ZeroAmount,
    RootSumMismatch {
        amount: u64,
        root_sum: u64,
    },
    MalformedOutpoint(&'static str),
}

impl fmt::Display for ProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tlv(err) => write!(f, "proof TLV error: {err}"),
            Self::Asset(err) => write!(f, "proof asset error: {err}"),
            Self::MissingField(field) => write!(f, "missing proof field {field}"),
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid proof field {field} length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidUtf8(field) => write!(f, "proof field {field} is not UTF-8"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported proof version {version}")
            }
            Self::UnsupportedScope(scope) => write!(f, "unsupported proof scope {scope}"),
            Self::ZeroAssetId => write!(f, "proof asset id cannot be zero"),
            Self::ZeroAmount => write!(f, "proof amount cannot be zero"),
            Self::RootSumMismatch { amount, root_sum } => {
                write!(
                    f,
                    "proof root sum mismatch: amount {amount}, root sum {root_sum}"
                )
            }
            Self::MalformedOutpoint(field) => write!(f, "malformed proof outpoint: {field}"),
        }
    }
}

impl Error for ProofError {}

fn required(fields: &BTreeMap<u64, Vec<u8>>, field: u64) -> Result<&[u8], ProofError> {
    fields
        .get(&field)
        .map(Vec::as_slice)
        .ok_or(ProofError::MissingField(field))
}

fn parse_u8(bytes: &[u8], field: &'static str) -> Result<u8, ProofError> {
    if bytes.len() != 1 {
        return Err(ProofError::InvalidFieldLength {
            field,
            expected: 1,
            actual: bytes.len(),
        });
    }

    Ok(bytes[0])
}

fn parse_u64(bytes: &[u8], field: &'static str) -> Result<u64, ProofError> {
    let actual = bytes.len();
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| ProofError::InvalidFieldLength {
            field,
            expected: 8,
            actual,
        })?;

    Ok(u64::from_be_bytes(bytes))
}

fn parse_string(bytes: &[u8]) -> Result<String, ProofError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| ProofError::InvalidUtf8("string"))
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, ProofError> {
    let actual = bytes.len();
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ProofError::InvalidFieldLength {
            field: "bytes32",
            expected: 32,
            actual,
        })?;
    Ok(Bytes32(bytes))
}

fn parse_compressed_key(bytes: &[u8]) -> Result<CompressedKey, ProofError> {
    CompressedKey::from_str(&encode_hex(bytes)).map_err(ProofError::Asset)
}

fn encode_hex(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(CHARS[(byte >> 4) as usize] as char);
        out.push(CHARS[(byte & 0x0f) as usize] as char);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> ProofFile {
        ProofFile {
            version: 0,
            asset_id: Bytes32::from_str(
                "dbe4d6f07f3751421793d77478b1da71c1a1382ea5766d4f9237a20351a862d8",
            )
            .expect("asset id parses"),
            genesis_outpoint: "9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0"
                .to_owned(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            amount: AssetAmount::new(1000000),
            script_key: CompressedKey::from_str(
                "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
            )
            .expect("script key parses"),
            tap_asset_root: RootHashSum {
                hash: Bytes32::from_str(
                    "3ed3ea50146d815594b28dc6bbff67dadf078ee245c10a1a06faeb5e8ff9c3c2",
                )
                .expect("root hash parses"),
                sum: AssetAmount::new(1000000),
            },
            verification_scope: VerificationScope::BoundedAnchorOnly,
        }
    }

    #[test]
    fn proof_round_trips_and_verifies() {
        let proof = proof();
        let encoded = proof.encode().expect("proof encodes");
        let decoded = ProofFile::decode(&encoded).expect("proof decodes");

        assert_eq!(decoded, proof);
        decoded.verify_bounded_anchor().expect("proof verifies");
    }

    #[test]
    fn root_sum_mismatch_fails_closed() {
        let mut proof = proof();
        proof.tap_asset_root.sum = AssetAmount::new(999999);

        assert_eq!(
            proof.verify_bounded_anchor(),
            Err(ProofError::RootSumMismatch {
                amount: 1000000,
                root_sum: 999999
            })
        );
    }

    #[test]
    fn unsupported_scope_fails_decode() {
        let proof = proof();
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [proof.version]),
            TlvRecord::new(TYPE_ASSET_ID, proof.asset_id.0),
            TlvRecord::new(TYPE_GENESIS_OUTPOINT, proof.genesis_outpoint.as_bytes()),
            TlvRecord::new(TYPE_ANCHOR_OUTPOINT, proof.anchor_outpoint.as_bytes()),
            TlvRecord::new(TYPE_AMOUNT, proof.amount.value().to_be_bytes()),
            TlvRecord::new(TYPE_SCRIPT_KEY, proof.script_key.0),
            TlvRecord::new(TYPE_ROOT_HASH, proof.tap_asset_root.hash.0),
            TlvRecord::new(
                TYPE_ROOT_SUM,
                proof.tap_asset_root.sum.value().to_be_bytes(),
            ),
            TlvRecord::new(TYPE_VERIFICATION_SCOPE, b"full-history-required"),
        ];
        let encoded = encode_stream(&records).expect("proof records encode");

        assert_eq!(
            ProofFile::decode(&encoded),
            Err(ProofError::UnsupportedScope(
                "full-history-required".to_owned()
            ))
        );
    }
}
