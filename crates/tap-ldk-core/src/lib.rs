pub mod tlv;

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
