use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_channel_negotiation::{
        NegotiatedChannelType, NegotiationError, require_asset_message_allowed,
    },
    tlv::{TlvError, TlvRecord, decode_stream, encode_stream, reject_unknown_required},
};

pub const TAP_MESSAGE_TYPE_BASE_OFFSET: u64 = 32_768 + 20_116;
pub const TAP_CHANNEL_MESSAGE_TYPE_OFFSET: u64 = TAP_MESSAGE_TYPE_BASE_OFFSET + 256;
pub const TX_ASSET_INPUT_PROOF_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET;
pub const TX_ASSET_OUTPUT_PROOF_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 1;
pub const ASSET_FUNDING_CREATED_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 2;
pub const ASSET_FUNDING_ACCEPTED_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 3;
pub const RFQ_REQUEST_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 64;
pub const RFQ_ACCEPT_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 65;
pub const RFQ_REJECT_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 66;
pub const ASSET_HTLC_BLOB_TYPE: u64 = TAP_CHANNEL_MESSAGE_TYPE_OFFSET + 96;

const TYPE_MESSAGE_KIND: u64 = 1;
const TYPE_PENDING_CHANNEL_ID: u64 = 3;
const TYPE_ASSET_ID: u64 = 5;
const TYPE_ASSET_AMOUNT: u64 = 7;
const TYPE_PROOF_DIGEST: u64 = 9;
const TYPE_CHUNK_INDEX: u64 = 11;
const TYPE_CHUNK_COUNT: u64 = 13;
const TYPE_CHUNK_BYTES: u64 = 15;
const TYPE_LAST_CHUNK: u64 = 17;
const TYPE_FUNDING_BLOB: u64 = 19;
const TYPE_ACCEPT: u64 = 21;
const TYPE_REJECT_REASON: u64 = 23;
const TYPE_RFQ_ID: u64 = 25;
const TYPE_BTC_MSAT: u64 = 27;
const TYPE_EXPIRY_UNIX_SECONDS: u64 = 29;
const TYPE_INVOICE_CONTEXT: u64 = 31;
const TYPE_HTLC_BLOB: u64 = 33;
const TYPE_QUOTE_ID: u64 = 35;
const TYPE_SCID_ALIAS: u64 = 37;

const KNOWN_TYPES: &[u64] = &[
    TYPE_MESSAGE_KIND,
    TYPE_PENDING_CHANNEL_ID,
    TYPE_ASSET_ID,
    TYPE_ASSET_AMOUNT,
    TYPE_PROOF_DIGEST,
    TYPE_CHUNK_INDEX,
    TYPE_CHUNK_COUNT,
    TYPE_CHUNK_BYTES,
    TYPE_LAST_CHUNK,
    TYPE_FUNDING_BLOB,
    TYPE_ACCEPT,
    TYPE_REJECT_REASON,
    TYPE_RFQ_ID,
    TYPE_BTC_MSAT,
    TYPE_EXPIRY_UNIX_SECONDS,
    TYPE_INVOICE_CONTEXT,
    TYPE_HTLC_BLOB,
    TYPE_QUOTE_ID,
    TYPE_SCID_ALIAS,
];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofChunk {
    pub proof_digest: Bytes32,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub bytes: Vec<u8>,
    pub last: bool,
}

