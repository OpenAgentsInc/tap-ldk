pub mod address;
pub mod asset;
pub mod asset_channel_boundary;
pub mod asset_channel_funding;
pub mod asset_channel_negotiation;
pub mod asset_close;
pub mod asset_commitment;
pub mod asset_htlc;
pub mod asset_payment;
pub mod asset_peer_message;
pub mod asset_recovery;
pub mod ldk_baseline;
pub mod ldk_fork;
pub mod lightning_labs_blob;
pub mod lightning_labs_funding;
pub mod lightning_labs_interop_checks;
pub mod lightning_labs_payment;
pub mod lightning_labs_rfq;
pub mod live_peer;
pub mod live_tapd_proof;
pub mod proof;
pub mod regtest;
pub mod rfq_invoice;
pub mod rfq_quote_store;
pub mod tapd_proof;
pub mod tlv;
pub mod virtual_psbt;
pub mod wallet;

pub const PROJECT_NAME: &str = "tap-ldk";
pub const PROJECT_SUMMARY: &str =
    "Experimental native Taproot Assets support for Rust Lightning/LDK.";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ProjectInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub summary: &'static str,
}

impl ProjectInfo {
    pub fn current() -> Self {
        Self {
            name: PROJECT_NAME,
            version: env!("CARGO_PKG_VERSION"),
            summary: PROJECT_SUMMARY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_project_info_is_populated() {
        let info = ProjectInfo::current();

        assert_eq!(info.name, "tap-ldk");
        assert!(!info.version.is_empty());
        assert!(info.summary.contains("Taproot Assets"));
    }
}
