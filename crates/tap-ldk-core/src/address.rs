use std::{error::Error, fmt, str::FromStr};

use bech32::{FromBase32, ToBase32, Variant};

use crate::{
    asset::{AssetAmount, AssetError, Bytes32, CompressedKey},
    tlv::{
        TlvError, TlvRecord, decode_stream, encode_big_size, encode_stream, reject_unknown_required,
    },
};

const TYPE_VERSION: u64 = 0;
const TYPE_ASSET_ID: u64 = 2;
const TYPE_GROUP_KEY: u64 = 3;
const TYPE_SCRIPT_KEY: u64 = 4;
const TYPE_INTERNAL_KEY: u64 = 6;
const TYPE_TAPSCRIPT_SIBLING: u64 = 7;
const TYPE_AMOUNT: u64 = 8;
const TYPE_PROOF_COURIER_ADDR: u64 = 10;

const KNOWN_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_ASSET_ID,
    TYPE_GROUP_KEY,
    TYPE_SCRIPT_KEY,
    TYPE_INTERNAL_KEY,
    TYPE_TAPSCRIPT_SIBLING,
    TYPE_AMOUNT,
    TYPE_PROOF_COURIER_ADDR,
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TapAddress {
    pub hrp: TapHrp,
    pub version: u8,
    pub asset_id: Bytes32,
    pub group_key: Option<CompressedKey>,
    pub script_key: CompressedKey,
    pub internal_key: CompressedKey,
    pub tapscript_sibling: Vec<u8>,
    pub amount: AssetAmount,
    pub proof_courier_addr: String,
}

impl TapAddress {
    pub fn validate(&self) -> Result<(), AddressError> {
        if self.version != 0 {
            return Err(AddressError::UnsupportedVersion(self.version));
        }

        if self.amount == AssetAmount::ZERO {
            return Err(AddressError::ZeroAmount);
        }

        if self.asset_id == Bytes32::ZERO {
            return Err(AddressError::ZeroAssetId);
        }

        let courier = self.proof_courier_addr.as_str();
        if !(courier.starts_with("hashmail://") || courier.starts_with("universerpc://")) {
            return Err(AddressError::UnsupportedCourier(courier.to_owned()));
        }

        Ok(())
    }

    pub fn encode(&self) -> Result<String, AddressError> {
        self.validate()?;
        let payload = self.encode_payload()?;
        bech32::encode(self.hrp.as_str(), payload.to_base32(), Variant::Bech32m)
            .map_err(AddressError::Bech32)
    }

    pub fn decode(encoded: &str) -> Result<Self, AddressError> {
        let (hrp, data, variant) = bech32::decode(encoded).map_err(AddressError::Bech32)?;
        if variant != Variant::Bech32m {
            return Err(AddressError::WrongBech32Variant);
        }

        let payload = Vec::<u8>::from_base32(&data).map_err(AddressError::Bech32)?;
        let address = Self::decode_payload(TapHrp::from_str(&hrp)?, &payload)?;
        address.validate()?;

        Ok(address)
    }