impl ProofChunk {
    pub fn split(proof: &[u8], chunk_size: usize) -> Result<Vec<Self>, AssetPeerMessageError> {
        if chunk_size == 0 {
            return Err(AssetPeerMessageError::InvalidChunkSize);
        }
        if proof.is_empty() {
            return Err(AssetPeerMessageError::EmptyProof);
        }

        let digest = Bytes32(Sha256::digest(proof).into());
        let chunks = proof.chunks(chunk_size).collect::<Vec<_>>();
        let chunk_count = chunks.len() as u32;
        Ok(chunks
            .into_iter()
            .enumerate()
            .map(|(index, bytes)| Self {
                proof_digest: digest,
                chunk_index: index as u32,
                chunk_count,
                bytes: bytes.to_vec(),
                last: index as u32 + 1 == chunk_count,
            })
            .collect())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AssetPeerMessage {
    TxAssetInputProof {
        pending_channel_id: Bytes32,
        asset_id: Bytes32,
        amount: u64,
        chunk: ProofChunk,
    },
    TxAssetOutputProof {
        pending_channel_id: Bytes32,
        asset_id: Bytes32,
        amount: u64,
        chunk: ProofChunk,
    },
    AssetFundingCreated {
        pending_channel_id: Bytes32,
        funding_blob: Vec<u8>,
    },
    AssetFundingAccepted {
        pending_channel_id: Bytes32,
        accept: bool,
        reject_reason: Option<String>,
    },
    RfqRequest {
        rfq_id: Bytes32,
        asset_id: Bytes32,
        asset_amount: u64,
        invoice_context: Bytes32,
    },
    RfqAccept {
        rfq_id: Bytes32,
        quote_id: Bytes32,
        btc_msat: u64,
        expiry_unix_seconds: u64,
        scid_alias: u64,
    },
    RfqReject {
        rfq_id: Bytes32,
        reject_reason: String,
    },
    AssetHtlcBlob {
        asset_id: Bytes32,
        asset_amount: u64,
        rfq_id: Bytes32,
        invoice_context: Bytes32,
        htlc_blob: Vec<u8>,
    },
}

impl AssetPeerMessage {
    pub fn message_type(&self) -> u64 {
        match self {
            Self::TxAssetInputProof { .. } => TX_ASSET_INPUT_PROOF_TYPE,
            Self::TxAssetOutputProof { .. } => TX_ASSET_OUTPUT_PROOF_TYPE,
            Self::AssetFundingCreated { .. } => ASSET_FUNDING_CREATED_TYPE,
            Self::AssetFundingAccepted { .. } => ASSET_FUNDING_ACCEPTED_TYPE,
            Self::RfqRequest { .. } => RFQ_REQUEST_TYPE,
            Self::RfqAccept { .. } => RFQ_ACCEPT_TYPE,
            Self::RfqReject { .. } => RFQ_REJECT_TYPE,
            Self::AssetHtlcBlob { .. } => ASSET_HTLC_BLOB_TYPE,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, AssetPeerMessageError> {
        let mut records = vec![TlvRecord::new(
            TYPE_MESSAGE_KIND,
            self.message_type().to_be_bytes(),
        )];

        match self {
            Self::TxAssetInputProof {
                pending_channel_id,
                asset_id,
                amount,
                chunk,
            }
            | Self::TxAssetOutputProof {
                pending_channel_id,
                asset_id,
                amount,
                chunk,
            } => {
                records.extend(chunk_records(chunk));
                records.push(TlvRecord::new(
                    TYPE_PENDING_CHANNEL_ID,
                    pending_channel_id.0,
                ));
                records.push(TlvRecord::new(TYPE_ASSET_ID, asset_id.0));
                records.push(TlvRecord::new(TYPE_ASSET_AMOUNT, amount.to_be_bytes()));
            }
            Self::AssetFundingCreated {
                pending_channel_id,
                funding_blob,
            } => {
                records.push(TlvRecord::new(
                    TYPE_PENDING_CHANNEL_ID,
                    pending_channel_id.0,
                ));
                records.push(TlvRecord::new(TYPE_FUNDING_BLOB, funding_blob.clone()));
            }
            Self::AssetFundingAccepted {
                pending_channel_id,
                accept,
                reject_reason,
            } => {
                records.push(TlvRecord::new(
                    TYPE_PENDING_CHANNEL_ID,
                    pending_channel_id.0,
                ));
                records.push(TlvRecord::new(TYPE_ACCEPT, [u8::from(*accept)]));
                if let Some(reason) = reject_reason {
                    records.push(TlvRecord::new(TYPE_REJECT_REASON, reason.as_bytes()));
                }
            }
            Self::RfqRequest {
                rfq_id,
                asset_id,
                asset_amount,
                invoice_context,
            } => {
                records.push(TlvRecord::new(TYPE_RFQ_ID, rfq_id.0));
                records.push(TlvRecord::new(TYPE_ASSET_ID, asset_id.0));
                records.push(TlvRecord::new(
                    TYPE_ASSET_AMOUNT,
                    asset_amount.to_be_bytes(),
                ));
                records.push(TlvRecord::new(TYPE_INVOICE_CONTEXT, invoice_context.0));
            }
            Self::RfqAccept {
                rfq_id,
                quote_id,
                btc_msat,
                expiry_unix_seconds,
                scid_alias,
            } => {
                records.push(TlvRecord::new(TYPE_RFQ_ID, rfq_id.0));
                records.push(TlvRecord::new(TYPE_QUOTE_ID, quote_id.0));
                records.push(TlvRecord::new(TYPE_BTC_MSAT, btc_msat.to_be_bytes()));
                records.push(TlvRecord::new(
                    TYPE_EXPIRY_UNIX_SECONDS,
                    expiry_unix_seconds.to_be_bytes(),
                ));
                records.push(TlvRecord::new(TYPE_SCID_ALIAS, scid_alias.to_be_bytes()));
            }
            Self::RfqReject {
                rfq_id,
                reject_reason,
            } => {
                records.push(TlvRecord::new(TYPE_RFQ_ID, rfq_id.0));
                records.push(TlvRecord::new(TYPE_REJECT_REASON, reject_reason.as_bytes()));
            }
            Self::AssetHtlcBlob {
                asset_id,
                asset_amount,
                rfq_id,
                invoice_context,
                htlc_blob,
            } => {
                records.push(TlvRecord::new(TYPE_ASSET_ID, asset_id.0));
                records.push(TlvRecord::new(
                    TYPE_ASSET_AMOUNT,
                    asset_amount.to_be_bytes(),
                ));
                records.push(TlvRecord::new(TYPE_RFQ_ID, rfq_id.0));
                records.push(TlvRecord::new(TYPE_INVOICE_CONTEXT, invoice_context.0));
                records.push(TlvRecord::new(TYPE_HTLC_BLOB, htlc_blob.clone()));
            }
        }

        records.sort_by_key(|record| record.type_id);
        encode_stream(&records).map_err(AssetPeerMessageError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, AssetPeerMessageError> {
        let records = decode_stream(bytes).map_err(AssetPeerMessageError::Tlv)?;
        reject_unknown_required(&records, KNOWN_TYPES).map_err(AssetPeerMessageError::Tlv)?;
        let fields = records
            .into_iter()
            .map(|record| (record.type_id, record.value))
            .collect::<BTreeMap<_, _>>();
        let message_type = parse_u64(required(&fields, TYPE_MESSAGE_KIND)?, "message_type")?;

        match message_type {
            TX_ASSET_INPUT_PROOF_TYPE => Ok(Self::TxAssetInputProof {
                pending_channel_id: parse_bytes32(required(&fields, TYPE_PENDING_CHANNEL_ID)?)?,
                asset_id: parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?,
                amount: parse_u64(required(&fields, TYPE_ASSET_AMOUNT)?, "amount")?,
                chunk: parse_chunk(&fields)?,
            }),
            TX_ASSET_OUTPUT_PROOF_TYPE => Ok(Self::TxAssetOutputProof {
                pending_channel_id: parse_bytes32(required(&fields, TYPE_PENDING_CHANNEL_ID)?)?,
                asset_id: parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?,
                amount: parse_u64(required(&fields, TYPE_ASSET_AMOUNT)?, "amount")?,
                chunk: parse_chunk(&fields)?,
            }),
            ASSET_FUNDING_CREATED_TYPE => Ok(Self::AssetFundingCreated {
                pending_channel_id: parse_bytes32(required(&fields, TYPE_PENDING_CHANNEL_ID)?)?,
                funding_blob: required(&fields, TYPE_FUNDING_BLOB)?.to_vec(),
            }),
            ASSET_FUNDING_ACCEPTED_TYPE => Ok(Self::AssetFundingAccepted {
                pending_channel_id: parse_bytes32(required(&fields, TYPE_PENDING_CHANNEL_ID)?)?,
                accept: parse_bool(required(&fields, TYPE_ACCEPT)?, "accept")?,
                reject_reason: optional_string(&fields, TYPE_REJECT_REASON)?,
            }),
            RFQ_REQUEST_TYPE => Ok(Self::RfqRequest {
                rfq_id: parse_bytes32(required(&fields, TYPE_RFQ_ID)?)?,
                asset_id: parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?,
                asset_amount: parse_u64(required(&fields, TYPE_ASSET_AMOUNT)?, "asset_amount")?,
                invoice_context: parse_bytes32(required(&fields, TYPE_INVOICE_CONTEXT)?)?,
            }),
            RFQ_ACCEPT_TYPE => Ok(Self::RfqAccept {
                rfq_id: parse_bytes32(required(&fields, TYPE_RFQ_ID)?)?,
                quote_id: parse_bytes32(required(&fields, TYPE_QUOTE_ID)?)?,
                btc_msat: parse_u64(required(&fields, TYPE_BTC_MSAT)?, "btc_msat")?,
                expiry_unix_seconds: parse_u64(
                    required(&fields, TYPE_EXPIRY_UNIX_SECONDS)?,
                    "expiry_unix_seconds",
                )?,
                scid_alias: parse_u64(required(&fields, TYPE_SCID_ALIAS)?, "scid_alias")?,
            }),
            RFQ_REJECT_TYPE => Ok(Self::RfqReject {
                rfq_id: parse_bytes32(required(&fields, TYPE_RFQ_ID)?)?,
                reject_reason: parse_string(required(&fields, TYPE_REJECT_REASON)?)?,
            }),
            ASSET_HTLC_BLOB_TYPE => Ok(Self::AssetHtlcBlob {
                asset_id: parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?,
                asset_amount: parse_u64(required(&fields, TYPE_ASSET_AMOUNT)?, "asset_amount")?,
                rfq_id: parse_bytes32(required(&fields, TYPE_RFQ_ID)?)?,
                invoice_context: parse_bytes32(required(&fields, TYPE_INVOICE_CONTEXT)?)?,
                htlc_blob: required(&fields, TYPE_HTLC_BLOB)?.to_vec(),
            }),
            other => Err(AssetPeerMessageError::UnknownMessageType(other)),
        }
    }
}

pub fn decode_negotiated_message(
    channel_type: &NegotiatedChannelType,
    bytes: &[u8],
) -> Result<AssetPeerMessage, AssetPeerMessageError> {
    require_asset_message_allowed(channel_type).map_err(AssetPeerMessageError::Negotiation)?;
    AssetPeerMessage::decode(bytes)
}

#[derive(Debug, Default)]
pub struct ProofReassembler {
    digest: Option<Bytes32>,
    chunk_count: Option<u32>,
    chunks: BTreeMap<u32, Vec<u8>>,
}

impl ProofReassembler {
    pub fn push(&mut self, chunk: ProofChunk) -> Result<ProofAssembly, AssetPeerMessageError> {
        if chunk.chunk_count == 0 {
            return Err(AssetPeerMessageError::InvalidChunkCount);
        }
        if chunk.chunk_index >= chunk.chunk_count {
            return Err(AssetPeerMessageError::ChunkIndexOutOfRange {
                index: chunk.chunk_index,
                count: chunk.chunk_count,
            });
        }
        if chunk.last && chunk.chunk_index + 1 != chunk.chunk_count {
            return Err(AssetPeerMessageError::InvalidLastChunk {
                index: chunk.chunk_index,
                count: chunk.chunk_count,
            });
        }

        match self.digest {
            Some(digest) if digest != chunk.proof_digest => {
                return Err(AssetPeerMessageError::MixedProofDigest);
            }
            None => self.digest = Some(chunk.proof_digest),
            _ => {}
        }
        match self.chunk_count {
            Some(count) if count != chunk.chunk_count => {
                return Err(AssetPeerMessageError::MixedChunkCount);
            }
            None => self.chunk_count = Some(chunk.chunk_count),
            _ => {}
        }
        if self.chunks.insert(chunk.chunk_index, chunk.bytes).is_some() {
            return Err(AssetPeerMessageError::DuplicateChunk(chunk.chunk_index));
        }

        if self.chunks.len() != chunk.chunk_count as usize {
            return Ok(ProofAssembly::Incomplete {
                received: self.chunks.len() as u32,
                expected: chunk.chunk_count,
            });
        }

        let mut proof = Vec::new();
        for index in 0..chunk.chunk_count {
            let bytes = self
                .chunks
                .get(&index)
                .ok_or(AssetPeerMessageError::IncompleteProof)?;
            proof.extend_from_slice(bytes);
        }
        let digest = Bytes32(Sha256::digest(&proof).into());
        if Some(digest) != self.digest {
            return Err(AssetPeerMessageError::ProofDigestMismatch);
        }

        Ok(ProofAssembly::Complete(proof))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProofAssembly {
    Incomplete { received: u32, expected: u32 },
    Complete(Vec<u8>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PeerMessageSmokeReport {
    pub message_count: usize,
    pub reassembled_proof_len: usize,
    pub premature_message_rejected: bool,
}

pub fn run_peer_message_smoke(
    channel_type: &NegotiatedChannelType,
    asset_id: Bytes32,
) -> Result<PeerMessageSmokeReport, AssetPeerMessageError> {
    let pending_channel_id = Bytes32([3; 32]);
    let proof = b"bounded native proof transport smoke";
    let chunks = ProofChunk::split(proof, 8)?;
    let mut reassembler = ProofReassembler::default();
    let mut message_count = 0;
    let mut complete = None;

    for chunk in chunks {
        let message = AssetPeerMessage::TxAssetInputProof {
            pending_channel_id,
            asset_id,
            amount: 1_000,
            chunk,
        };
        let encoded = message.encode()?;
        let decoded = decode_negotiated_message(channel_type, &encoded)?;
        if let AssetPeerMessage::TxAssetInputProof { chunk, .. } = decoded {
            if let ProofAssembly::Complete(proof) = reassembler.push(chunk)? {
                complete = Some(proof);
            }
        }
        message_count += 1;
    }

    let complete = complete.ok_or(AssetPeerMessageError::IncompleteProof)?;
    let premature_message_rejected = decode_negotiated_message(
        &NegotiatedChannelType::BtcOnly,
        &AssetPeerMessage::RfqRequest {
            rfq_id: Bytes32([4; 32]),
            asset_id,
            asset_amount: 1,
            invoice_context: Bytes32([5; 32]),
        }
        .encode()?,
    )
    .is_err();

    Ok(PeerMessageSmokeReport {
        message_count,
        reassembled_proof_len: complete.len(),
        premature_message_rejected,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AssetPeerMessageError {
    Tlv(TlvError),
    Negotiation(NegotiationError),
    MissingField(u64),
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidBool(u8),
    InvalidUtf8,
    UnknownMessageType(u64),
    InvalidChunkSize,
    EmptyProof,
    InvalidChunkCount,
    ChunkIndexOutOfRange {
        index: u32,
        count: u32,
    },
    InvalidLastChunk {
        index: u32,
        count: u32,
    },
    MixedProofDigest,
    MixedChunkCount,
    DuplicateChunk(u32),
    IncompleteProof,
    ProofDigestMismatch,
}

impl fmt::Display for AssetPeerMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tlv(err) => write!(f, "asset peer TLV error: {err}"),
            Self::Negotiation(err) => write!(f, "asset peer negotiation error: {err}"),
            Self::MissingField(field) => write!(f, "missing asset peer message field {field}"),
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "invalid asset peer field {field} length: expected {expected}, got {actual}"
            ),
            Self::InvalidBool(value) => write!(f, "invalid asset peer boolean value {value}"),
            Self::InvalidUtf8 => write!(f, "asset peer string field is not UTF-8"),
            Self::UnknownMessageType(message_type) => {
                write!(f, "unknown asset peer message type {message_type}")
            }
            Self::InvalidChunkSize => write!(f, "proof chunk size must be greater than zero"),
            Self::EmptyProof => write!(f, "proof transport cannot split an empty proof"),
            Self::InvalidChunkCount => write!(f, "proof chunk count must be greater than zero"),
            Self::ChunkIndexOutOfRange { index, count } => {
                write!(
                    f,
                    "proof chunk index {index} out of range for count {count}"
                )
            }
            Self::InvalidLastChunk { index, count } => {
                write!(f, "proof chunk {index} marked last for count {count}")
            }
            Self::MixedProofDigest => write!(f, "proof chunks contain mixed digests"),
            Self::MixedChunkCount => write!(f, "proof chunks contain mixed counts"),
            Self::DuplicateChunk(index) => write!(f, "duplicate proof chunk {index}"),
            Self::IncompleteProof => write!(f, "proof chunks are incomplete"),
            Self::ProofDigestMismatch => write!(f, "proof chunk digest mismatch"),
        }
    }
}

impl Error for AssetPeerMessageError {}

fn chunk_records(chunk: &ProofChunk) -> Vec<TlvRecord> {
    vec![
        TlvRecord::new(TYPE_PROOF_DIGEST, chunk.proof_digest.0),
        TlvRecord::new(TYPE_CHUNK_INDEX, chunk.chunk_index.to_be_bytes()),
        TlvRecord::new(TYPE_CHUNK_COUNT, chunk.chunk_count.to_be_bytes()),
        TlvRecord::new(TYPE_CHUNK_BYTES, chunk.bytes.clone()),
        TlvRecord::new(TYPE_LAST_CHUNK, [u8::from(chunk.last)]),
    ]
}

fn parse_chunk(fields: &BTreeMap<u64, Vec<u8>>) -> Result<ProofChunk, AssetPeerMessageError> {
    Ok(ProofChunk {
        proof_digest: parse_bytes32(required(fields, TYPE_PROOF_DIGEST)?)?,
        chunk_index: parse_u32(required(fields, TYPE_CHUNK_INDEX)?, "chunk_index")?,
        chunk_count: parse_u32(required(fields, TYPE_CHUNK_COUNT)?, "chunk_count")?,
        bytes: required(fields, TYPE_CHUNK_BYTES)?.to_vec(),
        last: parse_bool(required(fields, TYPE_LAST_CHUNK)?, "last")?,
    })
}

fn required(fields: &BTreeMap<u64, Vec<u8>>, field: u64) -> Result<&[u8], AssetPeerMessageError> {
    fields
        .get(&field)
        .map(Vec::as_slice)
        .ok_or(AssetPeerMessageError::MissingField(field))
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, AssetPeerMessageError> {
    let actual = bytes.len();
    let bytes: [u8; 32] =
        bytes
            .try_into()
            .map_err(|_| AssetPeerMessageError::InvalidFieldLength {
                field: "bytes32",
                expected: 32,
                actual,
            })?;
    Ok(Bytes32(bytes))
}

fn parse_u32(bytes: &[u8], field: &'static str) -> Result<u32, AssetPeerMessageError> {
    let actual = bytes.len();
    let bytes: [u8; 4] =
        bytes
            .try_into()
            .map_err(|_| AssetPeerMessageError::InvalidFieldLength {
                field,
                expected: 4,
                actual,
            })?;
    Ok(u32::from_be_bytes(bytes))
}

fn parse_u64(bytes: &[u8], field: &'static str) -> Result<u64, AssetPeerMessageError> {
    let actual = bytes.len();
    let bytes: [u8; 8] =
        bytes
            .try_into()
            .map_err(|_| AssetPeerMessageError::InvalidFieldLength {
                field,
                expected: 8,
                actual,
            })?;
    Ok(u64::from_be_bytes(bytes))
}

fn parse_bool(bytes: &[u8], field: &'static str) -> Result<bool, AssetPeerMessageError> {
    if bytes.len() != 1 {
        return Err(AssetPeerMessageError::InvalidFieldLength {
            field,
            expected: 1,
            actual: bytes.len(),
        });
    }
    match bytes[0] {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(AssetPeerMessageError::InvalidBool(other)),
    }
}

fn optional_string(
    fields: &BTreeMap<u64, Vec<u8>>,
    field: u64,
) -> Result<Option<String>, AssetPeerMessageError> {
    fields
        .get(&field)
        .map(|bytes| parse_string(bytes))
        .transpose()
}

fn parse_string(bytes: &[u8]) -> Result<String, AssetPeerMessageError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| AssetPeerMessageError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_id() -> Bytes32 {
        Bytes32([7; 32])
    }

    fn pending_channel_id() -> Bytes32 {
        Bytes32([8; 32])
    }

    fn negotiated_asset_channel() -> NegotiatedChannelType {
        NegotiatedChannelType::SingleAsset {
            asset_id: asset_id(),
            protocol_version: 1,
        }
    }

    #[test]
    fn funding_and_control_messages_round_trip() {
        let chunk = ProofChunk::split(b"proof bytes", 5)
            .expect("proof splits")
            .remove(0);
        let messages = vec![
            AssetPeerMessage::TxAssetInputProof {
                pending_channel_id: pending_channel_id(),
                asset_id: asset_id(),
                amount: 10,
                chunk: chunk.clone(),
            },
            AssetPeerMessage::TxAssetOutputProof {
                pending_channel_id: pending_channel_id(),
                asset_id: asset_id(),
                amount: 10,
                chunk,
            },
            AssetPeerMessage::AssetFundingCreated {
                pending_channel_id: pending_channel_id(),
                funding_blob: vec![1, 2, 3],
            },
            AssetPeerMessage::AssetFundingAccepted {
                pending_channel_id: pending_channel_id(),
                accept: false,
                reject_reason: Some("bad proof".to_owned()),
            },
        ];

        for message in messages {
            let encoded = message.encode().expect("message encodes");
            let decoded = AssetPeerMessage::decode(&encoded).expect("message decodes");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn rfq_and_htlc_messages_round_trip() {
        let rfq_id = Bytes32([9; 32]);
        let invoice_context = Bytes32([10; 32]);
        let messages = vec![
            AssetPeerMessage::RfqRequest {
                rfq_id,
                asset_id: asset_id(),
                asset_amount: 25,
                invoice_context,
            },
            AssetPeerMessage::RfqAccept {
                rfq_id,
                quote_id: Bytes32([11; 32]),
                btc_msat: 1_000,
                expiry_unix_seconds: 2_000,
                scid_alias: 101,
            },
            AssetPeerMessage::RfqReject {
                rfq_id,
                reject_reason: "expired".to_owned(),
            },
            AssetPeerMessage::AssetHtlcBlob {
                asset_id: asset_id(),
                asset_amount: 25,
                rfq_id,
                invoice_context,
                htlc_blob: vec![4, 5, 6],
            },
        ];

        for message in messages {
            let encoded = message.encode().expect("message encodes");
            let decoded = AssetPeerMessage::decode(&encoded).expect("message decodes");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn negotiated_decode_rejects_premature_asset_message() {
        let message = AssetPeerMessage::RfqRequest {
            rfq_id: Bytes32([1; 32]),
            asset_id: asset_id(),
            asset_amount: 1,
            invoice_context: Bytes32([2; 32]),
        };
        let encoded = message.encode().expect("message encodes");

        assert!(matches!(
            decode_negotiated_message(&NegotiatedChannelType::BtcOnly, &encoded),
            Err(AssetPeerMessageError::Negotiation(
                NegotiationError::PrematureAssetMessage
            ))
        ));
        assert_eq!(
            decode_negotiated_message(&negotiated_asset_channel(), &encoded)
                .expect("negotiated decode succeeds"),
            message
        );
    }

    #[test]
    fn proof_reassembler_completes_and_rejects_bad_chunks() {
        let chunks = ProofChunk::split(b"abcdefghijklmnopqrstuvwxyz", 7).expect("proof splits");
        let mut reassembler = ProofReassembler::default();

        for chunk in chunks.clone() {
            let result = reassembler.push(chunk).expect("chunk accepted");
            if let ProofAssembly::Complete(proof) = result {
                assert_eq!(proof, b"abcdefghijklmnopqrstuvwxyz");
                return;
            }
        }

        panic!("proof did not complete");
    }

    #[test]
    fn proof_reassembler_rejects_digest_mismatch_and_duplicate_chunks() {
        let chunks = ProofChunk::split(b"proof material", 6).expect("proof splits");
        let mut duplicate = ProofReassembler::default();
        duplicate
            .push(chunks[0].clone())
            .expect("first chunk accepted");
        assert_eq!(
            duplicate.push(chunks[0].clone()),
            Err(AssetPeerMessageError::DuplicateChunk(0))
        );

        let mut mismatch = ProofReassembler::default();
        let mut bad = chunks[0].clone();
        bad.proof_digest = Bytes32([99; 32]);
        mismatch.push(bad).expect("first bad digest accepted");
        assert_eq!(
            mismatch.push(chunks[1].clone()),
            Err(AssetPeerMessageError::MixedProofDigest)
        );
    }

    #[test]
    fn malformed_tlv_and_unknown_required_types_fail_closed() {
        let malformed = [TYPE_MESSAGE_KIND as u8, 8, 0, 1];
        assert!(matches!(
            AssetPeerMessage::decode(&malformed),
            Err(AssetPeerMessageError::Tlv(_))
        ));

        let encoded = encode_stream(&[
            TlvRecord::new(TYPE_MESSAGE_KIND, TX_ASSET_INPUT_PROOF_TYPE.to_be_bytes()),
            TlvRecord::new(2, []),
        ])
        .expect("records encode");
        assert!(matches!(
            AssetPeerMessage::decode(&encoded),
            Err(AssetPeerMessageError::Tlv(TlvError::UnknownRequiredType(2)))
        ));
    }

    #[test]
    fn smoke_reassembles_proof_and_rejects_btc_only_message() {
        let report = run_peer_message_smoke(&negotiated_asset_channel(), asset_id())
            .expect("peer message smoke passes");

        assert!(report.message_count > 1);
        assert_eq!(
            report.reassembled_proof_len,
            b"bounded native proof transport smoke".len()
        );
        assert!(report.premature_message_rejected);
    }
}
