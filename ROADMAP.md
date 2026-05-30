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
`lnd`/`tapd`/`litd` daemons.

## Status

Last updated: 2026-05-29

- Official BOLT Simple Taproot status: the pinned OpenAgentsInc
  `rust-lightning` fork implements the Bitcoin channel base from
  `bolt-simple-taproot.md`. That covers final negotiation, nonce/signature
  TLVs, MuSig2 signing, P2TR funding and commitments, close/RBF close,
  reconnect and BTC splice nonce maps, HTLC and second-level outputs,
  unilateral spend metadata, restart reconstruction, and BOLT vector replay.
  Taproot Assets proof history and asset-channel hardening are separate from
  that BOLT base and remain the experimental production-hardening track.
- Path A has the bounded native `tap-ldk` to `tap-ldk` demo path.
- Path B has fixture/RFQ/payment checks, `tapd` proof binding, local peer
  smokes, an integrated `litd` harness, a fork-backed `ldk-node` runtime pinned
  to OpenAgentsInc `rust-lightning`, and a consolidated Lightning Labs vector
  report that includes funding, HTLC, RFQ, TAPF proof, close/recovery, and
  simple-taproot asset-channel lifecycle checks. The live harness now settles
  the `litd` to native direction: `litd` issues an asset, completes
  live asset-channel funding, sends asset keysend, reports `SUCCEEDED`, native
  LDK claims the HTLC, and fork-backed `ldk-node` records the receiver asset
  payment plus local asset balance `125`. The current `rust-lightning` pin moves
  claimed full-amount asset HTLCs into the receiver balance output and adds a
  live zero-HTLC post-claim regression fixture. The current pin also persists
  the aggregate simple-taproot holder commitment signature and makes the
  force-close funding-input fallback a one-element key-path Schnorr witness
  instead of a legacy 2-of-2 script witness. The latest live rerun no longer
  logs the post-claim partial-signature failure, invalid-commitment failure, or
  invalid Taproot control-block failure. The current pin also makes
  simple-taproot and Taproot Asset opens private by construction, rejects
  missing required simple-taproot open/accept nonces immediately, uses the
  Lightning Labs staging scalar nonce for single-funding RAA/reestablish
  interop while keeping type-22 maps for final or multi-funding paths, adds a
  BTC-only
  simple-taproot conformance gate for open, payment, reconnect/reestablish,
  functional cooperative close, force-close, P2TR funding witnesses, and legacy
  P2WSH isolation, adds a cooperative-close gate that asserts the final close
  transaction's Taproot key-path witness and Taproot Asset allocation restart
  behavior, and now covers BOLT simple-taproot splice nonce maps for current,
  pending splice, and RBF funding txids with fail-closed checks for malformed
  or reused nonce state. It also covers cooperative-close RBF nonce rotation
  for both opener-as-closer and accepter-as-closer, retains signed close
  transactions until confirmation, persists close state across reload, and
  fails closed on missing or reused close nonce/signature state. It fixes the
  BOLT simple-taproot audit's legacy signature-field zeroing/rejection gap for
  funding and commitment messages. The current pin also implements explicit
  final `option_simple_taproot` negotiation behind a separate config flag with
  `option_channel_type`/`option_simple_close` dependency checks, private-channel
  behavior, and type-22 RAA/reestablish nonce-map coverage. The fork now also
  replays the BOLT commitment, trimming, HTLC resolution, deterministic HTLC
  signature, unilateral spend, and restart metadata vector surfaces needed for
  the BTC simple-taproot base claim.
  The live Lightning Labs cooperative-close path
  is exposed through the `litd` harness, but native post-close proof and
  balance observation is still recorded as a documented live boundary rather
  than success. The
  detailed audits in
  `docs/path-b-live-settlement-holistic-audit.md` and
  `docs/path-b-live-settlement-system-audit-2026-05-28.md`, plus
  `docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`, remain the
  file-level map for the #81 regression gate, #57, and BOLT conformance work.
  Broader BOLT simple-taproot production conformance is now closed by #94 and
  #95 and audited in
  `docs/bolt-simple-taproot-production-compliance-audit-2026-05-28.md`.
- `tap-ldk` is pinned to the OpenAgentsInc `rust-lightning` fork at
  `3db3229733b724f45e7a356d923715213cb4f269` and the OpenAgentsInc
  `ldk-node` fork at `1e439b10c94a6e42442d245f95945a906dd6221e`. BOLT
  simple-taproot issues #62 through #70 are implemented: negotiation, TLVs,
  MuSig2 primitives, P2TR
  funding, P2TR commitment outputs/control-block data, and commitment
  update/reestablish nonce state, cooperative-close nonce/signature handling,
  HTLC outputs, second-level HTLC signing helpers, exact BOLT vector replay,
  BOLT HTLC resolution transaction replay, unilateral spend-path checks, and
  restart metadata reconstruction for those surfaces. This pin also keeps
  Lightning Labs' zero-CSV
  behavior for Taproot Asset allocation/script-key derivation while using the
  negotiated channel CSV delay for the actual Bitcoin commitment to-local aux
  output, includes exact previous-output-bound Taproot Asset second-level HTLC
  aux leaves for outgoing signatures, and moves claimed full-amount asset HTLCs
  to the correct post-claim balance output.
- #72 is implemented in `tap-ldk-core::mssmt`: native MS-SMT root calculation,
  inclusion/exclusion proofs, compressed proof encoding, overflow rejection,
  and Lightning Labs fixture replay now replace the old root hash-list
  placeholder where the bounded helper needs a hash+sum commitment.
