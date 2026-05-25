#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "full-demo-smoke: unable to find repository root; run from inside the repo." >&2
  exit 1
fi

cd "$ROOT"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${TAP_LDK_FULL_DEMO_ARTIFACT_DIR:-$ROOT/target/full-demo-smoke/$STAMP}"
LOG_DIR="$ARTIFACT_DIR/logs"
SUMMARY="$ARTIFACT_DIR/summary.txt"

mkdir -p "$LOG_DIR"

run_path() {
  local name="$1"
  shift
  echo "full-demo-smoke: $name"
  "$@" >"$LOG_DIR/$name.out" 2>"$LOG_DIR/$name.err"
}

echo "full-demo-smoke: artifacts=$ARTIFACT_DIR"

export TAP_LDK_PATH_A_ARTIFACT_DIR="$ARTIFACT_DIR/path-a"
run_path path-a-native-demo ./scripts/path-a-native-demo.sh

export TAP_LDK_PATH_B_ARTIFACT_DIR="$ARTIFACT_DIR/path-b"
run_path path-b-lightning-labs-demo ./scripts/path-b-lightning-labs-demo.sh

cat >"$SUMMARY" <<SUMMARY_TEXT
Full tap-ldk demo smoke artifacts: $ARTIFACT_DIR

Path A artifacts:
- $ARTIFACT_DIR/path-a

Path B artifacts:
- $ARTIFACT_DIR/path-b

Logs:
- $LOG_DIR/path-a-native-demo.out
- $LOG_DIR/path-a-native-demo.err
- $LOG_DIR/path-b-lightning-labs-demo.out
- $LOG_DIR/path-b-lightning-labs-demo.err
SUMMARY_TEXT

cat "$SUMMARY"
