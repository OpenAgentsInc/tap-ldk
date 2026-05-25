use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_commitment::{AssetCommitmentError, AssetCommitmentStore, build_commitment_update},
    asset_htlc::{
        AssetHtlcCustomRecords, AssetHtlcDecode, AssetHtlcError, AssetHtlcStatus, AssetHtlcStore,
        decode_custom_records, validate_final_hop,
    },
    asset_peer_message::AssetPeerMessage,
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoice, QuoteBoundInvoiceRequest, RfqInvoiceError,
        bind_quote_to_invoice, pay_quote_bound_invoice, receive_native_rfq_request,
    },
    rfq_quote_store::{RfqHtlcAuthorization, RfqQuoteStore},
};

pub const NATIVE_ASSET_PAYMENT_STORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetPaymentStore {
    pub version: u32,
    pub metadata: NativeAssetPaymentStoreMetadata,
    pub payments: BTreeMap<String, NativeAssetPayment>,
}

impl Default for NativeAssetPaymentStore {
    fn default() -> Self {
        Self {
            version: NATIVE_ASSET_PAYMENT_STORE_SCHEMA_VERSION,
            metadata: NativeAssetPaymentStoreMetadata::default(),
            payments: BTreeMap::new(),
        }
    }
}

impl NativeAssetPaymentStore {
    pub fn record_payment(
        &mut self,
        payment: NativeAssetPayment,
    ) -> Result<NativeAssetPayment, NativeAssetPaymentError> {
        if self.payments.contains_key(&payment.payment_id) {
            return Err(NativeAssetPaymentError::DuplicatePayment(
                payment.payment_id,
            ));
        }
        let mut next = self.clone();
        next.payments
            .insert(payment.payment_id.clone(), payment.clone());
        next.validate()?;
        *self = next;
        Ok(payment)
    }

    pub fn inspect_payment(
        &self,
        payment_id: &str,
    ) -> Result<NativeAssetPayment, NativeAssetPaymentError> {
        self.payments
            .get(payment_id)
            .cloned()
            .ok_or_else(|| NativeAssetPaymentError::UnknownPayment(payment_id.to_owned()))
    }

