#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "live-tapd-proof-bind: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

REPORT_PATH="${1:-$ROOT/target/live-tapd-proof-binding/report.json}"
WALLET_PATH="${2:-$ROOT/target/live-tapd-proof-binding/tap-ldk-wallet.json}"
ARTIFACT_DIR="$(dirname "$REPORT_PATH")"
LOG_DIR="$ARTIFACT_DIR/logs"
PROOF_FILE="$ARTIFACT_DIR/live-tapd-proof.tapf"
COUNTERPARTY_REPORT="$ARTIFACT_DIR/lightning-labs-counterparty-ready.json"
ASSET_TAG="${TAP_LDK_LIVE_TAPD_ASSET_TAG:-OPENUSD}"
ASSET_SUPPLY="${TAP_LDK_LIVE_TAPD_ASSET_SUPPLY:-1000000}"
ASSET_DECIMAL_DISPLAY="${TAP_LDK_LIVE_TAPD_DECIMAL_DISPLAY:-2}"
FEE_RATE="${TAP_LDK_LIVE_TAPD_FEE_RATE:-1}"
BITCOIND_CONTAINER="${TAP_LDK_LL_BITCOIND_CONTAINER:-tap-ldk-ll-bitcoind}"
TAPD_CONTAINER="${TAP_LDK_LL_TAPD_CONTAINER:-tap-ldk-ll-tapd}"
RPC_USER="${TAP_LDK_BITCOIN_RPC_USER:-tapldk}"
RPC_PASSWORD="${TAP_LDK_BITCOIN_RPC_PASSWORD:-tapldk-regtest}"
BITCOIN_WALLET_NAME="${TAP_LDK_LL_BITCOIN_WALLET:-tapldk}"
DOCKER_APP_BIN="/Applications/Docker.app/Contents/Resources/bin/docker"
CONTAINER_RUNTIME_BIN=""

mkdir -p "$ARTIFACT_DIR" "$LOG_DIR"

detect_container_runtime() {
  if [ -n "${TAP_LDK_CONTAINER_RUNTIME:-}" ]; then
    if command -v "$TAP_LDK_CONTAINER_RUNTIME" >/dev/null 2>&1; then
      command -v "$TAP_LDK_CONTAINER_RUNTIME"
      return 0
    fi
    if [ "$TAP_LDK_CONTAINER_RUNTIME" = "docker" ] && [ -x "$DOCKER_APP_BIN" ]; then
      printf '%s\n' "$DOCKER_APP_BIN"
      return 0
    fi
    if [ -x "$TAP_LDK_CONTAINER_RUNTIME" ]; then
      printf '%s\n' "$TAP_LDK_CONTAINER_RUNTIME"
      return 0
    fi
    return 1
  fi

  if command -v docker >/dev/null 2>&1; then
    command -v docker
    return 0
  fi

  if [ -x "$DOCKER_APP_BIN" ]; then
    printf '%s\n' "$DOCKER_APP_BIN"
    return 0
  fi

  if command -v podman >/dev/null 2>&1; then
    command -v podman
    return 0
  fi

  return 1
}

write_blocked_report() {
  local step="$1"
  local reason="$2"
  jq -n \
    --arg status "blocked" \
    --arg source "live-tapd-proof-binding" \
    --arg step "$step" \
    --arg reason "$reason" \
    --arg asset_tag "$ASSET_TAG" \
    --arg amount "$ASSET_SUPPLY" \
    --arg wallet_path "$WALLET_PATH" \
    '{
      schema_version: 1,
      source: $source,
      status: $status,
      blocked_step: $step,
      reason: $reason,
      asset_tag: $asset_tag,
      amount: ($amount | tonumber),
      wallet_path: $wallet_path,
      fixture_only_path: false
    }' >"$REPORT_PATH"
}

container_exec() {
  local container="$1"
  shift
  "$CONTAINER_RUNTIME_BIN" exec "$container" "$@"
}

tap_cli() {
  container_exec "$TAPD_CONTAINER" tapcli \
    --network=regtest \
    --tlscertpath=/home/tap/.tapd/tls.cert \
    --macaroonpath=/home/tap/.tapd/data/regtest/admin.macaroon \
    "$@"
}

bitcoin_cli() {
  container_exec "$BITCOIND_CONTAINER" bitcoin-cli \
    -regtest \
    -rpcuser="$RPC_USER" \
    -rpcpassword="$RPC_PASSWORD" \
    "$@"
}

