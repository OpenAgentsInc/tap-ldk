use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::{AssetAmount, Bytes32, CompressedKey},
    tapd_proof::{TapdProofError, decode_tapd_proof_file},
    wallet::{ImportOutcome, TapdProofImportRequest, WalletError, WalletState},
};

pub const LIVE_TAPD_PROOF_BINDING_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LiveTapdProofBindingRequest {
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub owner_script_key: CompressedKey,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub tapd_proof_file: Vec<u8>,
    pub expected_asset_id: Option<Bytes32>,
    pub expected_proof_digest: Option<Bytes32>,
    pub expected_owner_script_key: Option<CompressedKey>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveTapdProofBindingReport {
    pub schema_version: u32,
    pub source: String,
    pub status: String,
    pub asset_id: String,
    pub amount: u64,
    pub wallet_balance: u64,
    pub proof_id: String,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub owner_script_key: String,
    pub tapd_proof_count: usize,
    pub tapd_proof_file_len: usize,
    pub tapd_proof_file_digest: String,
    pub tapd_final_chain_checksum: String,
    pub verification_scope: String,
    pub fixture_only_path: bool,
    pub semantic_ancestry_validation: String,
}

#[derive(Debug)]
pub enum LiveTapdProofBindingError {
    WrongAssetId {
        expected: Bytes32,
        actual: Bytes32,
    },
    StaleProof {
        expected_digest: Bytes32,
        actual_digest: Bytes32,
    },
    WrongOwnerBinding {
        expected: CompressedKey,
        actual: CompressedKey,
    },
    MissingBoundBalance(String),
    TapdProof(TapdProofError),
    Wallet(WalletError),
}

impl fmt::Display for LiveTapdProofBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongAssetId { expected, actual } => write!(
                f,
                "live tapd proof asset id mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::StaleProof {
                expected_digest,
                actual_digest,
            } => write!(
                f,
                "live tapd proof digest mismatch: expected {}, got {}",
                expected_digest.to_hex(),
                actual_digest.to_hex()
            ),
            Self::WrongOwnerBinding { expected, actual } => write!(
                f,
                "live tapd owner script key mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::MissingBoundBalance(asset_id) => {
                write!(
                    f,
                    "live tapd proof binding produced no balance for {asset_id}"
                )
            }
            Self::TapdProof(err) => write!(f, "live tapd proof decode error: {err}"),
            Self::Wallet(err) => write!(f, "live tapd proof wallet binding error: {err}"),
        }
    }
}

impl Error for LiveTapdProofBindingError {}

impl From<TapdProofError> for LiveTapdProofBindingError {
    fn from(err: TapdProofError) -> Self {
        Self::TapdProof(err)
    }
}

impl From<WalletError> for LiveTapdProofBindingError {
    fn from(err: WalletError) -> Self {
        Self::Wallet(err)
    }
}

pub fn bind_live_tapd_proof(
    wallet: &mut WalletState,
    request: LiveTapdProofBindingRequest,
) -> Result<LiveTapdProofBindingReport, LiveTapdProofBindingError> {
    if let Some(expected) = request.expected_asset_id {
        if expected != request.asset_id {
            return Err(LiveTapdProofBindingError::WrongAssetId {
                expected,
                actual: request.asset_id,
            });
        }
    }

    if let Some(expected) = request.expected_owner_script_key {
        if expected != request.owner_script_key {
            return Err(LiveTapdProofBindingError::WrongOwnerBinding {
                expected,
                actual: request.owner_script_key,
            });
        }
    }

    let proof_summary = decode_tapd_proof_file(&request.tapd_proof_file)?;
    let actual_digest = Bytes32(Sha256::digest(&request.tapd_proof_file).into());
    debug_assert_eq!(actual_digest, proof_summary.raw_digest);

    if let Some(expected_digest) = request.expected_proof_digest {
        if expected_digest != actual_digest {
            return Err(LiveTapdProofBindingError::StaleProof {
                expected_digest,
                actual_digest,
            });
        }
    }

    let outcome = wallet.import_tapd_proof_file(TapdProofImportRequest {
        asset_id: request.asset_id,
        genesis_outpoint: request.genesis_outpoint.clone(),
        anchor_outpoint: request.anchor_outpoint.clone(),
        amount: request.amount,
        script_key: request.owner_script_key,
        tapd_proof_file: request.tapd_proof_file,
    })?;

    let asset_id = request.asset_id.to_hex();
    let wallet_balance = wallet
        .balances()?
        .into_iter()
        .find(|balance| balance.asset_id == asset_id)
        .map(|balance| balance.spendable)
        .ok_or_else(|| LiveTapdProofBindingError::MissingBoundBalance(asset_id.clone()))?;

    Ok(LiveTapdProofBindingReport {
        schema_version: LIVE_TAPD_PROOF_BINDING_SCHEMA_VERSION,
        source: "live-tapd-proof-binding".to_owned(),
        status: outcome_status(&outcome).to_owned(),
        asset_id,
        amount: request.amount.value(),
        wallet_balance,
        proof_id: outcome.proof_id().to_owned(),
        genesis_outpoint: request.genesis_outpoint,
        anchor_outpoint: request.anchor_outpoint,
        owner_script_key: request.owner_script_key.to_hex(),
        tapd_proof_count: proof_summary.proof_count,
        tapd_proof_file_len: proof_summary.raw_len,
        tapd_proof_file_digest: proof_summary.raw_digest.to_hex(),
        tapd_final_chain_checksum: proof_summary.final_chain_checksum.to_hex(),
        verification_scope: "semantic_ancestry".to_owned(),
        fixture_only_path: false,
        semantic_ancestry_validation: "tap_ldk_core_semantic_ancestry".to_owned(),
    })
}

