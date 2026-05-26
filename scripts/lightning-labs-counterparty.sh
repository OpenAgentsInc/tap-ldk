#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "lightning-labs-counterparty: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

NETWORK_NAME="${TAP_LDK_LL_NETWORK:-tap-ldk-ll-regtest}"
BITCOIND_CONTAINER="${TAP_LDK_LL_BITCOIND_CONTAINER:-tap-ldk-ll-bitcoind}"
LND_CONTAINER="${TAP_LDK_LL_LND_CONTAINER:-tap-ldk-ll-lnd}"
TAPD_CONTAINER="${TAP_LDK_LL_TAPD_CONTAINER:-tap-ldk-ll-tapd}"
BITCOIND_IMAGE="${TAP_LDK_LL_BITCOIND_IMAGE:-polarlightning/bitcoind:30.0}"
LND_IMAGE="${TAP_LDK_LL_LND_IMAGE:-polarlightning/lnd:0.19.0-beta}"
TAPD_IMAGE="${TAP_LDK_LL_TAPD_IMAGE:-polarlightning/tapd:0.7.0-alpha}"
RPC_USER="${TAP_LDK_BITCOIN_RPC_USER:-tapldk}"
RPC_PASSWORD="${TAP_LDK_BITCOIN_RPC_PASSWORD:-tapldk-regtest}"
BITCOIN_WALLET_NAME="${TAP_LDK_LL_BITCOIN_WALLET:-tapldk}"
LND_WALLET_PASSWORD="${TAP_LDK_LL_LND_WALLET_PASSWORD:-tapldk-regtest-lnd-wallet}"
STATE_DIR="${TAP_LDK_LL_STATE_DIR:-$ROOT/.tap-ldk/regtest/lightning-labs}"
WAIT_TIMEOUT_SECONDS="${TAP_LDK_LL_WAIT_TIMEOUT_SECONDS:-180}"
WAIT_INTERVAL_SECONDS="${TAP_LDK_LL_WAIT_INTERVAL_SECONDS:-2}"
CONTAINER_RUN_TIMEOUT_SECONDS="${TAP_LDK_LL_CONTAINER_RUN_TIMEOUT_SECONDS:-300}"
LND_FUND_TARGET_SAT="${TAP_LDK_LL_LND_FUND_TARGET_SAT:-100000000}"
LND_FUND_BTC="${TAP_LDK_LL_LND_FUND_BTC:-10}"
LND_HOST_P2P_PORT="${TAP_LDK_LL_LND_HOST_P2P_PORT:-19735}"
DOCKER_APP_BIN="/Applications/Docker.app/Contents/Resources/bin/docker"
CONTAINER_RUNTIME_BIN=""

usage() {
  cat <<USAGE
Usage: scripts/lightning-labs-counterparty.sh <command>

Commands:
  start       Start Bitcoin Core, LND, and tapd, then print readiness JSON
  stop        Stop all counterparty containers
  status      Print container status and best-effort readiness JSON
  ready       Print best-effort readiness JSON
  connection  Print JSON connection material and local credential paths
  tapd-balance <asset-id>
             Print the current tapd balance for one asset ID
  smoke       Start, print readiness JSON, and stop

This harness starts Lightning Labs as an independent interop counterparty. It
does not run LND or tapd as a sidecar inside the native tap-ldk wallet.
Set TAP_LDK_CONTAINER_RUNTIME=docker or TAP_LDK_CONTAINER_RUNTIME=podman to
force a specific runtime. By default the script prefers Docker, including the
Docker Desktop app bundle CLI, then Podman.
Set TAP_LDK_LL_CONTAINER_RUN_TIMEOUT_SECONDS to bound image pull/container
startup time.
USAGE
}

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
    echo "lightning-labs-counterparty: requested container runtime $TAP_LDK_CONTAINER_RUNTIME is not installed." >&2
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

  echo "lightning-labs-counterparty: neither Docker nor Podman is installed." >&2
  return 1
}

runtime_name() {
  basename "$CONTAINER_RUNTIME_BIN"
}

