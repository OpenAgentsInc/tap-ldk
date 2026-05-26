use std::{error::Error, fmt};

use lightning::{
    ln::taproot_asset::{self, TaprootAssetChannelDescriptor, TaprootAssetChannelNegotiationError},
    types::features::{ChannelTypeFeatures, InitFeatures},
};
use serde::{Deserialize, Serialize};

use crate::asset::Bytes32;

pub const ASSET_CHANNEL_PROTOCOL_VERSION: u16 =
    taproot_asset::SUPPORTED_TAPROOT_ASSET_CHANNEL_PROTOCOL_VERSION;
pub const ASSET_CHANNEL_REQUIRED_FEATURE_BIT: u16 = 150;
pub const ASSET_CHANNEL_OPTIONAL_FEATURE_BIT: u16 = ASSET_CHANNEL_REQUIRED_FEATURE_BIT + 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetChannelFeatureSet {
    pub required: bool,
    pub optional: bool,
    pub protocol_version: u16,
}

impl AssetChannelFeatureSet {
    pub const fn disabled() -> Self {
        Self {
            required: false,
            optional: false,
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
        }
    }

    pub const fn advertise_optional() -> Self {
        Self {
            required: false,
            optional: true,
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
        }
    }

    pub const fn require() -> Self {
        Self {
            required: true,
            optional: false,
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
        }
    }

    pub fn supports_asset_channels(self) -> bool {
        self.required || self.optional
    }

    pub fn feature_bits(self) -> Vec<u16> {
        let mut bits = Vec::new();
        if self.required {
            bits.push(ASSET_CHANNEL_REQUIRED_FEATURE_BIT);
        } else if self.optional {
            bits.push(ASSET_CHANNEL_OPTIONAL_FEATURE_BIT);
        }
        bits
    }

