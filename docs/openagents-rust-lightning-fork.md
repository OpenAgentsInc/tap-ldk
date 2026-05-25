# OpenAgentsInc Rust-Lightning Fork

Date: 2026-05-25

The required `rust-lightning` fork for Taproot Asset channel work lives at:

- Fork: `https://github.com/OpenAgentsInc/rust-lightning`
- Upstream: `https://github.com/lightningdevkit/rust-lightning`
- Base revision: `0c37f08a55c0f7738f2691dc3690166fd42f851d`

This fork was created for issue #25 after the extension-boundary issue (#24)
identified hooks that must sit inside channel negotiation, funding,
commitment, HTLC, monitor persistence, close, and on-chain recovery logic.

## Current Wiring

`tap-ldk-core` has a direct git dependency on the forked `lightning` crate at
the pinned base revision. The dependency is intentionally narrow for this
phase: it proves CI can fetch and build against the OpenAgentsInc fork without
patching every transitive LDK crate before the fork contains asset-channel
changes.

Workspace metadata records the same fork in `Cargo.toml`:

```toml
[workspace.metadata.tap-ldk.rust-lightning-fork]
url = "https://github.com/OpenAgentsInc/rust-lightning.git"
upstream = "https://github.com/lightningdevkit/rust-lightning.git"
base_rev = "0c37f08a55c0f7738f2691dc3690166fd42f851d"
```

When the forked asset-channel code lands, the dependency strategy should move
from a direct touchpoint dependency to explicit `[patch.crates-io]` entries for
the LDK crates affected by the fork. Do that only in the issue that introduces
the forked code, so normal BTC behavior remains easy to compare against
upstream.

## Sync Process

1. Add upstream locally when working inside an owned fork clone:

   ```bash
   git remote add upstream https://github.com/lightningdevkit/rust-lightning.git
   git fetch upstream
   ```

2. Rebase or merge only after confirming normal upstream BTC-only tests still
   pass.
3. Keep asset-channel changes feature-gated and isolated to the boundaries in
   `docs/ldk-asset-channel-extension-boundary.md`.
4. Update this document, `Cargo.toml` workspace metadata, and the pinned Cargo
   dependency revision whenever the fork base changes.

## Drift Rules

- Reference clones under `projects/` remain read-only source material.
- The OpenAgentsInc fork is the implementation home for forked
  rust-lightning code.
- Do not weaken normal channel policy, monitor durability, or BTC-only tests to
  make asset-channel work pass.
- Any upstream conflict in monitor, HTLC, close, or recovery code is a protocol
  review event.
