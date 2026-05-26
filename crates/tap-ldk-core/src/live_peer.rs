use std::{
    error::Error,
    fmt,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    asset::Bytes32,
    asset_channel_negotiation::{
        ASSET_CHANNEL_PROTOCOL_VERSION, ASSET_CHANNEL_REQUIRED_FEATURE_BIT, AssetChannelFeatureSet,
        ChannelRequest, NegotiatedChannelType, NegotiationError, NegotiationInput,
        negotiate_channel, require_asset_message_allowed,
    },
    asset_peer_message::{AssetPeerMessage, AssetPeerMessageError},
};

const MAX_FRAME_LEN: usize = 1_048_576;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LivePeerSmokeReport {
    pub listener_addr: String,
    pub server_started: bool,
    pub client_connected: bool,
    pub asset_id: Bytes32,
    pub negotiated_asset_channel: bool,
    pub rust_lightning_fork_negotiation_used: bool,
    pub local_feature_bits: Vec<u16>,
    pub remote_feature_bits: Vec<u16>,
    pub negotiated_channel_type: NegotiatedChannelType,
    pub custom_message_type: u64,
    pub custom_message_kind: String,
    pub custom_message_payload_len: usize,
    pub custom_message_payload_digest: Bytes32,
    pub custom_message_round_trip: bool,
    pub transport: String,
    pub lightning_labs_peer_connected: bool,
    pub remaining_live_counterparty_gap: Option<String>,
}

pub fn run_live_peer_smoke(asset_id: Bytes32) -> Result<LivePeerSmokeReport, LivePeerError> {
    if asset_id == Bytes32::ZERO {
        return Err(LivePeerError::MissingAssetId);
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(LivePeerError::Io)?;
    let listener_addr = listener
        .local_addr()
        .map_err(LivePeerError::Io)?
        .to_string();
    let (server_ready_tx, server_ready_rx) = mpsc::channel();
    let server_asset_id = asset_id;
    let server = thread::spawn(move || {
        server_ready_tx
            .send(())
            .map_err(|_| LivePeerError::Thread("server readiness channel closed".to_owned()))?;
        run_live_peer_server(listener, server_asset_id)
    });
    server_ready_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|_| LivePeerError::Thread("server did not become ready".to_owned()))?;

    let mut client_stream = TcpStream::connect(&listener_addr).map_err(LivePeerError::Io)?;
    configure_stream(&client_stream)?;
    let client_feature_set = AssetChannelFeatureSet::advertise_optional();
    write_frame(
        &mut client_stream,
        &LivePeerWireMessage::Hello {
            peer_name: "tap-ldk-client".to_owned(),
            feature_bits: client_feature_set.feature_bits(),
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
            asset_id,
        },
    )?;
    let ack = match read_frame(&mut client_stream)? {
        LivePeerWireMessage::HelloAck {
            accepted,
            local_feature_bits,
            remote_feature_bits,
            channel_type,
        } => {
            if !accepted {
                return Err(LivePeerError::Protocol(
                    "server rejected asset-channel hello".to_owned(),
                ));
            }
            (local_feature_bits, remote_feature_bits, channel_type)
        }
        other => {
            return Err(LivePeerError::Protocol(format!(
                "expected hello ack, received {other:?}"
            )));
        }
    };

    let message = sample_custom_message(asset_id);
    let payload = message.encode().map_err(LivePeerError::PeerMessage)?;
    let custom_message_type = message.message_type();
    write_frame(
        &mut client_stream,
        &LivePeerWireMessage::CustomMessage {
            message_type: custom_message_type,
            payload,
        },
    )?;
    let (custom_message_kind, custom_message_payload_digest, custom_message_round_trip) =
        match read_frame(&mut client_stream)? {
            LivePeerWireMessage::CustomMessageAck {
                message_type,
                decoded_kind,
                payload_digest,
            } => (
                decoded_kind,
                payload_digest,
                message_type == custom_message_type
                    && payload_digest == custom_payload_digest(&message)?,
            ),
            other => {
                return Err(LivePeerError::Protocol(format!(
                    "expected custom message ack, received {other:?}"
                )));
            }
        };

    let server_result = server
        .join()
        .map_err(|_| LivePeerError::Thread("server thread panicked".to_owned()))??;
    if !server_result.custom_message_received {
        return Err(LivePeerError::Protocol(
            "server did not record custom message receipt".to_owned(),
        ));
    }

    Ok(LivePeerSmokeReport {
        listener_addr,
        server_started: true,
        client_connected: true,
        asset_id,
        negotiated_asset_channel: ack.2.is_asset_channel(),
        rust_lightning_fork_negotiation_used: true,
        local_feature_bits: ack.0,
        remote_feature_bits: ack.1,
        negotiated_channel_type: ack.2,
        custom_message_type,
        custom_message_kind,
        custom_message_payload_len: server_result.custom_message_payload_len,
        custom_message_payload_digest,
        custom_message_round_trip,
        transport: "loopback_tcp_framed_json".to_owned(),
        lightning_labs_peer_connected: false,
        remaining_live_counterparty_gap: Some(
            "Lightning Labs daemon-backed peer connection and Lightning wire custom-message exchange are handled by the next Path B counterparty issues."
                .to_owned(),
        ),
    })
}

