# Simple Taproot Splice Policy

Date: 2026-05-28

Issue #90 is resolved by explicitly gating concurrent simple-taproot splicing
out of the first public demo.

The first demo keeps one funding outpoint from open through payment,
restart/reestablish, cooperative close, and force-close. It does not splice a
simple-taproot channel and does not claim splice/RBF asset-channel support.

## Why It Is Excluded

The BOLT simple-taproot draft requires every active funding txid, including
concurrent splice candidates, to have its own type-22 nonce-map entry. The
OpenAgentsInc `rust-lightning` fork validates nonce maps for current and
pending funding txids, but the current test suite does not yet contain bounded
simple-taproot splice vectors for:

- missing nonce-map entries;
- stale nonce-map entries;
- duplicate nonce-map entries;
- wrong-funding-txid nonce-map entries;
- multiple concurrent splice/RBF candidates for the same channel.

Until those tests exist, the first public demo treats concurrent splicing as
out of scope instead of implying coverage.

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
    "feature": "simple-taproot concurrent splicing",
    "policy": "excluded",
    "first_public_demo": false,
    "covered_by_issue": "#90"
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
cargo test -p lightning simple_taproot --features simple_taproot_musig2 -- --nocapture
cargo test -p lightning splic --features simple_taproot_musig2 -- --nocapture
cargo check -p lightning --features simple_taproot_musig2
```

## Reopen Boundary

This exclusion must be reopened or replaced before:

- #61 is described as production-complete simple-taproot support;
- any public demo splices a simple-taproot channel;
- any Taproot Asset channel claim depends on concurrent splice/RBF candidates.

The follow-up implementation must prove that every active current or splice
funding txid has exactly one valid type-22 nonce-map entry, and that missing,
stale, duplicate, or wrong-funding-txid entries fail closed without weakening
legacy BTC channel behavior.
