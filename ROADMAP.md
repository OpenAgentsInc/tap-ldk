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
- `projects/repos/polar/`: local regtest orchestration reference and optional
  manual demo harness for Docker-backed Bitcoin, Lightning, Taproot Assets, and
  Lightning Terminal nodes.
- `docs/lightning-labs-interop-matrix.md`: Track B compatibility matrix for
  the independent Lightning Labs counterparty path.
- Any required forks of upstream dependencies, including `rust-lightning`,
  should be created in the `OpenAgentsInc` GitHub organization and referenced
  from `tap-ldk`; do not turn `projects/` reference clones into owned forks.

## Source Material

- `INVARIANTS.md`
- `stablecoins-may25-transcript.md`
- `blip-0029-taproot-assets-pr-29.md`
- `tap-ldk-proof-of-concept-analysis.md`

## Non-Goals

- No LDK wallet plus `tapd` sidecar demo.
- No LND process in the wallet runtime.
- No claim of production stablecoin issuance, redemption, compliance, or
  reserves.
- No broad discovery marketplace as a prerequisite for the first demo.
- No multi-asset-per-channel or multi-asset-per-HTLC demo in the first public
  cut unless explicitly reopened after the single-asset path works.
- No dual-funding asset-channel demo in the first public cut.
- No change to BOLT 11 invoice format for the demo path; asset semantics live
  in RFQ, route, and custom-record metadata.

## What Must Be Real

- Native Rust parsing and validation for the Taproot Assets structures used by
  the demo.
- Native LDK or rust-lightning channel integration for asset-channel state.
- RFQ or quote binding that determines the BTC HTLC amount for an asset
  payment.
- Asset-specific HTLC custom records.
- Asset-channel feature negotiation and channel type handling.
- Separate Taproot Asset proof messages for channel funding, so large proof
  data does not bloat `open_channel`.
- Asset-level witness, virtual transaction, MuSig2 signature, and nonce
  handling for the Taproot Assets layer.
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

## BLIP-0029 Scope Notes

The BLIP frames Taproot Asset channels as a variant of simple taproot channels:
asset balances are an overlay on normal initiator/responder balances, and the
Taproot Assets commitment appears as an additional tapscript sibling in the
relevant outputs. The demo should follow that shape rather than invent a
parallel payment protocol.

First public demo scope:

- one asset ID per asset channel;
- multiple funding proofs are allowed only to merge multiple inputs of the same
  asset ID into one channel asset UTXO;
- the asset proof sent during funding is the anchor proof needed for the final
  resting place, while full history may come from a local universe/proof
  service;
- asset proof transport remains separate from `open_channel` to respect
  Lightning message-size limits and minimize base message changes;
- HTLC and revocation scripts are lifted to the Taproot Assets layer for the
  single-asset case;
- BOLT 11 invoices remain BTC/msat invoices, with Taproot Asset settlement
  selected through RFQ and route metadata;
- BOLT 12 compatibility is a design note, not a first-demo blocker.

Follow-on scope:

- multiple asset IDs in one channel output;
- one outgoing HTLC using multiple asset IDs;
- multi-part payments across a set of USD-backed channels;
- dual-funded asset-channel opening;
- variable exchange-rate precision, scaling exponent, or characteristic logic
  for non-stablecoin Taproot Assets.

## Regtest Tooling

Polar is now synced under `projects/repos/polar` as a concrete reference for
the local regtest environment. It can create Docker-backed regtest networks,
expose RPC connection details, mine blocks, fund nodes, open channels, manage
logs, export/import networks, and run supported Lightning Labs stacks including
Bitcoin Core, LND, `tapd`, and `litd`.

Use Polar for:

- a fast manual/operator demo network;
- the Lightning Labs interop counterparty in Track B;
- Docker image, port, volume, log, mining, and node-lifecycle patterns;
- optional MCP-driven smoke tests for network setup, mining, Lightning
  payments, and Taproot Asset operations.

Do not use Polar as:

- a substitute for the native Rust Taproot Assets implementation;
- a `tapd` sidecar for the `tap-ldk` wallet runtime;
- the only automated test harness.

