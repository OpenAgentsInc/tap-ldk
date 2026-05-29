#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIGHTNING_DIR="${TAP_LDK_RUST_LIGHTNING_DIR:-$ROOT_DIR/../.worktrees/rust-lightning}"
EXPECTED_RUST_LIGHTNING_REV="cac9764f5926b081034b88e4fa1c13cc691335c1"

if [ ! -d "$RUST_LIGHTNING_DIR/.git" ]; then
  echo "check-simple-taproot-splice-policy: missing rust-lightning checkout at $RUST_LIGHTNING_DIR" >&2
  exit 2
fi

cd "$ROOT_DIR"
scope_json="$(cargo run -q -p tap-ldk-cli -- first-demo-scope)"
printf '%s\n' "$scope_json" | jq -e '
  .schema_version == 1
  and .simple_taproot_splicing.feature == "simple-taproot concurrent splicing"
  and .simple_taproot_splicing.policy == "excluded"
  and .simple_taproot_splicing.first_public_demo == false
  and .simple_taproot_splicing.covered_by_issue == "#90"
  and (.simple_taproot_splicing.reason | contains("type-22 nonce maps"))
  and (.simple_taproot_splicing.reopen_before | index("#61 production/simple-taproot-complete claim") != null)
  and (.simple_taproot_splicing.verification | index("cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture") != null)
' >/dev/null

cd "$RUST_LIGHTNING_DIR"
actual_rev="$(git rev-parse HEAD)"
if [ "$actual_rev" != "$EXPECTED_RUST_LIGHTNING_REV" ]; then
  echo "check-simple-taproot-splice-policy: expected rust-lightning $EXPECTED_RUST_LIGHTNING_REV, got $actual_rev" >&2
  exit 1
fi

cargo test -p lightning --features simple_taproot_musig2 simple_taproot -- --nocapture
cargo test -p lightning --features simple_taproot_musig2 splic -- --nocapture
cargo check -p lightning --features simple_taproot_musig2