    pub fn encode_payload(&self) -> Result<Vec<u8>, AddressError> {
        let mut amount = Vec::new();
        encode_big_size(self.amount.value(), &mut amount);

        let mut records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ASSET_ID, self.asset_id.0),
            TlvRecord::new(TYPE_SCRIPT_KEY, self.script_key.0),
            TlvRecord::new(TYPE_INTERNAL_KEY, self.internal_key.0),
            TlvRecord::new(TYPE_AMOUNT, amount),
            TlvRecord::new(TYPE_PROOF_COURIER_ADDR, self.proof_courier_addr.as_bytes()),
        ];

        if let Some(group_key) = self.group_key {
            records.push(TlvRecord::new(TYPE_GROUP_KEY, group_key.0));
        }
        if !self.tapscript_sibling.is_empty() {
            records.push(TlvRecord::new(
                TYPE_TAPSCRIPT_SIBLING,
                self.tapscript_sibling.clone(),
            ));
        }
        records.sort_by_key(|record| record.type_id);

        encode_stream(&records).map_err(AddressError::Tlv)
    }

    pub fn decode_payload(hrp: TapHrp, payload: &[u8]) -> Result<Self, AddressError> {
        let records = decode_stream(payload).map_err(AddressError::Tlv)?;
        reject_unknown_required(&records, KNOWN_TYPES).map_err(AddressError::Tlv)?;

        let find = |type_id| records.iter().find(|record| record.type_id == type_id);
        let version = parse_u8(required(find(TYPE_VERSION), TYPE_VERSION)?.value.as_slice())?;
        let asset_id = parse_bytes32(
            required(find(TYPE_ASSET_ID), TYPE_ASSET_ID)?
                .value
                .as_slice(),
        )?;
        let group_key = find(TYPE_GROUP_KEY)
            .map(|record| parse_compressed_key(record.value.as_slice()))
            .transpose()?;
        let script_key = parse_compressed_key(
            required(find(TYPE_SCRIPT_KEY), TYPE_SCRIPT_KEY)?
                .value
                .as_slice(),
        )?;
        let internal_key = parse_compressed_key(
            required(find(TYPE_INTERNAL_KEY), TYPE_INTERNAL_KEY)?
                .value
                .as_slice(),
        )?;
        let tapscript_sibling = find(TYPE_TAPSCRIPT_SIBLING)
            .map(|record| record.value.clone())
            .unwrap_or_default();
        let amount = AssetAmount::new(parse_big_size_value(
            required(find(TYPE_AMOUNT), TYPE_AMOUNT)?.value.as_slice(),
        )?);
        let proof_courier_addr = parse_string(
            required(find(TYPE_PROOF_COURIER_ADDR), TYPE_PROOF_COURIER_ADDR)?
                .value
                .as_slice(),
        )?;

        Ok(Self {
            hrp,
            version,
            asset_id,
            group_key,
            script_key,
            internal_key,
            tapscript_sibling,
            amount,
            proof_courier_addr,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TapHrp {
    Mainnet,
    Testnet,
    Regtest,
    Simnet,
}

impl TapHrp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "tapbc",
            Self::Testnet => "taptb",
            Self::Regtest => "taprt",
            Self::Simnet => "tapsb",
        }
    }
}

impl FromStr for TapHrp {
    type Err = AddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tapbc" => Ok(Self::Mainnet),
            "taptb" => Ok(Self::Testnet),
            "taprt" => Ok(Self::Regtest),
            "tapsb" => Ok(Self::Simnet),
            other => Err(AddressError::UnknownHrp(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AddressError {
    Tlv(TlvError),
    Asset(AssetError),
    Bech32(bech32::Error),
    MissingField(u64),
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidUtf8,
    UnknownHrp(String),
    WrongBech32Variant,
    UnsupportedVersion(u8),
    ZeroAssetId,
    ZeroAmount,
    UnsupportedCourier(String),
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tlv(err) => write!(f, "address TLV error: {err}"),
            Self::Asset(err) => write!(f, "address asset error: {err}"),
            Self::Bech32(err) => write!(f, "address bech32 error: {err}"),
            Self::MissingField(field) => write!(f, "missing address field {field}"),
            Self::InvalidLength {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid address field {field} length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidUtf8 => write!(f, "address field is not UTF-8"),
            Self::UnknownHrp(hrp) => write!(f, "unknown Taproot Asset HRP {hrp}"),
            Self::WrongBech32Variant => write!(f, "Taproot Asset address must use bech32m"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Taproot Asset address version {version}")
            }
            Self::ZeroAssetId => write!(f, "address asset id cannot be zero"),
            Self::ZeroAmount => write!(f, "address amount cannot be zero"),
            Self::UnsupportedCourier(courier) => {
                write!(f, "unsupported proof courier address {courier}")
            }
        }
    }
}

impl Error for AddressError {}

fn required(record: Option<&TlvRecord>, field: u64) -> Result<&TlvRecord, AddressError> {
    record.ok_or(AddressError::MissingField(field))
}

fn parse_u8(bytes: &[u8]) -> Result<u8, AddressError> {
    if bytes.len() != 1 {
        return Err(AddressError::InvalidLength {
            field: "version",
            expected: 1,
            actual: bytes.len(),
        });
    }

    Ok(bytes[0])
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, AddressError> {
    let actual = bytes.len();
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| AddressError::InvalidLength {
        field: "bytes32",
        expected: 32,
        actual,
    })?;
    Ok(Bytes32(bytes))
}

fn parse_compressed_key(bytes: &[u8]) -> Result<CompressedKey, AddressError> {
    CompressedKey::from_str(&encode_hex(bytes)).map_err(AddressError::Asset)
}

fn parse_string(bytes: &[u8]) -> Result<String, AddressError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| AddressError::InvalidUtf8)
}

fn parse_big_size_value(bytes: &[u8]) -> Result<u64, AddressError> {
    let mut cursor = bytes;
    let value = crate::tlv::decode_big_size(&mut cursor).map_err(AddressError::Tlv)?;
    if !cursor.is_empty() {
        return Err(AddressError::InvalidLength {
            field: "amount",
            expected: bytes.len() - cursor.len(),
            actual: bytes.len(),
        });
    }

    Ok(value)
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
