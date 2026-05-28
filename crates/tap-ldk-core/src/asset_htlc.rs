use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use lightning::{
    ln::taproot_asset::{
        SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION, TaprootAssetChannelDescriptor,
        TaprootAssetHtlcMetadata, TaprootAssetHtlcMetadataError,
        TaprootAssetHtlcMetadataExpectation, prepare_asset_htlc_metadata,
        validate_asset_htlc_final_hop as validate_ldk_asset_htlc_final_hop,
    },
    types::features::{ChannelTypeFeatures, InitFeatures},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_commitment::{AssetCommitmentError, AssetCommitmentStore, build_commitment_update},
    asset_peer_message::AssetPeerMessage,
    rfq_invoice::{
        NativeRfqPolicy, QuoteBoundInvoiceRequest, RfqInvoiceError, bind_quote_to_invoice,
        pay_quote_bound_invoice, receive_native_rfq_request,
    },
    rfq_quote_store::RfqHtlcAuthorization,
    rfq_quote_store::RfqQuoteStore,
    tlv::{TlvError, TlvRecord, decode_stream, encode_stream},
};

pub const ASSET_HTLC_RECORD_BASE: u64 = 760_000;
pub const RECORD_ASSET_ID: u64 = ASSET_HTLC_RECORD_BASE + 1;
pub const RECORD_ASSET_AMOUNT: u64 = ASSET_HTLC_RECORD_BASE + 3;
pub const RECORD_QUOTE_ID: u64 = ASSET_HTLC_RECORD_BASE + 5;
pub const RECORD_INVOICE_CONTEXT: u64 = ASSET_HTLC_RECORD_BASE + 7;
pub const RECORD_BTC_MSAT: u64 = ASSET_HTLC_RECORD_BASE + 9;
pub const RECORD_SCID_ALIAS: u64 = ASSET_HTLC_RECORD_BASE + 11;
pub const RECORD_PAYMENT_HASH: u64 = ASSET_HTLC_RECORD_BASE + 13;
pub const RECORD_FINAL_HOP_DIGEST: u64 = ASSET_HTLC_RECORD_BASE + 15;
pub const RECORD_PROTOCOL_VERSION: u64 = ASSET_HTLC_RECORD_BASE + 17;
pub const RECORD_PROOF_ROOT_HASH: u64 = ASSET_HTLC_RECORD_BASE + 19;
pub const RECORD_PROOF_ROOT_SUM: u64 = ASSET_HTLC_RECORD_BASE + 21;

const ASSET_RECORD_TYPES: &[u64] = &[
    RECORD_ASSET_ID,
    RECORD_ASSET_AMOUNT,
    RECORD_QUOTE_ID,
    RECORD_INVOICE_CONTEXT,
    RECORD_BTC_MSAT,
    RECORD_SCID_ALIAS,
    RECORD_PAYMENT_HASH,
    RECORD_FINAL_HOP_DIGEST,
    RECORD_PROTOCOL_VERSION,
    RECORD_PROOF_ROOT_HASH,
    RECORD_PROOF_ROOT_SUM,
];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetHtlcCustomRecords {
    pub protocol_version: u16,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub proof_root_hash: Bytes32,
    pub proof_root_sum: u64,
    pub quote_id: Bytes32,
    pub invoice_context: Bytes32,
    pub btc_msat: u64,
    pub scid_alias: u64,
    pub payment_hash: Bytes32,
    pub final_hop_digest: Bytes32,
}

impl AssetHtlcCustomRecords {
    pub fn from_authorization(
        invoice: &crate::rfq_invoice::QuoteBoundInvoice,
        authorization: &RfqHtlcAuthorization,
    ) -> Result<Self, AssetHtlcError> {
        Self::from_authorization_with_proof_root(
            invoice,
            authorization,
            authorization.asset_id,
            authorization.asset_amount,
        )
    }

    pub fn from_authorization_with_proof_root(
        invoice: &crate::rfq_invoice::QuoteBoundInvoice,
        authorization: &RfqHtlcAuthorization,
        proof_root_hash: Bytes32,
        proof_root_sum: u64,
    ) -> Result<Self, AssetHtlcError> {
        let quote_id = Bytes32::from_str(&authorization.quote_id)
            .map_err(|_| AssetHtlcError::InvalidQuoteId)?;
        if invoice.quote_id != authorization.quote_id
            || invoice.asset_id != authorization.asset_id
            || invoice.asset_amount != authorization.asset_amount
            || invoice.btc_msat != authorization.btc_msat
            || invoice.invoice_context != authorization.invoice_context
            || invoice.scid_alias != authorization.scid_alias
        {
            return Err(AssetHtlcError::InvoiceAuthorizationMismatch);
        }

        let final_hop_digest = final_hop_digest(
            SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
            authorization.asset_id,
            authorization.asset_amount,
            proof_root_hash,
            proof_root_sum,
            quote_id,
            invoice.payment_hash,
        );
        Ok(Self {
            protocol_version: SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
            asset_id: authorization.asset_id,
            asset_amount: authorization.asset_amount,
            proof_root_hash,
            proof_root_sum,
            quote_id,
            invoice_context: authorization.invoice_context,
            btc_msat: authorization.btc_msat,
            scid_alias: authorization.scid_alias,
            payment_hash: invoice.payment_hash,
            final_hop_digest,
        })
    }

