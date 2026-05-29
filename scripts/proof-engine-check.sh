#!/usr/bin/env bash
set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "proof-engine-check: unable to find repository root; run from inside the repo." >&2
  exit 1
fi
cd "$ROOT" || exit 1

STATUS=0

run_required() {
  local name="$1"
  shift
  echo "proof-engine-check: $name"
  "$@" || STATUS=$?
}

run_optional_extended() {
  local name="$1"
  shift
  if [ "${TAP_LDK_EXTENDED_CHECKS:-0}" = "1" ]; then
    run_required "$name" "$@"
  else
    echo "proof-engine-check: skipping $name; set TAP_LDK_EXTENDED_CHECKS=1 to run it."
  fi
}

run_required fmt cargo fmt --check
run_required locked-tests env CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-true}" cargo test --locked
run_required formal ./scripts/formal-check.sh
run_required rust-native-verification ./scripts/rust-verification-check.sh
run_required native-demo ./scripts/path-a-native-demo.sh

run_optional_extended btc-simple-taproot ./scripts/check-btc-simple-taproot-conformance.sh
run_optional_extended simple-taproot-cooperative-close ./scripts/check-simple-taproot-cooperative-close.sh
run_optional_extended simple-taproot-splice-policy ./scripts/check-simple-taproot-splice-policy.sh
run_optional_extended compatibility-demo ./scripts/path-b-lightning-labs-demo.sh

exit "$STATUS"
