# LDK Asset-Channel Extension Boundary

Date: 2026-05-25

This note fixes the first implementation boundary for native Taproot Asset
channels in `tap-ldk`. LND/`tapd` are compatibility references and Track B
counterparties, not wallet sidecars. Any `rust-lightning` fork required for the
demo must live under `OpenAgentsInc` and be wired explicitly from this repo.

## Boundary Summary

| Surface | Lightning Labs analogue | First home | Fork? | Boundary rule |
| --- | --- | --- | --- | --- |
| Feature negotiation | channel feature/channel type records | `OpenAgentsInc/rust-lightning` fork | Required | Asset messages and funding are illegal until both peers negotiate support. |
| Channel type | custom channel type | `OpenAgentsInc/rust-lightning` fork | Required | BTC-only channels remain BTC-only and cannot be upgraded implicitly. |
| Custom peer messages | `tapchannelmsg`, `rfqmsg` | `tap-ldk-core` | No | Native codecs and routing live in `tap-ldk`; fork only exposes delivery hooks. |
| Funding proof collector | `TxAssetInputProof`, `TxAssetOutputProof` | `tap-ldk-core` | No | Proof fragments are reassembled and verified before funding advances. |
| Funding controller | `AuxFundingController` | `OpenAgentsInc/rust-lightning` fork | Required | Funding approval needs asset ID, proof root, funding output, and allocation checks. |
| Commitment blob | `tapchannelmsg` commitment blob | `OpenAgentsInc/rust-lightning` fork | Required | Asset blob is versioned with the Lightning commitment number and monitor update. |
| Asset signer | `AuxLeafSigner` | `tap-ldk-core` | No | Asset virtual transaction signing and nonce state stay separate from BTC signing. |
| HTLC modifier | custom records / aux HTLC view | `OpenAgentsInc/rust-lightning` fork | Required | Asset metadata can only be attached with an accepted RFQ quote. |
| Final-hop validator | `AuxInvoiceManager` validation | `OpenAgentsInc/rust-lightning` fork | Required | Wrong, stale, or malformed asset metadata fails before settlement. |
| RFQ manager | `rfq.Manager` | `tap-ldk-core` | No | Quotes bind asset ID, asset amount, BTC amount, peer, expiry, invoice context, and replay domain. |
| Invoice binder | `AuxInvoiceManager` invoice behavior | `tap-ldk` LDK-node adapter | No | BOLT 11 stays unchanged; RFQ and route metadata select asset semantics. |
| Close handler | `AuxCloser` | `OpenAgentsInc/rust-lightning` fork | Required | Cooperative close returns the latest mutually valid asset allocation. |
| On-chain resolver/sweeper | `AuxSweeper` | `OpenAgentsInc/rust-lightning` fork | Required | Force-close support cannot claim recovery without proof ownership material. |
| Monitor persistence | channel monitor aux data | `OpenAgentsInc/rust-lightning` fork | Required | Asset-channel state is durable before the corresponding Lightning commitment is safe. |
| Lightning Labs blob codec | funding/commitment/HTLC fixtures | Track B interop harness | No | Blob mismatches are failing compatibility gaps, not partial success. |

The same boundary is encoded in
`crates/tap-ldk-core/src/asset_channel_boundary.rs` so later work has a typed
ledger to test against.

## Experimental Feature And Channel Type

The first native negotiation surface is encoded in
`crates/tap-ldk-core/src/asset_channel_negotiation.rs`.

- Required feature bit: `54032`
- Optional feature bit: `54033`
- Protocol version: `1`
- First channel type: `SingleAsset { asset_id, protocol_version }`

The feature numbers are local experimental values for the proof of concept.
They must be replaced or mapped when the BLIP process assigns stable feature or
message values. Until negotiation succeeds, asset-channel funding messages,
proof fragments, RFQ-bound asset HTLC metadata, and asset close/recovery
messages must be rejected.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-negotiation-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
```

## Native Peer Message Layer

`crates/tap-ldk-core/src/asset_peer_message.rs` defines the first native TLV
message layer for proof exchange, funding metadata, RFQ shells, and asset HTLC
blob transport. The Taproot Asset channel message type base follows the
Lightning Labs offset pattern:

- Taproot Assets base: `32768 + 20116`
- Channel message offset: base `+ 256`
- Funding proof and funding acceptance messages: offset `+ 0..3`
- Native RFQ shells: offset `+ 64..66`
- Native asset HTLC blob shell: offset `+ 96`

The funding proof path is separate from `open_channel`. Proof bytes are split
into chunks with a SHA-256 digest and reassembled before funding can advance.
Message decoding through `decode_negotiated_message` requires a negotiated
asset channel; BTC-only channels reject asset messages.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-peer-message-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
```

## What Stays In `tap-ldk`

- Taproot Asset proof parsing, proof import/export, wallet state, and local
  proof courier behavior.
- Native asset peer-message codecs, including funding proof fragments and RFQ
  request/accept/reject messages.
- RFQ quote store, fixed regtest oracle, expiry/replay policy, and SCID alias
  allocation, documented in `docs/rfq-quote-store.md`.
- Native RFQ invoice binding, documented in `docs/rfq-invoice-binding.md`,
  with BOLT 11 kept opaque and unchanged.
- Native asset-channel funding state and persistence, documented in
  `docs/asset-channel-funding.md`.
- Native asset commitment state transitions and bounded signing contexts,
  documented in `docs/asset-commitment-state.md`.
- Native asset HTLC custom records and final-hop validation, documented in
  `docs/asset-htlc-custom-records.md`.
- Native asset payment send/receive smoke, documented in
  `docs/native-asset-payment.md`.
- Asset virtual transaction construction, asset-level signing, and nonce
  management.
- Lightning Labs fixture and blob decoding for Track B compatibility tests.
- CLI/demo harness, balance checks, and public runbook.

## What Requires The OpenAgentsInc `rust-lightning` Fork

The fork is required wherever the asset-channel decision must happen inside
normal Lightning channel state, monitor state, HTLC state, or close/recovery
state:

- experimental feature bits and channel type acceptance/rejection;
- funding controller hook that can block funding before channel state advances;
- channel monitor aux blob storage coupled to commitment durability;
- commitment update hook for asset balances and asset signatures;
- HTLC custom-record injection and final-hop validation;
- close, force-close, second-level HTLC, sweep, and proof ownership hooks.

Issue #25 is responsible for creating and wiring any required
`OpenAgentsInc/rust-lightning` fork.

## Migration Plan

1. Keep the fork branch narrow and feature-gated behind experimental asset
   channel types.
2. Preserve all upstream BTC-only tests and add focused tests for every forked
   hook.
3. Track the upstream rust-lightning base commit in `docs/protocol-references.md`
   whenever the dependency is moved.
4. Keep `tap-ldk` as the owner of asset semantics; the fork should expose
   bounded hooks and persistence surfaces, not wallet policy.
5. Treat upstream conflicts in channel monitor, HTLC, or close logic as
   protocol-review events, not routine merge churn.

## Design Review Checklist

- ROADMAP.md: matches the first-demo single-asset scope and keeps Path A and
  Path B required.
- INVARIANTS.md: preserves BTC-only channel isolation, asset proof durability,
  RFQ binding, close/recovery ownership, and the no-sidecar runtime rule.
- docs/lightning-labs-interop-matrix.md: maps the required Lightning Labs
  funding, commitment, HTLC, RFQ, invoice, close, and balance surfaces.
- docs/blip-0029-implementation-note.md: keeps proof transport separate from
  `open_channel`, preserves BOLT 11 format, and treats force-close as a
  stronger-demo gate until implemented.
