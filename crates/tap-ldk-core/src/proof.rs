use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    asset::{
        AssetAmount, AssetError, AssetLeaf, AssetType, Bytes32, CompressedKey, RootHashSum,
        derive_hash_sum_root,
    },
    tapd_proof::TapdProofFileSummary,
    tlv::{TlvError, TlvRecord, decode_stream, encode_stream, reject_unknown_required},
};

const TYPE_VERSION: u64 = 1;
const TYPE_ASSET_ID: u64 = 3;
const TYPE_GENESIS_OUTPOINT: u64 = 5;
const TYPE_ANCHOR_OUTPOINT: u64 = 7;
const TYPE_AMOUNT: u64 = 9;
const TYPE_SCRIPT_KEY: u64 = 11;
const TYPE_ROOT_HASH: u64 = 13;
const TYPE_ROOT_SUM: u64 = 15;
const TYPE_VERIFICATION_SCOPE: u64 = 17;
const TYPE_NETWORK: u64 = 19;
const TYPE_ASSET_TYPE: u64 = 21;

const KNOWN_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_ASSET_ID,
    TYPE_GENESIS_OUTPOINT,
    TYPE_ANCHOR_OUTPOINT,
    TYPE_AMOUNT,
    TYPE_SCRIPT_KEY,
    TYPE_ROOT_HASH,
    TYPE_ROOT_SUM,
    TYPE_VERIFICATION_SCOPE,
    TYPE_NETWORK,
    TYPE_ASSET_TYPE,
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofFile {
    pub version: u8,
    pub asset_id: Bytes32,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub tap_asset_root: RootHashSum,
    pub verification_scope: VerificationScope,
    pub network: ProofNetwork,
    pub asset_type: AssetType,
}

impl ProofFile {
    pub fn encode(&self) -> Result<Vec<u8>, ProofError> {
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ASSET_ID, self.asset_id.0),
            TlvRecord::new(TYPE_GENESIS_OUTPOINT, self.genesis_outpoint.as_bytes()),
            TlvRecord::new(TYPE_ANCHOR_OUTPOINT, self.anchor_outpoint.as_bytes()),
            TlvRecord::new(TYPE_AMOUNT, self.amount.value().to_be_bytes()),
            TlvRecord::new(TYPE_SCRIPT_KEY, self.script_key.0),
            TlvRecord::new(TYPE_ROOT_HASH, self.tap_asset_root.hash.0),
            TlvRecord::new(TYPE_ROOT_SUM, self.tap_asset_root.sum.value().to_be_bytes()),
            TlvRecord::new(
                TYPE_VERIFICATION_SCOPE,
                self.verification_scope.as_str().as_bytes(),
            ),
            TlvRecord::new(TYPE_NETWORK, self.network.as_str().as_bytes()),
            TlvRecord::new(TYPE_ASSET_TYPE, [self.asset_type.as_u8()]),
        ];

        encode_stream(&records).map_err(ProofError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProofError> {
        let records = decode_stream(bytes).map_err(ProofError::Tlv)?;
        reject_unknown_required(&records, KNOWN_TYPES).map_err(ProofError::Tlv)?;

        let mut fields = BTreeMap::new();
        for record in records {
            fields.insert(record.type_id, record.value);
        }

        let version = parse_u8(required(&fields, TYPE_VERSION)?, "version")?;
        let asset_id = parse_bytes32(required(&fields, TYPE_ASSET_ID)?)?;
        let genesis_outpoint = parse_string(required(&fields, TYPE_GENESIS_OUTPOINT)?)?;
        let anchor_outpoint = parse_string(required(&fields, TYPE_ANCHOR_OUTPOINT)?)?;
        let amount = AssetAmount::new(parse_u64(required(&fields, TYPE_AMOUNT)?, "amount")?);
        let script_key = parse_compressed_key(required(&fields, TYPE_SCRIPT_KEY)?)?;
        let root_hash = parse_bytes32(required(&fields, TYPE_ROOT_HASH)?)?;
        let root_sum = AssetAmount::new(parse_u64(required(&fields, TYPE_ROOT_SUM)?, "root_sum")?);
        let verification_scope = VerificationScope::from_str(&parse_string(required(
            &fields,
            TYPE_VERIFICATION_SCOPE,
        )?)?)?;
        let network = ProofNetwork::from_str(&parse_string(required(&fields, TYPE_NETWORK)?)?)?;
        let asset_type = parse_asset_type(required(&fields, TYPE_ASSET_TYPE)?)?;

        Ok(Self {
            version,
            asset_id,
            genesis_outpoint,
            anchor_outpoint,
            amount,
            script_key,
            tap_asset_root: RootHashSum {
                hash: root_hash,
                sum: root_sum,
            },
            verification_scope,
            network,
            asset_type,
        })
    }

    pub fn verify_bounded_anchor(&self) -> Result<(), ProofError> {
        self.verify_semantic_ancestry(&ProofValidationContext::default())
            .map(|_| ())
    }

    pub fn verify_semantic_ancestry(
        &self,
        context: &ProofValidationContext,
    ) -> Result<ProofValidationReport, ProofError> {
        if self.version != 0 {
            return Err(ProofError::UnsupportedVersion(self.version));
        }

        if self.verification_scope != VerificationScope::SemanticAncestry {
            return Err(ProofError::UnsupportedScope(
                self.verification_scope.as_str().to_owned(),
            ));
        }

        if self.network != context.expected_network {
            return Err(ProofError::WrongNetwork {
                expected: context.expected_network,
                actual: self.network,
            });
        }

        if self.asset_type != context.expected_asset_type {
            return Err(ProofError::WrongAssetType {
                expected: context.expected_asset_type,
                actual: self.asset_type,
            });
        }

        if self.asset_id == Bytes32::ZERO {
            return Err(ProofError::ZeroAssetId);
        }

        if self.amount == AssetAmount::ZERO {
            return Err(ProofError::ZeroAmount);
        }

        if self.tap_asset_root.sum != self.amount {
            return Err(ProofError::RootSumMismatch {
                amount: self.amount.value(),
                root_sum: self.tap_asset_root.sum.value(),
            });
        }

        if self.tap_asset_root.hash == Bytes32::ZERO {
            return Err(ProofError::BrokenAncestry("zero tap asset root hash"));
        }

        let genesis = parse_outpoint(&self.genesis_outpoint, "genesis_outpoint")?;
        let anchor = parse_outpoint(&self.anchor_outpoint, "anchor_outpoint")?;
        if genesis == anchor {
            return Err(ProofError::BrokenAncestry(
                "genesis and anchor outpoints must differ",
            ));
        }

        let derived_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: self.asset_id,
            script_key: self.script_key,
            amount: self.amount,
        }])
        .map_err(ProofError::Asset)?;
        if self.tap_asset_root != derived_root {
            return Err(ProofError::CommitmentRootMismatch {
                expected_hash: derived_root.hash,
                actual_hash: self.tap_asset_root.hash,
                expected_sum: derived_root.sum.value(),
                actual_sum: self.tap_asset_root.sum.value(),
            });
        }

        if let Some(expected) = context.expected_asset_id {
            if self.asset_id != expected {
                return Err(ProofError::WrongAsset {
                    expected,
                    actual: self.asset_id,
                });
            }
        }
        if let Some(expected) = context.expected_amount {
            if self.amount != expected {
                return Err(ProofError::WrongAmount {
                    expected: expected.value(),
                    actual: self.amount.value(),
                });
            }
        }
        if let Some(expected) = context.expected_script_key {
            if self.script_key != expected {
                return Err(ProofError::WrongOwner {
                    expected,
                    actual: self.script_key,
                });
            }
        }
        if let Some(expected) = context.expected_genesis_outpoint.as_deref() {
            if self.genesis_outpoint != expected {
                return Err(ProofError::BrokenAncestry("genesis outpoint mismatch"));
            }
        }
        if let Some(expected) = context.expected_anchor_outpoint.as_deref() {
            if self.anchor_outpoint != expected {
                return Err(ProofError::BrokenAncestry("anchor outpoint mismatch"));
            }
        }
        if let Some(stale) = context.stale_anchor_outpoint.as_deref() {
            if self.anchor_outpoint == stale {
                return Err(ProofError::StaleProof {
                    anchor_outpoint: self.anchor_outpoint.clone(),
                });
            }
        }

        if context.require_tapd_ancestry {
            let summary = context
                .tapd_proof_summary
                .as_ref()
                .ok_or(ProofError::MissingTapdProofSummary)?;
            self.validate_tapd_ancestry(summary, context)?;
        }

        Ok(ProofValidationReport {
            validation_scope: self.verification_scope,
            network: self.network,
            asset_type: self.asset_type,
            asset_id: self.asset_id,
            amount: self.amount,
            genesis_outpoint: self.genesis_outpoint.clone(),
            anchor_outpoint: self.anchor_outpoint.clone(),
            script_key: self.script_key,
            tap_asset_root: self.tap_asset_root,
            tapd_proof_count: context
                .tapd_proof_summary
                .as_ref()
                .map(|summary| summary.proof_count),
            tapd_proof_file_digest: context
                .tapd_proof_summary
                .as_ref()
                .map(|summary| summary.raw_digest),
        })
    }

    fn validate_tapd_ancestry(
        &self,
        summary: &TapdProofFileSummary,
        context: &ProofValidationContext,
    ) -> Result<(), ProofError> {
        if let Some(expected_digest) = context.expected_tapd_proof_file_digest {
            if summary.raw_digest != expected_digest {
                return Err(ProofError::StaleTapdProof {
                    expected: expected_digest,
                    actual: summary.raw_digest,
                });
            }
        }

        for proof in &summary.proofs {
            if !matches!(proof.transition_version, Some(0) | Some(1)) {
                return Err(ProofError::BrokenAncestry(
                    "unsupported tapd transition version",
                ));
            }
            if !(proof.has_prev_out
                && proof.has_block_header
                && proof.has_anchor_tx
                && proof.has_tx_merkle_proof
                && proof.has_asset_leaf
                && proof.has_inclusion_proof)
            {
                return Err(ProofError::BrokenAncestry(
                    "tapd proof missing required ancestry records",
                ));
            }
        }

        let leaf = summary
            .latest_asset_leaf()
            .ok_or(ProofError::BrokenAncestry("tapd proof missing asset leaf"))?;
        if leaf.asset_id != self.asset_id {
            return Err(ProofError::WrongAsset {
                expected: self.asset_id,
                actual: leaf.asset_id,
            });
        }
        if leaf.asset_type != self.asset_type.as_u8() {
            return Err(ProofError::WrongAssetType {
                expected: self.asset_type,
                actual: AssetType::from_u8(leaf.asset_type)
                    .map_err(|_| ProofError::BrokenAncestry("unsupported tapd asset type"))?,
            });
        }
        if leaf.amount != self.amount.value() {
            return Err(ProofError::WrongAmount {
                expected: self.amount.value(),
                actual: leaf.amount,
            });
        }
        if leaf.script_key != self.script_key {
            return Err(ProofError::WrongOwner {
                expected: self.script_key,
                actual: leaf.script_key,
            });
        }
        if leaf.genesis.first_prev_out != self.genesis_outpoint {
            return Err(ProofError::BrokenAncestry("tapd genesis outpoint mismatch"));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VerificationScope {
    BoundedAnchorOnly,
    SemanticAncestry,
}

impl VerificationScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BoundedAnchorOnly => "bounded-anchor-only",
            Self::SemanticAncestry => "semantic-ancestry",
        }
    }
}

