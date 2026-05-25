use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::{
    asset::{AssetAmount, AssetError, Bytes32, CompressedKey},
    proof::{ProofError, ProofFile},
};

pub const WALLET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletState {
    pub version: u32,
    pub metadata: WalletMetadata,
    pub proofs: BTreeMap<String, StoredProof>,
    pub spendable_utxos: BTreeMap<String, SpendableAssetUtxo>,
    pub pending_operations: Vec<PendingOperation>,
}

impl Default for WalletState {
    fn default() -> Self {
        Self {
            version: WALLET_SCHEMA_VERSION,
            metadata: WalletMetadata::default(),
            proofs: BTreeMap::new(),
            spendable_utxos: BTreeMap::new(),
            pending_operations: Vec::new(),
        }
    }
}

impl WalletState {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, WalletError> {
        let raw = fs::read_to_string(path.as_ref()).map_err(WalletError::Io)?;
        let wallet = serde_json::from_str::<Self>(&raw).map_err(WalletError::Json)?;
        wallet.validate()?;
        Ok(wallet)
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, WalletError> {
        match Self::load(path.as_ref()) {
            Ok(wallet) => Ok(wallet),
            Err(WalletError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(err) => Err(err),
        }
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), WalletError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(WalletError::Io)?;
            }
        }

        let raw = serde_json::to_vec_pretty(self).map_err(WalletError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(WalletError::Io)?;
        fs::rename(&temp_path, path).map_err(WalletError::Io)?;
        Ok(())
    }

    pub fn import_verified_proof(
        &mut self,
        proof: ProofFile,
    ) -> Result<ImportOutcome, WalletError> {
        proof.verify_bounded_anchor().map_err(WalletError::Proof)?;
        let encoded = proof.encode().map_err(WalletError::Proof)?;
        let proof_id = proof_id(&proof);
        let proof_hex = encode_hex(&encoded);

        if let Some(existing) = self.proofs.get(&proof_id) {
            if existing.proof_tlv_hex != proof_hex {
                return Err(WalletError::ConflictingProof(proof_id));
            }

            return Ok(ImportOutcome::AlreadyPresent { proof_id });
        }

        let utxo = SpendableAssetUtxo {
            utxo_id: proof_id.clone(),
            proof_id: proof_id.clone(),
            asset_id: proof.asset_id.to_hex(),
            genesis_outpoint: proof.genesis_outpoint.clone(),
            anchor_outpoint: proof.anchor_outpoint.clone(),
            script_key: proof.script_key.to_hex(),
            amount: proof.amount.value(),
            status: UtxoStatus::Spendable,
        };

        self.proofs.insert(
            proof_id.clone(),
            StoredProof {
                proof_id: proof_id.clone(),
                proof_tlv_hex: proof_hex,
                verification_scope: proof.verification_scope.as_str().to_owned(),
            },
        );
        self.spendable_utxos.insert(proof_id.clone(), utxo);
        self.validate()?;

        Ok(ImportOutcome::Imported { proof_id })
    }

    pub fn import_encoded_proof(&mut self, bytes: &[u8]) -> Result<ImportOutcome, WalletError> {
        let proof = ProofFile::decode(bytes).map_err(WalletError::Proof)?;
        self.import_verified_proof(proof)
    }

    pub fn export_encoded_proof(&self, proof_id: &str) -> Result<Vec<u8>, WalletError> {
        let stored = self
            .proofs
            .get(proof_id)
            .ok_or_else(|| WalletError::UnknownProof(proof_id.to_owned()))?;
        decode_hex(&stored.proof_tlv_hex)
    }

    pub fn balances(&self) -> Result<Vec<AssetBalance>, WalletError> {
        let mut totals = BTreeMap::<String, AssetAmount>::new();
        for utxo in self.spendable_utxos.values() {
            if utxo.status != UtxoStatus::Spendable {
                continue;
            }

            let current = totals
                .get(&utxo.asset_id)
                .copied()
                .unwrap_or(AssetAmount::ZERO);
            let next = current
                .checked_add(AssetAmount::new(utxo.amount))
                .map_err(WalletError::Asset)?;
            totals.insert(utxo.asset_id.clone(), next);
        }

        Ok(totals
            .into_iter()
            .map(|(asset_id, amount)| AssetBalance {
                asset_id,
                spendable: amount.value(),
            })
            .collect())
    }

