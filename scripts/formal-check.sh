#!/usr/bin/env bash
set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "formal-check: unable to find repository root; run from inside the repo." >&2
  exit 1
fi
cd "$ROOT" || exit 1

RUNNER_KIND=""
if command -v tlc >/dev/null 2>&1; then
  RUNNER_KIND="tlc"
elif [ -n "${TLA_TOOLS_JAR:-}" ]; then
  if [ ! -r "$TLA_TOOLS_JAR" ]; then
    echo "formal-check: TLA_TOOLS_JAR is set but is not readable." >&2
    exit 1
  fi
  if ! command -v java >/dev/null 2>&1; then
    echo "formal-check: TLA_TOOLS_JAR is set but java is not available." >&2
    exit 1
  fi
  RUNNER_KIND="jar"
else
  echo "formal-check: skipping; no TLA+ runner found. Install tlc or set TLA_TOOLS_JAR."
  exit 0
fi

CONFIGS="$(git ls-files 'formal/tla/*/*.cfg' | sort)"
if [ -z "$CONFIGS" ]; then
  echo "formal-check: no checked-in formal/tla/*/*.cfg files found."
  exit 0
fi

STATUS=0
while IFS= read -r CONFIG; do
  [ -n "$CONFIG" ] || continue
  SPEC="${CONFIG%.cfg}.tla"
  if [ ! -f "$SPEC" ]; then
    echo "formal-check: missing TLA+ spec for config: $CONFIG" >&2
    STATUS=1
    continue
  fi

  echo "formal-check: running $CONFIG"
  if [ "$RUNNER_KIND" = "tlc" ]; then
    tlc -config "$CONFIG" "$SPEC" || STATUS=$?
  else
    java -cp "$TLA_TOOLS_JAR" tlc2.TLC -config "$CONFIG" "$SPEC" || STATUS=$?
  fi
done <<EOF
$CONFIGS
EOF

exit "$STATUS"