- #73 is implemented in `tap-ldk-core::taproot_commitment`: asset commitment
  keys, inner `AssetCommitment`s, outer `TapCommitment`s, tap leaf script
  parsing, output-root binding, and asset-channel funding roots now consume
  TapCommitment data instead of bounded root placeholders.
- #74 is implemented in `tap-ldk-core::tap_vm`: native virtual transitions now
  validate issuance, transfer/split, channel funding, and commitment-update
  conservation and witness rules, including generated TAP BIP valid/error
  vectors. Funding and commitment updates derive virtual IDs and witness
  digests only after this validation.
- #75 is implemented across the OpenAgentsInc `rust-lightning` fork and
  `tap-ldk-core::simple_taproot_asset_channel`: the fork exposes
  `TaprootAssetChannelState`, and the new smoke drives negotiation, separate
  proof exchange, funding, monitor aux persistence, HTLC settlement,
  cooperative close, proof-ownership recovery, restart/reestablish roundtrip,
  BTC-only isolation, and live `commitment_signed` asset-signature blob decoding
  through that state.
- No first-demo GitHub issues remain open. Issues #81, #57, #58, #59, #60,
  #61, #71, and #19 are complete and remain the live bidirectional Lightning
  Labs settlement, receiver-restart, observed-balance reporting, semantic
  proof-validation, first-demo BOLT simple-taproot, first-demo Taproot
  Assets-over-LDK, and Path B interop regression gates. The BOLT simple-taproot
  tracker #82 is complete for the first-demo scope; #90 records the historical
  first-demo splice boundary, #92 adds production BTC-level splice nonce-map
  coverage, and #71 is closed with the same first-demo scope qualifier.
  The
  post-success zero-HTLC commitment partial-signature mismatch is fixed and
  tracked as #83; the force-close funding-input key-path witness fallback is
  fixed and tracked as #84; the private-only simple-taproot channel rule is
  fixed and tracked as #85; immediate open/accept nonce validation is fixed and
  tracked as #86; RAA/reestablish nonce-field selection is fixed and tracked
  as #87; the BTC-only simple-taproot conformance gate is fixed and
  tracked as #88; native/fixture cooperative-close coverage and the live
  `litd` close command are tracked as #89; the historical first-demo splice
  boundary is tracked as #90; final feature-bit negotiation is tracked as #91;
  BTC-level splice nonce-map support is tracked as #92; cooperative-close RBF
  nonce rotation is tracked as #93; full vector/unilateral-spend coverage is
  tracked as #94; and production compliance closure is tracked as #95.
- Production BOLT simple-taproot work for the BTC base is complete in the
  pinned fork line. Asset-channel splice/RBF, grouped assets, and production
  proof-history/reorg hardening remain outside that BOLT base claim.
- Proof-engine hardening is complete for the first production-hardening
  sequence after the first demo. Issues #97
  through #106 add typed proof-history replay, the proof-validation formal
  model, negative proof vectors, wallet/funding/commitment/close/recovery
  replay gates, a bounded anchor-state policy for confirmed, pending, stale,
  and reorged anchors, Rust-native property/fuzz/Kani harnesses, a local
  proof-engine check wrapper, and GitHub Actions workflow coverage. Live
  chain watcher integration, production proof-courier policy, grouped assets,
  and full STXO/split/change proof history remain future production work.
- The next production epic should focus on proof courier/export policy:
  accepted proof bytes, optional Lightning Labs `TAPF` bytes, proof-history
  metadata, anchor state, and digests need to move together as a validated
  bundle instead of loose local files. The core bundle schema, wallet
  import/export helpers, and CLI commands are now in place; remaining work in
  this epic is negative-vector expansion, verification wiring, and final docs.
- The first-demo closure order is complete. Keep #81, #57, #58, #59, #60,
  #61, #71, and #19 green as regressions. The closure plan is
  `docs/remaining-issue-closure-plan.md`.

## Implementation Home

- `tap-ldk/`: code, repo-local docs, fixtures, and demo harness.
- `stablecoins/`: source notes, transcript, PR capture, and planning docs.
- `projects/lightninglabs/` and `projects/ldk/`: upstream references only.
- `projects/repos/polar/`: local regtest orchestration reference and optional
  manual demo harness for Docker-backed Bitcoin, Lightning, Taproot Assets, and
  Lightning Terminal nodes.
- `docs/lightning-labs-interop-matrix.md`: Track B compatibility matrix for
  the independent `lnd`/`tapd`/`litd` counterparty path.
- `docs/path-b-live-settlement-system-audit-2026-05-28.md`: detailed #81 audit
  and implementation sequence for the live settlement blocker now kept as
  regression context for #57 and later work.
- `docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`: current
  audit against the upstream BOLT simple-taproot draft, including known spec
  gaps that matter before broader production simple-taproot claims.
- `docs/bolt-simple-taproot-spec-compliance-issues.md`: focused GitHub issue
  plan for BOLT simple-taproot gaps that were split out of #81.
- Any required forks of upstream dependencies, including `rust-lightning` and
  `ldk-node`,
  should be created in the `OpenAgentsInc` GitHub organization and referenced
  from `tap-ldk`; do not turn `projects/` reference clones into owned forks.

## Source Material

- `INVARIANTS.md`
- `stablecoins-may25-transcript.md`
- `blip-tap-pr-29.md`
- `tap-ldk-proof-of-concept-analysis.md`
- `docs/bolt-simple-taproot-ldk-analysis.md`
- `docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`
- `docs/openagents-ldk-node-fork.md`
- BOLT simple taproot channels draft:
  https://github.com/lightning/bolts/blob/master/bolt-simple-taproot.md

## Non-Goals

