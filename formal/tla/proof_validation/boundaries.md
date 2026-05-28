# Proof Validation Boundaries

This model covers the policy enforced by
`crates/tap-ldk-core/src/proof.rs`:

- proof scope must be `semantic-ancestry`;
- network must be regtest;
- asset type must be the first-demo normal asset type;
- genesis and anchor outpoints must be strict and non-cyclic;
- root sum must equal amount;
- root hash must match the accepted asset leaf;
- expected asset, owner, amount, genesis, anchor, and stale-anchor checks must
  fail closed;
- Lightning Labs `TAPF` imports must agree with the latest decoded asset leaf.

The model does not prove Bitcoin transaction inclusion, script execution,
Taproot control-block validity, or historical proof-chain replay. Those are
protocol-vector and #71 production-hardening surfaces.