    pub fn validate(&self) -> Result<(), NativeAssetPaymentError> {
        if self.version != NATIVE_ASSET_PAYMENT_STORE_SCHEMA_VERSION {
            return Err(NativeAssetPaymentError::UnsupportedVersion(self.version));
        }

        for (payment_id, payment) in &self.payments {
            if payment_id != &payment.payment_id {
                return Err(NativeAssetPaymentError::StorageInvariant(format!(
                    "payment map key {payment_id} does not match payment_id {}",
                    payment.payment_id
                )));
            }
            if payment.sender_peer.trim().is_empty() || payment.receiver_peer.trim().is_empty() {
                return Err(NativeAssetPaymentError::EmptyPeer);
            }
            if payment.asset_amount == 0 || payment.btc_msat == 0 {
                return Err(NativeAssetPaymentError::InvalidAmount);
            }
            if payment
                .sender_balance_after
                .checked_add(payment.receiver_balance_after)
                .ok_or(NativeAssetPaymentError::BalanceOverflow)?
                != payment.total_balance
            {
                return Err(NativeAssetPaymentError::BalanceNotConserved {
                    sender_balance: payment.sender_balance_after,
                    receiver_balance: payment.receiver_balance_after,
                    total_balance: payment.total_balance,
                });
            }

            match payment.status {
                NativeAssetPaymentStatus::Settled => {
                    if payment.failure_reason.is_some()
                        || payment.htlc_id.is_none()
                        || payment.commitment_number.is_none()
                    {
                        return Err(NativeAssetPaymentError::StorageInvariant(format!(
                            "settled payment {payment_id} is missing durable settlement fields"
                        )));
                    }
                }
                NativeAssetPaymentStatus::Failed => {
                    if payment
                        .failure_reason
                        .as_ref()
                        .is_none_or(|reason| reason.is_empty())
                    {
                        return Err(NativeAssetPaymentError::StorageInvariant(format!(
                            "failed payment {payment_id} has no failure reason"
                        )));
                    }
                    if payment.htlc_id.is_some() || payment.commitment_number.is_some() {
                        return Err(NativeAssetPaymentError::StorageInvariant(format!(
                            "failed payment {payment_id} advanced durable settlement fields"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetPaymentStoreMetadata {
    pub implementation: String,
    pub schema: String,
}

impl Default for NativeAssetPaymentStoreMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk experimental native asset payment store".to_owned(),
            schema: "bounded-regtest-native-asset-payment-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAssetPaymentStatus {
    Settled,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetPayment {
    pub payment_id: String,
    pub channel_id: String,
    pub sender_peer: String,
    pub receiver_peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub quote_id: String,
    pub btc_msat: u64,
    pub payment_hash: Bytes32,
    pub htlc_id: Option<String>,
    pub commitment_number: Option<u64>,
    pub status: NativeAssetPaymentStatus,
    pub failure_reason: Option<String>,
    pub sender_balance_after: u64,
    pub receiver_balance_after: u64,
    pub total_balance: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NativeAssetPaymentRequest {
    pub channel_id: String,
    pub sender_peer: String,
    pub receiver_peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub rfq_id: Bytes32,
    pub invoice_context: Bytes32,
    pub payment_hash: Bytes32,
    pub asset_nonce: Bytes32,
    pub now_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NativeAssetPaymentFailureMode {
    WrongQuote,
    WrongInvoice,
    WrongMetadata,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeAssetPaymentSmokeReport {
    pub payment_id: String,
    pub htlc_id: String,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub sender_balance_before: u64,
    pub receiver_balance_before: u64,
    pub sender_balance_after: u64,
    pub receiver_balance_after: u64,
    pub commitment_number: u64,
    pub payment_status: NativeAssetPaymentStatus,
    pub htlc_status: AssetHtlcStatus,
    pub restart_balances_match: bool,
    pub wrong_quote_rejected: bool,
    pub wrong_invoice_rejected: bool,
    pub wrong_metadata_rejected: bool,
    pub failed_payment_reasons: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PreparedPayment {
    invoice: QuoteBoundInvoice,
    authorization: RfqHtlcAuthorization,
    records: AssetHtlcCustomRecords,
}

pub fn send_native_asset_payment(
    commitment_store: &mut AssetCommitmentStore,
    htlc_store: &mut AssetHtlcStore,
    payment_store: &mut NativeAssetPaymentStore,
    receiver_quote_store: &mut RfqQuoteStore,
    request: NativeAssetPaymentRequest,
) -> Result<NativeAssetPayment, NativeAssetPaymentError> {
    let prepared = prepare_payment(receiver_quote_store, &request, None)?;
    settle_prepared_payment(
        commitment_store,
        htlc_store,
        payment_store,
        request,
        prepared,
    )
}

pub fn try_native_asset_payment_with_failure(
    commitment_store: &mut AssetCommitmentStore,
    htlc_store: &mut AssetHtlcStore,
    payment_store: &mut NativeAssetPaymentStore,
    receiver_quote_store: &mut RfqQuoteStore,
    request: NativeAssetPaymentRequest,
    failure_mode: NativeAssetPaymentFailureMode,
) -> Result<NativeAssetPayment, NativeAssetPaymentError> {
    let before = commitment_store.channel_state(&request.channel_id)?;
    let result = match prepare_payment(receiver_quote_store, &request, Some(failure_mode)) {
        Ok(prepared) => settle_prepared_payment(
            commitment_store,
            htlc_store,
            payment_store,
            request.clone(),
            prepared,
        ),
        Err(err) => Err(err),
    };

    if result.is_err() {
        let after = commitment_store.channel_state(&request.channel_id)?;
        if before.local_balance != after.local_balance
            || before.remote_balance != after.remote_balance
            || before.latest_commitment_number != after.latest_commitment_number
        {
            return Err(NativeAssetPaymentError::FailedPaymentAdvancedState);
        }
    }

    result
}

pub fn run_native_asset_payment_smoke() -> Result<
    (
        NativeAssetPaymentStore,
        AssetCommitmentStore,
        AssetHtlcStore,
        NativeAssetPaymentSmokeReport,
    ),
    NativeAssetPaymentError,
> {
    let (mut commitment_store, initial_state) = initialized_commitment_store()?;
    let channel_id = initial_state.channel_id.clone();
    let request = NativeAssetPaymentRequest {
        channel_id: channel_id.clone(),
        sender_peer: "alice".to_owned(),
        receiver_peer: "bob".to_owned(),
        asset_id: initial_state.asset_id,
        asset_amount: 125,
        rfq_id: Bytes32([41; 32]),
        invoice_context: Bytes32([42; 32]),
        payment_hash: Bytes32([43; 32]),
        asset_nonce: Bytes32([44; 32]),
        now_unix_seconds: 1_000,
    };
    let sender_balance_before = initial_state.local_balance;
    let receiver_balance_before = initial_state.remote_balance;
    let mut htlc_store = AssetHtlcStore::default();
    let mut payment_store = NativeAssetPaymentStore::default();
    let mut receiver_quote_store = RfqQuoteStore::default();

    let payment = send_native_asset_payment(
        &mut commitment_store,
        &mut htlc_store,
        &mut payment_store,
        &mut receiver_quote_store,
        request.clone(),
    )?;
    let htlc_id = payment
        .htlc_id
        .clone()
        .ok_or_else(|| NativeAssetPaymentError::StorageInvariant("missing HTLC ID".to_owned()))?;
    let htlc_status = htlc_store
        .htlcs
        .get(&htlc_id)
        .ok_or_else(|| AssetHtlcError::UnknownHtlc(htlc_id.clone()))?
        .status;
    let commitment_number = payment.commitment_number.ok_or_else(|| {
        NativeAssetPaymentError::StorageInvariant("missing commitment number".to_owned())
    })?;

    let restart_commitments = serde_json::from_slice::<AssetCommitmentStore>(
        &serde_json::to_vec_pretty(&commitment_store)?,
    )?;
    restart_commitments.validate()?;
    let restart_htlcs =
        serde_json::from_slice::<AssetHtlcStore>(&serde_json::to_vec_pretty(&htlc_store)?)?;
    restart_htlcs.validate()?;
    let restart_payments = serde_json::from_slice::<NativeAssetPaymentStore>(
        &serde_json::to_vec_pretty(&payment_store)?,
    )?;
    restart_payments.validate()?;
    let restarted_state = restart_commitments.channel_state(&channel_id)?;
    let restarted_payment = restart_payments.inspect_payment(&payment.payment_id)?;
    let restart_balances_match = restarted_state.local_balance == payment.sender_balance_after
        && restarted_state.remote_balance == payment.receiver_balance_after
        && restarted_payment == payment
        && restart_htlcs
            .htlcs
            .get(&htlc_id)
            .map(|htlc| htlc.status == AssetHtlcStatus::Settled)
            .unwrap_or(false);

    let mut failed_payment_reasons = Vec::new();
    let wrong_quote_rejected = failure_is_rejected(
        request_for_failure(initial_state.asset_id, &channel_id, 51),
        NativeAssetPaymentFailureMode::WrongQuote,
        &mut failed_payment_reasons,
    )?;
    let wrong_invoice_rejected = failure_is_rejected(
        request_for_failure(initial_state.asset_id, &channel_id, 61),
        NativeAssetPaymentFailureMode::WrongInvoice,
        &mut failed_payment_reasons,
    )?;
    let wrong_metadata_rejected = failure_is_rejected(
        request_for_failure(initial_state.asset_id, &channel_id, 71),
        NativeAssetPaymentFailureMode::WrongMetadata,
        &mut failed_payment_reasons,
    )?;

    Ok((
        payment_store,
        commitment_store,
        htlc_store,
        NativeAssetPaymentSmokeReport {
            payment_id: payment.payment_id,
            htlc_id,
            channel_id,
            asset_id: payment.asset_id,
            asset_amount: payment.asset_amount,
            btc_msat: payment.btc_msat,
            sender_balance_before,
            receiver_balance_before,
            sender_balance_after: payment.sender_balance_after,
            receiver_balance_after: payment.receiver_balance_after,
            commitment_number,
            payment_status: payment.status,
            htlc_status,
            restart_balances_match,
            wrong_quote_rejected,
            wrong_invoice_rejected,
            wrong_metadata_rejected,
            failed_payment_reasons,
        },
    ))
}

fn prepare_payment(
    receiver_quote_store: &mut RfqQuoteStore,
    request: &NativeAssetPaymentRequest,
    failure_mode: Option<NativeAssetPaymentFailureMode>,
) -> Result<PreparedPayment, NativeAssetPaymentError> {
    validate_payment_request(request)?;
    let rfq = AssetPeerMessage::RfqRequest {
        rfq_id: request.rfq_id,
        asset_id: request.asset_id,
        asset_amount: request.asset_amount,
        invoice_context: request.invoice_context,
    };
    let accept = receive_native_rfq_request(
        receiver_quote_store,
        &request.sender_peer,
        &rfq,
        request.now_unix_seconds,
        NativeRfqPolicy::default(),
    )?;
    let invoice_expiry_unix_seconds = request
        .now_unix_seconds
        .checked_add(60)
        .ok_or(NativeAssetPaymentError::TimestampOverflow)?;
    let bind_time = request
        .now_unix_seconds
        .checked_add(1)
        .ok_or(NativeAssetPaymentError::TimestampOverflow)?;
    let pay_time = request
        .now_unix_seconds
        .checked_add(2)
        .ok_or(NativeAssetPaymentError::TimestampOverflow)?;
    let validate_time = request
        .now_unix_seconds
        .checked_add(3)
        .ok_or(NativeAssetPaymentError::TimestampOverflow)?;
    let mut invoice = bind_quote_to_invoice(
        &accept.quote,
        QuoteBoundInvoiceRequest {
            invoice: format!("lnbcrt1tapldk{}", &request.payment_hash.to_hex()[..12]),
            payment_hash: request.payment_hash,
            peer: request.sender_peer.clone(),
            asset_id: request.asset_id,
            asset_amount: request.asset_amount,
            btc_msat: accept.quote.btc_msat,
            invoice_context: request.invoice_context,
            invoice_expiry_unix_seconds,
            now_unix_seconds: bind_time,
        },
    )?;

    if matches!(
        failure_mode,
        Some(NativeAssetPaymentFailureMode::WrongQuote)
    ) {
        invoice.quote_id = Bytes32([99; 32]).to_hex();
    }

    let quote_payment = pay_quote_bound_invoice(receiver_quote_store, invoice.clone(), pay_time)?;
    let mut validation_invoice = quote_payment.invoice.clone();
    let mut records = AssetHtlcCustomRecords::from_authorization(
        &quote_payment.invoice,
        &quote_payment.authorization,
    )?;

    match failure_mode {
        Some(NativeAssetPaymentFailureMode::WrongInvoice) => {
            validation_invoice.payment_hash = Bytes32([98; 32]);
        }
        Some(NativeAssetPaymentFailureMode::WrongMetadata) => {
            records.asset_amount = records
                .asset_amount
                .checked_add(1)
                .ok_or(NativeAssetPaymentError::BalanceOverflow)?;
        }
        Some(NativeAssetPaymentFailureMode::WrongQuote) | None => {}
    }

    let encoded = records.encode_tlv()?;
    let decoded = match decode_custom_records(&encoded)? {
        AssetHtlcDecode::Asset(records) => records,
        AssetHtlcDecode::BtcOnly => return Err(NativeAssetPaymentError::MissingAssetHtlc),
    };
    validate_final_hop(
        &decoded,
        &validation_invoice,
        &quote_payment.authorization,
        validate_time,
    )?;

    Ok(PreparedPayment {
        invoice: quote_payment.invoice,
        authorization: quote_payment.authorization,
        records: decoded,
    })
}

fn settle_prepared_payment(
    commitment_store: &mut AssetCommitmentStore,
    htlc_store: &mut AssetHtlcStore,
    payment_store: &mut NativeAssetPaymentStore,
    request: NativeAssetPaymentRequest,
    prepared: PreparedPayment,
) -> Result<NativeAssetPayment, NativeAssetPaymentError> {
    let state = commitment_store.channel_state(&request.channel_id)?;
    if state.asset_id != request.asset_id {
        return Err(NativeAssetPaymentError::AssetIdMismatch {
            expected: state.asset_id,
            actual: request.asset_id,
        });
    }
    let validate_time = request
        .now_unix_seconds
        .checked_add(4)
        .ok_or(NativeAssetPaymentError::TimestampOverflow)?;

    let validation = validate_final_hop(
        &prepared.records,
        &prepared.invoice,
        &prepared.authorization,
        validate_time,
    )?;
    let update = build_commitment_update(&state, validation.asset_amount, 0, request.asset_nonce)?;
    let snapshot = commitment_store.apply_update(update)?;

    let offered = htlc_store.add_htlc(&request.channel_id, validation)?;
    let settled = htlc_store.settle_htlc(&offered.htlc_id)?;
    htlc_store.validate()?;

    let payment = NativeAssetPayment {
        payment_id: payment_id(
            &request.channel_id,
            &prepared.invoice.quote_id,
            prepared.records.payment_hash,
        ),
        channel_id: request.channel_id,
        sender_peer: request.sender_peer,
        receiver_peer: request.receiver_peer,
        asset_id: prepared.records.asset_id,
        asset_amount: prepared.records.asset_amount,
        quote_id: prepared.invoice.quote_id,
        btc_msat: prepared.records.btc_msat,
        payment_hash: prepared.records.payment_hash,
        htlc_id: Some(settled.htlc_id),
        commitment_number: Some(snapshot.commitment_number),
        status: NativeAssetPaymentStatus::Settled,
        failure_reason: None,
        sender_balance_after: snapshot.local_balance,
        receiver_balance_after: snapshot.remote_balance,
        total_balance: snapshot.total_amount,
    };
    payment_store.record_payment(payment)
}

fn failure_is_rejected(
    request: NativeAssetPaymentRequest,
    failure_mode: NativeAssetPaymentFailureMode,
    failed_payment_reasons: &mut Vec<String>,
) -> Result<bool, NativeAssetPaymentError> {
    let (mut commitment_store, initial_state) = initialized_commitment_store()?;
    let mut htlc_store = AssetHtlcStore::default();
    let mut payment_store = NativeAssetPaymentStore::default();
    let mut receiver_quote_store = RfqQuoteStore::default();
    let result = try_native_asset_payment_with_failure(
        &mut commitment_store,
        &mut htlc_store,
        &mut payment_store,
        &mut receiver_quote_store,
        request,
        failure_mode,
    );
    let final_state = commitment_store.channel_state(&initial_state.channel_id)?;
    let no_balance_drift = final_state.local_balance == initial_state.local_balance
        && final_state.remote_balance == initial_state.remote_balance
        && final_state.latest_commitment_number == initial_state.latest_commitment_number
        && htlc_store.htlcs.is_empty()
        && payment_store.payments.is_empty();

    match result {
        Ok(_) => Ok(false),
        Err(err) => {
            failed_payment_reasons.push(err.to_string());
            Ok(no_balance_drift)
        }
    }
}

fn request_for_failure(asset_id: Bytes32, channel_id: &str, seed: u8) -> NativeAssetPaymentRequest {
    NativeAssetPaymentRequest {
        channel_id: channel_id.to_owned(),
        sender_peer: "alice".to_owned(),
        receiver_peer: "bob".to_owned(),
        asset_id,
        asset_amount: 25,
        rfq_id: Bytes32([seed; 32]),
        invoice_context: Bytes32([seed + 1; 32]),
        payment_hash: Bytes32([seed + 2; 32]),
        asset_nonce: Bytes32([seed + 3; 32]),
        now_unix_seconds: 1_000,
    }
}

fn initialized_commitment_store() -> Result<
    (
        AssetCommitmentStore,
        crate::asset_commitment::AssetCommitmentChannelState,
    ),
    NativeAssetPaymentError,
> {
    let (funding_store, funding_report) =
        crate::asset_channel_funding::run_asset_channel_funding_smoke()?;
    let funded = funding_store
        .channels
        .get(&funding_report.channel_id)
        .ok_or_else(|| AssetCommitmentError::UnknownChannel(funding_report.channel_id.clone()))?;
    let mut store = AssetCommitmentStore::default();
    let state = store.initialize_channel(funded)?;
    Ok((store, state))
}

fn validate_payment_request(
    request: &NativeAssetPaymentRequest,
) -> Result<(), NativeAssetPaymentError> {
    if request.sender_peer.trim().is_empty() || request.receiver_peer.trim().is_empty() {
        return Err(NativeAssetPaymentError::EmptyPeer);
    }
    if request.asset_amount == 0 {
        return Err(NativeAssetPaymentError::InvalidAmount);
    }
    Ok(())
}

fn payment_id(channel_id: &str, quote_id: &str, payment_hash: Bytes32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:native-asset-payment-id:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update((quote_id.len() as u64).to_be_bytes());
    hasher.update(quote_id.as_bytes());
    hasher.update(payment_hash.0);
    Bytes32(hasher.finalize().into()).to_hex()
}

#[derive(Debug)]
pub enum NativeAssetPaymentError {
    Json(serde_json::Error),
    Rfq(RfqInvoiceError),
    Commitment(AssetCommitmentError),
    Htlc(AssetHtlcError),
    Funding(crate::asset_channel_funding::AssetChannelFundingError),
    UnsupportedVersion(u32),
    DuplicatePayment(String),
    UnknownPayment(String),
    EmptyPeer,
    InvalidAmount,
    TimestampOverflow,
    MissingAssetHtlc,
    FailedPaymentAdvancedState,
    AssetIdMismatch {
        expected: Bytes32,
        actual: Bytes32,
    },
    BalanceOverflow,
    BalanceNotConserved {
        sender_balance: u64,
        receiver_balance: u64,
        total_balance: u64,
    },
    StorageInvariant(String),
}

impl fmt::Display for NativeAssetPaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "native asset payment JSON error: {err}"),
            Self::Rfq(err) => write!(f, "native asset payment RFQ error: {err}"),
            Self::Commitment(err) => write!(f, "native asset payment commitment error: {err}"),
            Self::Htlc(err) => write!(f, "native asset payment HTLC error: {err}"),
            Self::Funding(err) => write!(f, "native asset payment funding error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "unsupported native asset payment schema version {version}"
                )
            }
            Self::DuplicatePayment(payment_id) => {
                write!(f, "duplicate native asset payment {payment_id}")
            }
            Self::UnknownPayment(payment_id) => {
                write!(f, "unknown native asset payment {payment_id}")
            }
            Self::EmptyPeer => write!(f, "native asset payment peers cannot be empty"),
            Self::InvalidAmount => write!(f, "native asset payment amount must be non-zero"),
            Self::TimestampOverflow => write!(f, "native asset payment timestamp overflowed"),
            Self::MissingAssetHtlc => write!(f, "native asset payment missing asset HTLC records"),
            Self::FailedPaymentAdvancedState => {
                write!(f, "failed native asset payment advanced durable state")
            }
            Self::AssetIdMismatch { expected, actual } => write!(
                f,
                "native asset payment asset mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::BalanceOverflow => write!(f, "native asset payment balance overflowed"),
            Self::BalanceNotConserved {
                sender_balance,
                receiver_balance,
                total_balance,
            } => write!(
                f,
                "native asset payment balances sender={sender_balance} receiver={receiver_balance} do not conserve total={total_balance}"
            ),
            Self::StorageInvariant(message) => {
                write!(
                    f,
                    "native asset payment storage invariant failed: {message}"
                )
            }
        }
    }
}

impl Error for NativeAssetPaymentError {}

impl From<serde_json::Error> for NativeAssetPaymentError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<RfqInvoiceError> for NativeAssetPaymentError {
    fn from(err: RfqInvoiceError) -> Self {
        Self::Rfq(err)
    }
}

impl From<AssetCommitmentError> for NativeAssetPaymentError {
    fn from(err: AssetCommitmentError) -> Self {
        Self::Commitment(err)
    }
}

impl From<AssetHtlcError> for NativeAssetPaymentError {
    fn from(err: AssetHtlcError) -> Self {
        Self::Htlc(err)
    }
}

impl From<crate::asset_channel_funding::AssetChannelFundingError> for NativeAssetPaymentError {
    fn from(err: crate::asset_channel_funding::AssetChannelFundingError) -> Self {
        Self::Funding(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_payment_smoke_settles_and_survives_restart() {
        let (_payment_store, _commitment_store, _htlc_store, report) =
            run_native_asset_payment_smoke().expect("payment smoke passes");
        assert_eq!(report.sender_balance_before, 700);
        assert_eq!(report.receiver_balance_before, 300);
        assert_eq!(report.sender_balance_after, 575);
        assert_eq!(report.receiver_balance_after, 425);
        assert_eq!(report.asset_amount, 125);
        assert_eq!(report.btc_msat, 12_500);
        assert_eq!(report.commitment_number, 1);
        assert_eq!(report.payment_status, NativeAssetPaymentStatus::Settled);
        assert_eq!(report.htlc_status, AssetHtlcStatus::Settled);
        assert!(report.restart_balances_match);
        assert!(report.wrong_quote_rejected);
        assert!(report.wrong_invoice_rejected);
        assert!(report.wrong_metadata_rejected);
        assert_eq!(report.failed_payment_reasons.len(), 3);
    }

    #[test]
    fn wrong_quote_invoice_and_metadata_fail_without_balance_drift() {
        let (commitment_store, state) = initialized_commitment_store().expect("state");
        for (offset, failure_mode) in [
            (80, NativeAssetPaymentFailureMode::WrongQuote),
            (90, NativeAssetPaymentFailureMode::WrongInvoice),
            (100, NativeAssetPaymentFailureMode::WrongMetadata),
        ] {
            let mut commitment_store = commitment_store.clone();
            let mut htlc_store = AssetHtlcStore::default();
            let mut payment_store = NativeAssetPaymentStore::default();
            let mut receiver_quote_store = RfqQuoteStore::default();
            let request = request_for_failure(state.asset_id, &state.channel_id, offset);
            let err = try_native_asset_payment_with_failure(
                &mut commitment_store,
                &mut htlc_store,
                &mut payment_store,
                &mut receiver_quote_store,
                request,
                failure_mode,
            )
            .expect_err("failure mode rejects");
            assert!(!err.to_string().is_empty());
            let after = commitment_store
                .channel_state(&state.channel_id)
                .expect("state reloads");
            assert_eq!(after.local_balance, state.local_balance);
            assert_eq!(after.remote_balance, state.remote_balance);
            assert_eq!(
                after.latest_commitment_number,
                state.latest_commitment_number
            );
            assert!(htlc_store.htlcs.is_empty());
            assert!(payment_store.payments.is_empty());
        }
    }

    #[test]
    fn payment_store_rejects_settled_payment_without_durable_fields() {
        let mut store = NativeAssetPaymentStore::default();
        let payment = NativeAssetPayment {
            payment_id: "payment".to_owned(),
            channel_id: "channel".to_owned(),
            sender_peer: "alice".to_owned(),
            receiver_peer: "bob".to_owned(),
            asset_id: Bytes32([1; 32]),
            asset_amount: 1,
            quote_id: Bytes32([2; 32]).to_hex(),
            btc_msat: 100,
            payment_hash: Bytes32([3; 32]),
            htlc_id: None,
            commitment_number: None,
            status: NativeAssetPaymentStatus::Settled,
            failure_reason: None,
            sender_balance_after: 9,
            receiver_balance_after: 1,
            total_balance: 10,
        };
        assert!(store.record_payment(payment).is_err());
    }
}
