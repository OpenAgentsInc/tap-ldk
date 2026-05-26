use std::{error::Error, fmt, str::FromStr};

use ripemd::Ripemd160;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use sha2::{Digest, Sha256};

use crate::{
    asset::{Bytes32, CompressedKey},
    mssmt::{MssmtCompressedProof, MssmtError},
    virtual_psbt::SigningDomain,
};

const ZERO_OUTPOINT: &str = "0000000000000000000000000000000000000000000000000000000000000000:0";
const ZERO_ASSET_ID: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const ZERO_SCRIPT_KEY: &str = "000000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetVirtualTransition {
    pub version: u8,
    pub kind: TapVmTransitionKind,
    pub asset_id: Bytes32,
    pub signing_domain: SigningDomain,
    pub inputs: Vec<AssetVirtualInput>,
    pub outputs: Vec<AssetVirtualOutput>,
    pub witnesses: Vec<AssetVirtualWitness>,
}

impl AssetVirtualTransition {
    pub fn issuance(asset_id: Bytes32, amount: u64, script_key: CompressedKey) -> Self {
        Self {
            version: 0,
            kind: TapVmTransitionKind::Issuance,
            asset_id,
            signing_domain: SigningDomain::TaprootAssets,
            inputs: Vec::new(),
            outputs: vec![AssetVirtualOutput {
                amount,
                script_key,
                output_index: 0,
            }],
            witnesses: Vec::new(),
        }
    }

    pub fn channel_funding(
        asset_id: Bytes32,
        input_amounts: impl IntoIterator<Item = u64>,
        output_amount: u64,
        script_key: CompressedKey,
        witness_context: Bytes32,
    ) -> Self {
        let inputs = input_amounts
            .into_iter()
            .enumerate()
            .map(|(input_index, amount)| AssetVirtualInput {
                amount,
                previous_id: virtual_prev_id(asset_id, input_index as u64, witness_context),
            })
            .collect::<Vec<_>>();
        let witnesses = inputs
            .iter()
            .enumerate()
            .map(|(input_index, _)| {
                AssetVirtualWitness::from_context(
                    b"tap-ldk:virtual-channel-funding-witness:v1",
                    asset_id,
                    input_index as u64,
                    witness_context,
                )
            })
            .collect();

        Self {
            version: 0,
            kind: TapVmTransitionKind::ChannelFunding,
            asset_id,
            signing_domain: SigningDomain::TaprootAssets,
            inputs,
            outputs: vec![AssetVirtualOutput {
                amount: output_amount,
                script_key,
                output_index: 0,
            }],
            witnesses,
        }
    }

    pub fn channel_balance_update(
        kind: TapVmTransitionKind,
        asset_id: Bytes32,
        total_amount: u64,
        local_balance: u64,
        remote_balance: u64,
        asset_nonce: Bytes32,
    ) -> Self {
        Self {
            version: 0,
            kind,
            asset_id,
            signing_domain: SigningDomain::TaprootAssets,
            inputs: vec![AssetVirtualInput {
                amount: total_amount,
                previous_id: virtual_prev_id(asset_id, 0, asset_nonce),
            }],
            outputs: vec![
                AssetVirtualOutput {
                    amount: local_balance,
                    script_key: CompressedKey([2; 33]),
                    output_index: 0,
                },
                AssetVirtualOutput {
                    amount: remote_balance,
                    script_key: CompressedKey([3; 33]),
                    output_index: 1,
                },
            ],
            witnesses: vec![AssetVirtualWitness::from_context(
                b"tap-ldk:virtual-channel-update-witness:v1",
                asset_id,
                0,
                asset_nonce,
            )],
        }
    }