fn run_live_peer_server(
    listener: TcpListener,
    asset_id: Bytes32,
) -> Result<LivePeerServerReport, LivePeerError> {
    let (mut stream, _addr) = listener.accept().map_err(LivePeerError::Io)?;
    configure_stream(&stream)?;
    let (remote_feature_bits, channel_type) = match read_frame(&mut stream)? {
        LivePeerWireMessage::Hello {
            peer_name,
            feature_bits,
            protocol_version,
            asset_id: remote_asset_id,
        } => {
            if peer_name.trim().is_empty() {
                return Err(LivePeerError::Protocol("empty peer name".to_owned()));
            }
            if protocol_version != ASSET_CHANNEL_PROTOCOL_VERSION {
                return Err(LivePeerError::Protocol(format!(
                    "unsupported peer protocol version {protocol_version}"
                )));
            }
            if remote_asset_id != asset_id {
                return Err(LivePeerError::Protocol("peer asset id mismatch".to_owned()));
            }
            let remote_feature_set = feature_set_from_bits(&feature_bits);
            let outcome = negotiate_channel(NegotiationInput {
                local: AssetChannelFeatureSet::require(),
                remote: remote_feature_set,
                request: ChannelRequest::SingleAsset { asset_id },
            })
            .map_err(LivePeerError::Negotiation)?;
            let channel_type = outcome.channel_type;
            write_frame(
                &mut stream,
                &LivePeerWireMessage::HelloAck {
                    accepted: true,
                    local_feature_bits: outcome.local_feature_bits,
                    remote_feature_bits: outcome.remote_feature_bits.clone(),
                    channel_type: channel_type.clone(),
                },
            )?;
            (outcome.remote_feature_bits, channel_type)
        }
        other => {
            return Err(LivePeerError::Protocol(format!(
                "expected hello, received {other:?}"
            )));
        }
    };

    require_asset_message_allowed(&channel_type).map_err(LivePeerError::Negotiation)?;
    let (custom_message_payload_len, decoded_kind) = match read_frame(&mut stream)? {
        LivePeerWireMessage::CustomMessage {
            message_type,
            payload,
        } => {
            let decoded = AssetPeerMessage::decode(&payload).map_err(LivePeerError::PeerMessage)?;
            if decoded.message_type() != message_type {
                return Err(LivePeerError::Protocol(
                    "custom message type mismatch".to_owned(),
                ));
            }
            let payload_digest = Bytes32(Sha256::digest(&payload).into());
            let decoded_kind = message_kind(&decoded).to_owned();
            write_frame(
                &mut stream,
                &LivePeerWireMessage::CustomMessageAck {
                    message_type,
                    decoded_kind: decoded_kind.clone(),
                    payload_digest,
                },
            )?;
            (payload.len(), decoded_kind)
        }
        other => {
            return Err(LivePeerError::Protocol(format!(
                "expected custom message, received {other:?}"
            )));
        }
    };

    Ok(LivePeerServerReport {
        remote_feature_bits,
        custom_message_received: !decoded_kind.is_empty(),
        custom_message_payload_len,
    })
}

fn configure_stream(stream: &TcpStream) -> Result<(), LivePeerError> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(LivePeerError::Io)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(LivePeerError::Io)
}

fn write_frame(stream: &mut TcpStream, message: &LivePeerWireMessage) -> Result<(), LivePeerError> {
    let raw = serde_json::to_vec(message).map_err(LivePeerError::Json)?;
    if raw.len() > MAX_FRAME_LEN {
        return Err(LivePeerError::FrameTooLarge(raw.len()));
    }
    stream
        .write_all(&(raw.len() as u32).to_be_bytes())
        .map_err(LivePeerError::Io)?;
    stream.write_all(&raw).map_err(LivePeerError::Io)
}

