use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{Bytes32, CompressedKey},
    tlv::{TlvError, TlvRecord, decode_big_size, decode_stream},
};

pub const TYPE_OPEN_CHANNEL_FUNDED_ASSETS: u64 = 0;
pub const TYPE_OPEN_CHANNEL_DECIMAL_DISPLAY: u64 = 1;
pub const TYPE_OPEN_CHANNEL_GROUP_KEY: u64 = 2;

pub const TYPE_HTLC_AMOUNTS: u64 = 65_536;
pub const TYPE_HTLC_RFQ_ID: u64 = 65_538;
pub const TYPE_HTLC_AVAILABLE_RFQ_IDS: u64 = 65_540;
pub const TYPE_HTLC_NOOP_ADD: u64 = 65_544;

pub const TYPE_COMMITMENT_LOCAL_ASSETS: u64 = 0;
pub const TYPE_COMMITMENT_REMOTE_ASSETS: u64 = 1;
pub const TYPE_COMMITMENT_OUTGOING_HTLCS: u64 = 2;
pub const TYPE_COMMITMENT_INCOMING_HTLCS: u64 = 3;
pub const TYPE_COMMITMENT_AUX_LEAVES: u64 = 4;
pub const TYPE_COMMITMENT_STXO: u64 = 5;

pub const LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT: &str = "743db21da57b5fdecf5daca9a925f0261ca94e40";

