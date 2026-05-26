# OpenAgentsInc Rust-Lightning Fork

Date: 2026-05-25

The required `rust-lightning` fork for Taproot Asset channel work lives at:

- Fork: `https://github.com/OpenAgentsInc/rust-lightning`
- Upstream: `https://github.com/lightningdevkit/rust-lightning`
- Base revision: `0c37f08a55c0f7738f2691dc3690166fd42f851d`
- Current `tap-ldk` revision: `0f442683da45af47daff313fefcfaef1ac7b82d7`

This fork was created for issue #25 after the extension-boundary issue (#24)
identified hooks that must sit inside channel negotiation, funding,
commitment, HTLC, monitor persistence, close, and on-chain recovery logic.

## Current Wiring

`tap-ldk-core` has a direct git dependency on the forked `lightning` crate at
the pinned current revision. The dependency is intentionally narrow for this
phase: it proves CI can fetch and build against the OpenAgentsInc fork while
keeping fork touchpoints explicit.

Workspace metadata records the same fork in `Cargo.toml`:

```toml
[workspace.metadata.tap-ldk.rust-lightning-fork]
url = "https://github.com/OpenAgentsInc/rust-lightning.git"
upstream = "https://github.com/lightningdevkit/rust-lightning.git"
base_rev = "0c37f08a55c0f7738f2691dc3690166fd42f851d"
rev = "0f442683da45af47daff313fefcfaef1ac7b82d7"
```

Revision `99ddb8b7033b3b5d056005c00ba650e716ed37da` added the first forked
asset-channel gate: an experimental Taproot Asset channel feature bit, config
opt-in, explicit single-asset channel type, and a public descriptor API that
binds asset ID plus protocol version.

Revision `84032b87d05a157ee9ef247102767bc100d84ed6` adds the bounded funding
controller hook. It validates pending channel ID, peer identities, asset ID,
genesis/group identity, proof-fragment completeness, proof root, funding
outpoint, output commitment, and local/remote allocation before asset-channel
funding state is allowed to advance.

Revision `4394c0e350dd5faf34ca37fc6bde5cc14497e3f9` adds the first channel
monitor aux blob surface for asset commitments. It exposes
`TaprootAssetMonitorAuxBlob`, `TaprootAssetMonitorAuxBlobExpectation`,
`ChannelMonitorUpdate::taproot_asset_aux_update`, and
`ChannelMonitorUpdate::require_taproot_asset_aux_blob`, with validation for
missing, stale, malformed, or digest-mismatched asset state.

Revision `ef2538fe181025231c1f2a946df713b3109fa9ef` adds the first asset HTLC
metadata and final-hop validation surface. It exposes
`TaprootAssetHtlcMetadata`, `TaprootAssetHtlcMetadataExpectation`,
`prepare_asset_htlc_metadata`, and `validate_asset_htlc_final_hop`, with
validation for missing, stale, malformed, wrong-asset, wrong-amount,
wrong-root, wrong-quote, wrong-payment, or digest-mismatched metadata.

Revision `d6862145b43225d5002445c3733e70293bb0646e` adds the first cooperative
close allocation surface. It exposes `TaprootAssetCloseAllocation`,
`TaprootAssetCloseAllocationExpectation`,
`prepare_cooperative_close_asset_allocation`, and
`validate_cooperative_close_asset_allocation`, with validation for missing,
stale, malformed, wrong-asset, wrong-amount, wrong-root, or digest-mismatched
close allocations.

Revision `0f442683da45af47daff313fefcfaef1ac7b82d7` adds the first
force-close and sweep proof-ownership recovery surface. It exposes
`TaprootAssetProofOwnershipState`, `TaprootAssetProofOwnershipExpectation`,
`prepare_asset_proof_ownership_recovery`, and
`validate_asset_proof_ownership_recovery`, with validation for missing, stale,
wrong-path, malformed, partial BTC-only, missing-proof-material, or
digest-mismatched recovery state.

As broader forked code lands, the dependency strategy may need to move from a
direct touchpoint dependency to explicit `[patch.crates-io]` entries for the
LDK crates affected by the fork. Do that only when the fork changes require
patching transitive LDK crates, so normal BTC behavior remains easy to compare
against upstream.

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
