# Tap-LDK Demo Roadmap

Date: 2026-05-25

## Goal

Build a demo showing a standalone Rust Lightning/LDK wallet issuing or
receiving a Taproot Asset stablecoin and transacting it over Lightning-style
asset channels without depending on an LND/tapd runtime sidecar. LND and tapd
are reference implementations and required compatibility test peers, not
sidecars inside the `tap-ldk` wallet runtime.

## Demo Claim

The demo should prove that Taproot Assets can be implemented natively in the
LDK ecosystem: asset issuance/proofs, asset-channel state, RFQ, HTLC metadata,
settlement, and recovery are handled by Rust/LDK code rather than delegated to
Lightning Labs daemons.

## Implementation Home

- `tap-ldk/`: code, repo-local docs, fixtures, and demo harness.
- `stablecoins/`: source notes, transcript, PR capture, and planning docs.
- `projects/lightninglabs/` and `projects/ldk/`: upstream references only.
- Any required forks of upstream dependencies, including `rust-lightning`,
  should be created in the `OpenAgentsInc` GitHub organization and referenced
  from `tap-ldk`; do not turn `projects/` reference clones into owned forks.

## Source Material

- `stablecoins-may25-transcript.md`
- `blip-0029-taproot-assets-pr-29.md`
- `tap-ldk-proof-of-concept-analysis.md`

## Non-Goals

- No LDK wallet plus `tapd` sidecar demo.
- No LND process in the wallet runtime.
- No claim of production stablecoin issuance, redemption, compliance, or
  reserves.
- No broad discovery marketplace as a prerequisite for the first demo.

## What Must Be Real

- Native Rust parsing and validation for the Taproot Assets structures used by
  the demo.
- Native LDK or rust-lightning channel integration for asset-channel state.
- RFQ or quote binding that determines the BTC HTLC amount for an asset
  payment.
- Asset-specific HTLC custom records.
- Channel persistence sufficient to restart the demo wallet without losing the
  asset-channel state.
- A native `tap-ldk` to native `tap-ldk` payment path.
- A native `tap-ldk` to Lightning Labs LND/tapd payment path.

## What Can Be Mocked For The First Demo

- Issuer identity and stablecoin branding.
- Price oracle, using a fixed regtest rate.
- Discovery, using manual pubkeys, static peers, or a tiny local registry.
- Universe/proof courier, using a local file or local service.
- UI, using CLI or a simple local web view.

## Demo Script

Track A: native-to-native.

1. Start Bitcoin regtest.
2. Start two native `tap-ldk` wallets.
3. Issue or load a grouped demo asset such as `OPENUSD`.
4. Sync the required proof data between wallets.
5. Open an asset channel.
6. Request an invoice or quote for a fixed asset amount.
7. Pay the invoice through the asset channel.
8. Show sender and receiver balances before and after settlement.
9. Restart one wallet and show the same channel and asset balance.
10. In the stronger demo, force-close and recover/export the asset proof.

Track B: `tap-ldk` to Lightning Labs TAP-D.

1. Start Bitcoin regtest.
2. Start one native `tap-ldk` wallet.
3. Start one Lightning Labs LND/tapd node as an independent counterparty.
4. Sync or import the demo asset proof data on both sides.
5. Open or connect through an asset channel using the shared protocol surface.
6. Negotiate an RFQ/quote where required.
7. Send an asset invoice payment between `tap-ldk` and LND/tapd.
8. Show both sides agree on the resulting asset balance and payment state.

## Milestone 0: Repo And Harness Setup

Deliverables:

- Initialize `tap-ldk/` as the implementation repo.
- Add a minimal Rust workspace with a CLI demo binary.
- Add a regtest harness that can launch Bitcoin Core.
- Add fixtures pointing to local reference repos and pinned upstream commits.
- Add CI for formatting, unit tests, and fixture tests.

Exit criteria:

- `cargo test` runs in `tap-ldk/`.
- The demo harness can start and stop a local regtest backend.
- The repo documents that LND/tapd are compatibility references, not runtime
  dependencies.

## Milestone 1: Spec And Compatibility Fixtures

Deliverables:

- Import TAP BIP JSON vectors from `Roasbeef/bips@bip-tap`.
- Create fixture tests for:
  - asset TLV encoding;
  - MS-SMT tree operations;
  - TAP VM validation cases;
  - address TLV encoding;
  - proof file encoding;
  - virtual PSBT encoding.
- Extract the important LND/tapd custom-channel flows into test notes:
  - asset channel funding;
  - asset invoice creation;
  - direct asset payment;
  - RFQ quote flow;
  - mixed BTC/asset routing;
  - close and force-close behavior.

