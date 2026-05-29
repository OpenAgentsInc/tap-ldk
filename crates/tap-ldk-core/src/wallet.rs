use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{
        AssetAmount, AssetError, AssetLeaf, AssetType, Bytes32, CompressedKey, Genesis,
        derive_hash_sum_root, validate_split_conservation,
    },
    proof::{
        AcceptedBalanceExplanation, ProofError, ProofFile, ProofHistoryEngine, ProofHistoryOutput,
        ProofHistoryRecord, ProofHistoryReplayError, ProofHistoryState, ProofNetwork,
        ProofTransitionKind, ProofValidationContext, VerificationScope,
    },
    tapd_proof::{TapdProofError, TapdProofFileSummary, decode_tapd_proof_file},
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
        self.import_verified_proof_with_tapd(proof, None, None)
    }

    pub fn import_tapd_proof_file(
        &mut self,
        request: TapdProofImportRequest,
    ) -> Result<ImportOutcome, WalletError> {
        let proof_summary =
            decode_tapd_proof_file(&request.tapd_proof_file).map_err(WalletError::TapdProof)?;
        let proof = request.build_semantic_proof(&proof_summary)?;
        self.import_verified_proof_with_tapd(
            proof,
            Some(request.tapd_proof_file),
            Some(proof_summary),
        )
    }

    fn import_verified_proof_with_tapd(
        &mut self,
        proof: ProofFile,
        tapd_proof_file: Option<Vec<u8>>,
        tapd_proof_summary: Option<TapdProofFileSummary>,
    ) -> Result<ImportOutcome, WalletError> {
        let validation_context = match tapd_proof_summary {
            Some(summary) => ProofValidationContext::for_tapd_import(summary),
            None => ProofValidationContext::default(),
        };
        proof
            .verify_semantic_ancestry(&validation_context)
            .map_err(WalletError::Proof)?;
        let encoded = proof.encode().map_err(WalletError::Proof)?;
        let proof_id = proof_id(&proof);
        let proof_history = accepted_wallet_proof_history(&proof_id, &proof)?;
        let proof_hex = encode_hex(&encoded);
        let tapd_raw_proof_file_hex = tapd_proof_file.as_deref().map(encode_hex);
        let tapd_raw_proof_file_digest = tapd_proof_file
            .as_deref()
            .map(|bytes| Bytes32(Sha256::digest(bytes).into()));

        if let Some(existing) = self.proofs.get(&proof_id) {
            if existing.proof_tlv_hex != proof_hex {
                return Err(WalletError::ConflictingProof(proof_id));
            }
            if !existing.matches_proof_history(&proof_history) {
                return Err(WalletError::UnexplainedProofHistory(proof_id));
            }

            if let (Some(existing_tapd), Some(new_tapd)) = (
                existing.tapd_raw_proof_file_hex.as_deref(),
                tapd_raw_proof_file_hex.as_deref(),
            ) {
                if existing_tapd != new_tapd {
                    return Err(WalletError::ConflictingProof(proof_id));
                }
            }

            if tapd_raw_proof_file_hex.is_none() || existing.tapd_raw_proof_file_hex.is_some() {
                return Ok(ImportOutcome::AlreadyPresent { proof_id });
            }

            let mut next = self.clone();
            let stored = next
                .proofs
                .get_mut(&proof_id)
                .ok_or_else(|| WalletError::UnknownProof(proof_id.clone()))?;
            stored.tapd_raw_proof_file_hex = tapd_raw_proof_file_hex;
            stored.tapd_raw_proof_file_digest = tapd_raw_proof_file_digest;
            next.validate()?;
            *self = next;

            return Ok(ImportOutcome::Imported { proof_id });
        }

        let utxo = SpendableAssetUtxo {
            utxo_id: proof_id.clone(),
            proof_id: proof_id.clone(),
            asset_id: proof.asset_id.to_hex(),
            genesis_outpoint: proof.genesis_outpoint.clone(),
            anchor_outpoint: proof.anchor_outpoint.clone(),
            script_key: proof.script_key.to_hex(),
            amount: proof.amount.value(),
            proof_history_output_id: proof_history.output_id.clone(),
            status: UtxoStatus::Spendable,
        };

        self.proofs.insert(
            proof_id.clone(),
            StoredProof {
                proof_id: proof_id.clone(),
                proof_tlv_hex: proof_hex,
                verification_scope: proof.verification_scope.as_str().to_owned(),
                tapd_raw_proof_file_hex,
                tapd_raw_proof_file_digest,
                proof_history_record_id: proof_history.record_id,
                proof_history_output_id: proof_history.output_id,
                proof_history_transition_id: proof_history.transition_id.to_hex(),
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
        self.accepted_wallet_balance_explanation(proof_id)?;
        let stored = self
            .proofs
            .get(proof_id)
            .ok_or_else(|| WalletError::UnknownProof(proof_id.to_owned()))?;
        decode_hex(&stored.proof_tlv_hex)
    }

    pub fn export_tapd_proof_file(&self, proof_id: &str) -> Result<Vec<u8>, WalletError> {
        self.accepted_wallet_balance_explanation(proof_id)?;
        let stored = self
            .proofs
            .get(proof_id)
            .ok_or_else(|| WalletError::UnknownProof(proof_id.to_owned()))?;
        let tapd_hex = stored
            .tapd_raw_proof_file_hex
            .as_deref()
            .ok_or_else(|| WalletError::NoTapdProofFile(proof_id.to_owned()))?;
        decode_hex(tapd_hex)
    }

    pub fn issue_regtest_asset(
        &mut self,
        request: RegtestIssueRequest,
    ) -> Result<RegtestIssueOutcome, WalletError> {
        let proof = request.build_proof()?;
        let asset_id = proof.asset_id.to_hex();
        let outcome = self.import_verified_proof(proof)?;

        Ok(RegtestIssueOutcome {
            status: outcome.status(),
            proof_id: outcome.proof_id().to_owned(),
            asset_id,
            amount: request.amount.value(),
            ticker: request.ticker,
        })
    }

    pub fn send_local_transfer(
        &mut self,
        request: LocalTransferRequest,
    ) -> Result<LocalTransferOutcome, WalletError> {
        if request.amount == AssetAmount::ZERO {
            return Err(WalletError::ZeroTransferAmount);
        }

        let (input_id, input_utxo) = self
            .spendable_utxos
            .iter()
            .find(|(_, utxo)| {
                utxo.status == UtxoStatus::Spendable
                    && utxo.asset_id == request.asset_id.to_hex()
                    && utxo.amount >= request.amount.value()
            })
            .map(|(id, utxo)| (id.clone(), utxo.clone()))
            .ok_or_else(|| WalletError::InsufficientAssetBalance {
                asset_id: request.asset_id.to_hex(),
                requested: request.amount.value(),
            })?;

        let input_proof = self.decode_proof(&input_utxo.proof_id)?;
        input_proof
            .verify_semantic_ancestry(&ProofValidationContext::default())
            .map_err(WalletError::Proof)?;
        let input_amount = AssetAmount::new(input_utxo.amount);
        let change_amount = input_amount
            .checked_sub(request.amount)
            .map_err(WalletError::Asset)?;
        let change_script_key =
            CompressedKey::from_str(&input_utxo.script_key).map_err(WalletError::Asset)?;

        let mut split_outputs = vec![AssetLeaf {
            asset_id: request.asset_id,
            script_key: request.receiver_script_key,
            amount: request.amount,
        }];
        if change_amount != AssetAmount::ZERO {
            split_outputs.push(AssetLeaf {
                asset_id: request.asset_id,
                script_key: change_script_key,
                amount: change_amount,
            });
        }
        validate_split_conservation(input_amount, &split_outputs).map_err(WalletError::Asset)?;

        let receiver_proof =
            transfer_output_proof(&input_proof, request.amount, request.receiver_script_key, 0)?;
        let receiver_proof_id = proof_id(&receiver_proof);
        let receiver_proof_tlv = receiver_proof.encode().map_err(WalletError::Proof)?;

        let mut next = self.clone();
        next.proofs.remove(&input_id);
        next.spendable_utxos.remove(&input_id);

        let change_proof_id = if change_amount == AssetAmount::ZERO {
            None
        } else {
            let change_proof =
                transfer_output_proof(&input_proof, change_amount, change_script_key, 1)?;
            let outcome = next.import_verified_proof(change_proof)?;
            Some(outcome.proof_id().to_owned())
        };
        next.validate()?;
        *self = next;

        Ok(LocalTransferOutcome {
            asset_id: request.asset_id.to_hex(),
            sent_amount: request.amount.value(),
            spent_proof_id: input_id,
            receiver_proof_id,
            receiver_proof_tlv,
            change_proof_id,
            change_amount: change_amount.value(),
        })
    }

    pub fn balances(&self) -> Result<Vec<AssetBalance>, WalletError> {
        let mut totals = BTreeMap::<String, AssetAmount>::new();
        for utxo in self.spendable_utxos.values() {
            if utxo.status != UtxoStatus::Spendable {
                continue;
            }
            let explanation = self.accepted_wallet_balance_explanation(&utxo.proof_id)?;

            let current = totals
                .get(&explanation.asset_id.to_hex())
                .copied()
                .unwrap_or(AssetAmount::ZERO);
            let next = current
                .checked_add(explanation.amount)
                .map_err(WalletError::Asset)?;
            totals.insert(explanation.asset_id.to_hex(), next);
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
            let tapd_summary = if let Some(tapd_hex) = stored.tapd_raw_proof_file_hex.as_deref() {
                let tapd_bytes = decode_hex(tapd_hex)?;
                Some(decode_tapd_proof_file(&tapd_bytes).map_err(WalletError::TapdProof)?)
            } else {
                None
            };
            let validation_context = match tapd_summary.clone() {
                Some(summary) => ProofValidationContext::for_tapd_import(summary),
                None => ProofValidationContext::default(),
            };
            proof
                .verify_semantic_ancestry(&validation_context)
                .map_err(WalletError::Proof)?;
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
            if let Some(tapd_summary) = tapd_summary {
                let expected_digest = stored.tapd_raw_proof_file_digest.ok_or_else(|| {
                    WalletError::StorageInvariant(format!(
                        "proof {key} has tapd proof bytes without digest"
                    ))
                })?;
                if tapd_summary.raw_digest != expected_digest {
                    return Err(WalletError::StorageInvariant(format!(
                        "proof {key} tapd proof digest does not match stored bytes"
                    )));
                }
            } else if stored.tapd_raw_proof_file_digest.is_some() {
                return Err(WalletError::StorageInvariant(format!(
                    "proof {key} has tapd proof digest without bytes"
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
                || utxo.proof_history_output_id != utxo.proof_id
            {
                return Err(WalletError::StorageInvariant(format!(
                    "utxo {key} does not match verified proof {}",
                    utxo.proof_id
                )));
            }
            let stored = self
                .proofs
                .get(&utxo.proof_id)
                .ok_or_else(|| WalletError::UnknownProof(utxo.proof_id.clone()))?;
            let explanation = validate_stored_proof_history(stored, proof)?;
            if explanation.output_id != utxo.proof_history_output_id {
                return Err(WalletError::UnexplainedProofHistory(utxo.proof_id.clone()));
            }
        }

        Ok(())
    }

    fn decode_proof(&self, proof_id: &str) -> Result<ProofFile, WalletError> {
        let encoded = self.export_encoded_proof(proof_id)?;
        ProofFile::decode(&encoded).map_err(WalletError::Proof)
    }

    fn accepted_wallet_balance_explanation(
        &self,
        proof_id: &str,
    ) -> Result<AcceptedBalanceExplanation, WalletError> {
        let stored = self
            .proofs
            .get(proof_id)
            .ok_or_else(|| WalletError::UnknownProof(proof_id.to_owned()))?;
        let utxo = self
            .spendable_utxos
            .get(proof_id)
            .ok_or_else(|| WalletError::ObsoleteProofExport(proof_id.to_owned()))?;
        if utxo.status != UtxoStatus::Spendable || utxo.proof_history_output_id != proof_id {
            return Err(WalletError::UnexplainedProofHistory(proof_id.to_owned()));
        }
        let encoded = decode_hex(&stored.proof_tlv_hex)?;
        let proof = ProofFile::decode(&encoded).map_err(WalletError::Proof)?;
        let explanation = validate_stored_proof_history(stored, &proof)?;
        if explanation.output_id != utxo.proof_history_output_id {
            return Err(WalletError::UnexplainedProofHistory(proof_id.to_owned()));
        }
        Ok(explanation)
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
    pub proof_history_record_id: String,
    pub proof_history_output_id: String,
    pub proof_history_transition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tapd_raw_proof_file_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tapd_raw_proof_file_digest: Option<Bytes32>,
}

impl StoredProof {
    fn matches_proof_history(&self, proof_history: &WalletProofHistory) -> bool {
        self.proof_history_record_id == proof_history.record_id
            && self.proof_history_output_id == proof_history.output_id
            && self.proof_history_transition_id == proof_history.transition_id.to_hex()
    }
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
    pub proof_history_output_id: String,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegtestIssueRequest {
    pub ticker: String,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
}

impl RegtestIssueRequest {
    pub fn openusd(amount: AssetAmount, script_key: CompressedKey) -> Self {
        Self {
            ticker: "OPENUSD".to_owned(),
            amount,
            script_key,
        }
    }

    fn build_proof(&self) -> Result<ProofFile, WalletError> {
        if self.amount == AssetAmount::ZERO {
            return Err(WalletError::ZeroIssuanceAmount);
        }

        let normalized_ticker = self.ticker.trim().to_ascii_uppercase();
        if normalized_ticker.is_empty() {
            return Err(WalletError::InvalidTicker);
        }

        let genesis = Genesis {
            first_prev_out: format!(
                "{}:0",
                tagged_hash(
                    b"tap-ldk:regtest:first-prev-out:v1",
                    normalized_ticker.as_bytes()
                )
                .to_hex()
            ),
            tag: tagged_hash(
                b"tap-ldk:regtest:asset-tag:v1",
                normalized_ticker.as_bytes(),
            ),
            meta_hash: tagged_hash(
                b"tap-ldk:regtest:asset-meta:v1",
                format!("{normalized_ticker}:mock-stablecoin").as_bytes(),
            ),
            output_index: 0,
            asset_type: AssetType::Normal,
        };
        let asset_id = genesis.asset_id();
        let leaf = AssetLeaf {
            asset_id,
            script_key: self.script_key,
            amount: self.amount,
        };
        let tap_asset_root = derive_hash_sum_root(&[leaf]).map_err(WalletError::Asset)?;
        let anchor_hash = tagged_hash(
            b"tap-ldk:regtest:issuance-anchor:v1",
            format!(
                "{}:{}:{}",
                asset_id.to_hex(),
                self.script_key.to_hex(),
                self.amount.value()
            )
            .as_bytes(),
        );

        Ok(ProofFile {
            version: 0,
            asset_id,
            genesis_outpoint: genesis.first_prev_out,
            anchor_outpoint: format!("{}:0", anchor_hash.to_hex()),
            amount: self.amount,
            script_key: self.script_key,
            tap_asset_root,
            verification_scope: VerificationScope::SemanticAncestry,
            network: ProofNetwork::Regtest,
            asset_type: AssetType::Normal,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegtestIssueOutcome {
    pub status: &'static str,
    pub ticker: String,
    pub asset_id: String,
    pub amount: u64,
    pub proof_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TapdProofImportRequest {
    pub asset_id: Bytes32,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub tapd_proof_file: Vec<u8>,
}

impl TapdProofImportRequest {
    fn build_semantic_proof(
        &self,
        proof_summary: &TapdProofFileSummary,
    ) -> Result<ProofFile, WalletError> {
        let leaf = proof_summary.latest_asset_leaf().ok_or(WalletError::Proof(
            ProofError::BrokenAncestry("tapd proof missing asset leaf"),
        ))?;
        let asset_type = AssetType::from_u8(leaf.asset_type).map_err(WalletError::Asset)?;
        let tap_asset_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: self.asset_id,
            script_key: self.script_key,
            amount: self.amount,
        }])
        .map_err(WalletError::Asset)?;

        Ok(ProofFile {
            version: 0,
            asset_id: self.asset_id,
            genesis_outpoint: self.genesis_outpoint.clone(),
            anchor_outpoint: self.anchor_outpoint.clone(),
            amount: self.amount,
            script_key: self.script_key,
            tap_asset_root,
            verification_scope: VerificationScope::SemanticAncestry,
            network: ProofNetwork::Regtest,
            asset_type,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalTransferRequest {
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub receiver_script_key: CompressedKey,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalTransferOutcome {
    pub asset_id: String,
    pub sent_amount: u64,
    pub spent_proof_id: String,
    pub receiver_proof_id: String,
    pub receiver_proof_tlv: Vec<u8>,
    pub change_proof_id: Option<String>,
    pub change_amount: u64,
}

#[derive(Debug)]
pub enum WalletError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Proof(ProofError),
    ProofHistory(ProofHistoryReplayError),
    TapdProof(TapdProofError),
    Asset(AssetError),
    UnsupportedVersion(u32),
    InvalidHexLength,
    InvalidHexByte(String),
    ConflictingProof(String),
    UnknownProof(String),
    NoTapdProofFile(String),
    UnexplainedProofHistory(String),
    ObsoleteProofExport(String),
    ZeroIssuanceAmount,
    ZeroTransferAmount,
    InvalidTicker,
    InsufficientAssetBalance { asset_id: String, requested: u64 },
    StorageInvariant(String),
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "wallet I/O error: {err}"),
            Self::Json(err) => write!(f, "wallet JSON error: {err}"),
            Self::Proof(err) => write!(f, "wallet proof error: {err}"),
            Self::ProofHistory(err) => write!(f, "wallet proof-history error: {err}"),
            Self::TapdProof(err) => write!(f, "wallet tapd proof error: {err}"),
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
            Self::NoTapdProofFile(proof_id) => {
                write!(f, "wallet proof {proof_id} has no tapd proof file")
            }
            Self::UnexplainedProofHistory(proof_id) => {
                write!(
                    f,
                    "wallet proof {proof_id} has no accepted proof-history explanation"
                )
            }
            Self::ObsoleteProofExport(proof_id) => {
                write!(
                    f,
                    "wallet proof {proof_id} is not a current spendable proof"
                )
            }
            Self::ZeroIssuanceAmount => write!(f, "issuance amount must be greater than zero"),
            Self::ZeroTransferAmount => write!(f, "transfer amount must be greater than zero"),
            Self::InvalidTicker => write!(f, "asset ticker cannot be empty"),
            Self::InsufficientAssetBalance {
                asset_id,
                requested,
            } => write!(
                f,
                "insufficient spendable asset balance for {asset_id}; requested {requested}"
            ),
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

#[derive(Debug, Clone, Eq, PartialEq)]
struct WalletProofHistory {
    record_id: String,
    output_id: String,
    transition_id: Bytes32,
}

fn accepted_wallet_proof_history(
    proof_id: &str,
    proof: &ProofFile,
) -> Result<WalletProofHistory, WalletError> {
    let metadata = wallet_proof_history_metadata(proof_id, proof)?;
    let explanation = replay_wallet_proof_history(&metadata, proof)?;
    if explanation.output_id != metadata.output_id
        || explanation.asset_id != proof.asset_id
        || explanation.amount != proof.amount
        || explanation.script_key != proof.script_key
        || explanation.anchor_outpoint != proof.anchor_outpoint
        || explanation.tap_asset_root != proof.tap_asset_root
        || explanation.resulting_state != ProofHistoryState::Accepted
    {
        return Err(WalletError::UnexplainedProofHistory(proof_id.to_owned()));
    }
    Ok(metadata)
}

fn validate_stored_proof_history(
    stored: &StoredProof,
    proof: &ProofFile,
) -> Result<AcceptedBalanceExplanation, WalletError> {
    let metadata = accepted_wallet_proof_history(&stored.proof_id, proof)?;
    if !stored.matches_proof_history(&metadata) {
        return Err(WalletError::UnexplainedProofHistory(
            stored.proof_id.clone(),
        ));
    }
    replay_wallet_proof_history(&metadata, proof)
}

fn replay_wallet_proof_history(
    metadata: &WalletProofHistory,
    proof: &ProofFile,
) -> Result<AcceptedBalanceExplanation, WalletError> {
    let record = ProofHistoryRecord {
        record_id: metadata.record_id.clone(),
        kind: ProofTransitionKind::Issuance,
        virtual_transition_id: metadata.transition_id,
        inputs: Vec::new(),
        outputs: vec![ProofHistoryOutput {
            output_id: metadata.output_id.clone(),
            asset_id: proof.asset_id,
            amount: proof.amount,
            script_key: proof.script_key,
            anchor_outpoint: proof.anchor_outpoint.clone(),
            tap_asset_root: proof.tap_asset_root,
            resulting_state: ProofHistoryState::Accepted,
        }],
    };
    let replay = ProofHistoryEngine::replay(&[record]).map_err(WalletError::ProofHistory)?;
    replay
        .accepted_explanation(&metadata.output_id)
        .cloned()
        .ok_or_else(|| WalletError::UnexplainedProofHistory(metadata.output_id.clone()))
}

fn wallet_proof_history_metadata(
    proof_id: &str,
    proof: &ProofFile,
) -> Result<WalletProofHistory, WalletError> {
    let encoded = proof.encode().map_err(WalletError::Proof)?;
    let mut payload = Vec::with_capacity(proof_id.len() + encoded.len());
    payload.extend_from_slice(proof_id.as_bytes());
    payload.extend_from_slice(&encoded);
    let transition_id = tagged_hash(b"tap-ldk:wallet-proof-history-transition:v1", &payload);

    Ok(WalletProofHistory {
        record_id: format!("wallet-import:{proof_id}"),
        output_id: proof_id.to_owned(),
        transition_id,
    })
}

fn transfer_output_proof(
    input_proof: &ProofFile,
    amount: AssetAmount,
    script_key: CompressedKey,
    output_index: u32,
) -> Result<ProofFile, WalletError> {
    let leaf = AssetLeaf {
        asset_id: input_proof.asset_id,
        script_key,
        amount,
    };
    let tap_asset_root = derive_hash_sum_root(&[leaf]).map_err(WalletError::Asset)?;
    let anchor_hash = tagged_hash(
        b"tap-ldk:regtest:transfer-anchor:v1",
        format!(
            "{}:{}:{}:{}",
            input_proof.anchor_outpoint,
            script_key.to_hex(),
            amount.value(),
            output_index
        )
        .as_bytes(),
    );

    Ok(ProofFile {
        version: 0,
        asset_id: input_proof.asset_id,
        genesis_outpoint: input_proof.genesis_outpoint.clone(),
        anchor_outpoint: format!("{}:{output_index}", anchor_hash.to_hex()),
        amount,
        script_key,
        tap_asset_root,
        verification_scope: VerificationScope::SemanticAncestry,
        network: input_proof.network,
        asset_type: input_proof.asset_type,
    })
}

fn tagged_hash(tag: &[u8], payload: &[u8]) -> Bytes32 {
    let tag_hash = Sha256::digest(tag);
    let mut hasher = Sha256::new();
    hasher.update(tag_hash);
    hasher.update(tag_hash);
    hasher.update(payload);
    Bytes32(hasher.finalize().into())
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
    use crate::tapd_proof;

    fn valid_proof() -> ProofFile {
        let asset_id =
            Bytes32::from_str("7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93")
                .expect("asset id parses");
        let script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("script key parses");
        let amount = AssetAmount::new(1_000_000);
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
    fn tapd_proof_file_import_exports_raw_file_and_survives_restart() {
        let path = temp_wallet_path("tapd_restart");
        let tapd_proof_file = tapd_proof_file_fixture();
        let request = tapd_fixture_import_request(tapd_proof_file.clone());
        let mut wallet = WalletState::default();
        let outcome = wallet
            .import_tapd_proof_file(request)
            .expect("tapd proof file imports");

        assert_eq!(outcome.status(), "imported");
        assert_eq!(
            wallet
                .export_tapd_proof_file(outcome.proof_id())
                .expect("tapd proof exports"),
            tapd_proof_file
        );
        assert_eq!(
            wallet.balances().expect("balances"),
            expected_tapd_balances()
        );

        wallet.save_atomic(&path).expect("wallet saves");
        let loaded = WalletState::load(&path).expect("wallet reloads");
        assert_eq!(
            loaded
                .export_tapd_proof_file(outcome.proof_id())
                .expect("tapd proof exports after reload"),
            tapd_proof_file
        );
        assert_eq!(
            loaded.balances().expect("loaded balances"),
            expected_tapd_balances()
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
    fn wallet_balances_and_exports_require_replayed_history() {
        let mut wallet = WalletState::default();
        let outcome = wallet
            .import_verified_proof(valid_proof())
            .expect("proof imports");
        let proof_id = outcome.proof_id().to_owned();
        let stored = wallet.proofs.get(&proof_id).expect("proof stored");
        assert_eq!(stored.proof_history_output_id, proof_id);
        assert!(!stored.proof_history_record_id.is_empty());
        assert!(!stored.proof_history_transition_id.is_empty());

        wallet
            .proofs
            .get_mut(&proof_id)
            .expect("proof stored")
            .proof_history_output_id = "obsolete-output".to_owned();
        assert!(matches!(
            wallet.balances(),
            Err(WalletError::UnexplainedProofHistory(id)) if id.as_str() == proof_id
        ));
        assert!(matches!(
            wallet.export_encoded_proof(&proof_id),
            Err(WalletError::UnexplainedProofHistory(id)) if id.as_str() == proof_id
        ));

        let mut wallet = WalletState::default();
        let outcome = wallet
            .import_verified_proof(valid_proof())
            .expect("proof imports");
        let proof_id = outcome.proof_id().to_owned();
        wallet.spendable_utxos.remove(&proof_id);
        assert!(matches!(
            wallet.export_encoded_proof(&proof_id),
            Err(WalletError::ObsoleteProofExport(id)) if id.as_str() == proof_id
        ));
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
    fn regtest_issuance_and_local_transfer_conserve_balance() {
        let sender_script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("sender script key parses");
        let receiver_script_key = CompressedKey::from_str(
            "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("receiver script key parses");
        let mut sender = WalletState::default();
        let issue = sender
            .issue_regtest_asset(RegtestIssueRequest::openusd(
                AssetAmount::new(1_000),
                sender_script_key,
            ))
            .expect("issuance succeeds");

        let transfer = sender
            .send_local_transfer(LocalTransferRequest {
                asset_id: Bytes32::from_str(&issue.asset_id).expect("asset id parses"),
                amount: AssetAmount::new(250),
                receiver_script_key,
            })
            .expect("transfer succeeds");
        let mut receiver = WalletState::default();
        receiver
            .import_encoded_proof(&transfer.receiver_proof_tlv)
            .expect("receiver imports proof");

        assert_eq!(
            sender.balances().expect("sender balances"),
            vec![AssetBalance {
                asset_id: issue.asset_id.clone(),
                spendable: 750
            }]
        );
        assert_eq!(
            receiver.balances().expect("receiver balances"),
            vec![AssetBalance {
                asset_id: issue.asset_id,
                spendable: 250
            }]
        );
        assert_eq!(transfer.change_amount, 750);
        assert!(transfer.change_proof_id.is_some());
    }

    #[test]
    fn local_transfer_rejects_wrong_asset_and_malformed_amounts() {
        let script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("script key parses");
        let mut wallet = WalletState::default();
        wallet
            .issue_regtest_asset(RegtestIssueRequest::openusd(
                AssetAmount::new(1_000),
                script_key,
            ))
            .expect("issuance succeeds");

        assert!(matches!(
            wallet.send_local_transfer(LocalTransferRequest {
                asset_id: Bytes32([9; 32]),
                amount: AssetAmount::new(1),
                receiver_script_key: script_key,
            }),
            Err(WalletError::InsufficientAssetBalance { .. })
        ));
        assert!(matches!(
            wallet.send_local_transfer(LocalTransferRequest {
                asset_id: Bytes32([9; 32]),
                amount: AssetAmount::ZERO,
                receiver_script_key: script_key,
            }),
            Err(WalletError::ZeroTransferAmount)
        ));
    }

    #[test]
    fn malformed_and_wrong_anchor_proofs_fail_import() {
        let mut wallet = WalletState::default();
        assert!(matches!(
            wallet.import_encoded_proof(&[0x01, 0x20, 0xaa]),
            Err(WalletError::Proof(ProofError::Tlv(_)))
        ));

        let mut proof = valid_proof();
        proof.anchor_outpoint = "wrong-anchor-without-index".to_owned();
        assert!(matches!(
            wallet.import_verified_proof(proof),
            Err(WalletError::Proof(ProofError::MalformedOutpoint(
                "anchor_outpoint"
            )))
        ));
    }

    #[test]
    fn malformed_and_wrong_anchor_tapd_proofs_fail_before_state_advances() {
        let tapd_proof_file = tapd_proof_file_fixture();
        let request = tapd_fixture_import_request(tapd_proof_file.clone());

        let mut malformed = tapd_proof_file.clone();
        let last = malformed.last_mut().expect("fixture has checksum");
        *last ^= 1;

        let mut wallet = WalletState::default();
        assert!(matches!(
            wallet.import_tapd_proof_file(TapdProofImportRequest {
                asset_id: request.asset_id,
                genesis_outpoint: request.genesis_outpoint.clone(),
                anchor_outpoint: request.anchor_outpoint.clone(),
                amount: request.amount,
                script_key: request.script_key,
                tapd_proof_file: malformed,
            }),
            Err(WalletError::TapdProof(
                TapdProofError::InvalidChecksum { .. }
            ))
        ));
        assert!(wallet.proofs.is_empty());
        assert!(wallet.spendable_utxos.is_empty());

        assert!(matches!(
            wallet.import_tapd_proof_file(TapdProofImportRequest {
                asset_id: request.asset_id,
                genesis_outpoint: request.genesis_outpoint,
                anchor_outpoint: "wrong-anchor-without-index".to_owned(),
                amount: request.amount,
                script_key: request.script_key,
                tapd_proof_file,
            }),
            Err(WalletError::Proof(ProofError::MalformedOutpoint(
                "anchor_outpoint"
            )))
        ));
        assert!(wallet.proofs.is_empty());
        assert!(wallet.spendable_utxos.is_empty());
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

    fn expected_tapd_balances() -> Vec<AssetBalance> {
        vec![AssetBalance {
            asset_id: "941c6b88de2e5c66797831545adabac0b55f8adb836e921c25d2963c65d15bd1".to_owned(),
            spendable: 600,
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

    fn tapd_proof_file_fixture() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/lightning-labs/proof/testdata/proof-file.hex");
        let raw = fs::read_to_string(path).expect("tapd proof fixture reads");
        tapd_proof::decode_hex_text(&raw).expect("tapd proof fixture hex decodes")
    }

    fn tapd_fixture_import_request(tapd_proof_file: Vec<u8>) -> TapdProofImportRequest {
        let summary = decode_tapd_proof_file(&tapd_proof_file).expect("tapd proof summary");
        let leaf = summary.latest_asset_leaf().expect("latest asset leaf");
        TapdProofImportRequest {
            asset_id: leaf.asset_id,
            genesis_outpoint: leaf.genesis.first_prev_out.clone(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            amount: AssetAmount::new(leaf.amount),
            script_key: leaf.script_key,
            tapd_proof_file,
        }
    }
}