- No LDK wallet plus `tapd` sidecar demo.
- No LND process in the wallet runtime.
- No claim of production stablecoin issuance, redemption, compliance, or
  reserves.
- No broad discovery marketplace as a prerequisite for the first demo.
- No multi-asset-per-channel or multi-asset-per-HTLC demo in the first public
  cut unless explicitly reopened after the single-asset path works.
- No dual-funding asset-channel demo in the first public cut.
- No splice/RBF asset-channel demo in the first public cut.
- No change to BOLT 11 invoice format for the demo path; asset semantics live
  in RFQ, route, and custom-record metadata.

## What Must Be Real

- BOLT simple taproot channel support in the OpenAgentsInc
  `rust-lightning` fork before the project claims full Taproot Asset channel
  support.
- Simple taproot feature/channel-type negotiation, wire TLVs, MuSig2
  nonce/signature state, P2TR funding, taproot commitment outputs, HTLC
  scripts, RBF cooperative close, reestablish, monitor persistence, and
  on-chain recovery.
- Full Taproot Assets protocol primitives above the simple taproot base:
  MS-SMT, split commitments, `AssetCommitment`, `TapCommitment`, virtual
  transactions, TAP VM validation, proof ancestry, anchor binding, and
  `tapd`-compatible proof import/export.
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
- A native `tap-ldk` to `lnd`/`tapd` payment path.

## What Can Be Mocked For The First Demo

- Issuer identity and stablecoin branding.
- Price oracle, using a fixed regtest rate.
- Discovery, using manual pubkeys, static peers, or a tiny local registry.
- Universe/proof courier, using a local file or local service.
- UI, using CLI or a simple local web view.

## BLIP-TAP Scope Notes

The BLIP frames Taproot Asset channels as a variant of simple taproot channels:
asset balances are an overlay on normal initiator/responder balances, and the
Taproot Assets commitment appears as an additional tapscript sibling in the
relevant outputs. The demo should follow that shape rather than invent a
parallel payment protocol.

That makes BOLT simple taproot a protocol dependency, not a background note.
The `rust-lightning` fork needs a BTC-only simple taproot channel foundation
first: feature and channel-type negotiation, simple-taproot TLVs, MuSig2
signer/nonce state, P2TR funding, taproot commitments, HTLC scripts,
second-level transactions, RBF cooperative close, reestablish, and vector
coverage. Taproot Asset channels then add the asset commitment sibling and
asset-state rules to that base. An asset channel negotiated without the simple
taproot base must fail closed.

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
- concurrent splicing is not exercised; the channel keeps one funding outpoint
  from open through payment, restart, close, and force-close;
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
logs, export/import networks, and run supported `lnd`/`tapd`/`litd` stacks including
Bitcoin Core, LND, `tapd`, and `litd`.

Use Polar for:

- a fast manual/operator demo network;
- the `lnd`/`tapd`/`litd` interop counterparty in Track B;
- Docker image, port, volume, log, mining, and node-lifecycle patterns;
- optional MCP-driven smoke tests for network setup, mining, Lightning
  payments, and Taproot Asset operations.

Do not use Polar as:

- a substitute for the native Rust Taproot Assets implementation;
- a `tapd` sidecar for the `tap-ldk` wallet runtime;
- the only automated test harness.

The project still needs a headless Rust/CI regtest harness. Polar can inform
that harness or wrap a human-facing demo, but the public proof must show the
native `tap-ldk` wallet interoperating with independent `lnd`/`tapd`/`litd` nodes,
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

Track B: `tap-ldk` to `tapd`.

1. Start Bitcoin regtest.
2. Start one native `tap-ldk` wallet.
3. Start one `litd` node as an independent counterparty. litd is
   the practical target because it runs LND and taproot-assets together with
   the aux funding controller enabled.
4. Prefer Polar for the manual Track B network if it can provide the needed
   LND/`tapd` or `litd` topology; otherwise reproduce its Docker patterns in
   the headless harness.
5. Sync or import the demo asset proof data on both sides.
6. Open or connect through an asset channel using the shared protocol surface.
7. Negotiate an RFQ/quote where required.
8. Send an asset invoice payment between `tap-ldk` and the `litd` or `tapd`
   counterparty node.
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
| 25 | Extract `lnd`/`tapd`/`litd` software interop protocol matrix | `tapd`/`litd` flows for issuance, proof sync, channel funding, RFQ, invoices, payments, close, and balance checks are mapped to native code surfaces | B |
| 26 | Build the `lnd`/`tapd`/`litd` counterparty harness | The headless harness or Polar-backed manual harness can start Bitcoin Core plus LND/`tapd` or `litd` with stable connection material | B |
| 27 | Decode Lightning Labs blob fixtures | Funding, HTLC, and commitment fixtures from `tapchannelmsg/testdata` decode into native read-only field maps and reject malformed data | B |
| 28 | Implement proof import/export compatibility with `tapd` | `tap-ldk` and the `lnd`/`tapd`/`litd` node can share or verify the same demo asset proof data | B |
| 29 | Implement asset-channel funding interop | `tap-ldk` can open or attach to the compatible asset-channel setup used by the `lnd`/`tapd`/`litd` counterparty | B |
| 30 | Implement RFQ and invoice compatibility | `tap-ldk` can create, parse, accept, or pay the quote-bound invoice format used by the `lnd`/`tapd`/`litd` stack | B |
| 31 | Implement `tap-ldk` to `litd` payment | `tap-ldk` pays an asset invoice to the LND/`tapd` or `litd` counterparty and both sides agree on payment and balance state | B |
| 32 | Implement `litd` to `tap-ldk` payment | The `lnd`/`tapd`/`litd` counterparty pays a `tap-ldk` asset invoice, or the gap is documented as the only remaining demo limitation | B |
| 33 | Add interop balance, proof, and restart checks | After each interop payment, both sides report expected balances and `tap-ldk` survives restart with the same state | B |
| 34 | Automate the full demo harness | CI or a local smoke command can run Track A fully and run Track B as far as external container dependencies allow | Both |
| 35 | Write the public demo runbook | The README or demo doc explains exact commands, mocked pieces, expected output, and compatibility limitations | Both |

