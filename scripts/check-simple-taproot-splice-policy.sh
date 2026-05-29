#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIGHTNING_DIR="${TAP_LDK_RUST_LIGHTNING_DIR:-$ROOT_DIR/../.worktrees/rust-lightning}"
EXPECTED_RUST_LIGHTNING_REV="90f2e34fac15b18011bee7d939cd9c80141f4b8e"

if [ ! -d "$RUST_LIGHTNING_DIR/.git" ]; then
  echo "check-simple-taproot-splice-policy: missing rust-lightning checkout at $RUST_LIGHTNING_DIR" >&2
  exit 2
fi

cd "$ROOT_DIR"
scope_json="$(cargo run -q -p tap-ldk-cli -- first-demo-scope)"
printf '%s\n' "$scope_json" | jq -e '
  .schema_version == 1
  and .simple_taproot_splicing.feature == "simple-taproot splice nonce maps"
  and .simple_taproot_splicing.policy == "bolt-base-supported"
  and .simple_taproot_splicing.first_public_demo == false
  and .simple_taproot_splicing.covered_by_issue == "#92"
  and (.simple_taproot_splicing.reason | contains("nonce-map coverage"))
  and (.simple_taproot_splicing.reopen_before | index("any Taproot Asset channel claim using concurrent splice/RBF candidates") != null)
  and (.simple_taproot_splicing.verification | index("cargo test -p lightning final_simple_taproot_uses_nonce_maps --features simple_taproot_musig2 -- --nocapture") != null)
  and (.simple_taproot_splicing.verification | index("cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture") != null)
' >/dev/null

cd "$RUST_LIGHTNING_DIR"
actual_rev="$(git rev-parse HEAD)"
if [ "$actual_rev" != "$EXPECTED_RUST_LIGHTNING_REV" ]; then
  echo "check-simple-taproot-splice-policy: expected rust-lightning $EXPECTED_RUST_LIGHTNING_REV, got $actual_rev" >&2
  exit 1
fi

cargo test -p lightning --features simple_taproot_musig2 final_simple_taproot_uses_nonce_maps -- --nocapture
cargo test -p lightning --features simple_taproot_musig2 simple_taproot -- --nocapture
cargo test -p lightning --features simple_taproot_musig2 splic -- --nocapture
cargo check -p lightning --features simple_taproot_musig2