require_container_runtime() {
  if ! CONTAINER_RUNTIME_BIN="$(detect_container_runtime)"; then
    return 1
  fi
  if ! "$CONTAINER_RUNTIME_BIN" info >/dev/null 2>&1; then
    echo "lightning-labs-counterparty: $(runtime_name) is installed at $CONTAINER_RUNTIME_BIN, but its daemon or machine is not reachable." >&2
    if [ "$(runtime_name)" = "docker" ]; then
      echo "lightning-labs-counterparty: start Docker Desktop and make sure its docker socket is available to this shell." >&2
    elif [ "$(runtime_name)" = "podman" ]; then
      echo "lightning-labs-counterparty: start the Podman machine, for example: podman machine start." >&2
    fi
    return 1
  fi
}

container_running() {
  "$CONTAINER_RUNTIME_BIN" ps --format '{{.Names}}' | grep -qx "$1"
}

container_exists() {
  "$CONTAINER_RUNTIME_BIN" ps -a --format '{{.Names}}' | grep -qx "$1"
}

remove_stopped_container() {
  local container="$1"
  if container_exists "$container" && ! container_running "$container"; then
    "$CONTAINER_RUNTIME_BIN" rm "$container" >/dev/null
  fi
}

ensure_network() {
  if ! "$CONTAINER_RUNTIME_BIN" network inspect "$NETWORK_NAME" >/dev/null 2>&1; then
    "$CONTAINER_RUNTIME_BIN" network create "$NETWORK_NAME" >/dev/null
  fi
}

wait_until() {
  local label="$1"
  shift
  local start now elapsed
  start="$(date +%s)"
  while true; do
    if "$@" >/dev/null 2>&1; then
      echo "lightning-labs-counterparty: ready: $label" >&2
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if [ "$elapsed" -ge "$WAIT_TIMEOUT_SECONDS" ]; then
      echo "lightning-labs-counterparty: timed out waiting for $label after ${WAIT_TIMEOUT_SECONDS}s" >&2
      return 1
    fi

    sleep "$WAIT_INTERVAL_SECONDS"
  done
}

wait_for_host_file() {
  test -f "$1"
}

run_with_timeout() {
  local label="$1"
  shift
  local start now elapsed pid status

  "$@" &
  pid=$!
  start="$(date +%s)"

  while kill -0 "$pid" 2>/dev/null; do
    now="$(date +%s)"
    elapsed=$((now - start))
    if [ "$elapsed" -ge "$CONTAINER_RUN_TIMEOUT_SECONDS" ]; then
      kill "$pid" 2>/dev/null || true
      sleep 1
      kill -9 "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      echo "lightning-labs-counterparty: timed out waiting for $label after ${CONTAINER_RUN_TIMEOUT_SECONDS}s" >&2
      return 1
    fi
    sleep 1
  done

  set +e
  wait "$pid"
  status=$?
  set -e
  return "$status"
}

exec_container() {
  local container="$1"
  shift
  "$CONTAINER_RUNTIME_BIN" exec "$container" "$@"
}

bitcoin_cli() {
  exec_container "$BITCOIND_CONTAINER" bitcoin-cli \
    -regtest \
    -rpcuser="$RPC_USER" \
    -rpcpassword="$RPC_PASSWORD" \
    "$@"
}

bitcoin_wallet_cli() {
  bitcoin_cli -rpcwallet="$BITCOIN_WALLET_NAME" "$@"
}

lnd_cli() {
  exec_container "$LND_CONTAINER" lncli --network=regtest "$@"
}

tap_cli() {
  exec_container "$TAPD_CONTAINER" tapcli \
    --network=regtest \
    --tlscertpath=/home/tap/.tapd/tls.cert \
    --macaroonpath=/home/tap/.tapd/data/regtest/admin.macaroon \
    "$@"
}

lnd_password_b64() {
  printf '%s' "$LND_WALLET_PASSWORD" | base64 | tr -d '\n'
}

lnd_rest_post() {
  local path="$1"
  local payload="$2"
  curl -ksS \
    --connect-timeout 2 \
    --max-time 10 \
    -H 'Content-Type: application/json' \
    -X POST \
    --data "$payload" \
    "https://127.0.0.1:18080$path"
}

lnd_unlock_wallet_rest() {
  local password payload
  command -v curl >/dev/null 2>&1 || return 1
  command -v jq >/dev/null 2>&1 || return 1
  password="$(lnd_password_b64)"
  payload="$(jq -n --arg password "$password" '{wallet_password: $password}')"
  lnd_rest_post /v1/unlockwallet "$payload" >/dev/null
}

