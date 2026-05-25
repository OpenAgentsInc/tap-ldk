use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_htlc::{
        AssetHtlcCustomRecords, AssetHtlcDecode, AssetHtlcError, decode_custom_records,
        validate_final_hop,
    },
    asset_peer_message::AssetPeerMessage,
    lightning_labs_funding::{
        FundingInteropGap, LightningLabsFundingInteropError, LightningLabsFundingInteropReport,
        run_lightning_labs_funding_interop_fixture_smoke,
    },
    lightning_labs_rfq::{
        LIGHTNING_LABS_RFQ_ACCEPT_TYPE, LIGHTNING_LABS_RFQ_REJECT_TYPE,
        LIGHTNING_LABS_RFQ_REQUEST_TYPE, LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT,
        LightningLabsRfqAccept, LightningLabsRfqError, LightningLabsRfqRequest,
        lightning_labs_accept_from_invoice, lightning_labs_rfq_id_to_scid_alias,
        lightning_labs_sell_request_from_invoice,
    },
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoiceRequest, RfqInvoiceError, bind_quote_to_invoice,
        pay_quote_bound_invoice, receive_native_rfq_request,
    },
    rfq_quote_store::{RFQ_ALIAS_BASE, RfqQuoteStore},
};

pub const LIGHTNING_LABS_OUTGOING_PAYMENT_SCHEMA_VERSION: u32 = 1;
pub const LIGHTNING_LABS_OUTGOING_PAYMENT_AMOUNT: u64 = 125;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsOutgoingPaymentStore {
    pub version: u32,
    pub metadata: LightningLabsOutgoingPaymentMetadata,
    pub payments: BTreeMap<String, LightningLabsOutgoingPaymentState>,
}

impl Default for LightningLabsOutgoingPaymentStore {
    fn default() -> Self {
        Self {
            version: LIGHTNING_LABS_OUTGOING_PAYMENT_SCHEMA_VERSION,
            metadata: LightningLabsOutgoingPaymentMetadata::default(),
            payments: BTreeMap::new(),
        }
    }
}

impl LightningLabsOutgoingPaymentStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LightningLabsOutgoingPaymentError> {
        let raw =
            fs::read_to_string(path.as_ref()).map_err(LightningLabsOutgoingPaymentError::Io)?;
        let store =
            serde_json::from_str::<Self>(&raw).map_err(LightningLabsOutgoingPaymentError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn save_atomic(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), LightningLabsOutgoingPaymentError> {
        self.validate()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(LightningLabsOutgoingPaymentError::Io)?;
            }
        }

        let raw =
            serde_json::to_vec_pretty(self).map_err(LightningLabsOutgoingPaymentError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(LightningLabsOutgoingPaymentError::Io)?;
        fs::rename(&temp_path, path).map_err(LightningLabsOutgoingPaymentError::Io)?;
        Ok(())
    }

    pub fn insert_payment(
        &mut self,
        payment: LightningLabsOutgoingPaymentState,
    ) -> Result<(), LightningLabsOutgoingPaymentError> {
        if self.payments.contains_key(&payment.payment_id) {
            return Err(LightningLabsOutgoingPaymentError::DuplicatePayment(
                payment.payment_id,
            ));
        }
        let mut next = self.clone();
        next.payments.insert(payment.payment_id.clone(), payment);
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), LightningLabsOutgoingPaymentError> {
        if self.version != LIGHTNING_LABS_OUTGOING_PAYMENT_SCHEMA_VERSION {
            return Err(LightningLabsOutgoingPaymentError::UnsupportedVersion(
                self.version,
            ));
        }
        self.metadata.validate()?;
        for (payment_id, payment) in &self.payments {
            if payment_id != &payment.payment_id {
                return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                    "payment map key does not match payment id".to_owned(),
                ));
            }
            payment.validate()?;
            if payment.binding_id() != *payment_id {
                return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                    "payment id binding hash does not match stored fields".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsOutgoingPaymentMetadata {
    pub implementation: String,
    pub schema: String,
    pub source_commit: String,
}

impl Default for LightningLabsOutgoingPaymentMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk Lightning Labs outgoing payment interop".to_owned(),
            schema: "bounded-outgoing-payment-interop-v1".to_owned(),
            source_commit: LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT.to_owned(),
        }
    }
}

