use std::{collections::BTreeMap, error::Error, fmt, sync::OnceLock};

use sha2::{Digest, Sha256};

use crate::asset::Bytes32;

pub const MAX_TREE_LEVELS: usize = 256;
const COMPRESSED_PROOF_BIT_BYTES: usize = MAX_TREE_LEVELS / 8;
const COMPRESSED_NODE_SIZE: usize = 32 + 8;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MssmtNode {
    pub hash: Bytes32,
    pub sum: u64,
}

impl MssmtNode {
    pub const ZERO: Self = Self {
        hash: Bytes32::ZERO,
        sum: 0,
    };
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MssmtLeaf {
    pub value: Vec<u8>,
    pub sum: u64,
}

impl MssmtLeaf {
    pub fn new(value: impl Into<Vec<u8>>, sum: u64) -> Self {
        Self {
            value: value.into(),
            sum,
        }
    }

    pub fn empty() -> Self {
        Self {
            value: Vec::new(),
            sum: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty() && self.sum == 0
    }

    pub fn node(&self) -> MssmtNode {
        let mut hasher = Sha256::new();
        hasher.update(&self.value);
        hasher.update(self.sum.to_be_bytes());
        MssmtNode {
            hash: Bytes32(hasher.finalize().into()),
            sum: self.sum,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MssmtTree {
    leaves: BTreeMap<Bytes32, MssmtLeaf>,
    levels: Vec<BTreeMap<Bytes32, MssmtNode>>,
    root: MssmtNode,
}

impl Default for MssmtTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MssmtTree {
    pub fn new() -> Self {
        let levels = vec![BTreeMap::new(); MAX_TREE_LEVELS + 1];
        Self {
            leaves: BTreeMap::new(),
            levels,
            root: empty_tree()[0],
        }
    }

    pub fn from_leaves(
        leaves: impl IntoIterator<Item = (Bytes32, MssmtLeaf)>,
    ) -> Result<Self, MssmtError> {
        let mut map = BTreeMap::new();
        for (key, leaf) in leaves {
            if leaf.is_empty() {
                return Err(MssmtError::EmptyLeafInsertion(key));
            }
            if map.insert(key, leaf).is_some() {
                return Err(MssmtError::DuplicateKey(key));
            }
        }

        Self::rebuild(map)
    }

    pub fn root(&self) -> MssmtNode {
        self.root
    }

    pub fn root_children(&self) -> (MssmtNode, MssmtNode) {
        let left_key = Bytes32::ZERO;
        let mut right_key = [0; 32];
        right_key[0] = 1;
        (
            self.levels[1]
                .get(&left_key)
                .copied()
                .unwrap_or_else(|| empty_tree()[1]),
            self.levels[1]
                .get(&Bytes32(right_key))
                .copied()
                .unwrap_or_else(|| empty_tree()[1]),
        )
    }

    pub fn get(&self, key: Bytes32) -> Option<&MssmtLeaf> {
        self.leaves.get(&key)
    }

    pub fn insert(&mut self, key: Bytes32, leaf: MssmtLeaf) -> Result<(), MssmtError> {
        let mut next = self.leaves.clone();
        if leaf.is_empty() {
            next.remove(&key);
        } else {
            next.insert(key, leaf);
        }
        *self = Self::rebuild(next)?;
        Ok(())
    }

    pub fn delete(&mut self, key: Bytes32) -> Result<(), MssmtError> {
        self.insert(key, MssmtLeaf::empty())
    }

    pub fn proof(&self, key: Bytes32) -> MssmtProof {
        let mut siblings = vec![MssmtNode::ZERO; MAX_TREE_LEVELS];
        for depth in 0..MAX_TREE_LEVELS {
            let sibling_key = sibling_prefix(key, depth);
            let sibling = self.levels[depth + 1]
                .get(&sibling_key)
                .copied()
                .unwrap_or_else(|| empty_tree()[depth + 1]);
            siblings[MAX_TREE_LEVELS - 1 - depth] = sibling;
        }

        MssmtProof { siblings }
    }

    fn rebuild(leaves: BTreeMap<Bytes32, MssmtLeaf>) -> Result<Self, MssmtError> {
        let mut levels = vec![BTreeMap::new(); MAX_TREE_LEVELS + 1];

        for (key, leaf) in &leaves {
            levels[MAX_TREE_LEVELS].insert(*key, leaf.node());
        }

        let mut current = levels[MAX_TREE_LEVELS].clone();
        for depth in (0..MAX_TREE_LEVELS).rev() {
            let mut grouped = BTreeMap::<Bytes32, ChildPair>::new();
            for (key, node) in current {
                let parent_key = prefix_key(key, depth);
                let children = grouped.entry(parent_key).or_default();
                if bit_at(key, depth) == 0 {
                    if children.left.replace(node).is_some() {
                        return Err(MssmtError::DuplicateKey(key));
                    }
                } else if children.right.replace(node).is_some() {
                    return Err(MssmtError::DuplicateKey(key));
                }
            }

            let mut parents = BTreeMap::new();
            for (parent_key, children) in grouped {
                let left = children.left.unwrap_or_else(|| empty_tree()[depth + 1]);
                let right = children.right.unwrap_or_else(|| empty_tree()[depth + 1]);
                let parent = branch_node(left, right)?;
                if parent != empty_tree()[depth] {
                    parents.insert(parent_key, parent);
                }
            }

            levels[depth] = parents.clone();
            current = parents;
        }

        let root = levels[0]
            .get(&Bytes32::ZERO)
            .copied()
            .unwrap_or_else(|| empty_tree()[0]);

        Ok(Self {
            leaves,
            levels,
            root,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MssmtProof {
    pub siblings: Vec<MssmtNode>,
}

impl MssmtProof {
    pub fn root_for(&self, key: Bytes32, leaf: &MssmtLeaf) -> Result<MssmtNode, MssmtError> {
        if self.siblings.len() != MAX_TREE_LEVELS {
            return Err(MssmtError::InvalidProofLength {
                expected: MAX_TREE_LEVELS,
                actual: self.siblings.len(),
            });
        }

        let mut current = leaf.node();
        for (index, sibling) in self.siblings.iter().copied().enumerate() {
            let depth = MAX_TREE_LEVELS - 1 - index;
            current = if bit_at(key, depth) == 0 {
                branch_node(current, sibling)?
            } else {
                branch_node(sibling, current)?
            };
        }

        Ok(current)
    }

    pub fn verify(
        &self,
        key: Bytes32,
        leaf: &MssmtLeaf,
        root: MssmtNode,
    ) -> Result<bool, MssmtError> {
        Ok(self.root_for(key, leaf)? == root)
    }

    pub fn compress(&self) -> Result<MssmtCompressedProof, MssmtError> {
        if self.siblings.len() != MAX_TREE_LEVELS {
            return Err(MssmtError::InvalidProofLength {
                expected: MAX_TREE_LEVELS,
                actual: self.siblings.len(),
            });
        }

        let mut bits = Vec::with_capacity(MAX_TREE_LEVELS);
        let mut nodes = Vec::new();
        for (index, node) in self.siblings.iter().copied().enumerate() {
            let empty = empty_tree()[MAX_TREE_LEVELS - index];
            if node == empty {
                bits.push(true);
            } else {
                bits.push(false);
                nodes.push(node);
            }
        }

        Ok(MssmtCompressedProof { bits, nodes })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MssmtCompressedProof {
    pub bits: Vec<bool>,
    pub nodes: Vec<MssmtNode>,
}

impl MssmtCompressedProof {
    pub fn encode(&self) -> Result<Vec<u8>, MssmtError> {
        if self.bits.len() != MAX_TREE_LEVELS {
            return Err(MssmtError::InvalidProofLength {
                expected: MAX_TREE_LEVELS,
                actual: self.bits.len(),
            });
        }
        let node_count = u16::try_from(self.nodes.len()).map_err(|_| MssmtError::TooManyNodes)?;
        let mut out = Vec::with_capacity(2 + self.nodes.len() * COMPRESSED_NODE_SIZE + 32);
        out.extend_from_slice(&node_count.to_be_bytes());
        for node in &self.nodes {
            out.extend_from_slice(&node.hash.0);
            out.extend_from_slice(&node.sum.to_be_bytes());
        }
        out.extend_from_slice(&pack_bits(&self.bits));
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, MssmtError> {
        if bytes.len() < 2 + COMPRESSED_PROOF_BIT_BYTES {
            return Err(MssmtError::TruncatedProof);
        }
        let node_count = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        let node_bytes = node_count
            .checked_mul(COMPRESSED_NODE_SIZE)
            .ok_or(MssmtError::TooManyNodes)?;
        let expected_len = 2 + node_bytes + COMPRESSED_PROOF_BIT_BYTES;
        if bytes.len() != expected_len {
            return Err(MssmtError::InvalidCompressedProofLength {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        let mut cursor = 2;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let mut hash = [0; 32];
            hash.copy_from_slice(&bytes[cursor..cursor + 32]);
            cursor += 32;
            let mut sum = [0; 8];
            sum.copy_from_slice(&bytes[cursor..cursor + 8]);
            cursor += 8;
            nodes.push(MssmtNode {
                hash: Bytes32(hash),
                sum: u64::from_be_bytes(sum),
            });
        }

        let bits = unpack_bits(&bytes[cursor..cursor + COMPRESSED_PROOF_BIT_BYTES]);
        let expected_nodes = bits.iter().filter(|bit| !**bit).count();
        if expected_nodes != nodes.len() {
            return Err(MssmtError::CompressedProofNodeCount {
                expected: expected_nodes,
                actual: nodes.len(),
            });
        }

        Ok(Self { bits, nodes })
    }

    pub fn decompress(&self) -> Result<MssmtProof, MssmtError> {
        if self.bits.len() != MAX_TREE_LEVELS {
            return Err(MssmtError::InvalidProofLength {
                expected: MAX_TREE_LEVELS,
                actual: self.bits.len(),
            });
        }

        let expected_nodes = self.bits.iter().filter(|bit| !**bit).count();
        if expected_nodes != self.nodes.len() {
            return Err(MssmtError::CompressedProofNodeCount {
                expected: expected_nodes,
                actual: self.nodes.len(),
            });
        }

        let mut next_node = 0;
        let mut siblings = Vec::with_capacity(MAX_TREE_LEVELS);
        for (index, bit) in self.bits.iter().copied().enumerate() {
            if bit {
                siblings.push(empty_tree()[MAX_TREE_LEVELS - index]);
            } else {
                siblings.push(self.nodes[next_node]);
                next_node += 1;
            }
        }

        Ok(MssmtProof { siblings })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MssmtError {
    AmountOverflow,
    DuplicateKey(Bytes32),
    EmptyLeafInsertion(Bytes32),
    InvalidProofLength { expected: usize, actual: usize },
    InvalidCompressedProofLength { expected: usize, actual: usize },
    TruncatedProof,
    TooManyNodes,
    CompressedProofNodeCount { expected: usize, actual: usize },
}

impl fmt::Display for MssmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmountOverflow => write!(f, "MS-SMT amount overflow"),
            Self::DuplicateKey(key) => write!(f, "duplicate MS-SMT key {}", key.to_hex()),
            Self::EmptyLeafInsertion(key) => {
                write!(f, "cannot insert empty MS-SMT leaf at {}", key.to_hex())
            }
            Self::InvalidProofLength { expected, actual } => {
                write!(
                    f,
                    "invalid MS-SMT proof length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidCompressedProofLength { expected, actual } => {
                write!(
                    f,
                    "invalid compressed MS-SMT proof length: expected {expected}, got {actual}"
                )
            }
            Self::TruncatedProof => write!(f, "compressed MS-SMT proof is truncated"),
            Self::TooManyNodes => write!(f, "compressed MS-SMT proof has too many nodes"),
            Self::CompressedProofNodeCount { expected, actual } => write!(
                f,
                "compressed MS-SMT proof node count mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl Error for MssmtError {}

#[derive(Debug, Default)]
struct ChildPair {
    left: Option<MssmtNode>,
    right: Option<MssmtNode>,
}

fn branch_node(left: MssmtNode, right: MssmtNode) -> Result<MssmtNode, MssmtError> {
    let sum = left
        .sum
        .checked_add(right.sum)
        .ok_or(MssmtError::AmountOverflow)?;
    let mut hasher = Sha256::new();
    hasher.update(left.hash.0);
    hasher.update(right.hash.0);
    hasher.update(sum.to_be_bytes());
    Ok(MssmtNode {
        hash: Bytes32(hasher.finalize().into()),
        sum,
    })
}

pub fn empty_root() -> MssmtNode {
    empty_tree()[0]
}

fn empty_tree() -> &'static [MssmtNode] {
    static EMPTY_TREE: OnceLock<Vec<MssmtNode>> = OnceLock::new();
    EMPTY_TREE.get_or_init(|| {
        let mut nodes = vec![MssmtNode::ZERO; MAX_TREE_LEVELS + 1];
        nodes[MAX_TREE_LEVELS] = MssmtLeaf::empty().node();
        for depth in (0..MAX_TREE_LEVELS).rev() {
            nodes[depth] = branch_node(nodes[depth + 1], nodes[depth + 1])
                .expect("empty MS-SMT cannot overflow");
        }
        nodes
    })
}

fn bit_at(key: Bytes32, depth: usize) -> u8 {
    debug_assert!(depth < MAX_TREE_LEVELS);
    (key.0[depth / 8] >> (depth % 8)) & 1
}

fn prefix_key(key: Bytes32, prefix_bits: usize) -> Bytes32 {
    debug_assert!(prefix_bits <= MAX_TREE_LEVELS);
    let mut out = key.0;
    for (byte_index, byte) in out.iter_mut().enumerate() {
        let byte_start = byte_index * 8;
        let byte_end = byte_start + 8;
        if prefix_bits >= byte_end {
            continue;
        }
        if prefix_bits <= byte_start {
            *byte = 0;
            continue;
        }

        let keep_bits = prefix_bits - byte_start;
        let mask = (1u16 << keep_bits) as u8 - 1;
        *byte &= mask;
    }
    Bytes32(out)
}

fn sibling_prefix(key: Bytes32, depth: usize) -> Bytes32 {
    let mut sibling = prefix_key(key, depth + 1);
    sibling.0[depth / 8] ^= 1 << (depth % 8);
    sibling
}

fn pack_bits(bits: &[bool]) -> [u8; COMPRESSED_PROOF_BIT_BYTES] {
    debug_assert_eq!(bits.len(), MAX_TREE_LEVELS);
    let mut packed = [0; COMPRESSED_PROOF_BIT_BYTES];
    for (index, bit) in bits.iter().copied().enumerate() {
        if bit {
            packed[index / 8] |= 1 << (index % 8);
        }
    }
    packed
}

fn unpack_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for byte in bytes {
        for bit in 0..8 {
            bits.push(((byte >> bit) & 1) == 1);
        }
    }
    bits
}

#[cfg(test)]
mod tests {
    use std::{path::Path, str::FromStr};

    use serde::Deserialize;

    use super::*;

    #[test]
    fn empty_tree_root_is_stable() {
        assert_eq!(
            empty_root().hash.to_hex(),
            "b1e8e8f2dc3b266452988cfe169aa73be25405eeead02ab5dd6b3c6fd0ca8d67"
        );
        assert_eq!(empty_root().sum, 0);
    }

    #[test]
    fn inclusion_and_exclusion_proofs_round_trip() {
        let key_a = Bytes32([1; 32]);
        let key_b = Bytes32([2; 32]);
        let tree = MssmtTree::from_leaves([
            (key_a, MssmtLeaf::new(b"alice".to_vec(), 7)),
            (key_b, MssmtLeaf::new(b"bob".to_vec(), 11)),
        ])
        .expect("tree builds");

        assert_eq!(tree.root().sum, 18);

        let proof = tree.proof(key_a);
        let leaf = tree.get(key_a).expect("leaf exists");
        assert!(
            proof
                .verify(key_a, leaf, tree.root())
                .expect("proof verifies")
        );

        let compressed = proof.compress().expect("proof compresses");
        let encoded = compressed.encode().expect("proof encodes");
        let decoded = MssmtCompressedProof::decode(&encoded).expect("proof decodes");
        assert_eq!(decoded, compressed);
        let decompressed = decoded.decompress().expect("proof decompresses");
        assert!(
            decompressed
                .verify(key_a, leaf, tree.root())
                .expect("decoded proof verifies")
        );

        let missing_key = Bytes32([3; 32]);
        let exclusion = tree.proof(missing_key);
        assert!(
            exclusion
                .verify(missing_key, &MssmtLeaf::empty(), tree.root())
                .expect("exclusion verifies")
        );
    }

    #[test]
    fn overflow_and_malformed_compressed_proofs_fail_closed() {
        let err = MssmtTree::from_leaves([
            (Bytes32([1; 32]), MssmtLeaf::new(b"max".to_vec(), u64::MAX)),
            (Bytes32([2; 32]), MssmtLeaf::new(b"one".to_vec(), 1)),
        ])
        .expect_err("overflow rejected");
        assert_eq!(err, MssmtError::AmountOverflow);

        let mut bad = vec![0, 1];
        bad.extend_from_slice(&[0; 32]);
        assert!(matches!(
            MssmtCompressedProof::decode(&bad),
            Err(MssmtError::InvalidCompressedProofLength { .. })
        ));

        let mut count_mismatch = MssmtCompressedProof {
            bits: vec![true; MAX_TREE_LEVELS],
            nodes: vec![MssmtNode::ZERO],
        };
        assert!(matches!(
            count_mismatch.decompress(),
            Err(MssmtError::CompressedProofNodeCount {
                expected: 0,
                actual: 1
            })
        ));
        count_mismatch.bits.pop();
        assert!(matches!(
            count_mismatch.encode(),
            Err(MssmtError::InvalidProofLength { .. })
        ));
    }

    #[test]
    fn lightning_labs_mssmt_fixture_root_and_proofs_verify() {
        let fixture = load_lightning_labs_fixture();
        let case = fixture
            .valid_test_cases
            .first()
            .expect("fixture has a valid case");
        let tree = MssmtTree::from_leaves(fixture.all_tree_leaves.iter().map(|leaf| {
            (
                Bytes32::from_str(&leaf.key).expect("key parses"),
                MssmtLeaf::new(
                    decode_hex(&leaf.node.value).expect("value hex parses"),
                    leaf.node.sum.parse::<u64>().expect("sum parses"),
                ),
            )
        }))
        .expect("fixture tree builds");
        let root = tree.root();

        assert_eq!(root.hash.to_hex(), case.root_hash);
        assert_eq!(root.sum.to_string(), case.root_sum);

        for fixture_proof in case.inclusion_proofs.iter().take(3) {
            let key = Bytes32::from_str(&fixture_proof.proof_key).expect("proof key parses");
            let compressed_bytes =
                decode_hex(&fixture_proof.compressed_proof).expect("proof hex parses");
            let compressed =
                MssmtCompressedProof::decode(&compressed_bytes).expect("proof decodes");
            assert_eq!(
                compressed.encode().expect("proof re-encodes"),
                compressed_bytes
            );
            let proof = compressed.decompress().expect("proof decompresses");
            let leaf = tree.get(key).expect("inclusion proof key has a leaf");
            assert!(proof.verify(key, leaf, root).expect("inclusion verifies"));
        }

        for fixture_proof in case.exclusion_proofs.iter().take(2) {
            let key = Bytes32::from_str(&fixture_proof.proof_key).expect("proof key parses");
            let compressed_bytes =
                decode_hex(&fixture_proof.compressed_proof).expect("proof hex parses");
            let compressed =
                MssmtCompressedProof::decode(&compressed_bytes).expect("proof decodes");
            assert_eq!(
                compressed.encode().expect("proof re-encodes"),
                compressed_bytes
            );
            let proof = compressed.decompress().expect("proof decompresses");
            assert!(tree.get(key).is_none());
            assert!(
                proof
                    .verify(key, &MssmtLeaf::empty(), root)
                    .expect("exclusion verifies")
            );
        }
    }

    #[derive(Debug, Deserialize)]
    struct LightningLabsFixture {
        all_tree_leaves: Vec<FixtureLeaf>,
        valid_test_cases: Vec<FixtureCase>,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureLeaf {
        key: String,
        node: FixtureNode,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureNode {
        value: String,
        sum: String,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureCase {
        root_hash: String,
        root_sum: String,
        inclusion_proofs: Vec<FixtureProof>,
        exclusion_proofs: Vec<FixtureProof>,
    }

    #[derive(Debug, Deserialize)]
    struct FixtureProof {
        proof_key: String,
        compressed_proof: String,
    }

    fn load_lightning_labs_fixture() -> LightningLabsFixture {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/lightning-labs/mssmt/testdata/mssmt_tree_proofs.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        serde_json::from_str(&raw).expect("Lightning Labs MS-SMT fixture parses")
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
