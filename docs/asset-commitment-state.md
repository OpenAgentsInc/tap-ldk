# Asset Commitment State

Date: 2026-05-25

`tap-ldk` has a bounded native asset commitment store layered on top of funded
single-asset channels. It tracks asset balances by Lightning commitment number,
revokes the previous asset state on each valid update, persists a commitment
monitor blob, builds a matching `rust-lightning` channel monitor aux blob, and
keeps asset signing/nonces in a separate domain from BTC commitment signing.

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

## Boundaries

This is a deterministic bounded signing model, not production MuSig2. It is
intended to enforce the state-machine contracts that the
`OpenAgentsInc/rust-lightning` monitor integration and Taproot Assets witness
code must preserve. Full Taproot Assets witness construction, real MuSig2
signing, HTLC custom records, and close/recovery remain separate issues.