impl LightningLabsOutgoingPaymentMetadata {
    fn validate(&self) -> Result<(), LightningLabsOutgoingPaymentError> {
        for (field, value) in [
            ("implementation", self.implementation.as_str()),
            ("schema", self.schema.as_str()),
            ("source_commit", self.source_commit.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                    format!("metadata {field} is empty"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsOutgoingPaymentStatus {
    StoppedAtLiveDaemonGap,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsOutgoingPaymentState {
    pub payment_id: String,
    pub status: LightningLabsOutgoingPaymentStatus,
    pub channel_id: String,
    pub peer: String,
    pub rfq_id: Bytes32,
    pub quote_id: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub payment_hash: Bytes32,
    pub invoice_context: Bytes32,
    pub lightning_labs_scid_alias: u64,
    pub native_scid_alias: u64,
    pub sender_balance_before: u64,
    pub lightning_labs_receiver_balance_before: u64,
    pub expected_sender_balance_after: u64,
    pub expected_lightning_labs_receiver_balance_after: u64,
    pub funding_interop_id: String,
    pub funding_blob_digest: Bytes32,
    pub commitment_blob_digest: Bytes32,
    pub request_message_type: u64,
    pub accept_message_type: u64,
    pub reject_message_type: u64,
    pub request_data_digest: Bytes32,
    pub accept_data_digest: Bytes32,
    pub asset_htlc_digest: Bytes32,
    pub quote_replay_rejected: bool,
    pub wrong_asset_rejected: bool,
    pub expected_balance_conserved: bool,
    pub restart_state_matches: bool,
    pub observed_lightning_labs_receiver_balance_after: Option<u64>,
    pub documented_gap: FundingInteropGap,
}

impl LightningLabsOutgoingPaymentState {
    fn validate(&self) -> Result<(), LightningLabsOutgoingPaymentError> {
        if self.payment_id.trim().is_empty()
            || self.channel_id.trim().is_empty()
            || self.peer.trim().is_empty()
            || self.quote_id.trim().is_empty()
            || self.funding_interop_id.trim().is_empty()
        {
            return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                "payment identity fields cannot be empty".to_owned(),
            ));
        }
        if self.asset_id == Bytes32::ZERO || self.asset_amount == 0 || self.btc_msat == 0 {
            return Err(LightningLabsOutgoingPaymentError::InvalidAmount);
        }
        if self.sender_balance_before < self.asset_amount {
            return Err(LightningLabsOutgoingPaymentError::BalanceUnderflow);
        }
        let expected_sender = self.sender_balance_before - self.asset_amount;
        let expected_receiver = self
            .lightning_labs_receiver_balance_before
            .checked_add(self.asset_amount)
            .ok_or(LightningLabsOutgoingPaymentError::BalanceOverflow)?;
        if expected_sender != self.expected_sender_balance_after
            || expected_receiver != self.expected_lightning_labs_receiver_balance_after
        {
            return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                "expected outgoing payment balances do not match amount".to_owned(),
            ));
        }
        if !self.expected_balance_conserved
            || !self.quote_replay_rejected
            || !self.wrong_asset_rejected
            || !self.restart_state_matches
        {
            return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                "payment safety checks must be true".to_owned(),
            ));
        }
        if self
            .observed_lightning_labs_receiver_balance_after
            .is_some()
        {
            return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                "bounded outgoing smoke must not claim an observed Lightning Labs receiver balance"
                    .to_owned(),
            ));
        }
        if self.lightning_labs_scid_alias != lightning_labs_rfq_id_to_scid_alias(self.rfq_id) {
            return Err(LightningLabsOutgoingPaymentError::StorageInvariant(
                "Lightning Labs SCID alias does not match RFQ ID".to_owned(),
            ));
        }
        self.documented_gap
            .validate_for_outgoing_payment()
            .map_err(LightningLabsOutgoingPaymentError::StorageInvariant)?;
        Ok(())
    }

    fn binding_id(&self) -> String {
        derive_payment_id(
            &self.channel_id,
            self.rfq_id,
            &self.quote_id,
            self.payment_hash,
            self.asset_id,
            self.asset_amount,
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsOutgoingPaymentReport {
    pub payment_id: String,
    pub status: LightningLabsOutgoingPaymentStatus,
    pub channel_id: String,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub quote_id: String,
    pub request_message_type: u64,
    pub accept_message_type: u64,
    pub reject_message_type: u64,
    pub lightning_labs_scid_alias: u64,
    pub native_scid_alias: u64,
    pub sender_balance_before: u64,
    pub lightning_labs_receiver_balance_before: u64,
    pub expected_sender_balance_after: u64,
    pub expected_lightning_labs_receiver_balance_after: u64,
    pub observed_lightning_labs_receiver_balance_after: Option<u64>,
    pub expected_balance_conserved: bool,
    pub quote_replay_rejected: bool,
    pub wrong_asset_rejected: bool,
    pub restart_state_matches: bool,
    pub request_data_digest: Bytes32,
    pub accept_data_digest: Bytes32,
    pub asset_htlc_digest: Bytes32,
    pub documented_gap: FundingInteropGap,
}

pub fn run_lightning_labs_outgoing_payment_smoke(
    funding_hexdump: &str,
    commitment_hexdump: &str,
) -> Result<
    (
        LightningLabsOutgoingPaymentStore,
        LightningLabsOutgoingPaymentReport,
    ),
    LightningLabsOutgoingPaymentError,
> {
    let (_funding_store, funding_report) =
        run_lightning_labs_funding_interop_fixture_smoke(funding_hexdump, commitment_hexdump)?;
    let state = build_outgoing_payment_state(&funding_report)?;
    let restart_state_matches = serde_json::from_slice::<LightningLabsOutgoingPaymentState>(
        &serde_json::to_vec_pretty(&state)?,
    )? == state;
    let mut state = state;
    state.restart_state_matches = restart_state_matches;

    let mut store = LightningLabsOutgoingPaymentStore::default();
    store.insert_payment(state.clone())?;

    let restart_store = serde_json::from_slice::<LightningLabsOutgoingPaymentStore>(
        &serde_json::to_vec_pretty(&store)?,
    )?;
    restart_store.validate()?;
    let restart_state_matches = restart_store.payments.get(&state.payment_id) == Some(&state);

    let report = LightningLabsOutgoingPaymentReport {
        payment_id: state.payment_id,
        status: state.status,
        channel_id: state.channel_id,
        peer: state.peer,
        asset_id: state.asset_id,
        asset_amount: state.asset_amount,
        btc_msat: state.btc_msat,
        quote_id: state.quote_id,
        request_message_type: state.request_message_type,
        accept_message_type: state.accept_message_type,
        reject_message_type: state.reject_message_type,
        lightning_labs_scid_alias: state.lightning_labs_scid_alias,
        native_scid_alias: state.native_scid_alias,
        sender_balance_before: state.sender_balance_before,
        lightning_labs_receiver_balance_before: state.lightning_labs_receiver_balance_before,
        expected_sender_balance_after: state.expected_sender_balance_after,
        expected_lightning_labs_receiver_balance_after: state
            .expected_lightning_labs_receiver_balance_after,
        observed_lightning_labs_receiver_balance_after: state
            .observed_lightning_labs_receiver_balance_after,
        expected_balance_conserved: state.expected_balance_conserved,
        quote_replay_rejected: state.quote_replay_rejected,
        wrong_asset_rejected: state.wrong_asset_rejected,
        restart_state_matches,
        request_data_digest: state.request_data_digest,
        accept_data_digest: state.accept_data_digest,
        asset_htlc_digest: state.asset_htlc_digest,
        documented_gap: state.documented_gap,
    };

    Ok((store, report))
}

fn build_outgoing_payment_state(
    funding_report: &LightningLabsFundingInteropReport,
) -> Result<LightningLabsOutgoingPaymentState, LightningLabsOutgoingPaymentError> {
    let peer = "lightning-labs-counterparty";
    let now = 1_000;
    let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 42);
    let invoice_context = Bytes32([31; 32]);
    let payment_hash = Bytes32([32; 32]);
    let asset_amount = LIGHTNING_LABS_OUTGOING_PAYMENT_AMOUNT;

    if funding_report.local_balance < asset_amount {
        return Err(LightningLabsOutgoingPaymentError::BalanceUnderflow);
    }

    let mut quote_store = RfqQuoteStore::default();
    let rfq = AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id: funding_report.asset_id,
        asset_amount,
        invoice_context,
    };
    let native_accept = receive_native_rfq_request(
        &mut quote_store,
        peer,
        &rfq,
        now,
        NativeRfqPolicy::default(),
    )?;
    let invoice = bind_quote_to_invoice(
        &native_accept.quote,
        QuoteBoundInvoiceRequest {
            invoice: "lnbcrt1lightninglabsoutgoingpayment".to_owned(),
            payment_hash,
            peer: peer.to_owned(),
            asset_id: funding_report.asset_id,
            asset_amount,
            btc_msat: native_accept.quote.btc_msat,
            invoice_context,
            invoice_expiry_unix_seconds: native_accept.quote.expiry_unix_seconds,
            now_unix_seconds: now + 1,
        },
    )?;
    let quote_payment = pay_quote_bound_invoice(&mut quote_store, invoice.clone(), now + 2)?;
    let records =
        AssetHtlcCustomRecords::from_authorization(&invoice, &quote_payment.authorization)?;
    let htlc_bytes = records.encode_tlv()?;
    let decoded_records = match decode_custom_records(&htlc_bytes)? {
        AssetHtlcDecode::Asset(records) => records,
        AssetHtlcDecode::BtcOnly => {
            return Err(LightningLabsOutgoingPaymentError::MissingAssetHtlc);
        }
    };
    validate_final_hop(
        &decoded_records,
        &invoice,
        &quote_payment.authorization,
        now + 3,
    )?;

    let mut wrong_asset_records = decoded_records.clone();
    wrong_asset_records.asset_amount += 1;
    let wrong_asset_rejected = validate_final_hop(
        &wrong_asset_records,
        &invoice,
        &quote_payment.authorization,
        now + 3,
    )
    .is_err();
    let quote_replay_rejected =
        pay_quote_bound_invoice(&mut quote_store, invoice.clone(), now + 4).is_err();

    let ll_request = lightning_labs_sell_request_from_invoice(&invoice, rfq_id)?;
    let request_data = ll_request.encode()?;
    let decoded_request = LightningLabsRfqRequest::decode(&request_data)?;
    decoded_request.validate_against_invoice(peer, &invoice, now + 2)?;
    let ll_accept = lightning_labs_accept_from_invoice(&invoice, rfq_id)?;
    let accept_data = ll_accept.encode()?;
    let decoded_accept = LightningLabsRfqAccept::decode(&accept_data)?;
    decoded_accept.validate_for_request(&decoded_request, now + 2)?;

    let expected_sender_balance_after = funding_report.local_balance - asset_amount;
    let expected_receiver_balance_after =
        funding_report
            .remote_balance
            .checked_add(asset_amount)
            .ok_or(LightningLabsOutgoingPaymentError::BalanceOverflow)?;
    let expected_balance_conserved = funding_report
        .local_balance
        .checked_add(funding_report.remote_balance)
        .ok_or(LightningLabsOutgoingPaymentError::BalanceOverflow)?
        == expected_sender_balance_after
            .checked_add(expected_receiver_balance_after)
            .ok_or(LightningLabsOutgoingPaymentError::BalanceOverflow)?;

    let payment_id = derive_payment_id(
        &funding_report.interop_id,
        rfq_id,
        &quote_payment.authorization.quote_id,
        payment_hash,
        funding_report.asset_id,
        asset_amount,
    );

    Ok(LightningLabsOutgoingPaymentState {
        payment_id,
        status: LightningLabsOutgoingPaymentStatus::StoppedAtLiveDaemonGap,
        channel_id: funding_report.interop_id.clone(),
        peer: peer.to_owned(),
        rfq_id,
        quote_id: quote_payment.authorization.quote_id,
        asset_id: funding_report.asset_id,
        asset_amount,
        btc_msat: quote_payment.authorization.btc_msat,
        payment_hash,
        invoice_context,
        lightning_labs_scid_alias: lightning_labs_rfq_id_to_scid_alias(rfq_id),
        native_scid_alias: quote_payment.authorization.scid_alias,
        sender_balance_before: funding_report.local_balance,
        lightning_labs_receiver_balance_before: funding_report.remote_balance,
        expected_sender_balance_after,
        expected_lightning_labs_receiver_balance_after: expected_receiver_balance_after,
        funding_interop_id: funding_report.interop_id.clone(),
        funding_blob_digest: funding_report.funding_blob_digest,
        commitment_blob_digest: funding_report.commitment_blob_digest,
        request_message_type: LIGHTNING_LABS_RFQ_REQUEST_TYPE,
        accept_message_type: LIGHTNING_LABS_RFQ_ACCEPT_TYPE,
        reject_message_type: LIGHTNING_LABS_RFQ_REJECT_TYPE,
        request_data_digest: sha256_digest(&request_data),
        accept_data_digest: sha256_digest(&accept_data),
        asset_htlc_digest: sha256_digest(&htlc_bytes),
        quote_replay_rejected,
        wrong_asset_rejected,
        expected_balance_conserved,
        restart_state_matches: false,
        observed_lightning_labs_receiver_balance_after: None,
        documented_gap: FundingInteropGap {
            field: "live_lnd_tapd_payment_settlement".to_owned(),
            reason:
                "sender-side RFQ, invoice binding, HTLC metadata, and expected balance delta are constructed, but no live LND/tapd receiver was driven in this bounded smoke"
                    .to_owned(),
            next_step:
                "run the same artifacts through the headless or Polar-backed Lightning Labs counterparty and replace the expected receiver balance with an observed daemon balance"
                    .to_owned(),
        },
    })
}