    pub fn to_custom_records(&self) -> BTreeMap<u64, Vec<u8>> {
        BTreeMap::from([
            (
                RECORD_PROTOCOL_VERSION,
                self.protocol_version.to_be_bytes().to_vec(),
            ),
            (RECORD_ASSET_ID, self.asset_id.0.to_vec()),
            (
                RECORD_ASSET_AMOUNT,
                self.asset_amount.to_be_bytes().to_vec(),
            ),
            (RECORD_PROOF_ROOT_HASH, self.proof_root_hash.0.to_vec()),
            (
                RECORD_PROOF_ROOT_SUM,
                self.proof_root_sum.to_be_bytes().to_vec(),
            ),
            (RECORD_QUOTE_ID, self.quote_id.0.to_vec()),
            (RECORD_INVOICE_CONTEXT, self.invoice_context.0.to_vec()),
            (RECORD_BTC_MSAT, self.btc_msat.to_be_bytes().to_vec()),
            (RECORD_SCID_ALIAS, self.scid_alias.to_be_bytes().to_vec()),
            (RECORD_PAYMENT_HASH, self.payment_hash.0.to_vec()),
            (RECORD_FINAL_HOP_DIGEST, self.final_hop_digest.0.to_vec()),
        ])
    }

    pub fn encode_tlv(&self) -> Result<Vec<u8>, AssetHtlcError> {
        encode_custom_records(&self.to_custom_records())
    }

    pub fn to_ldk_metadata(&self) -> Result<TaprootAssetHtlcMetadata, AssetHtlcError> {
        let mut metadata = TaprootAssetHtlcMetadata::new(
            self.asset_id.0,
            self.asset_amount,
            self.proof_root_hash.0,
            self.proof_root_sum,
            self.quote_id.0,
            self.payment_hash.0,
        )
        .map_err(AssetHtlcError::LdkHtlcMetadata)?;
        metadata.protocol_version = self.protocol_version;
        metadata.final_hop_digest = self.final_hop_digest.0;
        metadata
            .validate_integrity()
            .map_err(AssetHtlcError::LdkHtlcMetadata)?;
        Ok(metadata)
    }

