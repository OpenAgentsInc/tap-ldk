#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BoundaryHome {
    TapLdk,
    OpenAgentsRustLightningFork,
    LdkNodeAdapter,
    LightningLabsInteropHarness,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ForkRequirement {
    NotRequired,
    RequiredForDemo,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AssetChannelHook {
    FeatureNegotiation,
    ChannelType,
    CustomMessageRouter,
    FundingProofCollector,
    FundingController,
    AssetCommitmentBlob,
    AssetSigner,
    HtlcMetadataModifier,
    FinalHopValidator,
    RfqManager,
    InvoiceBinder,
    CloseHandler,
    OnChainResolver,
    MonitorPersistence,
    InteropBlobCodec,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AssetChannelBoundary {
    pub hook: AssetChannelHook,
    pub home: BoundaryHome,
    pub fork_requirement: ForkRequirement,
    pub invariant: &'static str,
}

pub const ASSET_CHANNEL_BOUNDARIES: &[AssetChannelBoundary] = &[
    AssetChannelBoundary {
        hook: AssetChannelHook::FeatureNegotiation,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "normal BTC channels remain BTC-only unless the asset feature is negotiated",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::ChannelType,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "an asset channel cannot be opened as an implicit normal channel",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::CustomMessageRouter,
        home: BoundaryHome::TapLdk,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "asset peer messages are rejected before feature negotiation succeeds",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::FundingProofCollector,
        home: BoundaryHome::TapLdk,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "funding cannot advance until all proof fragments are reconstructed and verified",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::FundingController,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "funding cannot advance with mismatched asset ID, proof root, or output commitment",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::AssetCommitmentBlob,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "asset-channel commitment blobs are versioned with the Lightning commitment number",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::AssetSigner,
        home: BoundaryHome::TapLdk,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "asset-level signatures and nonces remain separate from BTC-level signing",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::HtlcMetadataModifier,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "asset HTLC metadata cannot be attached without an accepted quote",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::FinalHopValidator,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "wrong, stale, missing, or malformed final-hop asset metadata fails closed",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::RfqManager,
        home: BoundaryHome::TapLdk,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "quotes bind asset ID, asset amount, BTC amount, peer, expiry, and replay domain",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::InvoiceBinder,
        home: BoundaryHome::LdkNodeAdapter,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "BOLT 11 format stays unchanged while RFQ binds the asset payment context",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::CloseHandler,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "cooperative close returns the latest mutually valid asset allocation",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::OnChainResolver,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "force-close recovery cannot discard proof ownership material",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::MonitorPersistence,
        home: BoundaryHome::OpenAgentsRustLightningFork,
        fork_requirement: ForkRequirement::RequiredForDemo,
        invariant: "asset-channel state is durable before the corresponding Lightning commitment is safe",
    },
    AssetChannelBoundary {
        hook: AssetChannelHook::InteropBlobCodec,
        home: BoundaryHome::LightningLabsInteropHarness,
        fork_requirement: ForkRequirement::NotRequired,
        invariant: "Lightning Labs blob mismatches are failing compatibility gaps",
    },
];

pub fn boundary_for(hook: AssetChannelHook) -> Option<&'static AssetChannelBoundary> {
    ASSET_CHANNEL_BOUNDARIES
        .iter()
        .find(|boundary| boundary.hook == hook)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_required_hook_has_one_boundary() {
        let mut seen = BTreeSet::new();
        for boundary in ASSET_CHANNEL_BOUNDARIES {
            assert!(
                seen.insert(boundary.hook as u8),
                "duplicate boundary for {:?}",
                boundary.hook
            );
            assert!(!boundary.invariant.is_empty());
        }

        assert_eq!(seen.len(), 15);
    }

    #[test]
    fn fork_required_hooks_are_explicitly_openagents_fork_hooks() {
        for boundary in ASSET_CHANNEL_BOUNDARIES {
            if boundary.fork_requirement == ForkRequirement::RequiredForDemo {
                assert_eq!(boundary.home, BoundaryHome::OpenAgentsRustLightningFork);
            }
        }
    }

    #[test]
    fn no_boundary_assigns_wallet_runtime_to_lightning_labs_sidecar() {
        for boundary in ASSET_CHANNEL_BOUNDARIES {
            if boundary.hook == AssetChannelHook::InteropBlobCodec {
                continue;
            }
            assert_ne!(boundary.home, BoundaryHome::LightningLabsInteropHarness);
        }

        assert_eq!(
            boundary_for(AssetChannelHook::InteropBlobCodec)
                .expect("interop boundary exists")
                .home,
            BoundaryHome::LightningLabsInteropHarness
        );
    }
}
