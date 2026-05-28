# Simple-Taproot Force-Close Witness Note

Date: 2026-05-28

This note records the #84 fix for the live `Invalid Taproot control block size`
failure.

## Root Cause

The failing transaction was the holder commitment transaction spending the
simple-taproot channel funding output. That funding output is P2TR and should
be spent through the key path with the final aggregate MuSig2 Schnorr
signature.

The previous fallback path reused the legacy P2WSH funding witness:

1. empty multisig dummy;
2. holder ECDSA signature;
3. counterparty ECDSA signature;
4. funding redeem script.

For a P2TR input, Bitcoin Core treats a multi-element witness as a script-path
spend and reads the last element as the Taproot control block. The legacy
funding redeem script is not a valid Taproot control block, which produced the
live `Invalid Taproot control block size` error.

## Implemented Fix

`OpenAgentsInc/rust-lightning@0a89b49bf1e822353e0e7c482c5630d5dff22c5c`
changes the holder fallback path:

- `HolderCommitmentTransaction` persists the final simple-taproot holder
  aggregate Schnorr signature.
- Initial `funding_signed` and later `commitment_signed` handling derive the
  holder nonce, sign the holder MuSig2 partial, aggregate it with the peer
  partial, and verify the final signature before storing it.
- `HolderFundingOutput` uses the stored signature directly when constructing
  the fallback transaction.
- `add_holder_sig` returns a key-path signed transaction for simple-taproot
  commitments instead of building the legacy 2-of-2 P2WSH witness.

The expected funding-input witness shape is now exactly one 64-byte BIP340
signature. There is no control block on this spend. Script-path control blocks
still belong to later spends of commitment outputs such as to-local, to-remote,
anchors, and HTLCs.

## Verification

- `cargo fmt --check -p lightning`
- `cargo check -p lightning`
- `cargo check -p lightning --features simple_taproot_musig2`
- `cargo test -p lightning --features simple_taproot_musig2 simple_taproot -- --nocapture`
- `cargo test -p lightning --features simple_taproot_musig2 taproot_asset -- --nocapture`
- `cargo test -p lightning --features simple_taproot_musig2 test_simple_taproot_funding_generation_uses_p2tr_and_rejects_wrong_script -- --nocapture`
- `cargo check` in `OpenAgentsInc/ldk-node`
- `cargo test reports_openagents_rust_lightning_fork_revision` in
  `OpenAgentsInc/ldk-node`
- `cargo fmt --check`, `cargo test`, and
  `./scripts/check-openagents-rust-lightning.sh` in `tap-ldk`
- Live harness rerun:
  `target/live-lightning-labs-outgoing-payment-force-close-witness/report.json`

The live rerun records `native_asset_receiver_local_balance_after: 125` and
`native_ldk_invalid_taproot_control_block_logged: false`.
