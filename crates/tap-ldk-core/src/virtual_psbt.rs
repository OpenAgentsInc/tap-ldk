use std::{error::Error, fmt};

use crate::asset::{AssetAmount, AssetError};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VirtualPacketSummary {
    pub version: u8,
    pub chain_params_hrp: String,
    pub input_count: usize,
    pub output_count: usize,
    pub total_output_amount: AssetAmount,
    pub signing_domain: SigningDomain,
}

impl VirtualPacketSummary {
    pub fn validate(&self) -> Result<(), VirtualPsbtError> {
        if self.version > 1 {
            return Err(VirtualPsbtError::UnsupportedVersion(self.version));
        }

        if self.input_count == 0 {
            return Err(VirtualPsbtError::NoInputs);
        }

        if self.output_count == 0 {
            return Err(VirtualPsbtError::NoOutputs);
        }

        if self.signing_domain != SigningDomain::TaprootAssets {
            return Err(VirtualPsbtError::WrongSigningDomain);
        }

        Ok(())
    }

    pub fn canonical_summary(&self) -> String {
        format!(
            "tap-vpsbt:v{}:{}:{}:{}:{}",
            self.version,
            self.chain_params_hrp,
            self.input_count,
            self.output_count,
            self.total_output_amount.value()
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SigningDomain {
    TaprootAssets,
    Bitcoin,
}

impl SigningDomain {
    pub fn nonce_context(self) -> &'static str {
        match self {
            Self::TaprootAssets => "tap-ldk:taproot-assets:asset-nonce:v0",
            Self::Bitcoin => "tap-ldk:bitcoin:btc-nonce:v0",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum VirtualPsbtError {
    Asset(AssetError),
    UnsupportedVersion(u8),
    NoInputs,
    NoOutputs,
    AmountOverflow,
    WrongSigningDomain,
}

impl fmt::Display for VirtualPsbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(err) => write!(f, "virtual PSBT asset error: {err}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported virtual PSBT version {version}")
            }
            Self::NoInputs => write!(f, "virtual PSBT must have at least one input"),
            Self::NoOutputs => write!(f, "virtual PSBT must have at least one output"),
            Self::AmountOverflow => write!(f, "virtual PSBT output amount overflow"),
            Self::WrongSigningDomain => {
                write!(f, "virtual PSBT must use the Taproot Assets signing domain")
            }
        }
    }
}

impl Error for VirtualPsbtError {}
