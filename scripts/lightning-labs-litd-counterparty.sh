#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "lightning-labs-litd-counterparty: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

NETWORK_NAME="${TAP_LDK_LL_NETWORK:-tap-ldk-ll-regtest}"
BITCOIND_CONTAINER="${TAP_LDK_LL_BITCOIND_CONTAINER:-tap-ldk-ll-bitcoind}"
LITD_CONTAINER="${TAP_LDK_LL_LITD_CONTAINER:-tap-ldk-ll-litd}"
BITCOIND_IMAGE="${TAP_LDK_LL_BITCOIND_IMAGE:-polarlightning/bitcoind:30.0}"
LITD_IMAGE="${TAP_LDK_LL_LITD_IMAGE:-polarlightning/litd:0.16.0-alpha}"
RPC_USER="${TAP_LDK_BITCOIN_RPC_USER:-tapldk}"
RPC_PASSWORD="${TAP_LDK_BITCOIN_RPC_PASSWORD:-tapldk-regtest}"
BITCOIN_WALLET_NAME="${TAP_LDK_LL_BITCOIN_WALLET:-tapldk}"
LITD_UI_PASSWORD="${TAP_LDK_LL_LITD_UI_PASSWORD:-tapldk-regtest-litd}"
STATE_DIR="${TAP_LDK_LL_LITD_STATE_DIR:-$ROOT/.tap-ldk/regtest/lightning-labs-litd}"
WAIT_TIMEOUT_SECONDS="${TAP_LDK_LL_WAIT_TIMEOUT_SECONDS:-180}"
WAIT_INTERVAL_SECONDS="${TAP_LDK_LL_WAIT_INTERVAL_SECONDS:-2}"
CONTAINER_RUN_TIMEOUT_SECONDS="${TAP_LDK_LL_CONTAINER_RUN_TIMEOUT_SECONDS:-300}"
LITD_FUND_TARGET_SAT="${TAP_LDK_LL_LITD_FUND_TARGET_SAT:-100000000}"
LITD_FUND_BTC="${TAP_LDK_LL_LITD_FUND_BTC:-10}"
LITD_HOST_GRPC_PORT="${TAP_LDK_LL_LITD_HOST_GRPC_PORT:-11009}"
LITD_HOST_REST_PORT="${TAP_LDK_LL_LITD_HOST_REST_PORT:-28080}"
LITD_HOST_HTTPS_PORT="${TAP_LDK_LL_LITD_HOST_HTTPS_PORT:-28443}"
LITD_HOST_P2P_PORT="${TAP_LDK_LL_LITD_HOST_P2P_PORT:-29735}"
DOCKER_APP_BIN="/Applications/Docker.app/Contents/Resources/bin/docker"
CONTAINER_RUNTIME_BIN=""

usage() {
  cat <<USAGE
Usage: scripts/lightning-labs-litd-counterparty.sh <command>

Commands:
  start       Start Bitcoin Core and integrated litd, then print readiness JSON
  stop        Stop the integrated litd counterparty container
  status      Print container status and best-effort readiness JSON
  ready       Print best-effort readiness JSON
  connection  Print JSON connection material and local credential paths
  balance <asset-id>
             Print the current integrated taproot-assets balance for one asset ID
  smoke       Start, print readiness JSON, and stop

This harness starts Lightning Labs litd as an independent interop counterparty.
It is the asset-channel target topology because litd runs integrated LND plus
taproot-assets with the aux funding controller and taproot overlay channel
support enabled. It is not a sidecar inside the native tap-ldk wallet.
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
    echo "lightning-labs-litd-counterparty: requested container runtime $TAP_LDK_CONTAINER_RUNTIME is not installed." >&2
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

  echo "lightning-labs-litd-counterparty: neither Docker nor Podman is installed." >&2
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
    echo "lightning-labs-litd-counterparty: $(runtime_name) is installed at $CONTAINER_RUNTIME_BIN, but its daemon or machine is not reachable." >&2
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
      echo "lightning-labs-litd-counterparty: ready: $label" >&2
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if [ "$elapsed" -ge "$WAIT_TIMEOUT_SECONDS" ]; then
      echo "lightning-labs-litd-counterparty: timed out waiting for $label after ${WAIT_TIMEOUT_SECONDS}s" >&2
      return 1
    fi

    sleep "$WAIT_INTERVAL_SECONDS"
  done
}

