# Invariants

- Recovered balance requires current proof data.
- Exported proof data must correspond to the recovered output.
- Failed sweeps do not produce recovered balances.
- Refused recovery is not reported as recovered.
- The latest durable commitment number is preserved through close/recovery
  transitions.

## Implementation Mapping

- `crates/tap-ldk-core/src/asset_recovery.rs` maps the bounded recovery model
  to funding, RFQ, HTLC, commitment, settlement, and close-preparation
  checkpoints.
- Recovery refuses checkpoints older than the latest durable asset commitment
  state and does not report those refusals as recovered.