fn rfq_id_with_scid_alias(scid_alias: u64) -> Bytes32 {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&scid_alias.to_be_bytes());
    Bytes32(bytes)
}

fn derive_payment_id(
    channel_id: &str,
    rfq_id: Bytes32,
    quote_id: &str,
    payment_hash: Bytes32,
    asset_id: Bytes32,
    asset_amount: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:lightning-labs-outgoing-payment:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(rfq_id.0);
    hasher.update((quote_id.len() as u64).to_be_bytes());
    hasher.update(quote_id.as_bytes());
    hasher.update(payment_hash.0);
    hasher.update(asset_id.0);
    hasher.update(asset_amount.to_be_bytes());
    Bytes32(hasher.finalize().into()).to_hex()
}

fn sha256_digest(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

trait OutgoingPaymentGapValidation {
    fn validate_for_outgoing_payment(&self) -> Result<(), String>;
}

impl OutgoingPaymentGapValidation for FundingInteropGap {
    fn validate_for_outgoing_payment(&self) -> Result<(), String> {
        for (field, value) in [
            ("field", self.field.as_str()),
            ("reason", self.reason.as_str()),
            ("next_step", self.next_step.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("documented gap {field} is empty"));
            }
        }
        Ok(())
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut temp_path = path.to_path_buf();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!("{ext}.tmp"))
        .unwrap_or_else(|| "tmp".to_owned());
    temp_path.set_extension(extension);
    temp_path
}

