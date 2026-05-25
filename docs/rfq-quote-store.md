# RFQ Quote Store

Date: 2026-05-25

`tap-ldk` now has a bounded regtest RFQ quote store for the first stablecoin
demo. It is separate from wallet proof storage and normal BTC invoice logic:
the store binds asset payment metadata to a quote, and later asset-payment
code can consume the quote-derived BTC msat amount without changing ordinary
BTC payments.

## Commands

```bash
cargo run -p tap-ldk-cli -- rfq-store-init target/rfq-quotes.json
cargo run -p tap-ldk-cli -- rfq-register-real-scid target/rfq-quotes.json 42
cargo run -p tap-ldk-cli -- rfq-request target/rfq-quotes.json alice 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 250000 200 1111111111111111111111111111111111111111111111111111111111111111 path-a-demo-1 100
cargo run -p tap-ldk-cli -- rfq-accept target/rfq-quotes.json '<quote-id>' 110
cargo run -p tap-ldk-cli -- rfq-authorize-htlc target/rfq-quotes.json '<quote-id>' 120
cargo run -p tap-ldk-cli -- rfq-quotes target/rfq-quotes.json
```

Reject and expiry paths are explicit:

```bash
cargo run -p tap-ldk-cli -- rfq-reject target/rfq-quotes.json '<quote-id>' 110 no-route
cargo run -p tap-ldk-cli -- rfq-expire target/rfq-quotes.json '<quote-id>' 201
```

## Bounded Regtest Oracle

The first-demo oracle is fixed and synthetic:

- ticker: `OPENUSD`
- conversion: `1 OPENUSD atomic unit = 100 msat`

The quote ID binds peer, asset ID, asset amount, derived BTC msat amount,
expiry, invoice context, RFQ SCID alias, and replay domain. Any tampering with
those fields fails store validation on load.

## Invariants

- A quote can be accepted only before expiry.
- A quote-derived HTLC authorization can be consumed only once.
- A used replay domain cannot authorize another quote.
- A requested or accepted quote owns a live RFQ SCID alias.
- RFQ SCID aliases cannot collide with real local channel SCIDs or other live
  quote aliases.
- Terminal quotes release live aliases.
- This store does not alter normal BTC invoice behavior.

The matching model boundary is `formal/tla/rfq_lifecycle/`.
