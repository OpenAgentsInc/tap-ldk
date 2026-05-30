#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "path-a-native-demo: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_PATH_A_ARTIFACT_DIR:-$ROOT/target/path-a-native-demo/$STAMP}"
LOG_DIR="$ARTIFACT_DIR/logs"
ALICE_WALLET="$ARTIFACT_DIR/alice-wallet.json"
BOB_WALLET="$ARTIFACT_DIR/bob-wallet.json"
BOB_RESTART_WALLET="$ARTIFACT_DIR/bob-wallet-after-restart.json"
BOB_PROOF="$ARTIFACT_DIR/bob-openusd-proof.tlv"
BOB_PROOF_BUNDLE="$ARTIFACT_DIR/bob-openusd-proof-bundle.json"
CHANNEL_STORE="$ARTIFACT_DIR/asset-channels.json"
COMMITMENT_STORE="$ARTIFACT_DIR/asset-commitments.json"
SUMMARY="$ARTIFACT_DIR/summary.txt"
CLOSE_REPORT="$ARTIFACT_DIR/native-close.json"
CLOSE_LOCAL_PROOF_HEX="$ARTIFACT_DIR/native-close-local-proof.hex"
CLOSE_REMOTE_PROOF_HEX="$ARTIFACT_DIR/native-close-remote-proof.hex"
CLOSE_RECOVERY_STATUS="$ARTIFACT_DIR/close-recovery-status.json"
ONCHAIN_LIFECYCLE_REPORT="$ARTIFACT_DIR/onchain-lifecycle.json"
CHAIN_OBSERVATION_REPORT="$ARTIFACT_DIR/chain-watcher-lifecycle.json"

ISSUER_KEY="02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"
BOB_KEY="03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f"

mkdir -p "$LOG_DIR"

run_log() {
  local name="$1"
  shift
  echo "path-a-native-demo: $name"
  "$@" >"$LOG_DIR/$name.out" 2>"$LOG_DIR/$name.err"
}

run_json() {
  local name="$1"
  local output="$2"
  shift 2
  echo "path-a-native-demo: $name"
  "$@" >"$output" 2>"$LOG_DIR/$name.err"
}

run_text_capture() {
  local name="$1"
  shift
  echo "path-a-native-demo: $name"
  "$@" 2>"$LOG_DIR/$name.err" | tee "$LOG_DIR/$name.out"
}

json_string_field() {
  local file="$1"
  local field="$2"
  sed -n "s/.*\"$field\": \"\\([^\"]*\\)\".*/\\1/p" "$file" | head -n 1
}

json_scalar_field() {
  local file="$1"
  local field="$2"
  sed -n "s/.*\"$field\": \\([^,}]*\\).*/\\1/p" "$file" | head -n 1
}

capture_close_artifacts() {
  local local_proof
  local remote_proof
  local force_close_status
  local restart_after_close_matches
  local obsolete_proof_rejected
  local failed_sweep_not_reported_recovered

  local_proof="$(json_string_field "$CLOSE_REPORT" local_proof_tlv_hex)"
  remote_proof="$(json_string_field "$CLOSE_REPORT" remote_proof_tlv_hex)"
  force_close_status="$(json_string_field "$CLOSE_REPORT" force_close_status)"
  restart_after_close_matches="$(json_scalar_field "$CLOSE_REPORT" restart_after_close_matches)"
  obsolete_proof_rejected="$(json_scalar_field "$CLOSE_REPORT" obsolete_proof_rejected)"
  failed_sweep_not_reported_recovered="$(json_scalar_field "$CLOSE_REPORT" failed_sweep_not_reported_recovered)"

  if [ -z "$local_proof" ] || [ -z "$remote_proof" ] || [ -z "$force_close_status" ]; then
    echo "path-a-native-demo: failed to extract close proof artifacts." >&2
    exit 1
  fi

  printf '%s\n' "$local_proof" >"$CLOSE_LOCAL_PROOF_HEX"
  printf '%s\n' "$remote_proof" >"$CLOSE_REMOTE_PROOF_HEX"
  cat >"$CLOSE_RECOVERY_STATUS" <<STATUS_JSON
{
  "cooperative_close_report": "$CLOSE_REPORT",
  "local_proof_hex": "$CLOSE_LOCAL_PROOF_HEX",
  "remote_proof_hex": "$CLOSE_REMOTE_PROOF_HEX",
  "force_close_status": "$force_close_status",
  "force_close_supported": false,
  "force_close_deferred_reason": "native force-close and sweep recovery are explicitly deferred; the demo must not claim force-close support",
  "restart_after_close_matches": $restart_after_close_matches,
  "obsolete_proof_rejected": $obsolete_proof_rejected,
  "failed_sweep_not_reported_recovered": $failed_sweep_not_reported_recovered
}
STATUS_JSON
}

echo "path-a-native-demo: artifacts=$ARTIFACT_DIR"

run_log regtest-start ./scripts/regtest-bitcoin.sh start
run_log regtest-mine ./scripts/regtest-bitcoin.sh mine 1

run_text_capture alice-wallet-init cargo run -q -p tap-ldk-cli -- wallet-init "$ALICE_WALLET" >/dev/null
run_text_capture bob-wallet-init cargo run -q -p tap-ldk-cli -- wallet-init "$BOB_WALLET" >/dev/null

