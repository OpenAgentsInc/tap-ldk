# Invariants

- Recovered balance requires current proof data.
- Exported proof data must correspond to the recovered output.
- Failed sweeps do not produce recovered balances.
- Refused recovery is not reported as recovered.
- The latest durable commitment number is preserved through close/recovery
  transitions.
- Second-level HTLC output state requires a closed parent output.
- Successful sweep requires a closed output and current proof state.
- Proof export must reference either the cooperative close output or the final
  sweep output, not an obsolete commitment view.

## Implementation Mapping

- `crates/tap-ldk-core/src/asset_recovery.rs` maps the bounded recovery model
  to funding, RFQ, HTLC, commitment, settlement, and close-preparation
  checkpoints.
- Recovery refuses checkpoints older than the latest durable asset commitment
  state and does not report those refusals as recovered.
- `crates/tap-ldk-core/src/asset_close.rs` maps cooperative close to the latest
  durable asset commitment and refuses proof material from obsolete commitment
  views. Close proof export now carries replayed proof-history metadata for
  the actual local and remote close outputs.
- Force-close, second-level HTLC, and sweep recovery remain bounded recovery
  reports, but each recovered report now carries replayed proof-history
  metadata for the closed or swept output. Failed sweep state is not reported
  as recovered.
