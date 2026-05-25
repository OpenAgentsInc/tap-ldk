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
HTLC state, payment state, and close-preparation state. It also verifies the
BTC-only restart abstraction remains unaffected.

## Boundaries

Close preparation is only a durable marker for the next issue. Cooperative
close construction, proof export, force-close, and sweep recovery are not
implemented here.