The project still needs a headless Rust/CI regtest harness. Polar can inform
that harness or wrap a human-facing demo, but the public proof must show the
native `tap-ldk` wallet interoperating with independent Lightning Labs nodes,
not delegating wallet behavior to those nodes.

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
4. Prefer Polar for the manual Track B network if it can provide the needed
   LND/`tapd` or `litd` topology; otherwise reproduce its Docker patterns in
   the headless harness.
5. Sync or import the demo asset proof data on both sides.
6. Open or connect through an asset channel using the shared protocol surface.
7. Negotiate an RFQ/quote where required.
8. Send an asset invoice payment between `tap-ldk` and the Lightning Labs
   counterparty.
9. Show both sides agree on the resulting asset balance and payment state.

## Implementation Issue Sequence

These are the implementation issues to open or execute in order. Each issue
should stay small enough to review independently, but the sequence is intended
to carry both demos to completion.

| Seq | Issue | Exit condition | Demo |
| --- | --- | --- | --- |
| 1 | Scaffold Rust workspace and CLI shell | `cargo test`, formatting, linting, and a no-op `tap-ldk` CLI run in CI | Both |
| 2 | Pin protocol references and fixture sources | Local paths, upstream commits, and fixture provenance are recorded for TAP BIP, `tapd`, `lnd`, and `rust-lightning` | Both |
| 3 | Import TAP BIP and Taproot Assets fixture vectors | Fixture tests exist for TLV, MS-SMT, address, proof, and virtual PSBT encoding, even if implementation initially fails | Both |
| 4 | Build a headless Bitcoin regtest harness | Tests can start, mine, fund, and stop Bitcoin Core without Polar or a desktop app | Both |
| 5 | Run and document the Polar smoke topology | A Polar network proves the usable LND/`tapd`/`litd` versions, ports, credentials, mining flow, and asset-channel flow for manual interop | B |
| 6 | Implement strict Taproot Assets TLV primitives | Native Rust encode/decode passes fixture tests and rejects malformed or non-canonical data | Both |
| 7 | Implement asset identity and commitment primitives | Genesis, asset ID, group key, script key, split commitments, and MS-SMT roots verify against fixtures | Both |
| 8 | Implement proof file parsing and verification | The CLI can load, verify, export, and reject invalid Taproot Asset proofs | Both |
| 9 | Implement address and virtual PSBT support | The CLI can create/decode Taproot Asset addresses and construct local asset transfer PSBT data | Both |
| 10 | Implement local asset wallet storage | Proofs, balances, spendable asset UTXOs, and wallet metadata persist across restart | Both |
| 11 | Implement regtest issuance and local transfer commands | The CLI can issue `OPENUSD`, verify the proof, send it on-chain, and show balances without Lightning | Both |
| 12 | Bring up a baseline LDK wallet node | Two `tap-ldk` nodes can peer, sync regtest, open a normal BTC channel, and make a normal BTC payment | A |
| 13 | Define the rust-lightning asset-channel extension boundary | The design maps each required LND aux hook to an LDK or forked `rust-lightning` surface with feature flags | Both |
| 14 | Create required OpenAgentsInc forks | Any needed `rust-lightning` or dependency forks exist under `OpenAgentsInc` and are wired explicitly from `tap-ldk` | Both |
| 15 | Add asset-channel feature negotiation | Peers can advertise, require, accept, and reject the experimental Taproot Assets channel feature | Both |
| 16 | Add Taproot Asset peer messages | Separate proof messages, channel funding metadata, RFQ request/accept/reject, and asset HTLC messages round-trip between native peers without bloating `open_channel` | Both |
| 17 | Implement RFQ quote store and fixed-rate oracle | Quotes bind asset ID, asset amount, BTC amount, peer, absolute expiry, invoice context, SCID alias, and replay protection | Both |
| 18 | Implement asset-channel funding for native peers | Two native wallets can open a single-asset channel, accept multiple same-asset input proofs, derive the final TAP asset root hash+sum, and persist initial asset balances | A |
| 19 | Implement asset commitment state transitions | Commitment updates move asset balances, handle asset-level MuSig2 nonces/signatures per asset ID, reject malformed HTLC blobs, and preserve revocation semantics | Both |
| 20 | Implement asset HTLC custom records and final-hop validation | Asset amount, asset ID, quote binding, and invoice metadata are encoded, decoded, and enforced | Both |
| 21 | Implement native asset payment send/receive | Wallet A can pay Wallet B `OPENUSD` through the asset channel and both balances update | A |
| 22 | Implement restart recovery for native channels | Restart after funding, quote acceptance, HTLC add, commitment sign, and settlement recovers the same asset state | A |
| 23 | Implement close and proof export for native channels | Cooperative close returns the expected asset allocation and exports the owner proof; force-close is either implemented or explicitly a stretch gate | A |
| 24 | Script the native-to-native demo | One command starts regtest, starts two wallets, issues `OPENUSD`, opens an asset channel, pays, restarts, and prints balances | A |
| 25 | Extract Lightning Labs interop protocol matrix | `tapd`/`litd` flows for issuance, proof sync, channel funding, RFQ, invoices, payments, close, and balance checks are mapped to native code surfaces | B |
| 26 | Build the Lightning Labs counterparty harness | The headless harness or Polar-backed manual harness can start Bitcoin Core plus LND/`tapd` or `litd` with stable connection material | B |
| 27 | Decode Lightning Labs blob fixtures | Funding, HTLC, and commitment fixtures from `tapchannelmsg/testdata` decode into native read-only field maps and reject malformed data | B |
| 28 | Implement proof import/export compatibility with `tapd` | `tap-ldk` and the Lightning Labs node can share or verify the same demo asset proof data | B |
| 29 | Implement asset-channel funding interop | `tap-ldk` can open or attach to the compatible asset-channel setup used by the Lightning Labs counterparty | B |
| 30 | Implement RFQ and invoice compatibility | `tap-ldk` can create, parse, accept, or pay the quote-bound invoice format used by the Lightning Labs stack | B |
| 31 | Implement `tap-ldk` to Lightning Labs payment | `tap-ldk` pays an asset invoice to the LND/`tapd` or `litd` counterparty and both sides agree on payment and balance state | B |
| 32 | Implement Lightning Labs to `tap-ldk` payment | The Lightning Labs counterparty pays a `tap-ldk` asset invoice, or the gap is documented as the only remaining demo limitation | B |
| 33 | Add interop balance, proof, and restart checks | After each interop payment, both sides report expected balances and `tap-ldk` survives restart with the same state | B |
| 34 | Automate the full demo harness | CI or a local smoke command can run Track A fully and run Track B as far as external container dependencies allow | Both |
| 35 | Write the public demo runbook | The README or demo doc explains exact commands, mocked pieces, expected output, and compatibility limitations | Both |

