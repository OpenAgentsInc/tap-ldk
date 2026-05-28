use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sha2::{Digest, Sha256};

use crate::mssmt::{MssmtError, MssmtLeaf, MssmtTree};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Bytes32(pub [u8; 32]);

impl Bytes32 {
    pub const ZERO: Self = Self([0; 32]);

    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }
}

impl FromStr for Bytes32 {
    type Err = AssetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = decode_hex(value)?;
        let bytes = bytes.try_into().map_err(|_| AssetError::InvalidLength {
            field: "bytes32",
            expected: 32,
            actual: value.len() / 2,
        })?;

        Ok(Self(bytes))
    }
}

impl Serialize for Bytes32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Bytes32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CompressedKey(pub [u8; 33]);

impl CompressedKey {
    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }
}

impl FromStr for CompressedKey {
    type Err = AssetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = decode_hex(value)?;
        let actual = bytes.len();
        let bytes: [u8; 33] = bytes.try_into().map_err(|_| AssetError::InvalidLength {
            field: "compressed_key",
            expected: 33,
            actual,
        })?;

        if !matches!(bytes[0], 0x02 | 0x03) {
            return Err(AssetError::InvalidCompressedKeyPrefix(bytes[0]));
        }

        Ok(Self(bytes))
    }
}

impl Serialize for CompressedKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for CompressedKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AssetType {
    Normal,
    Collectible,
}

