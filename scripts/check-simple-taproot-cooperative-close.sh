#!/usr/bin/env bash
set -euo pipefail

expected_rev="1e7b435a015dafb5cc314c135e2eebab18cf460f"
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

echo "Running simple-taproot cooperative-close gate at $expected_rev"
(
  cd "$rust_lightning_dir"
  cargo test -p lightning --features simple_taproot_musig2,simple_close simple_taproot -- --nocapture
  cargo test -p lightning --features simple_taproot_musig2,simple_close taproot_asset -- --nocapture
  cargo check -p lightning --features simple_taproot_musig2,simple_close
)