    pub fn validate(&self) -> Result<(), WalletError> {
        if self.version != WALLET_SCHEMA_VERSION {
            return Err(WalletError::UnsupportedVersion(self.version));
        }

        let mut decoded_proofs = BTreeMap::<String, ProofFile>::new();
        for (key, stored) in &self.proofs {
            if key != &stored.proof_id {
                return Err(WalletError::StorageInvariant(format!(
                    "proof map key {key} does not match proof_id {}",
                    stored.proof_id
                )));
            }

            let encoded = decode_hex(&stored.proof_tlv_hex)?;
            let proof = ProofFile::decode(&encoded).map_err(WalletError::Proof)?;
            proof.verify_bounded_anchor().map_err(WalletError::Proof)?;
            if proof_id(&proof) != *key {
                return Err(WalletError::StorageInvariant(format!(
                    "proof key {key} does not match encoded proof"
                )));
            }
            if proof.verification_scope.as_str() != stored.verification_scope {
                return Err(WalletError::StorageInvariant(format!(
                    "proof {key} verification scope does not match encoded proof"
                )));
            }

            decoded_proofs.insert(key.clone(), proof);
        }

        for (key, utxo) in &self.spendable_utxos {
            if key != &utxo.utxo_id {
                return Err(WalletError::StorageInvariant(format!(
                    "utxo map key {key} does not match utxo_id {}",
                    utxo.utxo_id
                )));
            }

            let proof = decoded_proofs
                .get(&utxo.proof_id)
                .ok_or_else(|| WalletError::UnknownProof(utxo.proof_id.clone()))?;
            let asset_id = Bytes32::from_str(&utxo.asset_id).map_err(WalletError::Asset)?;
            let script_key =
                CompressedKey::from_str(&utxo.script_key).map_err(WalletError::Asset)?;

            if asset_id != proof.asset_id
                || utxo.genesis_outpoint != proof.genesis_outpoint
                || utxo.anchor_outpoint != proof.anchor_outpoint
                || script_key != proof.script_key
                || utxo.amount != proof.amount.value()
            {
                return Err(WalletError::StorageInvariant(format!(
                    "utxo {key} does not match verified proof {}",
                    utxo.proof_id
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct WalletMetadata {
    pub implementation: String,
    pub schema: String,
}

impl Default for WalletMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk experimental wallet".to_owned(),
            schema: "bounded-regtest-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredProof {
    pub proof_id: String,
    pub proof_tlv_hex: String,
    pub verification_scope: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpendableAssetUtxo {
    pub utxo_id: String,
    pub proof_id: String,
    pub asset_id: String,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub script_key: String,
    pub amount: u64,
    pub status: UtxoStatus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtxoStatus {
    Spendable,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PendingOperation {
    pub operation_id: String,
    pub kind: String,
    pub asset_id: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetBalance {
    pub asset_id: String,
    pub spendable: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ImportOutcome {
    Imported { proof_id: String },
    AlreadyPresent { proof_id: String },
}

impl ImportOutcome {
    pub fn proof_id(&self) -> &str {
        match self {
            Self::Imported { proof_id } | Self::AlreadyPresent { proof_id } => proof_id,
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Self::Imported { .. } => "imported",
            Self::AlreadyPresent { .. } => "already_present",
        }
    }
}

#[derive(Debug)]
pub enum WalletError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Proof(ProofError),
    Asset(AssetError),
    UnsupportedVersion(u32),
    InvalidHexLength,
    InvalidHexByte(String),
    ConflictingProof(String),
    UnknownProof(String),
    StorageInvariant(String),
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "wallet I/O error: {err}"),
            Self::Json(err) => write!(f, "wallet JSON error: {err}"),
            Self::Proof(err) => write!(f, "wallet proof error: {err}"),
            Self::Asset(err) => write!(f, "wallet asset error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported wallet schema version {version}")
            }
            Self::InvalidHexLength => write!(f, "wallet hex value has odd length"),
            Self::InvalidHexByte(value) => write!(f, "invalid wallet hex byte: {value}"),
            Self::ConflictingProof(proof_id) => {
                write!(f, "conflicting proof already exists for {proof_id}")
            }
            Self::UnknownProof(proof_id) => write!(f, "unknown wallet proof: {proof_id}"),
            Self::StorageInvariant(message) => {
                write!(f, "wallet storage invariant failed: {message}")
            }
        }
    }
}

impl Error for WalletError {}

fn proof_id(proof: &ProofFile) -> String {
    format!("{}:{}", proof.asset_id.to_hex(), proof.anchor_outpoint)
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wallet.json".to_owned());
    path.with_file_name(format!("{file_name}.tmp"))
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

fn decode_hex(hex: &str) -> Result<Vec<u8>, WalletError> {
    if hex.len() % 2 != 0 {
        return Err(WalletError::InvalidHexLength);
    }

    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let value = std::str::from_utf8(chunk)
                .expect("hex input is str")
                .to_owned();
            u8::from_str_radix(&value, 16).map_err(|_| WalletError::InvalidHexByte(value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        asset::{AssetAmount, Bytes32, CompressedKey, RootHashSum},
        proof::VerificationScope,
    };

    fn valid_proof() -> ProofFile {
        ProofFile {
            version: 0,
            asset_id: Bytes32::from_str(
                "7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93",
            )
            .expect("asset id parses"),
            genesis_outpoint: "9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0"
                .to_owned(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            amount: AssetAmount::new(1_000_000),
            script_key: CompressedKey::from_str(
                "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
            )
            .expect("script key parses"),
            tap_asset_root: RootHashSum {
                hash: Bytes32::from_str(
                    "3ed3ea50146d815594b28dc6bbff67dadf078ee245c10a1a06faeb5e8ff9c3c2",
                )
                .expect("root hash parses"),
                sum: AssetAmount::new(1_000_000),
            },
            verification_scope: VerificationScope::BoundedAnchorOnly,
        }
    }

    #[test]
    fn verified_proof_import_persists_across_restart() {
        let path = temp_wallet_path("restart");
        let mut wallet = WalletState::default();
        let outcome = wallet
            .import_verified_proof(valid_proof())
            .expect("proof imports");
        assert_eq!(outcome.status(), "imported");
        assert_eq!(wallet.balances().expect("balances"), expected_balances());

        wallet.save_atomic(&path).expect("wallet saves");
        let loaded = WalletState::load(&path).expect("wallet loads");

        assert_eq!(
            loaded.balances().expect("loaded balances"),
            expected_balances()
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn duplicate_import_does_not_double_count_balance() {
        let mut wallet = WalletState::default();
        wallet
            .import_verified_proof(valid_proof())
            .expect("first import");
        let second = wallet
            .import_verified_proof(valid_proof())
            .expect("duplicate import is idempotent");

        assert_eq!(second.status(), "already_present");
        assert_eq!(wallet.balances().expect("balances"), expected_balances());
    }

    #[test]
    fn invalid_proof_is_rejected_before_state_advances() {
        let mut wallet = WalletState::default();
        let mut proof = valid_proof();
        proof.tap_asset_root.sum = AssetAmount::new(999_999);

        assert!(matches!(
            wallet.import_verified_proof(proof),
            Err(WalletError::Proof(ProofError::RootSumMismatch { .. }))
        ));
        assert!(wallet.proofs.is_empty());
        assert!(wallet.spendable_utxos.is_empty());
    }

    #[test]
    fn unsupported_schema_version_fails_closed() {
        let mut wallet = WalletState::default();
        wallet.version = WALLET_SCHEMA_VERSION + 1;

        assert!(matches!(
            wallet.validate(),
            Err(WalletError::UnsupportedVersion(version)) if version == WALLET_SCHEMA_VERSION + 1
        ));
    }

    #[test]
    fn tampered_utxo_amount_fails_validation() {
        let mut wallet = WalletState::default();
        let outcome = wallet
            .import_verified_proof(valid_proof())
            .expect("proof imports");
        wallet
            .spendable_utxos
            .get_mut(outcome.proof_id())
            .expect("utxo exists")
            .amount += 1;

        assert!(matches!(
            wallet.validate(),
            Err(WalletError::StorageInvariant(message)) if message.contains("does not match verified proof")
        ));
    }

    fn expected_balances() -> Vec<AssetBalance> {
        vec![AssetBalance {
            asset_id: "7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93".to_owned(),
            spendable: 1_000_000,
        }]
    }

    fn temp_wallet_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tap_ldk_wallet_{name}_{}_{}.json",
            std::process::id(),
            nanos
        ))
    }
}
