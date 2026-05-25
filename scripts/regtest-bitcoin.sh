#!/usr/bin/env bash
set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "regtest-bitcoin: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

CONTAINER_NAME="${TAP_LDK_BITCOIN_CONTAINER:-tap-ldk-bitcoin-regtest}"
IMAGE="${TAP_LDK_BITCOIN_IMAGE:-bitcoin/bitcoin:30.0}"
RPC_HOST="${TAP_LDK_BITCOIN_RPC_HOST:-127.0.0.1}"
RPC_PORT="${TAP_LDK_BITCOIN_RPC_PORT:-18443}"
RPC_USER="${TAP_LDK_BITCOIN_RPC_USER:-tapldk}"
RPC_PASSWORD="${TAP_LDK_BITCOIN_RPC_PASSWORD:-tapldk-regtest}"
STATE_DIR="${TAP_LDK_REGTEST_DIR:-$ROOT/.tap-ldk/regtest/bitcoin}"

usage() {
  cat <<USAGE
Usage: scripts/regtest-bitcoin.sh <command>

Commands:
  start       Start Bitcoin Core regtest in Docker
  stop        Stop the regtest container
  status      Print bitcoin-cli getblockchaininfo
  mine [n]    Mine n blocks, default 1
  address     Create and print a new regtest address
  fund <addr> <btc>
              Send BTC to addr and mine one confirmation
  connection  Print JSON connection material
  smoke       Start, mine one block, print status, and stop

Environment overrides:
  TAP_LDK_BITCOIN_IMAGE
  TAP_LDK_BITCOIN_CONTAINER
  TAP_LDK_BITCOIN_RPC_HOST
  TAP_LDK_BITCOIN_RPC_PORT
  TAP_LDK_BITCOIN_RPC_USER
  TAP_LDK_BITCOIN_RPC_PASSWORD
  TAP_LDK_REGTEST_DIR
USAGE
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "regtest-bitcoin: skipping; docker is not installed." >&2
    exit 0
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "regtest-bitcoin: skipping; docker daemon is not available." >&2
    exit 0
  fi
}

is_running() {
  docker ps --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"
}

rpc() {
  docker exec "$CONTAINER_NAME" bitcoin-cli \
    -regtest \
    -rpcuser="$RPC_USER" \
    -rpcpassword="$RPC_PASSWORD" \
    "$@"
}

wait_ready() {
  for _ in $(seq 1 60); do
    if rpc getblockchaininfo >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "regtest-bitcoin: Bitcoin Core RPC did not become ready." >&2
  docker logs "$CONTAINER_NAME" >&2 || true
  exit 1
}

start() {
  require_docker
  mkdir -p "$STATE_DIR"

  if is_running; then
    wait_ready
    connection
    return 0
  fi

  docker run -d --rm \
    --name "$CONTAINER_NAME" \
    -p "$RPC_HOST:$RPC_PORT:18443" \
    -v "$STATE_DIR:/home/bitcoin/.bitcoin" \
    "$IMAGE" \
    bitcoind \
    -regtest=1 \
    -server=1 \
    -printtoconsole=1 \
    -fallbackfee=0.0002 \
    -rpcbind=0.0.0.0 \
    -rpcallowip=0.0.0.0/0 \
    -rpcuser="$RPC_USER" \
    -rpcpassword="$RPC_PASSWORD" >/dev/null

  wait_ready
  connection
}

stop() {
  require_docker
  if is_running; then
    docker stop "$CONTAINER_NAME" >/dev/null
  fi
}

address() {
  require_docker
  wait_ready
  rpc -named getnewaddress address_type=bech32m
}

mine() {
  require_docker
  wait_ready
  local blocks="${1:-1}"
  local addr
  addr="$(address)"
  rpc generatetoaddress "$blocks" "$addr" >/dev/null
  rpc getblockcount
}

fund() {
  require_docker
  wait_ready
  if [ "$#" -ne 2 ]; then
    echo "regtest-bitcoin: fund requires <addr> <btc>." >&2
    exit 2
  fi
  rpc sendtoaddress "$1" "$2" >/dev/null
  mine 1 >/dev/null
}

status() {
  require_docker
  wait_ready
  rpc getblockchaininfo
}

connection() {
  cat <<JSON
{
  "network": "regtest",
  "rpc_url": "http://$RPC_HOST:$RPC_PORT",
  "rpc_user": "$RPC_USER",
  "rpc_password": "$RPC_PASSWORD",
  "container_name": "$CONTAINER_NAME",
  "image": "$IMAGE",
  "state_dir": "$STATE_DIR"
}
JSON
}

smoke() {
  require_docker
  start >/dev/null
  trap stop EXIT
  mine 1 >/dev/null
  status
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  status) status ;;
  mine) shift; mine "$@" ;;
  address) address ;;
  fund) shift; fund "$@" ;;
  connection) connection ;;
  smoke) smoke ;;
  ""|-h|--help) usage ;;
  *)
    echo "regtest-bitcoin: unknown command: $1" >&2
    usage >&2
    exit 2
    ;;
esac
