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
CHANNEL_STORE="$ARTIFACT_DIR/asset-channels.json"
COMMITMENT_STORE="$ARTIFACT_DIR/asset-commitments.json"
SUMMARY="$ARTIFACT_DIR/summary.txt"

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
run_json alice-wallet-balances "$ARTIFACT_DIR/alice-wallet-balances.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$ALICE_WALLET"
run_json bob-wallet-balances "$ARTIFACT_DIR/bob-wallet-balances.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$BOB_WALLET"

run_json asset-channel-funding "$ARTIFACT_DIR/asset-channel-funding.json" cargo run -q -p tap-ldk-cli -- asset-channel-funding-smoke "$CHANNEL_STORE"
run_json asset-commitment "$ARTIFACT_DIR/asset-commitment.json" cargo run -q -p tap-ldk-cli -- asset-commitment-smoke "$COMMITMENT_STORE"
run_json native-payment "$ARTIFACT_DIR/native-payment.json" cargo run -q -p tap-ldk-cli -- asset-payment-smoke
run_json native-recovery "$ARTIFACT_DIR/native-recovery.json" cargo run -q -p tap-ldk-cli -- asset-recovery-smoke
run_json native-close "$ARTIFACT_DIR/native-close.json" cargo run -q -p tap-ldk-cli -- asset-close-smoke

cp "$BOB_WALLET" "$BOB_RESTART_WALLET"
run_json bob-wallet-balances-after-restart "$ARTIFACT_DIR/bob-wallet-balances-after-restart.json" cargo run -q -p tap-ldk-cli -- wallet-balances "$BOB_RESTART_WALLET"

cat >"$SUMMARY" <<SUMMARY_TEXT
Path A native-to-native demo artifacts: $ARTIFACT_DIR

Visible mocked/experimental pieces:
- issuer identity: bounded local OPENUSD issuer key $ISSUER_KEY
- fixed oracle: regtest RFQ quote store uses 100 msat per OPENUSD unit
- proof courier: local proof file $BOB_PROOF
- UI/runtime: headless CLI smoke; no LND/tapd wallet sidecar

Expected demo path:
- local wallets issue and courier OPENUSD proof material
- native asset channel funds at alice=700 bob=300
- native payment settles 125 OPENUSD to bob
- recovery smoke checks funding/RFQ/HTLC/commitment/settlement/close-prep restart boundaries
- cooperative close exports final proofs at alice=575 bob=425
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
