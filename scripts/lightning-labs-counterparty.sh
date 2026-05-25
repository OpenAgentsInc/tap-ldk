#!/usr/bin/env bash
set -u

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
STATE_DIR="${TAP_LDK_LL_STATE_DIR:-$ROOT/.tap-ldk/regtest/lightning-labs}"

usage() {
  cat <<USAGE
Usage: scripts/lightning-labs-counterparty.sh <command>

Commands:
  start       Start Bitcoin Core, LND, and tapd containers
  stop        Stop all counterparty containers
  status      Print best-effort daemon status
  connection  Print JSON connection material and local credential paths
  smoke       Start, print status, and stop

This harness starts Lightning Labs as an independent interop counterparty. It
does not run LND or tapd as a sidecar inside the native tap-ldk wallet.
USAGE
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "lightning-labs-counterparty: skipping; docker is not installed." >&2
    exit 0
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "lightning-labs-counterparty: skipping; docker daemon is not available." >&2
    exit 0
  fi
}

container_running() {
  docker ps --format '{{.Names}}' | grep -qx "$1"
}

ensure_network() {
  if ! docker network inspect "$NETWORK_NAME" >/dev/null 2>&1; then
    docker network create "$NETWORK_NAME" >/dev/null
  fi
}

start_bitcoind() {
  if container_running "$BITCOIND_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/bitcoind"
  docker run -d --rm \
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
  if container_running "$LND_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/lnd"
  docker run -d --rm \
    --name "$LND_CONTAINER" \
    --network "$NETWORK_NAME" \
    -p 127.0.0.1:10009:10009 \
    -p 127.0.0.1:18080:8080 \
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
  if container_running "$TAPD_CONTAINER"; then
    return 0
  fi

  mkdir -p "$STATE_DIR/tapd"
  docker run -d --rm \
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

start() {
  require_docker
  ensure_network
  start_bitcoind
  start_lnd
  start_tapd
  connection
}

stop() {
  require_docker
  for container in "$TAPD_CONTAINER" "$LND_CONTAINER" "$BITCOIND_CONTAINER"; do
    if container_running "$container"; then
      docker stop "$container" >/dev/null
    fi
  done
}

status() {
  require_docker
  docker ps \
    --filter "name=$BITCOIND_CONTAINER" \
    --filter "name=$LND_CONTAINER" \
    --filter "name=$TAPD_CONTAINER" \
    --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}'
}

connection() {
  cat <<JSON
{
  "network": "regtest",
  "docker_network": "$NETWORK_NAME",
  "state_dir": "$STATE_DIR",
  "bitcoind": {
    "container_name": "$BITCOIND_CONTAINER",
    "image": "$BITCOIND_IMAGE",
    "rpc_url": "http://127.0.0.1:18443",
    "rpc_user": "$RPC_USER",
    "rpc_password": "$RPC_PASSWORD"
  },
  "lnd": {
    "container_name": "$LND_CONTAINER",
    "image": "$LND_IMAGE",
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

smoke() {
  require_docker
  start >/dev/null
  trap stop EXIT
  status
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  connection) connection ;;
  smoke) smoke ;;
  ""|-h|--help) usage ;;
  *)
    echo "lightning-labs-counterparty: unknown command: $1" >&2
    usage >&2
    exit 2
    ;;
esac