const MAX_OUTPUTS: u64 = 2048;
const MAX_HTLCS: u64 = 483;
const MAX_RFQ_IDS: u64 = 2048;
const MAX_SCRIPT_LEN: u64 = 65_535;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsBlobFixtureReport {
    pub source_commit: String,
    pub funding: LightningLabsFundingBlob,
    pub htlc: LightningLabsHtlcBlob,
    pub commitment: LightningLabsCommitmentBlob,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFundingBlob {
    pub raw_len: usize,
    pub raw_digest: Bytes32,
    pub decimal_display: u8,
    pub group_key: Option<CompressedKey>,
    pub funded_assets: AssetOutputListSummary,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsHtlcBlob {
    pub raw_len: usize,
    pub raw_digest: Bytes32,
    pub balances: AssetBalanceListSummary,
    pub rfq_id: Option<Bytes32>,
    pub available_rfq_ids: Vec<Bytes32>,
    pub noop_add: Option<bool>,
    pub optional_unknown_records: Vec<TlvRecordSummary>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsCommitmentBlob {
    pub raw_len: usize,
    pub raw_digest: Bytes32,
    pub local_assets: AssetOutputListSummary,
    pub remote_assets: AssetOutputListSummary,
    pub outgoing_htlcs: HtlcAssetOutputSummary,
    pub incoming_htlcs: HtlcAssetOutputSummary,
    pub aux_leaves: AuxLeavesSummary,
    pub stxo: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetOutputListSummary {
    pub output_count: usize,
    pub total_amount: u64,
    pub value_len: usize,
    pub value_digest: Bytes32,
    pub outputs: Vec<AssetOutputSummary>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetOutputSummary {
    pub asset_id: Bytes32,
    pub amount: u64,
    pub proof_len: usize,
    pub proof_digest: Bytes32,
    pub output_len: usize,
    pub output_digest: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetBalanceListSummary {
    pub balance_count: usize,
    pub total_amount: u64,
    pub value_len: usize,
    pub value_digest: Bytes32,
    pub balances: Vec<AssetBalanceSummary>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetBalanceSummary {
    pub asset_id: Bytes32,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HtlcAssetOutputSummary {
    pub htlc_count: usize,
    pub total_amount: u64,
    pub value_len: usize,
    pub value_digest: Bytes32,
    pub entries: Vec<HtlcAssetOutputEntrySummary>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HtlcAssetOutputEntrySummary {
    pub htlc_index: u64,
    pub assets: AssetOutputListSummary,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuxLeavesSummary {
    pub value_len: usize,
    pub value_digest: Bytes32,
    pub local_leaf: Option<TapLeafSummary>,
    pub remote_leaf: Option<TapLeafSummary>,
    pub outgoing_htlc_leaf_count: usize,
    pub incoming_htlc_leaf_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TapLeafSummary {
    pub leaf_version: u8,
    pub script_len: usize,
    pub script_digest: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TlvRecordSummary {
    pub type_id: u64,
    pub value_len: usize,
    pub value_digest: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LightningLabsBlobError {
    EmptyHexdump,
    InvalidHexByte {
        line: usize,
        token: String,
    },
    Tlv(TlvError),
    MissingRecord {
        blob: &'static str,
        type_id: u64,
    },
    UnsupportedRecord {
        blob: &'static str,
        type_id: u64,
    },
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidBoolean {
        field: &'static str,
        value: u8,
    },
    TooManyItems {
        field: &'static str,
        count: u64,
        max: u64,
    },
    TruncatedNested {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    TrailingBytes {
        field: &'static str,
        remaining: usize,
    },
    Semantic {
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for LightningLabsBlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHexdump => write!(f, "Lightning Labs hexdump contained no bytes"),
            Self::InvalidHexByte { line, token } => {
                write!(f, "invalid hex byte on hexdump line {line}: {token}")
            }
            Self::Tlv(err) => write!(f, "{err}"),
            Self::MissingRecord { blob, type_id } => {
                write!(f, "{blob} blob missing TLV record type {type_id}")
            }
            Self::UnsupportedRecord { blob, type_id } => {
                write!(
                    f,
                    "{blob} blob contains unsupported TLV record type {type_id}"
                )
            }
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "{field} has invalid length; expected {expected} bytes, got {actual}"
            ),
            Self::InvalidBoolean { field, value } => {
                write!(f, "{field} has invalid boolean value {value}")
            }
            Self::TooManyItems { field, count, max } => {
                write!(f, "{field} count {count} exceeds maximum {max}")
            }
            Self::TruncatedNested {
                field,
                expected,
                actual,
            } => write!(
                f,
                "{field} nested data is truncated; expected {expected} bytes, got {actual}"
            ),
            Self::TrailingBytes { field, remaining } => {
                write!(f, "{field} has {remaining} trailing bytes")
            }
            Self::Semantic { field, reason } => write!(f, "{field} is invalid: {reason}"),
        }
    }
}

impl Error for LightningLabsBlobError {}

impl From<TlvError> for LightningLabsBlobError {
    fn from(err: TlvError) -> Self {
        Self::Tlv(err)
    }
}

pub fn extract_hexdump_bytes(input: &str) -> Result<Vec<u8>, LightningLabsBlobError> {
    let mut bytes = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let before_ascii = trimmed.split('|').next().unwrap_or(trimmed);
        let mut tokens = before_ascii.split_whitespace();
        let Some(offset) = tokens.next() else {
            continue;
        };

        if !offset.chars().all(|ch| ch.is_ascii_hexdigit()) {
            continue;
        }

        for token in tokens {
            if token.len() != 2 {
                continue;
            }

            if !token.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return Err(LightningLabsBlobError::InvalidHexByte {
                    line: line_index + 1,
                    token: token.to_owned(),
                });
            }

            let byte = u8::from_str_radix(token, 16).map_err(|_| {
                LightningLabsBlobError::InvalidHexByte {
                    line: line_index + 1,
                    token: token.to_owned(),
                }
            })?;
            bytes.push(byte);
        }
    }

    if bytes.is_empty() {
        return Err(LightningLabsBlobError::EmptyHexdump);
    }

    Ok(bytes)
}

pub fn decode_funding_blob_hexdump(
    input: &str,
) -> Result<LightningLabsFundingBlob, LightningLabsBlobError> {
    let bytes = extract_hexdump_bytes(input)?;
    decode_funding_blob(&bytes)
}

pub fn decode_htlc_blob_hexdump(
    input: &str,
) -> Result<LightningLabsHtlcBlob, LightningLabsBlobError> {
    let bytes = extract_hexdump_bytes(input)?;
    decode_htlc_blob(&bytes)
}

pub fn decode_commitment_blob_hexdump(
    input: &str,
) -> Result<LightningLabsCommitmentBlob, LightningLabsBlobError> {
    let bytes = extract_hexdump_bytes(input)?;
    decode_commitment_blob(&bytes)
}

pub fn decode_fixture_hexdumps(
    funding_hexdump: &str,
    htlc_hexdump: &str,
    commitment_hexdump: &str,
) -> Result<LightningLabsBlobFixtureReport, LightningLabsBlobError> {
    Ok(LightningLabsBlobFixtureReport {
        source_commit: LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT.to_owned(),
        funding: decode_funding_blob_hexdump(funding_hexdump)?,
        htlc: decode_htlc_blob_hexdump(htlc_hexdump)?,
        commitment: decode_commitment_blob_hexdump(commitment_hexdump)?,
    })
}

pub fn decode_funding_blob(
    bytes: &[u8],
) -> Result<LightningLabsFundingBlob, LightningLabsBlobError> {
    let records = decode_stream(bytes)?;
    reject_unsupported("funding", &records, &[0, 1, 2])?;

    let funded_assets = required_record("funding", &records, TYPE_OPEN_CHANNEL_FUNDED_ASSETS)?;
    let decimal_display = parse_u8_record(
        required_record("funding", &records, TYPE_OPEN_CHANNEL_DECIMAL_DISPLAY)?,
        "funding.decimal_display",
    )?;
    let group_key = optional_record(&records, TYPE_OPEN_CHANNEL_GROUP_KEY)
        .map(|record| parse_compressed_key(&record.value, "funding.group_key"))
        .transpose()?;

    Ok(LightningLabsFundingBlob {
        raw_len: bytes.len(),
        raw_digest: digest(bytes),
        decimal_display,
        group_key,
        funded_assets: parse_asset_output_list(&funded_assets.value, "funding.funded_assets")?,
    })
}

pub fn decode_htlc_blob(bytes: &[u8]) -> Result<LightningLabsHtlcBlob, LightningLabsBlobError> {
    let records = decode_stream(bytes)?;
    if !records.iter().any(|record| {
        matches!(
            record.type_id,
            TYPE_HTLC_AMOUNTS | TYPE_HTLC_RFQ_ID | TYPE_HTLC_AVAILABLE_RFQ_IDS
        )
    }) {
        return Err(LightningLabsBlobError::Semantic {
            field: "htlc",
            reason: "missing asset HTLC records",
        });
    }

    let balances = match optional_record(&records, TYPE_HTLC_AMOUNTS) {
        Some(record) => parse_asset_balance_list(&record.value, "htlc.amounts")?,
        None => empty_asset_balance_list(),
    };
    let rfq_id = optional_record(&records, TYPE_HTLC_RFQ_ID)
        .map(|record| parse_bytes32(&record.value, "htlc.rfq_id"))
        .transpose()?;
    let available_rfq_ids = match optional_record(&records, TYPE_HTLC_AVAILABLE_RFQ_IDS) {
        Some(record) => parse_rfq_id_list(&record.value, "htlc.available_rfq_ids")?,
        None => Vec::new(),
    };
    let noop_add = optional_record(&records, TYPE_HTLC_NOOP_ADD)
        .map(|record| parse_bool_record(record, "htlc.noop_add"))
        .transpose()?;

    let mut optional_unknown_records = Vec::new();
    for record in &records {
        if matches!(
            record.type_id,
            TYPE_HTLC_AMOUNTS | TYPE_HTLC_RFQ_ID | TYPE_HTLC_AVAILABLE_RFQ_IDS | TYPE_HTLC_NOOP_ADD
        ) {
            continue;
        }

        if record.type_id % 2 == 0 {
            return Err(LightningLabsBlobError::UnsupportedRecord {
                blob: "htlc",
                type_id: record.type_id,
            });
        }

        optional_unknown_records.push(TlvRecordSummary::from(record));
    }

    Ok(LightningLabsHtlcBlob {
        raw_len: bytes.len(),
        raw_digest: digest(bytes),
        balances,
        rfq_id,
        available_rfq_ids,
        noop_add,
        optional_unknown_records,
    })
}

pub fn decode_commitment_blob(
    bytes: &[u8],
) -> Result<LightningLabsCommitmentBlob, LightningLabsBlobError> {
    let records = decode_stream(bytes)?;
    reject_unsupported("commitment", &records, &[0, 1, 2, 3, 4, 5])?;

    let local_assets = parse_asset_output_list(
        &required_record("commitment", &records, TYPE_COMMITMENT_LOCAL_ASSETS)?.value,
        "commitment.local_assets",
    )?;
    let remote_assets = parse_asset_output_list(
        &required_record("commitment", &records, TYPE_COMMITMENT_REMOTE_ASSETS)?.value,
        "commitment.remote_assets",
    )?;
    let outgoing_htlcs = parse_htlc_asset_output(
        &required_record("commitment", &records, TYPE_COMMITMENT_OUTGOING_HTLCS)?.value,
        "commitment.outgoing_htlcs",
    )?;
    let incoming_htlcs = parse_htlc_asset_output(
        &required_record("commitment", &records, TYPE_COMMITMENT_INCOMING_HTLCS)?.value,
        "commitment.incoming_htlcs",
    )?;
    let aux_leaves = parse_aux_leaves(
        &required_record("commitment", &records, TYPE_COMMITMENT_AUX_LEAVES)?.value,
        "commitment.aux_leaves",
    )?;
    let stxo = optional_record(&records, TYPE_COMMITMENT_STXO)
        .map(|record| parse_bool_record(record, "commitment.stxo"))
        .transpose()?;

    Ok(LightningLabsCommitmentBlob {
        raw_len: bytes.len(),
        raw_digest: digest(bytes),
        local_assets,
        remote_assets,
        outgoing_htlcs,
        incoming_htlcs,
        aux_leaves,
        stxo,
    })
}

fn parse_asset_output_list(
    bytes: &[u8],
    field: &'static str,
) -> Result<AssetOutputListSummary, LightningLabsBlobError> {
    let mut cursor = bytes;
    let output_count = read_varint(&mut cursor)?;
    if output_count > MAX_OUTPUTS {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: output_count,
            max: MAX_OUTPUTS,
        });
    }

    let mut outputs = Vec::with_capacity(output_count as usize);
    for _ in 0..output_count {
        let output_bytes = read_inline_varbytes(&mut cursor, field)?;
        outputs.push(parse_asset_output(output_bytes, field)?);
    }
    ensure_empty(cursor, field)?;

    let total_amount = checked_sum(outputs.iter().map(|output| output.amount), field)?;
    Ok(AssetOutputListSummary {
        output_count: outputs.len(),
        total_amount,
        value_len: bytes.len(),
        value_digest: digest(bytes),
        outputs,
    })
}

fn parse_asset_output(
    bytes: &[u8],
    field: &'static str,
) -> Result<AssetOutputSummary, LightningLabsBlobError> {
    let records = decode_stream(bytes)?;
    reject_unsupported(field, &records, &[0, 1, 2])?;

    let asset_id = parse_bytes32(&required_record(field, &records, 0)?.value, field)?;
    let amount = parse_u64_record(required_record(field, &records, 1)?, field)?;
    let proof = &required_record(field, &records, 2)?.value;

    Ok(AssetOutputSummary {
        asset_id,
        amount,
        proof_len: proof.len(),
        proof_digest: digest(proof),
        output_len: bytes.len(),
        output_digest: digest(bytes),
    })
}

fn parse_asset_balance_list(
    bytes: &[u8],
    field: &'static str,
) -> Result<AssetBalanceListSummary, LightningLabsBlobError> {
    let mut cursor = bytes;
    let balance_count = read_varint(&mut cursor)?;
    if balance_count > MAX_OUTPUTS {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: balance_count,
            max: MAX_OUTPUTS,
        });
    }

    let mut balances = Vec::with_capacity(balance_count as usize);
    for _ in 0..balance_count {
        let balance_bytes = read_inline_varbytes(&mut cursor, field)?;
        balances.push(parse_asset_balance(balance_bytes, field)?);
    }
    ensure_empty(cursor, field)?;

    let total_amount = checked_sum(balances.iter().map(|balance| balance.amount), field)?;
    Ok(AssetBalanceListSummary {
        balance_count: balances.len(),
        total_amount,
        value_len: bytes.len(),
        value_digest: digest(bytes),
        balances,
    })
}

fn parse_asset_balance(
    bytes: &[u8],
    field: &'static str,
) -> Result<AssetBalanceSummary, LightningLabsBlobError> {
    let records = decode_stream(bytes)?;
    reject_unsupported(field, &records, &[0, 1])?;

    Ok(AssetBalanceSummary {
        asset_id: parse_bytes32(&required_record(field, &records, 0)?.value, field)?,
        amount: parse_u64_record(required_record(field, &records, 1)?, field)?,
    })
}

fn parse_rfq_id_list(
    bytes: &[u8],
    field: &'static str,
) -> Result<Vec<Bytes32>, LightningLabsBlobError> {
    let mut cursor = bytes;
    let id_count = read_varint(&mut cursor)?;
    if id_count > MAX_RFQ_IDS {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: id_count,
            max: MAX_RFQ_IDS,
        });
    }

    let mut ids = Vec::with_capacity(id_count as usize);
    for _ in 0..id_count {
        let id = take_bytes(&mut cursor, 32, field)?;
        ids.push(parse_bytes32(id, field)?);
    }
    ensure_empty(cursor, field)?;
    Ok(ids)
}

fn parse_htlc_asset_output(
    bytes: &[u8],
    field: &'static str,
) -> Result<HtlcAssetOutputSummary, LightningLabsBlobError> {
    let mut cursor = bytes;
    let htlc_count = read_varint(&mut cursor)?;
    if htlc_count > MAX_HTLCS {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: htlc_count,
            max: MAX_HTLCS,
        });
    }

    let mut entries = Vec::with_capacity(htlc_count as usize);
    for _ in 0..htlc_count {
        let htlc_index = read_varint(&mut cursor)?;
        let asset_list = read_inline_varbytes(&mut cursor, field)?;
        entries.push(HtlcAssetOutputEntrySummary {
            htlc_index,
            assets: parse_asset_output_list(asset_list, field)?,
        });
    }
    ensure_empty(cursor, field)?;

    let total_amount = checked_sum(entries.iter().map(|entry| entry.assets.total_amount), field)?;
    Ok(HtlcAssetOutputSummary {
        htlc_count: entries.len(),
        total_amount,
        value_len: bytes.len(),
        value_digest: digest(bytes),
        entries,
    })
}

fn parse_aux_leaves(
    bytes: &[u8],
    field: &'static str,
) -> Result<AuxLeavesSummary, LightningLabsBlobError> {
    let mut cursor = bytes;
    let nested_bytes = read_inline_varbytes(&mut cursor, field)?;
    ensure_empty(cursor, field)?;

    let records = decode_stream(nested_bytes)?;
    reject_unsupported(field, &records, &[0, 1, 2, 3])?;

    let local_leaf = optional_record(&records, 0)
        .map(|record| parse_tap_leaf(&record.value, "commitment.aux_leaves.local_leaf"))
        .transpose()?;
    let remote_leaf = optional_record(&records, 1)
        .map(|record| parse_tap_leaf(&record.value, "commitment.aux_leaves.remote_leaf"))
        .transpose()?;
    let outgoing_htlc_leaf_count = optional_record(&records, 2)
        .map(|record| {
            parse_htlc_aux_leaf_map_count(&record.value, "commitment.aux_leaves.outgoing")
        })
        .transpose()?
        .unwrap_or(0);
    let incoming_htlc_leaf_count = optional_record(&records, 3)
        .map(|record| {
            parse_htlc_aux_leaf_map_count(&record.value, "commitment.aux_leaves.incoming")
        })
        .transpose()?
        .unwrap_or(0);

    Ok(AuxLeavesSummary {
        value_len: nested_bytes.len(),
        value_digest: digest(nested_bytes),
        local_leaf,
        remote_leaf,
        outgoing_htlc_leaf_count,
        incoming_htlc_leaf_count,
    })
}

fn parse_tap_leaf(
    bytes: &[u8],
    field: &'static str,
) -> Result<TapLeafSummary, LightningLabsBlobError> {
    let mut cursor = bytes;
    let leaf_version = *take_bytes(&mut cursor, 1, field)?
        .first()
        .expect("one byte was requested");
    let declared_script_len = read_varint(&mut cursor)?;
    if declared_script_len > MAX_SCRIPT_LEN {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: declared_script_len,
            max: MAX_SCRIPT_LEN,
        });
    }

    let script = read_inline_varbytes(&mut cursor, field)?;
    ensure_empty(cursor, field)?;
    if script.len() as u64 != declared_script_len {
        return Err(LightningLabsBlobError::Semantic {
            field,
            reason: "declared script length does not match inline script length",
        });
    }

    Ok(TapLeafSummary {
        leaf_version,
        script_len: script.len(),
        script_digest: digest(script),
    })
}

