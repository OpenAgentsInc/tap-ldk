pub const OPENAGENTS_RUST_LIGHTNING_FORK_URL: &str =
    "https://github.com/OpenAgentsInc/rust-lightning.git";
pub const OPENAGENTS_RUST_LIGHTNING_BASE_REV: &str = "0c37f08a55c0f7738f2691dc3690166fd42f851d";
pub const OPENAGENTS_RUST_LIGHTNING_REV: &str = "1602ac9e1e7454d39612e126c24a098e276d605a";

pub fn channel_type_features_type_name() -> &'static str {
    std::any::type_name::<lightning::types::features::ChannelTypeFeatures>()
}

pub fn init_features_type_name() -> &'static str {
    std::any::type_name::<lightning::types::features::InitFeatures>()
}

pub fn simple_taproot_nonce_state_type_name() -> &'static str {
    std::any::type_name::<lightning::ln::simple_taproot::SimpleTaprootNonceState>()
}

pub fn simple_taproot_signer_trait_type_name() -> &'static str {
    std::any::type_name::<dyn lightning::sign::SimpleTaprootChannelSigner>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_metadata_is_openagents_and_pinned() {
        assert_eq!(
            OPENAGENTS_RUST_LIGHTNING_FORK_URL,
            "https://github.com/OpenAgentsInc/rust-lightning.git"
        );
        assert_eq!(OPENAGENTS_RUST_LIGHTNING_BASE_REV.len(), 40);
        assert_eq!(OPENAGENTS_RUST_LIGHTNING_REV.len(), 40);
    }

    #[test]
    fn fork_dependency_exposes_expected_ldk_feature_types() {
        assert!(channel_type_features_type_name().contains("lightning"));
        assert!(init_features_type_name().contains("lightning"));
        assert!(simple_taproot_nonce_state_type_name().contains("lightning"));
        assert!(simple_taproot_signer_trait_type_name().contains("lightning"));
    }

    #[test]
    fn fork_dependency_builds_simple_taproot_p2tr_funding_script() {
        let secp_ctx = lightning::bitcoin::secp256k1::Secp256k1::new();
        let local_secret =
            lightning::bitcoin::secp256k1::SecretKey::from_slice(&[1_u8; 32]).unwrap();
        let remote_secret =
            lightning::bitcoin::secp256k1::SecretKey::from_slice(&[2_u8; 32]).unwrap();
        let local_pubkey =
            lightning::bitcoin::secp256k1::PublicKey::from_secret_key(&secp_ctx, &local_secret);
        let remote_pubkey =
            lightning::bitcoin::secp256k1::PublicKey::from_secret_key(&secp_ctx, &remote_secret);
        let script_pubkey =
            lightning::ln::simple_taproot::SimpleTaprootKeyAggContext::for_funding_keys(
                local_pubkey,
                remote_pubkey,
            )
            .bip86_funding_script_pubkey(&secp_ctx)
            .unwrap();

        assert!(script_pubkey.is_p2tr());
    }
}
