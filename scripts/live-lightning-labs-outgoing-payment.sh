#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "live-lightning-labs-outgoing-payment: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

REPORT_PATH="${1:-$ROOT/target/live-lightning-labs-outgoing-payment/report.json}"
WALLET_PATH="${2:-$ROOT/target/live-lightning-labs-outgoing-payment/tap-ldk-wallet.json}"
ARTIFACT_DIR="$(dirname "$REPORT_PATH")"
LOG_DIR="$ARTIFACT_DIR/logs"
OUTGOING_STORE="${3:-$ARTIFACT_DIR/lightning-labs-outgoing-payment-store.json}"
BOUNDED_REPORT="${4:-$ARTIFACT_DIR/lightning-labs-outgoing-payment-report.json}"
PROOF_BINDING_REPORT="$ARTIFACT_DIR/live-tapd-proof-binding.json"
NATIVE_SESSION_REPORT="$ARTIFACT_DIR/live-native-asset-payment-session.json"
CURRENT_BALANCE_REPORT="$ARTIFACT_DIR/lightning-labs-current-receiver-balance.json"
LITD_COUNTERPARTY_REPORT="$ARTIFACT_DIR/lightning-labs-litd-counterparty-ready.json"
LITD_PEER_PREFLIGHT_REPORT="$ARTIFACT_DIR/native-ldk-litd-peer-preflight.json"
TAPCHANNEL_FIXTURE_DIR="$ROOT/fixtures/lightning-labs/tapchannelmsg/testdata"

mkdir -p "$ARTIFACT_DIR" "$LOG_DIR"