## Open And Future Issue Backlog

The historical implementation sequence above is preserved as the completed
demo-building track. The issue list below records the completed first-demo
closure path and the remaining future hardening boundaries.

| Order | Issue | Work | Current state | Exit condition |
| --- | --- | --- | --- | --- |
| Done | #77 | Fork `ldk-node` for the live runtime | `OpenAgentsInc/ldk-node` exists and is documented as the owned live node implementation home. | Closed. |
| Done | #78 | Patch `ldk-node` to use the OpenAgentsInc `rust-lightning` fork | Implemented in `OpenAgentsInc/ldk-node` at `4b7d8de974a8b08ee8bfee94450dc5c332fe596c`; `tap-ldk` consumes the fork line and reports the OpenAgentsInc `rust-lightning` revision from `ldk_node::provenance`. | Closed. |
| Done | #79 | Expose simple-taproot and Taproot Asset channel config in `ldk-node` | Implemented in `OpenAgentsInc/ldk-node` at `0faa999235050a17b198e6bbfa63c2f19aac4cc6`; BTC-only defaults remain unchanged, Taproot Asset negotiation fails closed without simple taproot, and `tap-ldk` live preflight reports both opt-in flags. | Closed. |
| Done | #80 | Wire Taproot Asset messages and APIs through `ldk-node` | Implemented in `OpenAgentsInc/ldk-node` at `da05c714be061706806bc8757ee74b4709d5a8ef`, with live-feature negotiation fixes, the current rust-lightning HTLC aux-leaf derivation pin, and proof-derived channel-template binding carried through `0964b3d0cce5753a0ff42166ea4686702faf93b4`; `tap-ldk` pins the latest revision and the live preflight reaches typed asset custom-message, asset-channel open, asset APIs, Lightning Labs aux Init feature bits, and remote taproot feature reporting. The fork now advertises Lightning Labs no-op HTLC aux support and does not advertise STXO until native STXO commitment leaves are implemented. | Closed. |
| Done | #85 | Enforce private-only simple-taproot channels | Implemented in `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and carried through `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`: outbound simple-taproot and Taproot Asset opens clear `announce_channel`, inbound public simple-taproot/Taproot Asset opens fail closed, and legacy public BTC channel behavior remains unchanged. | Closed. |
| Done | #86 | Fail missing simple-taproot open/accept nonces immediately | Implemented in `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and carried through `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`: simple-taproot and Taproot Asset `open_channel`/`accept_channel` now fail before state advances when type-4 `next_local_nonce` is missing, while legacy channels can still omit the TLV. | Closed. |
| Done | #87 | Select correct RAA/reestablish nonce fields | Implemented in `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and carried through `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`: Lightning Labs staging/overlay channels use the legacy scalar nonce for single-funding RAA/reestablish interop; final simple-taproot and multi-funding paths use type-22 nonce maps; scalar fallback fails closed when more than one funding txid is active. | Closed. |
| Done | #88 | BTC-only simple-taproot conformance gate | Implemented in `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and carried through `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`: the gate opens a BTC-only simple-taproot channel, verifies P2TR funding, pays in both directions across reconnect/reestablish, covers functional cooperative close, force-closes with a one-element key-path funding witness, and proves legacy P2WSH channels remain unaffected. Run `./scripts/check-btc-simple-taproot-conformance.sh`. | Closed. |
| Done | #89 | Live-prove simple-taproot cooperative close | Implemented in `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and carried through `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`: native cooperative close now asserts the final P2TR funding spend has a single 64-byte key-path witness, Taproot Asset close checks preserve the latest allocation across restart, and `tap-ldk` exposes `./scripts/check-simple-taproot-cooperative-close.sh` plus `lightning-labs-litd-counterparty.sh close-asset-channel`. Live post-close proof/balance observation remains a documented Path B boundary, not a claimed success. | Closed. |
| Done | #90 | Cover simple-taproot splice nonce maps or gate splicing out of the first demo | Closed the original first-demo ambiguity by making the early demo boundary machine-readable. #92 supersedes the BTC-level nonce-map gap with bounded splice nonce-map coverage; asset-channel splice/RBF remains separate hardening. | Closed. |
| Done | #91 | Enable final `option_simple_taproot` production negotiation | Implemented in `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and carried through `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`: final bits 80/81 are behind `negotiate_final_simple_taproot_channels`, require `option_channel_type` and `option_simple_close`, remain separate from staging/overlay interop, keep simple-taproot opens private, and use type-22 nonce maps for final RAA/reestablish. `tap-ldk-cli simple-taproot-negotiation-report` reports staging, overlay, and final modes. | Closed. |
| Done | #92 | Implement full simple-taproot splice nonce-map compliance | Implemented in `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and carried through `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`: final/multi-funding RAA and reestablish maps cover current, pending splice, and RBF funding txids; missing, empty, duplicate, unknown, scalar-with-multiple-funding, and nonce-reuse cases fail closed; serialized channel state preserves the pending splice and counterparty nonce map. `./scripts/check-simple-taproot-splice-policy.sh` now verifies support instead of an exclusion. | Closed. |
| Done | #93 | Complete simple-taproot cooperative-close RBF nonce rotation | Implemented in `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and carried through `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`: after a signed simple-taproot close transaction, an explicit close-with-feerate request can produce a higher-fee RBF close using the latest peer closee nonce and a fresh closer nonce; signed close txids, received closer nonces, sent `closing_complete` state, and the RBF request flag persist; opener-as-closer and accepter-as-closer are covered; missing shutdown nonce, missing close partial, missing next closee nonce, and reused closer nonce fail closed. | Closed. |
| Done | #94 | Replay full simple-taproot BOLT vectors and unilateral spend paths | Implemented in `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and carried through `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`: exact commitment vectors now cover no-HTLC, five-HTLC, and trimmed-HTLC cases; HTLC resolution transaction hex, witness stacks, and deterministic remote HTLC signatures match the BOLT vectors; to-local, to-remote, anchor, HTLC, and second-level spend paths are consensus-verified; spend metadata reconstructs after commitment serialization. | Closed. |
| Done | #95 | Track production BOLT simple-taproot compliance | All production BOLT simple-taproot child issues #91 through #94 are pinned and documented. The remaining future work is Taproot Assets overlay hardening, not the BTC simple-taproot base. | Closed. |
| Done | #82 | Track BOLT simple-taproot spec compliance gaps | All first-demo child issues #83 through #90 are closed, #91/#92 cover final feature-bit negotiation plus BTC-level splice nonce maps, #93 covers cooperative-close RBF nonce rotation, and #94 covers full vector/unilateral-spend replay. | Closed for first-demo scope; production tracker #95 is now closed. |
| Done | #81 | Use fork-backed `ldk-node` for live `litd` settlement | `target/live-lightning-labs-outgoing-payment-issue81-rerun/report.json` completed with `issue_81_acceptance_met=true`: integrated `litd` funded the asset channel, sent the asset keysend, reported `SUCCEEDED`, native LDK claimed the HTLC, `ldk-node` recorded local receiver balance `125`, and no invalid commitment, partial-signature, control-block, or counterparty force-close logs were observed. | Closed; keep this command green as a Path B regression. |
| Done | #57 | Live `tap-ldk` pays `litd` asset payment | `target/live-lightning-labs-outgoing-payment-issue57-final/report.json` completed with `issue_57_acceptance_met=true`: integrated `litd` funded the asset channel, paid native LDK, native LDK recorded the received asset, native LDK sent the asset back with a canonical Taproot Asset HTLC blob and dust-covering BTC amount, `litd` settled the invoice, and the observed `litd` channel asset balance reflects the returned amount. | Closed; keep this command green as a bidirectional Path B regression. |
| Done | #58 | Live `litd` pays `tap-ldk` asset payment | `target/live-lightning-labs-outgoing-payment-issue58-rerun/report.json` completed with `issue_58_acceptance_met=true`: integrated `litd` paid native LDK, native LDK recorded the settled remote-to-local asset payment, bounded receiver metadata checks stayed fail-closed, and the restart snapshot reloaded the persisted receiver payment/balance checkpoint. | Closed; keep this command green as the `litd`-to-native receive/restart regression. |
| Done | #59 | Replace Path B documented gaps with observed live balance checks | `target/path-b-lightning-labs-demo-issue59/path-b-completion-report.json` completed with `path_b_live_observed_balance_gate_met=true`, `live_daemon_gaps_remaining=false`, fixture-only completion disabled, and expected-only balance completion disabled. | Closed; keep the Path B wrapper completion report green as the observed-balance regression. |
| Done | #60 | Full semantic Taproot Assets proof ancestry validation | `tap-ldk-core::proof` now requires `semantic-ancestry`, strict regtest outpoints, normal demo asset type, derived root hash/sum, expected asset/owner/amount checks, stale-anchor rejection, and Lightning Labs `TAPF` asset-leaf validation before wallet state advances. Funding, HTLC metadata, cooperative close, and recovery handoff use the same committed proof-root boundary. | Closed; keep `cargo test --locked` and the live tapd proof binding path green. Production full-history virtual transaction, STXO, grouped-asset, and reorg-watcher proof checks remain future production hardening work. |
| Done | #61 | BOLT simple taproot channels in `rust-lightning` epic | Fork issues #62 through #70 and #75 are implemented and pinned, with vector/lifecycle smoke coverage. #88 proves the BTC-only open/pay/reestablish/cooperative-close/force-close base with legacy-channel isolation, #89 strengthens cooperative-close close/restart evidence, #90 records the historical first-demo splice boundary, #91 adds final feature-bit negotiation, #92 adds BTC-level splice nonce-map support, #93 adds cooperative-close RBF nonce rotation, #94 adds full vector/unilateral-spend replay, and #95 closes the production BOLT tracker. `check-btc-simple-taproot-conformance`, `check-simple-taproot-cooperative-close`, and `check-simple-taproot-splice-policy` pass against the current fork line. | Closed. |
| Done | #62 | Simple-taproot feature bits and channel type | Implemented in `OpenAgentsInc/rust-lightning` at `90054d8fc512eb9506955f27806b496e33d2b346`. | Closed. |
| Done | #63 | Simple-taproot wire TLVs and message validation | Implemented in `OpenAgentsInc/rust-lightning` at `c237a0ae1189c0c59e27bdc8e8b99fd2bb018bcb`. | Closed. |
| Done | #64 | MuSig2 signer and nonce state | Implemented in `OpenAgentsInc/rust-lightning` at `6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941`; key aggregation, counter/JIT nonce generation, partial-signature verification, final Schnorr aggregation, persisted nonce-use rejection, and signer-facing `InMemorySigner` helpers are covered. | Closed. |
| Done | #65 | Simple-taproot P2TR funding flow | Implemented in `OpenAgentsInc/rust-lightning` at `1602ac9e1e7454d39612e126c24a098e276d605a`; BIP86 P2TR funding script generation, BOLT funding vector coverage, P2TR output scripts, wrong-script rejection, and P2TR monitor registration are covered. | Closed. |
| Done | #66 | Simple-taproot commitment outputs and control blocks | Implemented in `OpenAgentsInc/rust-lightning` at `b0b952531329a31265f8de28752ee5334d9d9d4f`; P2TR to-local, to-remote, and anchor scripts match BOLT vectors, with tap tweaks, tapscript roots, and control blocks reconstructable. | Closed. |
| Done | #67 | Simple-taproot commitment update and reestablish state | Implemented in `OpenAgentsInc/rust-lightning` at `1176e837e5aacac7d1a3237c2bb00910989dbd93`; channel-ready, commitment-signed, revoke-and-ack, and channel-reestablish nonce/signature state is persisted and fail-closed. | Closed. |
| Done | #68 | Simple-taproot RBF cooperative close | Implemented in `OpenAgentsInc/rust-lightning` at `26346a56af75eadf60763eb1e32a740656d4e384`; close nonce/signature state is persisted and malformed close state fails closed. | Closed. |
| Done | #69 | Simple-taproot HTLC scripts and second-level transactions | Implemented in `OpenAgentsInc/rust-lightning` at `6af69ad385b864d7666edebbbbb668dab485bdde`; offered/accepted HTLC P2TR outputs, second-level outputs, and BIP342 signing helpers are covered. | Closed. |
| Done | #70 | BOLT simple-taproot vector replay | Implemented in `OpenAgentsInc/rust-lightning` at `983c4385ff66105ab70d766d34f49c1bd547a81a`; vector replay covers implemented TLVs, funding, commitments, close, HTLC, second-level, and trimming surfaces. | Closed. |
| Done | #71 | Full Taproot Assets protocol support for LDK epic | `target/path-b-lightning-labs-demo-issue71/path-b-completion-report.json` completed with `path_b_complete=true`, `live_daemon_gaps_remaining=false`, `semantic_proof_ancestry_complete=true`, `issue_57_acceptance_met=true`, `issue_58_acceptance_met=true`, observed native receiver balance `125`, and observed `litd` receiver channel balance `125`. Native MS-SMT, TapCommitment, TAP VM, semantic proof ancestry, fork-backed asset-channel state, monitor persistence, HTLC state, close allocation, recovery, Path A, and Path B are covered for the first-demo scope. | Closed for first-demo scope; production proof-history replay, grouped/multi-asset paths, STXO/split/change proof replay, reorg watchers, production proof courier policy, live force-close/sweep recovery, and concurrent splice/RBF asset-channel candidates remain future hardening. |
| Done | #72 | MS-SMT hash-sum tree | Implemented in `tap-ldk-core::mssmt`; Lightning Labs root/proof fixtures, inclusion/exclusion proofs, compressed proof round trips, conservation, and overflow rejection pass. | Closed. |
| Done | #73 | `AssetCommitment` and `TapCommitment` layers | Implemented in `tap-ldk-core::taproot_commitment`; funding roots consume TapCommitment data, tap leaf fixture parsing passes, and wrong output roots fail closed. | Closed. |
| Done | #74 | Virtual transaction and TAP VM validation | Implemented in `tap-ldk-core::tap_vm`; TAP BIP generated valid/error vectors pass, channel funding and commitment updates consume native virtual transition validation, and invalid witnesses/amounts fail closed. | Closed. |
| Done | #75 | Full Taproot Asset channel state in simple-taproot LDK channels | Implemented in `OpenAgentsInc/rust-lightning` at `99fee582d4061af4b0a030353b0a409ee542e064` and extended through `8a54739ac030ba3e439496eacb7e1c1216e11c6f` for live HTLC blob validation, HTLC blob channel-state persistence, outbound HTLC blob re-emission, HTLC aux-leaf output plumbing, live `commitment_signed` asset-signature blob decoding, taproot-assets commitment aux-leaf script decoding, proof-derived single-asset channel-template persistence, first full-channel HTLC aux-leaf derivation, transcript diagnostics/fixture coverage for the rejected live HTLC signature path, second-level virtual-lock asset-leaf encoding, full counterparty commitment monitor persistence, and exact previous-output-bound second-level HTLC aux leaves; funding, commitments, HTLCs, close, monitor, recovery, restart, and BTC-only isolation pass through the fork lifecycle state. | Closed. |
| Done | #76 | `tapd`/`litd` vectors for simple-taproot asset channels | Implemented in `tap-ldk-core::lightning_labs_interop_checks`; consolidated checks cover funding, HTLC RFQ metadata, RFQ message types, TAPF proof vectors, lifecycle state, close/proof recovery, restart round trips, and observed-balance gates. | Closed. |
| Done | #19 | Path B `lnd`/`tapd`/`litd` interop demo | `target/path-b-lightning-labs-demo-issue71/path-b-completion-report.json` completed with `path_b_complete=true`, `path_b_live_observed_balance_gate_met=true`, `live_daemon_gaps_remaining=false`, `semantic_proof_ancestry_complete=true`, `issue_57_acceptance_met=true`, and `issue_58_acceptance_met=true`. `tap-ldk` pays integrated `litd`, `litd` pays `tap-ldk`, observed balances are recorded on both sides, native receiver state survives restart, and proof import uses semantic ancestry validation. | Closed for first-demo interop scope; future hardening remains separate from the completed demo claim. |

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

