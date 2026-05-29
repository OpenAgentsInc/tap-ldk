# Assumptions

- The `lnd`/`tapd`/`litd` peer is modeled only as an independent counterparty.
- Proof sync, channel open, RFQ acceptance, and payment settlement are abstract
  handshake states.
- A known incompatibility can terminate as an explicit gap state.
- Balance agreement is a boolean comparison over both sides' reported asset
  balances after the payment.