fn outcome_status(outcome: &ImportOutcome) -> &'static str {
    match outcome {
        ImportOutcome::Imported { .. } => "bound",
        ImportOutcome::AlreadyPresent { .. } => "already_bound",
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{
        asset::{AssetAmount, Bytes32, CompressedKey},
        tapd_proof::decode_hex_text,
        wallet::WalletState,
    };

    use super::{LiveTapdProofBindingError, LiveTapdProofBindingRequest, bind_live_tapd_proof};

    fn proof_file_fixture() -> Vec<u8> {
        decode_hex_text(include_str!(
            "../../../fixtures/lightning-labs/proof/testdata/proof-file.hex"
        ))
        .expect("proof fixture decodes")
    }

    fn base_request() -> LiveTapdProofBindingRequest {
        LiveTapdProofBindingRequest {
            asset_id: Bytes32::from_str(
                "941c6b88de2e5c66797831545adabac0b55f8adb836e921c25d2963c65d15bd1",
            )
            .expect("asset id"),
            amount: AssetAmount::new(600),
            owner_script_key: CompressedKey::from_str(
                "0285a7e2dfcad008f54094005db2424aa23431cfb62535950a590957fa6c7cdb27",
            )
            .expect("owner key"),
            genesis_outpoint: "c181733565d1ddc83fbdc36d7ad630f0b1a497a5f4f4d57a0bf664bb95d59905:0"
                .to_owned(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            tapd_proof_file: proof_file_fixture(),
            expected_asset_id: None,
            expected_proof_digest: None,
            expected_owner_script_key: None,
        }
    }

    #[test]
    fn live_tapd_proof_binding_imports_balance_and_raw_proof() {
        let mut wallet = WalletState::default();
        let request = base_request();

        let report = bind_live_tapd_proof(&mut wallet, request).expect("live tapd proof binds");

        assert_eq!(report.status, "bound");
        assert_eq!(report.amount, 600);
        assert_eq!(report.wallet_balance, 600);
        assert!(report.tapd_proof_count > 0);
        assert!(!report.fixture_only_path);
        assert_eq!(
            report.semantic_ancestry_validation,
            "tap_ldk_core_semantic_ancestry"
        );
        assert!(
            wallet
                .export_tapd_proof_file(&report.proof_id)
                .expect("raw tapd proof export")
                .starts_with(b"TAPF")
        );
    }

    #[test]
    fn live_tapd_proof_binding_rejects_wrong_asset_stale_proof_and_owner() {
        let wrong_asset = LiveTapdProofBindingRequest {
            expected_asset_id: Some(Bytes32::ZERO),
            ..base_request()
        };
        assert!(matches!(
            bind_live_tapd_proof(&mut WalletState::default(), wrong_asset),
            Err(LiveTapdProofBindingError::WrongAssetId { .. })
        ));

        let stale_proof = LiveTapdProofBindingRequest {
            expected_proof_digest: Some(Bytes32::ZERO),
            ..base_request()
        };
        assert!(matches!(
            bind_live_tapd_proof(&mut WalletState::default(), stale_proof),
            Err(LiveTapdProofBindingError::StaleProof { .. })
        ));

        let wrong_owner = LiveTapdProofBindingRequest {
            expected_owner_script_key: Some(
                CompressedKey::from_str(
                    "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
                )
                .expect("wrong owner key"),
            ),
            ..base_request()
        };
        assert!(matches!(
            bind_live_tapd_proof(&mut WalletState::default(), wrong_owner),
            Err(LiveTapdProofBindingError::WrongOwnerBinding { .. })
        ));
    }
}