wait_until_container_condition() {
  local label="$1"
  local container="$2"
  shift 2
  local start now elapsed
  start="$(date +%s)"
  while true; do
    if ! container_running "$container"; then
      echo "lightning-labs-litd-counterparty: $container exited while waiting for $label" >&2
      "$CONTAINER_RUNTIME_BIN" logs --tail 120 "$container" >&2 2>/dev/null || true
      return 1
    fi

    if "$@" >/dev/null 2>&1; then
      echo "lightning-labs-litd-counterparty: ready: $label" >&2
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start))
    if [ "$elapsed" -ge "$WAIT_TIMEOUT_SECONDS" ]; then
      echo "lightning-labs-litd-counterparty: timed out waiting for $label after ${WAIT_TIMEOUT_SECONDS}s" >&2
      "$CONTAINER_RUNTIME_BIN" logs --tail 120 "$container" >&2 2>/dev/null || true
      return 1
    fi

    sleep "$WAIT_INTERVAL_SECONDS"
  done
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
      echo "lightning-labs-litd-counterparty: timed out waiting for $label after ${CONTAINER_RUN_TIMEOUT_SECONDS}s" >&2
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

litd_ln_cli() {
  exec_container "$LITD_CONTAINER" /opt/litd/lncli \
    --network=regtest \
    --tlscertpath=/home/litd/.lnd/tls.cert \
    --macaroonpath=/home/litd/.lnd/data/chain/bitcoin/regtest/admin.macaroon \
    "$@"
}

litd_tap_cli() {
  exec_container "$LITD_CONTAINER" /opt/litd/tapcli \
    --network=regtest \
    --rpcserver=localhost:8443 \
    --tlscertpath=/home/litd/.lit/tls.cert \
    --macaroonpath=/home/litd/.tapd/data/regtest/admin.macaroon \
    "$@"
}