Exit criteria:

- Fixture tests fail for missing native code, not for missing fixture data.
- Every protocol surface used in the demo points to a local reference source.

## Milestone 2: Native Taproot Assets Core

Deliverables:

- Asset model:
  - genesis data;
  - asset ID;
  - group key;
  - script key;
  - amount;
  - previous witnesses;
  - split commitments.
- TLV serialization and strict decoding.
- MS-SMT commitment implementation.
- TAP VM validation for the demo asset transitions.
- Proof file parser and verifier.
- Address encode/decode.
- Virtual PSBT structures for asset sends and channel funding.
- Local asset database for proofs, balances, and spendable asset UTXOs.

Exit criteria:

- Native Rust code passes the imported TAP BIP vectors used by the demo.
- The CLI can issue a demo asset on regtest and verify its proof.
- The CLI can create and decode a Taproot Asset address.
- The CLI can construct and verify a local asset transfer without Lightning.

## Milestone 3: rust-lightning Extension Boundary

Deliverables:

- Design an experimental trait boundary modeled on LND aux components:
  - funding controller;
  - commitment leaf store;
  - asset signer;
  - HTLC modifier;
  - traffic shaper;
  - invoice/final-hop validator;
  - close handler;
  - on-chain resolver/sweeper;
  - blob parser and persistence codec;
  - channel negotiator.
- Decide which code lands as a fork, extension crate, or upstreamable patch.
- If a fork is needed, create it under `OpenAgentsInc` first, for example
  `OpenAgentsInc/rust-lightning`, then wire `tap-ldk` to that fork explicitly.
- Add feature flags so normal BTC Lightning behavior stays isolated.
- Define persistence data that must be written through channel monitors.

Exit criteria:

- The design maps each required LND aux hook to an LDK/rust-lightning surface.
- The demo can compile with an experimental asset-channel feature enabled.
- Normal LDK channel tests remain conceptually isolated from asset-channel code.

## Milestone 4: RFQ And Custom Messages

Deliverables:

- Custom message types for quote request, quote accept, and quote reject.
- Quote store with expiry and replay protection.
- Fixed-rate mock oracle for `OPENUSD`.
- SCID alias derivation for quote-bound routes.
- Collision checks so RFQ SCIDs cannot collide with real local channel SCIDs.
- Invoice binding so a quote and invoice expire coherently.
- Basic multi-peer quote query support.

Exit criteria:

- Two native wallets can negotiate a quote over the peer-message path.
- Expired quotes are rejected.
- Quotes bind to the asset ID, amount, rate, peer, and invoice context.
- The wallet can select a quote and produce route metadata for payment.

## Milestone 5: Asset Channel Funding

Deliverables:

- Asset-channel feature negotiation.
- Funding flow with asset proof exchange.
- Support for multiple proof messages to avoid Lightning message-size limits.
- Funding output construction with a Taproot Assets commitment sibling.
- Channel-level asset blob persistence.
- Confirmation handling that validates anchor proofs against the funding
  transaction.

Exit criteria:

- Two native wallets can open a single-asset Taproot Asset channel on regtest.
- The channel state records the initial asset balances.
- The funding flow can reject invalid proofs, wrong asset IDs, or missing
  anchor data.
- The flow has a compatibility test plan against LND/tapd.

## Milestone 6: Commitments And HTLC State

Deliverables:

- Per-commitment asset balances.
- Incoming and outgoing asset HTLC blobs.
- `ApplyHtlcView` equivalent for rust-lightning commitment updates.
- Auxiliary leaves for local and remote commitment outputs.
- Auxiliary leaves for second-level HTLC outputs.
- Asset-level signatures or witnesses where the TAP layer requires them.
- Revocation handling that preserves breach semantics at the asset layer.

Exit criteria:

- A commitment update can move asset balance from sender to receiver.
- Asset state is persisted before any commitment state can be considered safe.
- Wrong asset amount, wrong asset ID, stale quote, or malformed HTLC blob fails.
- Restart tests recover the same asset-channel state.

## Milestone 7: Payment Send, Receive, And Routing

Deliverables:

- Asset invoice representation in the wallet API.
- BOLT 11 compatibility path for demo invoices.
- BOLT 12 compatibility notes, since the BLIP expects support to be readily
  available without invoice-format changes.
- Final-hop validator for asset metadata.
- HTLC custom record encode/decode.
- Amount rewriting from asset amount to BTC HTLC amount using the accepted RFQ.
- Asset-aware channel eligibility and bandwidth checks.
- Direct send and invoice-based payment flows.

Exit criteria:

