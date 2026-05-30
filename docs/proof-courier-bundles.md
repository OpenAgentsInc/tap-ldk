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
`proof_courier`. Wallet import/export and CLI commands are the next step.