lnd_init_wallet_rest() {
  local password seed_json mnemonic payload
  command -v curl >/dev/null 2>&1 || return 1
  command -v jq >/dev/null 2>&1 || return 1
  password="$(lnd_password_b64)"
  seed_json="$(lnd_rest_post /v1/genseed '{}')" || return 1
  mnemonic="$(printf '%s' "$seed_json" | jq -c '.cipher_seed_mnemonic // empty')"
  if [ -z "$mnemonic" ]; then
    return 1
  fi
  payload="$(jq -n \
    --arg password "$password" \
    --argjson mnemonic "$mnemonic" \
    '{wallet_password: $password, cipher_seed_mnemonic: $mnemonic}')"
  lnd_rest_post /v1/initwallet "$payload" >/dev/null
}

json_value() {
  local query="$1"
  if command -v jq >/dev/null 2>&1; then
    jq -r "$query // empty" 2>/dev/null
  else
    sed -n 's/.*"'"${query#.}"'": *"\{0,1\}\([^",}]*\)"\{0,1\}.*/\1/p' | head -n 1
  fi
}

json_string() {
  if command -v jq >/dev/null 2>&1; then
    jq -Rn --arg value "$1" '$value'
  else
    printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  fi
}

json_number_or_null() {
  if printf '%s' "$1" | grep -Eq '^[0-9]+$'; then
    printf '%s' "$1"
  else
    printf 'null'
  fi
}

json_bool_or_null() {
  case "$1" in
    true|false) printf '%s' "$1" ;;
    *) printf 'null' ;;
  esac
}

start_bitcoind() {
  remove_stopped_container "$BITCOIND_CONTAINER"
  if container_running "$BITCOIND_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/bitcoind"
  run_with_timeout "bitcoind image pull/container start" \
    "$CONTAINER_RUNTIME_BIN" run -d --rm \
    --name "$BITCOIND_CONTAINER" \
    --network "$NETWORK_NAME" \
    -p 127.0.0.1:18443:18443 \
    -v "$STATE_DIR/bitcoind:/home/bitcoin/.bitcoin" \
    "$BITCOIND_IMAGE" \
    bitcoind \
    -server=1 \
    -regtest=1 \
    -rpcuser="$RPC_USER" \
    -rpcpassword="$RPC_PASSWORD" \
    -debug=1 \
    -zmqpubrawblock=tcp://0.0.0.0:28334 \
    -zmqpubrawtx=tcp://0.0.0.0:28335 \
    -zmqpubhashblock=tcp://0.0.0.0:28336 \
    -txindex=1 \
    -dnsseed=0 \
    -rpcbind=0.0.0.0 \
    -rpcallowip=0.0.0.0/0 \
    -rpcport=18443 \
    -rest \
    -listen=1 \
    -listenonion=0 \
    -fallbackfee=0.0002 \
    -blockfilterindex=1 \
    -peerblockfilters=1 >/dev/null
}

start_lnd() {
  remove_stopped_container "$LND_CONTAINER"
  if container_running "$LND_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/lnd"
  run_with_timeout "LND image pull/container start" \
    "$CONTAINER_RUNTIME_BIN" run -d --rm \
    --name "$LND_CONTAINER" \
    --network "$NETWORK_NAME" \
    -p 127.0.0.1:10009:10009 \
    -p 127.0.0.1:18080:8080 \
    -p 127.0.0.1:"$LND_HOST_P2P_PORT":9735 \
    -v "$STATE_DIR/lnd:/home/lnd/.lnd" \
    "$LND_IMAGE" \
    lnd \
    --noseedbackup \
    --debuglevel=debug \
    --trickledelay=5000 \
    --alias=tap-ldk-ll-lnd \
    --externalip="$LND_CONTAINER" \
    --tlsextradomain="$LND_CONTAINER" \
    --tlsextradomain=host.docker.internal \
    --listen=0.0.0.0:9735 \
    --rpclisten=0.0.0.0:10009 \
    --restlisten=0.0.0.0:8080 \
    --bitcoin.active \
    --bitcoin.regtest \
    --bitcoin.node=bitcoind \
    --bitcoind.rpchost="$BITCOIND_CONTAINER" \
    --bitcoind.rpcuser="$RPC_USER" \
    --bitcoind.rpcpass="$RPC_PASSWORD" \
    --bitcoind.zmqpubrawblock=tcp://"$BITCOIND_CONTAINER":28334 \
    --bitcoind.zmqpubrawtx=tcp://"$BITCOIND_CONTAINER":28335 \
    --accept-keysend \
    --accept-amp \
    --protocol.option-scid-alias \
    --protocol.zero-conf \
    --protocol.simple-taproot-chans \
    --protocol.simple-taproot-overlay-chans \
    --protocol.custom-message=17 >/dev/null
}

