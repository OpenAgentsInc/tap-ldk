# RFQ Invoice Binding

Date: 2026-05-25

The first native RFQ invoice path keeps BOLT 11 untouched. The invoice string
is treated as opaque BTC invoice text, while Taproot Asset semantics live in
RFQ peer messages, quote storage, route aliases, and future HTLC custom
records.

## Native Peer Flow

1. Requester sends an `RfqRequest` peer message with RFQ ID, asset ID, asset
   amount, and invoice context.
2. Responder stores and accepts a quote using the fixed regtest oracle.
3. Responder replies with `RfqAccept`, which carries the RFQ ID, quote ID,
   BTC msat amount, quote expiry, and RFQ SCID alias.
4. Either side can use `RfqReject` with a reason instead of accepting.
5. The invoice binder checks an opaque BOLT 11 invoice string against the
   accepted quote before any asset HTLC is authorized.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- rfq-invoice-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
```

## Binding Rules

- The quote must be accepted and unexpired.
- Invoice expiry must be less than or equal to quote expiry.
- Peer, asset ID, asset amount, BTC msat amount, and invoice context must match
  the quote.
- Paying the quote-bound invoice consumes the quote once through the RFQ store.
- A replayed quote-bound invoice fails closed.
- Normal BTC payments are unaffected because the BOLT 11 text is not rewritten.

This is still a bounded demo invoice binder. Full BOLT 11 parsing, route-hint
encoding, and Lightning payment dispatch belong to the asset HTLC/payment
issues that follow.