    pub fn validate(&self) -> Result<TapVmValidationSummary, TapVmError> {
        if self.version > 1 {
            return Err(TapVmError::UnsupportedVersion(self.version as u32));
        }
        if self.signing_domain != SigningDomain::TaprootAssets {
            return Err(TapVmError::WrongSigningDomain);
        }

        let output_total = checked_sum(self.outputs.iter().map(|output| output.amount))?;
        if self.outputs.is_empty() {
            return Err(TapVmError::NoOutputs);
        }
        for output in &self.outputs {
            validate_compressed_key(&output.script_key.to_hex())?;
        }

        match self.kind {
            TapVmTransitionKind::Issuance => {
                if !self.inputs.is_empty() || !self.witnesses.is_empty() {
                    return Err(TapVmError::GenesisHasInputs);
                }
                if output_total == 0 {
                    return Err(TapVmError::ZeroAmount);
                }
            }
            TapVmTransitionKind::Transfer
            | TapVmTransitionKind::ChannelFunding
            | TapVmTransitionKind::HtlcSettlement
            | TapVmTransitionKind::Close
            | TapVmTransitionKind::Recovery => {
                if self.inputs.is_empty() {
                    return Err(TapVmError::NoInputs);
                }
                if self.witnesses.len() != self.inputs.len() {
                    return Err(TapVmError::WitnessCountMismatch {
                        expected: self.inputs.len(),
                        actual: self.witnesses.len(),
                    });
                }
                let input_total = checked_sum(self.inputs.iter().map(|input| input.amount))?;
                if input_total != output_total {
                    return Err(TapVmError::AmountNotConserved {
                        inputs: input_total,
                        outputs: output_total,
                    });
                }
                for witness in &self.witnesses {
                    if witness.nonce == Bytes32::ZERO || witness.authorization == Bytes32::ZERO {
                        return Err(TapVmError::EmptyWitness);
                    }
                }
            }
        }

        Ok(TapVmValidationSummary {
            transition_kind: self.kind,
            input_count: self.inputs.len(),
            output_count: self.outputs.len(),
            input_amount: checked_sum(self.inputs.iter().map(|input| input.amount))?,
            output_amount: output_total,
            witness_count: self.witnesses.len(),
            split_root_sum: None,
            script_witnesses_checked: self.witnesses.len(),
        })
    }

    pub fn tx_id(&self) -> Result<Bytes32, TapVmError> {
        self.validate()?;
        Ok(self.digest(b"tap-ldk:asset-virtual-tx:v2"))
    }