start_tapd() {
  remove_stopped_container "$TAPD_CONTAINER"
  if container_running "$TAPD_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/tapd"
  run_with_timeout "tapd image pull/container start" \
    "$CONTAINER_RUNTIME_BIN" run -d --rm \
    --name "$TAPD_CONTAINER" \
    --network "$NETWORK_NAME" \
    -p 127.0.0.1:10029:10029 \
    -p 127.0.0.1:18089:8089 \
    -v "$STATE_DIR/tapd:/home/tap/.tapd" \
    -v "$STATE_DIR/lnd:/home/tap/.lnd:ro" \
    "$TAPD_IMAGE" \
    tapd \
    --network=regtest \
    --debuglevel=debug \
    --tlsextradomain="$TAPD_CONTAINER" \
    --tlsextradomain=host.docker.internal \
    --rpclisten=0.0.0.0:10029 \
    --restlisten=0.0.0.0:8089 \
    --lnd.host="$LND_CONTAINER":10009 \
    --lnd.macaroonpath=/home/tap/.lnd/data/chain/bitcoin/regtest/admin.macaroon \
    --lnd.tlspath=/home/tap/.lnd/tls.cert \
    --allow-public-uni-proof-courier \
    --allow-public-stats \
    --universe.public-access=rw \
    --universe.sync-all-assets >/dev/null
}

ensure_bitcoin_wallet() {
  if bitcoin_wallet_cli getwalletinfo >/dev/null 2>&1; then
    return 0
  fi

  bitcoin_cli loadwallet "$BITCOIN_WALLET_NAME" >/dev/null 2>&1 && return 0

  bitcoin_cli -named createwallet \
    wallet_name="$BITCOIN_WALLET_NAME" \
    descriptors=true \
    load_on_startup=true >/dev/null
}

mine_blocks() {
  local blocks="$1"
  local address
  address="$(bitcoin_wallet_cli getnewaddress "" bech32m 2>/dev/null || bitcoin_wallet_cli getnewaddress)"
  bitcoin_wallet_cli generatetoaddress "$blocks" "$address" >/dev/null
}

ensure_mature_bitcoin_funds() {
  local height missing
  height="$(bitcoin_cli getblockcount)"
  if [ "$height" -lt 101 ]; then
    missing=$((101 - height))
    mine_blocks "$missing"
  fi
}

