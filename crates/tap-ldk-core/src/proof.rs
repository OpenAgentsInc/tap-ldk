use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use crate::{
    asset::{
        AssetAmount, AssetError, AssetLeaf, AssetType, Bytes32, CompressedKey, RootHashSum,
        derive_hash_sum_root,
    },
    tapd_proof::TapdProofFileSummary,
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
const TYPE_NETWORK: u64 = 19;
const TYPE_ASSET_TYPE: u64 = 21;

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
    TYPE_NETWORK,
    TYPE_ASSET_TYPE,
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
    pub network: ProofNetwork,
    pub asset_type: AssetType,
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
            TlvRecord::new(TYPE_NETWORK, self.network.as_str().as_bytes()),
            TlvRecord::new(TYPE_ASSET_TYPE, [self.asset_type.as_u8()]),
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
        let network = ProofNetwork::from_str(&parse_string(required(&fields, TYPE_NETWORK)?)?)?;
        let asset_type = parse_asset_type(required(&fields, TYPE_ASSET_TYPE)?)?;

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
            network,
            asset_type,
        })
    }

    pub fn verify_bounded_anchor(&self) -> Result<(), ProofError> {
        self.verify_semantic_ancestry(&ProofValidationContext::default())
            .map(|_| ())
    }

    pub fn verify_semantic_ancestry(
        &self,
        context: &ProofValidationContext,
    ) -> Result<ProofValidationReport, ProofError> {
        if self.version != 0 {
            return Err(ProofError::UnsupportedVersion(self.version));
        }

        if self.verification_scope != VerificationScope::SemanticAncestry {
            return Err(ProofError::UnsupportedScope(
                self.verification_scope.as_str().to_owned(),
            ));
        }

        if self.network != context.expected_network {
            return Err(ProofError::WrongNetwork {
                expected: context.expected_network,
                actual: self.network,
            });
        }

        if self.asset_type != context.expected_asset_type {
            return Err(ProofError::WrongAssetType {
                expected: context.expected_asset_type,
                actual: self.asset_type,
            });
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

        if self.tap_asset_root.hash == Bytes32::ZERO {
            return Err(ProofError::BrokenAncestry("zero tap asset root hash"));
        }

        let genesis = parse_outpoint(&self.genesis_outpoint, "genesis_outpoint")?;
        let anchor = parse_outpoint(&self.anchor_outpoint, "anchor_outpoint")?;
        if genesis == anchor {
            return Err(ProofError::BrokenAncestry(
                "genesis and anchor outpoints must differ",
            ));
        }

        let derived_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: self.asset_id,
            script_key: self.script_key,
            amount: self.amount,
        }])
        .map_err(ProofError::Asset)?;
        if self.tap_asset_root != derived_root {
            return Err(ProofError::CommitmentRootMismatch {
                expected_hash: derived_root.hash,
                actual_hash: self.tap_asset_root.hash,
                expected_sum: derived_root.sum.value(),
                actual_sum: self.tap_asset_root.sum.value(),
            });
        }

        if let Some(expected) = context.expected_asset_id {
            if self.asset_id != expected {
                return Err(ProofError::WrongAsset {
                    expected,
                    actual: self.asset_id,
                });
            }
        }
        if let Some(expected) = context.expected_amount {
            if self.amount != expected {
                return Err(ProofError::WrongAmount {
                    expected: expected.value(),
                    actual: self.amount.value(),
                });
            }
        }
        if let Some(expected) = context.expected_script_key {
            if self.script_key != expected {
                return Err(ProofError::WrongOwner {
                    expected,
                    actual: self.script_key,
                });
            }
        }
        if let Some(expected) = context.expected_genesis_outpoint.as_deref() {
            if self.genesis_outpoint != expected {
                return Err(ProofError::BrokenAncestry("genesis outpoint mismatch"));
            }
        }
        if let Some(expected) = context.expected_anchor_outpoint.as_deref() {
            if self.anchor_outpoint != expected {
                return Err(ProofError::BrokenAncestry("anchor outpoint mismatch"));
            }
        }
        if let Some(stale) = context.stale_anchor_outpoint.as_deref() {
            if self.anchor_outpoint == stale {
                return Err(ProofError::StaleProof {
                    anchor_outpoint: self.anchor_outpoint.clone(),
                });
            }
        }

        if context.require_tapd_ancestry {
            let summary = context
                .tapd_proof_summary
                .as_ref()
                .ok_or(ProofError::MissingTapdProofSummary)?;
            self.validate_tapd_ancestry(summary, context)?;
        }

        Ok(ProofValidationReport {
            validation_scope: self.verification_scope,
            network: self.network,
            asset_type: self.asset_type,
            asset_id: self.asset_id,
            amount: self.amount,
            genesis_outpoint: self.genesis_outpoint.clone(),
            anchor_outpoint: self.anchor_outpoint.clone(),
            script_key: self.script_key,
            tap_asset_root: self.tap_asset_root,
            tapd_proof_count: context
                .tapd_proof_summary
                .as_ref()
                .map(|summary| summary.proof_count),
            tapd_proof_file_digest: context
                .tapd_proof_summary
                .as_ref()
                .map(|summary| summary.raw_digest),
        })
    }

    fn validate_tapd_ancestry(
        &self,
        summary: &TapdProofFileSummary,
        context: &ProofValidationContext,
    ) -> Result<(), ProofError> {
        if let Some(expected_digest) = context.expected_tapd_proof_file_digest {
            if summary.raw_digest != expected_digest {
                return Err(ProofError::StaleTapdProof {
                    expected: expected_digest,
                    actual: summary.raw_digest,
                });
            }
        }

        for proof in &summary.proofs {
            if !matches!(proof.transition_version, Some(0) | Some(1)) {
                return Err(ProofError::BrokenAncestry(
                    "unsupported tapd transition version",
                ));
            }
            if !(proof.has_prev_out
                && proof.has_block_header
                && proof.has_anchor_tx
                && proof.has_tx_merkle_proof
                && proof.has_asset_leaf
                && proof.has_inclusion_proof)
            {
                return Err(ProofError::BrokenAncestry(
                    "tapd proof missing required ancestry records",
                ));
            }
        }

        let leaf = summary
            .latest_asset_leaf()
            .ok_or(ProofError::BrokenAncestry("tapd proof missing asset leaf"))?;
        if leaf.asset_id != self.asset_id {
            return Err(ProofError::WrongAsset {
                expected: self.asset_id,
                actual: leaf.asset_id,
            });
        }
        if leaf.asset_type != self.asset_type.as_u8() {
            return Err(ProofError::WrongAssetType {
                expected: self.asset_type,
                actual: AssetType::from_u8(leaf.asset_type)
                    .map_err(|_| ProofError::BrokenAncestry("unsupported tapd asset type"))?,
            });
        }
        if leaf.amount != self.amount.value() {
            return Err(ProofError::WrongAmount {
                expected: self.amount.value(),
                actual: leaf.amount,
            });
        }
        if leaf.script_key != self.script_key {
            return Err(ProofError::WrongOwner {
                expected: self.script_key,
                actual: leaf.script_key,
            });
        }
        if leaf.genesis.first_prev_out != self.genesis_outpoint {
            return Err(ProofError::BrokenAncestry("tapd genesis outpoint mismatch"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VerificationScope {
    BoundedAnchorOnly,
    SemanticAncestry,
}

impl VerificationScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BoundedAnchorOnly => "bounded-anchor-only",
            Self::SemanticAncestry => "semantic-ancestry",
        }
    }
}

impl FromStr for VerificationScope {
    type Err = ProofError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bounded-anchor-only" => Ok(Self::BoundedAnchorOnly),
            "semantic-ancestry" => Ok(Self::SemanticAncestry),
            other => Err(ProofError::UnsupportedScope(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProofNetwork {
    Regtest,
}

impl ProofNetwork {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regtest => "regtest",
        }
    }
}

impl FromStr for ProofNetwork {
    type Err = ProofError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "regtest" => Ok(Self::Regtest),
            other => Err(ProofError::UnsupportedNetwork(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofValidationContext {
    pub expected_network: ProofNetwork,
    pub expected_asset_type: AssetType,
    pub expected_asset_id: Option<Bytes32>,
    pub expected_amount: Option<AssetAmount>,
    pub expected_script_key: Option<CompressedKey>,
    pub expected_genesis_outpoint: Option<String>,
    pub expected_anchor_outpoint: Option<String>,
    pub stale_anchor_outpoint: Option<String>,
    pub require_tapd_ancestry: bool,
    pub expected_tapd_proof_file_digest: Option<Bytes32>,
    pub tapd_proof_summary: Option<TapdProofFileSummary>,
}

impl Default for ProofValidationContext {
    fn default() -> Self {
        Self {
            expected_network: ProofNetwork::Regtest,
            expected_asset_type: AssetType::Normal,
            expected_asset_id: None,
            expected_amount: None,
            expected_script_key: None,
            expected_genesis_outpoint: None,
            expected_anchor_outpoint: None,
            stale_anchor_outpoint: None,
            require_tapd_ancestry: false,
            expected_tapd_proof_file_digest: None,
            tapd_proof_summary: None,
        }
    }
}

impl ProofValidationContext {
    pub fn for_asset(asset_id: Bytes32) -> Self {
        Self {
            expected_asset_id: Some(asset_id),
            ..Self::default()
        }
    }

    pub fn for_close(
        asset_id: Bytes32,
        amount: AssetAmount,
        script_key: CompressedKey,
        genesis_outpoint: String,
        anchor_outpoint: String,
    ) -> Self {
        Self {
            expected_asset_id: Some(asset_id),
            expected_amount: Some(amount),
            expected_script_key: Some(script_key),
            expected_genesis_outpoint: Some(genesis_outpoint),
            expected_anchor_outpoint: Some(anchor_outpoint),
            ..Self::default()
        }
    }

    pub fn for_tapd_import(summary: TapdProofFileSummary) -> Self {
        Self {
            require_tapd_ancestry: true,
            expected_tapd_proof_file_digest: Some(summary.raw_digest),
            tapd_proof_summary: Some(summary),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofValidationReport {
    pub validation_scope: VerificationScope,
    pub network: ProofNetwork,
    pub asset_type: AssetType,
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub script_key: CompressedKey,
    pub tap_asset_root: RootHashSum,
    pub tapd_proof_count: Option<usize>,
    pub tapd_proof_file_digest: Option<Bytes32>,
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
    UnsupportedNetwork(String),
    ZeroAssetId,
    ZeroAmount,
    RootSumMismatch {
        amount: u64,
        root_sum: u64,
    },
    MalformedOutpoint(&'static str),
    WrongNetwork {
        expected: ProofNetwork,
        actual: ProofNetwork,
    },
    WrongAssetType {
        expected: AssetType,
        actual: AssetType,
    },
    WrongAsset {
        expected: Bytes32,
        actual: Bytes32,
    },
    WrongOwner {
        expected: CompressedKey,
        actual: CompressedKey,
    },
    WrongAmount {
        expected: u64,
        actual: u64,
    },
    CommitmentRootMismatch {
        expected_hash: Bytes32,
        actual_hash: Bytes32,
        expected_sum: u64,
        actual_sum: u64,
    },
    BrokenAncestry(&'static str),
    StaleProof {
        anchor_outpoint: String,
    },
    MissingTapdProofSummary,
    StaleTapdProof {
        expected: Bytes32,
        actual: Bytes32,
    },
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
            Self::UnsupportedNetwork(network) => {
                write!(f, "unsupported proof network {network}")
            }
            Self::ZeroAssetId => write!(f, "proof asset id cannot be zero"),
            Self::ZeroAmount => write!(f, "proof amount cannot be zero"),
            Self::RootSumMismatch { amount, root_sum } => {
                write!(
                    f,
                    "proof root sum mismatch: amount {amount}, root sum {root_sum}"
                )
            }
            Self::MalformedOutpoint(field) => write!(f, "malformed proof outpoint: {field}"),
            Self::WrongNetwork { expected, actual } => write!(
                f,
                "proof network mismatch: expected {}, got {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::WrongAssetType { expected, actual } => write!(
                f,
                "proof asset type mismatch: expected {}, got {}",
                expected.as_u8(),
                actual.as_u8()
            ),
            Self::WrongAsset { expected, actual } => write!(
                f,
                "proof asset mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::WrongOwner { expected, actual } => write!(
                f,
                "proof owner mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::WrongAmount { expected, actual } => {
                write!(
                    f,
                    "proof amount mismatch: expected {expected}, got {actual}"
                )
            }
            Self::CommitmentRootMismatch {
                expected_hash,
                actual_hash,
                expected_sum,
                actual_sum,
            } => write!(
                f,
                "proof commitment root mismatch: expected {}:{expected_sum}, got {}:{actual_sum}",
                expected_hash.to_hex(),
                actual_hash.to_hex()
            ),
            Self::BrokenAncestry(reason) => write!(f, "broken proof ancestry: {reason}"),
            Self::StaleProof { anchor_outpoint } => {
                write!(f, "stale proof anchor outpoint: {anchor_outpoint}")
            }
            Self::MissingTapdProofSummary => write!(f, "missing tapd proof summary"),
            Self::StaleTapdProof { expected, actual } => write!(
                f,
                "stale tapd proof digest: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
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

fn parse_asset_type(bytes: &[u8]) -> Result<AssetType, ProofError> {
    let value = parse_u8(bytes, "asset_type")?;
    AssetType::from_u8(value).map_err(ProofError::Asset)
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct ParsedOutpoint {
    txid: String,
    vout: u32,
}

fn parse_outpoint(value: &str, field: &'static str) -> Result<ParsedOutpoint, ProofError> {
    let mut parts = value.split(':');
    let Some(txid) = parts.next() else {
        return Err(ProofError::MalformedOutpoint(field));
    };
    let Some(vout) = parts.next() else {
        return Err(ProofError::MalformedOutpoint(field));
    };
    if parts.next().is_some()
        || txid.len() != 64
        || txid.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || txid.bytes().all(|byte| byte == b'0')
        || vout.is_empty()
        || vout.starts_with('+')
    {
        return Err(ProofError::MalformedOutpoint(field));
    }
    let vout = vout
        .parse::<u32>()
        .map_err(|_| ProofError::MalformedOutpoint(field))?;

    Ok(ParsedOutpoint {
        txid: txid.to_ascii_lowercase(),
        vout,
    })
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
        let asset_id =
            Bytes32::from_str("dbe4d6f07f3751421793d77478b1da71c1a1382ea5766d4f9237a20351a862d8")
                .expect("asset id parses");
        let script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("script key parses");
        let amount = AssetAmount::new(1000000);
        let tap_asset_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id,
            script_key,
            amount,
        }])
        .expect("root derives");

        ProofFile {
            version: 0,
            asset_id,
            genesis_outpoint: "9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0"
                .to_owned(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            amount,
            script_key,
            tap_asset_root,
            verification_scope: VerificationScope::SemanticAncestry,
            network: ProofNetwork::Regtest,
            asset_type: AssetType::Normal,
        }
    }

    #[test]
    fn proof_round_trips_and_verifies() {
        let proof = proof();
        let encoded = proof.encode().expect("proof encodes");
        let decoded = ProofFile::decode(&encoded).expect("proof decodes");

        assert_eq!(decoded, proof);
        decoded
            .verify_semantic_ancestry(&ProofValidationContext::default())
            .expect("proof verifies");
    }

    #[test]
    fn root_sum_mismatch_fails_closed() {
        let mut proof = proof();
        proof.tap_asset_root.sum = AssetAmount::new(999999);

        assert_eq!(
            proof
                .verify_semantic_ancestry(&ProofValidationContext::default())
                .map(|_| ()),
            Err(ProofError::RootSumMismatch {
                amount: 1000000,
                root_sum: 999999
            })
        );
    }

    #[test]
    fn semantic_context_rejects_wrong_fields_and_stale_anchor() {
        let proof = proof();

        let mut wrong_asset = ProofValidationContext::default();
        wrong_asset.expected_asset_id = Some(Bytes32([7; 32]));
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_asset),
            Err(ProofError::WrongAsset { .. })
        ));

        let mut wrong_amount = ProofValidationContext::default();
        wrong_amount.expected_amount = Some(AssetAmount::new(999));
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_amount),
            Err(ProofError::WrongAmount { .. })
        ));

        let mut wrong_owner = ProofValidationContext::default();
        wrong_owner.expected_script_key = Some(
            CompressedKey::from_str(
                "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
            )
            .expect("script key parses"),
        );
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_owner),
            Err(ProofError::WrongOwner { .. })
        ));

        let mut stale = ProofValidationContext::default();
        stale.stale_anchor_outpoint = Some(proof.anchor_outpoint.clone());
        assert!(matches!(
            proof.verify_semantic_ancestry(&stale),
            Err(ProofError::StaleProof { .. })
        ));

        let mut wrong_type = proof.clone();
        wrong_type.asset_type = AssetType::Collectible;
        assert!(matches!(
            wrong_type.verify_semantic_ancestry(&ProofValidationContext::default()),
            Err(ProofError::WrongAssetType { .. })
        ));
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
            TlvRecord::new(TYPE_NETWORK, proof.network.as_str().as_bytes()),
            TlvRecord::new(TYPE_ASSET_TYPE, [proof.asset_type.as_u8()]),
        ];
        let encoded = encode_stream(&records).expect("proof records encode");

        assert_eq!(
            ProofFile::decode(&encoded),
            Err(ProofError::UnsupportedScope(
                "full-history-required".to_owned()
            ))
        );
    }

    #[test]
    fn unsupported_network_fails_decode() {
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
            TlvRecord::new(
                TYPE_VERIFICATION_SCOPE,
                proof.verification_scope.as_str().as_bytes(),
            ),
            TlvRecord::new(TYPE_NETWORK, b"mainnet"),
            TlvRecord::new(TYPE_ASSET_TYPE, [proof.asset_type.as_u8()]),
        ];
        let encoded = encode_stream(&records).expect("proof records encode");

        assert_eq!(
            ProofFile::decode(&encoded),
            Err(ProofError::UnsupportedNetwork("mainnet".to_owned()))
        );
    }
}
