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
NATIVE_LDK_PEER_STATE_DIR="$ARTIFACT_DIR/native-ldk-litd-peer"
LITD_MINTED_ASSET_REPORT="$ARTIFACT_DIR/lightning-labs-litd-minted-asset.json"
LITD_ASSET_CHANNEL_FUND_REPORT="$ARTIFACT_DIR/lightning-labs-litd-asset-channel-fund.json"
LITD_ASSET_CHANNEL_ACTIVE_REPORT="$ARTIFACT_DIR/lightning-labs-litd-asset-channel-active.json"
LITD_ASSET_PAYMENT_REPORT="$ARTIFACT_DIR/lightning-labs-litd-asset-keysend.json"
LITD_POST_PAYMENT_BALANCE_REPORT="$ARTIFACT_DIR/lightning-labs-litd-post-payment-balance.json"
TAPCHANNEL_FIXTURE_DIR="$ROOT/fixtures/lightning-labs/tapchannelmsg/testdata"
WAIT_TIMEOUT_SECONDS="${TAP_LDK_LL_WAIT_TIMEOUT_SECONDS:-180}"
WAIT_INTERVAL_SECONDS="${TAP_LDK_LL_WAIT_INTERVAL_SECONDS:-2}"
LITD_PEER_HOLD_SECONDS="${TAP_LDK_LL_NATIVE_LDK_HOLD_SECONDS:-240}"
LITD_ASSET_SUPPLY="${TAP_LDK_LL_LITD_ASSET_SUPPLY:-1000000}"
LITD_ASSET_DECIMAL_DISPLAY="${TAP_LDK_LL_LITD_ASSET_DECIMAL_DISPLAY:-2}"
LITD_ASSET_TAG="${TAP_LDK_LL_LITD_ASSET_TAG:-OPENUSD-LITD-$(date +%s)}"
LITD_ASSET_CHANNEL_AMOUNT="${TAP_LDK_LL_LITD_ASSET_CHANNEL_AMOUNT:-}"
LITD_FEE_RATE_SAT_PER_VBYTE="${TAP_LDK_LL_FEE_RATE_SAT_PER_VBYTE:-1}"
LITD_ASSET_PAYMENT_AMOUNT="${TAP_LDK_LL_LITD_ASSET_PAYMENT_AMOUNT:-1}"
LITD_ASSET_PAYMENT_TIMEOUT="${TAP_LDK_LL_LITD_ASSET_PAYMENT_TIMEOUT:-15s}"
LITD_ASSET_CHANNEL_POST_ACTIVE_SETTLE_SECONDS="${TAP_LDK_LL_LITD_ASSET_CHANNEL_POST_ACTIVE_SETTLE_SECONDS:-3}"
NATIVE_LDK_HOLD_PID=""

mkdir -p "$ARTIFACT_DIR" "$LOG_DIR"

