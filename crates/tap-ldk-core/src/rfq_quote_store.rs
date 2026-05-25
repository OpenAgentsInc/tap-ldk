use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::asset::Bytes32;

pub const RFQ_STORE_SCHEMA_VERSION: u32 = 1;
pub const REGTEST_OPENUSD_MSATS_PER_UNIT: u64 = 100;
pub const RFQ_ALIAS_BASE: u64 = 1 << 62;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RfqQuoteStore {
    pub version: u32,
    pub metadata: RfqStoreMetadata,
    pub real_local_scids: BTreeSet<u64>,
    pub live_aliases: BTreeMap<u64, String>,
    pub used_replay_domains: BTreeMap<String, String>,
    pub quotes: BTreeMap<String, StoredRfqQuote>,
}

impl Default for RfqQuoteStore {
    fn default() -> Self {
        Self {
            version: RFQ_STORE_SCHEMA_VERSION,
            metadata: RfqStoreMetadata::default(),
            real_local_scids: BTreeSet::new(),
            live_aliases: BTreeMap::new(),
            used_replay_domains: BTreeMap::new(),
            quotes: BTreeMap::new(),
        }
    }
}

impl RfqQuoteStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RfqQuoteError> {
        let raw = fs::read_to_string(path.as_ref()).map_err(RfqQuoteError::Io)?;
        let store = serde_json::from_str::<Self>(&raw).map_err(RfqQuoteError::Json)?;
        store.validate()?;
        Ok(store)
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, RfqQuoteError> {
        match Self::load(path.as_ref()) {
            Ok(store) => Ok(store),
            Err(RfqQuoteError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default())
            }
            Err(err) => Err(err),
        }
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), RfqQuoteError> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(RfqQuoteError::Io)?;
            }
        }

        let raw = serde_json::to_vec_pretty(self).map_err(RfqQuoteError::Json)?;
        let temp_path = temp_path_for(path);
        fs::write(&temp_path, raw).map_err(RfqQuoteError::Io)?;
        fs::rename(&temp_path, path).map_err(RfqQuoteError::Io)?;
        Ok(())
    }

    pub fn register_real_local_scid(&mut self, scid: u64) -> Result<(), RfqQuoteError> {
        if self.live_aliases.contains_key(&scid) {
            return Err(RfqQuoteError::AliasCollidesWithLiveQuote(scid));
        }

        self.real_local_scids.insert(scid);
        self.validate()
    }

    pub fn request_quote(
        &mut self,
        request: RfqQuoteRequest,
    ) -> Result<StoredRfqQuote, RfqQuoteError> {
        validate_request(&request)?;
        if request.expiry_unix_seconds <= request.now_unix_seconds {
            return Err(RfqQuoteError::QuoteExpired {
                quote_id: "new".to_owned(),
                now_unix_seconds: request.now_unix_seconds,
                expiry_unix_seconds: request.expiry_unix_seconds,
            });
        }
        if self
            .used_replay_domains
            .contains_key(&request.replay_domain)
            || self.live_replay_domain_exists(&request.replay_domain)
        {
            return Err(RfqQuoteError::ReplayDomainAlreadyUsed(
                request.replay_domain,
            ));
        }

        let oracle = FixedRateOracle::regtest_openusd();
        let btc_msat = oracle.quote_btc_msat(request.asset_amount)?;
        let scid_alias = self.allocate_scid_alias(&request, btc_msat)?;
        let quote_id = derive_quote_id(
            &request.peer,
            request.asset_id,
            request.asset_amount,
            btc_msat,
            request.expiry_unix_seconds,
            request.invoice_context,
            scid_alias,
            &request.replay_domain,
        );

        let quote = StoredRfqQuote {
            quote_id: quote_id.clone(),
            peer: request.peer,
            asset_id: request.asset_id,
            asset_amount: request.asset_amount,
            btc_msat,
            expiry_unix_seconds: request.expiry_unix_seconds,
            invoice_context: request.invoice_context,
            scid_alias,
            replay_domain: request.replay_domain,
            status: RfqQuoteStatus::Requested,
            created_at_unix_seconds: request.now_unix_seconds,
            accepted_at_unix_seconds: None,
            rejected_at_unix_seconds: None,
            rejected_reason: None,
            expired_at_unix_seconds: None,
            used_at_unix_seconds: None,
        };

        self.live_aliases.insert(scid_alias, quote_id.clone());
        self.quotes.insert(quote_id, quote.clone());
        self.validate()?;

        Ok(quote)
    }

    pub fn accept_quote(
        &mut self,
        quote_id: &str,
        now_unix_seconds: u64,
    ) -> Result<StoredRfqQuote, RfqQuoteError> {
        let quote = self
            .quotes
            .get_mut(quote_id)
            .ok_or_else(|| RfqQuoteError::UnknownQuote(quote_id.to_owned()))?;
        ensure_quote_not_expired(quote, now_unix_seconds)?;
        if quote.status != RfqQuoteStatus::Requested {
            return Err(RfqQuoteError::QuoteNotRequested {
                quote_id: quote_id.to_owned(),
                status: quote.status,
            });
        }
        if self.used_replay_domains.contains_key(&quote.replay_domain) {
            return Err(RfqQuoteError::ReplayDomainAlreadyUsed(
                quote.replay_domain.clone(),
            ));
        }

        quote.status = RfqQuoteStatus::Accepted;
        quote.accepted_at_unix_seconds = Some(now_unix_seconds);
        let quote = quote.clone();
        self.validate()?;

        Ok(quote)
    }

    pub fn reject_quote(
        &mut self,
        quote_id: &str,
        now_unix_seconds: u64,
        reason: String,
    ) -> Result<StoredRfqQuote, RfqQuoteError> {
        let reason = reason.trim().to_owned();
        if reason.is_empty() {
            return Err(RfqQuoteError::EmptyRejectReason);
        }
        let quote = self
            .quotes
            .get_mut(quote_id)
            .ok_or_else(|| RfqQuoteError::UnknownQuote(quote_id.to_owned()))?;
        if !matches!(
            quote.status,
            RfqQuoteStatus::Requested | RfqQuoteStatus::Accepted
        ) {
            return Err(RfqQuoteError::QuoteTerminal {
                quote_id: quote_id.to_owned(),
                status: quote.status,
            });
        }

        quote.status = RfqQuoteStatus::Rejected;
        quote.rejected_at_unix_seconds = Some(now_unix_seconds);
        quote.rejected_reason = Some(reason);
        self.live_aliases.remove(&quote.scid_alias);
        let quote = quote.clone();
        self.validate()?;

        Ok(quote)
    }

    pub fn expire_quote(
        &mut self,
        quote_id: &str,
        now_unix_seconds: u64,
    ) -> Result<StoredRfqQuote, RfqQuoteError> {
        let quote = self
            .quotes
            .get_mut(quote_id)
            .ok_or_else(|| RfqQuoteError::UnknownQuote(quote_id.to_owned()))?;
        if !matches!(
            quote.status,
            RfqQuoteStatus::Requested | RfqQuoteStatus::Accepted
        ) {
            return Err(RfqQuoteError::QuoteTerminal {
                quote_id: quote_id.to_owned(),
                status: quote.status,
            });
        }
        if now_unix_seconds <= quote.expiry_unix_seconds {
            return Err(RfqQuoteError::QuoteStillLive {
                quote_id: quote_id.to_owned(),
                now_unix_seconds,
                expiry_unix_seconds: quote.expiry_unix_seconds,
            });
        }

        quote.status = RfqQuoteStatus::Expired;
        quote.expired_at_unix_seconds = Some(now_unix_seconds);
        self.live_aliases.remove(&quote.scid_alias);
        let quote = quote.clone();
        self.validate()?;

        Ok(quote)
    }

    pub fn authorize_asset_htlc(
        &mut self,
        quote_id: &str,
        now_unix_seconds: u64,
    ) -> Result<RfqHtlcAuthorization, RfqQuoteError> {
        let quote = self
            .quotes
            .get_mut(quote_id)
            .ok_or_else(|| RfqQuoteError::UnknownQuote(quote_id.to_owned()))?;
        ensure_quote_not_expired(quote, now_unix_seconds)?;
        if quote.status != RfqQuoteStatus::Accepted {
            return Err(RfqQuoteError::QuoteNotAccepted {
                quote_id: quote_id.to_owned(),
                status: quote.status,
            });
        }
        if self.used_replay_domains.contains_key(&quote.replay_domain) {
            return Err(RfqQuoteError::ReplayDomainAlreadyUsed(
                quote.replay_domain.clone(),
            ));
        }

        quote.status = RfqQuoteStatus::Used;
        quote.used_at_unix_seconds = Some(now_unix_seconds);
        self.live_aliases.remove(&quote.scid_alias);
        self.used_replay_domains
            .insert(quote.replay_domain.clone(), quote.quote_id.clone());
        let authorization = RfqHtlcAuthorization::from_quote(quote);
        self.validate()?;

        Ok(authorization)
    }

    pub fn inspect_quote(&self, quote_id: &str) -> Result<StoredRfqQuote, RfqQuoteError> {
        self.quotes
            .get(quote_id)
            .cloned()
            .ok_or_else(|| RfqQuoteError::UnknownQuote(quote_id.to_owned()))
    }

    pub fn validate(&self) -> Result<(), RfqQuoteError> {
        if self.version != RFQ_STORE_SCHEMA_VERSION {
            return Err(RfqQuoteError::UnsupportedVersion(self.version));
        }

        let mut live_aliases = BTreeMap::<u64, String>::new();
        let mut used_replay_domains = BTreeMap::<String, String>::new();
        let mut live_replay_domains = BTreeSet::<String>::new();

        for (quote_id, quote) in &self.quotes {
            quote.validate_fields()?;
            if quote_id != &quote.quote_id {
                return Err(RfqQuoteError::StorageInvariant(format!(
                    "quote map key {quote_id} does not match quote_id {}",
                    quote.quote_id
                )));
            }
            let expected_quote_id = quote.binding_id();
            if expected_quote_id != *quote_id {
                return Err(RfqQuoteError::StorageInvariant(format!(
                    "quote {quote_id} binding hash does not match stored fields"
                )));
            }
            if self.real_local_scids.contains(&quote.scid_alias) {
                return Err(RfqQuoteError::AliasCollidesWithRealScid(quote.scid_alias));
            }

            match quote.status {
                RfqQuoteStatus::Requested => {
                    require_none("accepted_at_unix_seconds", quote.accepted_at_unix_seconds)?;
                    require_none("rejected_at_unix_seconds", quote.rejected_at_unix_seconds)?;
                    require_none("expired_at_unix_seconds", quote.expired_at_unix_seconds)?;
                    require_none("used_at_unix_seconds", quote.used_at_unix_seconds)?;
                    insert_unique_live_alias(&mut live_aliases, quote)?;
                    insert_unique_live_replay_domain(&mut live_replay_domains, quote)?;
                }
                RfqQuoteStatus::Accepted => {
                    require_some("accepted_at_unix_seconds", quote.accepted_at_unix_seconds)?;
                    require_none("rejected_at_unix_seconds", quote.rejected_at_unix_seconds)?;
                    require_none("expired_at_unix_seconds", quote.expired_at_unix_seconds)?;
                    require_none("used_at_unix_seconds", quote.used_at_unix_seconds)?;
                    insert_unique_live_alias(&mut live_aliases, quote)?;
                    insert_unique_live_replay_domain(&mut live_replay_domains, quote)?;
                }
                RfqQuoteStatus::Rejected => {
                    require_some("rejected_at_unix_seconds", quote.rejected_at_unix_seconds)?;
                    if quote
                        .rejected_reason
                        .as_ref()
                        .map(|reason| reason.trim().is_empty())
                        .unwrap_or(true)
                    {
                        return Err(RfqQuoteError::StorageInvariant(format!(
                            "rejected quote {quote_id} has no reason"
                        )));
                    }
                    require_none("expired_at_unix_seconds", quote.expired_at_unix_seconds)?;
                    require_none("used_at_unix_seconds", quote.used_at_unix_seconds)?;
                }
                RfqQuoteStatus::Expired => {
                    require_some("expired_at_unix_seconds", quote.expired_at_unix_seconds)?;
                    require_none("used_at_unix_seconds", quote.used_at_unix_seconds)?;
                }
                RfqQuoteStatus::Used => {
                    require_some("accepted_at_unix_seconds", quote.accepted_at_unix_seconds)?;
                    require_some("used_at_unix_seconds", quote.used_at_unix_seconds)?;
                    if used_replay_domains
                        .insert(quote.replay_domain.clone(), quote.quote_id.clone())
                        .is_some()
                    {
                        return Err(RfqQuoteError::StorageInvariant(format!(
                            "replay domain {} is used by multiple quotes",
                            quote.replay_domain
                        )));
                    }
                }
            }
        }

        if live_aliases != self.live_aliases {
            return Err(RfqQuoteError::StorageInvariant(
                "live alias index does not match quote statuses".to_owned(),
            ));
        }
        if used_replay_domains != self.used_replay_domains {
            return Err(RfqQuoteError::StorageInvariant(
                "used replay-domain index does not match used quotes".to_owned(),
            ));
        }

        Ok(())
    }

    fn live_replay_domain_exists(&self, replay_domain: &str) -> bool {
        self.quotes.values().any(|quote| {
            matches!(
                quote.status,
                RfqQuoteStatus::Requested | RfqQuoteStatus::Accepted
            ) && quote.replay_domain == replay_domain
        })
    }

    fn allocate_scid_alias(
        &self,
        request: &RfqQuoteRequest,
        btc_msat: u64,
    ) -> Result<u64, RfqQuoteError> {
        let seed = derive_quote_seed(
            &request.peer,
            request.asset_id,
            request.asset_amount,
            btc_msat,
            request.expiry_unix_seconds,
            request.invoice_context,
            &request.replay_domain,
        );
        let mut alias =
            RFQ_ALIAS_BASE | (u64::from_be_bytes(seed.0[..8].try_into().expect("8 bytes")) >> 1);

        for _ in 0..10_000 {
            if !self.real_local_scids.contains(&alias) && !self.live_aliases.contains_key(&alias) {
                return Ok(alias);
            }
            alias = alias.checked_add(1).ok_or(RfqQuoteError::AliasExhausted)?;
        }

        Err(RfqQuoteError::AliasExhausted)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RfqStoreMetadata {
    pub implementation: String,
    pub schema: String,
}

impl Default for RfqStoreMetadata {
    fn default() -> Self {
        Self {
            implementation: "tap-ldk experimental rfq store".to_owned(),
            schema: "bounded-regtest-rfq-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FixedRateOracle {
    pub ticker: &'static str,
    pub msats_per_asset_unit: u64,
}

impl FixedRateOracle {
    pub const fn regtest_openusd() -> Self {
        Self {
            ticker: "OPENUSD",
            msats_per_asset_unit: REGTEST_OPENUSD_MSATS_PER_UNIT,
        }
    }

    pub fn quote_btc_msat(self, asset_amount: u64) -> Result<u64, RfqQuoteError> {
        if asset_amount == 0 {
            return Err(RfqQuoteError::ZeroAssetAmount);
        }
        asset_amount
            .checked_mul(self.msats_per_asset_unit)
            .ok_or(RfqQuoteError::AmountOverflow)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RfqQuoteRequest {
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub expiry_unix_seconds: u64,
    pub invoice_context: Bytes32,
    pub replay_domain: String,
    pub now_unix_seconds: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredRfqQuote {
    pub quote_id: String,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub expiry_unix_seconds: u64,
    pub invoice_context: Bytes32,
    pub scid_alias: u64,
    pub replay_domain: String,
    pub status: RfqQuoteStatus,
    pub created_at_unix_seconds: u64,
    pub accepted_at_unix_seconds: Option<u64>,
    pub rejected_at_unix_seconds: Option<u64>,
    pub rejected_reason: Option<String>,
    pub expired_at_unix_seconds: Option<u64>,
    pub used_at_unix_seconds: Option<u64>,
}

impl StoredRfqQuote {
    pub fn binding_id(&self) -> String {
        derive_quote_id(
            &self.peer,
            self.asset_id,
            self.asset_amount,
            self.btc_msat,
            self.expiry_unix_seconds,
            self.invoice_context,
            self.scid_alias,
            &self.replay_domain,
        )
    }

    fn validate_fields(&self) -> Result<(), RfqQuoteError> {
        if self.peer.trim().is_empty() {
            return Err(RfqQuoteError::EmptyPeer);
        }
        if self.replay_domain.trim().is_empty() {
            return Err(RfqQuoteError::EmptyReplayDomain);
        }
        if self.asset_amount == 0 {
            return Err(RfqQuoteError::ZeroAssetAmount);
        }
        if self.btc_msat == 0 {
            return Err(RfqQuoteError::ZeroBtcAmount);
        }
        if self.expiry_unix_seconds <= self.created_at_unix_seconds {
            return Err(RfqQuoteError::InvalidExpiry {
                created_at_unix_seconds: self.created_at_unix_seconds,
                expiry_unix_seconds: self.expiry_unix_seconds,
            });
        }
        if self.scid_alias == 0 {
            return Err(RfqQuoteError::ZeroScidAlias);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RfqQuoteStatus {
    Requested,
    Accepted,
    Rejected,
    Expired,
    Used,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RfqHtlcAuthorization {
    pub quote_id: String,
    pub peer: String,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub btc_msat: u64,
    pub invoice_context: Bytes32,
    pub scid_alias: u64,
    pub replay_domain: String,
}

impl RfqHtlcAuthorization {
    fn from_quote(quote: &StoredRfqQuote) -> Self {
        Self {
            quote_id: quote.quote_id.clone(),
            peer: quote.peer.clone(),
            asset_id: quote.asset_id,
            asset_amount: quote.asset_amount,
            btc_msat: quote.btc_msat,
            invoice_context: quote.invoice_context,
            scid_alias: quote.scid_alias,
            replay_domain: quote.replay_domain.clone(),
        }
    }
}

#[derive(Debug)]
pub enum RfqQuoteError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    UnknownQuote(String),
    EmptyPeer,
    EmptyReplayDomain,
    EmptyRejectReason,
    ZeroAssetAmount,
    ZeroBtcAmount,
    ZeroScidAlias,
    AmountOverflow,
    InvalidExpiry {
        created_at_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    QuoteExpired {
        quote_id: String,
        now_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    QuoteStillLive {
        quote_id: String,
        now_unix_seconds: u64,
        expiry_unix_seconds: u64,
    },
    QuoteNotRequested {
        quote_id: String,
        status: RfqQuoteStatus,
    },
    QuoteNotAccepted {
        quote_id: String,
        status: RfqQuoteStatus,
    },
    QuoteTerminal {
        quote_id: String,
        status: RfqQuoteStatus,
    },
    ReplayDomainAlreadyUsed(String),
    AliasCollidesWithRealScid(u64),
    AliasCollidesWithLiveQuote(u64),
    AliasExhausted,
    StorageInvariant(String),
}

impl fmt::Display for RfqQuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "RFQ store I/O error: {err}"),
            Self::Json(err) => write!(f, "RFQ store JSON error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported RFQ store schema version {version}")
            }
            Self::UnknownQuote(quote_id) => write!(f, "unknown RFQ quote: {quote_id}"),
            Self::EmptyPeer => write!(f, "RFQ peer cannot be empty"),
            Self::EmptyReplayDomain => write!(f, "RFQ replay domain cannot be empty"),
            Self::EmptyRejectReason => write!(f, "RFQ reject reason cannot be empty"),
            Self::ZeroAssetAmount => write!(f, "RFQ asset amount must be greater than zero"),
            Self::ZeroBtcAmount => write!(f, "RFQ BTC amount must be greater than zero"),
            Self::ZeroScidAlias => write!(f, "RFQ SCID alias must be greater than zero"),
            Self::AmountOverflow => write!(f, "RFQ amount conversion overflowed"),
            Self::InvalidExpiry {
                created_at_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "RFQ expiry {expiry_unix_seconds} must be after creation time {created_at_unix_seconds}"
            ),
            Self::QuoteExpired {
                quote_id,
                now_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "RFQ quote {quote_id} expired at {expiry_unix_seconds}; now {now_unix_seconds}"
            ),
            Self::QuoteStillLive {
                quote_id,
                now_unix_seconds,
                expiry_unix_seconds,
            } => write!(
                f,
                "RFQ quote {quote_id} is still live at {now_unix_seconds}; expiry {expiry_unix_seconds}"
            ),
            Self::QuoteNotRequested { quote_id, status } => {
                write!(f, "RFQ quote {quote_id} is {status:?}, not requested")
            }
            Self::QuoteNotAccepted { quote_id, status } => {
                write!(f, "RFQ quote {quote_id} is {status:?}, not accepted")
            }
            Self::QuoteTerminal { quote_id, status } => {
                write!(f, "RFQ quote {quote_id} is terminal: {status:?}")
            }
            Self::ReplayDomainAlreadyUsed(replay_domain) => {
                write!(f, "RFQ replay domain already used: {replay_domain}")
            }
            Self::AliasCollidesWithRealScid(scid) => {
                write!(f, "RFQ alias {scid} collides with a real local SCID")
            }
            Self::AliasCollidesWithLiveQuote(scid) => {
                write!(f, "RFQ alias {scid} collides with a live quote")
            }
            Self::AliasExhausted => write!(f, "RFQ SCID alias space exhausted"),
            Self::StorageInvariant(message) => {
                write!(f, "RFQ storage invariant failed: {message}")
            }
        }
    }
}

impl Error for RfqQuoteError {}

fn validate_request(request: &RfqQuoteRequest) -> Result<(), RfqQuoteError> {
    if request.peer.trim().is_empty() {
        return Err(RfqQuoteError::EmptyPeer);
    }
    if request.replay_domain.trim().is_empty() {
        return Err(RfqQuoteError::EmptyReplayDomain);
    }
    if request.asset_amount == 0 {
        return Err(RfqQuoteError::ZeroAssetAmount);
    }
    Ok(())
}

fn ensure_quote_not_expired(
    quote: &StoredRfqQuote,
    now_unix_seconds: u64,
) -> Result<(), RfqQuoteError> {
    if now_unix_seconds > quote.expiry_unix_seconds {
        return Err(RfqQuoteError::QuoteExpired {
            quote_id: quote.quote_id.clone(),
            now_unix_seconds,
            expiry_unix_seconds: quote.expiry_unix_seconds,
        });
    }
    Ok(())
}

fn insert_unique_live_alias(
    live_aliases: &mut BTreeMap<u64, String>,
    quote: &StoredRfqQuote,
) -> Result<(), RfqQuoteError> {
    if live_aliases
        .insert(quote.scid_alias, quote.quote_id.clone())
        .is_some()
    {
        return Err(RfqQuoteError::AliasCollidesWithLiveQuote(quote.scid_alias));
    }
    Ok(())
}

fn insert_unique_live_replay_domain(
    live_replay_domains: &mut BTreeSet<String>,
    quote: &StoredRfqQuote,
) -> Result<(), RfqQuoteError> {
    if !live_replay_domains.insert(quote.replay_domain.clone()) {
        return Err(RfqQuoteError::ReplayDomainAlreadyUsed(
            quote.replay_domain.clone(),
        ));
    }
    Ok(())
}

fn require_some(field: &'static str, value: Option<u64>) -> Result<(), RfqQuoteError> {
    if value.is_none() {
        return Err(RfqQuoteError::StorageInvariant(format!(
            "{field} is required"
        )));
    }
    Ok(())
}

fn require_none(field: &'static str, value: Option<u64>) -> Result<(), RfqQuoteError> {
    if value.is_some() {
        return Err(RfqQuoteError::StorageInvariant(format!(
            "{field} must be empty"
        )));
    }
    Ok(())
}

fn derive_quote_seed(
    peer: &str,
    asset_id: Bytes32,
    asset_amount: u64,
    btc_msat: u64,
    expiry_unix_seconds: u64,
    invoice_context: Bytes32,
    replay_domain: &str,
) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:rfq-quote-seed:v1");
    hasher.update((peer.len() as u64).to_be_bytes());
    hasher.update(peer.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(asset_amount.to_be_bytes());
    hasher.update(btc_msat.to_be_bytes());
    hasher.update(expiry_unix_seconds.to_be_bytes());
    hasher.update(invoice_context.0);
    hasher.update((replay_domain.len() as u64).to_be_bytes());
    hasher.update(replay_domain.as_bytes());
    Bytes32(hasher.finalize().into())
}

fn derive_quote_id(
    peer: &str,
    asset_id: Bytes32,
    asset_amount: u64,
    btc_msat: u64,
    expiry_unix_seconds: u64,
    invoice_context: Bytes32,
    scid_alias: u64,
    replay_domain: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:rfq-quote-binding:v1");
    hasher.update((peer.len() as u64).to_be_bytes());
    hasher.update(peer.as_bytes());
    hasher.update(asset_id.0);
    hasher.update(asset_amount.to_be_bytes());
    hasher.update(btc_msat.to_be_bytes());
    hasher.update(expiry_unix_seconds.to_be_bytes());
    hasher.update(invoice_context.0);
    hasher.update(scid_alias.to_be_bytes());
    hasher.update((replay_domain.len() as u64).to_be_bytes());
    hasher.update(replay_domain.as_bytes());
    Bytes32(hasher.finalize().into()).to_hex()
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rfq-quotes.json".to_owned());
    path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn asset_id() -> Bytes32 {
        Bytes32([7; 32])
    }

    fn invoice_context() -> Bytes32 {
        Bytes32([8; 32])
    }

    fn request(replay_domain: &str) -> RfqQuoteRequest {
        RfqQuoteRequest {
            peer: "02peer".to_owned(),
            asset_id: asset_id(),
            asset_amount: 25,
            expiry_unix_seconds: 200,
            invoice_context: invoice_context(),
            replay_domain: replay_domain.to_owned(),
            now_unix_seconds: 100,
        }
    }

    #[test]
    fn quote_lifecycle_persists_and_replay_fails_closed() {
        let path = temp_store_path("lifecycle");
        let mut store = RfqQuoteStore::default();
        let quote = store
            .request_quote(request("path-a:invoice-1"))
            .expect("quote requested");
        assert_eq!(quote.status, RfqQuoteStatus::Requested);
        assert_eq!(
            quote.btc_msat,
            quote.asset_amount * REGTEST_OPENUSD_MSATS_PER_UNIT
        );

        store.accept_quote(&quote.quote_id, 110).expect("accepted");
        let authorization = store
            .authorize_asset_htlc(&quote.quote_id, 120)
            .expect("authorized once");
        assert_eq!(authorization.btc_msat, quote.btc_msat);
        assert!(matches!(
            store.authorize_asset_htlc(&quote.quote_id, 121),
            Err(RfqQuoteError::QuoteNotAccepted { .. })
        ));
        assert!(matches!(
            store.request_quote(request("path-a:invoice-1")),
            Err(RfqQuoteError::ReplayDomainAlreadyUsed(_))
        ));

        store.save_atomic(&path).expect("store saves");
        let loaded = RfqQuoteStore::load(&path).expect("store loads");
        assert_eq!(
            loaded
                .inspect_quote(&quote.quote_id)
                .expect("quote remains")
                .status,
            RfqQuoteStatus::Used
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn expiry_rejects_accept_and_moves_quote_to_terminal_expired() {
        let mut store = RfqQuoteStore::default();
        let quote = store
            .request_quote(request("path-a:invoice-2"))
            .expect("quote requested");

        assert!(matches!(
            store.accept_quote(&quote.quote_id, 201),
            Err(RfqQuoteError::QuoteExpired { .. })
        ));
        let expired = store
            .expire_quote(&quote.quote_id, 201)
            .expect("quote expires");
        assert_eq!(expired.status, RfqQuoteStatus::Expired);
        assert!(!store.live_aliases.contains_key(&quote.scid_alias));
    }

    #[test]
    fn aliases_skip_real_scids_and_live_alias_registration_fails() {
        let mut store = RfqQuoteStore::default();
        let req = request("path-a:invoice-3");
        let btc_msat = FixedRateOracle::regtest_openusd()
            .quote_btc_msat(req.asset_amount)
            .expect("amount converts");
        let first_alias = store
            .allocate_scid_alias(&req, btc_msat)
            .expect("alias allocates");
        store
            .register_real_local_scid(first_alias)
            .expect("scid stores");

        let quote = store.request_quote(req).expect("quote requested");
        assert_ne!(quote.scid_alias, first_alias);
        assert!(!store.real_local_scids.contains(&quote.scid_alias));
        assert!(matches!(
            store.register_real_local_scid(quote.scid_alias),
            Err(RfqQuoteError::AliasCollidesWithLiveQuote(alias)) if alias == quote.scid_alias
        ));
    }

    #[test]
    fn rejected_quote_releases_alias_and_replay_domain() {
        let mut store = RfqQuoteStore::default();
        let quote = store
            .request_quote(request("path-a:invoice-4"))
            .expect("quote requested");
        let rejected = store
            .reject_quote(&quote.quote_id, 120, "no route".to_owned())
            .expect("quote rejected");
        assert_eq!(rejected.status, RfqQuoteStatus::Rejected);
        assert!(!store.live_aliases.contains_key(&quote.scid_alias));

        let second = store
            .request_quote(request("path-a:invoice-4"))
            .expect("rejected replay domain can be requested again");
        assert_eq!(second.status, RfqQuoteStatus::Requested);
    }

    #[test]
    fn tampered_binding_fields_fail_validation() {
        let mut store = RfqQuoteStore::default();
        let quote = store
            .request_quote(request("path-a:invoice-5"))
            .expect("quote requested");
        store
            .quotes
            .get_mut(&quote.quote_id)
            .expect("quote exists")
            .asset_amount += 1;

        assert!(matches!(
            store.validate(),
            Err(RfqQuoteError::StorageInvariant(message))
                if message.contains("binding hash does not match")
        ));
    }

    fn temp_store_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tap_ldk_rfq_store_{name}_{}_{}.json",
            std::process::id(),
            nanos
        ))
    }
}