## Milestone 0: Repo And Harness Setup

Deliverables:

- Initialize `tap-ldk/` as the implementation repo.
- Add a minimal Rust workspace with a CLI demo binary.
- Add a regtest harness that can launch Bitcoin Core.
- Evaluate Polar as the manual regtest/demo harness and decide which pieces
  must be replicated in the headless Rust/CI harness.
- Add fixtures pointing to local reference repos and pinned upstream commits.
- Add CI for formatting, unit tests, and fixture tests.

Exit criteria:

- `cargo test` runs in `tap-ldk/`.
- The demo harness can start and stop a local regtest backend.
- The repo documents that LND/tapd are compatibility references, not runtime
  dependencies.
- The repo documents whether Polar is used directly for manual demos, only as
  a reference, or both.

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
  - feature bit and channel type negotiation;
  - Taproot Asset proof messages separate from `open_channel`;
  - asset channel funding;
  - merged same-asset inputs into one channel asset UTXO;
  - Taproot Asset root hash+sum handling;
  - TAP virtual transaction signing and per-asset-ID nonce handling;
  - asset invoice creation;
  - direct asset payment;
  - RFQ quote flow;
  - RFQ custom message type allocation;
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
- Add or fork the message/channel-type plumbing needed for a new Taproot Asset
  feature bit and channel type.
