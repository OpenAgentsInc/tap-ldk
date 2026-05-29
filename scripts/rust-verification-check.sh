#!/usr/bin/env bash
set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$ROOT" ]; then
  echo "rust-verification-check: unable to find repository root; run from inside the repo." >&2
  exit 1
fi
cd "$ROOT" || exit 1

STATUS=0

echo "rust-verification-check: property tests"
CARGO_NET_GIT_FETCH_WITH_CLI="${CARGO_NET_GIT_FETCH_WITH_CLI:-true}" \
  cargo test -p tap-ldk-core --test proof_replay_properties || STATUS=$?

if command -v cargo-fuzz >/dev/null 2>&1 || cargo fuzz --help >/dev/null 2>&1; then
  for target in \
    tlv_decode \
    tapd_proof_file \
    virtual_psbt_summary \
    taproot_commitment_leaf \
    lightning_labs_blobs
  do
    echo "rust-verification-check: fuzz smoke $target"
    cargo fuzz run "$target" -- -runs="${FUZZ_RUNS:-1}" || STATUS=$?
  done
else
  echo "rust-verification-check: skipping fuzz smoke; cargo-fuzz is not installed."
fi

if cargo kani --version >/dev/null 2>&1; then
  echo "rust-verification-check: kani"
  cargo kani -p tap-ldk-core || STATUS=$?
else
  echo "rust-verification-check: skipping Kani; cargo-kani is not installed."
fi

exit "$STATUS"