impl AssetType {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Collectible => 1,
        }
    }

    pub fn from_u8(value: u8) -> Result<Self, AssetError> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Collectible),
            other => Err(AssetError::UnsupportedAssetType(other)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Genesis {
    pub first_prev_out: String,
    pub tag: Bytes32,
    pub meta_hash: Bytes32,
    pub output_index: u32,
    pub asset_type: AssetType,
}

impl Genesis {
    pub fn asset_id(&self) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:asset-id:v0");
        hasher.update((self.first_prev_out.len() as u64).to_be_bytes());
        hasher.update(self.first_prev_out.as_bytes());
        hasher.update(self.tag.0);
        hasher.update(self.meta_hash.0);
        hasher.update(self.output_index.to_be_bytes());
        hasher.update([self.asset_type.as_u8()]);

        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct AssetAmount(u64);

impl AssetAmount {
    pub const ZERO: Self = Self(0);

    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, AssetError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(AssetError::AmountOverflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, AssetError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(AssetError::AmountUnderflow)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetLeaf {
    pub asset_id: Bytes32,
    pub script_key: CompressedKey,
    pub amount: AssetAmount,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RootHashSum {
    pub hash: Bytes32,
    pub sum: AssetAmount,
}

pub fn derive_hash_sum_root(leaves: &[AssetLeaf]) -> Result<RootHashSum, AssetError> {
    let mut sorted = leaves.to_vec();
    sorted.sort_by_key(|leaf| (leaf.asset_id, leaf.script_key, leaf.amount.value()));

    let mut sum = AssetAmount::ZERO;
    let mut mssmt_leaves = Vec::with_capacity(sorted.len());
    for (index, leaf) in sorted.into_iter().enumerate() {
        sum = sum.checked_add(leaf.amount)?;
        mssmt_leaves.push((
            synthetic_asset_leaf_key(&leaf, index),
            MssmtLeaf::new(synthetic_asset_leaf_value(&leaf), leaf.amount.value()),
        ));
    }
    let tree = MssmtTree::from_leaves(mssmt_leaves).map_err(AssetError::Mssmt)?;
    let root = tree.root();

    Ok(RootHashSum {
        hash: root.hash,
        sum,
    })
}

pub fn validate_split_conservation(
    input_amount: AssetAmount,
    outputs: &[AssetLeaf],
) -> Result<RootHashSum, AssetError> {
    let root = derive_hash_sum_root(outputs)?;
    if root.sum != input_amount {
        return Err(AssetError::AmountNotConserved {
            input: input_amount.value(),
            output: root.sum.value(),
        });
    }

    Ok(root)
}

pub fn merge_same_asset_inputs(inputs: &[AssetLeaf]) -> Result<AssetLeaf, AssetError> {
    let first = inputs.first().ok_or(AssetError::EmptyInputSet)?;
    let mut amount = AssetAmount::ZERO;

    for input in inputs {
        if input.asset_id != first.asset_id {
            return Err(AssetError::MismatchedAssetId {
                expected: first.asset_id,
                actual: input.asset_id,
            });
        }

        amount = amount.checked_add(input.amount)?;
    }

    Ok(AssetLeaf {
        asset_id: first.asset_id,
        script_key: first.script_key,
        amount,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AssetError {
    HexLength,
    HexCharacter(String),
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidCompressedKeyPrefix(u8),
    AmountOverflow,
    AmountUnderflow,
    AmountNotConserved {
        input: u64,
        output: u64,
    },
    UnsupportedAssetType(u8),
    EmptyInputSet,
    MismatchedAssetId {
        expected: Bytes32,
        actual: Bytes32,
    },
    Mssmt(MssmtError),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HexLength => write!(f, "hex length must be even"),
            Self::HexCharacter(value) => write!(f, "invalid hex byte: {value}"),
            Self::InvalidLength {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{field} has invalid length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidCompressedKeyPrefix(prefix) => {
                write!(f, "invalid compressed key prefix: 0x{prefix:02x}")
            }
            Self::AmountOverflow => write!(f, "asset amount overflow"),
            Self::AmountUnderflow => write!(f, "asset amount underflow"),
            Self::AmountNotConserved { input, output } => {
                write!(
                    f,
                    "asset amount not conserved: input {input}, output {output}"
                )
            }
            Self::UnsupportedAssetType(value) => {
                write!(f, "unsupported asset type {value}")
            }
            Self::EmptyInputSet => write!(f, "input set cannot be empty"),
            Self::MismatchedAssetId { expected, actual } => {
                write!(
                    f,
                    "mismatched asset id: expected {}, got {}",
                    expected.to_hex(),
                    actual.to_hex()
                )
            }
            Self::Mssmt(err) => write!(f, "MS-SMT asset commitment error: {err}"),
        }
    }
}

impl Error for AssetError {}

fn synthetic_asset_leaf_key(leaf: &AssetLeaf, index: usize) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:synthetic-asset-mssmt-key:v1");
    hasher.update(leaf.asset_id.0);
    hasher.update(leaf.script_key.0);
    hasher.update(leaf.amount.value().to_be_bytes());
    hasher.update((index as u64).to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn synthetic_asset_leaf_value(leaf: &AssetLeaf) -> Vec<u8> {
    let mut value = Vec::with_capacity(32 + 33 + 8);
    value.extend_from_slice(&leaf.asset_id.0);
    value.extend_from_slice(&leaf.script_key.0);
    value.extend_from_slice(&leaf.amount.value().to_be_bytes());
    value
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, AssetError> {
    if hex.len() % 2 != 0 {
        return Err(AssetError::HexLength);
    }

    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let value = std::str::from_utf8(chunk)
                .expect("hex input is str")
                .to_owned();
            u8::from_str_radix(&value, 16).map_err(|_| AssetError::HexCharacter(value))
        })
        .collect()
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

    #[test]
    fn asset_id_is_deterministic() {
        let genesis = Genesis {
            first_prev_out: "9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0"
                .to_owned(),
            tag: Bytes32::from_str(
                "0eb36dbcfdec90b302dcdc3b9ef522e2a6f1ed0afec1f8e20faabedf6b162e71",
            )
            .expect("tag parses"),
            meta_hash: Bytes32::from_str(
                "0eb36dbcfdec90b302dcdc3b9ef522e2a6f1ed0afec1f8e20faabedf6b162e71",
            )
            .expect("meta hash parses"),
            output_index: 0,
            asset_type: AssetType::Normal,
        };

        assert_eq!(
            genesis.asset_id().to_hex(),
            "dbe4d6f07f3751421793d77478b1da71c1a1382ea5766d4f9237a20351a862d8"
        );
    }

    #[test]
    fn split_conservation_fails_closed() {
        let leaf = AssetLeaf {
            asset_id: Bytes32::ZERO,
            script_key: CompressedKey::from_str(
                "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
            )
            .expect("script key parses"),
            amount: AssetAmount::new(9),
        };

        assert_eq!(
            validate_split_conservation(AssetAmount::new(10), &[leaf]),
            Err(AssetError::AmountNotConserved {
                input: 10,
                output: 9
            })
        );
    }

    #[test]
    fn merge_rejects_wrong_asset() {
        let script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("script key parses");
        let inputs = [
            AssetLeaf {
                asset_id: Bytes32([1; 32]),
                script_key,
                amount: AssetAmount::new(1),
            },
            AssetLeaf {
                asset_id: Bytes32([2; 32]),
                script_key,
                amount: AssetAmount::new(1),
            },
        ];

        assert!(matches!(
            merge_same_asset_inputs(&inputs),
            Err(AssetError::MismatchedAssetId { .. })
        ));
    }
}
