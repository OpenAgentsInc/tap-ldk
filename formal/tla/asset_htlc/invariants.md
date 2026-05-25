# Invariants

- Asset value is conserved across offer, settle, fail, and revoke transitions.
- An active asset HTLC requires an accepted quote.
- A settled asset HTLC must use the quote-derived BTC msat amount.
- A revoked offered state cannot later settle.
- The offered HTLC state requires a durable persistence checkpoint.
