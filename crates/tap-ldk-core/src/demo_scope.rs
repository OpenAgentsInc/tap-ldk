use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DemoFeaturePolicy {
    pub feature: String,
    pub policy: String,
    pub first_public_demo: bool,
    pub reason: String,
    pub covered_by_issue: String,
    pub reopen_before: Vec<String>,
    pub verification: Vec<String>,
}

impl DemoFeaturePolicy {
    pub fn validate_bolt_base_supported(
        &self,
        expected_feature: &str,
    ) -> Result<(), DemoScopeError> {
        if self.feature != expected_feature {
            return Err(DemoScopeError::UnexpectedFeature {
                expected: expected_feature.to_owned(),
                actual: self.feature.clone(),
            });
        }
        if self.policy != "bolt-base-supported" {
            return Err(DemoScopeError::UnexpectedPolicy {
                feature: self.feature.clone(),
                expected: "bolt-base-supported".to_owned(),
                actual: self.policy.clone(),
            });
        }
        if self.first_public_demo {
            return Err(DemoScopeError::ExcludedFeatureInDemo(self.feature.clone()));
        }
        if self.reason.trim().is_empty() {
            return Err(DemoScopeError::MissingReason(self.feature.clone()));
        }
        if self.reopen_before.is_empty() {
            return Err(DemoScopeError::MissingReopenBoundary(self.feature.clone()));
        }
        if self.verification.is_empty() {
            return Err(DemoScopeError::MissingVerification(self.feature.clone()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FirstDemoProtocolScope {
    pub schema_version: u8,
    pub simple_taproot_splicing: DemoFeaturePolicy,
}

impl FirstDemoProtocolScope {
    pub fn validate(&self) -> Result<(), DemoScopeError> {
        if self.schema_version != 1 {
            return Err(DemoScopeError::UnsupportedSchema(self.schema_version));
        }
        self.simple_taproot_splicing
            .validate_bolt_base_supported("simple-taproot splice nonce maps")
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimpleTaprootNegotiationMode {
    pub name: String,
    pub status: String,
    pub rust_lightning_config: String,
    pub init_feature_bits: String,
    pub channel_type: String,
    pub uses_simple_close: bool,
    pub first_demo_mode: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimpleTaprootNegotiationReport {
    pub schema_version: u8,
    pub active_first_demo_mode: String,
    pub modes: Vec<SimpleTaprootNegotiationMode>,
}

impl SimpleTaprootNegotiationReport {
    pub fn validate(&self) -> Result<(), DemoScopeError> {
        if self.schema_version != 1 {
            return Err(DemoScopeError::UnsupportedSchema(self.schema_version));
        }
        let final_mode = self
            .modes
            .iter()
            .find(|mode| mode.name == "final-bolt-simple-taproot")
            .ok_or(DemoScopeError::MissingNegotiationMode(
                "final-bolt-simple-taproot",
            ))?;
        if final_mode.status != "implemented" {
            return Err(DemoScopeError::UnexpectedPolicy {
                feature: final_mode.name.clone(),
                expected: "implemented".to_owned(),
                actual: final_mode.status.clone(),
            });
        }
        if !final_mode.uses_simple_close {
            return Err(DemoScopeError::MissingSimpleCloseDependency);
        }
        Ok(())
    }
}

pub fn first_demo_protocol_scope() -> FirstDemoProtocolScope {
    FirstDemoProtocolScope {
        schema_version: 1,
        simple_taproot_splicing: DemoFeaturePolicy {
            feature: "simple-taproot splice nonce maps".to_owned(),
            policy: "bolt-base-supported".to_owned(),
            first_public_demo: false,
            reason: "The OpenAgentsInc rust-lightning fork has bounded BOLT simple-taproot type-22 nonce-map coverage for current, pending splice, and RBF funding txids, including missing, empty, duplicate, unknown, scalar-with-multiple-funding, and nonce-reuse failures. The first public Taproot Assets demo still keeps one asset-channel funding outpoint, so asset-channel splice/RBF remains a separate hardening item."
                .to_owned(),
            covered_by_issue: "#92".to_owned(),
            reopen_before: vec![
                "any Taproot Asset channel claim using concurrent splice/RBF candidates".to_owned(),
                "any production-complete simple-taproot claim before #94 closes".to_owned(),
            ],
            verification: vec![
                "cargo test -p lightning final_simple_taproot_uses_nonce_maps --features simple_taproot_musig2 -- --nocapture".to_owned(),
                "cargo test -p lightning simple_taproot --features simple_taproot_musig2 -- --nocapture".to_owned(),
                "cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture".to_owned(),
                "cargo check -p lightning --features simple_taproot_musig2".to_owned(),
            ],
        },
    }
}

pub fn simple_taproot_negotiation_report() -> SimpleTaprootNegotiationReport {
    SimpleTaprootNegotiationReport {
        schema_version: 1,
        active_first_demo_mode: "staging-interop-plus-taproot-assets-overlay".to_owned(),
        modes: vec![
            SimpleTaprootNegotiationMode {
                name: "staging-interop".to_owned(),
                status: "implemented".to_owned(),
                rust_lightning_config: "negotiate_simple_taproot_channels".to_owned(),
                init_feature_bits: "180/181".to_owned(),
                channel_type: "simple_taproot_staging".to_owned(),
                uses_simple_close: false,
                first_demo_mode: false,
                notes: vec![
                    "Kept for draft/staging peers and Lightning Labs compatibility.".to_owned(),
                    "Single-funding RAA/reestablish may use the legacy scalar next-local nonce."
                        .to_owned(),
                ],
            },
            SimpleTaprootNegotiationMode {
                name: "taproot-assets-overlay".to_owned(),
                status: "implemented".to_owned(),
                rust_lightning_config: "negotiate_taproot_asset_channels".to_owned(),
                init_feature_bits: "180/181 plus taproot-overlay-chans".to_owned(),
                channel_type: "taproot_asset_single_asset".to_owned(),
                uses_simple_close: false,
                first_demo_mode: true,
                notes: vec![
                    "This is the current native asset-channel and litd interop demo path."
                        .to_owned(),
                    "It remains separate from final option_simple_taproot production mode."
                        .to_owned(),
                ],
            },
            SimpleTaprootNegotiationMode {
                name: "final-bolt-simple-taproot".to_owned(),
                status: "implemented".to_owned(),
                rust_lightning_config: "negotiate_final_simple_taproot_channels".to_owned(),
                init_feature_bits: "80/81".to_owned(),
                channel_type: "simple_taproot".to_owned(),
                uses_simple_close: true,
                first_demo_mode: false,
                notes: vec![
                    "Advertised only when option_channel_type and option_simple_close are present."
                        .to_owned(),
                    "Uses private explicit channel-type negotiation and type-22 nonce maps for RAA/reestablish."
                        .to_owned(),
                ],
            },
        ],
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DemoScopeError {
    UnsupportedSchema(u8),
    UnexpectedFeature {
        expected: String,
        actual: String,
    },
    UnexpectedPolicy {
        feature: String,
        expected: String,
        actual: String,
    },
    ExcludedFeatureInDemo(String),
    MissingReason(String),
    MissingReopenBoundary(String),
    MissingVerification(String),
    MissingNegotiationMode(&'static str),
    MissingSimpleCloseDependency,
}

impl core::fmt::Display for DemoScopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedSchema(version) => {
                write!(f, "unsupported first-demo scope schema {version}")
            }
            Self::UnexpectedFeature { expected, actual } => {
                write!(f, "expected demo feature {expected}, got {actual}")
            }
            Self::UnexpectedPolicy {
                feature,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "expected demo policy {expected} for {feature}, got {actual}"
                )
            }
            Self::ExcludedFeatureInDemo(feature) => {
                write!(f, "excluded feature {feature} is marked in first demo")
            }
            Self::MissingReason(feature) => {
                write!(f, "demo policy for {feature} is missing a reason")
            }
            Self::MissingReopenBoundary(feature) => {
                write!(f, "demo policy for {feature} is missing reopen boundaries")
            }
            Self::MissingVerification(feature) => {
                write!(f, "demo policy for {feature} is missing verification")
            }
            Self::MissingNegotiationMode(mode) => {
                write!(f, "simple taproot negotiation report is missing {mode}")
            }
            Self::MissingSimpleCloseDependency => {
                write!(
                    f,
                    "final simple taproot negotiation report is missing simple close dependency"
                )
            }
        }
    }
}

impl std::error::Error for DemoScopeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_demo_scope_reports_bolt_base_splice_nonce_map_support() {
        let scope = first_demo_protocol_scope();

        scope.validate().unwrap();
        assert_eq!(
            scope.simple_taproot_splicing.feature,
            "simple-taproot splice nonce maps"
        );
        assert_eq!(scope.simple_taproot_splicing.policy, "bolt-base-supported");
        assert!(!scope.simple_taproot_splicing.first_public_demo);
        assert!(scope.simple_taproot_splicing.reason.contains("type-22"));
        assert_eq!(scope.simple_taproot_splicing.covered_by_issue, "#92");
        assert!(
            scope
                .simple_taproot_splicing
                .reopen_before
                .iter()
                .any(|boundary| boundary.contains("Taproot Asset channel"))
        );
        assert!(
            scope
                .simple_taproot_splicing
                .verification
                .iter()
                .any(|command| command.contains("final_simple_taproot_uses_nonce_maps"))
        );
    }

    #[test]
    fn simple_taproot_negotiation_report_distinguishes_staging_and_final_modes() {
        let report = simple_taproot_negotiation_report();

        report.validate().unwrap();
        assert_eq!(
            report.active_first_demo_mode,
            "staging-interop-plus-taproot-assets-overlay"
        );
        assert!(
            report.modes.iter().any(|mode| {
                mode.name == "staging-interop" && mode.init_feature_bits == "180/181"
            })
        );
        assert!(report.modes.iter().any(|mode| {
            mode.name == "final-bolt-simple-taproot"
                && mode.init_feature_bits == "80/81"
                && mode.uses_simple_close
        }));
    }
}
