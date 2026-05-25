use std::{error::Error, fmt};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BitcoinRegtestConfig {
    pub image: String,
    pub container_name: String,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_user: String,
    pub rpc_password: String,
    pub network: String,
}

impl Default for BitcoinRegtestConfig {
    fn default() -> Self {
        Self {
            image: "bitcoin/bitcoin:30.0".to_owned(),
            container_name: "tap-ldk-bitcoin-regtest".to_owned(),
            rpc_host: "127.0.0.1".to_owned(),
            rpc_port: 18443,
            rpc_user: "tapldk".to_owned(),
            rpc_password: "tapldk-regtest".to_owned(),
            network: "regtest".to_owned(),
        }
    }
}

impl BitcoinRegtestConfig {
    pub fn rpc_url(&self) -> String {
        format!("http://{}:{}", self.rpc_host, self.rpc_port)
    }

    pub fn validate(&self) -> Result<(), RegtestConfigError> {
        if self.image.trim().is_empty() {
            return Err(RegtestConfigError::EmptyField("image"));
        }
        if self.container_name.trim().is_empty() {
            return Err(RegtestConfigError::EmptyField("container_name"));
        }
        if self.rpc_host.trim().is_empty() {
            return Err(RegtestConfigError::EmptyField("rpc_host"));
        }
        if self.rpc_port == 0 {
            return Err(RegtestConfigError::InvalidRpcPort);
        }
        if self.rpc_user.trim().is_empty() {
            return Err(RegtestConfigError::EmptyField("rpc_user"));
        }
        if self.rpc_password.trim().is_empty() {
            return Err(RegtestConfigError::EmptyField("rpc_password"));
        }
        if self.network != "regtest" {
            return Err(RegtestConfigError::UnsupportedNetwork(self.network.clone()));
        }

        Ok(())
    }

    pub fn connection_material_json(&self) -> Result<String, RegtestConfigError> {
        self.validate()?;

        Ok(format!(
            concat!(
                "{{\n",
                "  \"network\": \"{}\",\n",
                "  \"rpc_url\": \"{}\",\n",
                "  \"rpc_user\": \"{}\",\n",
                "  \"rpc_password\": \"{}\",\n",
                "  \"container_name\": \"{}\",\n",
                "  \"image\": \"{}\"\n",
                "}}"
            ),
            self.network,
            self.rpc_url(),
            self.rpc_user,
            self.rpc_password,
            self.container_name,
            self.image
        ))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RegtestConfigError {
    EmptyField(&'static str),
    InvalidRpcPort,
    UnsupportedNetwork(String),
}

impl fmt::Display for RegtestConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "missing regtest config field {field}"),
            Self::InvalidRpcPort => write!(f, "regtest RPC port cannot be zero"),
            Self::UnsupportedNetwork(network) => {
                write!(f, "unsupported regtest network {network}")
            }
        }
    }
}

impl Error for RegtestConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid_and_stable() {
        let config = BitcoinRegtestConfig::default();

        config.validate().expect("default config validates");
        assert_eq!(config.rpc_url(), "http://127.0.0.1:18443");
        assert!(
            config
                .connection_material_json()
                .unwrap()
                .contains("regtest")
        );
    }

    #[test]
    fn rejects_non_regtest_network() {
        let config = BitcoinRegtestConfig {
            network: "mainnet".to_owned(),
            ..BitcoinRegtestConfig::default()
        };

        assert_eq!(
            config.validate(),
            Err(RegtestConfigError::UnsupportedNetwork("mainnet".to_owned()))
        );
    }
}