- Import or derive BOLT simple taproot vectors for:
  - feature/channel-type negotiation;
  - simple-taproot wire TLVs;
  - MuSig2 key aggregation, nonce exchange, partial signatures, and final
    signatures;
  - P2TR funding outputs;
  - commitment transaction outputs and control blocks;
  - cooperative close nonce rotation;
  - offered/accepted HTLC scripts and second-level transactions.
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
- Real MS-SMT root/proof implementation is in place through #72, including
  inclusion and exclusion proofs, compressed proof fixture replay, checked sums,
  and bounded split-conservation roots.
- `AssetCommitment` and `TapCommitment` construction and verification.
- TAP VM validation for the demo asset transitions.
- Proof file parser and semantic proof ancestry verifier.
- Address encode/decode.
- Virtual PSBT structures for asset sends and channel funding.
- Local asset database for proofs, balances, and spendable asset UTXOs.

Exit criteria:

- Native Rust code passes the imported TAP BIP vectors used by the demo.
- The CLI can issue a demo asset on regtest and verify its proof.
- The CLI can create and decode a Taproot Asset address.
- The CLI can construct and verify a local asset transfer without Lightning.

## Milestone 2A: BOLT Simple Taproot LDK Foundation

This milestone is a prerequisite for the full Taproot Asset channel claim. It
is allowed to complete with BTC-only channels before any Taproot Asset overlay
is enabled.