- Wallet A can pay Wallet B a fixed `OPENUSD` amount.
- Wallet B receives the asset amount and can reject bad metadata.
- The BTC amount seen by the Lightning layer is quote-derived.
- Normal BTC payments are unaffected.

## Milestone 8: Recovery, Close, And Proof Export

Deliverables:

- Cooperative close with asset-aware output construction.
- Force-close tracking.
- Asset-aware second-level HTLC signing and sweep handling.
- Proof export after close or sweep.
- Recovery tests at:
  - after funding;
  - after quote accepted;
  - after HTLC added;
  - after commitment signed;
  - after payment settled;
  - after force-close.

Exit criteria:

- Restart does not lose asset balances or proofs.
- Cooperative close returns the expected asset allocation.
- Force-close can recover the asset proof for the owner.
- The demo can show at least restart recovery; force-close can be a second demo
  gate if schedule requires.

## Milestone 9: Discovery MVP

The transcript identified discovery as missing from the current Taproot Assets
stack. It is not required for the first controlled demo, but it is required for
a credible follow-on.

Deliverables:

- Minimal node-announcement feature bit or TLV proposal.
- Query message for supported assets and liquidity ranges after peer connect.
- Optional local registry for demo UX.
- Optional NIP-69-style intent advertisement for ranges only, not live RFQ order
  books.

Exit criteria:

- A wallet can discover which connected peers claim to support `OPENUSD`.
- The wallet can query supported asset IDs, min/max amounts, and liquidity
  ranges.
- Live RFQ still happens peer-to-peer with short expiry.

## Milestone 10: Interop Demo

Deliverables:

- Native LDK to native LDK happy path.
- Native LDK to LND/tapd compatibility path for:
  - RFQ negotiation;
  - asset-channel funding or compatible pre-funded asset-channel setup;
  - asset invoice payment;
  - balance/proof verification after settlement.
- A written compatibility matrix showing what works and what remains
  divergent.

Exit criteria:

- The demo proves both native-to-native and native-to-Lightning-Labs
  participation.
- Any use of LND/tapd is clearly labeled as a compatibility peer, not a sidecar.
- Gaps are reduced to explicit protocol or implementation issues.

## Public Demo Bar

The first public demo is ready when:

- the `tap-ldk` wallet runs without LND or tapd as a sidecar;
- the wallet can issue or load a demo stablecoin asset;
- two native wallets can open an asset channel;
- one wallet can pay the other using RFQ-bound asset HTLC metadata;
- one native wallet can interoperate with a Lightning Labs LND/tapd node for an
  asset invoice payment;
- the receiving wallet shows the asset balance change;
- restart recovery works;
- all mocked pieces are clearly labeled.

## Stretch Demo Bar

The stronger demo adds:

- BOLT 12 offer/invoice coverage;
- force-close and proof export;
- discovery through a node feature bit or TLV;
- a simple mobile or web presentation layer.

## Open Decisions

- Whether the native asset protocol core lives inside `tap-ldk` first or starts
  as a separate crate from day one.
- Which `OpenAgentsInc` forks are required before upstreaming, and which
  changes can remain in `tap-ldk` extension crates.
- How much of the TAP VM is required for the first demo versus full protocol
  coverage.
- How to represent quote expiry: relative seconds are fragile; an absolute
  timestamp is likely cleaner.
- How to handle multiple USD-backed channels for one quote and MPP routing.
- How to prevent RFQ SCID alias collisions and garbage-collect expired aliases.
- How much discovery belongs in node announcements versus an external registry
  or NIP-69-style intent layer.
- Which direction the first LND/tapd interop payment should run:
  `tap-ldk` pays LND/tapd, LND/tapd pays `tap-ldk`, or both.

## Immediate Next Steps

1. Scaffold the `tap-ldk` Rust workspace.
2. Copy or reference the TAP BIP test vectors from local synced refs.
3. Implement asset TLV parsing and MS-SMT fixtures.
4. Write the rust-lightning aux-hook-equivalent design document in `tap-ldk/`.
5. Build the RFQ custom-message skeleton.
6. Build the regtest demo harness.
7. Create the first native asset issuance and proof-verification CLI command.
8. Start the asset-channel funding spike once the core asset proof path passes
   fixture tests.

## Risks

- Scope is large: this is protocol work, not a wallet skin.
- The BLIP and TAP BIP materials are still draft inputs.
- Recovery must be designed early; adding it late risks invalid channel-state
  assumptions.
- An impressive UI without native channel semantics would undermine the demo
  claim.
- Issuer business requirements are outside this technical demo and should stay
  separate from the protocol proof.
