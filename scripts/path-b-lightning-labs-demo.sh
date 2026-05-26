#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "path-b-lightning-labs-demo: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_PATH_B_ARTIFACT_DIR:-$ROOT/target/path-b-lightning-labs-demo/$STAMP}"
LOG_DIR="$ARTIFACT_DIR/logs"
TAPCHANNEL_FIXTURE_DIR="$ROOT/fixtures/lightning-labs/tapchannelmsg/testdata"
PROOF_FIXTURE_DIR="$ROOT/fixtures/lightning-labs/proof/testdata"
ASSET_ID="7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93"
SUMMARY="$ARTIFACT_DIR/summary.txt"
DEPENDENCY_GAP="$ARTIFACT_DIR/lightning-labs-counterparty-gap.txt"
DOCKER_APP_BIN="/Applications/Docker.app/Contents/Resources/bin/docker"

mkdir -p "$LOG_DIR"

run_json() {
  local name="$1"
  local output="$2"
  shift 2
  echo "path-b-lightning-labs-demo: $name"
  "$@" >"$output" 2>"$LOG_DIR/$name.err"
}

run_log() {
  local name="$1"
  shift
  echo "path-b-lightning-labs-demo: $name"
  "$@" >"$LOG_DIR/$name.out" 2>"$LOG_DIR/$name.err"
}

