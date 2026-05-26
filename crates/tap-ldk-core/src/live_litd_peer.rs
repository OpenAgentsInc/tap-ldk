use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use ldk_node::{
    Builder, Node,
    bitcoin::{Network, secp256k1::PublicKey},
    lightning::ln::msgs::SocketAddress,
};
use serde::{Deserialize, Serialize};

use crate::ldk_fork::OPENAGENTS_RUST_LIGHTNING_REV;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLitdPeerPreflightRequest {
    pub storage_dir_path: PathBuf,
    pub listening_socket: String,
    pub bitcoind_rpc_host: String,
    pub bitcoind_rpc_port: u16,
    pub bitcoind_rpc_user: String,
    #[serde(skip_serializing)]
    pub bitcoind_rpc_password: String,
    pub litd_node_id: String,
    pub litd_p2p_address: String,
}

impl LiveLitdPeerPreflightRequest {
    pub fn new(
        storage_dir_path: impl AsRef<Path>,
        litd_node_id: impl Into<String>,
        litd_p2p_address: impl Into<String>,
    ) -> Self {
        Self {
            storage_dir_path: storage_dir_path.as_ref().to_path_buf(),
            listening_socket: "127.0.0.1:19860".to_owned(),
            bitcoind_rpc_host: "127.0.0.1".to_owned(),
            bitcoind_rpc_port: 18_443,
            bitcoind_rpc_user: "tapldk".to_owned(),
            bitcoind_rpc_password: "tapldk-regtest".to_owned(),
            litd_node_id: litd_node_id.into(),
            litd_p2p_address: litd_p2p_address.into(),
        }
    }