fn parse_htlc_aux_leaf_map_count(
    bytes: &[u8],
    field: &'static str,
) -> Result<usize, LightningLabsBlobError> {
    let mut cursor = bytes;
    let htlc_count = read_varint(&mut cursor)?;
    if htlc_count > MAX_HTLCS {
        return Err(LightningLabsBlobError::TooManyItems {
            field,
            count: htlc_count,
            max: MAX_HTLCS,
        });
    }

    for _ in 0..htlc_count {
        let _htlc_index = read_varint(&mut cursor)?;
        let _aux_leaf_bytes = read_inline_varbytes(&mut cursor, field)?;
    }
    ensure_empty(cursor, field)?;

    Ok(htlc_count as usize)
}

fn parse_u8_record(record: &TlvRecord, field: &'static str) -> Result<u8, LightningLabsBlobError> {
    if record.value.len() != 1 {
        return Err(LightningLabsBlobError::InvalidFieldLength {
            field,
            expected: 1,
            actual: record.value.len(),
        });
    }

    Ok(record.value[0])
}

fn parse_u64_record(
    record: &TlvRecord,
    field: &'static str,
) -> Result<u64, LightningLabsBlobError> {
    if record.value.len() != 8 {
        return Err(LightningLabsBlobError::InvalidFieldLength {
            field,
            expected: 8,
            actual: record.value.len(),
        });
    }

    let bytes = record
        .value
        .as_slice()
        .try_into()
        .expect("record length is checked");
    Ok(u64::from_be_bytes(bytes))
}

