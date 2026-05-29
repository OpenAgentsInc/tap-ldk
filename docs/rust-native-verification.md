# Rust-Native Verification

This repo uses TLA+ to keep the proof-engine policy small, but the Rust code
also needs executable checks for parser, arithmetic, serialization, and replay
behavior. The Rust-native verification layer added for #105 has three parts.

The fast property tests live in
`crates/tap-ldk-core/tests/proof_replay_properties.rs`. They run with normal
Cargo and cover proof-history amount conservation, split inflation rejection,
anchor-policy acceptance, wallet restart and reorg-state behavior, and
RFQ/invoice binding rejection for mismatched asset amounts. These correspond
to the `ProofValidation.tla` invariants for no accepted inflation, coherent
accepted fields, accepted balances requiring policy-approved anchors, and bad
proof or wrong payment context never becoming accepted wallet state.

The fuzz targets live under `fuzz/fuzz_targets/`. They are intentionally
isolated from the default workspace so normal tests stay fast. The current
targets exercise TLV stream decoding, Lightning Labs `TAPF` proof-file decode,
virtual PSBT summary validation, Taproot commitment leaf parsing, and imported
Lightning Labs funding/HTLC/commitment blobs. These targets map to the formal
model boundary where malformed proof-file transport, malformed TapCommitment
data, and parser-level invalid records must fail closed before state advances.

The Kani harnesses live behind `cfg(kani)` in
`crates/tap-ldk-core/src/kani_verification.rs`. They cover pure bounded
helpers: `AssetAmount` checked add/sub behavior, strict and pending anchor
policy, and proof-transition input-state rules. They correspond to the formal
model's no-overflow conservation and accepted-state policy invariants.

Run the default Rust-native verification wrapper with:

```bash
./scripts/rust-verification-check.sh
```

That command always runs the property tests. It runs one-iteration fuzz smoke
targets when `cargo-fuzz` is installed and skips them explicitly otherwise. It
runs Kani when `cargo kani` is available and skips it explicitly otherwise.

Useful direct commands:

```bash
cargo test -p tap-ldk-core --test proof_replay_properties
cargo fuzz run tlv_decode -- -runs=1
cargo fuzz run tapd_proof_file -- -runs=1
cargo fuzz run virtual_psbt_summary -- -runs=1
cargo fuzz run taproot_commitment_leaf -- -runs=1
cargo fuzz run lightning_labs_blobs -- -runs=1
cargo kani -p tap-ldk-core
```

`Miri` is not wired yet because the current proof-engine code does not use
unsafe Rust. `loom` is not wired yet because the proof-engine state is not
shared through concurrent mutation. Add those only when the implementation
introduces a real unsafe/FFI or concurrency boundary that needs them.