#[derive(Debug)]
pub enum LightningLabsOutgoingPaymentError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Funding(LightningLabsFundingInteropError),
    Rfq(RfqInvoiceError),
    LightningLabsRfq(LightningLabsRfqError),
    Htlc(AssetHtlcError),
    UnsupportedVersion(u32),
    DuplicatePayment(String),
    InvalidAmount,
    BalanceUnderflow,
    BalanceOverflow,
    MissingAssetHtlc,
    StorageInvariant(String),
}

impl fmt::Display for LightningLabsOutgoingPaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Lightning Labs outgoing payment I/O error: {err}"),
            Self::Json(err) => write!(f, "Lightning Labs outgoing payment JSON error: {err}"),
            Self::Funding(err) => write!(f, "Lightning Labs outgoing payment funding error: {err}"),
            Self::Rfq(err) => write!(f, "Lightning Labs outgoing payment RFQ error: {err}"),
            Self::LightningLabsRfq(err) => {
                write!(f, "Lightning Labs outgoing payment wire RFQ error: {err}")
            }
            Self::Htlc(err) => write!(f, "Lightning Labs outgoing payment HTLC error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported Lightning Labs outgoing payment schema version {version}"
                )
            }
            Self::DuplicatePayment(payment_id) => {
                write!(f, "duplicate Lightning Labs outgoing payment {payment_id}")
            }
            Self::InvalidAmount => write!(f, "Lightning Labs outgoing payment amount is invalid"),
            Self::BalanceUnderflow => {
                write!(f, "Lightning Labs outgoing payment balance underflow")
            }
            Self::BalanceOverflow => write!(f, "Lightning Labs outgoing payment balance overflow"),
            Self::MissingAssetHtlc => {
                write!(f, "Lightning Labs outgoing payment missing asset HTLC")
            }
            Self::StorageInvariant(message) => {
                write!(
                    f,
                    "Lightning Labs outgoing payment storage invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for LightningLabsOutgoingPaymentError {}

impl From<std::io::Error> for LightningLabsOutgoingPaymentError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for LightningLabsOutgoingPaymentError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<LightningLabsFundingInteropError> for LightningLabsOutgoingPaymentError {
    fn from(err: LightningLabsFundingInteropError) -> Self {
        Self::Funding(err)
    }
}

impl From<RfqInvoiceError> for LightningLabsOutgoingPaymentError {
    fn from(err: RfqInvoiceError) -> Self {
        Self::Rfq(err)
    }
}

impl From<LightningLabsRfqError> for LightningLabsOutgoingPaymentError {
    fn from(err: LightningLabsRfqError) -> Self {
        Self::LightningLabsRfq(err)
    }
}

impl From<AssetHtlcError> for LightningLabsOutgoingPaymentError {
    fn from(err: AssetHtlcError) -> Self {
        Self::Htlc(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUNDING_HEXDUMP: &str = include_str!(
        "../../../fixtures/lightning-labs/tapchannelmsg/testdata/funding-blob.hexdump"
    );
    const COMMITMENT_HEXDUMP: &str = include_str!(
        "../../../fixtures/lightning-labs/tapchannelmsg/testdata/commitment-blob.hexdump"
    );

    #[test]
    fn outgoing_payment_smoke_builds_gap_state_and_survives_restart() {
        let (store, report) =
            run_lightning_labs_outgoing_payment_smoke(FUNDING_HEXDUMP, COMMITMENT_HEXDUMP)
                .expect("outgoing smoke passes");
        assert_eq!(
            report.status,
            LightningLabsOutgoingPaymentStatus::StoppedAtLiveDaemonGap
        );
        assert_eq!(report.asset_amount, LIGHTNING_LABS_OUTGOING_PAYMENT_AMOUNT);
        assert_eq!(
            report.expected_sender_balance_after,
            report.sender_balance_before - report.asset_amount
        );
        assert_eq!(
            report.expected_lightning_labs_receiver_balance_after,
            report.lightning_labs_receiver_balance_before + report.asset_amount
        );
        assert_eq!(report.observed_lightning_labs_receiver_balance_after, None);
        assert!(report.expected_balance_conserved);
        assert!(report.quote_replay_rejected);
        assert!(report.wrong_asset_rejected);
        assert!(report.restart_state_matches);
        assert!(store.payments.contains_key(&report.payment_id));
    }

    #[test]
    fn outgoing_payment_store_rejects_claimed_observed_balance_in_gap_state() {
        let (store, report) =
            run_lightning_labs_outgoing_payment_smoke(FUNDING_HEXDUMP, COMMITMENT_HEXDUMP)
                .expect("outgoing smoke passes");
        let mut state = store
            .payments
            .get(&report.payment_id)
            .cloned()
            .expect("payment exists");
        state.observed_lightning_labs_receiver_balance_after =
            Some(state.expected_lightning_labs_receiver_balance_after);
        assert!(state.validate().is_err());
    }
}
