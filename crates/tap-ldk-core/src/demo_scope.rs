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
    pub fn validate_excluded(&self, expected_feature: &str) -> Result<(), DemoScopeError> {
        if self.feature != expected_feature {
            return Err(DemoScopeError::UnexpectedFeature {
                expected: expected_feature.to_owned(),
                actual: self.feature.clone(),
            });
        }
        if self.policy != "excluded" {
            return Err(DemoScopeError::UnexpectedPolicy {
                feature: self.feature.clone(),
                expected: "excluded".to_owned(),
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
            .validate_excluded("simple-taproot concurrent splicing")
    }
}

pub fn first_demo_protocol_scope() -> FirstDemoProtocolScope {
    FirstDemoProtocolScope {
        schema_version: 1,
        simple_taproot_splicing: DemoFeaturePolicy {
            feature: "simple-taproot concurrent splicing".to_owned(),
            policy: "excluded".to_owned(),
            first_public_demo: false,
            reason: "The first public demo uses a stable funding outpoint from open through payment, reestablish, cooperative close, and force-close. The OpenAgentsInc rust-lightning fork validates type-22 nonce maps for current and pending funding txids, but it does not yet have bounded simple-taproot splice vectors proving missing, stale, duplicate, or wrong-funding-txid nonce-map entries for concurrent splice candidates."
                .to_owned(),
            covered_by_issue: "#90".to_owned(),
            reopen_before: vec![
                "#61 production/simple-taproot-complete claim".to_owned(),
                "any public demo that splices a simple-taproot channel".to_owned(),
                "any Taproot Asset channel claim using concurrent splice/RBF candidates"
                    .to_owned(),
            ],
            verification: vec![
                "cargo test -p lightning simple_taproot --features simple_taproot_musig2 -- --nocapture".to_owned(),
                "cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture".to_owned(),
                "cargo check -p lightning --features simple_taproot_musig2".to_owned(),
            ],
        },
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
        }
    }
}

impl std::error::Error for DemoScopeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_demo_scope_excludes_concurrent_simple_taproot_splicing() {
        let scope = first_demo_protocol_scope();

        scope.validate().unwrap();
        assert_eq!(
            scope.simple_taproot_splicing.feature,
            "simple-taproot concurrent splicing"
        );
        assert_eq!(scope.simple_taproot_splicing.policy, "excluded");
        assert!(!scope.simple_taproot_splicing.first_public_demo);
        assert!(scope.simple_taproot_splicing.reason.contains("type-22"));
        assert_eq!(scope.simple_taproot_splicing.covered_by_issue, "#90");
        assert!(
            scope
                .simple_taproot_splicing
                .reopen_before
                .iter()
                .any(|boundary| boundary.contains("#61"))
        );
        assert!(
            scope
                .simple_taproot_splicing
                .verification
                .iter()
                .any(|command| command.contains("splic"))
        );
    }
}
