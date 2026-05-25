# Boundaries

This model covers cooperative close, force-close recovery, sweep success,
sweep failure, proof export, and refusal on stale state. It does not model
Bitcoin consensus, actual script paths, fee bumping, watchtower behavior,
penalty transactions, or second-level HTLC script details.

Counterexamples should become Rust tests for close allocation, recovery
refusal, proof export, stale monitor detection, or sweep-result reporting.