    pub fn witness_digest(&self) -> Result<Bytes32, TapVmError> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(b"tap-ldk:asset-virtual-witness:v2");
        hasher.update([self.version]);
        hasher.update([self.kind.as_u8()]);
        hasher.update(self.asset_id.0);
        hasher.update((self.witnesses.len() as u64).to_be_bytes());
        for witness in &self.witnesses {
            hasher.update(witness.nonce.0);
            hasher.update(witness.authorization.0);
        }
        Ok(Bytes32(hasher.finalize().into()))
    }

    fn digest(&self, domain: &[u8]) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update([self.version]);
        hasher.update([self.kind.as_u8()]);
        hasher.update(self.asset_id.0);
        hasher.update((self.inputs.len() as u64).to_be_bytes());
        for input in &self.inputs {
            hasher.update(input.amount.to_be_bytes());
            hasher.update(input.previous_id.0);
        }
        hasher.update((self.outputs.len() as u64).to_be_bytes());
        for output in &self.outputs {
            hasher.update(output.output_index.to_be_bytes());
            hasher.update(output.amount.to_be_bytes());
            hasher.update(output.script_key.0);
        }
        hasher.update((self.witnesses.len() as u64).to_be_bytes());
        for witness in &self.witnesses {
            hasher.update(witness.nonce.0);
            hasher.update(witness.authorization.0);
        }
        Bytes32(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetVirtualInput {
    pub amount: u64,
    pub previous_id: Bytes32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetVirtualOutput {
    pub amount: u64,
    pub script_key: CompressedKey,
    pub output_index: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AssetVirtualWitness {
    pub nonce: Bytes32,
    pub authorization: Bytes32,
}

impl AssetVirtualWitness {
    pub fn from_context(domain: &[u8], asset_id: Bytes32, index: u64, context: Bytes32) -> Self {
        let mut nonce_hasher = Sha256::new();
        nonce_hasher.update(domain);
        nonce_hasher.update(b":nonce");
        nonce_hasher.update(asset_id.0);
        nonce_hasher.update(index.to_be_bytes());
        nonce_hasher.update(context.0);
        let nonce = Bytes32(nonce_hasher.finalize().into());

        let mut auth_hasher = Sha256::new();
        auth_hasher.update(domain);
        auth_hasher.update(b":authorization");
        auth_hasher.update(asset_id.0);
        auth_hasher.update(index.to_be_bytes());
        auth_hasher.update(context.0);
        auth_hasher.update(nonce.0);
        let authorization = Bytes32(auth_hasher.finalize().into());

        Self {
            nonce,
            authorization,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TapVmTransitionKind {
    Issuance,
    Transfer,
    ChannelFunding,
    HtlcSettlement,
    Close,
    Recovery,
}

impl TapVmTransitionKind {
    fn as_u8(self) -> u8 {
        match self {
            Self::Issuance => 0,
            Self::Transfer => 1,
            Self::ChannelFunding => 2,
            Self::HtlcSettlement => 3,
            Self::Close => 4,
            Self::Recovery => 5,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TapVmValidationSummary {
    pub transition_kind: TapVmTransitionKind,
    pub input_count: usize,
    pub output_count: usize,
    pub input_amount: u64,
    pub output_amount: u64,
    pub witness_count: usize,
    pub split_root_sum: Option<u64>,
    pub script_witnesses_checked: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmFixture {
    #[serde(default, deserialize_with = "vec_from_null")]
    pub valid_test_cases: Vec<TapVmFixtureCase>,
    #[serde(default, deserialize_with = "vec_from_null")]
    pub error_test_cases: Vec<TapVmFixtureCase>,
}

impl TapVmFixture {
    pub fn from_json_str(raw: &str) -> Result<Self, TapVmError> {
        serde_json::from_str(raw).map_err(TapVmError::Json)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmFixtureCase {
    pub asset: TapVmAsset,
    #[serde(default)]
    pub split_set: Vec<TapVmSplit>,
    #[serde(default)]
    pub input_set: Vec<TapVmInput>,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub error: Option<String>,
}

impl TapVmFixtureCase {
    pub fn validate(&self) -> Result<TapVmValidationSummary, TapVmError> {
        validate_asset_shape(&self.asset)?;
        let witnesses = self
            .asset
            .prev_witnesses
            .as_deref()
            .ok_or(TapVmError::MissingPrevWitnesses)?;

        if self.input_set.is_empty() {
            return self.validate_genesis(witnesses);
        }

        self.validate_transfer(witnesses)
    }

    fn validate_genesis(
        &self,
        witnesses: &[TapVmPrevWitness],
    ) -> Result<TapVmValidationSummary, TapVmError> {
        if !self.split_set.is_empty() {
            return Err(TapVmError::GenesisHasSplitSet);
        }
        if witnesses.len() != 1 {
            return Err(TapVmError::WitnessCountMismatch {
                expected: 1,
                actual: witnesses.len(),
            });
        }
        if !witnesses[0].prev_id.is_zero() {
            return Err(TapVmError::GenesisPrevIdNotZero);
        }
        if witnesses[0].tx_witness.is_some() || witnesses[0].split_commitment.is_some() {
            return Err(TapVmError::GenesisHasInputs);
        }
        validate_asset_amount_for_type(&self.asset)?;

        Ok(TapVmValidationSummary {
            transition_kind: TapVmTransitionKind::Issuance,
            input_count: 0,
            output_count: 1,
            input_amount: 0,
            output_amount: self.asset.amount,
            witness_count: 0,
            split_root_sum: None,
            script_witnesses_checked: 0,
        })
    }

    fn validate_transfer(
        &self,
        witnesses: &[TapVmPrevWitness],
    ) -> Result<TapVmValidationSummary, TapVmError> {
        if witnesses.len() != self.input_set.len() {
            return Err(TapVmError::WitnessCountMismatch {
                expected: self.input_set.len(),
                actual: witnesses.len(),
            });
        }

        let transition_asset_id = Bytes32::from_str(&witnesses[0].prev_id.asset_id)
            .map_err(TapVmError::MalformedBytes32)?;
        if transition_asset_id == Bytes32::ZERO {
            return Err(TapVmError::ZeroAssetId);
        }

        let mut input_total = 0u64;
        let mut script_witnesses_checked = 0usize;
        for input in &self.input_set {
            validate_asset_shape(&input.asset)?;
            if !same_asset_definition(&self.asset, &input.asset) {
                return Err(TapVmError::AssetDefinitionMismatch);
            }
            let witness = witnesses
                .iter()
                .find(|witness| witness.prev_id == input.prev_id)
                .ok_or(TapVmError::InputPrevIdMismatch)?;
            validate_witness_stack(
                witness
                    .tx_witness
                    .as_ref()
                    .ok_or(TapVmError::MissingWitnessStack)?,
            )?;
            script_witnesses_checked += 1;
            input_total = input_total
                .checked_add(input.asset.amount)
                .ok_or(TapVmError::AmountOverflow)?;
        }

        if self.split_set.is_empty() {
            if self.asset.amount != input_total {
                return Err(TapVmError::AmountNotConserved {
                    inputs: input_total,
                    outputs: self.asset.amount,
                });
            }
            return Ok(TapVmValidationSummary {
                transition_kind: TapVmTransitionKind::Transfer,
                input_count: self.input_set.len(),
                output_count: 1,
                input_amount: input_total,
                output_amount: self.asset.amount,
                witness_count: witnesses.len(),
                split_root_sum: None,
                script_witnesses_checked,
            });
        }

        let root = self
            .asset
            .split_commitment_root
            .as_ref()
            .ok_or(TapVmError::MissingSplitRoot)?;
        if root.sum != input_total {
            return Err(TapVmError::SplitRootSumMismatch {
                expected: input_total,
                actual: root.sum,
            });
        }
        let mut split_total = 0u64;
        let mut top_level_locator_found = false;
        for split in &self.split_set {
            split.validate(&self.asset, transition_asset_id, root)?;
            split_total = split_total
                .checked_add(split.key.amount)
                .ok_or(TapVmError::AmountOverflow)?;
            if split.key.output_index == 0
                && split.key.amount == self.asset.amount
                && same_script_key_xonly(&split.key.script_key, &self.asset.script_key)?
                && split.key.asset_id == transition_asset_id.to_hex()
            {
                top_level_locator_found = true;
            }
        }
        if split_total != input_total {
            return Err(TapVmError::AmountNotConserved {
                inputs: input_total,
                outputs: split_total,
            });
        }
        if !top_level_locator_found {
            return Err(TapVmError::SplitRootLocatorMismatch);
        }

        Ok(TapVmValidationSummary {
            transition_kind: TapVmTransitionKind::Transfer,
            input_count: self.input_set.len(),
            output_count: self.split_set.len(),
            input_amount: input_total,
            output_amount: split_total,
            witness_count: witnesses.len(),
            split_root_sum: Some(root.sum),
            script_witnesses_checked,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmAsset {
    pub version: u32,
    pub genesis_first_prev_out: String,
    pub genesis_tag: String,
    pub genesis_meta_hash: String,
    #[serde(deserialize_with = "u64_from_any")]
    pub genesis_output_index: u64,
    pub genesis_type: u8,
    #[serde(deserialize_with = "u64_from_any")]
    pub amount: u64,
    #[serde(deserialize_with = "u64_from_any")]
    pub lock_time: u64,
    #[serde(deserialize_with = "u64_from_any")]
    pub relative_lock_time: u64,
    pub prev_witnesses: Option<Vec<TapVmPrevWitness>>,
    pub split_commitment_root: Option<TapVmRoot>,
    pub script_version: u32,
    pub script_key: String,
    pub group_key: Option<TapVmGroupKey>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct TapVmPrevId {
    pub out_point: String,
    pub asset_id: String,
    pub script_key: String,
}

impl TapVmPrevId {
    fn is_zero(&self) -> bool {
        self.out_point == ZERO_OUTPOINT
            && self.asset_id == ZERO_ASSET_ID
            && self.script_key == ZERO_SCRIPT_KEY
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmPrevWitness {
    pub prev_id: TapVmPrevId,
    pub tx_witness: Option<Vec<String>>,
    pub split_commitment: Option<TapVmSplitCommitment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmInput {
    pub prev_id: TapVmPrevId,
    pub asset: TapVmAsset,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmSplitCommitment {
    pub proof: String,
    pub root_asset: TapVmAsset,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmRoot {
    pub hash: String,
    #[serde(deserialize_with = "u64_from_any")]
    pub sum: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmGroupKey {
    pub group_key: String,
    pub group_key_sig: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmSplit {
    pub key: TapVmSplitKey,
    pub value: TapVmSplitValue,
}

impl TapVmSplit {
    fn validate(
        &self,
        top_asset: &TapVmAsset,
        asset_id: Bytes32,
        root: &TapVmRoot,
    ) -> Result<(), TapVmError> {
        if self.key.asset_id != asset_id.to_hex() {
            return Err(TapVmError::AssetIdMismatch);
        }
        if self.key.output_index != self.value.output_index {
            return Err(TapVmError::SplitOutputIndexMismatch);
        }
        if self.key.amount != self.value.asset.amount
            || !same_script_key_xonly(&self.key.script_key, &self.value.asset.script_key)?
        {
            return Err(TapVmError::SplitKeyValueMismatch);
        }
        validate_compressed_key(&self.key.script_key)?;
        validate_asset_shape(&self.value.asset)?;
        if !same_asset_definition(top_asset, &self.value.asset) {
            return Err(TapVmError::AssetDefinitionMismatch);
        }

        let witnesses = self
            .value
            .asset
            .prev_witnesses
            .as_deref()
            .ok_or(TapVmError::MissingPrevWitnesses)?;
        if witnesses.len() != 1 {
            return Err(TapVmError::WitnessCountMismatch {
                expected: 1,
                actual: witnesses.len(),
            });
        }
        if !witnesses[0].prev_id.is_zero() || witnesses[0].tx_witness.is_some() {
            return Err(TapVmError::InvalidSplitWitness);
        }
        let split_commitment = witnesses[0]
            .split_commitment
            .as_ref()
            .ok_or(TapVmError::MissingSplitCommitment)?;
        decode_mssmt_proof(&split_commitment.proof)?;
        if !same_asset_definition(top_asset, &split_commitment.root_asset) {
            return Err(TapVmError::AssetDefinitionMismatch);
        }
        let root_asset_root = split_commitment
            .root_asset
            .split_commitment_root
            .as_ref()
            .ok_or(TapVmError::MissingSplitRoot)?;
        if root_asset_root.hash != root.hash || root_asset_root.sum != root.sum {
            return Err(TapVmError::SplitRootMismatch);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmSplitKey {
    #[serde(deserialize_with = "u64_from_any")]
    pub output_index: u64,
    pub asset_id: String,
    pub script_key: String,
    #[serde(deserialize_with = "u64_from_any")]
    pub amount: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TapVmSplitValue {
    pub asset: TapVmAsset,
    #[serde(deserialize_with = "u64_from_any")]
    pub output_index: u64,
}

#[derive(Debug)]
pub enum TapVmError {
    Json(serde_json::Error),
    Mssmt(MssmtError),
    MalformedHex(String),
    MalformedBytes32(crate::asset::AssetError),
    UnsupportedVersion(u32),
    UnsupportedScriptVersion(u32),
    UnsupportedGenesisType(u8),
    MalformedOutpoint(String),
    MalformedKey(String),
    MalformedSignature(String),
    MissingPrevWitnesses,
    MissingWitnessStack,
    EmptyWitness,
    WrongSigningDomain,
    NoInputs,
    NoOutputs,
    GenesisHasInputs,
    GenesisHasSplitSet,
    GenesisPrevIdNotZero,
    ZeroAmount,
    ZeroAssetId,
    CollectibleAmountInvalid(u64),
    WitnessCountMismatch { expected: usize, actual: usize },
    InputPrevIdMismatch,
    AssetDefinitionMismatch,
    AssetIdMismatch,
    AmountOverflow,
    AmountNotConserved { inputs: u64, outputs: u64 },
    MissingSplitRoot,
    MissingSplitCommitment,
    SplitRootSumMismatch { expected: u64, actual: u64 },
    SplitRootMismatch,
    SplitRootLocatorMismatch,
    SplitOutputIndexMismatch,
    SplitKeyValueMismatch,
    InvalidSplitWitness,
    UnsupportedWitnessShape(usize),
    UnsupportedScript(String),
    HashLockFailed,
}

impl fmt::Display for TapVmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "TAP VM JSON error: {err}"),
            Self::Mssmt(err) => write!(f, "TAP VM MS-SMT proof error: {err}"),
            Self::MalformedHex(value) => write!(f, "malformed TAP VM hex value: {value}"),
            Self::MalformedBytes32(err) => write!(f, "malformed TAP VM bytes32: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported TAP VM version {version}")
            }
            Self::UnsupportedScriptVersion(version) => {
                write!(f, "unsupported TAP VM script version {version}")
            }
            Self::UnsupportedGenesisType(genesis_type) => {
                write!(f, "unsupported TAP VM genesis type {genesis_type}")
            }
            Self::MalformedOutpoint(outpoint) => write!(f, "malformed TAP VM outpoint: {outpoint}"),
            Self::MalformedKey(key) => write!(f, "malformed TAP VM key: {key}"),
            Self::MalformedSignature(signature) => {
                write!(f, "malformed TAP VM signature: {signature}")
            }
            Self::MissingPrevWitnesses => write!(f, "TAP VM asset is missing prev witnesses"),
            Self::MissingWitnessStack => write!(f, "TAP VM transition is missing a witness stack"),
            Self::EmptyWitness => write!(f, "TAP VM witness is empty"),
            Self::WrongSigningDomain => write!(f, "TAP VM must use the Taproot Assets domain"),
            Self::NoInputs => write!(f, "TAP VM transition must have inputs"),
            Self::NoOutputs => write!(f, "TAP VM transition must have outputs"),
            Self::GenesisHasInputs => write!(f, "TAP VM genesis transition has inputs"),
            Self::GenesisHasSplitSet => write!(f, "TAP VM genesis transition has split outputs"),
            Self::GenesisPrevIdNotZero => write!(f, "TAP VM genesis previous ID is not zero"),
            Self::ZeroAmount => write!(f, "TAP VM transition has zero amount"),
            Self::ZeroAssetId => write!(f, "TAP VM transition has zero asset ID"),
            Self::CollectibleAmountInvalid(amount) => {
                write!(f, "TAP VM collectible amount must be 1, got {amount}")
            }
            Self::WitnessCountMismatch { expected, actual } => write!(
                f,
                "TAP VM witness count mismatch: expected {expected}, got {actual}"
            ),
            Self::InputPrevIdMismatch => write!(f, "TAP VM input prev ID mismatch"),
            Self::AssetDefinitionMismatch => write!(f, "TAP VM asset definition mismatch"),
            Self::AssetIdMismatch => write!(f, "TAP VM asset ID mismatch"),
            Self::AmountOverflow => write!(f, "TAP VM amount overflow"),
            Self::AmountNotConserved { inputs, outputs } => write!(
                f,
                "TAP VM amount not conserved: inputs={inputs} outputs={outputs}"
            ),
            Self::MissingSplitRoot => write!(f, "TAP VM split transition missing root"),
            Self::MissingSplitCommitment => {
                write!(f, "TAP VM split output missing split commitment")
            }
            Self::SplitRootSumMismatch { expected, actual } => write!(
                f,
                "TAP VM split root sum mismatch: expected {expected}, got {actual}"
            ),
            Self::SplitRootMismatch => write!(f, "TAP VM split root mismatch"),
            Self::SplitRootLocatorMismatch => write!(f, "TAP VM split root locator mismatch"),
            Self::SplitOutputIndexMismatch => write!(f, "TAP VM split output index mismatch"),
            Self::SplitKeyValueMismatch => write!(f, "TAP VM split key/value mismatch"),
            Self::InvalidSplitWitness => write!(f, "TAP VM split witness is invalid"),
            Self::UnsupportedWitnessShape(count) => {
                write!(
                    f,
                    "unsupported TAP VM witness stack shape with {count} item(s)"
                )
            }
            Self::UnsupportedScript(script) => write!(f, "unsupported TAP VM script: {script}"),
            Self::HashLockFailed => write!(f, "TAP VM hash-lock witness failed"),
        }
    }
}

impl Error for TapVmError {}

impl From<MssmtError> for TapVmError {
    fn from(value: MssmtError) -> Self {
        Self::Mssmt(value)
    }
}

fn validate_asset_shape(asset: &TapVmAsset) -> Result<(), TapVmError> {
    if asset.version > 1 {
        return Err(TapVmError::UnsupportedVersion(asset.version));
    }
    if asset.script_version != 0 {
        return Err(TapVmError::UnsupportedScriptVersion(asset.script_version));
    }
    if !matches!(asset.genesis_type, 0 | 1) {
        return Err(TapVmError::UnsupportedGenesisType(asset.genesis_type));
    }
    validate_outpoint(&asset.genesis_first_prev_out)?;
    validate_hex_len(&asset.genesis_tag, 32)?;
    validate_hex_len(&asset.genesis_meta_hash, 32)?;
    validate_compressed_key(&asset.script_key)?;
    if let Some(group_key) = &asset.group_key {
        validate_compressed_key(&group_key.group_key)?;
        validate_signature_hex(&group_key.group_key_sig)?;
    }
    if let Some(root) = &asset.split_commitment_root {
        validate_hex_len(&root.hash, 32)?;
    }
    Ok(())
}

fn validate_asset_amount_for_type(asset: &TapVmAsset) -> Result<(), TapVmError> {
    match asset.genesis_type {
        0 if asset.amount > 0 => Ok(()),
        1 if asset.amount == 1 => Ok(()),
        1 => Err(TapVmError::CollectibleAmountInvalid(asset.amount)),
        _ => Err(TapVmError::ZeroAmount),
    }
}

fn validate_witness_stack(stack: &[String]) -> Result<(), TapVmError> {
    if stack.is_empty() {
        return Err(TapVmError::EmptyWitness);
    }
    match stack.len() {
        1 => validate_signature_hex(&stack[0]),
        3 => {
            let argument = decode_hex(&stack[0])?;
            let script = decode_hex(&stack[1])?;
            let control_block = decode_hex(&stack[2])?;
            validate_control_block(&control_block)?;
            validate_script_path(&argument, &script, &stack[1])
        }
        count => Err(TapVmError::UnsupportedWitnessShape(count)),
    }
}

fn validate_script_path(
    argument: &[u8],
    script: &[u8],
    script_hex: &str,
) -> Result<(), TapVmError> {
    if script.len() == 24
        && script[0] == 0x76
        && script[1] == 0xa9
        && script[2] == 0x14
        && script[23] == 0x88
    {
        let expected = &script[3..23];
        if hash160(argument).as_slice() != expected {
            return Err(TapVmError::HashLockFailed);
        }
        return Ok(());
    }

    if script.len() >= 34 && script[0] == 0x20 && matches!(script[33], 0xac | 0xad) {
        validate_signature_bytes(argument, &hex(argument))?;
        return Ok(());
    }

    Err(TapVmError::UnsupportedScript(script_hex.to_owned()))
}

fn validate_control_block(control_block: &[u8]) -> Result<(), TapVmError> {
    if control_block.len() < 33 || !matches!(control_block[0], 0xc0 | 0xc1) {
        return Err(TapVmError::MalformedKey(hex(control_block)));
    }
    Ok(())
}

fn validate_signature_hex(signature: &str) -> Result<(), TapVmError> {
    let bytes = decode_hex(signature)?;
    validate_signature_bytes(&bytes, signature)
}

fn validate_signature_bytes(bytes: &[u8], source: &str) -> Result<(), TapVmError> {
    if !matches!(bytes.len(), 64 | 65) || bytes.iter().all(|byte| *byte == 0) {
        return Err(TapVmError::MalformedSignature(source.to_owned()));
    }
    Ok(())
}

fn validate_compressed_key(key: &str) -> Result<(), TapVmError> {
    let bytes = decode_hex(key)?;
    if bytes.len() != 33 || !matches!(bytes[0], 2 | 3) {
        return Err(TapVmError::MalformedKey(key.to_owned()));
    }
    Ok(())
}

fn same_script_key_xonly(a: &str, b: &str) -> Result<bool, TapVmError> {
    let a = decode_hex(a)?;
    let b = decode_hex(b)?;
    if a.len() != 33 || b.len() != 33 {
        return Err(TapVmError::MalformedKey(format!(
            "{} / {}",
            hex(&a),
            hex(&b)
        )));
    }
    Ok(a[1..] == b[1..])
}

fn validate_outpoint(outpoint: &str) -> Result<(), TapVmError> {
    let Some((txid, vout)) = outpoint.split_once(':') else {
        return Err(TapVmError::MalformedOutpoint(outpoint.to_owned()));
    };
    validate_hex_len(txid, 32)?;
    vout.parse::<u32>()
        .map(|_| ())
        .map_err(|_| TapVmError::MalformedOutpoint(outpoint.to_owned()))
}

fn same_asset_definition(a: &TapVmAsset, b: &TapVmAsset) -> bool {
    a.version == b.version
        && a.genesis_type == b.genesis_type
        && a.script_version == b.script_version
}

fn decode_mssmt_proof(proof_hex: &str) -> Result<(), TapVmError> {
    let bytes = decode_hex(proof_hex)?;
    let proof = MssmtCompressedProof::decode(&bytes)?;
    proof.decompress()?;
    Ok(())
}

fn checked_sum(amounts: impl IntoIterator<Item = u64>) -> Result<u64, TapVmError> {
    amounts
        .into_iter()
        .try_fold(0u64, |total, amount| total.checked_add(amount))
        .ok_or(TapVmError::AmountOverflow)
}

fn virtual_prev_id(asset_id: Bytes32, index: u64, context: Bytes32) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:virtual-prev-id:v1");
    hasher.update(asset_id.0);
    hasher.update(index.to_be_bytes());
    hasher.update(context.0);
    Bytes32(hasher.finalize().into())
}

fn validate_hex_len(value: &str, byte_len: usize) -> Result<(), TapVmError> {
    let bytes = decode_hex(value)?;
    if bytes.len() != byte_len {
        return Err(TapVmError::MalformedHex(value.to_owned()));
    }
    Ok(())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, TapVmError> {
    if value.len() % 2 != 0 {
        return Err(TapVmError::MalformedHex(value.to_owned()));
    }
    value
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk)
                .map_err(|_| TapVmError::MalformedHex(value.to_owned()))?;
            u8::from_str_radix(text, 16).map_err(|_| TapVmError::MalformedHex(value.to_owned()))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash160(bytes: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(bytes);
    let ripe = Ripemd160::digest(sha);
    ripe.into()
}

fn u64_from_any<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct U64Visitor;

    impl Visitor<'_> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a u64 encoded as a JSON number or decimal string")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u64::try_from(value).map_err(|_| E::custom(format!("negative u64 value {value}")))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .parse::<u64>()
                .map_err(|err| E::custom(format!("invalid u64 string {value}: {err}")))
        }
    }

    deserializer.deserialize_any(U64Visitor)
}

fn vec_from_null<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_virtual_transition_conserves_amounts() {
        let asset_id = Bytes32([7; 32]);
        let script_key = CompressedKey([2; 33]);
        let transition = AssetVirtualTransition::channel_funding(
            asset_id,
            [700, 300],
            1_000,
            script_key,
            Bytes32([9; 32]),
        );

        let summary = transition.validate().expect("transition validates");
        assert_eq!(summary.input_amount, 1_000);
        assert_eq!(summary.output_amount, 1_000);
        assert_ne!(transition.tx_id().expect("txid"), Bytes32::ZERO);
        assert_ne!(
            transition.witness_digest().expect("witness digest"),
            Bytes32::ZERO
        );
    }

    #[test]
    fn generic_virtual_transition_rejects_bad_domain_and_amounts() {
        let mut transition = AssetVirtualTransition::channel_balance_update(
            TapVmTransitionKind::HtlcSettlement,
            Bytes32([7; 32]),
            1_000,
            500,
            400,
            Bytes32([9; 32]),
        );
        assert!(matches!(
            transition.validate(),
            Err(TapVmError::AmountNotConserved { .. })
        ));
        transition.outputs[1].amount = 500;
        transition.signing_domain = SigningDomain::Bitcoin;
        assert!(matches!(
            transition.validate(),
            Err(TapVmError::WrongSigningDomain)
        ));
    }
}
