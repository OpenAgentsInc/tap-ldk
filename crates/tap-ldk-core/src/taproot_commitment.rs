use std::{collections::BTreeMap, error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    asset::{AssetAmount, AssetError, AssetType, Bytes32, CompressedKey},
    mssmt::{MssmtError, MssmtLeaf, MssmtNode, MssmtProof, MssmtTree},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum AssetVersion {
    V0,
    V1,
}

impl AssetVersion {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::V0 => 0,
            Self::V1 => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum TapCommitmentVersion {
    V0,
    V1,
    V2,
}

impl TapCommitmentVersion {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::V0 => 0,
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TapAsset {
    pub version: AssetVersion,
    pub asset_id: Bytes32,
    pub asset_type: AssetType,
    pub genesis_outpoint: String,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub group_key: Option<CompressedKey>,
}

impl TapAsset {
    pub fn tap_commitment_key(&self) -> Bytes32 {
        match self.group_key {
            Some(group_key) => sha256_bytes(&x_only_key(group_key)),
            None => self.asset_id,
        }
    }

    pub fn asset_commitment_key(&self) -> Bytes32 {
        let script_key = x_only_key(self.script_key);
        match self.group_key {
            Some(_) => sha256_join(&[&self.asset_id.0, &script_key]),
            None => sha256_bytes(&script_key),
        }
    }

    pub fn asset_leaf(&self) -> MssmtLeaf {
        MssmtLeaf::new(self.asset_leaf_value(), self.amount.value())
    }

    fn asset_leaf_value(&self) -> Vec<u8> {
        let mut value = Vec::new();
        value.push(self.version.as_u8());
        value.push(self.asset_type.as_u8());
        value.extend_from_slice(&self.asset_id.0);
        push_len_prefixed_bytes(&mut value, self.genesis_outpoint.as_bytes());
        value.extend_from_slice(&self.amount.value().to_be_bytes());
        value.extend_from_slice(&self.script_key.0);
        match self.group_key {
            Some(group_key) => {
                value.push(1);
                value.extend_from_slice(&group_key.0);
            }
            None => value.push(0),
        }
        value
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetCommitment {
    pub version: AssetVersion,
    pub tap_key: Bytes32,
    pub asset_type: AssetType,
    pub tree_root: MssmtNode,
    pub root_identifier: Bytes32,
    assets: BTreeMap<Bytes32, TapAsset>,
    tree: MssmtTree,
}

impl AssetCommitment {
    pub fn new(assets: Vec<TapAsset>) -> Result<Self, TaprootCommitmentError> {
        let first = assets
            .first()
            .ok_or(TaprootCommitmentError::NoAssets)?
            .clone();
        let tap_key = first.tap_commitment_key();
        let asset_type = first.asset_type;
        let mut version = first.version;
        let mut committed_assets = BTreeMap::new();
        let mut leaves = Vec::with_capacity(assets.len());

        for asset in assets {
            if asset.tap_commitment_key() != tap_key {
                return Err(TaprootCommitmentError::TapKeyMismatch);
            }
            if asset.asset_type != asset_type {
                return Err(TaprootCommitmentError::AssetTypeMismatch);
            }
            version = version.max(asset.version);
            let key = asset.asset_commitment_key();
            if committed_assets.insert(key, asset.clone()).is_some() {
                return Err(TaprootCommitmentError::DuplicateAssetCommitmentKey(key));
            }
            leaves.push((key, asset.asset_leaf()));
        }

        let tree = MssmtTree::from_leaves(leaves).map_err(TaprootCommitmentError::Mssmt)?;
        let tree_root = tree.root();
        let root_identifier = asset_commitment_root_identifier(tap_key, &tree);

        Ok(Self {
            version,
            tap_key,
            asset_type,
            tree_root,
            root_identifier,
            assets: committed_assets,
            tree,
        })
    }

    pub fn tap_commitment_key(&self) -> Bytes32 {
        self.tap_key
    }

    pub fn tap_commitment_leaf(&self) -> MssmtLeaf {
        let mut value = Vec::with_capacity(1 + 32 + 8);
        value.push(self.version.as_u8());
        value.extend_from_slice(&self.root_identifier.0);
        value.extend_from_slice(&self.tree_root.sum.to_be_bytes());
        MssmtLeaf::new(value, self.tree_root.sum)
    }

    pub fn asset_proof(&self, key: Bytes32) -> (Option<&TapAsset>, MssmtProof) {
        (self.assets.get(&key), self.tree.proof(key))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TapCommitment {
    pub version: TapCommitmentVersion,
    pub tree_root: MssmtNode,
    commitments: BTreeMap<Bytes32, AssetCommitment>,
    tree: MssmtTree,
}

impl TapCommitment {
    pub fn new(
        version: Option<TapCommitmentVersion>,
        commitments: Vec<AssetCommitment>,
    ) -> Result<Self, TaprootCommitmentError> {
        let version = version.unwrap_or_else(|| {
            commitments
                .iter()
                .map(|commitment| match commitment.version {
                    AssetVersion::V0 => TapCommitmentVersion::V0,
                    AssetVersion::V1 => TapCommitmentVersion::V1,
                })
                .max()
                .unwrap_or(TapCommitmentVersion::V0)
        });
        let mut commitment_map = BTreeMap::new();
        let mut leaves = Vec::with_capacity(commitments.len());
        for commitment in commitments {
            let key = commitment.tap_commitment_key();
            if commitment_map.insert(key, commitment.clone()).is_some() {
                return Err(TaprootCommitmentError::DuplicateTapCommitmentKey(key));
            }
            leaves.push((key, commitment.tap_commitment_leaf()));
        }

        let tree = MssmtTree::from_leaves(leaves).map_err(TaprootCommitmentError::Mssmt)?;
        let tree_root = tree.root();
        Ok(Self {
            version,
            tree_root,
            commitments: commitment_map,
            tree,
        })
    }

    pub fn from_assets(
        version: Option<TapCommitmentVersion>,
        assets: Vec<TapAsset>,
    ) -> Result<Self, TaprootCommitmentError> {
        let mut groups = BTreeMap::<Bytes32, Vec<TapAsset>>::new();
        for asset in assets {
            groups
                .entry(asset.tap_commitment_key())
                .or_default()
                .push(asset);
        }
        let commitments = groups
            .into_values()
            .map(AssetCommitment::new)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(version, commitments)
    }

    pub fn commitment(&self, tap_key: Bytes32) -> Option<&AssetCommitment> {
        self.commitments.get(&tap_key)
    }

    pub fn tap_commitment_proof(&self, tap_key: Bytes32) -> MssmtProof {
        self.tree.proof(tap_key)
    }

    pub fn tap_leaf_script(&self) -> Vec<u8> {
        tap_leaf_script(self.version, self.tree_root)
    }

    pub fn tap_leaf_hash(&self) -> Bytes32 {
        tap_leaf_hash(&self.tap_leaf_script())
    }

    pub fn tapscript_root(
        &self,
        sibling: Option<Bytes32>,
    ) -> Result<Bytes32, TaprootCommitmentError> {
        let leaf_hash = self.tap_leaf_hash();
        match sibling {
            Some(sibling) if sibling == leaf_hash => {
                Err(TaprootCommitmentError::DuplicateTaprootSibling)
            }
            Some(sibling) => Ok(tap_branch_hash(leaf_hash, sibling)),
            None => Ok(leaf_hash),
        }
    }

    pub fn validate_tapscript_root(
        &self,
        sibling: Option<Bytes32>,
        expected_root: Bytes32,
    ) -> Result<(), TaprootCommitmentError> {
        let actual = self.tapscript_root(sibling)?;
        if actual != expected_root {
            return Err(TaprootCommitmentError::OutputCommitmentMismatch {
                expected: expected_root,
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TaprootCommitmentError {
    Asset(AssetError),
    Mssmt(MssmtError),
    NoAssets,
    TapKeyMismatch,
    AssetTypeMismatch,
    DuplicateAssetCommitmentKey(Bytes32),
    DuplicateTapCommitmentKey(Bytes32),
    InvalidTapLeafScript,
    DuplicateTaprootSibling,
    OutputCommitmentMismatch { expected: Bytes32, actual: Bytes32 },
}

impl fmt::Display for TaprootCommitmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(err) => write!(f, "Taproot commitment asset error: {err}"),
            Self::Mssmt(err) => write!(f, "Taproot commitment MS-SMT error: {err}"),
            Self::NoAssets => write!(f, "asset commitment requires at least one asset"),
            Self::TapKeyMismatch => write!(f, "asset commitment tap keys do not match"),
            Self::AssetTypeMismatch => write!(f, "asset commitment asset types do not match"),
            Self::DuplicateAssetCommitmentKey(key) => {
                write!(f, "duplicate asset commitment key {}", key.to_hex())
            }
            Self::DuplicateTapCommitmentKey(key) => {
                write!(f, "duplicate tap commitment key {}", key.to_hex())
            }
            Self::InvalidTapLeafScript => write!(f, "invalid Taproot Asset commitment script"),
            Self::DuplicateTaprootSibling => {
                write!(
                    f,
                    "taproot sibling duplicates Taproot Asset commitment leaf"
                )
            }
            Self::OutputCommitmentMismatch { expected, actual } => write!(
                f,
                "taproot output commitment mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
        }
    }
}

impl Error for TaprootCommitmentError {}

pub fn is_taproot_asset_commitment_script(script: &[u8]) -> bool {
    if script.len() != 73 {
        return false;
    }
    let marker = Sha256::digest(b"taproot-assets");
    let v2_marker = Sha256::digest(b"taproot-assets:194243");
    matches!(script[0], 0 | 1) && script[1..33] == marker[..]
        || script[0..32] == v2_marker[..] && script[32] == 2
}

pub fn parse_tap_leaf_script_root(script: &[u8]) -> Result<MssmtNode, TaprootCommitmentError> {
    if !is_taproot_asset_commitment_script(script) {
        return Err(TaprootCommitmentError::InvalidTapLeafScript);
    }

    let (hash_start, sum_start) = if matches!(script[0], 0 | 1) {
        (33, 65)
    } else {
        (33, 65)
    };
    let mut hash = [0; 32];
    hash.copy_from_slice(&script[hash_start..hash_start + 32]);
    let mut sum = [0; 8];
    sum.copy_from_slice(&script[sum_start..sum_start + 8]);
    Ok(MssmtNode {
        hash: Bytes32(hash),
        sum: u64::from_be_bytes(sum),
    })
}

pub fn tap_leaf_script(version: TapCommitmentVersion, root: MssmtNode) -> Vec<u8> {
    let mut script = Vec::with_capacity(73);
    match version {
        TapCommitmentVersion::V0 | TapCommitmentVersion::V1 => {
            script.push(version.as_u8());
            script.extend_from_slice(&Sha256::digest(b"taproot-assets"));
            script.extend_from_slice(&root.hash.0);
            script.extend_from_slice(&root.sum.to_be_bytes());
        }
        TapCommitmentVersion::V2 => {
            script.extend_from_slice(&Sha256::digest(b"taproot-assets:194243"));
            script.push(version.as_u8());
            script.extend_from_slice(&root.hash.0);
            script.extend_from_slice(&root.sum.to_be_bytes());
        }
    }
    script
}

fn asset_commitment_root_identifier(tap_key: Bytes32, tree: &MssmtTree) -> Bytes32 {
    let (left, right) = tree.root_children();
    let mut hasher = Sha256::new();
    hasher.update(tap_key.0);
    hasher.update(left.hash.0);
    hasher.update(right.hash.0);
    hasher.update(tree.root().sum.to_be_bytes());
    Bytes32(hasher.finalize().into())
}

fn x_only_key(key: CompressedKey) -> [u8; 32] {
    key.0[1..33]
        .try_into()
        .expect("compressed key has x-only body")
}

fn push_len_prefixed_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn tap_leaf_hash(script: &[u8]) -> Bytes32 {
    let mut preimage = Vec::with_capacity(2 + script.len());
    preimage.push(0xc0);
    push_compact_size(&mut preimage, script.len() as u64);
    preimage.extend_from_slice(script);
    tagged_hash_bip341(b"TapLeaf", &preimage)
}

fn tap_branch_hash(a: Bytes32, b: Bytes32) -> Bytes32 {
    let (left, right) = if a <= b { (a, b) } else { (b, a) };
    let mut preimage = Vec::with_capacity(64);
    preimage.extend_from_slice(&left.0);
    preimage.extend_from_slice(&right.0);
    tagged_hash_bip341(b"TapBranch", &preimage)
}

fn push_compact_size(out: &mut Vec<u8>, value: u64) {
    if value < 253 {
        out.push(value as u8);
    } else if value <= u16::MAX as u64 {
        out.push(253);
        out.extend_from_slice(&(value as u16).to_le_bytes());
    } else if value <= u32::MAX as u64 {
        out.push(254);
        out.extend_from_slice(&(value as u32).to_le_bytes());
    } else {
        out.push(255);
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn sha256_bytes(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

fn sha256_join(parts: &[&[u8]]) -> Bytes32 {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    Bytes32(hasher.finalize().into())
}

fn tagged_hash_bip341(tag: &[u8], msg: &[u8]) -> Bytes32 {
    let tag_hash = Sha256::digest(tag);
    let mut hasher = Sha256::new();
    hasher.update(tag_hash);
    hasher.update(tag_hash);
    hasher.update(msg);
    Bytes32(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn asset(script_prefix: u8, amount: u64) -> TapAsset {
        TapAsset {
            version: AssetVersion::V1,
            asset_id: Bytes32::from_str(
                "7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93",
            )
            .expect("asset id parses"),
            asset_type: AssetType::Normal,
            genesis_outpoint: "9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0"
                .to_owned(),
            amount: AssetAmount::new(amount),
            script_key: CompressedKey([script_prefix; 33]),
            group_key: None,
        }
    }

    #[test]
    fn asset_and_tap_commitments_build_and_prove() {
        let mut first = asset(2, 700);
        first.script_key.0[0] = 2;
        let mut second = asset(3, 300);
        second.script_key.0[0] = 3;
        let commitment = AssetCommitment::new(vec![first.clone(), second.clone()])
            .expect("asset commitment builds");

        assert_eq!(commitment.tree_root.sum, 1_000);
        assert_ne!(commitment.root_identifier, commitment.tree_root.hash);

        let key = first.asset_commitment_key();
        let (proof_asset, proof) = commitment.asset_proof(key);
        assert_eq!(proof_asset, Some(&first));
        assert!(
            proof
                .verify(key, &first.asset_leaf(), commitment.tree_root)
                .expect("asset proof verifies")
        );

        let tap = TapCommitment::from_assets(
            Some(TapCommitmentVersion::V2),
            vec![first.clone(), second.clone()],
        )
        .expect("tap commitment builds");
        assert_eq!(tap.tree_root.sum, 1_000);
        assert!(is_taproot_asset_commitment_script(&tap.tap_leaf_script()));
        assert_eq!(
            parse_tap_leaf_script_root(&tap.tap_leaf_script()).expect("script parses"),
            tap.tree_root
        );

        let sibling = Bytes32([42; 32]);
        let root = tap.tapscript_root(Some(sibling)).expect("root binds");
        tap.validate_tapscript_root(Some(sibling), root)
            .expect("root validates");
    }

    #[test]
    fn duplicate_keys_wrong_groups_and_bad_roots_fail_closed() {
        let mut first = asset(2, 700);
        first.script_key.0[0] = 2;
        let duplicate = first.clone();
        assert!(matches!(
            AssetCommitment::new(vec![first.clone(), duplicate]),
            Err(TaprootCommitmentError::DuplicateAssetCommitmentKey(_))
        ));

        let mut other = asset(3, 300);
        other.script_key.0[0] = 3;
        other.asset_id = Bytes32([99; 32]);
        assert!(matches!(
            AssetCommitment::new(vec![first.clone(), other]),
            Err(TaprootCommitmentError::TapKeyMismatch)
        ));

        let tap = TapCommitment::from_assets(Some(TapCommitmentVersion::V2), vec![first])
            .expect("tap commitment builds");
        let sibling = Bytes32([7; 32]);
        let wrong_unsorted =
            tagged_hash_bip341(b"TapBranch", &[tap.tap_leaf_hash().0, sibling.0].concat());
        assert!(matches!(
            tap.validate_tapscript_root(Some(sibling), wrong_unsorted),
            Err(TaprootCommitmentError::OutputCommitmentMismatch { .. })
        ));
        assert!(matches!(
            tap.tapscript_root(Some(tap.tap_leaf_hash())),
            Err(TaprootCommitmentError::DuplicateTaprootSibling)
        ));
    }

    #[test]
    fn lightning_labs_tap_commitment_script_fixture_parses() {
        let fixture = include_str!(
            "../../../fixtures/lightning-labs/commitment/testdata/tap-commitment-script.hex"
        );
        let script = decode_hex(fixture.trim()).expect("script hex parses");
        assert!(is_taproot_asset_commitment_script(&script));
        let root = parse_tap_leaf_script_root(&script).expect("script root parses");
        assert_eq!(root.sum, 5_001);
        assert_eq!(
            root.hash.to_hex(),
            "1cfee543eac337024a6f13bb5f496e99209207a3792a7489ccc21d4dbbe5ed18"
        );
    }

    fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
        if hex.len() % 2 != 0 {
            return Err("hex length must be even".to_owned());
        }

        hex.as_bytes()
            .chunks(2)
            .map(|chunk| {
                let byte = std::str::from_utf8(chunk)
                    .expect("hex input is str")
                    .to_owned();
                u8::from_str_radix(&byte, 16).map_err(|_| format!("bad hex byte: {byte}"))
            })
            .collect()
    }
}
