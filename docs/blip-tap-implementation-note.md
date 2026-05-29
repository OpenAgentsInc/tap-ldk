# BLIP-TAP Implementation Note

Date: 2026-05-25

This note scopes the first `tap-ldk` demo against BLIP-TAP and its PR review
discussion. BLIP-TAP is still draft material, so this document should be
treated as implementation planning, not as final protocol authority.

## First Demo Scope

The first public demo should stay narrow:

- one Taproot Asset ID per asset channel;
- one asset-channel payment path between two native `tap-ldk` wallets;
- one interop path between native `tap-ldk` and an independent LND/`tapd` or
  `litd` counterparty using the taproot-assets software stack;
- BOLT 11 invoice format unchanged;
- Taproot Asset semantics carried by RFQ, route metadata, and HTLC custom
  records;
- mocked issuer identity, fixed exchange rate, local proof/universe service,
  and CLI/demo UI clearly labeled as mocks.

Do not put multi-asset channel outputs, multi-asset HTLCs, MPP across a set of
USD-backed channels, dual funding, or variable exchange-rate precision into the
first demo unless the single-asset path is already working.

## Channel Model

BLIP-TAP treats Taproot Asset channels as a variant of simple taproot
channels. Asset state is an overlay on normal initiator/responder Lightning
balances, and the Taproot Assets commitment appears as an additional tapscript
sibling in relevant outputs.

Implementation consequences:

- add an explicit asset-channel feature bit and channel type;
- keep normal BTC channels BTC-only unless the asset feature/channel type is
  negotiated;
- keep normal BTC payments unaffected by asset metadata;
- model asset commitments as channel state that must be persisted alongside
  the corresponding rust-lightning channel monitor state.

## Funding Proof Transport

The PR discussion clarifies that Taproot Asset proof data should be sent as
separate messages rather than stuffed into `open_channel`.

First-demo rules:

- send separate asset proof messages during funding;
- allow multiple proof messages for multiple inputs of the same asset ID;
- merge those same-asset inputs into one channel asset UTXO;
- require all funding proofs to resolve to the expected asset ID;
- use the anchor proof for the final resting place in the funding output;
- rely on a local universe/proof service for full history when needed;
- reject funding if proof material is missing, malformed, wrong-asset,
  wrong-anchor, or incomplete.

Follow-on work can revisit multiple asset IDs in one channel output and dual
funding.

## Taproot Asset Commitment And Signing

The roadmap needs explicit coverage for Taproot Assets layer witnesses and
signing, not only Lightning-level signatures.

First-demo requirements:

- construct and verify the final `tap_asset_root` hash+sum for the funding
  output;
- represent the TAP virtual transaction that spends the TAP funding output;
- carry asset-level partial signatures where the BLIP requires them;
- carry a `next_local_nonce` or equivalent per distinct asset ID;
- never reuse BTC-level MuSig2 nonces for asset-level signing sessions;
- lift HTLC and revocation script semantics onto the Taproot Assets layer for
  the single-asset case.

## RFQ And Invoice Binding

The BLIP uses RFQ as last-mile exchange-rate negotiation while preserving the
existing BOLT 11 invoice format.

First-demo rules:

- quote request, accept, and reject are custom peer messages;
- quote accept binds asset ID, asset amount, BTC/msat amount, rate, peer,
  invoice context, and route context;
- a quote is treated as 1:1 with an invoice for the first demo;
- quote and invoice expiry must be coherent so neither can authorize a stale
  payment after the other has expired;
- prefer an absolute expiry timestamp in native code unless BLIP interop
  requires a different self-contained encoding;
- expired or replayed quotes fail closed;
- RFQ SCID aliases must not collide with real channel SCIDs or live quote
  aliases;
- expired aliases must be garbage-collectable;
- BOLT 12 support stays a compatibility note until the BOLT 11 path works.

Open BLIP items to track:

- custom message type allocation, including the proposed `32768 + 20116`
  Taproot Assets offset;
- optional reject reason/error payloads;
- scaled exchange-rate naming and precision;
- whether future non-stablecoin assets need exponent or characteristic-like
  metadata.

## HTLCs, Revocation, And Close

First-demo requirements:

- encode asset ID, asset amount, quote binding, and final-hop validation
  context in asset HTLC metadata;
- reject malformed, stale, wrong-asset, or wrong-amount HTLC blobs;
- preserve revocation semantics at the asset layer;
- define how multiple HTLCs for one asset ID map into Taproot Assets leaves and
  second-level outputs;
- cooperative close returns the latest valid asset allocation;
- force-close and proof export can remain a stronger-demo gate, but the first
  demo must not design itself into a recovery dead end.

## Interop Constraints

Track B must prove `tap-ldk` can interact with a `lnd`/`tapd`/`litd` node as an
external counterparty. It must not use LND, `tapd`, or `litd` as a wallet
sidecar.

Interop success requires both sides to agree on:

- asset ID;
- asset amount;
- proof/import state;
- payment state;
- resulting balance state;
- any documented compatibility gap.

## Formal Verification Hooks

The matching formal models should focus on:

- negotiation and funding proof completeness;
- no asset inflation across same-asset input merge;
- asset commitment and HTLC state conservation;
- RFQ expiry/replay and SCID alias non-collision;
- close/recovery proof ownership;
- interop success versus documented failure state.