lnd_wallet_ready_step() {
  if lnd_cli getinfo >/dev/null 2>&1; then
    return 0
  fi
  if lnd_unlock_wallet_rest >/dev/null 2>&1; then
    return 0
  fi
  if lnd_init_wallet_rest >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

lnd_synced() {
  local info
  info="$(lnd_cli getinfo 2>/dev/null)"
  printf '%s' "$info" | json_value '.synced_to_chain' | grep -qx true
}

tapd_ready() {
  tap_cli getinfo >/dev/null 2>&1
}

lnd_wallet_balance_sat() {
  local balance_json balance
  balance_json="$(lnd_cli walletbalance 2>/dev/null || true)"
  balance="$(printf '%s' "$balance_json" | json_value '.total_balance')"
  if [ -z "$balance" ]; then
    printf '0\n'
  else
    printf '%s\n' "$balance"
  fi
}

fund_lnd_wallet() {
  local balance address txid
  balance="$(lnd_wallet_balance_sat)"
  if [ "$balance" -ge "$LND_FUND_TARGET_SAT" ]; then
    return 0
  fi

  address="$(lnd_cli newaddress p2tr 2>/dev/null | json_value '.address' || true)"
  if [ -z "$address" ]; then
    address="$(lnd_cli newaddress p2wkh | json_value '.address')"
  fi

  txid="$(bitcoin_wallet_cli sendtoaddress "$address" "$LND_FUND_BTC")"
  echo "lightning-labs-counterparty: funded LND wallet with txid $txid" >&2
  mine_blocks 6
}

start() {
  require_container_runtime
  ensure_network

  start_bitcoind
  wait_until "bitcoind container running" container_running "$BITCOIND_CONTAINER"
  wait_until "bitcoind RPC" bitcoin_cli getblockchaininfo
  ensure_bitcoin_wallet
  ensure_mature_bitcoin_funds

  start_lnd
  wait_until "LND TLS certificate" wait_for_host_file "$STATE_DIR/lnd/tls.cert"
  wait_until "LND wallet initialized and unlocked" lnd_wallet_ready_step
  wait_until "LND admin macaroon" wait_for_host_file "$STATE_DIR/lnd/data/chain/bitcoin/regtest/admin.macaroon"
  wait_until "LND chain sync" lnd_synced
  fund_lnd_wallet
  wait_until "LND chain sync after funding" lnd_synced

  start_tapd
  wait_until "tapd TLS certificate" wait_for_host_file "$STATE_DIR/tapd/tls.cert"
  wait_until "tapd admin macaroon" wait_for_host_file "$STATE_DIR/tapd/data/regtest/admin.macaroon"
  wait_until "tapd RPC" tapd_ready

  ready_report
}

stop() {
  require_container_runtime
  for container in "$TAPD_CONTAINER" "$LND_CONTAINER" "$BITCOIND_CONTAINER"; do
    if container_running "$container"; then
      "$CONTAINER_RUNTIME_BIN" stop "$container" >/dev/null
    fi
  done
}

status() {
  require_container_runtime
  "$CONTAINER_RUNTIME_BIN" ps \
    --filter "name=$BITCOIND_CONTAINER" \
    --filter "name=$LND_CONTAINER" \
    --filter "name=$TAPD_CONTAINER" \
    --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}'
  ready_report
}

connection() {
  cat <<JSON
{
  "network": "regtest",
  "container_network": "$NETWORK_NAME",
  "state_dir": "$STATE_DIR",
  "bitcoind": {
    "container_name": "$BITCOIND_CONTAINER",
    "image": "$BITCOIND_IMAGE",
    "rpc_url": "http://127.0.0.1:18443",
    "rpc_user": "$RPC_USER",
    "rpc_password_env": "TAP_LDK_BITCOIN_RPC_PASSWORD"
  },
  "lnd": {
    "container_name": "$LND_CONTAINER",
    "image": "$LND_IMAGE",
    "p2p_url": "127.0.0.1:$LND_HOST_P2P_PORT",
    "grpc_url": "127.0.0.1:10009",
    "rest_url": "https://127.0.0.1:18080",
    "macaroon_path": "$STATE_DIR/lnd/data/chain/bitcoin/regtest/admin.macaroon",
    "tls_cert_path": "$STATE_DIR/lnd/tls.cert"
  },
  "tapd": {
    "container_name": "$TAPD_CONTAINER",
    "image": "$TAPD_IMAGE",
    "grpc_url": "127.0.0.1:10029",
    "rest_url": "https://127.0.0.1:18089",
    "macaroon_path": "$STATE_DIR/tapd/data/regtest/admin.macaroon",
    "tls_cert_path": "$STATE_DIR/tapd/tls.cert"
  }
}
JSON
}

