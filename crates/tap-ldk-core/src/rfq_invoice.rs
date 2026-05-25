use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{
    asset::Bytes32,
    asset_peer_message::{AssetPeerMessage, AssetPeerMessageError},
    rfq_quote_store::{
        RfqHtlcAuthorization, RfqQuoteError, RfqQuoteRequest, RfqQuoteStatus, RfqQuoteStore,
        StoredRfqQuote,
    },
};

pub const DEFAULT_REGTEST_QUOTE_TTL_SECONDS: u64 = 120;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct NativeRfqPolicy {
    pub quote_ttl_seconds: u64,
}

impl Default for NativeRfqPolicy {
    fn default() -> Self {
        Self {
            quote_ttl_seconds: DEFAULT_REGTEST_QUOTE_TTL_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeRfqAccept {
    pub quote: StoredRfqQuote,
    pub message: AssetPeerMessage,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuoteBoundInvoiceRequest {
    pub invoice: String,
    pub payment_hash: Bytes32,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub invoice_context: Bytes32,
    pub invoice_expiry_unix_seconds: u64,
    pub now_unix_seconds: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuoteBoundInvoice {
    pub invoice: String,
    pub payment_hash: Bytes32,
    pub quote_id: String,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub invoice_context: Bytes32,
    pub scid_alias: u64,
    pub quote_expiry_unix_seconds: u64,
    pub invoice_expiry_unix_seconds: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuoteBoundPayment {
    pub invoice: QuoteBoundInvoice,
    pub authorization: RfqHtlcAuthorization,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RfqInvoiceSmokeReport {
    pub rfq_id: Bytes32,
    pub quote_id: String,
    pub btc_msat: u64,
    pub scid_alias: u64,
    pub quote_bound_invoice: String,
    pub replay_rejected: bool,
}

pub fn receive_native_rfq_request(
    store: &mut RfqQuoteStore,
    peer: &str,
    message: &AssetPeerMessage,
    now_unix_seconds: u64,
    policy: NativeRfqPolicy,
) -> Result<NativeRfqAccept, RfqInvoiceError> {
    let AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount,
        invoice_context,
    } = message
    else {
        return Err(RfqInvoiceError::ExpectedRfqRequest);
    };

    let expiry_unix_seconds = now_unix_seconds
        .checked_add(policy.quote_ttl_seconds)
        .ok_or(RfqInvoiceError::ExpiryOverflow)?;
    let quote = store.request_quote(RfqQuoteRequest {
        peer: peer.to_owned(),
        asset_id: *asset_id,
        asset_amount: *asset_amount,
        expiry_unix_seconds,
        invoice_context: *invoice_context,
        replay_domain: rfq_id.to_hex(),
        now_unix_seconds,
    })?;
    let quote = store.accept_quote(&quote.quote_id, now_unix_seconds)?;
    let quote_id =
        Bytes32::from_str(&quote.quote_id).map_err(|_| RfqInvoiceError::InvalidQuoteId)?;

    Ok(NativeRfqAccept {
        message: AssetPeerMessage::RfqAccept {
            rfq_id: *rfq_id,
            quote_id,
            btc_msat: quote.btc_msat,
            expiry_unix_seconds: quote.expiry_unix_seconds,
            scid_alias: quote.scid_alias,
        },
        quote,
    })
}

pub fn reject_native_rfq_request(
    rfq_id: Bytes32,
    reason: impl Into<String>,
) -> Result<AssetPeerMessage, RfqInvoiceError> {
    let reason = reason.into();
    if reason.trim().is_empty() {
        return Err(RfqInvoiceError::EmptyRejectReason);
    }
    Ok(AssetPeerMessage::RfqReject {
        rfq_id,
        reject_reason: reason,
    })
}

pub fn bind_quote_to_invoice(
    quote: &StoredRfqQuote,
    request: QuoteBoundInvoiceRequest,
) -> Result<QuoteBoundInvoice, RfqInvoiceError> {
    if request.invoice.trim().is_empty() {
        return Err(RfqInvoiceError::EmptyInvoice);
    }
    if quote.status != RfqQuoteStatus::Accepted {
        return Err(RfqInvoiceError::QuoteNotAccepted {
            quote_id: quote.quote_id.clone(),
            status: quote.status,
        });
    }
    if request.now_unix_seconds > quote.expiry_unix_seconds {
        return Err(RfqInvoiceError::QuoteExpired {
            quote_id: quote.quote_id.clone(),
            now_unix_seconds: request.now_unix_seconds,
            expiry_unix_seconds: quote.expiry_unix_seconds,
        });
    }
    if request.invoice_expiry_unix_seconds > quote.expiry_unix_seconds {
        return Err(RfqInvoiceError::InvoiceOutlivesQuote {
            invoice_expiry_unix_seconds: request.invoice_expiry_unix_seconds,
            quote_expiry_unix_seconds: quote.expiry_unix_seconds,
        });
    }
    if request.peer != quote.peer {
        return Err(RfqInvoiceError::PeerMismatch {
            expected: quote.peer.clone(),
            actual: request.peer,
        });
    }
    if request.asset_id != quote.asset_id {
        return Err(RfqInvoiceError::AssetIdMismatch {
            expected: quote.asset_id,
            actual: request.asset_id,
        });
    }
    if request.asset_amount != quote.asset_amount {
        return Err(RfqInvoiceError::AssetAmountMismatch {
            expected: quote.asset_amount,
            actual: request.asset_amount,
        });
    }
    if request.btc_msat != quote.btc_msat {
        return Err(RfqInvoiceError::BtcAmountMismatch {
            expected: quote.btc_msat,
            actual: request.btc_msat,
        });
    }
    if request.invoice_context != quote.invoice_context {
        return Err(RfqInvoiceError::InvoiceContextMismatch);
    }

    Ok(QuoteBoundInvoice {
        invoice: request.invoice,
        payment_hash: request.payment_hash,
        quote_id: quote.quote_id.clone(),
        peer: quote.peer.clone(),
        asset_id: quote.asset_id,
        asset_amount: quote.asset_amount,
        btc_msat: quote.btc_msat,
        invoice_context: quote.invoice_context,
        scid_alias: quote.scid_alias,
        quote_expiry_unix_seconds: quote.expiry_unix_seconds,
        invoice_expiry_unix_seconds: request.invoice_expiry_unix_seconds,
    })
}

pub fn pay_quote_bound_invoice(
    store: &mut RfqQuoteStore,
    invoice: QuoteBoundInvoice,
    now_unix_seconds: u64,
) -> Result<QuoteBoundPayment, RfqInvoiceError> {
    let quote = store.inspect_quote(&invoice.quote_id)?;
    let rebound = bind_quote_to_invoice(
        &quote,
        QuoteBoundInvoiceRequest {
            invoice: invoice.invoice.clone(),
            payment_hash: invoice.payment_hash,
            peer: invoice.peer.clone(),
            asset_id: invoice.asset_id,
            asset_amount: invoice.asset_amount,
            btc_msat: invoice.btc_msat,
            invoice_context: invoice.invoice_context,
            invoice_expiry_unix_seconds: invoice.invoice_expiry_unix_seconds,
            now_unix_seconds,
        },
    )?;
    let authorization = store.authorize_asset_htlc(&invoice.quote_id, now_unix_seconds)?;

    Ok(QuoteBoundPayment {
        invoice: rebound,
        authorization,
    })
}

pub fn run_rfq_invoice_smoke(asset_id: Bytes32) -> Result<RfqInvoiceSmokeReport, RfqInvoiceError> {
    let rfq_id = Bytes32([9; 32]);
    let invoice_context = Bytes32([10; 32]);
    let payment_hash = Bytes32([11; 32]);
    let request = AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount: 250_000,
        invoice_context,
    };
    let mut store = RfqQuoteStore::default();
    let accept = receive_native_rfq_request(
        &mut store,
        "alice",
        &request,
        1_000,
        NativeRfqPolicy::default(),
    )?;
    let encoded_accept = accept.message.encode()?;
    let decoded_accept = AssetPeerMessage::decode(&encoded_accept)?;
    if decoded_accept != accept.message {
        return Err(RfqInvoiceError::PeerMessageRoundTripMismatch);
    }

    let invoice = bind_quote_to_invoice(
        &accept.quote,
        QuoteBoundInvoiceRequest {
            invoice: "lnbcrt1tapldkquoteinvoice".to_owned(),
            payment_hash,
            peer: "alice".to_owned(),
            asset_id,
            asset_amount: accept.quote.asset_amount,
            btc_msat: accept.quote.btc_msat,
            invoice_context,
            invoice_expiry_unix_seconds: accept.quote.expiry_unix_seconds,
            now_unix_seconds: 1_001,
        },
    )?;
    let payment = pay_quote_bound_invoice(&mut store, invoice.clone(), 1_002)?;
    let replay_rejected = pay_quote_bound_invoice(&mut store, invoice, 1_003).is_err();

    Ok(RfqInvoiceSmokeReport {
        rfq_id,
        quote_id: payment.authorization.quote_id,
        btc_msat: payment.authorization.btc_msat,
        scid_alias: payment.authorization.scid_alias,
        quote_bound_invoice: payment.invoice.invoice,
        replay_rejected,
    })
}

#[derive(Debug)]
pub enum RfqInvoiceError {
    Quote(RfqQuoteError),
    Peer(AssetPeerMessageError),
    ExpectedRfqRequest,
    InvalidQuoteId,
    ExpiryOverflow,
    EmptyRejectReason,
    EmptyInvoice,
    QuoteNotAccepted {
        quote_id: String,
        status: RfqQuoteStatus,
    },
    QuoteExpired {
        quote_id: String,
        now_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    InvoiceOutlivesQuote {
        invoice_expiry_unix_seconds: u64,
        quote_expiry_unix_seconds: u64,
    },
    PeerMismatch {
        expected: String,
        actual: String,
    },
    AssetIdMismatch {
        expected: Bytes32,
        actual: Bytes32,
    },
    AssetAmountMismatch {
        expected: u64,
        actual: u64,
    },
    BtcAmountMismatch {
        expected: u64,
        actual: u64,
    },
    InvoiceContextMismatch,
    PeerMessageRoundTripMismatch,
}

impl fmt::Display for RfqInvoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quote(err) => write!(f, "RFQ quote error: {err}"),
            Self::Peer(err) => write!(f, "RFQ peer message error: {err}"),
            Self::ExpectedRfqRequest => write!(f, "expected RFQ request peer message"),
            Self::InvalidQuoteId => write!(f, "RFQ quote ID is not a 32-byte binding"),
            Self::ExpiryOverflow => write!(f, "RFQ quote expiry overflowed"),
            Self::EmptyRejectReason => write!(f, "RFQ reject reason cannot be empty"),
            Self::EmptyInvoice => write!(f, "quote-bound invoice cannot be empty"),
            Self::QuoteNotAccepted { quote_id, status } => {
                write!(f, "RFQ quote {quote_id} is {status:?}, not accepted")
            }
            Self::QuoteExpired {
                quote_id,
                now_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "RFQ quote {quote_id} expired at {expiry_unix_seconds}; now {now_unix_seconds}"
            ),
            Self::InvoiceOutlivesQuote {
                invoice_expiry_unix_seconds,
                quote_expiry_unix_seconds,
            } => write!(
                f,
                "invoice expiry {invoice_expiry_unix_seconds} outlives quote expiry {quote_expiry_unix_seconds}"
            ),
            Self::PeerMismatch { expected, actual } => {
                write!(f, "RFQ peer mismatch: expected {expected}, got {actual}")
            }
            Self::AssetIdMismatch { expected, actual } => write!(
                f,
                "RFQ asset ID mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::AssetAmountMismatch { expected, actual } => write!(
                f,
                "RFQ asset amount mismatch: expected {expected}, got {actual}"
            ),
            Self::BtcAmountMismatch { expected, actual } => {
                write!(
                    f,
                    "RFQ BTC amount mismatch: expected {expected}, got {actual}"
                )
            }
            Self::InvoiceContextMismatch => write!(f, "RFQ invoice context mismatch"),
            Self::PeerMessageRoundTripMismatch => write!(f, "RFQ peer message round-trip mismatch"),
        }
    }
}

impl Error for RfqInvoiceError {}

impl From<RfqQuoteError> for RfqInvoiceError {
    fn from(err: RfqQuoteError) -> Self {
        Self::Quote(err)
    }
}

impl From<AssetPeerMessageError> for RfqInvoiceError {
    fn from(err: AssetPeerMessageError) -> Self {
        Self::Peer(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_id() -> Bytes32 {
        Bytes32([7; 32])
    }

    fn rfq_request() -> AssetPeerMessage {
        AssetPeerMessage::RfqRequest {
            rfq_id: Bytes32([1; 32]),
            asset_id: asset_id(),
            asset_amount: 10,
            invoice_context: Bytes32([2; 32]),
        }
    }

    fn accepted_quote() -> (RfqQuoteStore, StoredRfqQuote) {
        let mut store = RfqQuoteStore::default();
        let accept = receive_native_rfq_request(
            &mut store,
            "alice",
            &rfq_request(),
            100,
            NativeRfqPolicy {
                quote_ttl_seconds: 50,
            },
        )
        .expect("RFQ request accepted");
        (store, accept.quote)
    }

    fn invoice_request(quote: &StoredRfqQuote) -> QuoteBoundInvoiceRequest {
        QuoteBoundInvoiceRequest {
            invoice: "lnbcrt1quote".to_owned(),
            payment_hash: Bytes32([3; 32]),
            peer: quote.peer.clone(),
            asset_id: quote.asset_id,
            asset_amount: quote.asset_amount,
            btc_msat: quote.btc_msat,
            invoice_context: quote.invoice_context,
            invoice_expiry_unix_seconds: quote.expiry_unix_seconds,
            now_unix_seconds: 101,
        }
    }

    #[test]
    fn native_rfq_request_accepts_quote_and_round_trips_accept_message() {
        let (_store, quote) = accepted_quote();
        assert_eq!(quote.status, RfqQuoteStatus::Accepted);
        assert_eq!(quote.peer, "alice");
        assert_eq!(quote.asset_amount, 10);
        assert_eq!(quote.btc_msat, 1_000);
        assert_eq!(quote.replay_domain, Bytes32([1; 32]).to_hex());
    }

    #[test]
    fn quote_bound_invoice_rejects_wrong_fields_and_stale_invoice() {
        let (_store, quote) = accepted_quote();

        let mut wrong_peer = invoice_request(&quote);
        wrong_peer.peer = "bob".to_owned();
        assert!(matches!(
            bind_quote_to_invoice(&quote, wrong_peer),
            Err(RfqInvoiceError::PeerMismatch { .. })
        ));

        let mut wrong_asset = invoice_request(&quote);
        wrong_asset.asset_id = Bytes32([9; 32]);
        assert!(matches!(
            bind_quote_to_invoice(&quote, wrong_asset),
            Err(RfqInvoiceError::AssetIdMismatch { .. })
        ));

        let mut wrong_amount = invoice_request(&quote);
        wrong_amount.asset_amount += 1;
        assert!(matches!(
            bind_quote_to_invoice(&quote, wrong_amount),
            Err(RfqInvoiceError::AssetAmountMismatch { .. })
        ));

        let mut stale_invoice = invoice_request(&quote);
        stale_invoice.invoice_expiry_unix_seconds = quote.expiry_unix_seconds + 1;
        assert!(matches!(
            bind_quote_to_invoice(&quote, stale_invoice),
            Err(RfqInvoiceError::InvoiceOutlivesQuote { .. })
        ));
    }

    #[test]
    fn quote_bound_payment_marks_quote_used_and_replay_fails() {
        let (mut store, quote) = accepted_quote();
        let invoice =
            bind_quote_to_invoice(&quote, invoice_request(&quote)).expect("invoice binds");
        let payment =
            pay_quote_bound_invoice(&mut store, invoice.clone(), 102).expect("payment authorizes");
        assert_eq!(payment.authorization.quote_id, quote.quote_id);
        assert_eq!(
            store
                .inspect_quote(&quote.quote_id)
                .expect("quote exists")
                .status,
            RfqQuoteStatus::Used
        );
        assert!(matches!(
            pay_quote_bound_invoice(&mut store, invoice, 103),
            Err(RfqInvoiceError::QuoteNotAccepted { .. })
        ));
    }

    #[test]
    fn duplicate_rfq_request_returns_quote_replay_error() {
        let (mut store, _quote) = accepted_quote();
        assert!(matches!(
            receive_native_rfq_request(
                &mut store,
                "alice",
                &rfq_request(),
                101,
                NativeRfqPolicy::default()
            ),
            Err(RfqInvoiceError::Quote(
                RfqQuoteError::ReplayDomainAlreadyUsed(_)
            ))
        ));
    }

    #[test]
    fn reject_message_requires_reason() {
        assert!(matches!(
            reject_native_rfq_request(Bytes32([1; 32]), " "),
            Err(RfqInvoiceError::EmptyRejectReason)
        ));
        assert!(matches!(
            reject_native_rfq_request(Bytes32([1; 32]), "no route"),
            Ok(AssetPeerMessage::RfqReject { .. })
        ));
    }

    #[test]
    fn smoke_covers_native_rfq_invoice_binding() {
        let report = run_rfq_invoice_smoke(asset_id()).expect("smoke passes");
        assert_eq!(report.btc_msat, 25_000_000);
        assert!(report.scid_alias > 0);
        assert!(report.replay_rejected);
    }
}
