pub const OPENAGENTS_RUST_LIGHTNING_FORK_URL: &str =
    "https://github.com/OpenAgentsInc/rust-lightning.git";
pub const OPENAGENTS_RUST_LIGHTNING_BASE_REV: &str = "0c37f08a55c0f7738f2691dc3690166fd42f851d";
pub const OPENAGENTS_RUST_LIGHTNING_REV: &str = "a7cb50c64ba589e1171526f04f199d09cac35812";

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

pub fn taproot_asset_channel_state_type_name() -> &'static str {
    std::any::type_name::<lightning::ln::taproot_asset::TaprootAssetChannelState>()
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
        assert!(taproot_asset_channel_state_type_name().contains("lightning"));
    }

    #[test]
    fn fork_dependency_builds_simple_taproot_p2tr_funding_script() {
        use lightning::bitcoin::hashes::{Hash, sha256::Hash as Sha256};

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

        let to_remote = lightning::ln::simple_taproot::simple_taproot_to_remote_spend_info(
            &secp_ctx,
            &remote_pubkey,
        )
        .unwrap();
        assert!(to_remote.script_pubkey.is_p2tr());
        assert_eq!(to_remote.spend.control_block.len(), 33);

        let to_local = lightning::ln::simple_taproot::simple_taproot_to_local_spend_info(
            &secp_ctx,
            &local_pubkey,
            &remote_pubkey,
            144,
        )
        .unwrap();
        assert!(to_local.script_pubkey.is_p2tr());
        assert_eq!(to_local.delay.control_block.len(), 65);
        assert_eq!(to_local.revocation.control_block.len(), 65);

        let payment_hash =
            lightning::types::payment::PaymentHash(Sha256::hash(&[0_u8; 32]).to_byte_array());
        let htlc = lightning::ln::simple_taproot::simple_taproot_htlc_spend_info(
            &secp_ctx,
            true,
            &payment_hash,
            500,
            &local_pubkey,
            &remote_pubkey,
            &remote_pubkey,
        )
        .unwrap();
        assert!(htlc.script_pubkey.is_p2tr());
        assert_eq!(htlc.success.control_block.len(), 65);
        assert_eq!(htlc.timeout.control_block.len(), 65);
    }
}