    pub fn to_peer_message(&self, rfq_id: Bytes32) -> Result<AssetPeerMessage, AssetHtlcError> {
        Ok(AssetPeerMessage::AssetHtlcBlob {
            asset_id: self.asset_id,
            asset_amount: self.asset_amount,
            rfq_id,
            invoice_context: self.invoice_context,
            htlc_blob: self.encode_tlv()?,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetHtlcValidation {
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub quote_id: Bytes32,
    pub btc_msat: u64,
    pub payment_hash: Bytes32,
    pub settlement_allowed: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetHtlcStatus {
    Offered,
    Settled,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetHtlcStore {
    pub htlcs: BTreeMap<String, StoredAssetHtlc>,
}

impl Default for AssetHtlcStore {
    fn default() -> Self {
        Self {
            htlcs: BTreeMap::new(),
        }
    }
}

impl AssetHtlcStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AssetHtlcError> {
        let raw = fs::read_to_string(path.as_ref()).map_err(AssetHtlcError::Io)?;
        let store = serde_json::from_str::<Self>(&raw).map_err(AssetHtlcError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), AssetHtlcError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(AssetHtlcError::Io)?;
            }
        }

        let raw = serde_json::to_vec_pretty(self).map_err(AssetHtlcError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(AssetHtlcError::Io)?;
        fs::rename(&temp_path, path).map_err(AssetHtlcError::Io)?;
        Ok(())
    }

    pub fn add_htlc(
        &mut self,
        channel_id: &str,
        validation: AssetHtlcValidation,
    ) -> Result<StoredAssetHtlc, AssetHtlcError> {
        let htlc_id = htlc_id(channel_id, &validation);
        if self.htlcs.contains_key(&htlc_id) {
            return Err(AssetHtlcError::DuplicateHtlc(htlc_id));
        }
        let htlc = StoredAssetHtlc {
            htlc_id: htlc_id.clone(),
            channel_id: channel_id.to_owned(),
            asset_id: validation.asset_id,
            asset_amount: validation.asset_amount,
            quote_id: validation.quote_id,
            btc_msat: validation.btc_msat,
            payment_hash: validation.payment_hash,
            status: AssetHtlcStatus::Offered,
        };
        self.htlcs.insert(htlc_id, htlc.clone());
        Ok(htlc)
    }

    pub fn settle_htlc(&mut self, htlc_id: &str) -> Result<StoredAssetHtlc, AssetHtlcError> {
        self.transition(htlc_id, AssetHtlcStatus::Settled)
    }

    pub fn fail_htlc(&mut self, htlc_id: &str) -> Result<StoredAssetHtlc, AssetHtlcError> {
        self.transition(htlc_id, AssetHtlcStatus::Failed)
    }

    fn transition(
        &mut self,
        htlc_id: &str,
        status: AssetHtlcStatus,
    ) -> Result<StoredAssetHtlc, AssetHtlcError> {
        let htlc = self
            .htlcs
            .get_mut(htlc_id)
            .ok_or_else(|| AssetHtlcError::UnknownHtlc(htlc_id.to_owned()))?;
        if htlc.status != AssetHtlcStatus::Offered {
            return Err(AssetHtlcError::TerminalHtlc {
                htlc_id: htlc_id.to_owned(),
                status: htlc.status,
            });
        }
        htlc.status = status;
        Ok(htlc.clone())
    }

    pub fn validate(&self) -> Result<(), AssetHtlcError> {
        for (htlc_id, htlc) in &self.htlcs {
            if htlc_id != &htlc.htlc_id {
                return Err(AssetHtlcError::StorageInvariant(format!(
                    "HTLC map key {htlc_id} does not match htlc_id {}",
                    htlc.htlc_id
                )));
            }
            if htlc.channel_id.trim().is_empty() {
                return Err(AssetHtlcError::StorageInvariant(format!(
                    "HTLC {htlc_id} has empty channel ID"
                )));
            }
            if htlc.asset_amount == 0 || htlc.btc_msat == 0 {
                return Err(AssetHtlcError::StorageInvariant(format!(
                    "HTLC {htlc_id} has zero asset or BTC amount"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredAssetHtlc {
    pub htlc_id: String,
    pub channel_id: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub quote_id: Bytes32,
    pub btc_msat: u64,
    pub payment_hash: Bytes32,
    pub status: AssetHtlcStatus,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AssetHtlcDecode {
    BtcOnly,
    Asset(AssetHtlcCustomRecords),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetHtlcSmokeReport {
    pub settled_htlc_id: String,
    pub failed_htlc_id: String,
    pub latest_commitment_number: u64,
    pub local_balance: u64,
    pub remote_balance: u64,
    pub btc_msat: u64,
    pub wrong_metadata_rejected: bool,
    pub btc_only_unaffected: bool,
    pub ldk_metadata_attached_after_quote_acceptance: bool,
}

pub fn encode_custom_records(records: &BTreeMap<u64, Vec<u8>>) -> Result<Vec<u8>, AssetHtlcError> {
    let records = records
        .iter()
        .map(|(type_id, value)| TlvRecord::new(*type_id, value.clone()))
        .collect::<Vec<_>>();
    encode_stream(&records).map_err(AssetHtlcError::Tlv)
}

pub fn decode_custom_records(bytes: &[u8]) -> Result<AssetHtlcDecode, AssetHtlcError> {
    let records = decode_stream(bytes).map_err(AssetHtlcError::Tlv)?;
    let map = records
        .into_iter()
        .map(|record| (record.type_id, record.value))
        .collect::<BTreeMap<_, _>>();
    decode_custom_record_map(&map)
}

pub fn decode_custom_record_map(
    records: &BTreeMap<u64, Vec<u8>>,
) -> Result<AssetHtlcDecode, AssetHtlcError> {
    let has_asset_record = records
        .keys()
        .any(|type_id| ASSET_RECORD_TYPES.contains(type_id));
    if !has_asset_record {
        return Ok(AssetHtlcDecode::BtcOnly);
    }

    for type_id in records.keys() {
        if (ASSET_HTLC_RECORD_BASE..ASSET_HTLC_RECORD_BASE + 128).contains(type_id)
            && !ASSET_RECORD_TYPES.contains(type_id)
        {
            return Err(AssetHtlcError::UnknownAssetRecord(*type_id));
        }
    }

    Ok(AssetHtlcDecode::Asset(AssetHtlcCustomRecords {
        protocol_version: parse_u16(required(records, RECORD_PROTOCOL_VERSION)?)?,
        asset_id: parse_bytes32(required(records, RECORD_ASSET_ID)?)?,
        asset_amount: parse_u64(required(records, RECORD_ASSET_AMOUNT)?, "asset_amount")?,
        proof_root_hash: parse_bytes32(required(records, RECORD_PROOF_ROOT_HASH)?)?,
        proof_root_sum: parse_u64(required(records, RECORD_PROOF_ROOT_SUM)?, "proof_root_sum")?,
        quote_id: parse_bytes32(required(records, RECORD_QUOTE_ID)?)?,
        invoice_context: parse_bytes32(required(records, RECORD_INVOICE_CONTEXT)?)?,
        btc_msat: parse_u64(required(records, RECORD_BTC_MSAT)?, "btc_msat")?,
        scid_alias: parse_u64(required(records, RECORD_SCID_ALIAS)?, "scid_alias")?,
        payment_hash: parse_bytes32(required(records, RECORD_PAYMENT_HASH)?)?,
        final_hop_digest: parse_bytes32(required(records, RECORD_FINAL_HOP_DIGEST)?)?,
    }))
}

pub fn validate_final_hop(
    records: &AssetHtlcCustomRecords,
    invoice: &crate::rfq_invoice::QuoteBoundInvoice,
    authorization: &RfqHtlcAuthorization,
    now_unix_seconds: u64,
) -> Result<AssetHtlcValidation, AssetHtlcError> {
    if now_unix_seconds > invoice.invoice_expiry_unix_seconds
        || now_unix_seconds > invoice.quote_expiry_unix_seconds
    {
        return Err(AssetHtlcError::StaleHtlc);
    }
    let quote_id =
        Bytes32::from_str(&authorization.quote_id).map_err(|_| AssetHtlcError::InvalidQuoteId)?;
    if records.quote_id != quote_id || invoice.quote_id != authorization.quote_id {
        return Err(AssetHtlcError::QuoteMismatch);
    }
    if records.asset_id != authorization.asset_id || records.asset_id != invoice.asset_id {
        return Err(AssetHtlcError::AssetIdMismatch);
    }
    if records.asset_amount != authorization.asset_amount
        || records.asset_amount != invoice.asset_amount
    {
        return Err(AssetHtlcError::AssetAmountMismatch);
    }
    if records.btc_msat != authorization.btc_msat || records.btc_msat != invoice.btc_msat {
        return Err(AssetHtlcError::BtcAmountMismatch);
    }
    if records.invoice_context != authorization.invoice_context
        || records.invoice_context != invoice.invoice_context
    {
        return Err(AssetHtlcError::InvoiceContextMismatch);
    }
    if records.scid_alias != authorization.scid_alias || records.scid_alias != invoice.scid_alias {
        return Err(AssetHtlcError::ScidAliasMismatch);
    }
    if records.payment_hash != invoice.payment_hash {
        return Err(AssetHtlcError::PaymentHashMismatch);
    }
    let expected_digest = final_hop_digest(
        records.protocol_version,
        records.asset_id,
        records.asset_amount,
        records.proof_root_hash,
        records.proof_root_sum,
        records.quote_id,
        records.payment_hash,
    );
    if records.final_hop_digest != expected_digest {
        return Err(AssetHtlcError::FinalHopDigestMismatch);
    }
    let ldk_metadata = records.to_ldk_metadata()?;
    let ldk_expectation = TaprootAssetHtlcMetadataExpectation {
        asset_id: authorization.asset_id.0,
        asset_amount: authorization.asset_amount,
        proof_root_hash: records.proof_root_hash.0,
        proof_root_sum: records.proof_root_sum,
        quote_id: quote_id.0,
        payment_hash: invoice.payment_hash.0,
        quote_accepted: true,
        now_unix_seconds,
        quote_expiry_unix_seconds: invoice.quote_expiry_unix_seconds,
    };
    validate_ldk_asset_htlc_final_hop(Some(&ldk_metadata), &ldk_expectation)
        .map_err(AssetHtlcError::LdkHtlcMetadata)?;

    Ok(AssetHtlcValidation {
        asset_id: records.asset_id,
        asset_amount: records.asset_amount,
        quote_id: records.quote_id,
        btc_msat: records.btc_msat,
        payment_hash: records.payment_hash,
        settlement_allowed: true,
    })
}

pub fn validate_final_hop_against_proof_root(
    records: &AssetHtlcCustomRecords,
    invoice: &crate::rfq_invoice::QuoteBoundInvoice,
    authorization: &RfqHtlcAuthorization,
    expected_proof_root_hash: Bytes32,
    expected_proof_root_sum: u64,
    now_unix_seconds: u64,
) -> Result<AssetHtlcValidation, AssetHtlcError> {
    let validation = validate_final_hop(records, invoice, authorization, now_unix_seconds)?;
    if records.proof_root_hash != expected_proof_root_hash
        || records.proof_root_sum != expected_proof_root_sum
    {
        return Err(AssetHtlcError::ProofRootMismatch {
            expected_hash: expected_proof_root_hash,
            actual_hash: records.proof_root_hash,
            expected_sum: expected_proof_root_sum,
            actual_sum: records.proof_root_sum,
        });
    }
    Ok(validation)
}

pub fn run_asset_htlc_smoke()
-> Result<(AssetHtlcStore, AssetCommitmentStore, AssetHtlcSmokeReport), AssetHtlcError> {
    let (mut commitment_store, mut commitment_state) = initialized_commitment_store()?;
    let channel_id = commitment_state.channel_id.clone();
    let proof_root_hash = commitment_state.monitor_blob.proof_root_hash;
    let proof_root_sum = commitment_state.total_amount;
    let (settle_records, settle_invoice, settle_authorization) = quote_bound_records(
        commitment_state.asset_id,
        125,
        Bytes32([21; 32]),
        Bytes32([22; 32]),
        proof_root_hash,
        proof_root_sum,
    )?;
    let encoded = settle_records.encode_tlv()?;
    let decoded = match decode_custom_records(&encoded)? {
        AssetHtlcDecode::Asset(records) => records,
        AssetHtlcDecode::BtcOnly => return Err(AssetHtlcError::MissingAssetRecords),
    };
    let metadata_attached_after_quote_acceptance = prepare_ldk_asset_htlc_metadata(&decoded, true)
        .is_ok()
        && matches!(
            prepare_ldk_asset_htlc_metadata(&decoded, false),
            Err(AssetHtlcError::LdkHtlcMetadata(
                TaprootAssetHtlcMetadataError::MissingAcceptedQuote
            ))
        );
    let validation = validate_final_hop_against_proof_root(
        &decoded,
        &settle_invoice,
        &settle_authorization,
        proof_root_hash,
        proof_root_sum,
        1_002,
    )?;

    let mut wrong = decoded.clone();
    wrong.asset_amount += 1;
    let wrong_metadata_rejected =
        validate_final_hop(&wrong, &settle_invoice, &settle_authorization, 1_002).is_err();

    let update = build_commitment_update(
        &commitment_state,
        validation.asset_amount,
        0,
        Bytes32([23; 32]),
    )
    .map_err(AssetHtlcError::Commitment)?;
    let snapshot = commitment_store
        .apply_update(update)
        .map_err(AssetHtlcError::Commitment)?;
    commitment_state = commitment_store
        .channel_state(&channel_id)
        .map_err(AssetHtlcError::Commitment)?;

    let mut htlc_store = AssetHtlcStore::default();
    let offered = htlc_store.add_htlc(&channel_id, validation)?;
    let settled = htlc_store.settle_htlc(&offered.htlc_id)?;

    let (fail_records, fail_invoice, fail_authorization) = quote_bound_records(
        commitment_state.asset_id,
        10,
        Bytes32([24; 32]),
        Bytes32([25; 32]),
        proof_root_hash,
        proof_root_sum,
    )?;
    let fail_validation = validate_final_hop_against_proof_root(
        &fail_records,
        &fail_invoice,
        &fail_authorization,
        proof_root_hash,
        proof_root_sum,
        1_002,
    )?;
    let failed_offer = htlc_store.add_htlc(&channel_id, fail_validation)?;
    let failed = htlc_store.fail_htlc(&failed_offer.htlc_id)?;
    let btc_only_unaffected = matches!(
        decode_custom_record_map(&BTreeMap::from([(42, vec![1, 2, 3])]))?,
        AssetHtlcDecode::BtcOnly
    );

    Ok((
        htlc_store,
        commitment_store,
        AssetHtlcSmokeReport {
            settled_htlc_id: settled.htlc_id,
            failed_htlc_id: failed.htlc_id,
            latest_commitment_number: snapshot.commitment_number,
            local_balance: snapshot.local_balance,
            remote_balance: snapshot.remote_balance,
            btc_msat: settle_authorization.btc_msat,
            wrong_metadata_rejected,
            btc_only_unaffected,
            ldk_metadata_attached_after_quote_acceptance: metadata_attached_after_quote_acceptance,
        },
    ))
}

#[derive(Debug)]
pub enum AssetHtlcError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Tlv(TlvError),
    Rfq(RfqInvoiceError),
    Commitment(AssetCommitmentError),
    LdkHtlcMetadata(TaprootAssetHtlcMetadataError),
    InvalidQuoteId,
    InvoiceAuthorizationMismatch,
    MissingAssetRecords,
    MissingRecord(u64),
    UnknownAssetRecord(u64),
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    DuplicateHtlc(String),
    UnknownHtlc(String),
    TerminalHtlc {
        htlc_id: String,
        status: AssetHtlcStatus,
    },
    StaleHtlc,
    QuoteMismatch,
    AssetIdMismatch,
    AssetAmountMismatch,
    BtcAmountMismatch,
    InvoiceContextMismatch,
    ScidAliasMismatch,
    PaymentHashMismatch,
    FinalHopDigestMismatch,
    ProofRootMismatch {
        expected_hash: Bytes32,
        actual_hash: Bytes32,
        expected_sum: u64,
        actual_sum: u64,
    },
    StorageInvariant(String),
}

impl fmt::Display for AssetHtlcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "asset HTLC I/O error: {err}"),
            Self::Json(err) => write!(f, "asset HTLC JSON error: {err}"),
            Self::Tlv(err) => write!(f, "asset HTLC TLV error: {err}"),
            Self::Rfq(err) => write!(f, "asset HTLC RFQ error: {err}"),
            Self::Commitment(err) => write!(f, "asset HTLC commitment error: {err}"),
            Self::LdkHtlcMetadata(err) => {
                write!(f, "LDK asset HTLC metadata rejected payment: {err:?}")
            }
            Self::InvalidQuoteId => write!(f, "asset HTLC quote ID is not 32 bytes"),
            Self::InvoiceAuthorizationMismatch => {
                write!(f, "asset HTLC invoice and authorization mismatch")
            }
            Self::MissingAssetRecords => write!(f, "asset HTLC records are missing"),
            Self::MissingRecord(type_id) => write!(f, "missing asset HTLC record {type_id}"),
            Self::UnknownAssetRecord(type_id) => {
                write!(f, "unknown asset HTLC record {type_id}")
            }
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "invalid asset HTLC {field} length: expected {expected}, got {actual}"
            ),
            Self::DuplicateHtlc(htlc_id) => write!(f, "duplicate asset HTLC {htlc_id}"),
            Self::UnknownHtlc(htlc_id) => write!(f, "unknown asset HTLC {htlc_id}"),
            Self::TerminalHtlc { htlc_id, status } => {
                write!(f, "asset HTLC {htlc_id} is terminal: {status:?}")
            }
            Self::StaleHtlc => write!(f, "asset HTLC is stale"),
            Self::QuoteMismatch => write!(f, "asset HTLC quote binding mismatch"),
            Self::AssetIdMismatch => write!(f, "asset HTLC asset ID mismatch"),
            Self::AssetAmountMismatch => write!(f, "asset HTLC amount mismatch"),
            Self::BtcAmountMismatch => write!(f, "asset HTLC BTC msat mismatch"),
            Self::InvoiceContextMismatch => write!(f, "asset HTLC invoice context mismatch"),
            Self::ScidAliasMismatch => write!(f, "asset HTLC SCID alias mismatch"),
            Self::PaymentHashMismatch => write!(f, "asset HTLC payment hash mismatch"),
            Self::FinalHopDigestMismatch => write!(f, "asset HTLC final-hop digest mismatch"),
            Self::ProofRootMismatch {
                expected_hash,
                actual_hash,
                expected_sum,
                actual_sum,
            } => write!(
                f,
                "asset HTLC proof root mismatch: expected {}:{expected_sum}, got {}:{actual_sum}",
                expected_hash.to_hex(),
                actual_hash.to_hex()
            ),
            Self::StorageInvariant(message) => {
                write!(f, "asset HTLC storage invariant failed: {message}")
            }
        }
    }
}

impl Error for AssetHtlcError {}

impl From<TlvError> for AssetHtlcError {
    fn from(err: TlvError) -> Self {
        Self::Tlv(err)
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

fn required(records: &BTreeMap<u64, Vec<u8>>, type_id: u64) -> Result<&[u8], AssetHtlcError> {
    records
        .get(&type_id)
        .map(Vec::as_slice)
        .ok_or(AssetHtlcError::MissingRecord(type_id))
}

fn parse_bytes32(bytes: &[u8]) -> Result<Bytes32, AssetHtlcError> {
    let actual = bytes.len();
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AssetHtlcError::InvalidFieldLength {
            field: "bytes32",
            expected: 32,
            actual,
        })?;
    Ok(Bytes32(bytes))
}

fn parse_u64(bytes: &[u8], field: &'static str) -> Result<u64, AssetHtlcError> {
    let actual = bytes.len();
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| AssetHtlcError::InvalidFieldLength {
            field,
            expected: 8,
            actual,
        })?;
    Ok(u64::from_be_bytes(bytes))
}

fn parse_u16(bytes: &[u8]) -> Result<u16, AssetHtlcError> {
    let actual = bytes.len();
    let bytes: [u8; 2] = bytes
        .try_into()
        .map_err(|_| AssetHtlcError::InvalidFieldLength {
            field: "protocol_version",
            expected: 2,
            actual,
        })?;
    Ok(u16::from_be_bytes(bytes))
}

pub fn prepare_ldk_asset_htlc_metadata(
    records: &AssetHtlcCustomRecords,
    quote_accepted: bool,
) -> Result<TaprootAssetHtlcMetadata, AssetHtlcError> {
    let mut features = InitFeatures::empty();
    features.set_static_remote_key_optional();
    features.set_channel_type_optional();
    features.set_simple_taproot_staging_optional();
    features.set_taproot_asset_channel_optional();
    let descriptor = TaprootAssetChannelDescriptor::new(
        records.asset_id.0,
        SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION,
    )
    .map_err(|_| AssetHtlcError::LdkHtlcMetadata(TaprootAssetHtlcMetadataError::AssetIdMismatch))?;
    prepare_asset_htlc_metadata(
        &features,
        &features,
        &ChannelTypeFeatures::taproot_asset_single_asset(),
        descriptor,
        quote_accepted,
        records.to_ldk_metadata()?,
    )
    .map_err(AssetHtlcError::LdkHtlcMetadata)
}

fn final_hop_digest(
    protocol_version: u16,
    asset_id: Bytes32,
    asset_amount: u64,
    proof_root_hash: Bytes32,
    proof_root_sum: u64,
    quote_id: Bytes32,
    payment_hash: Bytes32,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"openagents:taproot-asset-htlc-final-hop:v1");
    hasher.update(protocol_version.to_be_bytes());
    hasher.update(asset_id.0);
    hasher.update(asset_amount.to_be_bytes());
    hasher.update(proof_root_hash.0);
    hasher.update(proof_root_sum.to_be_bytes());
    hasher.update(quote_id.0);
    hasher.update(payment_hash.0);
    Bytes32(hasher.finalize().into())
}

fn htlc_id(channel_id: &str, validation: &AssetHtlcValidation) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:asset-htlc-id:v1");
    hasher.update((channel_id.len() as u64).to_be_bytes());
    hasher.update(channel_id.as_bytes());
    hasher.update(validation.asset_id.0);
    hasher.update(validation.asset_amount.to_be_bytes());
    hasher.update(validation.quote_id.0);
    hasher.update(validation.btc_msat.to_be_bytes());
    hasher.update(validation.payment_hash.0);
    Bytes32(hasher.finalize().into()).to_hex()
}

fn initialized_commitment_store() -> Result<
    (
        AssetCommitmentStore,
        crate::asset_commitment::AssetCommitmentChannelState,
    ),
    AssetHtlcError,
> {
    let (funding_store, funding_report) =
        crate::asset_channel_funding::run_asset_channel_funding_smoke()
            .map_err(|err| AssetHtlcError::Commitment(AssetCommitmentError::Funding(err)))?;
    let funded = funding_store
        .channels
        .get(&funding_report.channel_id)
        .ok_or_else(|| {
            AssetHtlcError::Commitment(AssetCommitmentError::UnknownChannel(
                funding_report.channel_id.clone(),
            ))
        })?;
    let mut store = AssetCommitmentStore::default();
    let state = store
        .initialize_channel(funded)
        .map_err(AssetHtlcError::Commitment)?;
    Ok((store, state))
}

fn quote_bound_records(
    asset_id: Bytes32,
    asset_amount: u64,
    rfq_id: Bytes32,
    payment_hash: Bytes32,
    proof_root_hash: Bytes32,
    proof_root_sum: u64,
) -> Result<
    (
        AssetHtlcCustomRecords,
        crate::rfq_invoice::QuoteBoundInvoice,
        RfqHtlcAuthorization,
    ),
    AssetHtlcError,
> {
    let invoice_context = Bytes32([31; 32]);
    let request = AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount,
        invoice_context,
    };
    let mut quote_store = RfqQuoteStore::default();
    let accept = receive_native_rfq_request(
        &mut quote_store,
        "alice",
        &request,
        1_000,
        NativeRfqPolicy::default(),
    )
    .map_err(AssetHtlcError::Rfq)?;
    let invoice = bind_quote_to_invoice(
        &accept.quote,
        QuoteBoundInvoiceRequest {
            invoice: "lnbcrt1assethtlc".to_owned(),
            payment_hash,
            peer: "alice".to_owned(),
            asset_id,
            asset_amount,
            btc_msat: accept.quote.btc_msat,
            invoice_context,
            invoice_expiry_unix_seconds: accept.quote.expiry_unix_seconds,
            now_unix_seconds: 1_001,
        },
    )
    .map_err(AssetHtlcError::Rfq)?;
    let payment =
        pay_quote_bound_invoice(&mut quote_store, invoice, 1_002).map_err(AssetHtlcError::Rfq)?;
    let records = AssetHtlcCustomRecords::from_authorization_with_proof_root(
        &payment.invoice,
        &payment.authorization,
        proof_root_hash,
        proof_root_sum,
    )?;
    Ok((records, payment.invoice, payment.authorization))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> (
        AssetHtlcCustomRecords,
        crate::rfq_invoice::QuoteBoundInvoice,
        RfqHtlcAuthorization,
    ) {
        quote_bound_records(
            Bytes32([7; 32]),
            25,
            Bytes32([8; 32]),
            Bytes32([9; 32]),
            Bytes32([10; 32]),
            1_000,
        )
        .expect("records build")
    }

    #[test]
    fn custom_records_round_trip_and_peer_blob_round_trips() {
        let (records, _invoice, _authorization) = records();
        let encoded = records.encode_tlv().expect("records encode");
        assert_eq!(
            decode_custom_records(&encoded).expect("records decode"),
            AssetHtlcDecode::Asset(records.clone())
        );

        let peer_message = records
            .to_peer_message(Bytes32([8; 32]))
            .expect("peer message builds");
        let encoded_peer = peer_message.encode().expect("peer encodes");
        let decoded_peer = AssetPeerMessage::decode(&encoded_peer).expect("peer decodes");
        assert_eq!(decoded_peer, peer_message);
    }

    #[test]
    fn btc_only_records_are_unaffected() {
        assert_eq!(
            decode_custom_record_map(&BTreeMap::from([(42, vec![1, 2, 3])]))
                .expect("btc records pass"),
            AssetHtlcDecode::BtcOnly
        );
    }

    #[test]
    fn missing_malformed_and_unknown_asset_records_fail_closed() {
        let (records, _invoice, _authorization) = records();
        let mut missing = records.to_custom_records();
        missing.remove(&RECORD_ASSET_ID);
        assert!(matches!(
            decode_custom_record_map(&missing),
            Err(AssetHtlcError::MissingRecord(RECORD_ASSET_ID))
        ));

        let mut malformed = records.to_custom_records();
        malformed.insert(RECORD_ASSET_AMOUNT, vec![1, 2]);
        assert!(matches!(
            decode_custom_record_map(&malformed),
            Err(AssetHtlcError::InvalidFieldLength {
                field: "asset_amount",
                ..
            })
        ));

        let mut unknown = records.to_custom_records();
        unknown.insert(ASSET_HTLC_RECORD_BASE + 23, vec![1]);
        assert!(matches!(
            decode_custom_record_map(&unknown),
            Err(AssetHtlcError::UnknownAssetRecord(_))
        ));
    }

    #[test]
    fn final_hop_validation_rejects_wrong_and_stale_metadata() {
        let (records, invoice, authorization) = records();
        assert!(validate_final_hop(&records, &invoice, &authorization, 1_002).is_ok());
        assert!(
            validate_final_hop_against_proof_root(
                &records,
                &invoice,
                &authorization,
                records.proof_root_hash,
                records.proof_root_sum,
                1_002,
            )
            .is_ok()
        );

        let mut wrong_asset = records.clone();
        wrong_asset.asset_id = Bytes32([99; 32]);
        assert!(matches!(
            validate_final_hop(&wrong_asset, &invoice, &authorization, 1_002),
            Err(AssetHtlcError::AssetIdMismatch)
        ));

        let mut wrong_amount = records.clone();
        wrong_amount.asset_amount += 1;
        assert!(matches!(
            validate_final_hop(&wrong_amount, &invoice, &authorization, 1_002),
            Err(AssetHtlcError::AssetAmountMismatch)
        ));

        let mut wrong_digest = records.clone();
        wrong_digest.final_hop_digest = Bytes32([42; 32]);
        assert!(matches!(
            validate_final_hop(&wrong_digest, &invoice, &authorization, 1_002),
            Err(AssetHtlcError::FinalHopDigestMismatch)
        ));

        assert!(matches!(
            validate_final_hop(
                &records,
                &invoice,
                &authorization,
                invoice.quote_expiry_unix_seconds + 1
            ),
            Err(AssetHtlcError::StaleHtlc)
        ));

        let wrong_root_records = AssetHtlcCustomRecords::from_authorization_with_proof_root(
            &invoice,
            &authorization,
            Bytes32([88; 32]),
            records.proof_root_sum + 1,
        )
        .expect("wrong-root records still build");
        assert!(validate_final_hop(&wrong_root_records, &invoice, &authorization, 1_002).is_ok());
        assert!(matches!(
            validate_final_hop_against_proof_root(
                &wrong_root_records,
                &invoice,
                &authorization,
                records.proof_root_hash,
                records.proof_root_sum,
                1_002,
            ),
            Err(AssetHtlcError::ProofRootMismatch { .. })
        ));
    }

    #[test]
    fn ldk_metadata_attaches_only_after_quote_acceptance() {
        let (records, _invoice, _authorization) = records();
        let metadata = prepare_ldk_asset_htlc_metadata(&records, true).expect("metadata attaches");
        assert_eq!(metadata.asset_id, records.asset_id.0);
        assert_eq!(metadata.asset_amount, records.asset_amount);
        assert_eq!(metadata.proof_root_hash, records.proof_root_hash.0);
        assert!(matches!(
            prepare_ldk_asset_htlc_metadata(&records, false),
            Err(AssetHtlcError::LdkHtlcMetadata(
                TaprootAssetHtlcMetadataError::MissingAcceptedQuote
            ))
        ));
    }

    #[test]
    fn wrong_metadata_fails_before_commitment_state_advances() {
        let (mut commitment_store, state) =
            initialized_commitment_store().expect("commitment state");
        let (mut records, invoice, authorization) = quote_bound_records(
            state.asset_id,
            125,
            Bytes32([10; 32]),
            Bytes32([11; 32]),
            state.monitor_blob.proof_root_hash,
            state.total_amount,
        )
        .expect("records build");
        records.asset_amount += 1;

        assert!(
            validate_final_hop_against_proof_root(
                &records,
                &invoice,
                &authorization,
                state.monitor_blob.proof_root_hash,
                state.total_amount,
                1_002,
            )
            .is_err()
        );
        let unchanged = commitment_store
            .channel_state(&state.channel_id)
            .expect("state remains");
        assert_eq!(unchanged.latest_commitment_number, 0);
        assert_eq!(unchanged.local_balance, 700);
        assert_eq!(unchanged.remote_balance, 300);

        let valid_records = AssetHtlcCustomRecords::from_authorization_with_proof_root(
            &invoice,
            &authorization,
            state.monitor_blob.proof_root_hash,
            state.total_amount,
        )
        .expect("valid records");
        let validation = validate_final_hop_against_proof_root(
            &valid_records,
            &invoice,
            &authorization,
            state.monitor_blob.proof_root_hash,
            state.total_amount,
            1_002,
        )
        .expect("valid");
        let update = build_commitment_update(&state, validation.asset_amount, 0, Bytes32([12; 32]))
            .expect("update builds");
        commitment_store
            .apply_update(update)
            .expect("state advances after validation");
    }

    #[test]
    fn htlc_store_add_settle_fail_and_terminal_states() {
        let (records, invoice, authorization) = records();
        let validation =
            validate_final_hop(&records, &invoice, &authorization, 1_002).expect("valid");
        let mut store = AssetHtlcStore::default();
        let offered = store.add_htlc("channel", validation).expect("offered");
        assert_eq!(offered.status, AssetHtlcStatus::Offered);
        let settled = store.settle_htlc(&offered.htlc_id).expect("settled");
        assert_eq!(settled.status, AssetHtlcStatus::Settled);
        assert!(matches!(
            store.fail_htlc(&offered.htlc_id),
            Err(AssetHtlcError::TerminalHtlc { .. })
        ));
    }

    #[test]
    fn bounded_malformed_record_property() {
        let (records, _invoice, _authorization) = records();
        for record_type in ASSET_RECORD_TYPES {
            let mut map = records.to_custom_records();
            map.insert(*record_type, vec![0]);
            assert!(decode_custom_record_map(&map).is_err());
        }
    }

    #[test]
    fn smoke_covers_add_settle_fail_and_balance_update() {
        let (_htlc_store, _commitment_store, report) =
            run_asset_htlc_smoke().expect("smoke passes");
        assert_eq!(report.latest_commitment_number, 1);
        assert_eq!(report.local_balance, 575);
        assert_eq!(report.remote_balance, 425);
        assert_eq!(report.btc_msat, 12_500);
        assert!(report.wrong_metadata_rejected);
        assert!(report.btc_only_unaffected);
        assert!(report.ldk_metadata_attached_after_quote_acceptance);
    }
}
