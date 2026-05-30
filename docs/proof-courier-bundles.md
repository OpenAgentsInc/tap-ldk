# Proof Courier Bundles

`tap-ldk` uses a local proof-courier bundle as the handoff boundary between a
wallet balance and any file or fixture that moves that proof to another wallet.
The bundle is intentionally typed and self-checking. It carries the native
Taproot Assets proof TLV bytes, the accepted proof-history identifiers, the
anchor state, the asset fields that must match the proof, and SHA-256 digests
for the bytes being moved.

Schema version `1` is identified by the transport string
`tap-ldk-local-proof-courier-v1`. The schema is local transport policy, not a
network courier protocol and not a universe service. It can include the raw
Lightning Labs TAPF proof file bytes when the proof came from `tapd`, but the
native proof TLV remains the wallet validation object.

Before a bundle can be accepted, `tap-ldk-core` decodes the proof, checks the
proof digest, validates the proof ID, asset ID, amount, script key, genesis
outpoint, anchor outpoint, proof-history identifiers, optional TAPF digest, and
semantic ancestry. Bad schema versions, wrong transport strings, mismatched
fields, malformed proof bytes, and mismatched digests fail closed.

Current status: the core bundle schema and validation are implemented in
`proof_courier`, and the wallet can import and export bundles through
`export_proof_courier_bundle` and `import_proof_courier_bundle`. The CLI exposes
that path through `wallet-export-proof-bundle` and
`wallet-import-proof-bundle`.

Bundle export is gated by the same replayed wallet balance check as raw proof
export. Confirmed, spendable proofs can be exported. Pending, stale, reorged,
obsolete, or proof-history-mismatched proofs cannot be exported as accepted
bundles. Bundle import passes through semantic proof validation, TAPF validation
when TAPF bytes are present, deterministic proof-history metadata checks, and
the wallet storage validator before state is saved.

Negative coverage is part of the normal locked Rust test suite. It covers
unsupported schema versions, wrong transport strings, malformed proof hex,
wrong proof digest, wrong proof ID, wrong asset, wrong amount, wrong owner,
wrong genesis, wrong anchor, wrong TAPF digest, missing TAPF bytes or digest,
stale/reorged anchor handling, and proof-history metadata mismatch.
