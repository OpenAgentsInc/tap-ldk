# Assumptions

- The model is bounded to one issued asset supply of `100`.
- Outputs `a` and `b` stand in for verified spendable asset outputs.
- The channel balance is a single aggregate bucket for a valid asset-channel
  allocation.
- Cryptographic proof validation is abstracted as state transition choice; the
  model checks balance accounting, not signatures or Bitcoin consensus.