fn read_frame(stream: &mut TcpStream) -> Result<LivePeerWireMessage, LivePeerError> {
    let mut len = [0_u8; 4];
    stream.read_exact(&mut len).map_err(LivePeerError::Io)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_LEN {
        return Err(LivePeerError::FrameTooLarge(len));
    }
    let mut raw = vec![0_u8; len];
    stream.read_exact(&mut raw).map_err(LivePeerError::Io)?;
    serde_json::from_slice(&raw).map_err(LivePeerError::Json)
}

fn feature_set_from_bits(bits: &[u16]) -> AssetChannelFeatureSet {
    if bits.contains(&ASSET_CHANNEL_REQUIRED_FEATURE_BIT) {
        AssetChannelFeatureSet::require()
    } else {
        AssetChannelFeatureSet::advertise_optional()
    }
}

fn sample_custom_message(asset_id: Bytes32) -> AssetPeerMessage {
    AssetPeerMessage::RfqRequest {
        rfq_id: Bytes32([41; 32]),
        asset_id,
        asset_amount: 25,
        invoice_context: Bytes32([42; 32]),
    }
}

fn custom_payload_digest(message: &AssetPeerMessage) -> Result<Bytes32, LivePeerError> {
    let payload = message.encode().map_err(LivePeerError::PeerMessage)?;
    Ok(Bytes32(Sha256::digest(payload).into()))
}

fn message_kind(message: &AssetPeerMessage) -> &'static str {
    match message {
        AssetPeerMessage::TxAssetInputProof { .. } => "tx_asset_input_proof",
        AssetPeerMessage::TxAssetOutputProof { .. } => "tx_asset_output_proof",
        AssetPeerMessage::AssetFundingCreated { .. } => "asset_funding_created",
        AssetPeerMessage::AssetFundingAccepted { .. } => "asset_funding_accepted",
        AssetPeerMessage::RfqRequest { .. } => "rfq_request",
        AssetPeerMessage::RfqAccept { .. } => "rfq_accept",
        AssetPeerMessage::RfqReject { .. } => "rfq_reject",
        AssetPeerMessage::AssetHtlcBlob { .. } => "asset_htlc_blob",
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
enum LivePeerWireMessage {
    Hello {
        peer_name: String,
        feature_bits: Vec<u16>,
        protocol_version: u16,
        asset_id: Bytes32,
    },
    HelloAck {
        accepted: bool,
        local_feature_bits: Vec<u16>,
        remote_feature_bits: Vec<u16>,
        channel_type: NegotiatedChannelType,
    },
    CustomMessage {
        message_type: u64,
        payload: Vec<u8>,
    },
    CustomMessageAck {
        message_type: u64,
        decoded_kind: String,
        payload_digest: Bytes32,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LivePeerServerReport {
    remote_feature_bits: Vec<u16>,
    custom_message_received: bool,
    custom_message_payload_len: usize,
}

#[derive(Debug)]
pub enum LivePeerError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Negotiation(NegotiationError),
    PeerMessage(AssetPeerMessageError),
    MissingAssetId,
    FrameTooLarge(usize),
    Protocol(String),
    Thread(String),
}

impl fmt::Display for LivePeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "live peer IO error: {err}"),
            Self::Json(err) => write!(f, "live peer JSON error: {err}"),
            Self::Negotiation(err) => write!(f, "live peer negotiation error: {err}"),
            Self::PeerMessage(err) => write!(f, "live peer message error: {err}"),
            Self::MissingAssetId => write!(f, "live peer requires a non-zero asset id"),
            Self::FrameTooLarge(len) => write!(f, "live peer frame too large: {len} bytes"),
            Self::Protocol(message) => write!(f, "live peer protocol error: {message}"),
            Self::Thread(message) => write!(f, "live peer thread error: {message}"),
        }
    }
}

impl Error for LivePeerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_peer_smoke_negotiates_and_round_trips_custom_message() {
        let report = run_live_peer_smoke(Bytes32([7; 32])).expect("live peer smoke");

        assert!(report.server_started);
        assert!(report.client_connected);
        assert!(report.negotiated_asset_channel);
        assert!(report.rust_lightning_fork_negotiation_used);
        assert_eq!(
            report.local_feature_bits,
            vec![ASSET_CHANNEL_REQUIRED_FEATURE_BIT]
        );
        assert!(!report.remote_feature_bits.is_empty());
        assert_eq!(report.custom_message_kind, "rfq_request");
        assert!(report.custom_message_payload_len > 0);
        assert!(report.custom_message_round_trip);
        assert!(!report.lightning_labs_peer_connected);
        assert!(report.remaining_live_counterparty_gap.is_some());
    }
}
