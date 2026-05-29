# Assumptions

- The model has one funded single-asset channel.
- The commitment number can advance from `0` to `1`.
- Signatures are abstracted to a domain label; only the asset domain can
  advance asset state.
- Asset nonces are represented by bounded identifiers.
- Durability is modeled as a boolean checkpoint attached to the latest asset
  state.
- Proof replay is modeled by a commitment-number checkpoint. An accepted
  restart requires that checkpoint to match the latest commitment number.
