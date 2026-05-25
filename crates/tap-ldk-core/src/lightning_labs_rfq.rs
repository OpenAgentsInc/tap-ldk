use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_peer_message::{AssetPeerMessage, RFQ_ACCEPT_TYPE, RFQ_REJECT_TYPE, RFQ_REQUEST_TYPE},
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoice, QuoteBoundInvoiceRequest, RfqInvoiceError,
        bind_quote_to_invoice, pay_quote_bound_invoice, receive_native_rfq_request,
    },
    rfq_quote_store::{RFQ_ALIAS_BASE, RfqQuoteStore},
    tlv::{TlvError, TlvRecord, decode_stream, encode_stream, reject_unknown_required},
};

pub const LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT: &str = "743db21da57b5fdecf5daca9a925f0261ca94e40";
pub const LIGHTNING_LABS_RFQ_MESSAGE_TYPE_BASE: u64 = 32_768 + 20_116;
pub const LIGHTNING_LABS_RFQ_REQUEST_TYPE: u64 = LIGHTNING_LABS_RFQ_MESSAGE_TYPE_BASE;
pub const LIGHTNING_LABS_RFQ_ACCEPT_TYPE: u64 = LIGHTNING_LABS_RFQ_MESSAGE_TYPE_BASE + 1;
pub const LIGHTNING_LABS_RFQ_REJECT_TYPE: u64 = LIGHTNING_LABS_RFQ_MESSAGE_TYPE_BASE + 2;
pub const LIGHTNING_LABS_RFQ_WIRE_VERSION: u8 = 1;
pub const LIGHTNING_LABS_MAX_ORACLE_METADATA_LENGTH: usize = 32_768;
pub const LIGHTNING_LABS_MSAT_PER_BTC_COEFFICIENT: u64 = 100;
pub const LIGHTNING_LABS_MSAT_PER_BTC_SCALE: u8 = 9;
pub const MSAT_PER_BTC: u128 = 100_000_000_000;

const TYPE_VERSION: u64 = 0;
const TYPE_ID: u64 = 2;
const TYPE_REQUEST_TRANSFER_TYPE: u64 = 4;
const TYPE_ACCEPT_EXPIRY: u64 = 4;
const TYPE_REQUEST_EXPIRY: u64 = 6;
const TYPE_ACCEPT_SIGNATURE: u64 = 6;
const TYPE_ACCEPT_IN_ASSET_RATE: u64 = 8;
const TYPE_ACCEPT_OUT_ASSET_RATE: u64 = 10;
const TYPE_ACCEPT_MAX_IN_ASSET: u64 = 11;
const TYPE_REQUEST_IN_ASSET_ID: u64 = 9;
const TYPE_REQUEST_IN_ASSET_GROUP_KEY: u64 = 11;
const TYPE_REQUEST_OUT_ASSET_ID: u64 = 13;
const TYPE_REQUEST_OUT_ASSET_GROUP_KEY: u64 = 15;
const TYPE_REQUEST_MAX_IN_ASSET: u64 = 16;
const TYPE_REQUEST_IN_ASSET_RATE_HINT: u64 = 19;
const TYPE_REQUEST_OUT_ASSET_RATE_HINT: u64 = 21;
const TYPE_REQUEST_MIN_IN_ASSET: u64 = 23;
const TYPE_REQUEST_MIN_OUT_ASSET: u64 = 25;
const TYPE_REQUEST_ORACLE_METADATA: u64 = 27;
const TYPE_REQUEST_ASSET_RATE_LIMIT: u64 = 29;
const TYPE_REQUEST_EXECUTION_POLICY: u64 = 31;
const TYPE_REJECT_ERROR: u64 = 5;

const KNOWN_REQUEST_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_ID,
    TYPE_REQUEST_TRANSFER_TYPE,
    TYPE_REQUEST_EXPIRY,
    TYPE_REQUEST_IN_ASSET_ID,
    TYPE_REQUEST_IN_ASSET_GROUP_KEY,
    TYPE_REQUEST_OUT_ASSET_ID,
    TYPE_REQUEST_OUT_ASSET_GROUP_KEY,
    TYPE_REQUEST_MAX_IN_ASSET,
    TYPE_REQUEST_IN_ASSET_RATE_HINT,
    TYPE_REQUEST_OUT_ASSET_RATE_HINT,
    TYPE_REQUEST_MIN_IN_ASSET,
    TYPE_REQUEST_MIN_OUT_ASSET,
    TYPE_REQUEST_ORACLE_METADATA,
    TYPE_REQUEST_ASSET_RATE_LIMIT,
    TYPE_REQUEST_EXECUTION_POLICY,
];

const KNOWN_ACCEPT_TYPES: &[u64] = &[
    TYPE_VERSION,
    TYPE_ID,
    TYPE_ACCEPT_EXPIRY,
    TYPE_ACCEPT_SIGNATURE,
    TYPE_ACCEPT_IN_ASSET_RATE,
    TYPE_ACCEPT_OUT_ASSET_RATE,
    TYPE_ACCEPT_MAX_IN_ASSET,
];

const KNOWN_REJECT_TYPES: &[u64] = &[TYPE_VERSION, TYPE_ID, TYPE_REJECT_ERROR];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsTransferType {
    PayInvoice,
    ReceivePayment,
}

impl LightningLabsTransferType {
    fn from_wire(value: u8) -> Result<Self, LightningLabsRfqError> {
        match value {
            1 => Ok(Self::PayInvoice),
            2 => Ok(Self::ReceivePayment),
            other => Err(LightningLabsRfqError::UnknownTransferType(other)),
        }
    }

