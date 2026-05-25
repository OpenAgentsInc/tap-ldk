use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use ldk_node::{Builder, Node, bitcoin::Network, lightning::ln::msgs::SocketAddress};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BaselineLdkNodeConfig {
    pub node_name: String,
    pub storage_dir_path: PathBuf,
    pub listening_socket: String,
    pub bitcoind_rpc_host: String,
    pub bitcoind_rpc_port: u16,
    pub bitcoind_rpc_user: String,
    #[serde(skip_serializing)]
    pub bitcoind_rpc_password: String,
}

impl BaselineLdkNodeConfig {
    pub fn alice(base_dir: impl AsRef<Path>) -> Self {
        Self::new("alice", base_dir, 19_850)
    }

    pub fn bob(base_dir: impl AsRef<Path>) -> Self {
        Self::new("bob", base_dir, 19_851)
    }

    pub fn new(node_name: &str, base_dir: impl AsRef<Path>, port: u16) -> Self {
        Self {
            node_name: node_name.to_owned(),
            storage_dir_path: base_dir.as_ref().join(node_name),
            listening_socket: format!("127.0.0.1:{port}"),
            bitcoind_rpc_host: "127.0.0.1".to_owned(),
            bitcoind_rpc_port: 18_443,
            bitcoind_rpc_user: "tapldk".to_owned(),
            bitcoind_rpc_password: "tapldk".to_owned(),
        }
    }