    pub fn validate(&self) -> Result<ValidatedLiveLitdPeerPreflightRequest, LiveLitdPeerError> {
        if self.storage_dir_path.as_os_str().is_empty() {
            return Err(LiveLitdPeerError::InvalidRequest(
                "storage directory cannot be empty".to_owned(),
            ));
        }
        if self.bitcoind_rpc_host.trim().is_empty()
            || self.bitcoind_rpc_user.trim().is_empty()
            || self.bitcoind_rpc_password.trim().is_empty()
        {
            return Err(LiveLitdPeerError::InvalidRequest(
                "bitcoind RPC host, user, and password must be present".to_owned(),
            ));
        }
        let listening_socket = SocketAddress::from_str(&self.listening_socket)
            .map_err(|err| LiveLitdPeerError::InvalidSocketAddress(err.to_string()))?;
        let litd_node_id = PublicKey::from_str(&self.litd_node_id)
            .map_err(|err| LiveLitdPeerError::InvalidNodeId(err.to_string()))?;
        let litd_p2p_address = SocketAddress::from_str(&self.litd_p2p_address)
            .map_err(|err| LiveLitdPeerError::InvalidSocketAddress(err.to_string()))?;

        Ok(ValidatedLiveLitdPeerPreflightRequest {
            storage_dir_path: self.storage_dir_path.clone(),
            listening_socket,
            bitcoind_rpc_host: self.bitcoind_rpc_host.clone(),
            bitcoind_rpc_port: self.bitcoind_rpc_port,
            bitcoind_rpc_user: self.bitcoind_rpc_user.clone(),
            bitcoind_rpc_password: self.bitcoind_rpc_password.clone(),
            litd_node_id,
            litd_p2p_address,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedLiveLitdPeerPreflightRequest {
    pub storage_dir_path: PathBuf,
    pub listening_socket: SocketAddress,
    pub bitcoind_rpc_host: String,
    pub bitcoind_rpc_port: u16,
    pub bitcoind_rpc_user: String,
    pub bitcoind_rpc_password: String,
    pub litd_node_id: PublicKey,
    pub litd_p2p_address: SocketAddress,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLitdPeerPreflightReport {
    pub status: String,
    pub network: String,
    pub storage_dir_path: String,
    pub live_node_runtime: String,
    pub live_node_uses_openagents_rust_lightning_fork: bool,
    pub openagents_rust_lightning_rev: String,
    pub fork_asset_channel_hooks_reachable_from_live_node: bool,
    pub native_node_id: String,
    pub native_listening_socket: String,
    pub litd_node_id: String,
    pub litd_p2p_address: String,
    pub native_node_started: bool,
    pub peer_connected: bool,
    pub peer_persisted: bool,
    pub known_peer_count: usize,
    pub asset_channel_settlement_ready: bool,
    pub remaining_asset_channel_gap: String,
}

pub fn run_live_litd_peer_preflight(
    request: LiveLitdPeerPreflightRequest,
) -> Result<LiveLitdPeerPreflightReport, LiveLitdPeerError> {
    let request = request.validate()?;
    let node = build_node(&request)?;
    node.start()
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;

    let native_node_id = node.node_id();
    let connect_result = node.connect(
        request.litd_node_id,
        request.litd_p2p_address.clone(),
        false,
    );
    thread::sleep(Duration::from_millis(500));

    let peer_details = node.list_peers();
    let matched_peer = peer_details
        .iter()
        .find(|peer| peer.node_id == request.litd_node_id);
    let peer_connected = matched_peer.map(|peer| peer.is_connected).unwrap_or(false);
    let peer_persisted = matched_peer.map(|peer| peer.is_persisted).unwrap_or(false);

    let stop_result = node.stop();
    if let Err(err) = stop_result {
        return Err(LiveLitdPeerError::Node(err.to_string()));
    }
    connect_result.map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;

    if !peer_connected {
        return Err(LiveLitdPeerError::PeerNotConnected);
    }

    Ok(LiveLitdPeerPreflightReport {
        status: "connected".to_owned(),
        network: "regtest".to_owned(),
        storage_dir_path: request.storage_dir_path.display().to_string(),
        live_node_runtime: "ldk-node 0.7.0".to_owned(),
        live_node_uses_openagents_rust_lightning_fork: false,
        openagents_rust_lightning_rev: OPENAGENTS_RUST_LIGHTNING_REV.to_owned(),
        fork_asset_channel_hooks_reachable_from_live_node: false,
        native_node_id: native_node_id.to_string(),
        native_listening_socket: request.listening_socket.to_string(),
        litd_node_id: request.litd_node_id.to_string(),
        litd_p2p_address: request.litd_p2p_address.to_string(),
        native_node_started: true,
        peer_connected,
        peer_persisted,
        known_peer_count: peer_details.len(),
        asset_channel_settlement_ready: false,
        remaining_asset_channel_gap: "Native LDK can connect to the independent litd peer, but this preflight uses ldk-node's upstream Lightning runtime. #57 still needs a live node built directly on the OpenAgentsInc rust-lightning fork, or an ldk-node patch that exposes that fork's simple-taproot and Taproot Asset channel-manager surfaces, before asset-channel funding/payment can settle."
            .to_owned(),
    })
}

fn build_node(request: &ValidatedLiveLitdPeerPreflightRequest) -> Result<Node, LiveLitdPeerError> {
    let mut builder = Builder::new();
    builder.set_network(Network::Regtest);
    builder.set_storage_dir_path(request.storage_dir_path.display().to_string());
    builder.set_chain_source_bitcoind_rpc(
        request.bitcoind_rpc_host.clone(),
        request.bitcoind_rpc_port,
        request.bitcoind_rpc_user.clone(),
        request.bitcoind_rpc_password.clone(),
    );
    builder
        .set_listening_addresses(vec![request.listening_socket.clone()])
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;
    builder
        .build()
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))
}

#[derive(Debug)]
pub enum LiveLitdPeerError {
    InvalidRequest(String),
    InvalidNodeId(String),
    InvalidSocketAddress(String),
    Node(String),
    PeerNotConnected,
}

impl fmt::Display for LiveLitdPeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => {
                write!(f, "invalid live litd peer request: {message}")
            }
            Self::InvalidNodeId(err) => write!(f, "invalid litd node id: {err}"),
            Self::InvalidSocketAddress(err) => {
                write!(f, "invalid live litd peer socket address: {err}")
            }
            Self::Node(err) => write!(f, "live litd peer node error: {err}"),
            Self::PeerNotConnected => write!(f, "native LDK node did not connect to litd peer"),
        }
    }
}

impl Error for LiveLitdPeerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validates_pubkey_and_address() {
        let request = LiveLitdPeerPreflightRequest::new(
            "target/live-litd-peer-test",
            "034c68b1fbc81995a97d5710e6dd6a5dd50f866a1b3a5a86a13828921d681f98dd",
            "127.0.0.1:29735",
        );

        let validated = request.validate().expect("request validates");

        assert_eq!(
            validated.litd_node_id.to_string(),
            "034c68b1fbc81995a97d5710e6dd6a5dd50f866a1b3a5a86a13828921d681f98dd"
        );
        assert_eq!(validated.litd_p2p_address.to_string(), "127.0.0.1:29735");
    }

    #[test]
    fn request_rejects_invalid_pubkey() {
        let request = LiveLitdPeerPreflightRequest::new(
            "target/live-litd-peer-test",
            "not-a-key",
            "127.0.0.1:29735",
        );

        assert!(matches!(
            request.validate(),
            Err(LiveLitdPeerError::InvalidNodeId(_))
        ));
    }
}
