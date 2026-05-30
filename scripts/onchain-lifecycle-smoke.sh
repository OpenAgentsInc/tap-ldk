#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "onchain-lifecycle-smoke: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_ONCHAIN_LIFECYCLE_ARTIFACT_DIR:-$ROOT/target/onchain-lifecycle-smoke/$STAMP}"
REPORT="$ARTIFACT_DIR/onchain-lifecycle.json"

mkdir -p "$ARTIFACT_DIR"

cargo run -q -p tap-ldk-cli -- onchain-lifecycle-smoke >"$REPORT"

python3 - "$REPORT" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as handle:
    report = json.load(handle)

required_true = [
    "cooperative_close_exported",
    "unilateral_recovery_explained",
    "second_level_success_explained",
    "second_level_timeout_explained",
    "final_sweep_explained",
    "failed_sweep_refused",
    "btc_only_sweep_refused",
    "restart_recovery_explained",
]

missing = [field for field in required_true if report.get(field) is not True]
if missing:
    raise SystemExit(f"lifecycle report missing true fields: {', '.join(missing)}")

if report.get("live_chain_watcher_backed") is not False:
    raise SystemExit("bounded lifecycle report must not claim live chain watcher backing")

if report.get("production_ready") is not False:
    raise SystemExit("bounded lifecycle report must not claim production readiness")

events = report.get("events", [])
required_kinds = {
    "cooperative_close_local",
    "cooperative_close_remote",
    "unilateral_commitment",
    "second_level_htlc_success",
    "second_level_htlc_timeout",
    "final_sweep",
    "failed_sweep",
    "btc_only_sweep_refusal",
    "stale_proof_ownership_refusal",
    "missing_proof_ownership_refusal",
    "restart_recovery",
}
observed_kinds = {event.get("kind") for event in events}
missing_kinds = sorted(required_kinds - observed_kinds)
if missing_kinds:
    raise SystemExit(f"lifecycle report missing event kinds: {', '.join(missing_kinds)}")
PY

echo "onchain-lifecycle-smoke: report=$REPORT"