fn parse_bool_record(
    record: &TlvRecord,
    field: &'static str,
) -> Result<bool, LightningLabsBlobError> {
    let value = parse_u8_record(record, field)?;
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(LightningLabsBlobError::InvalidBoolean { field, value }),
    }
}

fn parse_bytes32(bytes: &[u8], field: &'static str) -> Result<Bytes32, LightningLabsBlobError> {
    if bytes.len() != 32 {
        return Err(LightningLabsBlobError::InvalidFieldLength {
            field,
            expected: 32,
            actual: bytes.len(),
        });
    }

    Ok(Bytes32(
        bytes.try_into().expect("bytes32 length is checked"),
    ))
}

fn parse_compressed_key(
    bytes: &[u8],
    field: &'static str,
) -> Result<CompressedKey, LightningLabsBlobError> {
    if bytes.len() != 33 {
        return Err(LightningLabsBlobError::InvalidFieldLength {
            field,
            expected: 33,
            actual: bytes.len(),
        });
    }

    if !matches!(bytes[0], 0x02 | 0x03) {
        return Err(LightningLabsBlobError::Semantic {
            field,
            reason: "compressed key prefix must be 02 or 03",
        });
    }

    Ok(CompressedKey(
        bytes.try_into().expect("compressed key length is checked"),
    ))
}

