# Boundaries

This model covers commitment-number monotonicity, asset balance conservation,
previous-state revocation, nonce uniqueness, asset-domain signing, durable
latest state, and the restart rule that proof replay must match the latest
commitment number. It does not model real MuSig2, Bitcoin signatures, Taproot
script execution, fee updates, HTLCs, or close/recovery.

Counterexamples should become Rust tests for commitment transitions, signing
domain separation, nonce reuse, balance conservation, monitor persistence, or
proof-history metadata mismatch.