write_report() {
  local status="$1"
  local blocked_step="$2"
  local reason="$3"

  local payment_id asset_id asset_amount quote_id expected_sender_after
  local expected_receiver_after wrong_asset wrong_amount quote_replay proof_status native_session_status current_observed_balance
  local litd_topology litd_identity_pubkey litd_asset_channel_rpc_ready native_litd_peer_connected native_litd_node_id
  local live_node_runtime live_node_uses_fork openagents_rust_lightning_rev fork_asset_channel_hooks_reachable
  local live_node_asset_custom_message_api_ready live_node_asset_channel_open_api_ready live_node_asset_payment_api_ready
  local live_node_asset_runtime_event_count asset_channel_settlement_ready
  local litd_peer_supports_simple_taproot_staging litd_peer_supports_taproot_asset_channel
  local native_ldk_litd_peer_preflight_gap
  payment_id="$(jq -r '.payment_id // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  asset_id="$(jq -r '.asset_id // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  asset_amount="$(jq -r '.asset_amount // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  quote_id="$(jq -r '.quote_id // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  expected_sender_after="$(jq -r '.expected_sender_balance_after // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  expected_receiver_after="$(jq -r '.expected_lightning_labs_receiver_balance_after // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  wrong_asset="$(jq -r '.wrong_asset_rejected // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  wrong_amount="$(jq -r '.wrong_amount_rejected // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  quote_replay="$(jq -r '.quote_replay_rejected // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
  proof_status="$(jq -r '.status // empty' "$PROOF_BINDING_REPORT" 2>/dev/null || true)"
  native_session_status="$(jq -r '.status // empty' "$NATIVE_SESSION_REPORT" 2>/dev/null || true)"
  current_observed_balance="$(jq -r '.observed_balance // empty' "$CURRENT_BALANCE_REPORT" 2>/dev/null || true)"
  litd_topology="$(jq -r '.counterparty_topology // empty' "$LITD_COUNTERPARTY_REPORT" 2>/dev/null || true)"
  litd_identity_pubkey="$(jq -r '.litd.identity_pubkey // empty' "$LITD_COUNTERPARTY_REPORT" 2>/dev/null || true)"
  litd_asset_channel_rpc_ready="$(jq -r '.litd.asset_channel_rpc_ready // empty' "$LITD_COUNTERPARTY_REPORT" 2>/dev/null || true)"
  native_litd_peer_connected="$(jq -r '.peer_connected // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  native_litd_node_id="$(jq -r '.native_node_id // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_runtime="$(jq -r '.live_node_runtime // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_uses_fork="$(jq -r '.live_node_uses_openagents_rust_lightning_fork // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  openagents_rust_lightning_rev="$(jq -r '.openagents_rust_lightning_rev // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  fork_asset_channel_hooks_reachable="$(jq -r '.fork_asset_channel_hooks_reachable_from_live_node // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_asset_custom_message_api_ready="$(jq -r '.live_node_asset_custom_message_api_ready // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_asset_channel_open_api_ready="$(jq -r '.live_node_asset_channel_open_api_ready // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_asset_payment_api_ready="$(jq -r '.live_node_asset_payment_api_ready // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  live_node_asset_runtime_event_count="$(jq -r '.live_node_asset_runtime_event_count // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  asset_channel_settlement_ready="$(jq -r '.asset_channel_settlement_ready // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  litd_peer_supports_simple_taproot_staging="$(jq -r '.litd_peer_supports_simple_taproot_staging // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  litd_peer_supports_taproot_asset_channel="$(jq -r '.litd_peer_supports_taproot_asset_channel // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  native_ldk_litd_peer_preflight_gap="$(jq -r '.remaining_asset_channel_gap // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"

  jq -n \
    --arg source "live-lightning-labs-outgoing-payment" \
    --arg status "$status" \
    --arg blocked_step "$blocked_step" \
    --arg reason "$reason" \
    --arg payment_id "$payment_id" \
    --arg asset_id "$asset_id" \
    --arg asset_amount "$asset_amount" \
    --arg quote_id "$quote_id" \
    --arg expected_sender_after "$expected_sender_after" \
    --arg expected_receiver_after "$expected_receiver_after" \
    --arg wrong_asset "$wrong_asset" \
    --arg wrong_amount "$wrong_amount" \
    --arg quote_replay "$quote_replay" \
    --arg proof_status "$proof_status" \
    --arg native_session_status "$native_session_status" \
    --arg current_observed_balance "$current_observed_balance" \
    --arg proof_binding_report "$PROOF_BINDING_REPORT" \
    --arg native_session_report "$NATIVE_SESSION_REPORT" \
    --arg current_balance_report "$CURRENT_BALANCE_REPORT" \
    --arg litd_counterparty_report "$LITD_COUNTERPARTY_REPORT" \
    --arg litd_peer_preflight_report "$LITD_PEER_PREFLIGHT_REPORT" \
    --arg bounded_report "$BOUNDED_REPORT" \
    --arg outgoing_store "$OUTGOING_STORE" \
    --arg litd_topology "$litd_topology" \
    --arg litd_identity_pubkey "$litd_identity_pubkey" \
    --arg litd_asset_channel_rpc_ready "$litd_asset_channel_rpc_ready" \
    --arg native_litd_peer_connected "$native_litd_peer_connected" \
    --arg native_litd_node_id "$native_litd_node_id" \
    --arg live_node_runtime "$live_node_runtime" \
    --arg live_node_uses_fork "$live_node_uses_fork" \
    --arg openagents_rust_lightning_rev "$openagents_rust_lightning_rev" \
    --arg fork_asset_channel_hooks_reachable "$fork_asset_channel_hooks_reachable" \
    --arg live_node_asset_custom_message_api_ready "$live_node_asset_custom_message_api_ready" \
    --arg live_node_asset_channel_open_api_ready "$live_node_asset_channel_open_api_ready" \
    --arg live_node_asset_payment_api_ready "$live_node_asset_payment_api_ready" \
    --arg live_node_asset_runtime_event_count "$live_node_asset_runtime_event_count" \
    --arg asset_channel_settlement_ready "$asset_channel_settlement_ready" \
    --arg litd_peer_supports_simple_taproot_staging "$litd_peer_supports_simple_taproot_staging" \
    --arg litd_peer_supports_taproot_asset_channel "$litd_peer_supports_taproot_asset_channel" \
    --arg native_ldk_litd_peer_preflight_gap "$native_ldk_litd_peer_preflight_gap" \
    '{
      schema_version: 1,
      source: $source,
      status: $status,
      blocked_step: $blocked_step,
      reason: $reason,
      fixture_only_path: false,
      tap_ldk_sender: "native tap-ldk",
      lightning_labs_receiver: "independent Lightning Labs integrated litd counterparty",
      payment_id: ($payment_id | if length > 0 then . else null end),
      asset_id: ($asset_id | if length > 0 then . else null end),
      asset_amount: (if ($asset_amount | test("^[0-9]+$")) then ($asset_amount | tonumber) else null end),
      quote_id: ($quote_id | if length > 0 then . else null end),
      expected_sender_balance_after: (if ($expected_sender_after | test("^[0-9]+$")) then ($expected_sender_after | tonumber) else null end),
      expected_lightning_labs_receiver_balance_after: (if ($expected_receiver_after | test("^[0-9]+$")) then ($expected_receiver_after | tonumber) else null end),
      observed_lightning_labs_receiver_balance_after: null,
      observed_lightning_labs_receiver_current_balance: (if ($current_observed_balance | test("^[0-9]+$")) then ($current_observed_balance | tonumber) else null end),
      observed_live_balance: false,
      failure_checks: {
        quote_replay_rejected: ($quote_replay == "true"),
        wrong_asset_rejected: ($wrong_asset == "true"),
        wrong_amount_rejected: ($wrong_amount == "true")
      },
      artifacts: {
        proof_binding_report: $proof_binding_report,
        native_asset_payment_session_report: $native_session_report,
        current_receiver_balance_report: $current_balance_report,
        integrated_litd_counterparty_report: $litd_counterparty_report,
        native_ldk_litd_peer_preflight_report: $litd_peer_preflight_report,
        bounded_outgoing_payment_report: $bounded_report,
        outgoing_payment_store: $outgoing_store
      },
      proof_binding_status: ($proof_status | if length > 0 then . else null end),
      native_asset_payment_session_ready: ($native_session_status == "completed"),
      integrated_litd_counterparty_ready: ($litd_asset_channel_rpc_ready == "true"),
      integrated_litd_counterparty_topology: ($litd_topology | if length > 0 then . else null end),
      integrated_litd_identity_pubkey: ($litd_identity_pubkey | if length > 0 then . else null end),
      native_litd_peer_connected: ($native_litd_peer_connected == "true"),
      native_ldk_node_id: ($native_litd_node_id | if length > 0 then . else null end),
      live_node_runtime: ($live_node_runtime | if length > 0 then . else null end),
      live_node_uses_openagents_rust_lightning_fork: ($live_node_uses_fork == "true"),
      openagents_rust_lightning_rev: ($openagents_rust_lightning_rev | if length > 0 then . else null end),
      fork_asset_channel_hooks_reachable_from_live_node: ($fork_asset_channel_hooks_reachable == "true"),
      live_node_asset_custom_message_api_ready: ($live_node_asset_custom_message_api_ready == "true"),
      live_node_asset_channel_open_api_ready: ($live_node_asset_channel_open_api_ready == "true"),
      live_node_asset_payment_api_ready: ($live_node_asset_payment_api_ready == "true"),
      live_node_asset_runtime_event_count: (if ($live_node_asset_runtime_event_count | test("^[0-9]+$")) then ($live_node_asset_runtime_event_count | tonumber) else null end),
      litd_peer_supports_simple_taproot_staging: ($litd_peer_supports_simple_taproot_staging == "true"),
      litd_peer_supports_taproot_asset_channel: ($litd_peer_supports_taproot_asset_channel == "true"),
      asset_channel_settlement_ready: ($asset_channel_settlement_ready == "true"),
      native_ldk_litd_peer_preflight_gap: ($native_ldk_litd_peer_preflight_gap | if length > 0 then . else null end),
      issue_57_acceptance_met: false,
      next_required_work: [
        "replace the loopback native payment-session message exchange with the connected independent Lightning Labs litd peer",
        "drive the fork-backed ldk-node Taproot Asset channel-open API over the connected independent litd peer instead of the current synthetic API preflight",
        "send the asset payment through the live litd asset-channel settlement path",
        "record the post-settlement Lightning Labs receiver balance and compare it to the expected delta"
      ]
    }' >"$REPORT_PATH"
}

if [ ! -f "$BOUNDED_REPORT" ]; then
  if ! cargo run -q -p tap-ldk-cli -- lightning-labs-outgoing-payment-smoke \
    "$TAPCHANNEL_FIXTURE_DIR" "$OUTGOING_STORE" >"$BOUNDED_REPORT" 2>"$LOG_DIR/bounded-outgoing-payment.err"; then
    reason="$(cat "$LOG_DIR/bounded-outgoing-payment.err")"
    write_report "blocked" "bounded_outgoing_payment_artifacts" "$reason"
    cat "$REPORT_PATH"
    exit 0
  fi
fi

native_asset_id="$(jq -r '.asset_id // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
native_asset_amount="$(jq -r '.asset_amount // empty' "$BOUNDED_REPORT" 2>/dev/null || true)"
if [ -z "$native_asset_id" ] || [ -z "$native_asset_amount" ]; then
  write_report "blocked" "native_asset_payment_session" "bounded outgoing payment report did not contain asset id and amount"
  cat "$REPORT_PATH"
  exit 0
fi

if ! cargo run -q -p tap-ldk-cli -- live-asset-payment-session-smoke \
  "$NATIVE_SESSION_REPORT" "$native_asset_id" "$native_asset_amount" \
  >"$LOG_DIR/live-native-asset-payment-session.out" \
  2>"$LOG_DIR/live-native-asset-payment-session.err"; then
  reason="$(cat "$LOG_DIR/live-native-asset-payment-session.err")"
  write_report "blocked" "native_asset_payment_session" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

if ! ./scripts/live-tapd-proof-bind.sh "$PROOF_BINDING_REPORT" "$WALLET_PATH" \
  >"$LOG_DIR/live-tapd-proof-binding.out" \
  2>"$LOG_DIR/live-tapd-proof-binding.err"; then
  reason="$(cat "$LOG_DIR/live-tapd-proof-binding.err")"
  write_report "blocked" "live_tapd_proof_binding" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

proof_status="$(jq -r '.status // empty' "$PROOF_BINDING_REPORT" 2>/dev/null || true)"
if [ "$proof_status" = "blocked" ]; then
  reason="$(jq -r '.reason // "live tapd proof binding did not complete"' "$PROOF_BINDING_REPORT")"
  write_report "blocked" "live_tapd_proof_binding" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

live_asset_id="$(jq -r '.asset_id // empty' "$PROOF_BINDING_REPORT" 2>/dev/null || true)"
live_asset_amount="$(jq -r '.amount // empty' "$PROOF_BINDING_REPORT" 2>/dev/null || true)"
if [ -n "$live_asset_id" ] && [ -n "$live_asset_amount" ]; then
  if ! cargo run -q -p tap-ldk-cli -- live-asset-payment-session-smoke \
    "$NATIVE_SESSION_REPORT" "$live_asset_id" "$live_asset_amount" \
    >"$LOG_DIR/live-native-asset-payment-session-live-proof.out" \
    2>"$LOG_DIR/live-native-asset-payment-session-live-proof.err"; then
    reason="$(cat "$LOG_DIR/live-native-asset-payment-session-live-proof.err")"
    write_report "blocked" "native_asset_payment_session_live_proof" "$reason"
    cat "$REPORT_PATH"
    exit 0
  fi

  ./scripts/lightning-labs-counterparty.sh tapd-balance "$live_asset_id" \
    >"$CURRENT_BALANCE_REPORT" \
    2>"$LOG_DIR/lightning-labs-current-receiver-balance.err" || true
fi

if ! ./scripts/lightning-labs-litd-counterparty.sh start \
  >"$LITD_COUNTERPARTY_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-counterparty.err"; then
  reason="$(cat "$LOG_DIR/lightning-labs-litd-counterparty.err")"
  write_report "blocked" "integrated_litd_counterparty" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

litd_peer_pubkey="$(jq -r '.litd.identity_pubkey // empty' "$LITD_COUNTERPARTY_REPORT" 2>/dev/null || true)"
litd_peer_address="$(jq -r '.litd.p2p_url // empty' "$LITD_COUNTERPARTY_REPORT" 2>/dev/null || true)"
if [ -z "$litd_peer_pubkey" ] || [ -z "$litd_peer_address" ]; then
  write_report "blocked" "native_ldk_litd_peer_preflight" "integrated litd readiness report did not include litd identity pubkey and P2P address"
  cat "$REPORT_PATH"
  exit 0
fi

if ! cargo run -q -p tap-ldk-cli -- live-litd-peer-preflight \
  "$LITD_PEER_PREFLIGHT_REPORT" \
  "$ARTIFACT_DIR/native-ldk-litd-peer" \
  "$litd_peer_pubkey" \
  "$litd_peer_address" \
  >"$LOG_DIR/native-ldk-litd-peer-preflight.out" \
  2>"$LOG_DIR/native-ldk-litd-peer-preflight.err"; then
  reason="$(cat "$LOG_DIR/native-ldk-litd-peer-preflight.err")"
  write_report "blocked" "native_ldk_litd_peer_preflight" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

final_reason="The live tapd proof can be bound and the native outgoing RFQ/HTLC artifacts now include an ordered native asset-payment wire session, current tapd balance observation, an integrated litd counterparty with asset-channel RPCs ready, a fork-backed ldk-node connection to litd, confirmed remote simple-taproot/Taproot Asset feature support, and fork-backed Taproot Asset message/channel/payment APIs. #81 still needs live asset-channel funding/payment over the connected litd peer and the post-settlement receiver-balance check."
if [ "$(jq -r '.litd_peer_supports_taproot_asset_channel // false' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)" != "true" ]; then
  final_reason="The live tapd proof can be bound and the native outgoing RFQ/HTLC artifacts now include an ordered native asset-payment wire session, current tapd balance observation, an integrated litd counterparty with asset-channel RPCs ready, a fork-backed ldk-node connection to litd, remote taproot feature observation, and fork-backed Taproot Asset message/channel/payment APIs. The connected litd peer did not advertise the Taproot Asset channel feature, so #81 still needs compatible feature negotiation before live asset-channel funding/payment and the post-settlement receiver-balance check."
fi

write_report \
  "blocked" \
  "live_asset_channel_payment_settlement" \
  "$final_reason"

cat "$REPORT_PATH"