fn read_varint(cursor: &mut &[u8]) -> Result<u64, LightningLabsBlobError> {
    decode_big_size(cursor).map_err(LightningLabsBlobError::Tlv)
}

fn read_inline_varbytes<'a>(
    cursor: &mut &'a [u8],
    field: &'static str,
) -> Result<&'a [u8], LightningLabsBlobError> {
    let len = read_varint(cursor)? as usize;
    take_bytes(cursor, len, field)
}

fn take_bytes<'a>(
    cursor: &mut &'a [u8],
    len: usize,
    field: &'static str,
) -> Result<&'a [u8], LightningLabsBlobError> {
    if cursor.len() < len {
        return Err(LightningLabsBlobError::TruncatedNested {
            field,
            expected: len,
            actual: cursor.len(),
        });
    }

    let (head, tail) = cursor.split_at(len);
    *cursor = tail;
    Ok(head)
}

fn ensure_empty(cursor: &[u8], field: &'static str) -> Result<(), LightningLabsBlobError> {
    if cursor.is_empty() {
        return Ok(());
    }

    Err(LightningLabsBlobError::TrailingBytes {
        field,
        remaining: cursor.len(),
    })
}

fn reject_unsupported(
    blob: &'static str,
    records: &[TlvRecord],
    known_types: &[u64],
) -> Result<(), LightningLabsBlobError> {
    for record in records {
        if known_types.binary_search(&record.type_id).is_err() {
            return Err(LightningLabsBlobError::UnsupportedRecord {
                blob,
                type_id: record.type_id,
            });
        }
    }

    Ok(())
}