Deliverables:

- `option_simple_taproot` feature and explicit simple-taproot channel-type
  handling in the OpenAgentsInc `rust-lightning` fork.
- Message TLV codecs and validation for open, accept, funding, channel-ready,
  commitment signing, revocation, reestablish, shutdown, and close messages.
- MuSig2 signer integration, nonce generation, nonce rotation, partial
  signature verification, final signature aggregation, and persistence.
- P2TR funding output construction and validation.
- Taproot commitment outputs for local, remote, anchor, offered HTLC, accepted
  HTLC, HTLC-success, and HTLC-timeout paths.
- Control-block or reconstruction-data persistence in channel monitors.
- RBF cooperative close flow with close nonce rotation.
- BOLT simple taproot vector replay or equivalent fixture tests.
- Test isolation proving existing legacy channel behavior is unaffected.

Exit criteria:

- Two LDK nodes can open a BTC-only simple-taproot channel on regtest.
- The channel can send a normal BTC payment.
- Restart and `channel_reestablish` preserve nonce/signature state.
- Cooperative close and force-close paths are covered.
- Normal non-taproot channel tests still pass.
- Taproot Asset channel code cannot negotiate unless this base is available.

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
  feature bit and channel type on top of simple taproot.
- Define how asset-level MuSig2 nonces and partial signatures are carried
  without reusing BTC-level nonces.
