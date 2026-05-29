# Tap-LDK Formal Verification Harness

This directory contains bounded formal models for the protocol surfaces that
are most likely to break the Tap-LDK demo claim: asset-channel funding,
commitment/HTLC state, proof validation, RFQ expiry/replay, close/recovery,
and interop handshakes.

Do not try to prove Bitcoin, Lightning, Taproot Assets, or Rust Lightning as a
whole. Each model should cover one narrow production contract, document its
assumptions and boundaries, run with a repeatable command when the checker is
available, and produce either implementation guidance, a Rust regression test,
or a clear model-boundary exception.

## Layout

```text
formal/
  README.md
  tla/
    asset_conservation/
      AssetConservation.tla
      AssetConservation.cfg
      assumptions.md
      boundaries.md
      invariants.md
    asset_channel/
      AssetChannel.tla
      AssetChannel.cfg
      assumptions.md
      boundaries.md
      invariants.md
      counterexamples/
        README.md
    rfq_lifecycle/
      RfqLifecycle.tla
      RfqLifecycle.cfg
      assumptions.md
      boundaries.md
      invariants.md
    asset_commitment/
      AssetCommitment.tla
      AssetCommitment.cfg
      assumptions.md
      boundaries.md
      invariants.md
    asset_htlc/
      AssetHtlc.tla
      AssetHtlc.cfg
      assumptions.md
      boundaries.md
      invariants.md
    close_recovery/
      CloseRecovery.tla
      CloseRecovery.cfg
      assumptions.md
      boundaries.md
      invariants.md
    interop_handshake/
      InteropHandshake.tla
      InteropHandshake.cfg
      assumptions.md
      boundaries.md
      invariants.md
    proof_validation/
      ProofValidation.tla
      ProofValidation.cfg
      assumptions.md
      boundaries.md
      invariants.md
      counterexamples/
        README.md
scripts/
  formal-check.sh
```

TLA+ specs should live below `formal/tla/<model>/` with a matching `.tla` and
`.cfg` pair. The runner discovers checked-in `formal/tla/*/*.cfg` files.

## Runner

Run:

```bash
./scripts/formal-check.sh
```

The script is safe on developer machines without TLA+ installed:

- it runs from the repository root;
- it prefers a `tlc` executable on `PATH`;
- it otherwise uses `java -cp "$TLA_TOOLS_JAR" tlc2.TLC` when
  `TLA_TOOLS_JAR` is set;
- it exits 0 with a clear skip message if no runner is available;
- it exits nonzero when TLC runs and reports a model-checking failure;
- it never downloads tools and never mutates the developer environment.

## Counterexample Policy

Every meaningful counterexample must be reduced to the smallest useful trace
and recorded under the model's `counterexamples/` directory. A counterexample
is resolved only when it becomes a Rust regression test or is documented as
outside the model boundary with rationale.
