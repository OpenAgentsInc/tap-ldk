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
TAPCHANNEL_FIXTURE_DIR="$ROOT/fixtures/lightning-labs/tapchannelmsg/testdata"

mkdir -p "$ARTIFACT_DIR" "$LOG_DIR"

write_report() {
  local status="$1"
  local blocked_step="$2"
  local reason="$3"

  local payment_id asset_id asset_amount quote_id expected_sender_after
  local expected_receiver_after wrong_asset wrong_amount quote_replay proof_status native_session_status
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
    --arg proof_binding_report "$PROOF_BINDING_REPORT" \
    --arg native_session_report "$NATIVE_SESSION_REPORT" \
    --arg bounded_report "$BOUNDED_REPORT" \
    --arg outgoing_store "$OUTGOING_STORE" \
    '{
      schema_version: 1,
      source: $source,
      status: $status,
      blocked_step: $blocked_step,
      reason: $reason,
      fixture_only_path: false,
      tap_ldk_sender: "native tap-ldk",
      lightning_labs_receiver: "independent LND/tapd counterparty",
      payment_id: ($payment_id | if length > 0 then . else null end),
      asset_id: ($asset_id | if length > 0 then . else null end),
      asset_amount: (if ($asset_amount | test("^[0-9]+$")) then ($asset_amount | tonumber) else null end),
      quote_id: ($quote_id | if length > 0 then . else null end),
      expected_sender_balance_after: (if ($expected_sender_after | test("^[0-9]+$")) then ($expected_sender_after | tonumber) else null end),
      expected_lightning_labs_receiver_balance_after: (if ($expected_receiver_after | test("^[0-9]+$")) then ($expected_receiver_after | tonumber) else null end),
      observed_lightning_labs_receiver_balance_after: null,
      observed_live_balance: false,
      failure_checks: {
        quote_replay_rejected: ($quote_replay == "true"),
        wrong_asset_rejected: ($wrong_asset == "true"),
        wrong_amount_rejected: ($wrong_amount == "true")
      },
      artifacts: {
        proof_binding_report: $proof_binding_report,
        native_asset_payment_session_report: $native_session_report,
        bounded_outgoing_payment_report: $bounded_report,
        outgoing_payment_store: $outgoing_store
      },
      proof_binding_status: ($proof_status | if length > 0 then . else null end),
      native_asset_payment_session_ready: ($native_session_status == "completed"),
      issue_57_acceptance_met: false,
      next_required_work: [
        "replace the loopback native payment-session peer with the independent Lightning Labs peer",
        "drive native LDK asset-channel funding against the Lightning Labs peer",
        "query Lightning Labs receiver balance after settlement and replace the expected-only balance"
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

write_report \
  "blocked" \
  "live_asset_channel_payment_settlement" \
  "The live tapd proof can be bound and the native outgoing RFQ/HTLC artifacts now include an ordered native asset-payment wire session, but this repo does not yet replace that loopback session with the independent Lightning Labs peer and observed receiver-balance settlement."

cat "$REPORT_PATH"
