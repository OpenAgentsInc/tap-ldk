# Semantic Proof Ancestry Validation

Issue #60 replaces the old shallow proof-envelope acceptance with a shared
semantic proof boundary in `tap-ldk-core::proof`.

## Implemented Boundary

Every native `ProofFile` accepted by wallet import, wallet restart validation,
asset-channel funding, local transfer, and cooperative close must now use:

- `verification_scope = semantic-ancestry`;
- `network = regtest`;
- `asset_type = normal` for the demo stablecoin path;
- strict `<txid>:<vout>` genesis and anchor outpoints;
- nonzero asset ID, nonzero amount, nonzero Taproot Asset root hash;
- amount/root-sum conservation;
- a Taproot Asset root hash derived from the accepted asset leaf;
- expected asset ID, owner script key, amount, genesis, anchor, and stale-anchor
  checks when the call site has that context.

Lightning Labs `TAPF` imports now additionally decode the latest `TAPP` asset
leaf, derive the asset ID from the Taproot Assets genesis fields, and compare
the resulting asset ID, asset type, amount, script key, and genesis outpoint to
the local proof record before wallet state advances. The raw proof file is still
preserved byte-for-byte for export.

Funding, HTLC receipt, close, and recovery share this boundary through the
state they accept: funding proofs call the semantic validator directly; HTLC
receipt validates against the committed proof root and amount carried in the
LDK Taproot Asset HTLC metadata; close validates final owner proofs through the
same validator; recovery validates proof ownership handoff against the latest
committed proof root and commitment number.

## Proof-History Replay Surface

Issue #97 adds the first typed proof-history replay engine in
`tap-ldk-core::proof`. This is not yet wired into every wallet and channel
state-advance boundary; #100 through #103 track that integration. It is the
runtime vocabulary those later gates will use.

The replay engine defines explicit transition records for issuance, split,
transfer, channel funding, commitment update, cooperative close, unilateral
close, second-level HTLC, sweep, and proof export. Each output carries the
asset ID, amount, owner script key, anchor outpoint, Taproot Asset root,
virtual transition ID, prior proof state, and resulting proof state needed to
explain why the output can be treated as accepted, channel-locked, closed, or
swept.

The state names intentionally match the planned
`formal/tla/proof_validation` model vocabulary: accepted, rejected,
unresolved, pending, stale, spent, channel-locked, closed, and swept. The first
regressions cover valid synthetic lifecycle replay plus missing input,
contradictory amount, invalid output state, and mismatched root failures.

Issue #100 wires that replay surface into wallet balance and proof export
authority. A wallet-imported proof now records a deterministic proof-history
record ID, output ID, and transition ID. `WalletState::balances`,
`export_encoded_proof`, and `export_tapd_proof_file` require the stored proof
and spendable UTXO to replay to an accepted balance explanation before they
return user-visible balance or export bytes. The current implementation is
still the bounded first-demo path: each imported proof is represented as a
single accepted issuance-style output. Later channel, close, sweep, and
recovery issues replace that bounded import record with full proof-chain
history at the relevant state boundaries.

Issue #101 extends the same replay gate to asset-channel funding. Funding now
constructs a replay history that accepts each input proof, consumes those
inputs through a channel-funding transition, and requires the resulting output
to be channel-locked with the expected asset ID, amount, funding script key,
funding outpoint, and root. The durable channel record stores deterministic
funding proof-history metadata and validation fails if that metadata no longer
matches the channel.

Issue #102 extends replay authority to asset commitment updates. The commitment
store now treats the funding proof-history output as the initial channel-locked
state, consumes that output for each commitment update, and records a new
channel-locked proof-history output tied to the TapVM virtual transition. Store
validation rebuilds that chain on restart, so a newer commitment number without
matching proof-history metadata or monitor aux state is refused instead of
being treated as recovered.

Issue #103 extends replay authority through close, recovery, and close-proof
export. Cooperative close now consumes the latest channel-locked proof-history
output, creates closed local and remote outputs, and then records proof-export
transitions for the exact wallet-importable close proofs. The recovery matrix
also records replayed proof-history output metadata for unilateral commitment,
second-level HTLC, and final sweep spend kinds, ending in closed or swept
state as appropriate.

Issue #104 adds the first chain-state boundary to proof-history replay. The
replay engine now accepts a typed anchor policy with unknown, pending,
confirmed, stale, and reorged states. The legacy bounded-regtest `replay()`
entry point still assumes confirmed anchors, but wallet balance/export paths
use explicit anchor state. Confirmed anchors are spendable, pending anchors are
represented but not spendable by default, and stale or reorged anchors are
kept as rejected wallet state until a replacement proof path is imported.

## Fail-Closed Cases

Tests and fixtures cover malformed outpoints, unsupported scopes/networks,
wrong asset, wrong owner, wrong amount, wrong asset type, stale anchors, root
sum mismatch, root hash mismatch, broken genesis/anchor ancestry, corrupted
`TAPF` checksums, and `TAPF` asset-leaf mismatches.

## Out Of Scope After #60

The first-demo validator does not yet claim production-complete Taproot Assets
proof verification. Still out of scope:

- full Bitcoin transaction and merkle-proof validation for every anchor;
- full virtual transaction witness execution for every historical proof;
- grouped assets, collectibles, reissuance, and multi-asset proof paths;
- full split/change/STXO inclusion and exclusion proof replay;
- live reorg watcher integration and production proof courier/universe policy.

Those items remain future production-hardening work, not a reason to accept
shallow proof fields in the demo wallet.
