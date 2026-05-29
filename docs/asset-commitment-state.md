# Asset Commitment State

Date: 2026-05-25

`tap-ldk` has a bounded native asset commitment store layered on top of funded
single-asset channels. It tracks asset balances by Lightning commitment number,
revokes the previous asset state on each valid update, persists a commitment
monitor blob, builds a matching `rust-lightning` channel monitor aux blob, and
keeps asset signing/nonces in a separate domain from BTC commitment signing.
Each update also consumes the prior channel-locked proof-history output and
records the new channel-locked proof-history output for the latest commitment.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-commitment-smoke target/asset-commitments.json
cargo run -p tap-ldk-cli -- asset-commitment-list target/asset-commitments.json
cargo run -p tap-ldk-cli -- asset-commitment-state target/asset-commitments.json '<channel-id>'
```

## Current Scope

- Strict next commitment number: current `+ 1`.
- Local-to-remote and remote-to-local asset balance transitions.
- Conservation of total channel asset amount.
- Previous asset commitment revocation.
- Asset nonce reuse rejection.
- Bounded asset virtual transaction ID, witness digest, and signature context.
- Rejection of BTC-domain signatures in the asset-domain verifier.
- Durable monitor blob validation on restart.
- Rejection of missing or tampered LDK monitor aux blob digests before restart
  recovery is considered valid.
- Proof-history replay from funded channel state through each commitment
  update.
- Rejection of stale or mismatched proof-history metadata before a restarted
  commitment state is considered recovered.

## Boundaries

This is a deterministic bounded signing model, not production MuSig2. It is
intended to enforce the state-machine contracts that the
`OpenAgentsInc/rust-lightning` monitor integration and Taproot Assets witness
code must preserve. The proof-history replay here starts from the already
validated channel-funding output; it is not a full historical proof-file replay
back to every issuance and split proof. Full Taproot Assets witness
construction, real MuSig2 signing, and close/recovery proof replay remain
separate issues.
