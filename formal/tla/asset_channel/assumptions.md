# Asset Channel Phase 1 Assumptions

This model is an abstract state-machine model for the first asset-channel
funding contract. It is not a line-by-line proof of Rust code or BLIP-0029.

Assumptions:

- The bounded model has exactly two peers: local and remote.
- Both peers must negotiate asset-channel support before open.
- Proof data is modeled symbolically as none, complete, valid, or invalid.
- The model uses one asset ID and one fixed maximum asset amount.
- Full proof history and universe retrieval are outside this phase.
- Cryptographic verification is represented as a symbolic proof-validation
  transition.
- The model traces are synthetic and contain no real keys, proofs, wallet
  data, or payment data.
