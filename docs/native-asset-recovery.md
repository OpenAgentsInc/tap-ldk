# Native Asset Recovery Matrix

Date: 2026-05-25

`tap-ldk` now has a bounded restart recovery matrix for Path A native asset
channels. The matrix recovers funding, quote acceptance, HTLC-added,
commitment-signed, settled-payment, and close-preparation checkpoints, and it
refuses stale checkpoints whose commitment number is older than the latest
durable asset commitment state.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-recovery-smoke
```

The smoke round-trips channel, RFQ, HTLC, commitment, and payment state through
the project persistence codecs and reports recovered balances, quote state,
HTLC state, payment state, and close-preparation state. It also validates the
OpenAgentsInc rust-lightning proof-ownership recovery hook for commitment
force-close, second-level HTLC, and final sweep paths. A BTC-only sweep without
asset proof ownership is refused as asset recovery, missing proof ownership is
refused, stale proof ownership is refused, and the BTC-only restart abstraction
remains unaffected.

## Boundaries

This is still a bounded smoke, not a live on-chain resolver. It proves the
asset recovery claim cannot be made without proof ownership material and that
the three unilateral recovery spend paths have stable fork hook records. The
remaining live work is wiring those records through real channel-manager,
resolver, and sweeper call sites.
