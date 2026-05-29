use std::{
    env,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ldk_node::{
    Builder, ChannelDetails, Node,
    bitcoin::{
        Network, Txid,
        secp256k1::{PublicKey, Secp256k1, SecretKey},
    },
    config::ExperimentalChannelConfig,
    entropy::NodeEntropy,
    lightning::{chain::transaction::OutPoint as LdkOutPoint, ln::msgs::SocketAddress},
    logger::LogLevel,
    provenance::{RuntimeProvenance, runtime_provenance},
    taproot_asset::{
        TaprootAssetChannelOpenRequest, TaprootAssetChannelStatus, TaprootAssetMessageKind,
        TaprootAssetMonitorAuxRequest, TaprootAssetPaymentDirection, TaprootAssetPaymentMetadata,
        TaprootAssetPaymentRequest, TaprootAssetPaymentStatus, encode_taproot_asset_htlc_blob,
    },
};
use serde::{Deserialize, Serialize};

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
    pub live_node_simple_taproot_channels_enabled: bool,
    pub live_node_taproot_asset_channels_enabled: bool,
    pub live_node_asset_custom_message_api_ready: bool,
    pub live_node_asset_channel_open_api_ready: bool,
    pub live_node_asset_payment_api_ready: bool,
    pub live_node_asset_runtime_event_count: usize,
    pub live_node_lightning_channel_count: usize,
    pub live_node_asset_channel_count: usize,
    pub live_node_asset_payment_count: usize,
    pub live_node_lightning_channels: Vec<LiveLightningChannelSnapshot>,
    pub live_node_asset_channels: Vec<TaprootAssetChannelStatus>,
    pub live_node_asset_payments: Vec<TaprootAssetPaymentStatus>,
    pub openagents_rust_lightning_rev: String,
    pub fork_asset_channel_hooks_reachable_from_live_node: bool,
    pub native_node_id: String,
    pub native_listening_socket: String,
    pub litd_node_id: String,
    pub litd_p2p_address: String,
    pub native_node_started: bool,
    pub peer_connected: bool,
    pub peer_persisted: bool,
    pub litd_peer_supports_simple_taproot_staging: bool,
    pub litd_peer_supports_taproot_asset_channel: bool,
    pub known_peer_count: usize,
    pub asset_channel_settlement_ready: bool,
    pub native_to_litd_asset_payment: Option<LiveLitdNativeAssetSendReport>,
    pub remaining_asset_channel_gap: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLitdPeerRestartSnapshotReport {
    pub status: String,
    pub network: String,
    pub storage_dir_path: String,
    pub native_node_id: String,
    pub expected_payment_id: String,
    pub expected_asset_id: [u8; 32],
    pub expected_asset_amount: u64,
    pub live_node_asset_channel_count: usize,
    pub live_node_asset_payment_count: usize,
    pub live_node_asset_channels: Vec<TaprootAssetChannelStatus>,
    pub live_node_asset_payments: Vec<TaprootAssetPaymentStatus>,
    pub matched_payment: Option<TaprootAssetPaymentStatus>,
    pub matched_channel: Option<TaprootAssetChannelStatus>,
    pub received_asset_payment_persisted: bool,
    pub received_asset_balance_persisted: bool,
    pub restart_state_matches: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLitdNativeAssetSendRequest {
    pub asset_id: [u8; 32],
    pub asset_amount: u64,
    pub amount_msat: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLitdNativeAssetSendReport {
    pub attempted: bool,
    pub status: String,
    pub payment_id: Option<String>,
    pub channel_id: Option<String>,
    pub lightning_channel_id: Option<String>,
    pub asset_id: [u8; 32],
    pub asset_amount: u64,
    pub amount_msat: u64,
    pub lightning_outbound_capacity_msat_before: Option<u64>,
    pub lightning_next_outbound_htlc_limit_msat_before: Option<u64>,
    pub lightning_next_outbound_htlc_minimum_msat_before: Option<u64>,
    pub local_balance_before: Option<u64>,
    pub remote_balance_before: Option<u64>,
    pub local_balance_after: Option<u64>,
    pub remote_balance_after: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveLightningChannelSnapshot {
    pub channel_id: String,
    pub counterparty_node_id: String,
    pub channel_value_sats: u64,
    pub outbound_capacity_msat: u64,
    pub inbound_capacity_msat: u64,
    pub next_outbound_htlc_limit_msat: u64,
    pub next_outbound_htlc_minimum_msat: u64,
    pub is_outbound: bool,
    pub is_channel_ready: bool,
    pub is_usable: bool,
    pub confirmations: Option<u32>,
    pub unspendable_punishment_reserve_sats: Option<u64>,
    pub counterparty_unspendable_punishment_reserve_sats: u64,
}

pub fn run_live_litd_peer_preflight(
    request: LiveLitdPeerPreflightRequest,
) -> Result<LiveLitdPeerPreflightReport, LiveLitdPeerError> {
    let request = request.validate()?;
    let provenance = runtime_provenance();
    let node = build_node(&request)?;
    let experimental_channel_config = node.experimental_channel_config();
    let asset_runtime_probe = run_asset_runtime_probe(&node)?;
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
    let litd_peer_supports_simple_taproot_staging = matched_peer
        .map(|peer| peer.supports_simple_taproot_staging)
        .unwrap_or(false);
    let litd_peer_supports_taproot_asset_channel = matched_peer
        .map(|peer| peer.supports_taproot_asset_channel)
        .unwrap_or(false);

    let stop_result = node.stop();
    if let Err(err) = stop_result {
        return Err(LiveLitdPeerError::Node(err.to_string()));
    }
    connect_result.map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;

    if !peer_connected {
        return Err(LiveLitdPeerError::PeerNotConnected);
    }

    Ok(build_report(
        &request,
        provenance,
        experimental_channel_config,
        asset_runtime_probe,
        asset_runtime_snapshot(&node),
        native_node_id,
        peer_connected,
        peer_persisted,
        litd_peer_supports_simple_taproot_staging,
        litd_peer_supports_taproot_asset_channel,
        peer_details.len(),
        None,
        "connected",
    ))
}

pub fn run_live_litd_peer_hold(
    request: LiveLitdPeerPreflightRequest,
    report_path: impl AsRef<Path>,
    hold_seconds: u64,
    native_to_litd_send: Option<LiveLitdNativeAssetSendRequest>,
) -> Result<LiveLitdPeerPreflightReport, LiveLitdPeerError> {
    let report_path = report_path.as_ref().to_path_buf();
    let request = request.validate()?;
    let provenance = runtime_provenance();
    let node = build_node(&request)?;
    let experimental_channel_config = node.experimental_channel_config();
    let asset_runtime_probe = run_asset_runtime_probe(&node)?;
    node.start()
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;

    let native_node_id = node.node_id();
    node.connect(
        request.litd_node_id,
        request.litd_p2p_address.clone(),
        false,
    )
    .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;
    thread::sleep(Duration::from_millis(500));

    let peer_details = node.list_peers();
    let matched_peer = peer_details
        .iter()
        .find(|peer| peer.node_id == request.litd_node_id);
    let peer_connected = matched_peer.map(|peer| peer.is_connected).unwrap_or(false);
    let peer_persisted = matched_peer.map(|peer| peer.is_persisted).unwrap_or(false);
    let litd_peer_supports_simple_taproot_staging = matched_peer
        .map(|peer| peer.supports_simple_taproot_staging)
        .unwrap_or(false);
    let litd_peer_supports_taproot_asset_channel = matched_peer
        .map(|peer| peer.supports_taproot_asset_channel)
        .unwrap_or(false);

    if !peer_connected {
        let _ = node.stop();
        return Err(LiveLitdPeerError::PeerNotConnected);
    }

    let report = build_report(
        &request,
        provenance.clone(),
        experimental_channel_config,
        asset_runtime_probe.clone(),
        asset_runtime_snapshot(&node),
        native_node_id,
        peer_connected,
        peer_persisted,
        litd_peer_supports_simple_taproot_staging,
        litd_peer_supports_taproot_asset_channel,
        peer_details.len(),
        None,
        "holding",
    );
    write_report(&report_path, &report)?;

    let started = Instant::now();
    let mut latest_report = report;
    let mut native_to_litd_send_report = None;
    while started.elapsed() < Duration::from_secs(hold_seconds) {
        thread::sleep(Duration::from_secs(1));
        let snapshot = asset_runtime_snapshot(&node);
        if native_to_litd_send_report.is_none() {
            native_to_litd_send_report = maybe_send_native_to_litd_asset_payment(
                &node,
                request.litd_node_id,
                native_to_litd_send,
                &snapshot,
            )?;
        }
        let peer_details = node.list_peers();
        let matched_peer = peer_details
            .iter()
            .find(|peer| peer.node_id == request.litd_node_id);
        latest_report = build_report(
            &request,
            provenance.clone(),
            experimental_channel_config,
            asset_runtime_probe.clone(),
            asset_runtime_snapshot(&node),
            native_node_id,
            matched_peer.map(|peer| peer.is_connected).unwrap_or(false),
            matched_peer.map(|peer| peer.is_persisted).unwrap_or(false),
            matched_peer
                .map(|peer| peer.supports_simple_taproot_staging)
                .unwrap_or(false),
            matched_peer
                .map(|peer| peer.supports_taproot_asset_channel)
                .unwrap_or(false),
            peer_details.len(),
            native_to_litd_send_report.clone(),
            "holding",
        );
        write_report(&report_path, &latest_report)?;
    }

    node.stop()
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;
    Ok(latest_report)
}

pub fn run_live_litd_peer_restart_snapshot(
    request: LiveLitdPeerPreflightRequest,
    expected_payment_id: [u8; 32],
    expected_asset_id: [u8; 32],
    expected_asset_amount: u64,
) -> Result<LiveLitdPeerRestartSnapshotReport, LiveLitdPeerError> {
    let request = request.validate()?;
    let node = build_node(&request)?;
    let native_node_id = node.node_id();
    let snapshot = asset_runtime_snapshot(&node);
    let expected_payment_id_hex = hex32(expected_payment_id);

    let matched_payment = snapshot
        .payments
        .iter()
        .find(|payment| {
            payment.payment_id == expected_payment_id_hex
                && payment.direction == "remote_to_local"
                && payment.asset_id == expected_asset_id
                && payment.asset_amount == expected_asset_amount
                && payment.status == "settled"
        })
        .cloned();
    let matched_channel = matched_payment
        .as_ref()
        .and_then(|payment| {
            snapshot
                .channels
                .iter()
                .find(|channel| channel.channel_id == payment.channel_id)
        })
        .or_else(|| {
            snapshot.channels.iter().find(|channel| {
                channel.asset_id == expected_asset_id
                    && channel.local_balance >= expected_asset_amount
                    && !channel.closed
            })
        })
        .cloned();

    let received_asset_payment_persisted = matched_payment.is_some();
    let received_payment_balance_persisted = matched_payment
        .as_ref()
        .map(|payment| payment.local_balance_after >= expected_asset_amount)
        .unwrap_or(false);
    let received_channel_balance_persisted = matched_channel
        .as_ref()
        .map(|channel| {
            channel.asset_id == expected_asset_id
                && channel.local_balance >= expected_asset_amount
                && channel.monitor_aux_persisted
                && !channel.closed
        })
        .unwrap_or(false);
    let received_asset_balance_persisted =
        received_payment_balance_persisted || received_channel_balance_persisted;
    let restart_state_matches =
        received_asset_payment_persisted && received_asset_balance_persisted;

    Ok(LiveLitdPeerRestartSnapshotReport {
        status: if restart_state_matches {
            "completed".to_owned()
        } else {
            "blocked".to_owned()
        },
        network: "regtest".to_owned(),
        storage_dir_path: request.storage_dir_path.display().to_string(),
        native_node_id: native_node_id.to_string(),
        expected_payment_id: expected_payment_id_hex,
        expected_asset_id,
        expected_asset_amount,
        live_node_asset_channel_count: snapshot.channels.len(),
        live_node_asset_payment_count: snapshot.payments.len(),
        live_node_asset_channels: snapshot.channels,
        live_node_asset_payments: snapshot.payments,
        matched_payment,
        matched_channel,
        received_asset_payment_persisted,
        received_asset_balance_persisted,
        restart_state_matches,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_report(
    request: &ValidatedLiveLitdPeerPreflightRequest,
    provenance: RuntimeProvenance,
    experimental_channel_config: ExperimentalChannelConfig,
    asset_runtime_probe: LiveLitdAssetRuntimeProbe,
    asset_runtime_snapshot: LiveLitdAssetRuntimeSnapshot,
    native_node_id: PublicKey,
    peer_connected: bool,
    peer_persisted: bool,
    litd_peer_supports_simple_taproot_staging: bool,
    litd_peer_supports_taproot_asset_channel: bool,
    known_peer_count: usize,
    native_to_litd_asset_payment: Option<LiveLitdNativeAssetSendReport>,
    status: &str,
) -> LiveLitdPeerPreflightReport {
    let remaining_asset_channel_gap = if !litd_peer_supports_taproot_asset_channel {
        "Native LDK can connect to the independent litd peer through the OpenAgentsInc ldk-node fork and exposes typed Taproot Asset message/channel/payment APIs, but the connected litd peer does not advertise the Taproot Asset channel feature yet. #81 cannot honestly settle until the live peer negotiates that feature and the asset-channel funding/payment flow runs over it."
    } else {
        "Native LDK can connect to the independent litd peer through the OpenAgentsInc ldk-node fork, enables opt-in simple-taproot plus Taproot Asset channel negotiation, and exposes typed asset message/channel/payment APIs. The live outgoing-payment gate is beyond readiness: integrated litd fundchannel completes, Lightning Labs to native keysend settles, ldk-node records the native receiver asset balance, and the post-claim partial-signature plus stale funding-input control-block blockers are cleared."
    };

    LiveLitdPeerPreflightReport {
        status: status.to_owned(),
        network: "regtest".to_owned(),
        storage_dir_path: request.storage_dir_path.display().to_string(),
        live_node_runtime: format!(
            "ldk-node {} ({})",
            provenance.ldk_node_crate_version, provenance.ldk_node_fork_url
        ),
        live_node_uses_openagents_rust_lightning_fork: provenance
            .uses_openagents_rust_lightning_fork,
        live_node_simple_taproot_channels_enabled: experimental_channel_config
            .negotiate_simple_taproot_channels,
        live_node_taproot_asset_channels_enabled: experimental_channel_config
            .negotiate_taproot_asset_channels,
        live_node_asset_custom_message_api_ready: asset_runtime_probe.custom_message_api_ready,
        live_node_asset_channel_open_api_ready: asset_runtime_probe.channel_open_api_ready,
        live_node_asset_payment_api_ready: asset_runtime_probe.payment_api_ready,
        live_node_asset_runtime_event_count: asset_runtime_probe.runtime_event_count,
        live_node_lightning_channel_count: asset_runtime_snapshot.lightning_channels.len(),
        live_node_asset_channel_count: asset_runtime_snapshot.channels.len(),
        live_node_asset_payment_count: asset_runtime_snapshot.payments.len(),
        live_node_lightning_channels: asset_runtime_snapshot.lightning_channels,
        live_node_asset_channels: asset_runtime_snapshot.channels,
        live_node_asset_payments: asset_runtime_snapshot.payments,
        openagents_rust_lightning_rev: provenance.rust_lightning_fork_rev.to_owned(),
        fork_asset_channel_hooks_reachable_from_live_node: provenance
            .uses_openagents_rust_lightning_fork
            && experimental_channel_config.negotiate_simple_taproot_channels
            && experimental_channel_config.negotiate_taproot_asset_channels,
        native_node_id: native_node_id.to_string(),
        native_listening_socket: request.listening_socket.to_string(),
        litd_node_id: request.litd_node_id.to_string(),
        litd_p2p_address: request.litd_p2p_address.to_string(),
        native_node_started: true,
        peer_connected,
        peer_persisted,
        litd_peer_supports_simple_taproot_staging,
        litd_peer_supports_taproot_asset_channel,
        known_peer_count,
        asset_channel_settlement_ready: false,
        native_to_litd_asset_payment,
        remaining_asset_channel_gap: remaining_asset_channel_gap.to_owned(),
    }
}

fn maybe_send_native_to_litd_asset_payment(
    node: &Node,
    litd_node_id: PublicKey,
    request: Option<LiveLitdNativeAssetSendRequest>,
    snapshot: &LiveLitdAssetRuntimeSnapshot,
) -> Result<Option<LiveLitdNativeAssetSendReport>, LiveLitdPeerError> {
    let Some(request) = request else {
        return Ok(None);
    };
    let Some(channel) = snapshot.channels.iter().find(|channel| {
        channel.counterparty_node_id == litd_node_id.to_string()
            && channel.asset_id == request.asset_id
            && channel.local_balance >= request.asset_amount
            && !channel.closed
    }) else {
        return Ok(None);
    };
    let Some(lightning_channel) = snapshot.lightning_channels.iter().find(|channel| {
        channel.counterparty_node_id == litd_node_id.to_string()
            && channel.is_usable
            && channel.next_outbound_htlc_minimum_msat <= request.amount_msat
            && channel.next_outbound_htlc_limit_msat >= request.amount_msat
    }) else {
        return Ok(None);
    };

    let blob = encode_taproot_asset_htlc_blob(request.asset_id, request.asset_amount)
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;
    let send_result = node
        .spontaneous_payment()
        .send_with_taproot_asset_htlc_blob(request.amount_msat, litd_node_id, None, blob);
    let payment_id = match send_result {
        Ok(payment_id) => payment_id,
        Err(err) => {
            return Ok(Some(LiveLitdNativeAssetSendReport {
                attempted: true,
                status: "failed".to_owned(),
                payment_id: None,
                channel_id: Some(channel.channel_id.clone()),
                lightning_channel_id: Some(lightning_channel.channel_id.clone()),
                asset_id: request.asset_id,
                asset_amount: request.asset_amount,
                amount_msat: request.amount_msat,
                lightning_outbound_capacity_msat_before: Some(
                    lightning_channel.outbound_capacity_msat,
                ),
                lightning_next_outbound_htlc_limit_msat_before: Some(
                    lightning_channel.next_outbound_htlc_limit_msat,
                ),
                lightning_next_outbound_htlc_minimum_msat_before: Some(
                    lightning_channel.next_outbound_htlc_minimum_msat,
                ),
                local_balance_before: Some(channel.local_balance),
                remote_balance_before: Some(channel.remote_balance),
                local_balance_after: None,
                remote_balance_after: None,
                error: Some(err.to_string()),
            }));
        }
    };

    let channel_id = parse_hex32(&channel.channel_id)?;
    let now = now_unix_seconds()?;
    let metadata = TaprootAssetPaymentMetadata {
        asset_id: request.asset_id,
        asset_amount: request.asset_amount,
        proof_root_hash: channel.proof_root_hash,
        proof_root_sum: channel.proof_root_sum,
        quote_id: payment_id.0,
        payment_hash: payment_id.0,
    };
    let status = node
        .taproot_asset()
        .send_payment(TaprootAssetPaymentRequest {
            channel_id,
            payment_id: payment_id.0,
            direction: TaprootAssetPaymentDirection::LocalToRemote,
            expected: metadata,
            metadata: Some(metadata),
            quote_accepted: true,
            now_unix_seconds: now,
            quote_expiry_unix_seconds: now + 300,
            monitor_aux: Some(TaprootAssetMonitorAuxRequest {
                state_digest: derived_digest(payment_id.0, 0x51),
                nonce_digest: derived_digest(payment_id.0, 0x52),
                signature_digest: derived_digest(payment_id.0, 0x53),
            }),
        })
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;

    Ok(Some(LiveLitdNativeAssetSendReport {
        attempted: true,
        status: status.status,
        payment_id: Some(status.payment_id),
        channel_id: Some(status.channel_id),
        lightning_channel_id: Some(lightning_channel.channel_id.clone()),
        asset_id: status.asset_id,
        asset_amount: status.asset_amount,
        amount_msat: request.amount_msat,
        lightning_outbound_capacity_msat_before: Some(lightning_channel.outbound_capacity_msat),
        lightning_next_outbound_htlc_limit_msat_before: Some(
            lightning_channel.next_outbound_htlc_limit_msat,
        ),
        lightning_next_outbound_htlc_minimum_msat_before: Some(
            lightning_channel.next_outbound_htlc_minimum_msat,
        ),
        local_balance_before: Some(channel.local_balance),
        remote_balance_before: Some(channel.remote_balance),
        local_balance_after: Some(status.local_balance_after),
        remote_balance_after: Some(status.remote_balance_after),
        error: None,
    }))
}

fn parse_hex32(input: &str) -> Result<[u8; 32], LiveLitdPeerError> {
    if input.len() != 64 {
        return Err(LiveLitdPeerError::AssetRuntime(format!(
            "expected 32-byte hex value, got {} bytes of text",
            input.len()
        )));
    }
    let mut out = [0u8; 32];
    for idx in 0..32 {
        out[idx] = u8::from_str_radix(&input[idx * 2..idx * 2 + 2], 16)
            .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;
    }
    Ok(out)
}

fn hex32(input: [u8; 32]) -> String {
    input.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_unix_seconds() -> Result<u64, LiveLitdPeerError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?
        .as_secs())
}

fn derived_digest(mut base: [u8; 32], marker: u8) -> [u8; 32] {
    base[0] ^= marker;
    base
}

fn write_report(
    report_path: impl AsRef<Path>,
    report: &LiveLitdPeerPreflightReport,
) -> Result<(), LiveLitdPeerError> {
    if let Some(parent) = report_path.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|err| LiveLitdPeerError::Report(err.to_string()))?;
    }
    let json = serde_json::to_vec_pretty(report)
        .map_err(|err| LiveLitdPeerError::Report(err.to_string()))?;
    fs::write(report_path, json).map_err(|err| LiveLitdPeerError::Report(err.to_string()))
}

fn build_node(request: &ValidatedLiveLitdPeerPreflightRequest) -> Result<Node, LiveLitdPeerError> {
    let mut builder = Builder::new();
    builder.set_network(Network::Regtest);
    builder.set_experimental_channel_config(live_litd_experimental_channel_config());
    builder.set_storage_dir_path(request.storage_dir_path.display().to_string());
    builder.set_filesystem_logger(None, Some(live_litd_log_level()));
    builder.set_chain_source_bitcoind_rpc(
        request.bitcoind_rpc_host.clone(),
        request.bitcoind_rpc_port,
        request.bitcoind_rpc_user.clone(),
        request.bitcoind_rpc_password.clone(),
    );
    builder
        .set_listening_addresses(vec![request.listening_socket.clone()])
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;
    let node_entropy = node_entropy_from_storage(&request.storage_dir_path)?;
    builder
        .build(node_entropy)
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))
}

fn live_litd_log_level() -> LogLevel {
    match env::var("TAP_LDK_LIVE_LITD_LDK_LOG_LEVEL")
        .unwrap_or_else(|_| "debug".to_owned())
        .to_ascii_lowercase()
        .as_str()
    {
        "gossip" => LogLevel::Gossip,
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        _ => LogLevel::Debug,
    }
}

fn live_litd_experimental_channel_config() -> ExperimentalChannelConfig {
    ExperimentalChannelConfig::taproot_assets_regtest()
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LiveLitdAssetRuntimeProbe {
    custom_message_api_ready: bool,
    channel_open_api_ready: bool,
    payment_api_ready: bool,
    runtime_event_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LiveLitdAssetRuntimeSnapshot {
    lightning_channels: Vec<LiveLightningChannelSnapshot>,
    channels: Vec<TaprootAssetChannelStatus>,
    payments: Vec<TaprootAssetPaymentStatus>,
}

fn run_asset_runtime_probe(node: &Node) -> Result<LiveLitdAssetRuntimeProbe, LiveLitdPeerError> {
    let taproot_asset = node.taproot_asset();
    let synthetic_peer = synthetic_peer(21)?;
    let ids = SyntheticAssetRuntimeIds::fresh()?;
    let queued = taproot_asset
        .send_message(
            synthetic_peer,
            TaprootAssetMessageKind::RfqRequest,
            b"tap-ldk-live-litd-rfq-preflight".to_vec(),
        )
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;
    let channel = taproot_asset
        .open_channel(synthetic_open_request(synthetic_peer, &ids)?)
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;
    let payment = taproot_asset
        .send_payment(synthetic_payment_request(
            TaprootAssetPaymentDirection::LocalToRemote,
            &ids,
        ))
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;

    Ok(LiveLitdAssetRuntimeProbe {
        custom_message_api_ready: queued.payload_len > 0,
        channel_open_api_ready: channel.funding_accepted && channel.monitor_aux_persisted,
        payment_api_ready: payment.status == "settled",
        runtime_event_count: taproot_asset.list_events().len(),
    })
}

fn asset_runtime_snapshot(node: &Node) -> LiveLitdAssetRuntimeSnapshot {
    let taproot_asset = node.taproot_asset();
    LiveLitdAssetRuntimeSnapshot {
        lightning_channels: node
            .list_channels()
            .into_iter()
            .map(lightning_channel_snapshot)
            .collect(),
        channels: taproot_asset.list_channels(),
        payments: taproot_asset.list_payments(),
    }
}

fn lightning_channel_snapshot(channel: ChannelDetails) -> LiveLightningChannelSnapshot {
    LiveLightningChannelSnapshot {
        channel_id: channel.channel_id.to_string(),
        counterparty_node_id: channel.counterparty_node_id.to_string(),
        channel_value_sats: channel.channel_value_sats,
        outbound_capacity_msat: channel.outbound_capacity_msat,
        inbound_capacity_msat: channel.inbound_capacity_msat,
        next_outbound_htlc_limit_msat: channel.next_outbound_htlc_limit_msat,
        next_outbound_htlc_minimum_msat: channel.next_outbound_htlc_minimum_msat,
        is_outbound: channel.is_outbound,
        is_channel_ready: channel.is_channel_ready,
        is_usable: channel.is_usable,
        confirmations: channel.confirmations,
        unspendable_punishment_reserve_sats: channel.unspendable_punishment_reserve,
        counterparty_unspendable_punishment_reserve_sats: channel
            .counterparty_unspendable_punishment_reserve,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SyntheticAssetRuntimeIds {
    channel_id: [u8; 32],
    pending_channel_id: [u8; 32],
    payment_id: [u8; 32],
}

impl SyntheticAssetRuntimeIds {
    fn fresh() -> Result<Self, LiveLitdPeerError> {
        let marker = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?
            .as_nanos()
            .to_le_bytes();
        Ok(Self {
            channel_id: unique_32(4, marker),
            pending_channel_id: unique_32(5, marker),
            payment_id: unique_32(16, marker),
        })
    }

    #[cfg(test)]
    fn deterministic() -> Self {
        Self {
            channel_id: nonzero_32(4),
            pending_channel_id: nonzero_32(5),
            payment_id: nonzero_32(16),
        }
    }
}

fn synthetic_open_request(
    counterparty_node_id: PublicKey,
    ids: &SyntheticAssetRuntimeIds,
) -> Result<TaprootAssetChannelOpenRequest, LiveLitdPeerError> {
    Ok(TaprootAssetChannelOpenRequest {
        counterparty_node_id,
        channel_id: ids.channel_id,
        pending_channel_id: ids.pending_channel_id,
        funding_outpoint: LdkOutPoint {
            txid: Txid::from_str(
                "1111111111111111111111111111111111111111111111111111111111111111",
            )
            .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?,
            index: 0,
        },
        asset_id: nonzero_32(7),
        genesis_id: nonzero_32(8),
        group_key: None,
        proof_root_hash: nonzero_32(9),
        proof_root_sum: 1_000,
        output_commitment: nonzero_32(10),
        local_amount: 700,
        remote_amount: 300,
        complete_fragment_count: 2,
        expected_fragment_count: 2,
        monitor_aux: TaprootAssetMonitorAuxRequest {
            state_digest: nonzero_32(11),
            nonce_digest: nonzero_32(12),
            signature_digest: nonzero_32(13),
        },
    })
}

fn synthetic_payment_request(
    direction: TaprootAssetPaymentDirection,
    ids: &SyntheticAssetRuntimeIds,
) -> TaprootAssetPaymentRequest {
    let metadata = TaprootAssetPaymentMetadata {
        asset_id: nonzero_32(7),
        asset_amount: 125,
        proof_root_hash: nonzero_32(9),
        proof_root_sum: 1_000,
        quote_id: nonzero_32(14),
        payment_hash: nonzero_32(15),
    };
    TaprootAssetPaymentRequest {
        channel_id: ids.channel_id,
        payment_id: ids.payment_id,
        direction,
        expected: metadata,
        metadata: Some(metadata),
        quote_accepted: true,
        now_unix_seconds: 10,
        quote_expiry_unix_seconds: 20,
        monitor_aux: Some(TaprootAssetMonitorAuxRequest {
            state_digest: nonzero_32(17),
            nonce_digest: nonzero_32(18),
            signature_digest: nonzero_32(19),
        }),
    }
}

fn synthetic_peer(seed: u8) -> Result<PublicKey, LiveLitdPeerError> {
    let secp = Secp256k1::signing_only();
    let secret = SecretKey::from_slice(&[seed; 32])
        .map_err(|err| LiveLitdPeerError::AssetRuntime(err.to_string()))?;
    Ok(PublicKey::from_secret_key(&secp, &secret))
}

fn nonzero_32(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn unique_32(seed: u8, marker: [u8; 16]) -> [u8; 32] {
    let mut out = [seed; 32];
    out[..16].copy_from_slice(&marker);
    out[16] = seed;
    out
}

fn node_entropy_from_storage(storage_dir_path: &Path) -> Result<NodeEntropy, LiveLitdPeerError> {
    fs::create_dir_all(storage_dir_path).map_err(|err| LiveLitdPeerError::Node(err.to_string()))?;
    NodeEntropy::from_seed_path(storage_dir_path.join("node-entropy").display().to_string())
        .map_err(|err| LiveLitdPeerError::Node(err.to_string()))
}

#[derive(Debug)]
pub enum LiveLitdPeerError {
    InvalidRequest(String),
    InvalidNodeId(String),
    InvalidSocketAddress(String),
    Node(String),
    AssetRuntime(String),
    Report(String),
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
            Self::AssetRuntime(err) => write!(f, "live litd peer asset runtime error: {err}"),
            Self::Report(err) => write!(f, "live litd peer report error: {err}"),
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

    #[test]
    fn imported_ldk_node_reports_openagents_rust_lightning_fork() {
        let provenance = runtime_provenance();

        assert!(provenance.uses_openagents_rust_lightning_fork);
        assert_eq!(
            provenance.rust_lightning_fork_rev,
            "3db3229733b724f45e7a356d923715213cb4f269"
        );
        assert_eq!(
            provenance.ldk_node_fork_url,
            "https://github.com/OpenAgentsInc/ldk-node"
        );
    }

    #[test]
    fn live_litd_preflight_enables_experimental_channel_config() {
        let config = live_litd_experimental_channel_config();

        assert!(config.negotiate_simple_taproot_channels);
        assert!(config.negotiate_taproot_asset_channels);
    }

    #[test]
    fn synthetic_asset_runtime_requests_bind_channel_and_payment_ids() {
        let peer = synthetic_peer(21).expect("peer");
        let ids = SyntheticAssetRuntimeIds::deterministic();
        let open = synthetic_open_request(peer, &ids).expect("open request");
        let payment = synthetic_payment_request(TaprootAssetPaymentDirection::LocalToRemote, &ids);

        assert_eq!(open.channel_id, payment.channel_id);
        assert_eq!(open.asset_id, payment.expected.asset_id);
        assert_eq!(payment.expected.asset_amount, 125);
    }
}