ready_report() {
  local runtime bitcoind_height lnd_info lnd_balance lnd_pubkey lnd_height lnd_synced_flag
  local tapd_info tapd_pubkey tapd_height tapd_synced_flag

  runtime="$(runtime_name 2>/dev/null || true)"
  bitcoind_height="$(bitcoin_cli getblockcount 2>/dev/null || true)"
  lnd_info="$(lnd_cli getinfo 2>/dev/null || true)"
  lnd_balance="$(lnd_wallet_balance_sat 2>/dev/null || true)"
  lnd_pubkey="$(printf '%s' "$lnd_info" | json_value '.identity_pubkey')"
  lnd_height="$(printf '%s' "$lnd_info" | json_value '.block_height')"
  lnd_synced_flag="$(printf '%s' "$lnd_info" | json_value '.synced_to_chain')"

  tapd_info="$(tap_cli getinfo 2>/dev/null || true)"
  tapd_pubkey="$(printf '%s' "$tapd_info" | json_value '.lnd_identity_pubkey')"
  tapd_height="$(printf '%s' "$tapd_info" | json_value '.block_height')"
  tapd_synced_flag="$(printf '%s' "$tapd_info" | json_value '.sync_to_chain')"

  cat <<JSON
{
  "network": "regtest",
  "container_runtime": $(json_string "$runtime"),
  "container_network": $(json_string "$NETWORK_NAME"),
  "state_dir": $(json_string "$STATE_DIR"),
  "bitcoind": {
    "container_name": $(json_string "$BITCOIND_CONTAINER"),
    "image": $(json_string "$BITCOIND_IMAGE"),
    "rpc_ready": $(json_bool_or_null "$([ -n "$bitcoind_height" ] && echo true || echo false)"),
    "chain_height": $(json_number_or_null "$bitcoind_height")
  },
  "lnd": {
    "container_name": $(json_string "$LND_CONTAINER"),
    "image": $(json_string "$LND_IMAGE"),
    "p2p_url": $(json_string "127.0.0.1:$LND_HOST_P2P_PORT"),
    "grpc_url": $(json_string "127.0.0.1:10009"),
    "rest_url": $(json_string "https://127.0.0.1:18080"),
    "rpc_ready": $(json_bool_or_null "$([ -n "$lnd_pubkey" ] && echo true || echo false)"),
    "identity_pubkey": $(json_string "$lnd_pubkey"),
    "chain_height": $(json_number_or_null "$lnd_height"),
    "synced_to_chain": $(json_bool_or_null "$lnd_synced_flag"),
    "wallet_balance_sat": $(json_number_or_null "$lnd_balance"),
    "macaroon_path": $(json_string "$STATE_DIR/lnd/data/chain/bitcoin/regtest/admin.macaroon"),
    "tls_cert_path": $(json_string "$STATE_DIR/lnd/tls.cert")
  },
  "tapd": {
    "container_name": $(json_string "$TAPD_CONTAINER"),
    "image": $(json_string "$TAPD_IMAGE"),
    "grpc_url": $(json_string "127.0.0.1:10029"),
    "rest_url": $(json_string "https://127.0.0.1:18089"),
    "rpc_ready": $(json_bool_or_null "$([ -n "$tapd_pubkey" ] && echo true || echo false)"),
    "lnd_identity_pubkey": $(json_string "$tapd_pubkey"),
    "chain_height": $(json_number_or_null "$tapd_height"),
    "sync_to_chain": $(json_bool_or_null "$tapd_synced_flag"),
    "macaroon_path": $(json_string "$STATE_DIR/tapd/data/regtest/admin.macaroon"),
    "tls_cert_path": $(json_string "$STATE_DIR/tapd/tls.cert")
  }
}
JSON
}

tapd_balance() {
  local asset_id="${1:-}"
  if [ -z "$asset_id" ]; then
    echo "lightning-labs-counterparty: tapd-balance requires an asset id." >&2
    exit 2
  fi

  require_container_runtime

  local balance_json observed_balance
  balance_json="$(tap_cli assets balance \
    --asset_id "$asset_id" \
    --all_script_key_types)"
  observed_balance="$(
    printf '%s' "$balance_json" | jq -r \
      --arg asset_id "$asset_id" \
      '.asset_balances[$asset_id].balance //
       .assetBalances[$asset_id].balance //
       "0"'
  )"

  jq -n \
    --arg source "lightning-labs-counterparty-tapd-balance" \
    --arg asset_id "$asset_id" \
    --arg observed_balance "$observed_balance" \
    --argjson raw "$balance_json" \
    '{
      schema_version: 1,
      source: $source,
      asset_id: $asset_id,
      observed_balance: ($observed_balance | tonumber),
      raw_tapd_balance: $raw
    }'
}

smoke() {
  require_container_runtime
  trap 'stop >/dev/null 2>&1 || true' EXIT
  start
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  ready) require_container_runtime && ready_report ;;
  connection) connection ;;
  tapd-balance) tapd_balance "${2:-}" ;;
  smoke) smoke ;;
  ""|-h|--help) usage ;;
  *)
    echo "lightning-labs-counterparty: unknown command: $1" >&2
    usage >&2
    exit 2
    ;;
esac