impl FromStr for VerificationScope {
    type Err = ProofError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bounded-anchor-only" => Ok(Self::BoundedAnchorOnly),
            "semantic-ancestry" => Ok(Self::SemanticAncestry),
            other => Err(ProofError::UnsupportedScope(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProofNetwork {
    Regtest,
}

impl ProofNetwork {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regtest => "regtest",
        }
    }
}

impl FromStr for ProofNetwork {
    type Err = ProofError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "regtest" => Ok(Self::Regtest),
            other => Err(ProofError::UnsupportedNetwork(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofValidationContext {
    pub expected_network: ProofNetwork,
    pub expected_asset_type: AssetType,
    pub expected_asset_id: Option<Bytes32>,
    pub expected_amount: Option<AssetAmount>,
    pub expected_script_key: Option<CompressedKey>,
    pub expected_genesis_outpoint: Option<String>,
    pub expected_anchor_outpoint: Option<String>,
    pub stale_anchor_outpoint: Option<String>,
    pub require_tapd_ancestry: bool,
    pub expected_tapd_proof_file_digest: Option<Bytes32>,
    pub tapd_proof_summary: Option<TapdProofFileSummary>,
}

impl Default for ProofValidationContext {
    fn default() -> Self {
        Self {
            expected_network: ProofNetwork::Regtest,
            expected_asset_type: AssetType::Normal,
            expected_asset_id: None,
            expected_amount: None,
            expected_script_key: None,
            expected_genesis_outpoint: None,
            expected_anchor_outpoint: None,
            stale_anchor_outpoint: None,
            require_tapd_ancestry: false,
            expected_tapd_proof_file_digest: None,
            tapd_proof_summary: None,
        }
    }
}

impl ProofValidationContext {
    pub fn for_asset(asset_id: Bytes32) -> Self {
        Self {
            expected_asset_id: Some(asset_id),
            ..Self::default()
        }
    }

    pub fn for_close(
        asset_id: Bytes32,
        amount: AssetAmount,
        script_key: CompressedKey,
        genesis_outpoint: String,
        anchor_outpoint: String,
    ) -> Self {
        Self {
            expected_asset_id: Some(asset_id),
            expected_amount: Some(amount),
            expected_script_key: Some(script_key),
            expected_genesis_outpoint: Some(genesis_outpoint),
            expected_anchor_outpoint: Some(anchor_outpoint),
            ..Self::default()
        }
    }

    pub fn for_tapd_import(summary: TapdProofFileSummary) -> Self {
        Self {
            require_tapd_ancestry: true,
            expected_tapd_proof_file_digest: Some(summary.raw_digest),
            tapd_proof_summary: Some(summary),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofValidationReport {
    pub validation_scope: VerificationScope,
    pub network: ProofNetwork,
    pub asset_type: AssetType,
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub genesis_outpoint: String,
    pub anchor_outpoint: String,
    pub script_key: CompressedKey,
    pub tap_asset_root: RootHashSum,
    pub tapd_proof_count: Option<usize>,
    pub tapd_proof_file_digest: Option<Bytes32>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofAnchorState {
    Unknown,
    Pending,
    Confirmed,
    Stale,
    Reorged,
}

impl ProofAnchorState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Stale => "stale",
            Self::Reorged => "reorged",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofAnchorPolicy {
    default_anchor_state: ProofAnchorState,
    accept_pending: bool,
    anchor_states: BTreeMap<String, ProofAnchorState>,
}

impl ProofAnchorPolicy {
    pub fn strict_confirmed() -> Self {
        Self {
            default_anchor_state: ProofAnchorState::Unknown,
            accept_pending: false,
            anchor_states: BTreeMap::new(),
        }
    }

    pub fn assume_all_confirmed_for_regtest() -> Self {
        Self {
            default_anchor_state: ProofAnchorState::Confirmed,
            accept_pending: false,
            anchor_states: BTreeMap::new(),
        }
    }

    pub fn with_pending_accepted(mut self, accept_pending: bool) -> Self {
        self.accept_pending = accept_pending;
        self
    }

    pub fn with_anchor_state(
        mut self,
        anchor_outpoint: impl Into<String>,
        state: ProofAnchorState,
    ) -> Self {
        self.anchor_states.insert(anchor_outpoint.into(), state);
        self
    }

    pub fn anchor_state(&self, anchor_outpoint: &str) -> ProofAnchorState {
        self.anchor_states
            .get(anchor_outpoint)
            .copied()
            .unwrap_or(self.default_anchor_state)
    }

    pub fn accepts_anchor_state(&self, state: ProofAnchorState) -> bool {
        match state {
            ProofAnchorState::Confirmed => true,
            ProofAnchorState::Pending => self.accept_pending,
            ProofAnchorState::Unknown | ProofAnchorState::Stale | ProofAnchorState::Reorged => {
                false
            }
        }
    }
}

impl Default for ProofAnchorPolicy {
    fn default() -> Self {
        Self::strict_confirmed()
    }
}

/// Runtime proof-history states.
///
/// These names intentionally mirror the future `formal/tla/proof_validation`
/// model states so counterexamples can be translated into Rust regressions
/// without inventing another vocabulary.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProofHistoryState {
    Accepted,
    Rejected,
    Unresolved,
    Pending,
    Stale,
    Spent,
    ChannelLocked,
    Closed,
    Swept,
}

impl ProofHistoryState {
    fn can_explain_balance(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::ChannelLocked | Self::Closed | Self::Swept
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProofTransitionKind {
    Issuance,
    Split,
    Transfer,
    ChannelFunding,
    CommitmentUpdate,
    CooperativeClose,
    UnilateralClose,
    SecondLevelHtlc,
    Sweep,
    ProofExport,
}

impl ProofTransitionKind {
    fn consumes_inputs(self) -> bool {
        self != Self::Issuance
    }

    fn allows_input_state(self, state: ProofHistoryState) -> bool {
        match self {
            Self::Issuance => false,
            Self::Split | Self::Transfer | Self::ChannelFunding => {
                state == ProofHistoryState::Accepted
            }
            Self::CommitmentUpdate => state == ProofHistoryState::ChannelLocked,
            Self::CooperativeClose | Self::UnilateralClose => {
                state == ProofHistoryState::ChannelLocked
            }
            Self::SecondLevelHtlc | Self::Sweep => state == ProofHistoryState::Closed,
            Self::ProofExport => state.can_explain_balance(),
        }
    }

    fn allows_output_state(self, state: ProofHistoryState) -> bool {
        match self {
            Self::Issuance | Self::Split | Self::Transfer | Self::ProofExport => {
                state == ProofHistoryState::Accepted
            }
            Self::ChannelFunding | Self::CommitmentUpdate => {
                state == ProofHistoryState::ChannelLocked
            }
            Self::CooperativeClose | Self::UnilateralClose | Self::SecondLevelHtlc => {
                state == ProofHistoryState::Closed
            }
            Self::Sweep => state == ProofHistoryState::Swept,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofHistoryInput {
    pub output_id: String,
}

impl ProofHistoryInput {
    pub fn new(output_id: impl Into<String>) -> Self {
        Self {
            output_id: output_id.into(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofHistoryOutput {
    pub output_id: String,
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub anchor_outpoint: String,
    pub tap_asset_root: RootHashSum,
    pub resulting_state: ProofHistoryState,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofHistoryRecord {
    pub record_id: String,
    pub kind: ProofTransitionKind,
    pub virtual_transition_id: Bytes32,
    pub inputs: Vec<ProofHistoryInput>,
    pub outputs: Vec<ProofHistoryOutput>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofHistoryPriorState {
    pub output_id: String,
    pub state: ProofHistoryState,
    pub amount: AssetAmount,
    pub anchor_outpoint: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AcceptedBalanceExplanation {
    pub output_id: String,
    pub record_id: String,
    pub transition_kind: ProofTransitionKind,
    pub virtual_transition_id: Bytes32,
    pub asset_id: Bytes32,
    pub amount: AssetAmount,
    pub script_key: CompressedKey,
    pub anchor_outpoint: String,
    pub anchor_state: ProofAnchorState,
    pub tap_asset_root: RootHashSum,
    pub prior_states: Vec<ProofHistoryPriorState>,
    pub resulting_state: ProofHistoryState,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProofHistoryReplay {
    explanations: BTreeMap<String, AcceptedBalanceExplanation>,
    states: BTreeMap<String, ProofHistoryState>,
}

impl ProofHistoryReplay {
    pub fn accepted_explanations(&self) -> impl Iterator<Item = &AcceptedBalanceExplanation> {
        self.explanations.values()
    }

    pub fn accepted_explanation(&self, output_id: &str) -> Option<&AcceptedBalanceExplanation> {
        self.explanations.get(output_id)
    }

    pub fn output_state(&self, output_id: &str) -> Option<ProofHistoryState> {
        self.states.get(output_id).copied()
    }
}

#[derive(Debug, Default)]
pub struct ProofHistoryEngine;

impl ProofHistoryEngine {
    pub fn replay(
        records: &[ProofHistoryRecord],
    ) -> Result<ProofHistoryReplay, ProofHistoryReplayError> {
        Self::replay_with_anchor_policy(
            records,
            &ProofAnchorPolicy::assume_all_confirmed_for_regtest(),
        )
    }

    pub fn replay_with_anchor_policy(
        records: &[ProofHistoryRecord],
        anchor_policy: &ProofAnchorPolicy,
    ) -> Result<ProofHistoryReplay, ProofHistoryReplayError> {
        let mut seen_records = BTreeMap::<String, ()>::new();
        let mut outputs = BTreeMap::<String, AcceptedBalanceExplanation>::new();
        let mut explanations = BTreeMap::<String, AcceptedBalanceExplanation>::new();
        let mut states = BTreeMap::<String, ProofHistoryState>::new();

        for record in records {
            validate_record_identity(record, &mut seen_records)?;
            let prior_states = validate_inputs(record, &outputs, &mut explanations, &mut states)?;
            let output_total = validate_outputs(record)?;
            let input_total = prior_states
                .iter()
                .try_fold(AssetAmount::ZERO, |total, prior| {
                    total
                        .checked_add(prior.amount)
                        .map_err(ProofHistoryReplayError::Asset)
                })?;

            if record.kind != ProofTransitionKind::Issuance && input_total != output_total {
                return Err(ProofHistoryReplayError::AmountNotConserved {
                    record_id: record.record_id.clone(),
                    input: input_total.value(),
                    output: output_total.value(),
                });
            }

            let expected_asset_id = prior_states.first().map(|prior| {
                outputs
                    .get(&prior.output_id)
                    .expect("prior state came from outputs")
                    .asset_id
            });
            if let Some(expected) = expected_asset_id {
                for prior in &prior_states {
                    let actual = outputs
                        .get(&prior.output_id)
                        .expect("prior state came from outputs")
                        .asset_id;
                    if actual != expected {
                        return Err(ProofHistoryReplayError::AssetMismatch {
                            record_id: record.record_id.clone(),
                            expected,
                            actual,
                        });
                    }
                }
            }
            for output in &record.outputs {
                if outputs.contains_key(&output.output_id) {
                    return Err(ProofHistoryReplayError::DuplicateOutput {
                        record_id: record.record_id.clone(),
                        output_id: output.output_id.clone(),
                    });
                }
                if let Some(asset_id) = expected_asset_id {
                    if output.asset_id != asset_id {
                        return Err(ProofHistoryReplayError::AssetMismatch {
                            record_id: record.record_id.clone(),
                            expected: asset_id,
                            actual: output.asset_id,
                        });
                    }
                }

                let anchor_state = anchor_policy.anchor_state(&output.anchor_outpoint);
                if output.resulting_state.can_explain_balance()
                    && !anchor_policy.accepts_anchor_state(anchor_state)
                {
                    return Err(ProofHistoryReplayError::UnacceptableAnchorState {
                        record_id: record.record_id.clone(),
                        output_id: output.output_id.clone(),
                        anchor_outpoint: output.anchor_outpoint.clone(),
                        anchor_state,
                    });
                }

                let explanation = AcceptedBalanceExplanation {
                    output_id: output.output_id.clone(),
                    record_id: record.record_id.clone(),
                    transition_kind: record.kind,
                    virtual_transition_id: record.virtual_transition_id,
                    asset_id: output.asset_id,
                    amount: output.amount,
                    script_key: output.script_key,
                    anchor_outpoint: output.anchor_outpoint.clone(),
                    anchor_state,
                    tap_asset_root: output.tap_asset_root,
                    prior_states: prior_states.clone(),
                    resulting_state: output.resulting_state,
                };
                outputs.insert(output.output_id.clone(), explanation.clone());
                states.insert(output.output_id.clone(), output.resulting_state);
                if output.resulting_state.can_explain_balance() {
                    explanations.insert(output.output_id.clone(), explanation);
                }
            }
        }

        Ok(ProofHistoryReplay {
            explanations,
            states,
        })
    }
}

fn validate_record_identity(
    record: &ProofHistoryRecord,
    seen_records: &mut BTreeMap<String, ()>,
) -> Result<(), ProofHistoryReplayError> {
    if record.record_id.is_empty() {
        return Err(ProofHistoryReplayError::EmptyRecordId);
    }
    if seen_records.insert(record.record_id.clone(), ()).is_some() {
        return Err(ProofHistoryReplayError::DuplicateRecord {
            record_id: record.record_id.clone(),
        });
    }
    if record.virtual_transition_id == Bytes32::ZERO {
        return Err(ProofHistoryReplayError::ZeroTransitionId {
            record_id: record.record_id.clone(),
        });
    }
    if record.kind == ProofTransitionKind::Issuance {
        if !record.inputs.is_empty() {
            return Err(ProofHistoryReplayError::UnexpectedInputs {
                record_id: record.record_id.clone(),
                kind: record.kind,
            });
        }
    } else if record.inputs.is_empty() {
        return Err(ProofHistoryReplayError::MissingInputs {
            record_id: record.record_id.clone(),
            kind: record.kind,
        });
    }
    if record.outputs.is_empty() {
        return Err(ProofHistoryReplayError::MissingOutputs {
            record_id: record.record_id.clone(),
        });
    }

    Ok(())
}

fn validate_inputs(
    record: &ProofHistoryRecord,
    outputs: &BTreeMap<String, AcceptedBalanceExplanation>,
    explanations: &mut BTreeMap<String, AcceptedBalanceExplanation>,
    states: &mut BTreeMap<String, ProofHistoryState>,
) -> Result<Vec<ProofHistoryPriorState>, ProofHistoryReplayError> {
    let mut prior_states = Vec::with_capacity(record.inputs.len());
    let mut seen_inputs = BTreeMap::<String, ()>::new();
    for input in &record.inputs {
        if seen_inputs.insert(input.output_id.clone(), ()).is_some() {
            return Err(ProofHistoryReplayError::DuplicateInput {
                record_id: record.record_id.clone(),
                input_output_id: input.output_id.clone(),
            });
        }
        let Some(existing) = outputs.get(&input.output_id) else {
            return Err(ProofHistoryReplayError::MissingInput {
                record_id: record.record_id.clone(),
                input_output_id: input.output_id.clone(),
            });
        };
        if !record.kind.allows_input_state(existing.resulting_state) {
            return Err(ProofHistoryReplayError::InvalidInputState {
                record_id: record.record_id.clone(),
                input_output_id: input.output_id.clone(),
                kind: record.kind,
                state: existing.resulting_state,
            });
        }
        prior_states.push(ProofHistoryPriorState {
            output_id: existing.output_id.clone(),
            state: existing.resulting_state,
            amount: existing.amount,
            anchor_outpoint: existing.anchor_outpoint.clone(),
        });
    }

    if record.kind.consumes_inputs() {
        for input in &record.inputs {
            explanations.remove(&input.output_id);
            states.insert(input.output_id.clone(), ProofHistoryState::Spent);
        }
    }

    Ok(prior_states)
}

fn validate_outputs(record: &ProofHistoryRecord) -> Result<AssetAmount, ProofHistoryReplayError> {
    let mut output_total = AssetAmount::ZERO;
    let mut seen_outputs = BTreeMap::<String, ()>::new();
    for output in &record.outputs {
        if output.output_id.is_empty() {
            return Err(ProofHistoryReplayError::EmptyOutputId {
                record_id: record.record_id.clone(),
            });
        }
        if seen_outputs.insert(output.output_id.clone(), ()).is_some() {
            return Err(ProofHistoryReplayError::DuplicateOutput {
                record_id: record.record_id.clone(),
                output_id: output.output_id.clone(),
            });
        }
        if !record.kind.allows_output_state(output.resulting_state) {
            return Err(ProofHistoryReplayError::InvalidOutputState {
                record_id: record.record_id.clone(),
                output_id: output.output_id.clone(),
                kind: record.kind,
                state: output.resulting_state,
            });
        }
        if output.amount == AssetAmount::ZERO {
            return Err(ProofHistoryReplayError::ZeroOutputAmount {
                record_id: record.record_id.clone(),
                output_id: output.output_id.clone(),
            });
        }
        parse_outpoint(&output.anchor_outpoint, "proof_history_output_anchor").map_err(|_| {
            ProofHistoryReplayError::MalformedOutputAnchor {
                record_id: record.record_id.clone(),
                output_id: output.output_id.clone(),
            }
        })?;
        let expected_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id: output.asset_id,
            script_key: output.script_key,
            amount: output.amount,
        }])
        .map_err(ProofHistoryReplayError::Asset)?;
        if expected_root != output.tap_asset_root {
            return Err(ProofHistoryReplayError::OutputRootMismatch {
                record_id: record.record_id.clone(),
                output_id: output.output_id.clone(),
                expected_hash: expected_root.hash,
                actual_hash: output.tap_asset_root.hash,
                expected_sum: expected_root.sum.value(),
                actual_sum: output.tap_asset_root.sum.value(),
            });
        }
        output_total = output_total
            .checked_add(output.amount)
            .map_err(ProofHistoryReplayError::Asset)?;
    }
    Ok(output_total)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProofHistoryReplayError {
    Asset(AssetError),
    EmptyRecordId,
    DuplicateRecord {
        record_id: String,
    },
    ZeroTransitionId {
        record_id: String,
    },
    UnexpectedInputs {
        record_id: String,
        kind: ProofTransitionKind,
    },
    MissingInputs {
        record_id: String,
        kind: ProofTransitionKind,
    },
    MissingOutputs {
        record_id: String,
    },
    MissingInput {
        record_id: String,
        input_output_id: String,
    },
    DuplicateInput {
        record_id: String,
        input_output_id: String,
    },
    InvalidInputState {
        record_id: String,
        input_output_id: String,
        kind: ProofTransitionKind,
        state: ProofHistoryState,
    },
    EmptyOutputId {
        record_id: String,
    },
    DuplicateOutput {
        record_id: String,
        output_id: String,
    },
    InvalidOutputState {
        record_id: String,
        output_id: String,
        kind: ProofTransitionKind,
        state: ProofHistoryState,
    },
    ZeroOutputAmount {
        record_id: String,
        output_id: String,
    },
    MalformedOutputAnchor {
        record_id: String,
        output_id: String,
    },
    OutputRootMismatch {
        record_id: String,
        output_id: String,
        expected_hash: Bytes32,
        actual_hash: Bytes32,
        expected_sum: u64,
        actual_sum: u64,
    },
    UnacceptableAnchorState {
        record_id: String,
        output_id: String,
        anchor_outpoint: String,
        anchor_state: ProofAnchorState,
    },
    AmountNotConserved {
        record_id: String,
        input: u64,
        output: u64,
    },
    AssetMismatch {
        record_id: String,
        expected: Bytes32,
        actual: Bytes32,
    },
}

impl fmt::Display for ProofHistoryReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(err) => write!(f, "proof history asset error: {err}"),
            Self::EmptyRecordId => write!(f, "proof history record id cannot be empty"),
            Self::DuplicateRecord { record_id } => {
                write!(f, "duplicate proof history record: {record_id}")
            }
            Self::ZeroTransitionId { record_id } => {
                write!(f, "proof history record {record_id} has zero transition id")
            }
            Self::UnexpectedInputs { record_id, kind } => write!(
                f,
                "proof history record {record_id} of kind {kind:?} must not have inputs"
            ),
            Self::MissingInputs { record_id, kind } => write!(
                f,
                "proof history record {record_id} of kind {kind:?} requires inputs"
            ),
            Self::MissingOutputs { record_id } => {
                write!(f, "proof history record {record_id} requires outputs")
            }
            Self::MissingInput {
                record_id,
                input_output_id,
            } => write!(
                f,
                "proof history record {record_id} references missing input {input_output_id}"
            ),
            Self::DuplicateInput {
                record_id,
                input_output_id,
            } => write!(
                f,
                "proof history record {record_id} references duplicate input {input_output_id}"
            ),
            Self::InvalidInputState {
                record_id,
                input_output_id,
                kind,
                state,
            } => write!(
                f,
                "proof history record {record_id} of kind {kind:?} cannot spend input {input_output_id} in state {state:?}"
            ),
            Self::EmptyOutputId { record_id } => write!(
                f,
                "proof history record {record_id} has an output with an empty id"
            ),
            Self::DuplicateOutput {
                record_id,
                output_id,
            } => write!(
                f,
                "proof history record {record_id} has duplicate output {output_id}"
            ),
            Self::InvalidOutputState {
                record_id,
                output_id,
                kind,
                state,
            } => write!(
                f,
                "proof history record {record_id} output {output_id} cannot end {kind:?} as {state:?}"
            ),
            Self::ZeroOutputAmount {
                record_id,
                output_id,
            } => write!(
                f,
                "proof history record {record_id} output {output_id} has zero amount"
            ),
            Self::MalformedOutputAnchor {
                record_id,
                output_id,
            } => write!(
                f,
                "proof history record {record_id} output {output_id} has malformed anchor"
            ),
            Self::OutputRootMismatch {
                record_id,
                output_id,
                expected_hash,
                actual_hash,
                expected_sum,
                actual_sum,
            } => write!(
                f,
                "proof history record {record_id} output {output_id} root mismatch: expected {}:{expected_sum}, got {}:{actual_sum}",
                expected_hash.to_hex(),
                actual_hash.to_hex()
            ),
            Self::UnacceptableAnchorState {
                record_id,
                output_id,
                anchor_outpoint,
                anchor_state,
            } => write!(
                f,
                "proof history record {record_id} output {output_id} has {} anchor {anchor_outpoint}",
                anchor_state.as_str()
            ),
            Self::AmountNotConserved {
                record_id,
                input,
                output,
            } => write!(
                f,
                "proof history record {record_id} does not conserve amount: input {input}, output {output}"
            ),
            Self::AssetMismatch {
                record_id,
                expected,
                actual,
            } => write!(
                f,
                "proof history record {record_id} asset mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
        }
    }
}

impl Error for ProofHistoryReplayError {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProofError {
    Tlv(TlvError),
    Asset(AssetError),
    MissingField(u64),
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidUtf8(&'static str),
    UnsupportedVersion(u8),
    UnsupportedScope(String),
    UnsupportedNetwork(String),
    ZeroAssetId,
    ZeroAmount,
    RootSumMismatch {
        amount: u64,
        root_sum: u64,
    },
    MalformedOutpoint(&'static str),
    WrongNetwork {
        expected: ProofNetwork,
        actual: ProofNetwork,
    },
    WrongAssetType {
        expected: AssetType,
        actual: AssetType,
    },
    WrongAsset {
        expected: Bytes32,
        actual: Bytes32,
    },
    WrongOwner {
        expected: CompressedKey,
        actual: CompressedKey,
    },
    WrongAmount {
        expected: u64,
        actual: u64,
    },
    CommitmentRootMismatch {
        expected_hash: Bytes32,
        actual_hash: Bytes32,
        expected_sum: u64,
        actual_sum: u64,
    },
    BrokenAncestry(&'static str),
    StaleProof {
        anchor_outpoint: String,
    },
    MissingTapdProofSummary,
    StaleTapdProof {
        expected: Bytes32,
        actual: Bytes32,
    },
}

impl fmt::Display for ProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tlv(err) => write!(f, "proof TLV error: {err}"),
            Self::Asset(err) => write!(f, "proof asset error: {err}"),
            Self::MissingField(field) => write!(f, "missing proof field {field}"),
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid proof field {field} length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidUtf8(field) => write!(f, "proof field {field} is not UTF-8"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported proof version {version}")
            }
            Self::UnsupportedScope(scope) => write!(f, "unsupported proof scope {scope}"),
            Self::UnsupportedNetwork(network) => {
                write!(f, "unsupported proof network {network}")
            }
            Self::ZeroAssetId => write!(f, "proof asset id cannot be zero"),
            Self::ZeroAmount => write!(f, "proof amount cannot be zero"),
            Self::RootSumMismatch { amount, root_sum } => {
                write!(
                    f,
                    "proof root sum mismatch: amount {amount}, root sum {root_sum}"
                )
            }
            Self::MalformedOutpoint(field) => write!(f, "malformed proof outpoint: {field}"),
            Self::WrongNetwork { expected, actual } => write!(
                f,
                "proof network mismatch: expected {}, got {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::WrongAssetType { expected, actual } => write!(
                f,
                "proof asset type mismatch: expected {}, got {}",
                expected.as_u8(),
                actual.as_u8()
            ),
            Self::WrongAsset { expected, actual } => write!(
                f,
                "proof asset mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::WrongOwner { expected, actual } => write!(
                f,
                "proof owner mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::WrongAmount { expected, actual } => {
                write!(
                    f,
                    "proof amount mismatch: expected {expected}, got {actual}"
                )
            }
            Self::CommitmentRootMismatch {
                expected_hash,
                actual_hash,
                expected_sum,
                actual_sum,
            } => write!(
                f,
                "proof commitment root mismatch: expected {}:{expected_sum}, got {}:{actual_sum}",
                expected_hash.to_hex(),
                actual_hash.to_hex()
            ),
            Self::BrokenAncestry(reason) => write!(f, "broken proof ancestry: {reason}"),
            Self::StaleProof { anchor_outpoint } => {
                write!(f, "stale proof anchor outpoint: {anchor_outpoint}")
            }
            Self::MissingTapdProofSummary => write!(f, "missing tapd proof summary"),
            Self::StaleTapdProof { expected, actual } => write!(
                f,
                "stale tapd proof digest: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
        }
    }
}

impl Error for ProofError {}

fn required(fields: &BTreeMap<u64, Vec<u8>>, field: u64) -> Result<&[u8], ProofError> {
    fields
        .get(&field)
        .map(Vec::as_slice)
        .ok_or(ProofError::MissingField(field))
}

fn parse_u8(bytes: &[u8], field: &'static str) -> Result<u8, ProofError> {
    if bytes.len() != 1 {
        return Err(ProofError::InvalidFieldLength {
            field,
            expected: 1,
            actual: bytes.len(),
        });
    }

    Ok(bytes[0])
}

fn parse_u64(bytes: &[u8], field: &'static str) -> Result<u64, ProofError> {
    let actual = bytes.len();
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| ProofError::InvalidFieldLength {
            field,
            expected: 8,
            actual,
        })?;

    Ok(u64::from_be_bytes(bytes))
}

fn parse_asset_type(bytes: &[u8]) -> Result<AssetType, ProofError> {
    let value = parse_u8(bytes, "asset_type")?;
    AssetType::from_u8(value).map_err(ProofError::Asset)
}

fn parse_string(bytes: &[u8]) -> Result<String, ProofError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| ProofError::InvalidUtf8("string"))
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, ProofError> {
    let actual = bytes.len();
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ProofError::InvalidFieldLength {
            field: "bytes32",
            expected: 32,
            actual,
        })?;
    Ok(Bytes32(bytes))
}

fn parse_compressed_key(bytes: &[u8]) -> Result<CompressedKey, ProofError> {
    CompressedKey::from_str(&encode_hex(bytes)).map_err(ProofError::Asset)
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ParsedOutpoint {
    txid: String,
    vout: u32,
}

fn parse_outpoint(value: &str, field: &'static str) -> Result<ParsedOutpoint, ProofError> {
    let mut parts = value.split(':');
    let Some(txid) = parts.next() else {
        return Err(ProofError::MalformedOutpoint(field));
    };
    let Some(vout) = parts.next() else {
        return Err(ProofError::MalformedOutpoint(field));
    };
    if parts.next().is_some()
        || txid.len() != 64
        || txid.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || txid.bytes().all(|byte| byte == b'0')
        || vout.is_empty()
        || vout.starts_with('+')
    {
        return Err(ProofError::MalformedOutpoint(field));
    }
    let vout = vout
        .parse::<u32>()
        .map_err(|_| ProofError::MalformedOutpoint(field))?;

    Ok(ParsedOutpoint {
        txid: txid.to_ascii_lowercase(),
        vout,
    })
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
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    fn proof() -> ProofFile {
        let asset_id =
            Bytes32::from_str("dbe4d6f07f3751421793d77478b1da71c1a1382ea5766d4f9237a20351a862d8")
                .expect("asset id parses");
        let script_key = CompressedKey::from_str(
            "02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
        )
        .expect("script key parses");
        let amount = AssetAmount::new(1000000);
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
    fn proof_round_trips_and_verifies() {
        let proof = proof();
        let encoded = proof.encode().expect("proof encodes");
        let decoded = ProofFile::decode(&encoded).expect("proof decodes");

        assert_eq!(decoded, proof);
        decoded
            .verify_semantic_ancestry(&ProofValidationContext::default())
            .expect("proof verifies");
    }

    #[test]
    fn root_sum_mismatch_fails_closed() {
        let mut proof = proof();
        proof.tap_asset_root.sum = AssetAmount::new(999999);

        assert_eq!(
            proof
                .verify_semantic_ancestry(&ProofValidationContext::default())
                .map(|_| ()),
            Err(ProofError::RootSumMismatch {
                amount: 1000000,
                root_sum: 999999
            })
        );
    }

    #[test]
    fn commitment_root_hash_mismatch_fails_closed() {
        let mut proof = proof();
        proof.tap_asset_root.hash = Bytes32([42; 32]);

        assert!(matches!(
            proof.verify_semantic_ancestry(&ProofValidationContext::default()),
            Err(ProofError::CommitmentRootMismatch { .. })
        ));
    }

    #[test]
    fn semantic_context_rejects_wrong_fields_and_stale_anchor() {
        let proof = proof();

        let mut wrong_asset = ProofValidationContext::default();
        wrong_asset.expected_asset_id = Some(Bytes32([7; 32]));
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_asset),
            Err(ProofError::WrongAsset { .. })
        ));

        let mut wrong_amount = ProofValidationContext::default();
        wrong_amount.expected_amount = Some(AssetAmount::new(999));
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_amount),
            Err(ProofError::WrongAmount { .. })
        ));

        let mut wrong_owner = ProofValidationContext::default();
        wrong_owner.expected_script_key = Some(
            CompressedKey::from_str(
                "03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f",
            )
            .expect("script key parses"),
        );
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_owner),
            Err(ProofError::WrongOwner { .. })
        ));

        let mut wrong_genesis = ProofValidationContext::default();
        wrong_genesis.expected_genesis_outpoint =
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:0".to_owned());
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_genesis),
            Err(ProofError::BrokenAncestry("genesis outpoint mismatch"))
        ));

        let mut wrong_anchor = ProofValidationContext::default();
        wrong_anchor.expected_anchor_outpoint =
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:1".to_owned());
        assert!(matches!(
            proof.verify_semantic_ancestry(&wrong_anchor),
            Err(ProofError::BrokenAncestry("anchor outpoint mismatch"))
        ));

        let mut stale = ProofValidationContext::default();
        stale.stale_anchor_outpoint = Some(proof.anchor_outpoint.clone());
        assert!(matches!(
            proof.verify_semantic_ancestry(&stale),
            Err(ProofError::StaleProof { .. })
        ));

        let mut wrong_type = proof.clone();
        wrong_type.asset_type = AssetType::Collectible;
        assert!(matches!(
            wrong_type.verify_semantic_ancestry(&ProofValidationContext::default()),
            Err(ProofError::WrongAssetType { .. })
        ));
    }

    #[test]
    fn negative_proof_vector_fixture_covers_required_classes() {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/synthetic/proof_negative_vectors.json");
        let raw = fs::read_to_string(fixture_path).expect("negative vector fixture reads");
        let fixture: serde_json::Value =
            serde_json::from_str(&raw).expect("negative vector fixture parses");
        let vectors = fixture["vectors"]
            .as_array()
            .expect("vectors must be an array");
        let seen = vectors
            .iter()
            .map(|vector| {
                let id = vector["id"].as_str().expect("vector id");
                assert!(
                    !vector["boundary"].as_str().unwrap_or_default().is_empty(),
                    "vector {id} missing boundary"
                );
                assert!(
                    !vector["expected_rejection"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty(),
                    "vector {id} missing expected rejection"
                );
                assert!(
                    !vector["test_target"]
                        .as_str()
                        .unwrap_or_default()
                        .is_empty(),
                    "vector {id} missing test target"
                );
                id.to_owned()
            })
            .collect::<BTreeSet<_>>();

        for required in [
            "wrong-genesis",
            "wrong-anchor",
            "stale-proof",
            "malformed-tapf-proof-file-transport",
            "invalid-split-sum",
            "wrong-owner-script-key",
            "missing-stxo",
            "wrong-asset-type",
            "wrong-amount",
            "wrong-root-hash",
            "wrong-root-sum",
            "mismatched-tap-commitment-output-root",
            "reorg-sensitive-history",
        ] {
            assert!(
                seen.contains(required),
                "missing negative proof vector: {required}"
            );
        }
    }

    #[test]
    fn unsupported_scope_fails_decode() {
        let proof = proof();
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [proof.version]),
            TlvRecord::new(TYPE_ASSET_ID, proof.asset_id.0),
            TlvRecord::new(TYPE_GENESIS_OUTPOINT, proof.genesis_outpoint.as_bytes()),
            TlvRecord::new(TYPE_ANCHOR_OUTPOINT, proof.anchor_outpoint.as_bytes()),
            TlvRecord::new(TYPE_AMOUNT, proof.amount.value().to_be_bytes()),
            TlvRecord::new(TYPE_SCRIPT_KEY, proof.script_key.0),
            TlvRecord::new(TYPE_ROOT_HASH, proof.tap_asset_root.hash.0),
            TlvRecord::new(
                TYPE_ROOT_SUM,
                proof.tap_asset_root.sum.value().to_be_bytes(),
            ),
            TlvRecord::new(TYPE_VERIFICATION_SCOPE, b"full-history-required"),
            TlvRecord::new(TYPE_NETWORK, proof.network.as_str().as_bytes()),
            TlvRecord::new(TYPE_ASSET_TYPE, [proof.asset_type.as_u8()]),
        ];
        let encoded = encode_stream(&records).expect("proof records encode");

        assert_eq!(
            ProofFile::decode(&encoded),
            Err(ProofError::UnsupportedScope(
                "full-history-required".to_owned()
            ))
        );
    }

    #[test]
    fn unsupported_network_fails_decode() {
        let proof = proof();
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [proof.version]),
            TlvRecord::new(TYPE_ASSET_ID, proof.asset_id.0),
            TlvRecord::new(TYPE_GENESIS_OUTPOINT, proof.genesis_outpoint.as_bytes()),
            TlvRecord::new(TYPE_ANCHOR_OUTPOINT, proof.anchor_outpoint.as_bytes()),
            TlvRecord::new(TYPE_AMOUNT, proof.amount.value().to_be_bytes()),
            TlvRecord::new(TYPE_SCRIPT_KEY, proof.script_key.0),
            TlvRecord::new(TYPE_ROOT_HASH, proof.tap_asset_root.hash.0),
            TlvRecord::new(
                TYPE_ROOT_SUM,
                proof.tap_asset_root.sum.value().to_be_bytes(),
            ),
            TlvRecord::new(
                TYPE_VERIFICATION_SCOPE,
                proof.verification_scope.as_str().as_bytes(),
            ),
            TlvRecord::new(TYPE_NETWORK, b"mainnet"),
            TlvRecord::new(TYPE_ASSET_TYPE, [proof.asset_type.as_u8()]),
        ];
        let encoded = encode_stream(&records).expect("proof records encode");

        assert_eq!(
            ProofFile::decode(&encoded),
            Err(ProofError::UnsupportedNetwork("mainnet".to_owned()))
        );
    }

    #[test]
    fn proof_history_replay_accepts_full_lifecycle_explanations() {
        let asset_id = Bytes32([42; 32]);
        let owner = key(2);
        let receiver = key(3);
        let close_owner = key(4);
        let htlc_owner = key(5);
        let export_owner = key(6);
        let htlc_export_owner = key(7);

        let records = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output(
                    "issued",
                    asset_id,
                    1_000,
                    owner,
                    1,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "split",
                ProofTransitionKind::Split,
                2,
                vec!["issued"],
                vec![
                    output(
                        "receiver",
                        asset_id,
                        400,
                        receiver,
                        2,
                        ProofHistoryState::Accepted,
                    ),
                    output(
                        "change",
                        asset_id,
                        600,
                        owner,
                        3,
                        ProofHistoryState::Accepted,
                    ),
                ],
            ),
            record(
                "transfer",
                ProofTransitionKind::Transfer,
                3,
                vec!["receiver"],
                vec![output(
                    "transferred",
                    asset_id,
                    400,
                    htlc_owner,
                    4,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "funding",
                ProofTransitionKind::ChannelFunding,
                4,
                vec!["change"],
                vec![output(
                    "channel",
                    asset_id,
                    600,
                    owner,
                    5,
                    ProofHistoryState::ChannelLocked,
                )],
            ),
            record(
                "commitment",
                ProofTransitionKind::CommitmentUpdate,
                5,
                vec!["channel"],
                vec![output(
                    "channel-v2",
                    asset_id,
                    600,
                    owner,
                    6,
                    ProofHistoryState::ChannelLocked,
                )],
            ),
            record(
                "cooperative-close",
                ProofTransitionKind::CooperativeClose,
                6,
                vec!["channel-v2"],
                vec![output(
                    "closed",
                    asset_id,
                    600,
                    close_owner,
                    7,
                    ProofHistoryState::Closed,
                )],
            ),
            record(
                "sweep",
                ProofTransitionKind::Sweep,
                7,
                vec!["closed"],
                vec![output(
                    "swept",
                    asset_id,
                    600,
                    close_owner,
                    8,
                    ProofHistoryState::Swept,
                )],
            ),
            record(
                "export",
                ProofTransitionKind::ProofExport,
                8,
                vec!["swept"],
                vec![output(
                    "exported",
                    asset_id,
                    600,
                    export_owner,
                    9,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "funding-htlc",
                ProofTransitionKind::ChannelFunding,
                10,
                vec!["transferred"],
                vec![output(
                    "htlc-channel",
                    asset_id,
                    400,
                    htlc_owner,
                    10,
                    ProofHistoryState::ChannelLocked,
                )],
            ),
            record(
                "unilateral-close",
                ProofTransitionKind::UnilateralClose,
                11,
                vec!["htlc-channel"],
                vec![output(
                    "unilateral-closed",
                    asset_id,
                    400,
                    htlc_owner,
                    11,
                    ProofHistoryState::Closed,
                )],
            ),
            record(
                "second-level-htlc",
                ProofTransitionKind::SecondLevelHtlc,
                12,
                vec!["unilateral-closed"],
                vec![output(
                    "second-level",
                    asset_id,
                    400,
                    htlc_owner,
                    12,
                    ProofHistoryState::Closed,
                )],
            ),
            record(
                "sweep-htlc",
                ProofTransitionKind::Sweep,
                13,
                vec!["second-level"],
                vec![output(
                    "swept-htlc",
                    asset_id,
                    400,
                    htlc_owner,
                    13,
                    ProofHistoryState::Swept,
                )],
            ),
            record(
                "export-htlc",
                ProofTransitionKind::ProofExport,
                14,
                vec!["swept-htlc"],
                vec![output(
                    "exported-htlc",
                    asset_id,
                    400,
                    htlc_export_owner,
                    14,
                    ProofHistoryState::Accepted,
                )],
            ),
        ];

        let replay = ProofHistoryEngine::replay(&records).expect("history replays");
        let accepted = replay
            .accepted_explanations()
            .map(|explanation| explanation.output_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(accepted, vec!["exported", "exported-htlc"]);
        assert_eq!(
            replay.output_state("issued"),
            Some(ProofHistoryState::Spent)
        );
        assert_eq!(
            replay.output_state("channel-v2"),
            Some(ProofHistoryState::Spent)
        );
        let exported = replay
            .accepted_explanation("exported")
            .expect("exported explanation exists");
        assert_eq!(exported.amount, AssetAmount::new(600));
        assert_eq!(exported.resulting_state, ProofHistoryState::Accepted);
        assert_eq!(exported.prior_states[0].state, ProofHistoryState::Swept);
    }

    #[test]
    fn proof_history_replay_rejects_missing_inputs_without_balances() {
        let records = vec![record(
            "transfer",
            ProofTransitionKind::Transfer,
            1,
            vec!["missing"],
            vec![output(
                "receiver",
                Bytes32([42; 32]),
                100,
                key(3),
                1,
                ProofHistoryState::Accepted,
            )],
        )];

        assert!(matches!(
            ProofHistoryEngine::replay(&records),
            Err(ProofHistoryReplayError::MissingInput {
                record_id,
                input_output_id,
            }) if record_id == "transfer" && input_output_id == "missing"
        ));
    }

    #[test]
    fn proof_history_replay_rejects_contradictory_amounts() {
        let asset_id = Bytes32([42; 32]);
        let records = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output(
                    "issued",
                    asset_id,
                    100,
                    key(2),
                    1,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "transfer",
                ProofTransitionKind::Transfer,
                2,
                vec!["issued"],
                vec![output(
                    "receiver",
                    asset_id,
                    99,
                    key(3),
                    2,
                    ProofHistoryState::Accepted,
                )],
            ),
        ];

        assert!(matches!(
            ProofHistoryEngine::replay(&records),
            Err(ProofHistoryReplayError::AmountNotConserved {
                record_id,
                input: 100,
                output: 99,
            }) if record_id == "transfer"
        ));
    }

    #[test]
    fn proof_history_replay_rejects_duplicate_and_mixed_asset_inputs() {
        let asset_a = Bytes32([42; 32]);
        let asset_b = Bytes32([43; 32]);
        let duplicate_input_records = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output(
                    "issued",
                    asset_a,
                    100,
                    key(2),
                    1,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "merge",
                ProofTransitionKind::Transfer,
                2,
                vec!["issued", "issued"],
                vec![output(
                    "merged",
                    asset_a,
                    200,
                    key(3),
                    2,
                    ProofHistoryState::Accepted,
                )],
            ),
        ];
        assert!(matches!(
            ProofHistoryEngine::replay(&duplicate_input_records),
            Err(ProofHistoryReplayError::DuplicateInput {
                record_id,
                input_output_id,
            }) if record_id == "merge" && input_output_id == "issued"
        ));

        let mixed_asset_records = vec![
            record(
                "issue-a",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output(
                    "asset-a",
                    asset_a,
                    100,
                    key(2),
                    1,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "issue-b",
                ProofTransitionKind::Issuance,
                2,
                vec![],
                vec![output(
                    "asset-b",
                    asset_b,
                    50,
                    key(3),
                    2,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "merge",
                ProofTransitionKind::Transfer,
                3,
                vec!["asset-a", "asset-b"],
                vec![output(
                    "mixed",
                    asset_a,
                    150,
                    key(4),
                    3,
                    ProofHistoryState::Accepted,
                )],
            ),
        ];
        assert!(matches!(
            ProofHistoryEngine::replay(&mixed_asset_records),
            Err(ProofHistoryReplayError::AssetMismatch {
                record_id,
                expected,
                actual,
            }) if record_id == "merge" && expected == asset_a && actual == asset_b
        ));
    }

    #[test]
    fn proof_history_replay_rejects_invalid_output_state_and_root() {
        let asset_id = Bytes32([42; 32]);
        let pending_issuance = vec![record(
            "issue",
            ProofTransitionKind::Issuance,
            1,
            vec![],
            vec![output(
                "issued",
                asset_id,
                100,
                key(2),
                1,
                ProofHistoryState::Pending,
            )],
        )];
        assert!(matches!(
            ProofHistoryEngine::replay(&pending_issuance),
            Err(ProofHistoryReplayError::InvalidOutputState { .. })
        ));

        let mut wrong_root = output(
            "issued",
            asset_id,
            100,
            key(2),
            1,
            ProofHistoryState::Accepted,
        );
        wrong_root.tap_asset_root = derive_hash_sum_root(&[AssetLeaf {
            asset_id,
            script_key: key(2),
            amount: AssetAmount::new(99),
        }])
        .expect("wrong root derives");
        let records = vec![record(
            "issue",
            ProofTransitionKind::Issuance,
            1,
            vec![],
            vec![wrong_root],
        )];
        assert!(matches!(
            ProofHistoryEngine::replay(&records),
            Err(ProofHistoryReplayError::OutputRootMismatch { .. })
        ));
    }

    #[test]
    fn proof_history_replay_rejects_stale_reorg_sensitive_history() {
        let asset_id = Bytes32([8; 32]);
        let stale_issuance = vec![record(
            "stale-reorg-issuance",
            ProofTransitionKind::Issuance,
            1,
            vec![],
            vec![output(
                "stale-output",
                asset_id,
                100,
                key(2),
                1,
                ProofHistoryState::Stale,
            )],
        )];
        assert!(matches!(
            ProofHistoryEngine::replay(&stale_issuance),
            Err(ProofHistoryReplayError::InvalidOutputState {
                record_id,
                output_id,
                state,
                ..
            }) if record_id == "stale-reorg-issuance"
                && output_id == "stale-output"
                && state == ProofHistoryState::Stale
        ));

        let accepted_then_stale = vec![
            record(
                "issue",
                ProofTransitionKind::Issuance,
                1,
                vec![],
                vec![output(
                    "issued",
                    asset_id,
                    100,
                    key(2),
                    1,
                    ProofHistoryState::Accepted,
                )],
            ),
            record(
                "reorg-sensitive-transfer",
                ProofTransitionKind::Transfer,
                2,
                vec!["issued"],
                vec![output(
                    "reorged",
                    asset_id,
                    100,
                    key(3),
                    2,
                    ProofHistoryState::Stale,
                )],
            ),
        ];
        assert!(matches!(
            ProofHistoryEngine::replay(&accepted_then_stale),
            Err(ProofHistoryReplayError::InvalidOutputState {
                record_id,
                output_id,
                state,
                ..
            }) if record_id == "reorg-sensitive-transfer"
                && output_id == "reorged"
                && state == ProofHistoryState::Stale
        ));
    }

    #[test]
    fn proof_history_replay_applies_anchor_policy() {
        let asset_id = Bytes32([10; 32]);
        let confirmed_output = output(
            "confirmed-output",
            asset_id,
            100,
            key(2),
            1,
            ProofHistoryState::Accepted,
        );
        let confirmed_anchor = confirmed_output.anchor_outpoint.clone();
        let records = vec![record(
            "issue",
            ProofTransitionKind::Issuance,
            1,
            vec![],
            vec![confirmed_output],
        )];

        assert!(matches!(
            ProofHistoryEngine::replay_with_anchor_policy(
                &records,
                &ProofAnchorPolicy::strict_confirmed()
            ),
            Err(ProofHistoryReplayError::UnacceptableAnchorState {
                output_id,
                anchor_state: ProofAnchorState::Unknown,
                ..
            }) if output_id == "confirmed-output"
        ));

        let replay = ProofHistoryEngine::replay_with_anchor_policy(
            &records,
            &ProofAnchorPolicy::strict_confirmed()
                .with_anchor_state(&confirmed_anchor, ProofAnchorState::Confirmed),
        )
        .expect("confirmed anchor replays");
        assert_eq!(
            replay
                .accepted_explanation("confirmed-output")
                .expect("accepted explanation")
                .anchor_state,
            ProofAnchorState::Confirmed
        );

        assert!(matches!(
            ProofHistoryEngine::replay_with_anchor_policy(
                &records,
                &ProofAnchorPolicy::strict_confirmed()
                    .with_anchor_state(&confirmed_anchor, ProofAnchorState::Stale),
            ),
            Err(ProofHistoryReplayError::UnacceptableAnchorState {
                output_id,
                anchor_state: ProofAnchorState::Stale,
                ..
            }) if output_id == "confirmed-output"
        ));
        assert!(matches!(
            ProofHistoryEngine::replay_with_anchor_policy(
                &records,
                &ProofAnchorPolicy::strict_confirmed()
                    .with_anchor_state(&confirmed_anchor, ProofAnchorState::Reorged),
            ),
            Err(ProofHistoryReplayError::UnacceptableAnchorState {
                output_id,
                anchor_state: ProofAnchorState::Reorged,
                ..
            }) if output_id == "confirmed-output"
        ));
        assert!(matches!(
            ProofHistoryEngine::replay_with_anchor_policy(
                &records,
                &ProofAnchorPolicy::strict_confirmed()
                    .with_anchor_state(&confirmed_anchor, ProofAnchorState::Pending),
            ),
            Err(ProofHistoryReplayError::UnacceptableAnchorState {
                output_id,
                anchor_state: ProofAnchorState::Pending,
                ..
            }) if output_id == "confirmed-output"
        ));

        let pending_replay = ProofHistoryEngine::replay_with_anchor_policy(
            &records,
            &ProofAnchorPolicy::strict_confirmed()
                .with_pending_accepted(true)
                .with_anchor_state(&confirmed_anchor, ProofAnchorState::Pending),
        )
        .expect("pending anchor can be policy accepted");
        assert_eq!(
            pending_replay
                .accepted_explanation("confirmed-output")
                .expect("accepted pending explanation")
                .anchor_state,
            ProofAnchorState::Pending
        );

        let replacement_output = output(
            "replacement-output",
            asset_id,
            100,
            key(2),
            2,
            ProofHistoryState::Accepted,
        );
        let replacement_anchor = replacement_output.anchor_outpoint.clone();
        let replacement_records = vec![record(
            "replacement-issue",
            ProofTransitionKind::Issuance,
            2,
            vec![],
            vec![replacement_output],
        )];
        ProofHistoryEngine::replay_with_anchor_policy(
            &replacement_records,
            &ProofAnchorPolicy::strict_confirmed()
                .with_anchor_state(confirmed_anchor, ProofAnchorState::Reorged)
                .with_anchor_state(replacement_anchor, ProofAnchorState::Confirmed),
        )
        .expect("confirmed replacement path replays");
    }

    fn record(
        record_id: &str,
        kind: ProofTransitionKind,
        transition_seed: u8,
        inputs: Vec<&str>,
        outputs: Vec<ProofHistoryOutput>,
    ) -> ProofHistoryRecord {
        ProofHistoryRecord {
            record_id: record_id.to_owned(),
            kind,
            virtual_transition_id: Bytes32([transition_seed; 32]),
            inputs: inputs.into_iter().map(ProofHistoryInput::new).collect(),
            outputs,
        }
    }

    fn output(
        output_id: &str,
        asset_id: Bytes32,
        amount: u64,
        script_key: CompressedKey,
        anchor_seed: u8,
        resulting_state: ProofHistoryState,
    ) -> ProofHistoryOutput {
        let amount = AssetAmount::new(amount);
        ProofHistoryOutput {
            output_id: output_id.to_owned(),
            asset_id,
            amount,
            script_key,
            anchor_outpoint: format!("{}:0", Bytes32([anchor_seed; 32]).to_hex()),
            tap_asset_root: derive_hash_sum_root(&[AssetLeaf {
                asset_id,
                script_key,
                amount,
            }])
            .expect("root derives"),
            resulting_state,
        }
    }

    fn key(prefix: u8) -> CompressedKey {
        CompressedKey([prefix; 33])
    }
}
