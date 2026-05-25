# Invariants

- Asset value is conserved across offer, settle, fail, and revoke transitions.
- An active asset HTLC requires an accepted quote.
- A settled asset HTLC must use the quote-derived BTC msat amount.
- A revoked offered state cannot later settle.
- The offered HTLC state requires a durable persistence checkpoint.

## Implementation Mapping

- `crates/tap-ldk-core/src/asset_htlc.rs` enforces the quote-derived BTC msat
  amount and final-hop metadata validation before the bounded settlement path
  can advance.
- `crates/tap-ldk-core/src/asset_commitment.rs` supplies the durable balance
  transition used by the asset HTLC smoke before the HTLC is recorded as
  settled.
- `crates/tap-ldk-core/src/asset_payment.rs` composes those checks for the
  bounded native payment path and records a settled payment only after the
  commitment update and HTLC settlement stores both validate.
