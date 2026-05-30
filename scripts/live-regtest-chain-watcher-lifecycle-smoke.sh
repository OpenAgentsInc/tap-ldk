#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "live-regtest-chain-watcher-lifecycle-smoke: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

if ! command -v docker >/dev/null 2>&1; then
  echo "live-regtest-chain-watcher-lifecycle-smoke: docker is required for live regtest callback coverage." >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "live-regtest-chain-watcher-lifecycle-smoke: docker daemon is not reachable." >&2
  exit 1
fi

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_LIVE_REGTEST_CHAIN_WATCHER_ARTIFACT_DIR:-$ROOT/target/live-regtest-chain-watcher-lifecycle-smoke/$STAMP}"
START_STATUS="$ARTIFACT_DIR/start-chain-status.json"
OBSERVED_STATUS="$ARTIFACT_DIR/observed-chain-status.json"
CONNECTION="$ARTIFACT_DIR/bitcoin-connection.json"
MINED_HEIGHT="$ARTIFACT_DIR/mined-height.txt"
REPORT="$ARTIFACT_DIR/live-regtest-chain-watcher-lifecycle.json"

mkdir -p "$ARTIFACT_DIR"

./scripts/regtest-bitcoin.sh start >"$CONNECTION"
./scripts/regtest-bitcoin.sh status >"$START_STATUS"
START_HEIGHT="$(python3 - "$START_STATUS" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    print(json.load(handle)["blocks"])
PY
)"

./scripts/regtest-bitcoin.sh mine 1 >"$MINED_HEIGHT"
./scripts/regtest-bitcoin.sh status >"$OBSERVED_STATUS"

read -r OBSERVED_HEIGHT BEST_BLOCK_HASH <<EOF_STATUS
$(python3 - "$OBSERVED_STATUS" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    status = json.load(handle)
print(status["blocks"], status["bestblockhash"])
PY
)
EOF_STATUS

MINED_BLOCKS="$((OBSERVED_HEIGHT - START_HEIGHT))"
if [ "$MINED_BLOCKS" -lt 1 ]; then
  echo "live-regtest-chain-watcher-lifecycle-smoke: expected at least one mined block, got $MINED_BLOCKS." >&2
  exit 1
fi

cargo run -q -p tap-ldk-cli -- \
  live-regtest-chain-watcher-lifecycle-smoke \
  "$START_HEIGHT" \
  "$OBSERVED_HEIGHT" \
  "$MINED_BLOCKS" \
  "$BEST_BLOCK_HASH" >"$REPORT"

python3 - "$REPORT" "$START_HEIGHT" "$OBSERVED_HEIGHT" "$MINED_BLOCKS" "$BEST_BLOCK_HASH" <<'PY'
import json
import sys

report_path, start_height, observed_height, mined_blocks, block_hash = sys.argv[1:6]
start_height = int(start_height)
observed_height = int(observed_height)
mined_blocks = int(mined_blocks)

with open(report_path, "r", encoding="utf-8") as handle:
    report = json.load(handle)

if report.get("live_regtest_chain_backed") is not True:
    raise SystemExit("live regtest report did not claim live regtest backing")
if report.get("production_ready") is not False:
    raise SystemExit("live regtest report must not claim production readiness")
if report.get("all_callbacks_bound") is not True:
    raise SystemExit("live regtest report did not bind all callbacks")

snapshot = report.get("regtest_snapshot") or {}
if snapshot.get("network") != "regtest":
    raise SystemExit("live regtest snapshot network is not regtest")
if snapshot.get("source") != "bitcoin_core_regtest":
    raise SystemExit("live regtest snapshot source is not Bitcoin Core regtest")
if snapshot.get("start_height") != start_height:
    raise SystemExit("live regtest snapshot start height mismatch")
if snapshot.get("observed_height") != observed_height:
    raise SystemExit("live regtest snapshot observed height mismatch")
if snapshot.get("mined_blocks") != mined_blocks:
    raise SystemExit("live regtest snapshot mined block count mismatch")
if snapshot.get("best_block_hash") != block_hash:
    raise SystemExit("live regtest snapshot block hash mismatch")

chain_report = report.get("chain_observation_report") or {}
observations = chain_report.get("observations") or []
callbacks = report.get("callbacks") or []
if len(callbacks) != len(observations):
    raise SystemExit(
        f"live regtest report has {len(callbacks)} callbacks for {len(observations)} observations"
    )
observation_ids = {observation.get("observation_id") for observation in observations}
callback_observation_ids = {callback.get("observation_id") for callback in callbacks}
missing = sorted(observation_ids - callback_observation_ids)
if missing:
    raise SystemExit("live regtest callbacks missing observations: " + ", ".join(missing))
for callback in callbacks:
    if callback.get("height") != observed_height:
        raise SystemExit(f"callback {callback.get('callback_id')} height mismatch")
    if callback.get("block_hash") != block_hash:
        raise SystemExit(f"callback {callback.get('callback_id')} block hash mismatch")
PY

echo "live-regtest-chain-watcher-lifecycle-smoke: report=$REPORT"
