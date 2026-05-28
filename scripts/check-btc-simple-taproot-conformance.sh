#!/usr/bin/env bash
set -euo pipefail

expected_rev="8a54739ac030ba3e439496eacb7e1c1216e11c6f"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
rust_lightning_dir="${OPENAGENTS_RUST_LIGHTNING_DIR:-$repo_root/../.worktrees/rust-lightning}"

if [[ ! -f "$rust_lightning_dir/lightning/Cargo.toml" ]]; then
  echo "Rust Lightning checkout not found at $rust_lightning_dir" >&2
  echo "Set OPENAGENTS_RUST_LIGHTNING_DIR to the OpenAgentsInc rust-lightning checkout." >&2
  exit 1
fi

actual_rev="$(git -C "$rust_lightning_dir" rev-parse HEAD)"
if [[ "$actual_rev" != "$expected_rev" ]]; then
  echo "Rust Lightning checkout is $actual_rev, expected $expected_rev" >&2
  echo "Update the checkout or set OPENAGENTS_RUST_LIGHTNING_DIR to the pinned checkout." >&2
  exit 1
fi

echo "Running BTC-only simple-taproot conformance gate at $expected_rev"
(
  cd "$rust_lightning_dir"
  # The broad simple_taproot filter includes the focused BTC-only lifecycle,
  # cooperative-close, force-close, nonce, funding, and legacy-isolation tests.
  cargo test -p lightning --features simple_taproot_musig2 simple_taproot -- --nocapture
  cargo check -p lightning --features simple_taproot_musig2
  cargo check -p lightning
)
