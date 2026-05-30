#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "chain-watcher-lifecycle-smoke: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_CHAIN_WATCHER_LIFECYCLE_ARTIFACT_DIR:-$ROOT/target/chain-watcher-lifecycle-smoke/$STAMP}"
REPORT="$ARTIFACT_DIR/chain-watcher-lifecycle.json"

mkdir -p "$ARTIFACT_DIR"

cargo run -q -p tap-ldk-cli -- chain-watcher-lifecycle-smoke >"$REPORT"

python3 - "$REPORT" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as handle:
    report = json.load(handle)

required_true = [
    "all_lifecycle_events_observed",
    "confirmed_recovery_observed",
    "refusal_observations_present",
    "restart_observation_present",
]

missing = [field for field in required_true if report.get(field) is not True]
if missing:
    raise SystemExit(f"chain observation report missing true fields: {', '.join(missing)}")

if report.get("live_chain_watcher_backed") is not False:
    raise SystemExit("bounded chain observation report must not claim live chain watcher backing")

if report.get("production_ready") is not False:
    raise SystemExit("bounded chain observation report must not claim production readiness")

lifecycle_report = report.get("lifecycle_report") or {}
events = lifecycle_report.get("events") or []
observations = report.get("observations") or []
if not events:
    raise SystemExit("chain observation report has no lifecycle events")
if len(observations) != len(events):
    raise SystemExit(
        f"chain observation report has {len(observations)} observations for {len(events)} lifecycle events"
    )

event_ids = {event.get("event_id") for event in events}
observation_event_ids = {observation.get("lifecycle_event_id") for observation in observations}
missing_event_observations = sorted(event_ids - observation_event_ids)
if missing_event_observations:
    raise SystemExit(
        "chain observation report missing observations for lifecycle events: "
        + ", ".join(missing_event_observations)
    )

required_kinds = {
    "cooperative_close_anchor",
    "unilateral_commitment_anchor",
    "second_level_htlc_anchor",
    "final_sweep_anchor",
    "failed_sweep",
    "btc_only_sweep_refusal",
    "stale_proof_ownership_anchor",
    "missing_proof_ownership_refusal",
    "restart_evidence",
}
observed_kinds = {observation.get("kind") for observation in observations}
missing_kinds = sorted(required_kinds - observed_kinds)
if missing_kinds:
    raise SystemExit(f"chain observation report missing observation kinds: {', '.join(missing_kinds)}")

confirmed_recovery_kinds = {
    "unilateral_commitment_anchor",
    "second_level_htlc_anchor",
    "final_sweep_anchor",
}
for observation in observations:
    kind = observation.get("kind")
    status = observation.get("lifecycle_event_status")
    anchor_state = observation.get("anchor_state")
    if kind in confirmed_recovery_kinds and status == "asset_proof_recovered":
        if anchor_state != "confirmed":
            raise SystemExit(
                f"recovered observation {observation.get('observation_id')} is not confirmed"
            )
    if kind in {"failed_sweep", "btc_only_sweep_refusal", "stale_proof_ownership_anchor", "missing_proof_ownership_refusal"}:
        if status != "refused":
            raise SystemExit(
                f"refusal observation {observation.get('observation_id')} has status {status}"
            )
PY

echo "chain-watcher-lifecycle-smoke: report=$REPORT"
