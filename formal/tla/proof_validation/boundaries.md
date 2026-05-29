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

`ProofValidation.tla` also covers the next proof-history replay boundary added
for #97. It models a bounded valid path through well-formed proof import,
issuance, accepted issuance, split, transfer, channel funding, commitment
update, close, and sweep. It also models invalid paths for wrong genesis,
wrong anchor, wrong owner script key, wrong asset type, wrong amount, wrong
root hash, wrong root sum, mismatched TapCommitment output root, invalid split
sum, malformed proof-file transport, missing STXO, stale proof, and
reorg-sensitive history.

The checked invariants are narrow:

- an accepted balance must have a valid issuance history;
- accepted asset ID, amount, owner, anchor, and symbolic root must agree;
- accepted states require a well-formed proof file, present STXO, stable chain
  view, and no recorded bad proof reason;
- invalid or reorg-sensitive paths cannot end in an accepted balance;
- accepted amount cannot exceed the modeled issued supply.

The model does not prove Bitcoin transaction inclusion, script execution,
Taproot control-block validity, MuSig2 correctness, real hash preimages,
database crash consistency, peer networking, or full historical proof-chain
storage. Those are covered by protocol-vector tests, Rust regression tests,
and later production-hardening issues.
