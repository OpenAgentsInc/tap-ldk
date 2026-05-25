# Boundaries

This model covers commitment-number monotonicity, asset balance conservation,
previous-state revocation, nonce uniqueness, asset-domain signing, and durable
latest state. It does not model real MuSig2, Bitcoin signatures, Taproot
script execution, fee updates, HTLCs, or close/recovery.

Counterexamples should become Rust tests for commitment transitions, signing
domain separation, nonce reuse, balance conservation, or monitor persistence.