ISSUE_OUT="$(run_text_capture alice-issue-openusd cargo run -q -p tap-ldk-cli -- wallet-issue-openusd "$ALICE_WALLET" 1000 "$ISSUER_KEY")"
ASSET_ID="$(printf '%s\n' "$ISSUE_OUT" | sed -n 's/.*asset_id=\([0-9a-f]*\).*/\1/p')"
if [ -z "$ASSET_ID" ]; then
  echo "path-a-native-demo: failed to parse issued asset_id." >&2
  exit 1
fi

run_text_capture proof-courier-local-send cargo run -q -p tap-ldk-cli -- wallet-send-local "$ALICE_WALLET" "$ASSET_ID" 300 "$BOB_KEY" "$BOB_PROOF" >/dev/null
run_text_capture bob-import-openusd-proof cargo run -q -p tap-ldk-cli -- wallet-import-proof-file "$BOB_WALLET" "$BOB_PROOF" >/dev/null
BOB_PROOF_ID="$(cargo run -q -p tap-ldk-cli -- wallet-proofs "$BOB_WALLET" 2>"$LOG_DIR/bob-wallet-proofs.err" | tee "$LOG_DIR/bob-wallet-proofs.out" | head -n 1)"
if [ -z "$BOB_PROOF_ID" ]; then
  echo "path-a-native-demo: failed to parse bob proof id." >&2
  exit 1
fi
run_text_capture bob-export-proof-bundle cargo run -q -p tap-ldk-cli -- wallet-export-proof-bundle "$BOB_WALLET" "$BOB_PROOF_ID" "$BOB_PROOF_BUNDLE" >/dev/null
run_json alice-wallet-balances "$ARTIFACT_DIR/alice-wallet-balances.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$ALICE_WALLET"
run_json bob-wallet-balances "$ARTIFACT_DIR/bob-wallet-balances.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$BOB_WALLET"

run_json asset-channel-funding "$ARTIFACT_DIR/asset-channel-funding.json" cargo run -q -p tap-ldk-cli -- asset-channel-funding-smoke "$CHANNEL_STORE"
run_json asset-commitment "$ARTIFACT_DIR/asset-commitment.json" cargo run -q -p tap-ldk-cli -- asset-commitment-smoke "$COMMITMENT_STORE"
run_json native-payment "$ARTIFACT_DIR/native-payment.json" cargo run -q -p tap-ldk-cli -- asset-payment-smoke
run_json native-recovery "$ARTIFACT_DIR/native-recovery.json" cargo run -q -p tap-ldk-cli -- asset-recovery-smoke
run_json native-close "$CLOSE_REPORT" cargo run -q -p tap-ldk-cli -- asset-close-smoke
run_json onchain-lifecycle "$ONCHAIN_LIFECYCLE_REPORT" cargo run -q -p tap-ldk-cli -- onchain-lifecycle-smoke
run_json chain-watcher-lifecycle "$CHAIN_OBSERVATION_REPORT" cargo run -q -p tap-ldk-cli -- chain-watcher-lifecycle-smoke
capture_close_artifacts

cp "$BOB_WALLET" "$BOB_RESTART_WALLET"
run_json bob-wallet-balances-after-restart "$ARTIFACT_DIR/bob-wallet-balances-after-restart.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$BOB_RESTART_WALLET"

cat >"$SUMMARY" <<SUMMARY_TEXT
Path A native-to-native demo artifacts: $ARTIFACT_DIR

Visible mocked/experimental pieces:
- issuer identity: bounded local OPENUSD issuer key $ISSUER_KEY
- fixed oracle: regtest RFQ quote store uses 100 msat per OPENUSD unit
- proof courier: local transfer proof file $BOB_PROOF plus accepted bundle $BOB_PROOF_BUNDLE
- UI/runtime: headless CLI smoke; no LND/tapd wallet sidecar

Expected demo path:
- local wallets issue and courier OPENUSD proof material
- native asset channel funds at alice=700 bob=300
- native payment settles 125 OPENUSD to bob
- recovery smoke checks funding/RFQ/HTLC/commitment/settlement/close-prep restart boundaries
- cooperative close exports final proofs at alice=575 bob=425
- on-chain lifecycle report records close proof export, bounded force-close recovery evidence, sweep refusals, and restart evidence at $ONCHAIN_LIFECYCLE_REPORT
- chain observation report binds those lifecycle events to bounded chain/sweeper observations at $CHAIN_OBSERVATION_REPORT
- close proofs are captured at $CLOSE_LOCAL_PROOF_HEX and $CLOSE_REMOTE_PROOF_HEX
- force-close status is machine-visible in $CLOSE_RECOVERY_STATUS and remains deferred
- bob wallet reload after restart keeps the same imported proof balance
SUMMARY_TEXT

cat "$SUMMARY"
echo
echo "path-a-native-demo: key outputs"
echo "--- native-payment.json ---"
cat "$ARTIFACT_DIR/native-payment.json"
echo
echo "--- native-recovery.json ---"
cat "$ARTIFACT_DIR/native-recovery.json"
echo
echo "--- native-close.json ---"
cat "$ARTIFACT_DIR/native-close.json"
echo
echo "--- close-recovery-status.json ---"
cat "$CLOSE_RECOVERY_STATUS"
echo
echo "--- onchain-lifecycle.json ---"
cat "$ONCHAIN_LIFECYCLE_REPORT"
echo
echo "--- chain-watcher-lifecycle.json ---"
cat "$CHAIN_OBSERVATION_REPORT"
