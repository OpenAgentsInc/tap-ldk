# Native Asset Payment Smoke

Date: 2026-05-25

`tap-ldk` now has a bounded native asset payment path for the Path A demo. It
wires a receiver RFQ quote, quote-bound invoice, asset HTLC custom records,
final-hop validation, asset commitment update, settled HTLC state, and payment
state into one command.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-payment-smoke
```

The smoke starts from the regtest `OPENUSD` channel with `alice=700` and
`bob=300`, pays `125` units from Alice to Bob, and reports `alice=575` and
`bob=425` after the commitment and HTLC settlement states are durable. It also
round-trips the commitment, HTLC, and payment stores through JSON to model a
restart after settlement.

## Failure Coverage

The payment path rejects:

- wrong quote binding;
- wrong invoice payment hash;
- wrong HTLC asset metadata.

Each negative path confirms the channel balance, commitment number, HTLC store,
and payment store do not advance after failure.

## Boundaries

This is still a bounded native-to-native demo path. It does not yet perform
real onion forwarding, rust-lightning HTLC dispatch, cooperative close, proof
export, or Lightning Labs software interop.
