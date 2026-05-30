use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    proof::{ProofAnchorState, ProofError, ProofFile, ProofValidationContext},
    tapd_proof::{TapdProofError, decode_hex_text, decode_tapd_proof_file, encode_hex},
};

pub const PROOF_COURIER_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const PROOF_COURIER_TRANSPORT: &str = "tap-ldk-local-proof-courier-v1";

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofCourierBundle {
    pub version: u32,
    pub transport: String,
    pub proof_id: String,
    pub asset_id: String,
    pub amount: u64,
    pub script_key: String,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub anchor_state: ProofAnchorState,
    pub proof_history_record_id: String,
    pub proof_history_output_id: String,
    pub proof_history_transition_id: String,
    pub proof_tlv_hex: String,
    pub proof_tlv_digest: Bytes32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tapd_raw_proof_file_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tapd_raw_proof_file_digest: Option<Bytes32>,
}

impl ProofCourierBundle {
    pub fn validate(&self) -> Result<ProofCourierValidation, ProofCourierError> {
        if self.version != PROOF_COURIER_BUNDLE_SCHEMA_VERSION {
            return Err(ProofCourierError::UnsupportedVersion(self.version));
        }
        if self.transport != PROOF_COURIER_TRANSPORT {
            return Err(ProofCourierError::UnsupportedTransport(
                self.transport.clone(),
            ));
        }
        require_nonempty("proof_id", &self.proof_id)?;
        require_nonempty("asset_id", &self.asset_id)?;
        require_nonempty("script_key", &self.script_key)?;
        require_nonempty("genesis_outpoint", &self.genesis_outpoint)?;
        require_nonempty("anchor_outpoint", &self.anchor_outpoint)?;
        require_nonempty("proof_history_record_id", &self.proof_history_record_id)?;
        require_nonempty("proof_history_output_id", &self.proof_history_output_id)?;
        require_nonempty(
            "proof_history_transition_id",
            &self.proof_history_transition_id,
        )?;

        let proof_bytes =
            decode_hex_text(&self.proof_tlv_hex).map_err(ProofCourierError::ProofHex)?;
        let proof_digest = sha256(&proof_bytes);
        if proof_digest != self.proof_tlv_digest {
            return Err(ProofCourierError::DigestMismatch("proof_tlv_digest"));
        }
        let proof = ProofFile::decode(&proof_bytes).map_err(ProofCourierError::Proof)?;
        let expected_proof_id = proof_id_for(&proof);
        if self.proof_id != expected_proof_id {
            return Err(ProofCourierError::FieldMismatch("proof_id"));
        }
        if self.asset_id != proof.asset_id.to_hex() {
            return Err(ProofCourierError::FieldMismatch("asset_id"));
        }
        if self.amount != proof.amount.value() {
            return Err(ProofCourierError::FieldMismatch("amount"));
        }
        if self.script_key != proof.script_key.to_hex() {
            return Err(ProofCourierError::FieldMismatch("script_key"));
        }
        if self.genesis_outpoint != proof.genesis_outpoint {
            return Err(ProofCourierError::FieldMismatch("genesis_outpoint"));
        }
        if self.anchor_outpoint != proof.anchor_outpoint {
            return Err(ProofCourierError::FieldMismatch("anchor_outpoint"));
        }

        let tapd_summary = match (
            self.tapd_raw_proof_file_hex.as_deref(),
            self.tapd_raw_proof_file_digest,
        ) {
            (Some(hex), Some(expected_digest)) => {
                let tapd_bytes = decode_hex_text(hex).map_err(ProofCourierError::ProofHex)?;
                let digest = sha256(&tapd_bytes);
                if digest != expected_digest {
                    return Err(ProofCourierError::DigestMismatch(
                        "tapd_raw_proof_file_digest",
                    ));
                }
                let summary =
                    decode_tapd_proof_file(&tapd_bytes).map_err(ProofCourierError::TapdProof)?;
                if summary.raw_digest != expected_digest {
                    return Err(ProofCourierError::DigestMismatch(
                        "tapd_raw_proof_file_digest",
                    ));
                }
                Some(summary)
            }
            (None, None) => None,
            (Some(_), None) => {
                return Err(ProofCourierError::MissingDigest(
                    "tapd_raw_proof_file_digest",
                ));
            }
            (None, Some(_)) => {
                return Err(ProofCourierError::MissingBytes("tapd_raw_proof_file_hex"));
            }
        };

        match tapd_summary.clone() {
            Some(summary) => proof
                .verify_semantic_ancestry(&ProofValidationContext::for_tapd_import(summary))
                .map_err(ProofCourierError::Proof)?,
            None => proof
                .verify_semantic_ancestry(&ProofValidationContext::default())
                .map_err(ProofCourierError::Proof)?,
        };

        Ok(ProofCourierValidation {
            proof,
            proof_digest,
            tapd_proof_file_digest: tapd_summary.map(|summary| summary.raw_digest),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofCourierValidation {
    pub proof: ProofFile,
    pub proof_digest: Bytes32,
    pub tapd_proof_file_digest: Option<Bytes32>,
}

#[derive(Debug)]
pub enum ProofCourierError {
    UnsupportedVersion(u32),
    UnsupportedTransport(String),
    MissingField(&'static str),
    MissingDigest(&'static str),
    MissingBytes(&'static str),
    FieldMismatch(&'static str),
    DigestMismatch(&'static str),
    ProofHex(TapdProofError),
    Proof(ProofError),
    TapdProof(TapdProofError),
}

impl fmt::Display for ProofCourierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported proof courier bundle version {version}")
            }
            Self::UnsupportedTransport(transport) => {
                write!(f, "unsupported proof courier transport {transport}")
            }
            Self::MissingField(field) => write!(f, "proof courier bundle missing {field}"),
            Self::MissingDigest(field) => write!(f, "proof courier bundle missing {field}"),
            Self::MissingBytes(field) => write!(f, "proof courier bundle missing {field}"),
            Self::FieldMismatch(field) => write!(f, "proof courier bundle {field} mismatch"),
            Self::DigestMismatch(field) => write!(f, "proof courier bundle {field} mismatch"),
            Self::ProofHex(err) => write!(f, "proof courier hex error: {err}"),
            Self::Proof(err) => write!(f, "proof courier proof error: {err}"),
            Self::TapdProof(err) => write!(f, "proof courier tapd proof error: {err}"),
        }
    }
}

impl Error for ProofCourierError {}

pub fn proof_id_for(proof: &ProofFile) -> String {
    format!("{}:{}", proof.asset_id.to_hex(), proof.anchor_outpoint)
}

pub fn proof_digest_for(bytes: &[u8]) -> Bytes32 {
    sha256(bytes)
}

pub fn proof_hex_for(bytes: &[u8]) -> String {
    encode_hex(bytes)
}

fn require_nonempty(field: &'static str, value: &str) -> Result<(), ProofCourierError> {
    if value.is_empty() {
        Err(ProofCourierError::MissingField(field))
    } else {
        Ok(())
    }
}

fn sha256(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, str::FromStr};

    use crate::{
        asset::{AssetAmount, AssetLeaf, AssetType, CompressedKey, derive_hash_sum_root},
        proof::{ProofNetwork, VerificationScope},
        tapd_proof::decode_tapd_proof_file,
    };

    use super::*;

    #[test]
    fn native_bundle_validates() {
        let bundle = valid_native_bundle();
        let validation = bundle.validate().expect("bundle validates");

        assert_eq!(validation.proof.asset_id.to_hex(), bundle.asset_id);
        assert_eq!(validation.proof_digest, bundle.proof_tlv_digest);
        assert_eq!(validation.tapd_proof_file_digest, None);
    }

    #[test]
    fn tapd_bundle_validates() {
        let bundle = valid_tapd_bundle();
        let validation = bundle.validate().expect("tapd bundle validates");

        assert_eq!(validation.proof.asset_id.to_hex(), bundle.asset_id);
        assert_eq!(
            validation.tapd_proof_file_digest,
            bundle.tapd_raw_proof_file_digest
        );
    }

    #[test]
    fn bundle_fails_closed_on_manifest_mismatches() {
        let mut wrong_digest = valid_native_bundle();
        wrong_digest.proof_tlv_digest = Bytes32([42; 32]);
        assert!(matches!(
            wrong_digest.validate(),
            Err(ProofCourierError::DigestMismatch("proof_tlv_digest"))
        ));

        let mut wrong_proof_id = valid_native_bundle();
        wrong_proof_id.proof_id = "wrong-proof".to_owned();
        assert!(matches!(
            wrong_proof_id.validate(),
            Err(ProofCourierError::FieldMismatch("proof_id"))
        ));

        let mut wrong_asset = valid_native_bundle();
        wrong_asset.asset_id = Bytes32([99; 32]).to_hex();
        assert!(matches!(
            wrong_asset.validate(),
            Err(ProofCourierError::FieldMismatch("asset_id"))
        ));

        let mut wrong_amount = valid_native_bundle();
        wrong_amount.amount += 1;
        assert!(matches!(
            wrong_amount.validate(),
            Err(ProofCourierError::FieldMismatch("amount"))
        ));

        let mut wrong_owner = valid_native_bundle();
        wrong_owner.script_key =
            "03aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        assert!(matches!(
            wrong_owner.validate(),
            Err(ProofCourierError::FieldMismatch("script_key"))
        ));

        let mut wrong_genesis = valid_native_bundle();
        wrong_genesis.genesis_outpoint =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:0".to_owned();
        assert!(matches!(
            wrong_genesis.validate(),
            Err(ProofCourierError::FieldMismatch("genesis_outpoint"))
        ));

        let mut wrong_anchor = valid_native_bundle();
        wrong_anchor.anchor_outpoint =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:1".to_owned();
        assert!(matches!(
            wrong_anchor.validate(),
            Err(ProofCourierError::FieldMismatch("anchor_outpoint"))
        ));

        let mut malformed_proof_hex = valid_native_bundle();
        malformed_proof_hex.proof_tlv_hex = "abc".to_owned();
        assert!(matches!(
            malformed_proof_hex.validate(),
            Err(ProofCourierError::ProofHex(_))
        ));

        let mut wrong_version = valid_native_bundle();
        wrong_version.version = PROOF_COURIER_BUNDLE_SCHEMA_VERSION + 1;
        assert!(matches!(
            wrong_version.validate(),
            Err(ProofCourierError::UnsupportedVersion(_))
        ));

        let mut wrong_transport = valid_native_bundle();
        wrong_transport.transport = "other".to_owned();
        assert!(matches!(
            wrong_transport.validate(),
            Err(ProofCourierError::UnsupportedTransport(_))
        ));
    }

    #[test]
    fn tapd_bundle_fails_closed_on_digest_mismatch() {
        let mut bundle = valid_tapd_bundle();
        bundle.tapd_raw_proof_file_digest = Some(Bytes32([7; 32]));

        assert!(matches!(
            bundle.validate(),
            Err(ProofCourierError::DigestMismatch(
                "tapd_raw_proof_file_digest"
            ))
        ));

        let mut missing_tapf_digest = valid_tapd_bundle();
        missing_tapf_digest.tapd_raw_proof_file_digest = None;
        assert!(matches!(
            missing_tapf_digest.validate(),
            Err(ProofCourierError::MissingDigest(
                "tapd_raw_proof_file_digest"
            ))
        ));

        let mut missing_tapf_bytes = valid_tapd_bundle();
        missing_tapf_bytes.tapd_raw_proof_file_hex = None;
        assert!(matches!(
            missing_tapf_bytes.validate(),
            Err(ProofCourierError::MissingBytes("tapd_raw_proof_file_hex"))
        ));
    }

    fn valid_native_bundle() -> ProofCourierBundle {
        bundle_for_proof(valid_native_proof(), None)
    }

    fn valid_tapd_bundle() -> ProofCourierBundle {
        let proof_file = tapd_proof_file_fixture();
        let summary = decode_tapd_proof_file(&proof_file).expect("tapd proof file decodes");
        let leaf = summary.latest_asset_leaf().expect("latest asset leaf");
        let proof = tapd_backed_proof(leaf.asset_id, leaf.amount, leaf.script_key, &summary);
        bundle_for_proof(proof, Some(proof_file))
    }

    fn bundle_for_proof(proof: ProofFile, tapd_bytes: Option<Vec<u8>>) -> ProofCourierBundle {
        let encoded = proof.encode().expect("proof encodes");
        let proof_id = proof_id_for(&proof);
        let (tapd_raw_proof_file_hex, tapd_raw_proof_file_digest) = tapd_bytes
            .as_deref()
            .map(|bytes| (Some(proof_hex_for(bytes)), Some(proof_digest_for(bytes))))
            .unwrap_or((None, None));
        ProofCourierBundle {
            version: PROOF_COURIER_BUNDLE_SCHEMA_VERSION,
            transport: PROOF_COURIER_TRANSPORT.to_owned(),
            proof_id: proof_id.clone(),
            asset_id: proof.asset_id.to_hex(),
            amount: proof.amount.value(),
            script_key: proof.script_key.to_hex(),
            genesis_outpoint: proof.genesis_outpoint,
            anchor_outpoint: proof.anchor_outpoint,
            anchor_state: ProofAnchorState::Confirmed,
            proof_history_record_id: format!("wallet-proof:{proof_id}"),
            proof_history_output_id: proof_id,
            proof_history_transition_id: Bytes32([5; 32]).to_hex(),
            proof_tlv_hex: proof_hex_for(&encoded),
            proof_tlv_digest: proof_digest_for(&encoded),
            tapd_raw_proof_file_hex,
            tapd_raw_proof_file_digest,
        }
    }

    fn valid_native_proof() -> ProofFile {
        let asset_id =
            Bytes32::from_str("7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93")
                .expect("asset id parses");
        let script_key = CompressedKey::from_str(
            "02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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

    fn tapd_backed_proof(
        asset_id: Bytes32,
        amount: u64,
        script_key: CompressedKey,
        summary: &crate::tapd_proof::TapdProofFileSummary,
    ) -> ProofFile {
        let latest = summary.latest_asset_leaf().expect("latest asset leaf");
        let amount = AssetAmount::new(amount);
        let tap_asset_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id,
            script_key,
            amount,
        }])
        .expect("root derives");
        ProofFile {
            version: 0,
            asset_id,
            genesis_outpoint: latest.genesis.first_prev_out.clone(),
            anchor_outpoint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1"
                .to_owned(),
            amount,
            script_key,
            tap_asset_root,
            verification_scope: VerificationScope::SemanticAncestry,
            network: ProofNetwork::Regtest,
            asset_type: AssetType::from_u8(latest.asset_type).expect("asset type parses"),
        }
    }

    fn tapd_proof_file_fixture() -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/lightning-labs/proof/testdata/proof-file.hex");
        let raw = fs::read_to_string(path).expect("tapd proof fixture reads");
        decode_hex_text(&raw).expect("tapd proof fixture hex decodes")
    }
}
