# Assumptions

- The model has one asset channel, one accepted quote, and one outgoing asset
  HTLC.
- The quoted BTC msat amount is fixed and exact.
- The model abstracts preimages, signatures, channel update messages, and
  Bitcoin timelocks into settle, fail, and revoke transitions.
- Durability is modeled as a boolean checkpoint before the HTLC can be treated
  as offered.