bitcoin_wallet_cli() {
  bitcoin_cli -rpcwallet="$BITCOIN_WALLET_NAME" "$@"
}

mine_blocks() {
  local blocks="$1"
  local address
  address="$(bitcoin_wallet_cli getnewaddress "" bech32m 2>/dev/null || bitcoin_wallet_cli getnewaddress)"
  bitcoin_wallet_cli generatetoaddress "$blocks" "$address" >/dev/null
}

normalize_script_key() {
  local key="$1"
  case "${#key}" in
    64) printf '02%s\n' "$key" ;;
    66) printf '%s\n' "$key" ;;
    *)
      echo "live-tapd-proof-bind: unsupported script key length ${#key}" >&2
      return 1
      ;;
  esac
}

if ! ./scripts/lightning-labs-counterparty.sh start >"$COUNTERPARTY_REPORT" 2>"$LOG_DIR/counterparty-start.err"; then
  reason="$(cat "$LOG_DIR/counterparty-start.err")"
  write_blocked_report "counterparty_start" "$reason"
  cat "$REPORT_PATH"
  exit 0
fi

if ! CONTAINER_RUNTIME_BIN="$(detect_container_runtime)"; then
  write_blocked_report "container_runtime" "No Docker or Podman runtime was found after counterparty start."
  cat "$REPORT_PATH"
  exit 0
fi

meta_json="$(jq -nc --arg ticker "$ASSET_TAG" '{ticker: $ticker, experimental: true, source: "tap-ldk-live-demo"}')"

tap_cli assets mint \
  --type normal \
  --name "$ASSET_TAG" \
  --supply "$ASSET_SUPPLY" \
  --decimal_display "$ASSET_DECIMAL_DISPLAY" \
  --meta_type json \
  --meta_bytes "$meta_json" >"$ARTIFACT_DIR/tapd-mint.json" 2>"$LOG_DIR/tapd-mint.err"

tap_cli assets mint finalize \
  --sat_per_vbyte "$FEE_RATE" >"$ARTIFACT_DIR/tapd-finalize.json" 2>"$LOG_DIR/tapd-finalize.err"

mine_blocks 6

tap_cli assets list >"$ARTIFACT_DIR/tapd-assets.json" 2>"$LOG_DIR/tapd-assets.err"

asset_json="$(
  jq -c \
    --arg tag "$ASSET_TAG" \
    --arg amount "$ASSET_SUPPLY" \
    '[.assets[] | select(((.asset_genesis.name // .assetGenesis.name) == $tag) and ((.amount | tostring) == $amount))][0] // empty' \
    "$ARTIFACT_DIR/tapd-assets.json"
)"

if [ -z "$asset_json" ]; then
  write_blocked_report "tapd_asset_lookup" "tapd assets list did not return the minted asset."
  cat "$REPORT_PATH"
  exit 0
fi

asset_id="$(printf '%s' "$asset_json" | jq -r '.asset_genesis.asset_id // .assetGenesis.assetId // empty')"
script_key="$(printf '%s' "$asset_json" | jq -r '.script_key // .scriptKey // empty')"
genesis_outpoint="$(printf '%s' "$asset_json" | jq -r '.asset_genesis.genesis_point // .assetGenesis.genesisPoint // empty')"
anchor_outpoint="$(printf '%s' "$asset_json" | jq -r '.chain_anchor.anchor_outpoint // .chainAnchor.anchorOutpoint // empty')"

if [ -z "$asset_id" ] || [ -z "$script_key" ] || [ -z "$genesis_outpoint" ] || [ -z "$anchor_outpoint" ]; then
  write_blocked_report "tapd_asset_fields" "tapd asset output was missing asset id, script key, genesis outpoint, or anchor outpoint."
  cat "$REPORT_PATH"
  exit 0
fi

tap_cli proofs export \
  --asset_id "$asset_id" \
  --script_key "$script_key" \
  --proof_file - >"$PROOF_FILE" 2>"$LOG_DIR/tapd-proof-export.err"

owner_script_key="$(normalize_script_key "$script_key")"

cargo run -q -p tap-ldk-cli -- live-tapd-proof-bind \
  "$WALLET_PATH" \
  "$PROOF_FILE" \
  "$asset_id" \
  "$ASSET_SUPPLY" \
  "$owner_script_key" \
  "$genesis_outpoint" \
  "$anchor_outpoint" \
  "$REPORT_PATH" >"$LOG_DIR/tap-ldk-bind.stdout.json" 2>"$LOG_DIR/tap-ldk-bind.err"

cat "$REPORT_PATH"