run_optional_log() {
  local name="$1"
  shift
  echo "path-b-lightning-labs-demo: $name"
  set +e
  "$@" >"$LOG_DIR/$name.out" 2>"$LOG_DIR/$name.err"
  local status=$?
  set -e
  echo "$status" >"$LOG_DIR/$name.status"
  return "$status"
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

write_versions() {
  local runtime
  runtime="$(detect_container_runtime || true)"
  {
    echo "tap-ldk path-b versions"
    echo "date_utc=$STAMP"
    printf "git_commit="
    git rev-parse HEAD
    printf "rustc="
    rustc --version 2>/dev/null || true
    printf "cargo="
    cargo --version 2>/dev/null || true
    printf "container_runtime="
    if [ -n "$runtime" ]; then
      echo "$runtime"
      printf "container_runtime_version="
      "$runtime" --version 2>/dev/null || true
    else
      echo "unavailable"
    fi
  } >"$ARTIFACT_DIR/versions.txt"
}

try_counterparty() {
  local runtime
  runtime="$(detect_container_runtime || true)"
  if [ -z "$runtime" ]; then
    cat >"$DEPENDENCY_GAP" <<GAP
Neither Docker nor Podman is installed. Path B fixture-backed checks ran, but
the independent Lightning Labs LND/tapd counterparty was not started.
GAP
    return 0
  fi

  if ! "$runtime" info >"$LOG_DIR/container-runtime-info.out" 2>"$LOG_DIR/container-runtime-info.err"; then
    cat >"$DEPENDENCY_GAP" <<GAP
$runtime is installed, but its daemon/machine is not available. Path B
fixture-backed checks ran, but the independent Lightning Labs LND/tapd
counterparty was not started.
GAP
    return 0
  fi

  if TAP_LDK_CONTAINER_RUNTIME="$runtime" run_optional_log lightning-labs-counterparty-smoke ./scripts/lightning-labs-counterparty.sh smoke; then
    echo "Lightning Labs counterparty smoke completed with $runtime." >"$DEPENDENCY_GAP"
  else
    cat >"$DEPENDENCY_GAP" <<GAP
Lightning Labs counterparty smoke failed with $runtime. See:
- $LOG_DIR/lightning-labs-counterparty-smoke.out
- $LOG_DIR/lightning-labs-counterparty-smoke.err
GAP
  fi
}

echo "path-b-lightning-labs-demo: artifacts=$ARTIFACT_DIR"

write_versions
run_json counterparty-config "$ARTIFACT_DIR/lightning-labs-counterparty-config.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-counterparty-config
run_json live-tap-ldk-peer "$ARTIFACT_DIR/live-tap-ldk-peer.json" \
  cargo run -q -p tap-ldk-cli -- live-peer-smoke "$ARTIFACT_DIR/live-tap-ldk-peer-saved.json" "$ASSET_ID"
run_log counterparty-status ./scripts/lightning-labs-counterparty.sh status || true
try_counterparty

run_json blob-fixtures "$ARTIFACT_DIR/lightning-labs-blob-fixtures.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-blob-fixture-smoke "$TAPCHANNEL_FIXTURE_DIR"
run_json proof-fixtures "$ARTIFACT_DIR/lightning-labs-proof-fixtures.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-proof-fixture-smoke "$PROOF_FIXTURE_DIR"
run_json funding-interop "$ARTIFACT_DIR/lightning-labs-funding-interop-report.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-funding-interop-smoke \
    "$TAPCHANNEL_FIXTURE_DIR" "$ARTIFACT_DIR/lightning-labs-funding-interop-store.json"
run_json rfq-invoice "$ARTIFACT_DIR/lightning-labs-rfq-invoice.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-rfq-invoice-compat-smoke "$ASSET_ID"
run_json outgoing-payment "$ARTIFACT_DIR/lightning-labs-outgoing-payment-report.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-outgoing-payment-smoke \
    "$TAPCHANNEL_FIXTURE_DIR" "$ARTIFACT_DIR/lightning-labs-outgoing-payment-store.json"
run_json incoming-payment "$ARTIFACT_DIR/lightning-labs-incoming-payment-report.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-incoming-payment-smoke \
    "$TAPCHANNEL_FIXTURE_DIR" "$ARTIFACT_DIR/lightning-labs-incoming-payment-store.json"
run_json interop-checks "$ARTIFACT_DIR/lightning-labs-interop-checks.stdout.json" \
  cargo run -q -p tap-ldk-cli -- lightning-labs-interop-check-smoke \
    "$TAPCHANNEL_FIXTURE_DIR" "$PROOF_FIXTURE_DIR" "$ARTIFACT_DIR/lightning-labs-interop-checks.json"

cat >"$SUMMARY" <<SUMMARY_TEXT
Path B Lightning Labs interop demo artifacts: $ARTIFACT_DIR

Independent counterparty:
- target: Bitcoin Core 30.0, LND 0.19.0-beta, tapd 0.7.0-alpha
- status/gap: $DEPENDENCY_GAP

Fixture-backed checks:
- live tap-ldk peer smoke: $ARTIFACT_DIR/live-tap-ldk-peer.json
- blob fixtures: $ARTIFACT_DIR/lightning-labs-blob-fixtures.json
- proof fixtures: $ARTIFACT_DIR/lightning-labs-proof-fixtures.json
- funding interop: $ARTIFACT_DIR/lightning-labs-funding-interop-report.json
- RFQ invoice compatibility: $ARTIFACT_DIR/lightning-labs-rfq-invoice.json
- tap-ldk pays Lightning Labs artifacts: $ARTIFACT_DIR/lightning-labs-outgoing-payment-report.json
- Lightning Labs pays tap-ldk artifacts: $ARTIFACT_DIR/lightning-labs-incoming-payment-report.json
- consolidated checks: $ARTIFACT_DIR/lightning-labs-interop-checks.json

Visible mocked/experimental pieces:
- issuer identity and price oracle remain bounded demo fixtures
- proof courier is local fixture/import-export plumbing
- live tap-ldk peer smoke is local tap-ldk to tap-ldk until the Lightning Labs
  daemon-backed peer session is wired
- LND/tapd are independent compatibility peers, not tap-ldk runtime sidecars
- live daemon settlement remains a documented gap until observed balances replace expected deltas
SUMMARY_TEXT

cat "$SUMMARY"
echo
echo "path-b-lightning-labs-demo: dependency gap"
cat "$DEPENDENCY_GAP"
echo
echo "path-b-lightning-labs-demo: consolidated checks"
cat "$ARTIFACT_DIR/lightning-labs-interop-checks.json"
