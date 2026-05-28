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
- reorg watcher integration and production proof courier/universe policy.

Those items remain #71 production-hardening work, not a reason to accept shallow
proof fields in the demo wallet.