- Define persistence data that must be written through channel monitors.

Exit criteria:

- The design maps each required LND aux hook to an LDK/rust-lightning surface.
- The demo can compile with an experimental asset-channel feature enabled only
  when the simple-taproot base is present.
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

- Asset-channel feature negotiation layered on negotiated BOLT simple taproot
  channels.
- Funding flow with separate asset proof exchange.
- Support for multiple proof messages to avoid Lightning message-size limits.
- Support for merging multiple inputs of the same asset ID into a single
  channel asset UTXO.
- Anchor-proof handling for the channel funding output, with full proof history
  retrieved from local universe/proof service when needed.
- Funding output construction by adding the Taproot Assets commitment sibling
  to the simple-taproot funding/commitment tree.
- Final `TapCommitment` and `AssetCommitment` root construction, not only a
  bounded root hash+sum placeholder.
- Asset-level `funding_signed` and channel-ready nonce handling, including
  `next_local_nonce` per distinct asset ID.
- Channel-level asset blob persistence.
- Confirmation handling that validates anchor proofs against the funding
  transaction.

Exit criteria:

- Two native wallets can open a single-asset Taproot Asset channel on regtest.
- The channel is a simple-taproot LDK channel with an asset overlay, not a
  parallel channel model.
- The channel state records the initial asset balances.
- The funding flow can reject invalid proofs, wrong asset IDs, or missing
  anchor data.
- The flow has a compatibility test plan against LND/tapd.

Current implementation note:

- `tap-ldk-core::asset_channel_funding` implements bounded native funding for
  one asset ID per channel, same-asset multi-input merge, funding root
  derivation, spent-proof replay protection, initial balance persistence, and a
  persisted monitor blob at commitment number `0`. Commitment updates and
  signing context now build on top of this funded state in Milestone 6. This
  remains a scaffold until it is rewired through the simple-taproot funding
  and real Taproot Assets commitment layers.

## Milestone 6: Commitments And HTLC State

Deliverables:

- Per-commitment asset balances.
- Incoming and outgoing asset HTLC blobs.
- `ApplyHtlcView` equivalent for rust-lightning commitment updates.
- Taproot Assets auxiliary leaves attached to the simple-taproot local and
  remote commitment outputs.
- Taproot Assets auxiliary leaves attached to simple-taproot second-level HTLC
  outputs.
- Asset-level signatures or witnesses where the TAP layer requires them.
- HTLC and revocation script semantics lifted onto the Taproot Assets layer for
  the single-asset channel case.
- A scoped answer for how multiple HTLCs in one single-asset channel map into
  Taproot Assets leaves and second-level outputs.
- Revocation handling that preserves breach semantics at the asset layer.

Exit criteria:

- A real simple-taproot commitment update can move asset balance from sender
  to receiver.
- Asset state is persisted before any commitment state can be considered safe.
- Wrong asset amount, wrong asset ID, stale quote, or malformed HTLC blob fails.
- Restart tests recover the same asset-channel state.

Current implementation note:

- `tap-ldk-core::asset_commitment` implements bounded commitment-numbered
  balance transitions, previous-state revocation, asset nonce reuse checks,
  deterministic asset virtual transaction/witness/signature contexts, BTC-vs-
  asset signing-domain separation, and restart validation through a persisted
  commitment monitor blob plus the OpenAgentsInc rust-lightning fork's asset
  monitor aux blob surface. This is not yet a full simple-taproot channel
  state-machine integration.
- `tap-ldk-core::asset_htlc` implements asset HTLC custom-record codecs,
  final-hop validation against quote-bound invoices, quote-derived BTC msat
  enforcement, BTC-only pass-through behavior, OpenAgentsInc rust-lightning
  HTLC metadata/final-hop hook validation, and bounded add/settle/fail smoke
  coverage. Real MuSig2/Taproot Assets witness integration and Lightning HTLC
  dispatch over simple-taproot HTLC outputs remain follow-on surfaces.
- `tap-ldk-core::asset_payment` wires the bounded native payment path across
  RFQ, quote-bound invoice, asset HTLC records, final-hop validation,
  commitment update, settled HTLC state, payment state, restart round-trip, and
  wrong-quote/wrong-invoice/wrong-metadata failures.
- `tap-ldk-core::asset_recovery` adds bounded restart checkpoints for funding,
  quote acceptance, HTLC add, commitment sign, settlement, and close
  preparation, including stale-checkpoint refusal.
- `tap-ldk-core::asset_close` adds bounded cooperative close and final owner
  proof export from the latest asset commitment view, validated through the
  OpenAgentsInc rust-lightning cooperative-close allocation hook. Force-close
  and sweep recovery remain explicitly deferred.

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
  - simple-taproot channel negotiation and transaction shape;
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
  state for restart-safe export. `tap-ldk-core::proof` now adds the #60
  semantic boundary over those bytes: latest asset leaf, asset ID, normal demo
  asset type, amount, script key, genesis outpoint, anchor staleness, and the
  native proof root must agree before wallet or channel state advances.
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
  rejection, and restart-safe documented gap state. The live #57 gate now
  proves bidirectional integrated-`litd` settlement; the bounded fixture report
  remains separate from that live regression.
- `tap-ldk-core::lightning_labs_interop_checks` produces a consolidated Track
  B check report across funding, TAPF proof fixtures, both payment directions,
  restart round trips, metadata rejection checks, and expected balance deltas.
  Failed comparisons include side, field, expected value, actual value, and
  artifact path. The report now includes explicit live observed-balance gates;
  #57 supplies the returned `litd` channel-balance observation, #58 supplies
  the receiver restart snapshot, and #59 supplies the consolidated completion
  gate that prevents fixture-only or expected-only balances from completing
  Path B.
- `scripts/path-b-lightning-labs-demo.sh` captures the current Track B harness
  into `target/path-b-lightning-labs-demo/<timestamp>` and records an explicit
  runtime/counterparty dependency gap when the independent `lnd`/`tapd`/`litd`
  target cannot be started. The live outgoing-payment gate now reaches proof
  binding, native payment-session readiness, integrated `litd` readiness,
  fork-backed `ldk-node` to `litd` peer connection, live asset-channel funding,
  `litd` to native settlement, native to `litd` settlement, and
  returned `litd` channel-balance observation before writing a completed #57
  report.
  asset-channel payment settlement. `scripts/full-demo-smoke.sh` runs Path A
  and Path B into a single ignored artifact tree.
- `scripts/path-a-native-demo.sh` now exports cooperative close proof artifacts
  and `close-recovery-status.json`, which makes restart-after-close,
  obsolete-proof rejection, failed-sweep gating, and deferred force-close
  status machine-visible in the demo artifact directory.

## Public Demo Bar

The first public demo is ready when:

- the `tap-ldk` wallet runs without LND or tapd as a sidecar;
- the wallet can issue or load a demo stablecoin asset;
- two native wallets can open a single-asset channel on the simple-taproot LDK
  base;
- one wallet can pay the other using RFQ-bound asset HTLC metadata;
- one native wallet can interoperate with a `lnd`/`tapd` node for an
  asset invoice payment;
- the receiving wallet shows the asset balance change;
- restart recovery works;
- all mocked pieces are clearly labeled.

The stronger full-LDK support bar also requires:

- BTC-only BOLT simple taproot channels implemented and tested in the
  OpenAgentsInc `rust-lightning` fork;
- full MS-SMT, `AssetCommitment`, `TapCommitment`, TAP VM, virtual
  transaction, and semantic proof ancestry support;
- asset funding, commitment, HTLC, close, monitor, and recovery state wired
  into the real simple-taproot channel state machine.

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

- When to move the public demo from staging/overlay interop to final
  `option_simple_taproot`; the BTC BOLT base is now ready, but the Taproot
  Assets overlay still needs interop and proof-recovery hardening before the
  public demo should default to final mode.
- Whether to upstream BTC-only simple-taproot support independently before
  layering Taproot Assets on top.
- How the Taproot Assets commitment sibling maps into each simple-taproot
  output type: funding, local commitment, remote commitment, offered HTLC,
  accepted HTLC, and second-level HTLC.
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

1. Keep #81, #57, #58, #59, #60, #61, #71, and #19 green as first-demo
   regressions. Open new issues for production hardening rather than reopening
   the completed first-demo closure path.

## Risks

- Scope is large: this is protocol work, not a wallet skin.
- The BLIP and TAP BIP materials are still draft inputs.
- The current asset hooks can become the wrong abstraction if the simple
  taproot state machine is not implemented first.
- BLIP-TAP has unresolved review questions around proof transport, per-asset
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