    pub fn to_ldk_init_features(self) -> InitFeatures {
        let mut features = InitFeatures::empty();
        features.set_static_remote_key_optional();
        features.set_channel_type_optional();
        if self.required {
            features.set_taproot_asset_channel_required();
        } else if self.optional {
            features.set_taproot_asset_channel_optional();
        }
        features
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChannelRequest {
    BtcOnly,
    SingleAsset { asset_id: Bytes32 },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum NegotiatedChannelType {
    BtcOnly,
    SingleAsset {
        asset_id: Bytes32,
        protocol_version: u16,
    },
}

impl NegotiatedChannelType {
    pub fn is_asset_channel(&self) -> bool {
        matches!(self, Self::SingleAsset { .. })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NegotiationInput {
    pub local: AssetChannelFeatureSet,
    pub remote: AssetChannelFeatureSet,
    pub request: ChannelRequest,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NegotiationOutcome {
    pub channel_type: NegotiatedChannelType,
    pub local_feature_bits: Vec<u16>,
    pub remote_feature_bits: Vec<u16>,
}

pub fn negotiate_channel(input: NegotiationInput) -> Result<NegotiationOutcome, NegotiationError> {
    if input.local.protocol_version != ASSET_CHANNEL_PROTOCOL_VERSION {
        return Err(NegotiationError::UnsupportedLocalProtocolVersion(
            input.local.protocol_version,
        ));
    }
    if input.remote.protocol_version != ASSET_CHANNEL_PROTOCOL_VERSION {
        return Err(NegotiationError::UnsupportedRemoteProtocolVersion(
            input.remote.protocol_version,
        ));
    }

    match input.request {
        ChannelRequest::BtcOnly => Ok(NegotiationOutcome {
            channel_type: NegotiatedChannelType::BtcOnly,
            local_feature_bits: input.local.feature_bits(),
            remote_feature_bits: input.remote.feature_bits(),
        }),
        ChannelRequest::SingleAsset { asset_id } => {
            if asset_id == Bytes32::ZERO {
                return Err(NegotiationError::MissingAssetId);
            }
            let local_features = input.local.to_ldk_init_features();
            let remote_features = input.remote.to_ldk_init_features();
            let descriptor =
                TaprootAssetChannelDescriptor::new(asset_id.0, ASSET_CHANNEL_PROTOCOL_VERSION)
                    .map_err(map_ldk_negotiation_error)?;
            let fork_negotiation = taproot_asset::negotiate_single_asset_channel(
                &local_features,
                &remote_features,
                descriptor,
            )
            .map_err(map_ldk_negotiation_error)?;
            taproot_asset::validate_single_asset_channel_open(
                &local_features,
                &remote_features,
                &fork_negotiation.channel_type,
                descriptor,
            )
            .map_err(map_ldk_negotiation_error)?;

            Ok(NegotiationOutcome {
                channel_type: NegotiatedChannelType::SingleAsset {
                    asset_id,
                    protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
                },
                local_feature_bits: input.local.feature_bits(),
                remote_feature_bits: input.remote.feature_bits(),
            })
        }
    }
}

pub fn require_asset_message_allowed(
    channel_type: &NegotiatedChannelType,
) -> Result<(), NegotiationError> {
    if !channel_type.is_asset_channel() {
        return Err(NegotiationError::PrematureAssetMessage);
    }

    Ok(())
}

pub fn fork_rejects_implicit_asset_upgrade(asset_id: Bytes32) -> Result<bool, NegotiationError> {
    let features = AssetChannelFeatureSet::require().to_ldk_init_features();
    let descriptor = TaprootAssetChannelDescriptor::new(asset_id.0, ASSET_CHANNEL_PROTOCOL_VERSION)
        .map_err(map_ldk_negotiation_error)?;
    Ok(taproot_asset::validate_single_asset_channel_open(
        &features,
        &features,
        &ChannelTypeFeatures::only_static_remote_key(),
        descriptor,
    )
    .is_err())
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NegotiationError {
    UnsupportedLocalProtocolVersion(u16),
    UnsupportedRemoteProtocolVersion(u16),
    LocalFeatureMissing,
    RemoteFeatureMissing,
    MissingAssetId,
    PrematureAssetMessage,
    MissingAssetChannelType,
    MalformedAssetChannelType,
    UnsupportedAssetChannelType,
}

impl fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocalProtocolVersion(version) => {
                write!(
                    f,
                    "unsupported local asset-channel protocol version {version}"
                )
            }
            Self::UnsupportedRemoteProtocolVersion(version) => {
                write!(
                    f,
                    "unsupported remote asset-channel protocol version {version}"
                )
            }
            Self::LocalFeatureMissing => write!(f, "local peer did not advertise asset channels"),
            Self::RemoteFeatureMissing => write!(f, "remote peer did not advertise asset channels"),
            Self::MissingAssetId => write!(f, "asset channel request requires a non-zero asset id"),
            Self::PrematureAssetMessage => {
                write!(
                    f,
                    "asset-channel message sent before successful negotiation"
                )
            }
            Self::MissingAssetChannelType => {
                write!(
                    f,
                    "asset channel request requires explicit asset channel type"
                )
            }
            Self::MalformedAssetChannelType => write!(f, "asset channel type is malformed"),
            Self::UnsupportedAssetChannelType => write!(f, "asset channel type is unsupported"),
        }
    }
}

impl Error for NegotiationError {}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NegotiationSmokeReport {
    pub btc_only_channel: NegotiatedChannelType,
    pub asset_channel: NegotiatedChannelType,
    pub premature_asset_message_rejected: bool,
    pub fork_implicit_asset_upgrade_rejected: bool,
}

pub fn run_negotiation_smoke(
    asset_id: Bytes32,
) -> Result<NegotiationSmokeReport, NegotiationError> {
    let btc_only = negotiate_channel(NegotiationInput {
        local: AssetChannelFeatureSet::disabled(),
        remote: AssetChannelFeatureSet::disabled(),
        request: ChannelRequest::BtcOnly,
    })?;
    let asset = negotiate_channel(NegotiationInput {
        local: AssetChannelFeatureSet::require(),
        remote: AssetChannelFeatureSet::advertise_optional(),
        request: ChannelRequest::SingleAsset { asset_id },
    })?;
    let premature_asset_message_rejected =
        require_asset_message_allowed(&btc_only.channel_type).is_err();
    require_asset_message_allowed(&asset.channel_type)?;
    let fork_implicit_asset_upgrade_rejected = fork_rejects_implicit_asset_upgrade(asset_id)?;

    Ok(NegotiationSmokeReport {
        btc_only_channel: btc_only.channel_type,
        asset_channel: asset.channel_type,
        premature_asset_message_rejected,
        fork_implicit_asset_upgrade_rejected,
    })
}

fn map_ldk_negotiation_error(err: TaprootAssetChannelNegotiationError) -> NegotiationError {
    match err {
        TaprootAssetChannelNegotiationError::MissingLocalSupport => {
            NegotiationError::LocalFeatureMissing
        }
        TaprootAssetChannelNegotiationError::MissingRemoteSupport => {
            NegotiationError::RemoteFeatureMissing
        }
        TaprootAssetChannelNegotiationError::MissingAssetChannelType => {
            NegotiationError::MissingAssetChannelType
        }
        TaprootAssetChannelNegotiationError::MalformedChannelType => {
            NegotiationError::MalformedAssetChannelType
        }
        TaprootAssetChannelNegotiationError::UnsupportedChannelType => {
            NegotiationError::UnsupportedAssetChannelType
        }
        TaprootAssetChannelNegotiationError::MalformedAssetId => NegotiationError::MissingAssetId,
        TaprootAssetChannelNegotiationError::UnsupportedProtocolVersion => {
            NegotiationError::UnsupportedLocalProtocolVersion(ASSET_CHANNEL_PROTOCOL_VERSION)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_id() -> Bytes32 {
        Bytes32([7; 32])
    }

    #[test]
    fn btc_only_channel_negotiates_without_asset_features() {
        let outcome = negotiate_channel(NegotiationInput {
            local: AssetChannelFeatureSet::disabled(),
            remote: AssetChannelFeatureSet::disabled(),
            request: ChannelRequest::BtcOnly,
        })
        .expect("btc-only negotiation succeeds");

        assert_eq!(outcome.channel_type, NegotiatedChannelType::BtcOnly);
        assert!(outcome.local_feature_bits.is_empty());
        assert!(outcome.remote_feature_bits.is_empty());
    }

    #[test]
    fn single_asset_channel_requires_both_peers_to_support_feature() {
        let outcome = negotiate_channel(NegotiationInput {
            local: AssetChannelFeatureSet::require(),
            remote: AssetChannelFeatureSet::advertise_optional(),
            request: ChannelRequest::SingleAsset {
                asset_id: asset_id(),
            },
        })
        .expect("asset negotiation succeeds");

        assert_eq!(
            outcome.channel_type,
            NegotiatedChannelType::SingleAsset {
                asset_id: asset_id(),
                protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION
            }
        );
        assert_eq!(
            outcome.local_feature_bits,
            vec![ASSET_CHANNEL_REQUIRED_FEATURE_BIT]
        );
        assert_eq!(
            outcome.remote_feature_bits,
            vec![ASSET_CHANNEL_OPTIONAL_FEATURE_BIT]
        );
    }

    #[test]
    fn missing_or_stale_feature_fails_closed() {
        assert_eq!(
            negotiate_channel(NegotiationInput {
                local: AssetChannelFeatureSet::require(),
                remote: AssetChannelFeatureSet::disabled(),
                request: ChannelRequest::SingleAsset {
                    asset_id: asset_id(),
                },
            }),
            Err(NegotiationError::RemoteFeatureMissing)
        );

        let mut stale = AssetChannelFeatureSet::advertise_optional();
        stale.protocol_version = ASSET_CHANNEL_PROTOCOL_VERSION + 1;
        assert_eq!(
            negotiate_channel(NegotiationInput {
                local: AssetChannelFeatureSet::require(),
                remote: stale,
                request: ChannelRequest::SingleAsset {
                    asset_id: asset_id(),
                },
            }),
            Err(NegotiationError::UnsupportedRemoteProtocolVersion(
                ASSET_CHANNEL_PROTOCOL_VERSION + 1
            ))
        );
    }

    #[test]
    fn asset_messages_are_rejected_before_asset_channel_negotiation() {
        assert_eq!(
            require_asset_message_allowed(&NegotiatedChannelType::BtcOnly),
            Err(NegotiationError::PrematureAssetMessage)
        );
        require_asset_message_allowed(&NegotiatedChannelType::SingleAsset {
            asset_id: asset_id(),
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
        })
        .expect("asset channel permits asset messages");
    }

    #[test]
    fn smoke_covers_disabled_and_enabled_paths() {
        let report = run_negotiation_smoke(asset_id()).expect("smoke passes");

        assert_eq!(report.btc_only_channel, NegotiatedChannelType::BtcOnly);
        assert!(report.asset_channel.is_asset_channel());
        assert!(report.premature_asset_message_rejected);
        assert!(report.fork_implicit_asset_upgrade_rejected);
    }
}
