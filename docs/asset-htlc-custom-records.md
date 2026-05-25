# Asset HTLC Custom Records

Date: 2026-05-25

`tap-ldk` now has a bounded custom-record codec and final-hop validator for
quote-bound asset HTLCs. The asset metadata is carried outside the BOLT 11
invoice text and must match the accepted RFQ quote, invoice context, payment
hash, and quote-derived BTC msat amount before settlement or balance movement
is allowed.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-htlc-smoke
```

## Encoded Fields

The current record namespace begins at `760000` and encodes:

- asset ID;
- asset amount;
- quote ID;
- invoice context;
- quote-derived BTC msat amount;
- RFQ SCID alias;
- payment hash;
- final-hop digest.

Records outside the asset namespace are treated as BTC-only and are left
untouched. Partial, malformed, unknown-in-namespace, stale, wrong-asset,
wrong-amount, wrong-BTC-amount, wrong-quote, or wrong-payment-hash records fail
closed.

## Boundaries

This is the native custom-record and final-hop validation layer. It is ready
for fixture comparison, but it does not yet implement full onion forwarding,
real Lightning HTLC dispatch, Taproot Assets second-level HTLC scripts, or
Lightning Labs interop decoding.