fn required_record<'a>(
    blob: &'static str,
    records: &'a [TlvRecord],
    type_id: u64,
) -> Result<&'a TlvRecord, LightningLabsBlobError> {
    optional_record(records, type_id).ok_or(LightningLabsBlobError::MissingRecord { blob, type_id })
}

fn optional_record(records: &[TlvRecord], type_id: u64) -> Option<&TlvRecord> {
    records.iter().find(|record| record.type_id == type_id)
}

fn checked_sum(
    amounts: impl Iterator<Item = u64>,
    field: &'static str,
) -> Result<u64, LightningLabsBlobError> {
    let mut sum = 0_u64;
    for amount in amounts {
        sum = sum
            .checked_add(amount)
            .ok_or(LightningLabsBlobError::Semantic {
                field,
                reason: "asset amount overflow",
            })?;
    }
    Ok(sum)
}

fn empty_asset_balance_list() -> AssetBalanceListSummary {
    AssetBalanceListSummary {
        balance_count: 0,
        total_amount: 0,
        value_len: 0,
        value_digest: digest(&[]),
        balances: Vec::new(),
    }
}

fn digest(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

impl From<&TlvRecord> for TlvRecordSummary {
    fn from(record: &TlvRecord) -> Self {
        Self {
            type_id: record.type_id,
            value_len: record.value.len(),
            value_digest: digest(&record.value),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tlv::{TlvRecord, encode_stream};

    use super::*;

    #[test]
    fn extract_hexdump_bytes_ignores_non_dump_lines() {
        let bytes =
            extract_hexdump_bytes("noise\n00000000  01 02 0a  |...|\n00000003  ff          |.|\n")
                .expect("hexdump extracts");

        assert_eq!(bytes, vec![1, 2, 10, 255]);
    }

    #[test]
    fn htlc_decoder_keeps_unknown_odd_records_visible() {
        let rfq_id = [3_u8; 32];
        let encoded = encode_stream(&[
            TlvRecord::new(TYPE_HTLC_RFQ_ID, rfq_id),
            TlvRecord::new(106_823, [0]),
        ])
        .expect("stream encodes");

        let decoded = decode_htlc_blob(&encoded).expect("HTLC decodes");
        assert_eq!(decoded.rfq_id, Some(Bytes32(rfq_id)));
        assert_eq!(decoded.optional_unknown_records.len(), 1);
        assert_eq!(decoded.optional_unknown_records[0].type_id, 106_823);
    }

    #[test]
    fn htlc_decoder_rejects_unknown_even_records() {
        let encoded = encode_stream(&[
            TlvRecord::new(TYPE_HTLC_RFQ_ID, [3_u8; 32]),
            TlvRecord::new(65_542, [0]),
        ])
        .expect("stream encodes");

        assert!(matches!(
            decode_htlc_blob(&encoded),
            Err(LightningLabsBlobError::UnsupportedRecord {
                blob: "htlc",
                type_id: 65_542
            })
        ));
    }
}