- Define how asset-level MuSig2 nonces and partial signatures are carried
  without reusing BTC-level nonces.
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
- Absolute quote expiry, using a timestamp-style representation unless the
  BLIP settles on a different self-contained encoding.
- Invoice binding so a quote and invoice expire coherently and are treated as
  a 1:1 pair for the first demo.
- Taproot Assets custom message type allocation for quote request, quote
  accept, and quote reject.
- Optional reject reason/error payload if it can be added without blocking
  interop.
- Stablecoin fixed-rate representation using a scaled exchange rate; variable
  precision/exponent handling stays follow-on.
- Basic multi-peer quote query support.

Exit criteria:

- Two native wallets can negotiate a quote over the peer-message path.
- Expired quotes are rejected.
- Quotes bind to the asset ID, amount, rate, peer, and invoice context.
- The wallet can select a quote and produce route metadata for payment.

Current implementation note:

- The local RFQ quote store, fixed `OPENUSD` regtest oracle, expiry/replay
  checks, SCID alias allocation, and CLI inspection commands are implemented in
  `tap-ldk-core::rfq_quote_store`.
- Native RFQ peer request/accept/reject messages are wired to quote storage,
  and `tap-ldk-core::rfq_invoice` binds accepted quotes to opaque BOLT 11
  invoice text without changing the invoice format. The next step is consuming
  this binding in asset HTLC custom records and payment state.

## Milestone 5: Asset Channel Funding

Deliverables:

- Asset-channel feature negotiation.
- Funding flow with separate asset proof exchange.
- Support for multiple proof messages to avoid Lightning message-size limits.
- Support for merging multiple inputs of the same asset ID into a single
  channel asset UTXO.
- Anchor-proof handling for the channel funding output, with full proof history
  retrieved from local universe/proof service when needed.
- Funding output construction with a Taproot Assets commitment sibling.
- Final `tap_asset_root` hash+sum construction.
- Asset-level `funding_signed` and channel-ready nonce handling, including
  `next_local_nonce` per distinct asset ID.
- Channel-level asset blob persistence.
- Confirmation handling that validates anchor proofs against the funding
  transaction.

Exit criteria:

- Two native wallets can open a single-asset Taproot Asset channel on regtest.
- The channel state records the initial asset balances.
- The funding flow can reject invalid proofs, wrong asset IDs, or missing
  anchor data.
- The flow has a compatibility test plan against LND/tapd.

Current implementation note:

- `tap-ldk-core::asset_channel_funding` implements bounded native funding for
  one asset ID per channel, same-asset multi-input merge, funding root
  derivation, spent-proof replay protection, initial balance persistence, and a
  persisted monitor blob at commitment number `0`. The next milestone wires
  commitment updates and signing context on top of this funded state.

## Milestone 6: Commitments And HTLC State

Deliverables:

- Per-commitment asset balances.
- Incoming and outgoing asset HTLC blobs.
- `ApplyHtlcView` equivalent for rust-lightning commitment updates.
- Auxiliary leaves for local and remote commitment outputs.
- Auxiliary leaves for second-level HTLC outputs.
- Asset-level signatures or witnesses where the TAP layer requires them.
- HTLC and revocation script semantics lifted onto the Taproot Assets layer for
  the single-asset channel case.
- A scoped answer for how multiple HTLCs in one single-asset channel map into
  Taproot Assets leaves and second-level outputs.
- Revocation handling that preserves breach semantics at the asset layer.

Exit criteria:

- A commitment update can move asset balance from sender to receiver.
- Asset state is persisted before any commitment state can be considered safe.
- Wrong asset amount, wrong asset ID, stale quote, or malformed HTLC blob fails.
- Restart tests recover the same asset-channel state.

Current implementation note:

- `tap-ldk-core::asset_commitment` implements bounded commitment-numbered
  balance transitions, previous-state revocation, asset nonce reuse checks,
  deterministic asset virtual transaction/witness/signature contexts, BTC-vs-
  asset signing-domain separation, and restart validation through a persisted
  commitment monitor blob.
- `tap-ldk-core::asset_htlc` implements asset HTLC custom-record codecs,
  final-hop validation against quote-bound invoices, quote-derived BTC msat
  enforcement, BTC-only pass-through behavior, and bounded add/settle/fail
  smoke coverage. Real MuSig2/Taproot Assets witness integration and Lightning
  HTLC dispatch remain follow-on surfaces.
- `tap-ldk-core::asset_payment` wires the bounded native payment path across
  RFQ, quote-bound invoice, asset HTLC records, final-hop validation,
  commitment update, settled HTLC state, payment state, restart round-trip, and
  wrong-quote/wrong-invoice/wrong-metadata failures.
- `tap-ldk-core::asset_recovery` adds bounded restart checkpoints for funding,
  quote acceptance, HTLC add, commitment sign, settlement, and close
  preparation, including stale-checkpoint refusal.
- `tap-ldk-core::asset_close` adds bounded cooperative close and final owner
  proof export from the latest asset commitment view. Force-close and sweep
  recovery remain explicitly deferred.

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

Current implementation note:

- `tap-ldk-cli asset-payment-smoke` performs the Path A bounded native
  Alice-to-Bob payment with pre/post balances, payment state, HTLC state,
  restart confirmation, and negative-path failure reasons. Real rust-lightning
  HTLC dispatch and routing remain follow-on work.
- `tap-ldk-cli asset-recovery-smoke` verifies restart recovery across funding,
  RFQ, HTLC, commitment, settlement, and close-prep checkpoints. Close-prep is
  only a durable marker until the close/proof-export issue lands.
- `tap-ldk-cli asset-close-smoke` closes the bounded native channel after the
  demo payment, exports final owner proofs, imports them into wallets,
  round-trips close state across restart, and rejects obsolete proof material.
- `scripts/path-a-native-demo.sh` is the one-command Path A harness. It creates
  local wallet artifacts, issues/couriers proof material, runs funding,
  payment, recovery, and close smokes, captures logs, and prints final
  balances and artifact paths.

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
  - Polar-managed or Polar-inspired Bitcoin/LND/`tapd` or `litd` topology;
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

Current implementation note:

- `tap-ldk-core::lightning_labs_blob` decodes the imported Lightning Labs
  funding, HTLC, and commitment hexdump fixtures into read-only native field
  maps with raw digests, asset output summaries, RFQ id data, aux leaf
  summaries, and fail-closed malformed/truncated/non-canonical tests. Applying
  those maps to live funding, RFQ, payment, and balance state remains the next
  Track B work.
- `tap-ldk-core::lightning_labs_funding` reconciles the Lightning Labs funding
  and commitment fixtures into a restart-safe Track B funding interop state:
  asset ID, funded amount, local balance, and remote balance match. The state
  intentionally records the remaining live LND/`tapd` funding outpoint and
  full proof-chain mapping as a documented gap.
- `tap-ldk-core::tapd_proof` decodes imported Lightning Labs `TAPF` proof-file
  fixtures, validates chained checksums and `TAPP` TLV transport, wraps single
  proofs for tapd file tooling, and stores raw tapd proof-file bytes in wallet
  state for restart-safe export. Full Taproot Asset proof ancestry validation
  remains later Track B work before live funding can depend on these proofs.
- `tap-ldk-core::lightning_labs_rfq` implements bounded Lightning Labs RFQ
  request, accept, and reject TLV payload compatibility for message types
  `52884..52886`, derives SCID aliases from RFQ IDs the same way
  `rfqmsg.ID.Scid()` does, and binds decoded requests to native quote-bound
  invoice state. Live daemon RFQ exchange, accept-signature verification, and
  payment execution remain the next Track B work.
