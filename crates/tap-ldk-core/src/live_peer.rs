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
    asset_peer_message::{
        AssetPeerMessage, AssetPeerMessageError, ProofAssembly, ProofChunk, ProofReassembler,
    },
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveAssetPaymentSessionReport {
    pub status: String,
    pub listener_addr: String,
    pub server_started: bool,
    pub client_connected: bool,
    pub asset_id: Bytes32,
    pub asset_amount: u64,
    pub negotiated_asset_channel: bool,
    pub rust_lightning_fork_negotiation_used: bool,
    pub local_feature_bits: Vec<u16>,
    pub remote_feature_bits: Vec<u16>,
    pub negotiated_channel_type: NegotiatedChannelType,
    pub message_count: usize,
    pub message_reports: Vec<LivePeerMessageReport>,
    pub ordered_message_kinds: Vec<String>,
    pub input_proof_reassembled_len: usize,
    pub output_proof_reassembled_len: usize,
    pub session_payment_id: Bytes32,
    pub settlement_ack_received: bool,
    pub native_wire_session_ready: bool,
    pub lightning_labs_peer_connected: bool,
    pub remaining_live_counterparty_gap: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LivePeerMessageReport {
    pub sequence: usize,
    pub message_type: u64,
    pub decoded_kind: String,
    pub payload_len: usize,
    pub payload_digest: Bytes32,
    pub acked: bool,
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

pub fn run_live_asset_payment_session_smoke(
    asset_id: Bytes32,
    asset_amount: u64,
) -> Result<LiveAssetPaymentSessionReport, LivePeerError> {
    if asset_id == Bytes32::ZERO {
        return Err(LivePeerError::MissingAssetId);
    }
    if asset_amount == 0 {
        return Err(LivePeerError::Protocol(
            "live asset payment session requires a non-zero asset amount".to_owned(),
        ));
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
        run_live_asset_payment_session_server(listener, server_asset_id, asset_amount)
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
            peer_name: "tap-ldk-payment-client".to_owned(),
            feature_bits: client_feature_set.feature_bits(),
            protocol_version: ASSET_CHANNEL_PROTOCOL_VERSION,
            asset_id,
        },
    )?;
    let (local_feature_bits, remote_feature_bits, negotiated_channel_type) =
        match read_frame(&mut client_stream)? {
            LivePeerWireMessage::HelloAck {
                accepted,
                local_feature_bits,
                remote_feature_bits,
                channel_type,
            } => {
                if !accepted {
                    return Err(LivePeerError::Protocol(
                        "server rejected asset-payment session hello".to_owned(),
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

    let messages = sample_asset_payment_session_messages(asset_id, asset_amount)?;
    let session_payment_id = session_payment_id(asset_id, asset_amount, &messages)?;
    let mut message_reports = Vec::with_capacity(messages.len());

    for (sequence, message) in messages.iter().enumerate() {
        let payload = message.encode().map_err(LivePeerError::PeerMessage)?;
        let payload_digest = Bytes32(Sha256::digest(&payload).into());
        let message_type = message.message_type();
        write_frame(
            &mut client_stream,
            &LivePeerWireMessage::CustomMessage {
                message_type,
                payload: payload.clone(),
            },
        )?;
        match read_frame(&mut client_stream)? {
            LivePeerWireMessage::CustomMessageAck {
                message_type: acked_message_type,
                decoded_kind,
                payload_digest: acked_payload_digest,
            } => {
                let acked =
                    acked_message_type == message_type && acked_payload_digest == payload_digest;
                if !acked {
                    return Err(LivePeerError::Protocol(format!(
                        "server ack mismatch for payment-session message {sequence}"
                    )));
                }
                message_reports.push(LivePeerMessageReport {
                    sequence,
                    message_type,
                    decoded_kind,
                    payload_len: payload.len(),
                    payload_digest,
                    acked,
                });
            }
            other => {
                return Err(LivePeerError::Protocol(format!(
                    "expected custom message ack, received {other:?}"
                )));
            }
        }
    }

    write_frame(
        &mut client_stream,
        &LivePeerWireMessage::SessionComplete {
            payment_id: session_payment_id,
            message_count: messages.len(),
        },
    )?;
    let (
        settlement_ack_received,
        acked_payment_id,
        input_proof_reassembled_len,
        output_proof_reassembled_len,
    ) = match read_frame(&mut client_stream)? {
        LivePeerWireMessage::SessionCompleteAck {
            accepted,
            payment_id,
            message_count,
            input_proof_reassembled_len,
            output_proof_reassembled_len,
        } => (
            accepted && payment_id == session_payment_id && message_count == messages.len(),
            payment_id,
            input_proof_reassembled_len,
            output_proof_reassembled_len,
        ),
        other => {
            return Err(LivePeerError::Protocol(format!(
                "expected session complete ack, received {other:?}"
            )));
        }
    };
    if !settlement_ack_received || acked_payment_id != session_payment_id {
        return Err(LivePeerError::Protocol(
            "payment-session completion ack did not match client state".to_owned(),
        ));
    }

    let server_result = server
        .join()
        .map_err(|_| LivePeerError::Thread("server thread panicked".to_owned()))??;
    if !server_result.session_complete {
        return Err(LivePeerError::Protocol(
            "server did not record payment-session completion".to_owned(),
        ));
    }

    let ordered_message_kinds = message_reports
        .iter()
        .map(|report| report.decoded_kind.clone())
        .collect::<Vec<_>>();
    Ok(LiveAssetPaymentSessionReport {
        status: "completed".to_owned(),
        listener_addr,
        server_started: true,
        client_connected: true,
        asset_id,
        asset_amount,
        negotiated_asset_channel: negotiated_channel_type.is_asset_channel(),
        rust_lightning_fork_negotiation_used: true,
        local_feature_bits,
        remote_feature_bits,
        negotiated_channel_type,
        message_count: message_reports.len(),
        message_reports,
        ordered_message_kinds,
        input_proof_reassembled_len,
        output_proof_reassembled_len,
        session_payment_id,
        settlement_ack_received,
        native_wire_session_ready: true,
        lightning_labs_peer_connected: false,
        remaining_live_counterparty_gap: Some(
            "This is the native tap-ldk ordered asset-payment wire session. Issue #57 still requires replacing the loopback peer with the independent Lightning Labs LND/tapd counterparty and observing its receiver balance after settlement."
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

fn run_live_asset_payment_session_server(
    listener: TcpListener,
    asset_id: Bytes32,
    asset_amount: u64,
) -> Result<LiveAssetPaymentSessionServerReport, LivePeerError> {
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
    let expected_messages = sample_asset_payment_session_messages(asset_id, asset_amount)?;
    let expected_payment_id = session_payment_id(asset_id, asset_amount, &expected_messages)?;
    let mut input_reassembler = ProofReassembler::default();
    let mut output_reassembler = ProofReassembler::default();
    let mut input_proof_reassembled_len = 0;
    let mut output_proof_reassembled_len = 0;
    let mut ordered_message_kinds = Vec::with_capacity(expected_messages.len());

    for (sequence, expected) in expected_messages.iter().enumerate() {
        match read_frame(&mut stream)? {
            LivePeerWireMessage::CustomMessage {
                message_type,
                payload,
            } => {
                let decoded =
                    AssetPeerMessage::decode(&payload).map_err(LivePeerError::PeerMessage)?;
                if decoded.message_type() != message_type {
                    return Err(LivePeerError::Protocol(format!(
                        "payment-session message {sequence} type mismatch"
                    )));
                }
                if &decoded != expected {
                    return Err(LivePeerError::Protocol(format!(
                        "payment-session message {sequence} did not match expected {}",
                        message_kind(expected)
                    )));
                }

                match &decoded {
                    AssetPeerMessage::TxAssetInputProof { chunk, .. } => {
                        if let ProofAssembly::Complete(proof) = input_reassembler
                            .push(chunk.clone())
                            .map_err(LivePeerError::PeerMessage)?
                        {
                            input_proof_reassembled_len = proof.len();
                        }
                    }
                    AssetPeerMessage::TxAssetOutputProof { chunk, .. } => {
                        if let ProofAssembly::Complete(proof) = output_reassembler
                            .push(chunk.clone())
                            .map_err(LivePeerError::PeerMessage)?
                        {
                            output_proof_reassembled_len = proof.len();
                        }
                    }
                    AssetPeerMessage::AssetFundingCreated { .. }
                    | AssetPeerMessage::AssetFundingAccepted { .. }
                    | AssetPeerMessage::RfqRequest { .. }
                    | AssetPeerMessage::RfqAccept { .. }
                    | AssetPeerMessage::RfqReject { .. }
                    | AssetPeerMessage::AssetHtlcBlob { .. } => {}
                }

                let payload_digest = Bytes32(Sha256::digest(&payload).into());
                let decoded_kind = message_kind(&decoded).to_owned();
                ordered_message_kinds.push(decoded_kind.clone());
                write_frame(
                    &mut stream,
                    &LivePeerWireMessage::CustomMessageAck {
                        message_type,
                        decoded_kind,
                        payload_digest,
                    },
                )?;
            }
            other => {
                return Err(LivePeerError::Protocol(format!(
                    "expected payment-session custom message {sequence}, received {other:?}"
                )));
            }
        }
    }

    match read_frame(&mut stream)? {
        LivePeerWireMessage::SessionComplete {
            payment_id,
            message_count,
        } => {
            if payment_id != expected_payment_id {
                return Err(LivePeerError::Protocol(
                    "payment-session completion id mismatch".to_owned(),
                ));
            }
            if message_count != expected_messages.len() {
                return Err(LivePeerError::Protocol(
                    "payment-session completion count mismatch".to_owned(),
                ));
            }
            write_frame(
                &mut stream,
                &LivePeerWireMessage::SessionCompleteAck {
                    accepted: input_proof_reassembled_len > 0 && output_proof_reassembled_len > 0,
                    payment_id,
                    message_count,
                    input_proof_reassembled_len,
                    output_proof_reassembled_len,
                },
            )?;
        }
        other => {
            return Err(LivePeerError::Protocol(format!(
                "expected session completion, received {other:?}"
            )));
        }
    }

    Ok(LiveAssetPaymentSessionServerReport {
        remote_feature_bits,
        session_complete: true,
        ordered_message_kinds,
        input_proof_reassembled_len,
        output_proof_reassembled_len,
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

fn sample_asset_payment_session_messages(
    asset_id: Bytes32,
    asset_amount: u64,
) -> Result<Vec<AssetPeerMessage>, LivePeerError> {
    let pending_channel_id = Bytes32([51; 32]);
    let rfq_id = Bytes32([61; 32]);
    let quote_id = Bytes32([62; 32]);
    let invoice_context = Bytes32([63; 32]);
    let btc_msat = asset_amount
        .checked_mul(10)
        .ok_or_else(|| LivePeerError::Protocol("asset amount too large for quote".to_owned()))?;
    let input_proof = format!(
        "tap-ldk live asset input proof asset={} amount={asset_amount}",
        asset_id.to_hex()
    );
    let output_proof = format!(
        "tap-ldk live asset output proof asset={} amount={asset_amount}",
        asset_id.to_hex()
    );

    let mut messages = Vec::new();
    for chunk in
        ProofChunk::split(input_proof.as_bytes(), 18).map_err(LivePeerError::PeerMessage)?
    {
        messages.push(AssetPeerMessage::TxAssetInputProof {
            pending_channel_id,
            asset_id,
            amount: asset_amount,
            chunk,
        });
    }
    for chunk in
        ProofChunk::split(output_proof.as_bytes(), 18).map_err(LivePeerError::PeerMessage)?
    {
        messages.push(AssetPeerMessage::TxAssetOutputProof {
            pending_channel_id,
            asset_id,
            amount: asset_amount,
            chunk,
        });
    }
    messages.push(AssetPeerMessage::AssetFundingCreated {
        pending_channel_id,
        funding_blob: format!("asset-channel-funding:{}:{asset_amount}", asset_id.to_hex())
            .into_bytes(),
    });
    messages.push(AssetPeerMessage::AssetFundingAccepted {
        pending_channel_id,
        accept: true,
        reject_reason: None,
    });
    messages.push(AssetPeerMessage::RfqRequest {
        rfq_id,
        asset_id,
        asset_amount,
        invoice_context,
    });
    messages.push(AssetPeerMessage::RfqAccept {
        rfq_id,
        quote_id,
        btc_msat,
        expiry_unix_seconds: 1_700_000_600,
        scid_alias: 42,
    });
    messages.push(AssetPeerMessage::AssetHtlcBlob {
        asset_id,
        asset_amount,
        rfq_id,
        invoice_context,
        htlc_blob: format!("asset-htlc-final-hop:{}:{asset_amount}", quote_id.to_hex())
            .into_bytes(),
    });

    Ok(messages)
}

fn custom_payload_digest(message: &AssetPeerMessage) -> Result<Bytes32, LivePeerError> {
    let payload = message.encode().map_err(LivePeerError::PeerMessage)?;
    Ok(Bytes32(Sha256::digest(payload).into()))
}

fn session_payment_id(
    asset_id: Bytes32,
    asset_amount: u64,
    messages: &[AssetPeerMessage],
) -> Result<Bytes32, LivePeerError> {
    let mut hasher = Sha256::new();
    hasher.update(asset_id.0);
    hasher.update(asset_amount.to_be_bytes());
    for message in messages {
        hasher.update(message.message_type().to_be_bytes());
        hasher.update(message.encode().map_err(LivePeerError::PeerMessage)?);
    }
    Ok(Bytes32(hasher.finalize().into()))
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
    SessionComplete {
        payment_id: Bytes32,
        message_count: usize,
    },
    SessionCompleteAck {
        accepted: bool,
        payment_id: Bytes32,
        message_count: usize,
        input_proof_reassembled_len: usize,
        output_proof_reassembled_len: usize,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LivePeerServerReport {
    remote_feature_bits: Vec<u16>,
    custom_message_received: bool,
    custom_message_payload_len: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LiveAssetPaymentSessionServerReport {
    remote_feature_bits: Vec<u16>,
    session_complete: bool,
    ordered_message_kinds: Vec<String>,
    input_proof_reassembled_len: usize,
    output_proof_reassembled_len: usize,
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

    #[test]
    fn live_asset_payment_session_smoke_round_trips_ordered_messages() {
        let report = run_live_asset_payment_session_smoke(Bytes32([8; 32]), 125)
            .expect("live asset payment session");

        assert_eq!(report.status, "completed");
        assert!(report.server_started);
        assert!(report.client_connected);
        assert!(report.negotiated_asset_channel);
        assert!(report.rust_lightning_fork_negotiation_used);
        assert_eq!(
            report.local_feature_bits,
            vec![ASSET_CHANNEL_REQUIRED_FEATURE_BIT]
        );
        assert!(report.message_count >= 7);
        assert!(report.message_reports.iter().all(|message| message.acked));
        assert!(report.input_proof_reassembled_len > 0);
        assert!(report.output_proof_reassembled_len > 0);
        assert_eq!(
            report.ordered_message_kinds.last().map(String::as_str),
            Some("asset_htlc_blob")
        );
        assert!(report.settlement_ack_received);
        assert!(report.native_wire_session_ready);
        assert!(!report.lightning_labs_peer_connected);
        assert!(report.remaining_live_counterparty_gap.is_some());
    }
}
