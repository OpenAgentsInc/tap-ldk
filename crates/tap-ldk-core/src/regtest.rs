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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LightningLabsCounterpartyConfig {
    pub network_name: String,
    pub bitcoind_image: String,
    pub lnd_image: String,
    pub tapd_image: String,
    pub bitcoind_container: String,
    pub lnd_container: String,
    pub tapd_container: String,
    pub bitcoin_rpc_port: u16,
    pub lnd_p2p_port: u16,
    pub lnd_grpc_port: u16,
    pub lnd_rest_port: u16,
    pub tapd_grpc_port: u16,
    pub tapd_rest_port: u16,
    pub rpc_user: String,
    pub rpc_password: String,
}

impl Default for LightningLabsCounterpartyConfig {
    fn default() -> Self {
        Self {
            network_name: "tap-ldk-ll-regtest".to_owned(),
            bitcoind_image: "polarlightning/bitcoind:30.0".to_owned(),
            lnd_image: "polarlightning/lnd:0.19.0-beta".to_owned(),
            tapd_image: "polarlightning/tapd:0.7.0-alpha".to_owned(),
            bitcoind_container: "tap-ldk-ll-bitcoind".to_owned(),
            lnd_container: "tap-ldk-ll-lnd".to_owned(),
            tapd_container: "tap-ldk-ll-tapd".to_owned(),
            bitcoin_rpc_port: 18443,
            lnd_p2p_port: 19735,
            lnd_grpc_port: 10009,
            lnd_rest_port: 18080,
            tapd_grpc_port: 10029,
            tapd_rest_port: 18089,
            rpc_user: "tapldk".to_owned(),
            rpc_password: "tapldk-regtest".to_owned(),
        }
    }
}

impl LightningLabsCounterpartyConfig {
    pub fn validate(&self) -> Result<(), RegtestConfigError> {
        for (field, value) in [
            ("network_name", self.network_name.as_str()),
            ("bitcoind_image", self.bitcoind_image.as_str()),
            ("lnd_image", self.lnd_image.as_str()),
            ("tapd_image", self.tapd_image.as_str()),
            ("bitcoind_container", self.bitcoind_container.as_str()),
            ("lnd_container", self.lnd_container.as_str()),
            ("tapd_container", self.tapd_container.as_str()),
            ("rpc_user", self.rpc_user.as_str()),
            ("rpc_password", self.rpc_password.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(RegtestConfigError::EmptyField(field));
            }
        }

        for (field, port) in [
            ("bitcoin_rpc_port", self.bitcoin_rpc_port),
            ("lnd_p2p_port", self.lnd_p2p_port),
            ("lnd_grpc_port", self.lnd_grpc_port),
            ("lnd_rest_port", self.lnd_rest_port),
            ("tapd_grpc_port", self.tapd_grpc_port),
            ("tapd_rest_port", self.tapd_rest_port),
        ] {
            if port == 0 {
                return Err(RegtestConfigError::InvalidPort(field));
            }
        }

        Ok(())
    }

    pub fn connection_material_json(&self) -> Result<String, RegtestConfigError> {
        self.validate()?;

        Ok(format!(
            concat!(
                "{{\n",
                "  \"network\": \"regtest\",\n",
                "  \"docker_network\": \"{}\",\n",
                "  \"bitcoind\": {{\n",
                "    \"container_name\": \"{}\",\n",
                "    \"image\": \"{}\",\n",
                "    \"rpc_url\": \"http://127.0.0.1:{}\"\n",
                "  }},\n",
                "  \"lnd\": {{\n",
                "    \"container_name\": \"{}\",\n",
                "    \"image\": \"{}\",\n",
                "    \"p2p_url\": \"127.0.0.1:{}\",\n",
                "    \"grpc_url\": \"127.0.0.1:{}\",\n",
                "    \"rest_url\": \"https://127.0.0.1:{}\"\n",
                "  }},\n",
                "  \"tapd\": {{\n",
                "    \"container_name\": \"{}\",\n",
                "    \"image\": \"{}\",\n",
                "    \"grpc_url\": \"127.0.0.1:{}\",\n",
                "    \"rest_url\": \"https://127.0.0.1:{}\"\n",
                "  }}\n",
                "}}"
            ),
            self.network_name,
            self.bitcoind_container,
            self.bitcoind_image,
            self.bitcoin_rpc_port,
            self.lnd_container,
            self.lnd_image,
            self.lnd_p2p_port,
            self.lnd_grpc_port,
            self.lnd_rest_port,
            self.tapd_container,
            self.tapd_image,
            self.tapd_grpc_port,
            self.tapd_rest_port
        ))
    }
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
    InvalidPort(&'static str),
    UnsupportedNetwork(String),
}

impl fmt::Display for RegtestConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "missing regtest config field {field}"),
            Self::InvalidRpcPort => write!(f, "regtest RPC port cannot be zero"),
            Self::InvalidPort(field) => write!(f, "regtest port {field} cannot be zero"),
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

    #[test]
    fn lightning_labs_counterparty_config_is_valid_and_stable() {
        let config = LightningLabsCounterpartyConfig::default();

        config.validate().expect("default config validates");
        let json = config.connection_material_json().unwrap();
        assert!(json.contains("polarlightning/lnd:0.19.0-beta"));
        assert!(json.contains("polarlightning/tapd:0.7.0-alpha"));
    }
}
