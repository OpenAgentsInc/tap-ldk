# Invariants

- A quote can be used at most once.
- A paid quote must be paid before quote and invoice expiry.
- Live RFQ aliases cannot collide with real channel SCIDs.
- Accepted quotes carry a live alias.
- Invoice expiry cannot outlive quote expiry in the bounded first-demo model.

## Implementation Mapping

- `crates/tap-ldk-core/src/rfq_quote_store.rs` enforces the quote lifecycle
  represented here for the current bounded regtest implementation.
- `RfqQuoteStatus::Used` corresponds to the model's paid/consumed terminal
  state; future HTLC settlement can refine the name without relaxing
  single-use replay protection.
- The implementation tracks quote SCID aliases against both real local SCIDs
  and other requested/accepted quote aliases.
- `crates/tap-ldk-core/src/rfq_invoice.rs` enforces the invoice-expiry boundary
  by rejecting invoice bindings whose expiry outlives the accepted quote.
