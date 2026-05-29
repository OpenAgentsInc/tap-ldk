# Simple Taproot Splice Nonce-Map Policy

Date: 2026-05-28

Issue #90 recorded the original first-demo boundary. Issue #92 replaces that
BTC-level gap with bounded BOLT simple-taproot splice nonce-map support.

The first Taproot Assets demo still keeps one asset-channel funding outpoint
from open through payment, restart/reestablish, cooperative close, and
force-close. It does not claim splice/RBF asset-channel support.

## What Is Covered

The BOLT simple-taproot draft requires every active funding txid, including
concurrent splice candidates, to have its own type-22 nonce-map entry. The
OpenAgentsInc `rust-lightning` fork now validates nonce maps for current,
pending splice, and RBF funding txids, with bounded tests for:

- missing nonce-map entries;
- empty nonce-map entries;
- duplicate nonce-map entries;
- unknown funding txids;
- scalar fallback while multiple funding txids are active;
- reused public nonces across distinct funding txids.

The fork also serializes/deserializes the pending splice candidate and
counterparty nonce-map state so reestablish can continue from the same active
funding set.

## Machine-Readable Policy

The policy lives in `tap_ldk_core::demo_scope::first_demo_protocol_scope` and is
exposed with:

```bash
cargo run -p tap-ldk-cli -- first-demo-scope
```

Expected policy:

```json
{
  "simple_taproot_splicing": {
    "feature": "simple-taproot splice nonce maps",
    "policy": "bolt-base-supported",
    "first_public_demo": false,
    "covered_by_issue": "#92"
  }
}
```

The verification wrapper is:

```bash
./scripts/check-simple-taproot-splice-policy.sh
```

It checks the machine-readable policy and runs the required Rust Lightning
filters against the pinned fork:

```bash
cargo test -p lightning final_simple_taproot_uses_nonce_maps --features simple_taproot_musig2 -- --nocapture
cargo test -p lightning simple_taproot --features simple_taproot_musig2 -- --nocapture
cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture
cargo check -p lightning --features simple_taproot_musig2
```

## Remaining Boundary

Do not treat this as a production-complete BOLT claim yet. The remaining BOLT
work is #93 close-RBF nonce rotation and #94 full vector/unilateral spend-path
replay.

Any Taproot Asset channel claim that uses concurrent splice/RBF candidates must
add asset-state and proof-transition coverage for every active funding txid.