cleanup() {
  if [ -n "$NATIVE_LDK_HOLD_PID" ] && kill -0 "$NATIVE_LDK_HOLD_PID" 2>/dev/null; then
    kill "$NATIVE_LDK_HOLD_PID" 2>/dev/null || true
    wait "$NATIVE_LDK_HOLD_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

wait_for_file() {
  local label="$1"
  local file="$2"
  local start now elapsed
  start="$(date +%s)"
  while true; do
    if [ -s "$file" ]; then
      return 0
    fi
    now="$(date +%s)"
    elapsed=$((now - start))
    if [ "$elapsed" -ge "$WAIT_TIMEOUT_SECONDS" ]; then
      echo "timed out waiting for $label after ${WAIT_TIMEOUT_SECONDS}s" >&2
      return 1
    fi
    sleep "$WAIT_INTERVAL_SECONDS"
  done
}

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
  local native_ldk_litd_peer_preflight_gap litd_minted_asset_id litd_minted_asset_supply
  local litd_asset_channel_fund_status litd_asset_channel_fund_exit_code litd_asset_channel_fund_stderr
  local litd_asset_channel_active litd_asset_channel_local_balance litd_asset_channel_active_stderr
  local litd_asset_payment_status litd_asset_payment_exit_code litd_asset_payment_hash litd_asset_payment_wire_status
  local litd_asset_payment_error litd_post_payment_balance
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
  litd_minted_asset_id="$(jq -r '.asset_id // empty' "$LITD_MINTED_ASSET_REPORT" 2>/dev/null || true)"
  litd_minted_asset_supply="$(jq -r '.supply // empty' "$LITD_MINTED_ASSET_REPORT" 2>/dev/null || true)"
  litd_asset_channel_fund_status="$(jq -r '.status // empty' "$LITD_ASSET_CHANNEL_FUND_REPORT" 2>/dev/null || true)"
  litd_asset_channel_fund_exit_code="$(jq -r '.exit_code // empty' "$LITD_ASSET_CHANNEL_FUND_REPORT" 2>/dev/null || true)"
  litd_asset_channel_fund_stderr="$(jq -r '.stderr // empty' "$LITD_ASSET_CHANNEL_FUND_REPORT" 2>/dev/null || true)"
  litd_asset_channel_active="$(jq -r '.usable_for_keysend // empty' "$LITD_ASSET_CHANNEL_ACTIVE_REPORT" 2>/dev/null || true)"
  litd_asset_channel_local_balance="$(jq -r '.local_asset_balance // empty' "$LITD_ASSET_CHANNEL_ACTIVE_REPORT" 2>/dev/null || true)"
  litd_asset_channel_active_stderr="$(cat "$LOG_DIR/lightning-labs-litd-asset-channel-active.err" 2>/dev/null || true)"
  litd_asset_payment_status="$(jq -r '.status // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
  litd_asset_payment_exit_code="$(jq -r '.exit_code // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
  litd_asset_payment_hash="$(jq -r '.payment_hash // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
  litd_asset_payment_wire_status="$(jq -r '.payment_status // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
  litd_asset_payment_error="$(jq -r '.payment_error // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
  litd_post_payment_balance="$(jq -r '.observed_balance // empty' "$LITD_POST_PAYMENT_BALANCE_REPORT" 2>/dev/null || true)"

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
    --arg litd_minted_asset_id "$litd_minted_asset_id" \
    --arg litd_minted_asset_supply "$litd_minted_asset_supply" \
    --arg litd_asset_channel_fund_status "$litd_asset_channel_fund_status" \
    --arg litd_asset_channel_fund_exit_code "$litd_asset_channel_fund_exit_code" \
    --arg litd_asset_channel_fund_stderr "$litd_asset_channel_fund_stderr" \
    --arg litd_asset_channel_active "$litd_asset_channel_active" \
    --arg litd_asset_channel_local_balance "$litd_asset_channel_local_balance" \
    --arg litd_asset_channel_active_stderr "$litd_asset_channel_active_stderr" \
    --arg litd_asset_payment_status "$litd_asset_payment_status" \
    --arg litd_asset_payment_exit_code "$litd_asset_payment_exit_code" \
    --arg litd_asset_payment_hash "$litd_asset_payment_hash" \
    --arg litd_asset_payment_wire_status "$litd_asset_payment_wire_status" \
    --arg litd_asset_payment_error "$litd_asset_payment_error" \
    --arg litd_post_payment_balance "$litd_post_payment_balance" \
    --arg litd_minted_asset_report "$LITD_MINTED_ASSET_REPORT" \
    --arg litd_asset_channel_fund_report "$LITD_ASSET_CHANNEL_FUND_REPORT" \
    --arg litd_asset_channel_active_report "$LITD_ASSET_CHANNEL_ACTIVE_REPORT" \
    --arg litd_asset_payment_report "$LITD_ASSET_PAYMENT_REPORT" \
    --arg litd_post_payment_balance_report "$LITD_POST_PAYMENT_BALANCE_REPORT" \
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
        integrated_litd_minted_asset_report: $litd_minted_asset_report,
        integrated_litd_asset_channel_fund_report: $litd_asset_channel_fund_report,
        integrated_litd_asset_channel_active_report: $litd_asset_channel_active_report,
        integrated_litd_asset_payment_report: $litd_asset_payment_report,
        integrated_litd_post_payment_balance_report: $litd_post_payment_balance_report,
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
      integrated_litd_minted_asset_id: ($litd_minted_asset_id | if length > 0 then . else null end),
      integrated_litd_minted_asset_supply: (if ($litd_minted_asset_supply | test("^[0-9]+$")) then ($litd_minted_asset_supply | tonumber) else null end),
      integrated_litd_asset_channel_fund_status: ($litd_asset_channel_fund_status | if length > 0 then . else null end),
      integrated_litd_asset_channel_fund_exit_code: (if ($litd_asset_channel_fund_exit_code | test("^[0-9]+$")) then ($litd_asset_channel_fund_exit_code | tonumber) else null end),
      integrated_litd_asset_channel_fund_stderr: ($litd_asset_channel_fund_stderr | if length > 0 then . else null end),
      integrated_litd_asset_channel_usable_for_keysend: ($litd_asset_channel_active == "true"),
      integrated_litd_asset_channel_local_balance: (if ($litd_asset_channel_local_balance | test("^[0-9]+$")) then ($litd_asset_channel_local_balance | tonumber) else null end),
      integrated_litd_asset_channel_active_stderr: ($litd_asset_channel_active_stderr | if length > 0 then . else null end),
      integrated_litd_asset_payment_status: ($litd_asset_payment_status | if length > 0 then . else null end),
      integrated_litd_asset_payment_exit_code: (if ($litd_asset_payment_exit_code | test("^[0-9]+$")) then ($litd_asset_payment_exit_code | tonumber) else null end),
      integrated_litd_asset_payment_hash: ($litd_asset_payment_hash | if length > 0 then . else null end),
      integrated_litd_asset_payment_wire_status: ($litd_asset_payment_wire_status | if length > 0 then . else null end),
      integrated_litd_asset_payment_error: ($litd_asset_payment_error | if length > 0 then . else null end),
      integrated_litd_post_payment_balance: (if ($litd_post_payment_balance | test("^[0-9]+$")) then ($litd_post_payment_balance | tonumber) else null end),
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

if ! ./scripts/lightning-labs-litd-counterparty.sh mint-asset \
  "$LITD_ASSET_TAG" \
  "$LITD_ASSET_SUPPLY" \
  "$LITD_ASSET_DECIMAL_DISPLAY" \
  >"$LITD_MINTED_ASSET_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-minted-asset.err"; then
  reason="$(cat "$LOG_DIR/lightning-labs-litd-minted-asset.err")"
  write_report "blocked" "integrated_litd_asset_mint" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

if [ -z "$LITD_ASSET_CHANNEL_AMOUNT" ]; then
  LITD_ASSET_CHANNEL_AMOUNT="$native_asset_amount"
fi

rm -f "$LITD_PEER_PREFLIGHT_REPORT"
rm -rf "$NATIVE_LDK_PEER_STATE_DIR"
cargo run -q -p tap-ldk-cli -- live-litd-peer-hold \
  "$LITD_PEER_PREFLIGHT_REPORT" \
  "$NATIVE_LDK_PEER_STATE_DIR" \
  "$litd_peer_pubkey" \
  "$litd_peer_address" \
  "$LITD_PEER_HOLD_SECONDS" \
  >"$LOG_DIR/native-ldk-litd-peer-preflight.out" \
  2>"$LOG_DIR/native-ldk-litd-peer-preflight.err" &
NATIVE_LDK_HOLD_PID=$!

if ! wait_for_file "native LDK litd peer hold report" "$LITD_PEER_PREFLIGHT_REPORT"; then
  reason="$(cat "$LOG_DIR/native-ldk-litd-peer-preflight.err")"
  write_report "blocked" "native_ldk_litd_peer_preflight" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

if [ "$(jq -r '.peer_connected // false' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)" != "true" ]; then
  reason="$(jq -r '.remaining_asset_channel_gap // "native LDK peer did not connect to litd"' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
  write_report "blocked" "native_ldk_litd_peer_preflight" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

litd_minted_asset_id="$(jq -r '.asset_id // empty' "$LITD_MINTED_ASSET_REPORT" 2>/dev/null || true)"
native_litd_node_id="$(jq -r '.native_node_id // empty' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)"
if [ -z "$litd_minted_asset_id" ] || [ -z "$native_litd_node_id" ]; then
  write_report "blocked" "live_asset_channel_funding" "live asset-channel funding could not start because the litd minted asset id or native LDK node id was missing"
  cat "$REPORT_PATH"
  exit 0
fi

./scripts/lightning-labs-litd-counterparty.sh fund-asset-channel \
  "$native_litd_node_id" \
  "$litd_minted_asset_id" \
  "$LITD_ASSET_CHANNEL_AMOUNT" \
  "$LITD_FEE_RATE_SAT_PER_VBYTE" \
  0 >"$LITD_ASSET_CHANNEL_FUND_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-asset-channel-fund.err" || true

if [ "$(jq -r '.status // empty' "$LITD_ASSET_CHANNEL_FUND_REPORT" 2>/dev/null || true)" != "completed" ]; then
  reason="$(jq -r '.stderr // .stdout // "integrated litd fundchannel did not complete"' "$LITD_ASSET_CHANNEL_FUND_REPORT" 2>/dev/null || true)"
  write_report "blocked" "live_asset_channel_funding" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

./scripts/lightning-labs-litd-counterparty.sh mine 6 \
  >"$LOG_DIR/lightning-labs-litd-post-fund-mine.out" \
  2>"$LOG_DIR/lightning-labs-litd-post-fund-mine.err" || true

./scripts/lightning-labs-litd-counterparty.sh wait-asset-channel-active \
  "$native_litd_node_id" \
  "$litd_minted_asset_id" \
  "$LITD_ASSET_PAYMENT_AMOUNT" >"$LITD_ASSET_CHANNEL_ACTIVE_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-asset-channel-active.err" || true

if [ "$(jq -r '.usable_for_keysend // false' "$LITD_ASSET_CHANNEL_ACTIVE_REPORT" 2>/dev/null || true)" != "true" ]; then
  reason="$(cat "$LOG_DIR/lightning-labs-litd-asset-channel-active.err" 2>/dev/null || true)"
  if [ -z "$reason" ]; then
    reason="integrated litd asset channel did not become active with spendable local asset balance"
  fi
  write_report "blocked" "live_asset_channel_active" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

if [ "$LITD_ASSET_CHANNEL_POST_ACTIVE_SETTLE_SECONDS" != "0" ]; then
  sleep "$LITD_ASSET_CHANNEL_POST_ACTIVE_SETTLE_SECONDS"
fi

./scripts/lightning-labs-litd-counterparty.sh send-asset-keysend \
  "$native_litd_node_id" \
  "$litd_minted_asset_id" \
  "$LITD_ASSET_PAYMENT_AMOUNT" \
  "$LITD_ASSET_PAYMENT_TIMEOUT" >"$LITD_ASSET_PAYMENT_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-asset-keysend.err" || true

./scripts/lightning-labs-litd-counterparty.sh mine 1 \
  >"$LOG_DIR/lightning-labs-litd-post-payment-mine.out" \
  2>"$LOG_DIR/lightning-labs-litd-post-payment-mine.err" || true

./scripts/lightning-labs-litd-counterparty.sh balance "$litd_minted_asset_id" \
  >"$LITD_POST_PAYMENT_BALANCE_REPORT" \
  2>"$LOG_DIR/lightning-labs-litd-post-payment-balance.err" || true

litd_asset_payment_status="$(jq -r '.status // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
litd_asset_payment_wire_status="$(jq -r '.payment_status // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"
litd_asset_payment_error="$(jq -r '.payment_error // empty' "$LITD_ASSET_PAYMENT_REPORT" 2>/dev/null || true)"

final_reason="The live tapd proof can be bound, the native outgoing RFQ/HTLC artifacts are ready, integrated litd minted a real asset, the fork-backed native LDK node stayed connected to litd, litd completed fundchannel, and the harness now attempted a real litd asset keysend. #81 still needs the payment to settle and the native receiver asset-balance check to pass."
if [ "$litd_asset_payment_status" = "completed" ]; then
  final_reason="The integrated litd asset keysend reported SUCCEEDED after live fundchannel. #81 still needs native tap-ldk to expose and verify the receiver-side asset balance durably before this can be closed."
elif [ -n "$litd_asset_payment_wire_status" ]; then
  final_reason="The integrated litd asset keysend was attempted after live fundchannel but did not settle yet; latest LND payment status is $litd_asset_payment_wire_status. #81 still needs the remaining payment-settlement and native receiver-balance work."
elif [ -n "$litd_asset_payment_error" ]; then
  final_reason="The integrated litd asset keysend was attempted after live fundchannel but returned a payment error: $litd_asset_payment_error. #81 still needs the remaining payment-settlement and native receiver-balance work."
fi
if [ "$(jq -r '.litd_peer_supports_taproot_asset_channel // false' "$LITD_PEER_PREFLIGHT_REPORT" 2>/dev/null || true)" != "true" ]; then
  final_reason="The live tapd proof can be bound and the native outgoing RFQ/HTLC artifacts now include an ordered native asset-payment wire session, current tapd balance observation, an integrated litd counterparty with asset-channel RPCs ready, a fork-backed ldk-node connection to litd, remote taproot feature observation, and fork-backed Taproot Asset message/channel/payment APIs. The connected litd peer did not advertise the Taproot Asset channel feature, so #81 still needs compatible feature negotiation before live asset-channel funding/payment and the post-settlement receiver-balance check."
fi

write_report \
  "blocked" \
  "live_asset_channel_payment_settlement" \
  "$final_reason"

cat "$REPORT_PATH"