litd_cli() {
  exec_container "$LITD_CONTAINER" /opt/litd/litcli \
    --network=regtest \
    --tlscertpath=/home/litd/.lit/tls.cert \
    --macaroonpath=/home/litd/.lit/regtest/lit.macaroon \
    "$@"
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
    "$CONTAINER_RUNTIME_BIN" run -d \
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

start_litd() {
  remove_stopped_container "$LITD_CONTAINER"
  if container_running "$LITD_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/lit" "$STATE_DIR/lnd" "$STATE_DIR/tapd"
  run_with_timeout "litd image pull/container start" \
    "$CONTAINER_RUNTIME_BIN" run -d \
    --name "$LITD_CONTAINER" \
    --network "$NETWORK_NAME" \
    -p 127.0.0.1:"$LITD_HOST_GRPC_PORT":10009 \
    -p 127.0.0.1:"$LITD_HOST_REST_PORT":8080 \
    -p 127.0.0.1:"$LITD_HOST_HTTPS_PORT":8443 \
    -p 127.0.0.1:"$LITD_HOST_P2P_PORT":9735 \
    -v "$STATE_DIR/lit:/home/litd/.lit" \
    -v "$STATE_DIR/lnd:/home/litd/.lnd" \
    -v "$STATE_DIR/tapd:/home/litd/.tapd" \
    "$LITD_IMAGE" \
    litd \
    --httpslisten=0.0.0.0:8443 \
    --enablerest \
    --uipassword="$LITD_UI_PASSWORD" \
    --network=regtest \
    --lnd-mode=integrated \
    --pool-mode=disable \
    --loop-mode=disable \
    --autopilot.disable \
    --lnd.noseedbackup \
    --lnd.debuglevel=debug \
    --lnd.alias="$LITD_CONTAINER" \
    --lnd.externalip="$LITD_CONTAINER" \
    --lnd.tlsextradomain="$LITD_CONTAINER" \
    --lnd.tlsextradomain=host.docker.internal \
    --lnd.listen=0.0.0.0:9735 \
    --lnd.rpclisten=0.0.0.0:10009 \
    --lnd.restlisten=0.0.0.0:8080 \
    --lnd.bitcoin.active \
    --lnd.bitcoin.regtest \
    --lnd.bitcoin.node=bitcoind \
    --lnd.bitcoind.rpchost="$BITCOIND_CONTAINER" \
    --lnd.bitcoind.rpcuser="$RPC_USER" \
    --lnd.bitcoind.rpcpass="$RPC_PASSWORD" \
    --lnd.bitcoind.zmqpubrawblock=tcp://"$BITCOIND_CONTAINER":28334 \
    --lnd.bitcoind.zmqpubrawtx=tcp://"$BITCOIND_CONTAINER":28335 \
    --taproot-assets.allow-public-uni-proof-courier \
    --taproot-assets.universe.public-access=rw \
    --taproot-assets.universe.sync-all-assets \
    --taproot-assets.allow-public-stats \
    --taproot-assets.proofcourieraddr=universerpc://"$LITD_CONTAINER":8443 \
    --taproot-assets.universerpccourier.skipinitdelay \
    --taproot-assets.universerpccourier.backoffresetwait=1s \
    --taproot-assets.universerpccourier.numtries=5 \
    --taproot-assets.universerpccourier.initialbackoff=300ms \
    --taproot-assets.universerpccourier.maxbackoff=600ms \
    --taproot-assets.experimental.rfq.priceoracleaddress=use_mock_price_oracle_service_promise_to_not_use_on_mainnet \
    --taproot-assets.experimental.rfq.mockoracleassetsperbtc=100000000 \
    --lnd.trickledelay=50 \
    --lnd.gossip.sub-batch-delay=5ms \
    --lnd.caches.rpc-graph-cache-duration=100ms \
    --lnd.default-remote-max-htlcs=483 \
    --lnd.dust-threshold=5000000 \
    --lnd.protocol.option-scid-alias \
    --lnd.protocol.zero-conf \
    --lnd.protocol.simple-taproot-chans \
    --lnd.protocol.simple-taproot-overlay-chans \
    --lnd.protocol.wumbo-channels \
    --lnd.accept-keysend \
    --lnd.protocol.custom-message=17 >/dev/null
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

litd_synced() {
  local info
  info="$(litd_ln_cli getinfo 2>/dev/null)"
  printf '%s' "$info" | json_value '.synced_to_chain' | grep -qx true
}

litd_wallet_balance_sat() {
  local balance_json balance
  balance_json="$(litd_ln_cli walletbalance 2>/dev/null || true)"
  balance="$(printf '%s' "$balance_json" | json_value '.total_balance')"
  if [ -z "$balance" ]; then
    printf '0\n'
  else
    printf '%s\n' "$balance"
  fi
}

fund_litd_wallet() {
  local balance address txid
  balance="$(litd_wallet_balance_sat)"
  if [ "$balance" -ge "$LITD_FUND_TARGET_SAT" ]; then
    return 0
  fi

  address="$(litd_ln_cli newaddress p2tr 2>/dev/null | json_value '.address' || true)"
  if [ -z "$address" ]; then
    address="$(litd_ln_cli newaddress p2wkh | json_value '.address')"
  fi

  txid="$(bitcoin_wallet_cli sendtoaddress "$address" "$LITD_FUND_BTC")"
  echo "lightning-labs-litd-counterparty: funded litd wallet with txid $txid" >&2
  mine_blocks 6
}

litd_status_ready() {
  local status_json
  status_json="$(litd_cli status 2>/dev/null | jq -s '.[0]' 2>/dev/null || echo '{}')"
  printf '%s' "$status_json" | grep -q '"taproot-assets"'
}

taproot_assets_ready() {
  litd_tap_cli getinfo >/dev/null 2>&1
}

asset_channel_rpc_ready() {
  litd_cli ln fundchannel --help >/dev/null 2>&1
}

start() {
  require_container_runtime
  ensure_network

  start_bitcoind
  wait_until "bitcoind container running" container_running "$BITCOIND_CONTAINER"
  wait_until_container_condition "bitcoind RPC" "$BITCOIND_CONTAINER" bitcoin_cli getblockchaininfo
  ensure_bitcoin_wallet
  ensure_mature_bitcoin_funds

  start_litd
  wait_until "litd container running" container_running "$LITD_CONTAINER"
  wait_until_container_condition "litd LND TLS certificate" "$LITD_CONTAINER" test -f "$STATE_DIR/lnd/tls.cert"
  wait_until_container_condition "litd LND admin macaroon" "$LITD_CONTAINER" test -f "$STATE_DIR/lnd/data/chain/bitcoin/regtest/admin.macaroon"
  wait_until_container_condition "litd TLS certificate" "$LITD_CONTAINER" test -f "$STATE_DIR/lit/tls.cert"
  wait_until_container_condition "litd macaroon" "$LITD_CONTAINER" test -f "$STATE_DIR/lit/regtest/lit.macaroon"
  wait_until_container_condition "litd taproot-assets macaroon" "$LITD_CONTAINER" test -f "$STATE_DIR/tapd/data/regtest/admin.macaroon"
  wait_until_container_condition "litd subservers" "$LITD_CONTAINER" litd_status_ready
  mine_blocks 1
  wait_until_container_condition "litd LND chain sync" "$LITD_CONTAINER" litd_synced
  wait_until_container_condition "litd taproot-assets RPC" "$LITD_CONTAINER" taproot_assets_ready
  wait_until_container_condition "litd asset-channel RPC" "$LITD_CONTAINER" asset_channel_rpc_ready
  fund_litd_wallet
  mine_blocks 1
  wait_until_container_condition "litd LND chain sync after funding" "$LITD_CONTAINER" litd_synced

  ready_report
}

stop() {
  require_container_runtime
  if container_running "$LITD_CONTAINER"; then
    "$CONTAINER_RUNTIME_BIN" stop "$LITD_CONTAINER" >/dev/null
  fi
}

status() {
  require_container_runtime
  "$CONTAINER_RUNTIME_BIN" ps \
    --filter "name=$BITCOIND_CONTAINER" \
    --filter "name=$LITD_CONTAINER" \
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
  "litd": {
    "container_name": "$LITD_CONTAINER",
    "image": "$LITD_IMAGE",
    "p2p_url": "127.0.0.1:$LITD_HOST_P2P_PORT",
    "lnd_grpc_url": "127.0.0.1:$LITD_HOST_GRPC_PORT",
    "lnd_rest_url": "https://127.0.0.1:$LITD_HOST_REST_PORT",
    "https_url": "https://127.0.0.1:$LITD_HOST_HTTPS_PORT",
    "lnd_macaroon_path": "$STATE_DIR/lnd/data/chain/bitcoin/regtest/admin.macaroon",
    "lnd_tls_cert_path": "$STATE_DIR/lnd/tls.cert",
    "lit_macaroon_path": "$STATE_DIR/lit/regtest/lit.macaroon",
    "lit_tls_cert_path": "$STATE_DIR/lit/tls.cert",
    "tapd_macaroon_path": "$STATE_DIR/tapd/data/regtest/admin.macaroon"
  }
}
JSON
}

ready_report() {
  local runtime bitcoind_height lnd_info lnd_balance lnd_pubkey lnd_height lnd_synced_flag
  local tapd_info tapd_pubkey tapd_height tapd_synced_flag lit_status asset_channel_rpc_flag

  runtime="$(runtime_name 2>/dev/null || true)"
  bitcoind_height="$(bitcoin_cli getblockcount 2>/dev/null || true)"
  lnd_info="$(litd_ln_cli getinfo 2>/dev/null || true)"
  lnd_balance="$(litd_wallet_balance_sat 2>/dev/null || true)"
  lnd_pubkey="$(printf '%s' "$lnd_info" | json_value '.identity_pubkey')"
  lnd_height="$(printf '%s' "$lnd_info" | json_value '.block_height')"
  lnd_synced_flag="$(printf '%s' "$lnd_info" | json_value '.synced_to_chain')"
  tapd_info="$(litd_tap_cli getinfo 2>/dev/null || true)"
  tapd_pubkey="$(printf '%s' "$tapd_info" | json_value '.lnd_identity_pubkey')"
  tapd_height="$(printf '%s' "$tapd_info" | json_value '.block_height')"
  tapd_synced_flag="$(printf '%s' "$tapd_info" | json_value '.sync_to_chain')"
  lit_status="$(litd_cli status 2>/dev/null | jq -s '.[0]' 2>/dev/null || echo '{}')"
  if asset_channel_rpc_ready >/dev/null 2>&1; then
    asset_channel_rpc_flag=true
  else
    asset_channel_rpc_flag=false
  fi

  jq -n \
    --arg runtime "$runtime" \
    --arg network "$NETWORK_NAME" \
    --arg state_dir "$STATE_DIR" \
    --arg bitcoind_container "$BITCOIND_CONTAINER" \
    --arg bitcoind_image "$BITCOIND_IMAGE" \
    --arg bitcoind_height "$bitcoind_height" \
    --arg litd_container "$LITD_CONTAINER" \
    --arg litd_image "$LITD_IMAGE" \
    --arg lnd_pubkey "$lnd_pubkey" \
    --arg lnd_height "$lnd_height" \
    --arg lnd_synced "$lnd_synced_flag" \
    --arg lnd_balance "$lnd_balance" \
    --arg tapd_pubkey "$tapd_pubkey" \
    --arg tapd_height "$tapd_height" \
    --arg tapd_synced "$tapd_synced_flag" \
    --argjson asset_channel_rpc_ready "$asset_channel_rpc_flag" \
    --argjson lit_status "$lit_status" \
    '{
      network: "regtest",
      counterparty_topology: "integrated_litd",
      container_runtime: $runtime,
      container_network: $network,
      state_dir: $state_dir,
      bitcoind: {
        container_name: $bitcoind_container,
        image: $bitcoind_image,
        rpc_ready: ($bitcoind_height | length > 0),
        chain_height: (if ($bitcoind_height | test("^[0-9]+$")) then ($bitcoind_height | tonumber) else null end)
      },
      litd: {
        container_name: $litd_container,
        image: $litd_image,
        p2p_url: "127.0.0.1:'"$LITD_HOST_P2P_PORT"'",
        lnd_grpc_url: "127.0.0.1:'"$LITD_HOST_GRPC_PORT"'",
        lnd_rest_url: "https://127.0.0.1:'"$LITD_HOST_REST_PORT"'",
        https_url: "https://127.0.0.1:'"$LITD_HOST_HTTPS_PORT"'",
        lnd_rpc_ready: ($lnd_pubkey | length > 0),
        identity_pubkey: $lnd_pubkey,
        chain_height: (if ($lnd_height | test("^[0-9]+$")) then ($lnd_height | tonumber) else null end),
        synced_to_chain: ($lnd_synced == "true"),
        wallet_balance_sat: (if ($lnd_balance | test("^[0-9]+$")) then ($lnd_balance | tonumber) else null end),
        taproot_assets_rpc_ready: ($tapd_pubkey | length > 0),
        taproot_assets_identity_pubkey: $tapd_pubkey,
        taproot_assets_chain_height: (if ($tapd_height | test("^[0-9]+$")) then ($tapd_height | tonumber) else null end),
        taproot_assets_sync_to_chain: ($tapd_synced == "true"),
        asset_channel_rpc_ready: $asset_channel_rpc_ready,
        subservers: $lit_status.sub_servers
      }
    }'
}

balance() {
  local asset_id="${1:-}"
  if [ -z "$asset_id" ]; then
    echo "lightning-labs-litd-counterparty: balance requires an asset id." >&2
    exit 2
  fi

  require_container_runtime

  local balance_json observed_balance
  balance_json="$(litd_tap_cli assets balance \
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
    --arg source "lightning-labs-litd-counterparty-balance" \
    --arg asset_id "$asset_id" \
    --arg observed_balance "$observed_balance" \
    --argjson raw "$balance_json" \
    '{
      schema_version: 1,
      source: $source,
      asset_id: $asset_id,
      observed_balance: ($observed_balance | tonumber),
      raw_taproot_assets_balance: $raw
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
  balance) balance "${2:-}" ;;
  smoke) smoke ;;
  ""|-h|--help) usage ;;
  *)
    echo "lightning-labs-litd-counterparty: unknown command: $1" >&2
    usage >&2
    exit 2
    ;;
esac
