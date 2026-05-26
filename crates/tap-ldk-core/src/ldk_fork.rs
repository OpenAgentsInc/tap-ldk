pub const OPENAGENTS_RUST_LIGHTNING_FORK_URL: &str =
    "https://github.com/OpenAgentsInc/rust-lightning.git";
pub const OPENAGENTS_RUST_LIGHTNING_BASE_REV: &str = "0c37f08a55c0f7738f2691dc3690166fd42f851d";
pub const OPENAGENTS_RUST_LIGHTNING_REV: &str = "6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941";

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
}