    fn wire_value(self) -> u8 {
        match self {
            Self::PayInvoice => 1,
            Self::ReceivePayment => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsExecutionPolicy {
    ImmediateOrCancel,
    FillOrKill,
}

impl LightningLabsExecutionPolicy {
    fn from_wire(value: u8) -> Result<Self, LightningLabsRfqError> {
        match value {
            0 => Ok(Self::ImmediateOrCancel),
            1 => Ok(Self::FillOrKill),
            other => Err(LightningLabsRfqError::UnknownExecutionPolicy(other)),
        }
    }

    fn wire_value(self) -> u8 {
        match self {
            Self::ImmediateOrCancel => 0,
            Self::FillOrKill => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightningLabsRejectCode {
    PriceOracleUnspecified,
    PriceOracleUnavailable,
    MinFillNotMet,
    PriceBoundMiss,
    FillOrKillNotViable,
    FillExceedsMax,
}

impl LightningLabsRejectCode {
    fn from_wire(value: u8) -> Result<Self, LightningLabsRfqError> {
        match value {
            0 => Ok(Self::PriceOracleUnspecified),
            1 => Ok(Self::PriceOracleUnavailable),
            2 => Ok(Self::MinFillNotMet),
            3 => Ok(Self::PriceBoundMiss),
            4 => Ok(Self::FillOrKillNotViable),
            5 => Ok(Self::FillExceedsMax),
            other => Err(LightningLabsRfqError::UnknownRejectCode(other)),
        }
    }

    fn wire_value(self) -> u8 {
        match self {
            Self::PriceOracleUnspecified => 0,
            Self::PriceOracleUnavailable => 1,
            Self::MinFillNotMet => 2,
            Self::PriceBoundMiss => 3,
            Self::FillOrKillNotViable => 4,
            Self::FillExceedsMax => 5,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsFixedPoint {
    pub scale: u8,
    pub coefficient_be: Vec<u8>,
}

impl LightningLabsFixedPoint {
    pub fn from_u64(coefficient: u64, scale: u8) -> Self {
        Self {
            scale,
            coefficient_be: coefficient
                .to_be_bytes()
                .into_iter()
                .skip_while(|b| *b == 0)
                .collect(),
        }
    }

    pub fn msat_per_btc() -> Self {
        Self::from_u64(
            LIGHTNING_LABS_MSAT_PER_BTC_COEFFICIENT,
            LIGHTNING_LABS_MSAT_PER_BTC_SCALE,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, LightningLabsRfqError> {
        self.validate("fixed_point")?;
        let mut encoded = Vec::with_capacity(1 + self.coefficient_be.len());
        encoded.push(self.scale);
        encoded.extend_from_slice(&self.coefficient_be);
        Ok(encoded)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LightningLabsRfqError> {
        let Some((scale, coefficient_be)) = bytes.split_first() else {
            return Err(LightningLabsRfqError::InvalidFixedPointLength(0));
        };
        let value = Self {
            scale: *scale,
            coefficient_be: coefficient_be.to_vec(),
        };
        value.validate("fixed_point")?;
        Ok(value)
    }

    fn validate(&self, field: &'static str) -> Result<(), LightningLabsRfqError> {
        if self.coefficient_be.len() > 16 {
            return Err(LightningLabsRfqError::FixedPointTooWide {
                field,
                actual: self.coefficient_be.len(),
            });
        }
        Ok(())
    }

    fn is_zero(&self) -> bool {
        self.coefficient_be.iter().all(|byte| *byte == 0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsRfqRequest {
    pub version: u8,
    pub id: Bytes32,
    pub transfer_type: LightningLabsTransferType,
    pub expiry_unix_seconds: u64,
    pub in_asset_id: Option<Bytes32>,
    pub in_asset_group_key: Option<Vec<u8>>,
    pub out_asset_id: Option<Bytes32>,
    pub out_asset_group_key: Option<Vec<u8>>,
    pub max_in_asset: u64,
    pub in_asset_rate_hint: Option<LightningLabsFixedPoint>,
    pub out_asset_rate_hint: Option<LightningLabsFixedPoint>,
    pub min_in_asset: Option<u64>,
    pub min_out_asset: Option<u64>,
    pub oracle_metadata: Vec<u8>,
    pub asset_rate_limit: Option<LightningLabsFixedPoint>,
    pub execution_policy: Option<LightningLabsExecutionPolicy>,
}

impl LightningLabsRfqRequest {
    pub fn encode(&self) -> Result<Vec<u8>, LightningLabsRfqError> {
        self.validate_shape()?;

        let mut records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ID, self.id.0),
            TlvRecord::new(
                TYPE_REQUEST_TRANSFER_TYPE,
                [self.transfer_type.wire_value()],
            ),
            TlvRecord::new(TYPE_REQUEST_EXPIRY, self.expiry_unix_seconds.to_be_bytes()),
            TlvRecord::new(TYPE_REQUEST_MAX_IN_ASSET, self.max_in_asset.to_be_bytes()),
        ];

        if let Some(asset_id) = self.in_asset_id {
            records.push(TlvRecord::new(TYPE_REQUEST_IN_ASSET_ID, asset_id.0));
        }
        if let Some(group_key) = &self.in_asset_group_key {
            records.push(TlvRecord::new(
                TYPE_REQUEST_IN_ASSET_GROUP_KEY,
                group_key.clone(),
            ));
        }
        if let Some(asset_id) = self.out_asset_id {
            records.push(TlvRecord::new(TYPE_REQUEST_OUT_ASSET_ID, asset_id.0));
        }
        if let Some(group_key) = &self.out_asset_group_key {
            records.push(TlvRecord::new(
                TYPE_REQUEST_OUT_ASSET_GROUP_KEY,
                group_key.clone(),
            ));
        }
        if let Some(rate) = &self.in_asset_rate_hint {
            records.push(TlvRecord::new(
                TYPE_REQUEST_IN_ASSET_RATE_HINT,
                rate.encode()?,
            ));
        }
        if let Some(rate) = &self.out_asset_rate_hint {
            records.push(TlvRecord::new(
                TYPE_REQUEST_OUT_ASSET_RATE_HINT,
                rate.encode()?,
            ));
        }
        if let Some(amount) = self.min_in_asset {
            records.push(TlvRecord::new(
                TYPE_REQUEST_MIN_IN_ASSET,
                amount.to_be_bytes(),
            ));
        }
        if let Some(amount) = self.min_out_asset {
            records.push(TlvRecord::new(
                TYPE_REQUEST_MIN_OUT_ASSET,
                amount.to_be_bytes(),
            ));
        }
        if !self.oracle_metadata.is_empty() {
            records.push(TlvRecord::new(
                TYPE_REQUEST_ORACLE_METADATA,
                self.oracle_metadata.clone(),
            ));
        }
        if let Some(rate) = &self.asset_rate_limit {
            records.push(TlvRecord::new(
                TYPE_REQUEST_ASSET_RATE_LIMIT,
                rate.encode()?,
            ));
        }
        if let Some(policy) = self.execution_policy {
            records.push(TlvRecord::new(
                TYPE_REQUEST_EXECUTION_POLICY,
                [policy.wire_value()],
            ));
        }

        records.sort_by_key(|record| record.type_id);
        encode_stream(&records).map_err(LightningLabsRfqError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LightningLabsRfqError> {
        let records = decode_stream(bytes).map_err(LightningLabsRfqError::Tlv)?;
        reject_unknown_required(&records, KNOWN_REQUEST_TYPES)
            .map_err(LightningLabsRfqError::Tlv)?;
        let fields = record_map(records);
        let request = Self {
            version: parse_u8(required(&fields, TYPE_VERSION)?, "version")?,
            id: parse_bytes32(required(&fields, TYPE_ID)?)?,
            transfer_type: LightningLabsTransferType::from_wire(parse_u8(
                required(&fields, TYPE_REQUEST_TRANSFER_TYPE)?,
                "transfer_type",
            )?)?,
            expiry_unix_seconds: parse_u64(
                required(&fields, TYPE_REQUEST_EXPIRY)?,
                "expiry_unix_seconds",
            )?,
            in_asset_id: optional_bytes32(&fields, TYPE_REQUEST_IN_ASSET_ID)?,
            in_asset_group_key: optional_group_key(&fields, TYPE_REQUEST_IN_ASSET_GROUP_KEY)?,
            out_asset_id: optional_bytes32(&fields, TYPE_REQUEST_OUT_ASSET_ID)?,
            out_asset_group_key: optional_group_key(&fields, TYPE_REQUEST_OUT_ASSET_GROUP_KEY)?,
            max_in_asset: parse_u64(
                required(&fields, TYPE_REQUEST_MAX_IN_ASSET)?,
                "max_in_asset",
            )?,
            in_asset_rate_hint: optional_fixed_point(&fields, TYPE_REQUEST_IN_ASSET_RATE_HINT)?,
            out_asset_rate_hint: optional_fixed_point(&fields, TYPE_REQUEST_OUT_ASSET_RATE_HINT)?,
            min_in_asset: optional_u64(&fields, TYPE_REQUEST_MIN_IN_ASSET, "min_in_asset")?,
            min_out_asset: optional_u64(&fields, TYPE_REQUEST_MIN_OUT_ASSET, "min_out_asset")?,
            oracle_metadata: fields
                .get(&TYPE_REQUEST_ORACLE_METADATA)
                .cloned()
                .unwrap_or_default(),
            asset_rate_limit: optional_fixed_point(&fields, TYPE_REQUEST_ASSET_RATE_LIMIT)?,
            execution_policy: optional_execution_policy(&fields)?,
        };
        request.validate_shape()?;
        Ok(request)
    }

    pub fn validate_at(&self, now_unix_seconds: u64) -> Result<(), LightningLabsRfqError> {
        self.validate_shape()?;
        if self.expiry_unix_seconds <= now_unix_seconds {
            return Err(LightningLabsRfqError::ExpiredRequest {
                now_unix_seconds,
                expiry_unix_seconds: self.expiry_unix_seconds,
            });
        }
        Ok(())
    }

    pub fn validate_against_invoice(
        &self,
        envelope_peer: &str,
        invoice: &QuoteBoundInvoice,
        now_unix_seconds: u64,
    ) -> Result<(), LightningLabsRfqError> {
        self.validate_at(now_unix_seconds)?;
        if envelope_peer != invoice.peer {
            return Err(LightningLabsRfqError::PeerMismatch {
                expected: invoice.peer.clone(),
                actual: envelope_peer.to_owned(),
            });
        }
        if self.transfer_type != LightningLabsTransferType::PayInvoice {
            return Err(LightningLabsRfqError::UnexpectedTransferType {
                expected: LightningLabsTransferType::PayInvoice,
                actual: self.transfer_type,
            });
        }
        if self.in_asset_id != Some(Bytes32::ZERO) || self.out_asset_id != Some(invoice.asset_id) {
            return Err(LightningLabsRfqError::AssetIdMismatch {
                expected: invoice.asset_id,
                actual: self.out_asset_id.unwrap_or(Bytes32::ZERO),
            });
        }
        if self.max_in_asset != invoice.btc_msat {
            return Err(LightningLabsRfqError::BtcAmountMismatch {
                expected: invoice.btc_msat,
                actual: self.max_in_asset,
            });
        }
        if let Some(min_out_asset) = self.min_out_asset {
            if min_out_asset > invoice.btc_msat {
                return Err(LightningLabsRfqError::BtcAmountMismatch {
                    expected: invoice.btc_msat,
                    actual: min_out_asset,
                });
            }
        }
        if self.expiry_unix_seconds > invoice.quote_expiry_unix_seconds
            || invoice.invoice_expiry_unix_seconds > self.expiry_unix_seconds
        {
            return Err(LightningLabsRfqError::InvoiceExpiryOutlivesQuote {
                request_expiry_unix_seconds: self.expiry_unix_seconds,
                quote_expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
                invoice_expiry_unix_seconds: invoice.invoice_expiry_unix_seconds,
            });
        }
        if self.oracle_metadata != invoice.invoice_context.0 {
            return Err(LightningLabsRfqError::InvoiceContextMismatch);
        }
        let expected_rate = asset_units_per_btc_rate(invoice)?;
        if self.out_asset_rate_hint.as_ref() != Some(&expected_rate) {
            return Err(LightningLabsRfqError::AssetAmountMismatch {
                expected: invoice.asset_amount,
                actual: 0,
            });
        }
        Ok(())
    }

    pub fn validate_receive_against_invoice(
        &self,
        envelope_peer: &str,
        invoice: &QuoteBoundInvoice,
        now_unix_seconds: u64,
    ) -> Result<(), LightningLabsRfqError> {
        self.validate_at(now_unix_seconds)?;
        if envelope_peer != invoice.peer {
            return Err(LightningLabsRfqError::PeerMismatch {
                expected: invoice.peer.clone(),
                actual: envelope_peer.to_owned(),
            });
        }
        if self.transfer_type != LightningLabsTransferType::ReceivePayment {
            return Err(LightningLabsRfqError::UnexpectedTransferType {
                expected: LightningLabsTransferType::ReceivePayment,
                actual: self.transfer_type,
            });
        }
        if self.in_asset_id != Some(invoice.asset_id) || self.out_asset_id != Some(Bytes32::ZERO) {
            return Err(LightningLabsRfqError::AssetIdMismatch {
                expected: invoice.asset_id,
                actual: self.in_asset_id.unwrap_or(Bytes32::ZERO),
            });
        }
        if self.max_in_asset != invoice.asset_amount {
            return Err(LightningLabsRfqError::AssetAmountMismatch {
                expected: invoice.asset_amount,
                actual: self.max_in_asset,
            });
        }
        if let Some(min_in_asset) = self.min_in_asset {
            if min_in_asset > invoice.asset_amount {
                return Err(LightningLabsRfqError::AssetAmountMismatch {
                    expected: invoice.asset_amount,
                    actual: min_in_asset,
                });
            }
        }
        if self.expiry_unix_seconds > invoice.quote_expiry_unix_seconds
            || invoice.invoice_expiry_unix_seconds > self.expiry_unix_seconds
        {
            return Err(LightningLabsRfqError::InvoiceExpiryOutlivesQuote {
                request_expiry_unix_seconds: self.expiry_unix_seconds,
                quote_expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
                invoice_expiry_unix_seconds: invoice.invoice_expiry_unix_seconds,
            });
        }
        if self.oracle_metadata != invoice.invoice_context.0 {
            return Err(LightningLabsRfqError::InvoiceContextMismatch);
        }
        let expected_rate = asset_units_per_btc_rate(invoice)?;
        if self.in_asset_rate_hint.as_ref() != Some(&expected_rate) {
            return Err(LightningLabsRfqError::AssetAmountMismatch {
                expected: invoice.asset_amount,
                actual: 0,
            });
        }
        Ok(())
    }

    pub fn scid_alias(&self) -> u64 {
        lightning_labs_rfq_id_to_scid_alias(self.id)
    }

    fn validate_shape(&self) -> Result<(), LightningLabsRfqError> {
        if self.version != LIGHTNING_LABS_RFQ_WIRE_VERSION {
            return Err(LightningLabsRfqError::UnsupportedVersion(self.version));
        }
        if self.max_in_asset == 0 {
            return Err(LightningLabsRfqError::ZeroMaxInAsset);
        }
        validate_asset_specifier(
            "in_asset",
            self.in_asset_id,
            self.in_asset_group_key.as_deref(),
        )?;
        validate_asset_specifier(
            "out_asset",
            self.out_asset_id,
            self.out_asset_group_key.as_deref(),
        )?;
        let in_is_btc = self.in_asset_id == Some(Bytes32::ZERO);
        let out_is_btc = self.out_asset_id == Some(Bytes32::ZERO);
        if in_is_btc == out_is_btc {
            return Err(LightningLabsRfqError::ExpectedExactlyOneBtcSide);
        }
        if self.oracle_metadata.len() > LIGHTNING_LABS_MAX_ORACLE_METADATA_LENGTH {
            return Err(LightningLabsRfqError::OracleMetadataTooLarge {
                actual: self.oracle_metadata.len(),
                max: LIGHTNING_LABS_MAX_ORACLE_METADATA_LENGTH,
            });
        }
        if let Some(rate) = &self.in_asset_rate_hint {
            rate.validate("in_asset_rate_hint")?;
            if rate.is_zero() {
                return Err(LightningLabsRfqError::ZeroFixedPoint("in_asset_rate_hint"));
            }
        }
        if let Some(rate) = &self.out_asset_rate_hint {
            rate.validate("out_asset_rate_hint")?;
            if rate.is_zero() {
                return Err(LightningLabsRfqError::ZeroFixedPoint("out_asset_rate_hint"));
            }
        }
        if let Some(rate) = &self.asset_rate_limit {
            rate.validate("asset_rate_limit")?;
            if rate.is_zero() {
                return Err(LightningLabsRfqError::ZeroFixedPoint("asset_rate_limit"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsRfqAccept {
    pub version: u8,
    pub id: Bytes32,
    pub expiry_unix_seconds: u64,
    pub signature: Vec<u8>,
    pub in_asset_rate: LightningLabsFixedPoint,
    pub out_asset_rate: LightningLabsFixedPoint,
    pub max_in_asset: Option<u64>,
}

impl LightningLabsRfqAccept {
    pub fn encode(&self) -> Result<Vec<u8>, LightningLabsRfqError> {
        self.validate_shape()?;
        let mut records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ID, self.id.0),
            TlvRecord::new(TYPE_ACCEPT_EXPIRY, self.expiry_unix_seconds.to_be_bytes()),
            TlvRecord::new(TYPE_ACCEPT_SIGNATURE, self.signature.clone()),
            TlvRecord::new(TYPE_ACCEPT_IN_ASSET_RATE, self.in_asset_rate.encode()?),
            TlvRecord::new(TYPE_ACCEPT_OUT_ASSET_RATE, self.out_asset_rate.encode()?),
        ];
        if let Some(amount) = self.max_in_asset {
            records.push(TlvRecord::new(
                TYPE_ACCEPT_MAX_IN_ASSET,
                amount.to_be_bytes(),
            ));
        }
        records.sort_by_key(|record| record.type_id);
        encode_stream(&records).map_err(LightningLabsRfqError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LightningLabsRfqError> {
        let records = decode_stream(bytes).map_err(LightningLabsRfqError::Tlv)?;
        reject_unknown_required(&records, KNOWN_ACCEPT_TYPES)
            .map_err(LightningLabsRfqError::Tlv)?;
        let fields = record_map(records);
        let max_in_asset = match optional_u64(&fields, TYPE_ACCEPT_MAX_IN_ASSET, "max_in_asset")? {
            Some(0) | None => None,
            Some(amount) => Some(amount),
        };
        let accept = Self {
            version: parse_u8(required(&fields, TYPE_VERSION)?, "version")?,
            id: parse_bytes32(required(&fields, TYPE_ID)?)?,
            expiry_unix_seconds: parse_u64(
                required(&fields, TYPE_ACCEPT_EXPIRY)?,
                "expiry_unix_seconds",
            )?,
            signature: required(&fields, TYPE_ACCEPT_SIGNATURE)?.to_vec(),
            in_asset_rate: parse_fixed_point(
                required(&fields, TYPE_ACCEPT_IN_ASSET_RATE)?,
                "in_asset_rate",
            )?,
            out_asset_rate: parse_fixed_point(
                required(&fields, TYPE_ACCEPT_OUT_ASSET_RATE)?,
                "out_asset_rate",
            )?,
            max_in_asset,
        };
        accept.validate_shape()?;
        Ok(accept)
    }

    pub fn validate_for_request(
        &self,
        request: &LightningLabsRfqRequest,
        now_unix_seconds: u64,
    ) -> Result<(), LightningLabsRfqError> {
        self.validate_shape()?;
        if self.expiry_unix_seconds <= now_unix_seconds {
            return Err(LightningLabsRfqError::ExpiredAccept {
                now_unix_seconds,
                expiry_unix_seconds: self.expiry_unix_seconds,
            });
        }
        if self.id != request.id {
            return Err(LightningLabsRfqError::RfqIdMismatch {
                expected: request.id,
                actual: self.id,
            });
        }
        if self.expiry_unix_seconds > request.expiry_unix_seconds {
            return Err(LightningLabsRfqError::AcceptExpiryOutlivesRequest {
                request_expiry_unix_seconds: request.expiry_unix_seconds,
                accept_expiry_unix_seconds: self.expiry_unix_seconds,
            });
        }
        if let Some(max_in_asset) = self.max_in_asset {
            if max_in_asset > request.max_in_asset {
                return Err(LightningLabsRfqError::BtcAmountMismatch {
                    expected: request.max_in_asset,
                    actual: max_in_asset,
                });
            }
        }
        Ok(())
    }

    pub fn scid_alias(&self) -> u64 {
        lightning_labs_rfq_id_to_scid_alias(self.id)
    }

    fn validate_shape(&self) -> Result<(), LightningLabsRfqError> {
        if self.version != LIGHTNING_LABS_RFQ_WIRE_VERSION {
            return Err(LightningLabsRfqError::UnsupportedVersion(self.version));
        }
        if self.signature.len() != 64 {
            return Err(LightningLabsRfqError::InvalidFieldLength {
                field: "signature",
                expected: 64,
                actual: self.signature.len(),
            });
        }
        self.in_asset_rate.validate("in_asset_rate")?;
        self.out_asset_rate.validate("out_asset_rate")?;
        if self.in_asset_rate.is_zero() {
            return Err(LightningLabsRfqError::ZeroFixedPoint("in_asset_rate"));
        }
        if self.out_asset_rate.is_zero() {
            return Err(LightningLabsRfqError::ZeroFixedPoint("out_asset_rate"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsRfqReject {
    pub version: u8,
    pub id: Bytes32,
    pub code: LightningLabsRejectCode,
    pub message: String,
}

impl LightningLabsRfqReject {
    pub fn encode(&self) -> Result<Vec<u8>, LightningLabsRfqError> {
        self.validate_shape()?;
        let mut reject = Vec::with_capacity(1 + self.message.len());
        reject.push(self.code.wire_value());
        reject.extend_from_slice(self.message.as_bytes());
        let records = vec![
            TlvRecord::new(TYPE_VERSION, [self.version]),
            TlvRecord::new(TYPE_ID, self.id.0),
            TlvRecord::new(TYPE_REJECT_ERROR, reject),
        ];
        encode_stream(&records).map_err(LightningLabsRfqError::Tlv)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, LightningLabsRfqError> {
        let records = decode_stream(bytes).map_err(LightningLabsRfqError::Tlv)?;
        reject_unknown_required(&records, KNOWN_REJECT_TYPES)
            .map_err(LightningLabsRfqError::Tlv)?;
        let fields = record_map(records);
        let reject = required(&fields, TYPE_REJECT_ERROR)?;
        let Some((code, message)) = reject.split_first() else {
            return Err(LightningLabsRfqError::InvalidFieldLength {
                field: "reject_error",
                expected: 1,
                actual: 0,
            });
        };
        let message = std::str::from_utf8(message)
            .map_err(|_| LightningLabsRfqError::InvalidUtf8("reject_message"))?
            .to_owned();
        let reject = Self {
            version: parse_u8(required(&fields, TYPE_VERSION)?, "version")?,
            id: parse_bytes32(required(&fields, TYPE_ID)?)?,
            code: LightningLabsRejectCode::from_wire(*code)?,
            message,
        };
        reject.validate_shape()?;
        Ok(reject)
    }

    fn validate_shape(&self) -> Result<(), LightningLabsRfqError> {
        if self.version != LIGHTNING_LABS_RFQ_WIRE_VERSION {
            return Err(LightningLabsRfqError::UnsupportedVersion(self.version));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LightningLabsRfqInvoiceCompatReport {
    pub source_commit: String,
    pub request_message_type: u64,
    pub accept_message_type: u64,
    pub reject_message_type: u64,
    pub native_request_message_type: u64,
    pub native_accept_message_type: u64,
    pub native_reject_message_type: u64,
    pub message_types_match: bool,
    pub rfq_id: Bytes32,
    pub lightning_labs_scid_alias: u64,
    pub native_scid_alias: u64,
    pub quote_id: String,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub quote_expiry_unix_seconds: u64,
    pub invoice_expiry_unix_seconds: u64,
    pub request_expiry_unix_seconds: u64,
    pub accept_expiry_unix_seconds: u64,
    pub invoice_expiry_within_quote: bool,
    pub request_data_digest: Bytes32,
    pub accept_data_digest: Bytes32,
    pub reject_data_digest: Bytes32,
    pub request_data_len: usize,
    pub accept_data_len: usize,
    pub reject_data_len: usize,
    pub replay_rejected: bool,
    pub wrong_peer_rejected: bool,
    pub wrong_asset_rejected: bool,
    pub expired_request_rejected: bool,
    pub documented_gap: String,
}

pub fn lightning_labs_sell_request_from_invoice(
    invoice: &QuoteBoundInvoice,
    rfq_id: Bytes32,
) -> Result<LightningLabsRfqRequest, LightningLabsRfqError> {
    Ok(LightningLabsRfqRequest {
        version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
        id: rfq_id,
        transfer_type: LightningLabsTransferType::PayInvoice,
        expiry_unix_seconds: invoice
            .invoice_expiry_unix_seconds
            .min(invoice.quote_expiry_unix_seconds),
        in_asset_id: Some(Bytes32::ZERO),
        in_asset_group_key: None,
        out_asset_id: Some(invoice.asset_id),
        out_asset_group_key: None,
        max_in_asset: invoice.btc_msat,
        in_asset_rate_hint: None,
        out_asset_rate_hint: Some(asset_units_per_btc_rate(invoice)?),
        min_in_asset: None,
        min_out_asset: Some(invoice.btc_msat),
        oracle_metadata: invoice.invoice_context.0.to_vec(),
        asset_rate_limit: Some(asset_units_per_btc_rate(invoice)?),
        execution_policy: Some(LightningLabsExecutionPolicy::FillOrKill),
    })
}

pub fn lightning_labs_buy_request_from_invoice(
    invoice: &QuoteBoundInvoice,
    rfq_id: Bytes32,
) -> Result<LightningLabsRfqRequest, LightningLabsRfqError> {
    Ok(LightningLabsRfqRequest {
        version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
        id: rfq_id,
        transfer_type: LightningLabsTransferType::ReceivePayment,
        expiry_unix_seconds: invoice
            .invoice_expiry_unix_seconds
            .min(invoice.quote_expiry_unix_seconds),
        in_asset_id: Some(invoice.asset_id),
        in_asset_group_key: None,
        out_asset_id: Some(Bytes32::ZERO),
        out_asset_group_key: None,
        max_in_asset: invoice.asset_amount,
        in_asset_rate_hint: Some(asset_units_per_btc_rate(invoice)?),
        out_asset_rate_hint: None,
        min_in_asset: Some(invoice.asset_amount),
        min_out_asset: None,
        oracle_metadata: invoice.invoice_context.0.to_vec(),
        asset_rate_limit: Some(asset_units_per_btc_rate(invoice)?),
        execution_policy: Some(LightningLabsExecutionPolicy::FillOrKill),
    })
}

pub fn lightning_labs_accept_from_invoice(
    invoice: &QuoteBoundInvoice,
    rfq_id: Bytes32,
) -> Result<LightningLabsRfqAccept, LightningLabsRfqError> {
    Ok(LightningLabsRfqAccept {
        version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
        id: rfq_id,
        expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
        signature: vec![0; 64],
        in_asset_rate: LightningLabsFixedPoint::msat_per_btc(),
        out_asset_rate: asset_units_per_btc_rate(invoice)?,
        max_in_asset: Some(invoice.btc_msat),
    })
}

pub fn lightning_labs_buy_accept_from_invoice(
    invoice: &QuoteBoundInvoice,
    rfq_id: Bytes32,
) -> Result<LightningLabsRfqAccept, LightningLabsRfqError> {
    Ok(LightningLabsRfqAccept {
        version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
        id: rfq_id,
        expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
        signature: vec![0; 64],
        in_asset_rate: asset_units_per_btc_rate(invoice)?,
        out_asset_rate: LightningLabsFixedPoint::msat_per_btc(),
        max_in_asset: Some(invoice.asset_amount),
    })
}

pub fn run_lightning_labs_rfq_invoice_compat_smoke(
    asset_id: Bytes32,
) -> Result<LightningLabsRfqInvoiceCompatReport, LightningLabsRfqError> {
    let peer = "lightning-labs-counterparty";
    let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 41);
    let invoice_context = Bytes32([12; 32]);
    let payment_hash = Bytes32([13; 32]);
    let request = AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount: 250_000,
        invoice_context,
    };
    let mut store = RfqQuoteStore::default();
    let native_accept = receive_native_rfq_request(
        &mut store,
        peer,
        &request,
        1_000,
        NativeRfqPolicy::default(),
    )?;
    let invoice = bind_quote_to_invoice(
        &native_accept.quote,
        QuoteBoundInvoiceRequest {
            invoice: "lnbcrt1lightninglabsquotecompat".to_owned(),
            payment_hash,
            peer: peer.to_owned(),
            asset_id,
            asset_amount: native_accept.quote.asset_amount,
            btc_msat: native_accept.quote.btc_msat,
            invoice_context,
            invoice_expiry_unix_seconds: native_accept.quote.expiry_unix_seconds,
            now_unix_seconds: 1_001,
        },
    )?;

    let ll_request = lightning_labs_sell_request_from_invoice(&invoice, rfq_id)?;
    let request_data = ll_request.encode()?;
    let decoded_request = LightningLabsRfqRequest::decode(&request_data)?;
    decoded_request.validate_against_invoice(peer, &invoice, 1_002)?;

    let ll_accept = lightning_labs_accept_from_invoice(&invoice, rfq_id)?;
    let accept_data = ll_accept.encode()?;
    let decoded_accept = LightningLabsRfqAccept::decode(&accept_data)?;
    decoded_accept.validate_for_request(&decoded_request, 1_002)?;

    let ll_reject = LightningLabsRfqReject {
        version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
        id: rfq_id,
        code: LightningLabsRejectCode::PriceOracleUnavailable,
        message: "price oracle unavailable".to_owned(),
    };
    let reject_data = ll_reject.encode()?;
    let decoded_reject = LightningLabsRfqReject::decode(&reject_data)?;
    if decoded_reject != ll_reject {
        return Err(LightningLabsRfqError::RoundTripMismatch("reject"));
    }

    let payment = pay_quote_bound_invoice(&mut store, invoice.clone(), 1_003)?;
    let replay_rejected = pay_quote_bound_invoice(&mut store, invoice.clone(), 1_004).is_err();

    let wrong_peer_rejected = decoded_request
        .validate_against_invoice("wrong-peer", &invoice, 1_002)
        .is_err();
    let mut wrong_asset = decoded_request.clone();
    wrong_asset.out_asset_id = Some(Bytes32([99; 32]));
    let wrong_asset_rejected = wrong_asset
        .validate_against_invoice(peer, &invoice, 1_002)
        .is_err();
    let mut expired_request = decoded_request.clone();
    expired_request.expiry_unix_seconds = 999;
    let expired_request_rejected = expired_request.validate_at(1_000).is_err();

    Ok(LightningLabsRfqInvoiceCompatReport {
        source_commit: LIGHTNING_LABS_TAPROOT_ASSETS_COMMIT.to_owned(),
        request_message_type: LIGHTNING_LABS_RFQ_REQUEST_TYPE,
        accept_message_type: LIGHTNING_LABS_RFQ_ACCEPT_TYPE,
        reject_message_type: LIGHTNING_LABS_RFQ_REJECT_TYPE,
        native_request_message_type: RFQ_REQUEST_TYPE,
        native_accept_message_type: RFQ_ACCEPT_TYPE,
        native_reject_message_type: RFQ_REJECT_TYPE,
        message_types_match: RFQ_REQUEST_TYPE == LIGHTNING_LABS_RFQ_REQUEST_TYPE
            && RFQ_ACCEPT_TYPE == LIGHTNING_LABS_RFQ_ACCEPT_TYPE
            && RFQ_REJECT_TYPE == LIGHTNING_LABS_RFQ_REJECT_TYPE,
        rfq_id,
        lightning_labs_scid_alias: decoded_request.scid_alias(),
        native_scid_alias: payment.authorization.scid_alias,
        quote_id: payment.authorization.quote_id,
        peer: peer.to_owned(),
        asset_id,
        asset_amount: payment.authorization.asset_amount,
        btc_msat: payment.authorization.btc_msat,
        quote_expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
        invoice_expiry_unix_seconds: invoice.invoice_expiry_unix_seconds,
        request_expiry_unix_seconds: decoded_request.expiry_unix_seconds,
        accept_expiry_unix_seconds: decoded_accept.expiry_unix_seconds,
        invoice_expiry_within_quote: invoice.invoice_expiry_unix_seconds
            <= invoice.quote_expiry_unix_seconds
            && decoded_request.expiry_unix_seconds <= invoice.quote_expiry_unix_seconds
            && decoded_accept.expiry_unix_seconds <= invoice.quote_expiry_unix_seconds,
        request_data_digest: sha256_digest(&request_data),
        accept_data_digest: sha256_digest(&accept_data),
        reject_data_digest: sha256_digest(&reject_data),
        request_data_len: request_data.len(),
        accept_data_len: accept_data.len(),
        reject_data_len: reject_data.len(),
        replay_rejected,
        wrong_peer_rejected,
        wrong_asset_rejected,
        expired_request_rejected,
        documented_gap:
            "Lightning Labs accept signatures are preserved as 64-byte fields; verifier-side peer signature validation is still a live-interoperability task."
                .to_owned(),
    })
}

pub fn lightning_labs_rfq_id_to_scid_alias(id: Bytes32) -> u64 {
    u64::from_be_bytes(id.0[24..].try_into().expect("slice is 8 bytes"))
}

fn rfq_id_with_scid_alias(scid_alias: u64) -> Bytes32 {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&scid_alias.to_be_bytes());
    Bytes32(bytes)
}

fn asset_units_per_btc_rate(
    invoice: &QuoteBoundInvoice,
) -> Result<LightningLabsFixedPoint, LightningLabsRfqError> {
    let numerator = (invoice.asset_amount as u128)
        .checked_mul(MSAT_PER_BTC)
        .ok_or(LightningLabsRfqError::AmountOverflow)?;
    let units_per_btc = numerator / invoice.btc_msat as u128;
    if units_per_btc == 0 || numerator % invoice.btc_msat as u128 != 0 {
        return Err(LightningLabsRfqError::NonIntegralAssetRate);
    }
    let units_per_btc =
        u64::try_from(units_per_btc).map_err(|_| LightningLabsRfqError::AmountOverflow)?;
    Ok(LightningLabsFixedPoint::from_u64(units_per_btc, 0))
}

fn sha256_digest(bytes: &[u8]) -> Bytes32 {
    Bytes32(Sha256::digest(bytes).into())
}

fn validate_asset_specifier(
    field: &'static str,
    asset_id: Option<Bytes32>,
    group_key: Option<&[u8]>,
) -> Result<(), LightningLabsRfqError> {
    if asset_id.is_none() && group_key.is_none() {
        return Err(LightningLabsRfqError::MissingAssetSpecifier(field));
    }
    if asset_id.is_some() && group_key.is_some() {
        return Err(LightningLabsRfqError::AmbiguousAssetSpecifier(field));
    }
    if let Some(group_key) = group_key {
        if group_key.len() != 33 {
            return Err(LightningLabsRfqError::InvalidFieldLength {
                field,
                expected: 33,
                actual: group_key.len(),
            });
        }
        if !matches!(group_key[0], 0x02 | 0x03) {
            return Err(LightningLabsRfqError::InvalidGroupKeyPrefix(group_key[0]));
        }
    }
    Ok(())
}

fn record_map(records: Vec<TlvRecord>) -> BTreeMap<u64, Vec<u8>> {
    records
        .into_iter()
        .map(|record| (record.type_id, record.value))
        .collect()
}

fn optional_execution_policy(
    fields: &BTreeMap<u64, Vec<u8>>,
) -> Result<Option<LightningLabsExecutionPolicy>, LightningLabsRfqError> {
    fields
        .get(&TYPE_REQUEST_EXECUTION_POLICY)
        .map(|bytes| {
            parse_u8(bytes, "execution_policy").and_then(LightningLabsExecutionPolicy::from_wire)
        })
        .transpose()
}

fn optional_fixed_point(
    fields: &BTreeMap<u64, Vec<u8>>,
    field: u64,
) -> Result<Option<LightningLabsFixedPoint>, LightningLabsRfqError> {
    fields
        .get(&field)
        .map(|bytes| LightningLabsFixedPoint::decode(bytes))
        .transpose()
}

fn optional_group_key(
    fields: &BTreeMap<u64, Vec<u8>>,
    field: u64,
) -> Result<Option<Vec<u8>>, LightningLabsRfqError> {
    fields
        .get(&field)
        .map(|bytes| {
            if bytes.len() != 33 {
                return Err(LightningLabsRfqError::InvalidFieldLength {
                    field: "group_key",
                    expected: 33,
                    actual: bytes.len(),
                });
            }
            Ok(bytes.clone())
        })
        .transpose()
}

fn optional_bytes32(
    fields: &BTreeMap<u64, Vec<u8>>,
    field: u64,
) -> Result<Option<Bytes32>, LightningLabsRfqError> {
    fields
        .get(&field)
        .map(Vec::as_slice)
        .map(parse_bytes32)
        .transpose()
}

fn optional_u64(
    fields: &BTreeMap<u64, Vec<u8>>,
    field: u64,
    field_name: &'static str,
) -> Result<Option<u64>, LightningLabsRfqError> {
    fields
        .get(&field)
        .map(|bytes| parse_u64(bytes, field_name))
        .transpose()
}

fn required(fields: &BTreeMap<u64, Vec<u8>>, field: u64) -> Result<&[u8], LightningLabsRfqError> {
    fields
        .get(&field)
        .map(Vec::as_slice)
        .ok_or(LightningLabsRfqError::MissingField(field))
}

fn parse_fixed_point(
    bytes: &[u8],
    field: &'static str,
) -> Result<LightningLabsFixedPoint, LightningLabsRfqError> {
    let value = LightningLabsFixedPoint::decode(bytes)?;
    value.validate(field)?;
    Ok(value)
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, LightningLabsRfqError> {
    let actual = bytes.len();
    let bytes: [u8; 32] =
        bytes
            .try_into()
            .map_err(|_| LightningLabsRfqError::InvalidFieldLength {
                field: "bytes32",
                expected: 32,
                actual,
            })?;
    Ok(Bytes32(bytes))
}

fn parse_u8(bytes: &[u8], field: &'static str) -> Result<u8, LightningLabsRfqError> {
    if bytes.len() != 1 {
        return Err(LightningLabsRfqError::InvalidFieldLength {
            field,
            expected: 1,
            actual: bytes.len(),
        });
    }
    Ok(bytes[0])
}

fn parse_u64(bytes: &[u8], field: &'static str) -> Result<u64, LightningLabsRfqError> {
    let actual = bytes.len();
    let bytes: [u8; 8] =
        bytes
            .try_into()
            .map_err(|_| LightningLabsRfqError::InvalidFieldLength {
                field,
                expected: 8,
                actual,
            })?;
    Ok(u64::from_be_bytes(bytes))
}

#[derive(Debug)]
pub enum LightningLabsRfqError {
    Tlv(TlvError),
    Invoice(RfqInvoiceError),
    MissingField(u64),
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidFixedPointLength(usize),
    FixedPointTooWide {
        field: &'static str,
        actual: usize,
    },
    InvalidUtf8(&'static str),
    UnsupportedVersion(u8),
    UnknownTransferType(u8),
    UnknownExecutionPolicy(u8),
    UnknownRejectCode(u8),
    MissingAssetSpecifier(&'static str),
    AmbiguousAssetSpecifier(&'static str),
    InvalidGroupKeyPrefix(u8),
    ExpectedExactlyOneBtcSide,
    ZeroMaxInAsset,
    ZeroFixedPoint(&'static str),
    OracleMetadataTooLarge {
        actual: usize,
        max: usize,
    },
    ExpiredRequest {
        now_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    ExpiredAccept {
        now_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    UnexpectedTransferType {
        expected: LightningLabsTransferType,
        actual: LightningLabsTransferType,
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
    InvoiceExpiryOutlivesQuote {
        request_expiry_unix_seconds: u64,
        quote_expiry_unix_seconds: u64,
        invoice_expiry_unix_seconds: u64,
    },
    RfqIdMismatch {
        expected: Bytes32,
        actual: Bytes32,
    },
    AcceptExpiryOutlivesRequest {
        request_expiry_unix_seconds: u64,
        accept_expiry_unix_seconds: u64,
    },
    AmountOverflow,
    NonIntegralAssetRate,
    RoundTripMismatch(&'static str),
}

impl fmt::Display for LightningLabsRfqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tlv(err) => write!(f, "Lightning Labs RFQ TLV error: {err}"),
            Self::Invoice(err) => write!(f, "Lightning Labs RFQ invoice error: {err}"),
            Self::MissingField(field) => write!(f, "missing Lightning Labs RFQ field {field}"),
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "invalid Lightning Labs RFQ {field} length: expected {expected}, got {actual}"
            ),
            Self::InvalidFixedPointLength(actual) => {
                write!(
                    f,
                    "invalid Lightning Labs fixed-point length: expected at least 1, got {actual}"
                )
            }
            Self::FixedPointTooWide { field, actual } => write!(
                f,
                "Lightning Labs fixed-point {field} coefficient is too wide: {actual} bytes"
            ),
            Self::InvalidUtf8(field) => write!(f, "Lightning Labs RFQ {field} is not UTF-8"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Lightning Labs RFQ wire version {version}")
            }
            Self::UnknownTransferType(value) => {
                write!(f, "unknown Lightning Labs RFQ transfer type {value}")
            }
            Self::UnknownExecutionPolicy(value) => {
                write!(f, "unknown Lightning Labs RFQ execution policy {value}")
            }
            Self::UnknownRejectCode(value) => {
                write!(f, "unknown Lightning Labs RFQ reject code {value}")
            }
            Self::MissingAssetSpecifier(field) => {
                write!(f, "Lightning Labs RFQ {field} asset specifier is missing")
            }
            Self::AmbiguousAssetSpecifier(field) => {
                write!(f, "Lightning Labs RFQ {field} asset specifier is ambiguous")
            }
            Self::InvalidGroupKeyPrefix(prefix) => {
                write!(
                    f,
                    "invalid Lightning Labs RFQ group key prefix 0x{prefix:02x}"
                )
            }
            Self::ExpectedExactlyOneBtcSide => {
                write!(
                    f,
                    "Lightning Labs RFQ request must have exactly one BTC side"
                )
            }
            Self::ZeroMaxInAsset => write!(f, "Lightning Labs RFQ max_in_asset cannot be zero"),
            Self::ZeroFixedPoint(field) => {
                write!(f, "Lightning Labs RFQ fixed-point {field} cannot be zero")
            }
            Self::OracleMetadataTooLarge { actual, max } => write!(
                f,
                "Lightning Labs RFQ oracle metadata length {actual} exceeds max {max}"
            ),
            Self::ExpiredRequest {
                now_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "Lightning Labs RFQ request expired at {expiry_unix_seconds}; now {now_unix_seconds}"
            ),
            Self::ExpiredAccept {
                now_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "Lightning Labs RFQ accept expired at {expiry_unix_seconds}; now {now_unix_seconds}"
            ),
            Self::UnexpectedTransferType { expected, actual } => write!(
                f,
                "unexpected Lightning Labs RFQ transfer type: expected {expected:?}, got {actual:?}"
            ),
            Self::PeerMismatch { expected, actual } => {
                write!(
                    f,
                    "Lightning Labs RFQ peer mismatch: expected {expected}, got {actual}"
                )
            }
            Self::AssetIdMismatch { expected, actual } => write!(
                f,
                "Lightning Labs RFQ asset ID mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::AssetAmountMismatch { expected, actual } => write!(
                f,
                "Lightning Labs RFQ asset amount mismatch: expected {expected}, got {actual}"
            ),
            Self::BtcAmountMismatch { expected, actual } => write!(
                f,
                "Lightning Labs RFQ BTC amount mismatch: expected {expected}, got {actual}"
            ),
            Self::InvoiceContextMismatch => {
                write!(f, "Lightning Labs RFQ invoice context mismatch")
            }
            Self::InvoiceExpiryOutlivesQuote {
                request_expiry_unix_seconds,
                quote_expiry_unix_seconds,
                invoice_expiry_unix_seconds,
            } => write!(
                f,
                "Lightning Labs RFQ expiry mismatch: request {request_expiry_unix_seconds}, quote {quote_expiry_unix_seconds}, invoice {invoice_expiry_unix_seconds}"
            ),
            Self::RfqIdMismatch { expected, actual } => write!(
                f,
                "Lightning Labs RFQ id mismatch: expected {}, got {}",
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::AcceptExpiryOutlivesRequest {
                request_expiry_unix_seconds,
                accept_expiry_unix_seconds,
            } => write!(
                f,
                "Lightning Labs RFQ accept expiry {accept_expiry_unix_seconds} outlives request expiry {request_expiry_unix_seconds}"
            ),
            Self::AmountOverflow => write!(f, "Lightning Labs RFQ amount overflow"),
            Self::NonIntegralAssetRate => {
                write!(
                    f,
                    "Lightning Labs RFQ asset rate is not integral for bounded demo"
                )
            }
            Self::RoundTripMismatch(message) => {
                write!(f, "Lightning Labs RFQ {message} round-trip mismatch")
            }
        }
    }
}

impl Error for LightningLabsRfqError {}

impl From<RfqInvoiceError> for LightningLabsRfqError {
    fn from(err: RfqInvoiceError) -> Self {
        Self::Invoice(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_invoice() -> QuoteBoundInvoice {
        QuoteBoundInvoice {
            invoice: "lnbcrt1sample".to_owned(),
            payment_hash: Bytes32([9; 32]),
            quote_id: Bytes32([8; 32]).to_hex(),
            peer: "lightning-labs-counterparty".to_owned(),
            asset_id: Bytes32([7; 32]),
            asset_amount: 250_000,
            btc_msat: 25_000_000,
            invoice_context: Bytes32([6; 32]),
            scid_alias: RFQ_ALIAS_BASE | 9,
            quote_expiry_unix_seconds: 1_120,
            invoice_expiry_unix_seconds: 1_120,
        }
    }

    #[test]
    fn lightning_labs_rfq_id_derives_scid_alias_from_last_eight_bytes() {
        let id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 123);
        assert_eq!(
            lightning_labs_rfq_id_to_scid_alias(id),
            RFQ_ALIAS_BASE | 123
        );
    }

    #[test]
    fn request_accept_and_reject_round_trip_with_lightning_labs_types() {
        let invoice = sample_invoice();
        let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 42);

        let request = lightning_labs_sell_request_from_invoice(&invoice, rfq_id).expect("request");
        assert_eq!(request.scid_alias(), RFQ_ALIAS_BASE | 42);
        let request_bytes = request.encode().expect("request encodes");
        let decoded_request =
            LightningLabsRfqRequest::decode(&request_bytes).expect("request decodes");
        assert_eq!(decoded_request, request);
        decoded_request
            .validate_against_invoice(&invoice.peer, &invoice, 1_001)
            .expect("request validates against invoice");

        let accept = lightning_labs_accept_from_invoice(&invoice, rfq_id).expect("accept");
        let accept_bytes = accept.encode().expect("accept encodes");
        let decoded_accept = LightningLabsRfqAccept::decode(&accept_bytes).expect("accept decodes");
        assert_eq!(decoded_accept, accept);
        decoded_accept
            .validate_for_request(&decoded_request, 1_001)
            .expect("accept validates against request");

        let reject = LightningLabsRfqReject {
            version: LIGHTNING_LABS_RFQ_WIRE_VERSION,
            id: rfq_id,
            code: LightningLabsRejectCode::PriceOracleUnavailable,
            message: "price oracle unavailable".to_owned(),
        };
        let reject_bytes = reject.encode().expect("reject encodes");
        let decoded_reject = LightningLabsRfqReject::decode(&reject_bytes).expect("reject decodes");
        assert_eq!(decoded_reject, reject);
    }

    #[test]
    fn request_negative_cases_fail_before_payment() {
        let invoice = sample_invoice();
        let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 43);
        let request = lightning_labs_sell_request_from_invoice(&invoice, rfq_id).expect("request");

        let mut wrong_asset = request.clone();
        wrong_asset.out_asset_id = Some(Bytes32([2; 32]));
        assert!(matches!(
            wrong_asset.validate_against_invoice(&invoice.peer, &invoice, 1_001),
            Err(LightningLabsRfqError::AssetIdMismatch { .. })
        ));

        let mut wrong_btc = request.clone();
        wrong_btc.max_in_asset += 1;
        assert!(matches!(
            wrong_btc.validate_against_invoice(&invoice.peer, &invoice, 1_001),
            Err(LightningLabsRfqError::BtcAmountMismatch { .. })
        ));

        assert!(matches!(
            request.validate_against_invoice("other-peer", &invoice, 1_001),
            Err(LightningLabsRfqError::PeerMismatch { .. })
        ));

        let mut expired = request.clone();
        expired.expiry_unix_seconds = 1_000;
        assert!(matches!(
            expired.validate_at(1_000),
            Err(LightningLabsRfqError::ExpiredRequest { .. })
        ));
    }

    #[test]
    fn receive_payment_request_round_trips_with_buy_direction_fields() {
        let invoice = sample_invoice();
        let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 45);

        let request = lightning_labs_buy_request_from_invoice(&invoice, rfq_id).expect("request");
        assert_eq!(
            request.transfer_type,
            LightningLabsTransferType::ReceivePayment
        );
        assert_eq!(request.in_asset_id, Some(invoice.asset_id));
        assert_eq!(request.out_asset_id, Some(Bytes32::ZERO));
        assert_eq!(request.max_in_asset, invoice.asset_amount);
        let request_bytes = request.encode().expect("request encodes");
        let decoded_request =
            LightningLabsRfqRequest::decode(&request_bytes).expect("request decodes");
        decoded_request
            .validate_receive_against_invoice(&invoice.peer, &invoice, 1_001)
            .expect("request validates against invoice");

        let accept = lightning_labs_buy_accept_from_invoice(&invoice, rfq_id).expect("accept");
        let accept_bytes = accept.encode().expect("accept encodes");
        let decoded_accept = LightningLabsRfqAccept::decode(&accept_bytes).expect("accept decodes");
        decoded_accept
            .validate_for_request(&decoded_request, 1_001)
            .expect("accept validates against request");

        let mut wrong_amount = decoded_request.clone();
        wrong_amount.max_in_asset += 1;
        assert!(matches!(
            wrong_amount.validate_receive_against_invoice(&invoice.peer, &invoice, 1_001),
            Err(LightningLabsRfqError::AssetAmountMismatch { .. })
        ));
    }

    #[test]
    fn accept_zero_max_in_asset_is_normalized_to_none_on_decode() {
        let invoice = sample_invoice();
        let rfq_id = rfq_id_with_scid_alias(RFQ_ALIAS_BASE | 44);
        let mut accept = lightning_labs_accept_from_invoice(&invoice, rfq_id).expect("accept");
        accept.max_in_asset = Some(0);
        let encoded = accept.encode().expect("accept encodes");
        let decoded = LightningLabsRfqAccept::decode(&encoded).expect("accept decodes");
        assert_eq!(decoded.max_in_asset, None);
    }

    #[test]
    fn smoke_covers_lightning_labs_rfq_invoice_bridge() {
        let report = run_lightning_labs_rfq_invoice_compat_smoke(Bytes32([7; 32])).expect("smoke");
        assert_eq!(report.request_message_type, LIGHTNING_LABS_RFQ_REQUEST_TYPE);
        assert!(!report.message_types_match);
        assert_eq!(report.asset_amount, 250_000);
        assert_eq!(report.btc_msat, 25_000_000);
        assert!(report.invoice_expiry_within_quote);
        assert!(report.replay_rejected);
        assert!(report.wrong_peer_rejected);
        assert!(report.wrong_asset_rejected);
        assert!(report.expired_request_rejected);
    }
}