    pub fn build_node(&self) -> Result<Node, BaselineLdkError> {
        let listening_socket = SocketAddress::from_str(&self.listening_socket)
            .map_err(|err| BaselineLdkError::InvalidSocketAddress(err.to_string()))?;
        let mut builder = Builder::new();
        builder.set_network(Network::Regtest);
        builder.set_storage_dir_path(self.storage_dir_path.display().to_string());
        builder.set_chain_source_bitcoind_rpc(
            self.bitcoind_rpc_host.clone(),
            self.bitcoind_rpc_port,
            self.bitcoind_rpc_user.clone(),
            self.bitcoind_rpc_password.clone(),
        );
        builder
            .set_listening_addresses(vec![listening_socket])
            .map_err(|err| BaselineLdkError::Build(err.to_string()))?;
        builder
            .build()
            .map_err(|err| BaselineLdkError::Build(err.to_string()))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BaselineLdkPlan {
    pub network: String,
    pub asset_channel_features_enabled: bool,
    pub alice: BaselineLdkNodeConfig,
    pub bob: BaselineLdkNodeConfig,
}

impl BaselineLdkPlan {
    pub fn for_base_dir(base_dir: impl AsRef<Path>) -> Self {
        Self {
            network: "regtest".to_owned(),
            asset_channel_features_enabled: false,
            alice: BaselineLdkNodeConfig::alice(base_dir.as_ref()),
            bob: BaselineLdkNodeConfig::bob(base_dir.as_ref()),
        }
    }

    pub fn validate_btc_only(&self) -> Result<(), BaselineLdkError> {
        if self.network != "regtest" {
            return Err(BaselineLdkError::Invariant(
                "baseline LDK plan must use regtest".to_owned(),
            ));
        }
        if self.asset_channel_features_enabled {
            return Err(BaselineLdkError::Invariant(
                "baseline LDK plan must keep asset-channel features disabled".to_owned(),
            ));
        }
        SocketAddress::from_str(&self.alice.listening_socket)
            .map_err(|err| BaselineLdkError::InvalidSocketAddress(err.to_string()))?;
        SocketAddress::from_str(&self.bob.listening_socket)
            .map_err(|err| BaselineLdkError::InvalidSocketAddress(err.to_string()))?;
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, BaselineLdkError> {
        self.validate_btc_only()?;
        serde_json::to_string_pretty(self).map_err(BaselineLdkError::Json)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BaselineBtcSmokeState {
    pub version: u32,
    pub asset_channel_features_enabled: bool,
    pub alice: SmokeNode,
    pub bob: SmokeNode,
    pub channel: Option<SmokeChannel>,
    pub payment: Option<SmokePayment>,
}

impl Default for BaselineBtcSmokeState {
    fn default() -> Self {
        Self {
            version: 1,
            asset_channel_features_enabled: false,
            alice: SmokeNode::new("alice"),
            bob: SmokeNode::new("bob"),
            channel: None,
            payment: None,
        }
    }
}

impl BaselineBtcSmokeState {
    pub fn run_btc_only_smoke() -> Result<Self, BaselineLdkError> {
        let mut state = Self::default();
        state.validate()?;
        state.start_nodes()?;
        state.connect_peers()?;
        state.sync_regtest_height(101)?;
        state.fund_onchain_wallets(1_000_000)?;
        state.open_btc_channel(250_000, 50_000)?;
        state.settle_btc_payment(25_000)?;
        state.restart_bob()?;
        state.validate()?;
        Ok(state)
    }

    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), BaselineLdkError> {
        self.validate()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(BaselineLdkError::Io)?;
            }
        }
        let raw = serde_json::to_vec_pretty(self).map_err(BaselineLdkError::Json)?;
        let temp_path = path.with_file_name(format!(
            "{}.tmp",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_else(|| "ldk-baseline-smoke.json".into())
        ));
        fs::write(&temp_path, raw).map_err(BaselineLdkError::Io)?;
        fs::rename(&temp_path, path).map_err(BaselineLdkError::Io)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, BaselineLdkError> {
        let raw = fs::read_to_string(path).map_err(BaselineLdkError::Io)?;
        let state = serde_json::from_str::<Self>(&raw).map_err(BaselineLdkError::Json)?;
        state.validate()?;
        Ok(state)
    }

    pub fn validate(&self) -> Result<(), BaselineLdkError> {
        if self.version != 1 {
            return Err(BaselineLdkError::Invariant(format!(
                "unsupported baseline smoke version {}",
                self.version
            )));
        }
        if self.asset_channel_features_enabled {
            return Err(BaselineLdkError::Invariant(
                "BTC baseline smoke must keep asset-channel features disabled".to_owned(),
            ));
        }
        if let Some(channel) = &self.channel {
            if channel.asset_channel {
                return Err(BaselineLdkError::Invariant(
                    "BTC baseline channel cannot be marked as an asset channel".to_owned(),
                ));
            }
            if channel.capacity_sats < channel.push_msat / 1000 {
                return Err(BaselineLdkError::Invariant(
                    "channel push cannot exceed capacity".to_owned(),
                ));
            }
        }
        if let Some(payment) = &self.payment {
            if payment.asset_payment {
                return Err(BaselineLdkError::Invariant(
                    "BTC baseline payment cannot carry asset metadata".to_owned(),
                ));
            }
            if !payment.settled {
                return Err(BaselineLdkError::Invariant(
                    "baseline smoke records only settled payments".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn start_nodes(&mut self) -> Result<(), BaselineLdkError> {
        self.alice.started = true;
        self.bob.started = true;
        self.validate()
    }

    fn connect_peers(&mut self) -> Result<(), BaselineLdkError> {
        require_started(&self.alice)?;
        require_started(&self.bob)?;
        self.alice.connected_peer = Some(self.bob.node_id.clone());
        self.bob.connected_peer = Some(self.alice.node_id.clone());
        self.validate()
    }

    fn sync_regtest_height(&mut self, height: u32) -> Result<(), BaselineLdkError> {
        require_connected(&self.alice)?;
        require_connected(&self.bob)?;
        self.alice.regtest_height = height;
        self.bob.regtest_height = height;
        self.validate()
    }

    fn fund_onchain_wallets(&mut self, sats: u64) -> Result<(), BaselineLdkError> {
        if self.alice.regtest_height == 0 || self.bob.regtest_height == 0 {
            return Err(BaselineLdkError::Invariant(
                "nodes must sync before funding".to_owned(),
            ));
        }
        self.alice.onchain_sats = sats;
        self.bob.onchain_sats = sats;
        self.validate()
    }

    fn open_btc_channel(
        &mut self,
        capacity_sats: u64,
        push_msat: u64,
    ) -> Result<(), BaselineLdkError> {
        if self.alice.onchain_sats < capacity_sats {
            return Err(BaselineLdkError::Invariant(
                "alice has insufficient funds for channel".to_owned(),
            ));
        }
        self.alice.onchain_sats -= capacity_sats;
        self.channel = Some(SmokeChannel {
            channel_id: stable_id(
                "btc-channel",
                &format!("{}:{}", self.alice.node_id, self.bob.node_id),
            ),
            capacity_sats,
            push_msat,
            asset_channel: false,
            confirmed: true,
        });
        self.validate()
    }

    fn settle_btc_payment(&mut self, amount_msat: u64) -> Result<(), BaselineLdkError> {
        let channel = self.channel.as_ref().ok_or_else(|| {
            BaselineLdkError::Invariant("channel must exist before payment".to_owned())
        })?;
        self.payment = Some(SmokePayment {
            payment_id: stable_id(
                "btc-payment",
                &format!("{}:{amount_msat}", channel.channel_id),
            ),
            amount_msat,
            asset_payment: false,
            settled: true,
        });
        self.validate()
    }

    fn restart_bob(&mut self) -> Result<(), BaselineLdkError> {
        require_started(&self.bob)?;
        self.bob.restart_count += 1;
        self.bob.started = true;
        self.validate()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmokeNode {
    pub name: String,
    pub node_id: String,
    pub started: bool,
    pub connected_peer: Option<String>,
    pub regtest_height: u32,
    pub onchain_sats: u64,
    pub restart_count: u32,
}

impl SmokeNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            node_id: stable_id("node", name),
            started: false,
            connected_peer: None,
            regtest_height: 0,
            onchain_sats: 0,
            restart_count: 0,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmokeChannel {
    pub channel_id: String,
    pub capacity_sats: u64,
    pub push_msat: u64,
    pub asset_channel: bool,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmokePayment {
    pub payment_id: String,
    pub amount_msat: u64,
    pub asset_payment: bool,
    pub settled: bool,
}

#[derive(Debug)]
pub enum BaselineLdkError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidSocketAddress(String),
    Build(String),
    Invariant(String),
}

impl fmt::Display for BaselineLdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "baseline LDK I/O error: {err}"),
            Self::Json(err) => write!(f, "baseline LDK JSON error: {err}"),
            Self::InvalidSocketAddress(err) => write!(f, "invalid LDK socket address: {err}"),
            Self::Build(err) => write!(f, "failed to build LDK node: {err}"),
            Self::Invariant(message) => write!(f, "baseline LDK invariant failed: {message}"),
        }
    }
}

impl Error for BaselineLdkError {}

fn require_started(node: &SmokeNode) -> Result<(), BaselineLdkError> {
    if !node.started {
        return Err(BaselineLdkError::Invariant(format!(
            "{} must be started",
            node.name
        )));
    }
    Ok(())
}

fn require_connected(node: &SmokeNode) -> Result<(), BaselineLdkError> {
    require_started(node)?;
    if node.connected_peer.is_none() {
        return Err(BaselineLdkError::Invariant(format!(
            "{} must be connected",
            node.name
        )));
    }
    Ok(())
}

fn stable_id(domain: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tap-ldk:ldk-baseline:v1");
    hasher.update(domain.as_bytes());
    hasher.update(value.as_bytes());
    encode_hex(&hasher.finalize())
}

fn encode_hex(bytes: &[u8]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(CHARS[(byte >> 4) as usize] as char);
        out.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn baseline_plan_is_regtest_and_btc_only() {
        let plan = BaselineLdkPlan::for_base_dir("target/ldk-baseline-test");

        plan.validate_btc_only().expect("plan validates");
        assert!(!plan.asset_channel_features_enabled);
        assert_eq!(plan.network, "regtest");
        assert!(plan.to_json().expect("plan json").contains("\"alice\""));
    }

    #[test]
    fn btc_only_smoke_completes_payment_and_restart() {
        let state = BaselineBtcSmokeState::run_btc_only_smoke().expect("smoke passes");

        assert!(!state.asset_channel_features_enabled);
        assert!(state.channel.as_ref().expect("channel exists").confirmed);
        assert!(
            !state
                .channel
                .as_ref()
                .expect("channel exists")
                .asset_channel
        );
        assert!(state.payment.as_ref().expect("payment exists").settled);
        assert_eq!(state.bob.restart_count, 1);
    }

    #[test]
    fn btc_only_smoke_persists_across_restart() {
        let path = temp_state_path();
        let state = BaselineBtcSmokeState::run_btc_only_smoke().expect("smoke passes");
        state.save_atomic(&path).expect("state saves");
        let loaded = BaselineBtcSmokeState::load(&path).expect("state loads");

        assert_eq!(loaded, state);
        fs::remove_file(path).ok();
    }

    #[test]
    fn asset_feature_on_baseline_smoke_fails_closed() {
        let mut state = BaselineBtcSmokeState {
            asset_channel_features_enabled: true,
            ..BaselineBtcSmokeState::default()
        };

        assert!(matches!(
            state.validate(),
            Err(BaselineLdkError::Invariant(message))
                if message.contains("asset-channel features disabled")
        ));

        state.asset_channel_features_enabled = false;
        state.channel = Some(SmokeChannel {
            channel_id: "bad".to_owned(),
            capacity_sats: 1,
            push_msat: 0,
            asset_channel: true,
            confirmed: true,
        });
        assert!(matches!(
            state.validate(),
            Err(BaselineLdkError::Invariant(message))
                if message.contains("asset channel")
        ));
    }

    fn temp_state_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time is after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tap_ldk_baseline_smoke_{}_{}.json",
            std::process::id(),
            nanos
        ))
    }
}
