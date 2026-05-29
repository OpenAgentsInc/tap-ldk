# Boundaries

This model covers cooperative close output state, force-close recovery,
second-level HTLC output state, sweep success, sweep failure, proof export,
and refusal on stale state. It does not model Bitcoin consensus, actual script
paths, fee bumping, watchtower behavior, penalty transactions, or executable
second-level HTLC script details.

Counterexamples should become Rust tests for close allocation, recovery
refusal, proof export, stale monitor detection, proof-history output
selection, or sweep-result reporting.