- `tap-ldk-core::lightning_labs_payment` builds and persists the bounded Track
  B outgoing and incoming payment artifacts: fixture-backed funding state,
  Lightning Labs RFQ payloads, quote-bound invoice, asset HTLC/final-hop
  metadata, expected balance deltas, replay and malformed/wrong metadata
  rejection, and restart-safe documented gap state. It does not yet claim live
  LND/`tapd` settlement or observed receiver balances.
- `tap-ldk-core::lightning_labs_interop_checks` produces a consolidated Track
  B check report across funding, TAPF proof fixtures, both payment directions,
  restart round trips, metadata rejection checks, and expected balance deltas.
  Failed comparisons include side, field, expected value, actual value, and
  artifact path. Live daemon balance observation remains a documented gap.

## Public Demo Bar

The first public demo is ready when:

- the `tap-ldk` wallet runs without LND or tapd as a sidecar;
- the wallet can issue or load a demo stablecoin asset;
- two native wallets can open a single-asset channel;
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
- multiple asset IDs in one channel output;
- MPP over multiple USD-backed channels;
- dual-funded asset-channel opening;
- a simple mobile or web presentation layer.

## Open Decisions

- Whether the native asset protocol core lives inside `tap-ldk` first or starts
  as a separate crate from day one.
- Which `OpenAgentsInc` forks are required before upstreaming, and which
  changes can remain in `tap-ldk` extension crates.
- How much of the TAP VM is required for the first demo versus full protocol
  coverage.
- Whether to lock the native design to absolute quote expiry timestamps while
  the BLIP still discusses relative expiry fields.
- Whether quote and invoice expiry must be identical, or only coherent enough
  that neither can outlive the other in a stale-payment path.
- How to handle multiple USD-backed channels for one quote and MPP routing
  after the single-asset direct demo works.
- How to prevent RFQ SCID alias collisions and garbage-collect expired aliases.
- Whether to use the proposed Taproot Assets custom-message type offset
  `32768 + 20116` for request, accept, and reject messages.
- Whether quote reject messages need an optional error field for interop.
- How to represent scaled exchange rates, exponent/precision, and
  characteristic-like metadata for non-stablecoin Taproot Assets.
- How to support multiple asset IDs in one channel output after the first
  single-asset demo.
- How dual funding should alter proof exchange, asset input merging, and
  funding transaction validation.
- How much discovery belongs in node announcements versus an external registry
  or NIP-69-style intent layer.
- Which direction the first LND/tapd interop payment should run:
  `tap-ldk` pays LND/tapd, LND/tapd pays `tap-ldk`, or both.

## Immediate Next Steps

1. Scaffold the `tap-ldk` Rust workspace.
2. Write a BLIP-0029 implementation note covering first-demo scope,
   single-asset constraints, proof messages, RFQ expiry, SCID aliases, and
   per-asset nonce/signature handling.
3. Copy or reference the TAP BIP test vectors from local synced refs.
4. Implement asset TLV parsing and MS-SMT fixtures.
5. Write the rust-lightning aux-hook-equivalent design document in `tap-ldk/`.
6. Build the RFQ custom-message skeleton.
7. Run a Polar smoke network and record the exact LND/`tapd`/`litd` topology
   usable for the Lightning Labs interop demo.
8. Build the headless regtest demo harness.
9. Create the first native asset issuance and proof-verification CLI command.
10. Start the asset-channel funding spike once the core asset proof path passes
   fixture tests.

## Risks

- Scope is large: this is protocol work, not a wallet skin.
- The BLIP and TAP BIP materials are still draft inputs.
- BLIP-0029 has unresolved review questions around proof transport, per-asset
  nonces, quote expiry, SCID aliases, scaling precision, multiple HTLCs, MPP,
  and multi-asset channel outputs; first-demo scope should stay single-asset
  until those edges are explicit.
- Recovery must be designed early; adding it late risks invalid channel-state
  assumptions.
- Polar is useful for manual and MCP-driven regtest orchestration, but relying
  on an Electron/desktop harness alone would leave CI and reproducibility weak.
- An impressive UI without native channel semantics would undermine the demo
  claim.
- Issuer business requirements are outside this technical demo and should stay
  separate from the protocol proof.
