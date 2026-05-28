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

## Status

Last updated: 2026-05-28

- Path A has the bounded native `tap-ldk` to `tap-ldk` demo path.
- Path B has fixture/RFQ/payment checks, `tapd` proof binding, local peer
  smokes, an integrated `litd` harness, a fork-backed `ldk-node` runtime pinned
  to OpenAgentsInc `rust-lightning`, and a consolidated Lightning Labs vector
  report that includes funding, HTLC, RFQ, TAPF proof, close/recovery, and
  simple-taproot asset-channel lifecycle checks. The live harness now settles
  the Lightning Labs to native direction: `litd` issues an asset, completes
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
  simple-taproot and Taproot Asset opens private by construction and fixes the BOLT
  simple-taproot audit's legacy signature-field zeroing/rejection gap for
  funding and commitment messages, but the fork is still not spec complete
  until the remaining broader conformance gaps are fixed. The
  detailed audits in
  `docs/path-b-live-settlement-holistic-audit.md` and
  `docs/path-b-live-settlement-system-audit-2026-05-28.md`, plus
  `docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`, remain the
  file-level map for the remaining #81 and BOLT conformance work. Broader
  BOLT simple-taproot conformance is split into the issue set described in
  `docs/bolt-simple-taproot-spec-compliance-issues.md`.
- `tap-ldk` is pinned to the OpenAgentsInc `rust-lightning` fork at
  `98e25016540ed98b450a2bf270d8d50c846f1d18` and the OpenAgentsInc
  `ldk-node` fork at `6d44b0bda8305b71544c9996ea23b7ab653b8ce2`. BOLT
  simple-taproot issues #62 through #70 are implemented: negotiation, TLVs,
  MuSig2 primitives, P2TR
  funding, P2TR commitment outputs/control-block data, and commitment
  update/reestablish nonce state, cooperative-close nonce/signature handling,
  HTLC outputs, second-level HTLC signing helpers, and BOLT vector replay
  coverage for those surfaces. This pin also keeps Lightning Labs' zero-CSV
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
- Current open work includes #81, #57, #58, #59, #60, #61, #71, and #19. Issue
  #81 now focuses on keeping the live Lightning Labs to native settlement gate
  green while the remaining BOLT simple-taproot audit items are tracked under
  #82, with focused issues #86 through #90, before #61/#71 can close. The
  post-success zero-HTLC commitment partial-signature mismatch is fixed and
  tracked as #83; the force-close funding-input key-path witness fallback is
  fixed and tracked as #84; the private-only simple-taproot channel rule is
  fixed and tracked as #85. Issue #57 is still the true native
  `tap-ldk` to Lightning Labs payment direction.
- The required closure order is #81 for the live settlement gate, then #57,
  #58, #59, #60, and BOLT simple-taproot spec-compliance tracker #82 before the
  epics #61, #71, and #19 close. The dedicated closure plan is
  `docs/remaining-issue-closure-plan.md`.

## Implementation Home

- `tap-ldk/`: code, repo-local docs, fixtures, and demo harness.
- `stablecoins/`: source notes, transcript, PR capture, and planning docs.
- `projects/lightninglabs/` and `projects/ldk/`: upstream references only.
- `projects/repos/polar/`: local regtest orchestration reference and optional
  manual demo harness for Docker-backed Bitcoin, Lightning, Taproot Assets, and
  Lightning Terminal nodes.
- `docs/lightning-labs-interop-matrix.md`: Track B compatibility matrix for
  the independent Lightning Labs counterparty path.
- `docs/path-b-live-settlement-system-audit-2026-05-28.md`: current detailed
  #81 audit and implementation sequence for the live settlement blocker.
- `docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`: current
  audit against the upstream BOLT simple-taproot draft, including known spec
  gaps that can block #81.
- `docs/bolt-simple-taproot-spec-compliance-issues.md`: focused GitHub issue
  plan for BOLT simple-taproot gaps that should not be folded into #81.
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
- A native `tap-ldk` to Lightning Labs LND/tapd payment path.

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
3. Start one Lightning Labs litd node as an independent counterparty. litd is
   the practical target because it runs LND and taproot-assets together with
   the aux funding controller enabled.
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

## Open And Future Issue Backlog

The historical implementation sequence above is preserved as the completed
demo-building track. The open issue list is now the closure plan for the live
interop and full-protocol claim. Close issues in this order unless new test
evidence changes the dependency graph.

| Order | Issue | Work | Current state | Exit condition |
| --- | --- | --- | --- | --- |
| Done | #77 | Fork `ldk-node` for the live runtime | `OpenAgentsInc/ldk-node` exists and is documented as the owned live node implementation home. | Closed. |
| Done | #78 | Patch `ldk-node` to use the OpenAgentsInc `rust-lightning` fork | Implemented in `OpenAgentsInc/ldk-node` at `4b7d8de974a8b08ee8bfee94450dc5c332fe596c`; `tap-ldk` consumes the fork line and reports the OpenAgentsInc `rust-lightning` revision from `ldk_node::provenance`. | Closed. |
| Done | #79 | Expose simple-taproot and Taproot Asset channel config in `ldk-node` | Implemented in `OpenAgentsInc/ldk-node` at `0faa999235050a17b198e6bbfa63c2f19aac4cc6`; BTC-only defaults remain unchanged, Taproot Asset negotiation fails closed without simple taproot, and `tap-ldk` live preflight reports both opt-in flags. | Closed. |
| Done | #80 | Wire Taproot Asset messages and payment APIs through `ldk-node` | Implemented in `OpenAgentsInc/ldk-node` at `da05c714be061706806bc8757ee74b4709d5a8ef`, with live-feature negotiation fixes, the current rust-lightning HTLC aux-leaf derivation pin, and proof-derived channel-template binding carried through `6d44b0bda8305b71544c9996ea23b7ab653b8ce2`; `tap-ldk` pins the latest revision and the live preflight reaches typed asset custom-message, asset-channel open, asset-payment APIs, Lightning Labs aux Init feature bits, and remote taproot feature reporting. The fork now advertises Lightning Labs no-op HTLC aux support and does not advertise STXO until native STXO commitment leaves are implemented. | Closed. |
| Done | #85 | Enforce private-only simple-taproot channels | Implemented in `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18` and carried through `OpenAgentsInc/ldk-node@6d44b0bda8305b71544c9996ea23b7ab653b8ce2`: outbound simple-taproot and Taproot Asset opens clear `announce_channel`, inbound public simple-taproot/Taproot Asset opens fail closed, and legacy public BTC channel behavior remains unchanged. | Closed. |
| 1 | #81 | Use fork-backed `ldk-node` for live Lightning Labs settlement | The live harness now connects to `litd`, observes both taproot feature sets, issues an asset, completes live asset-channel funding, settles a Lightning Labs to native asset keysend, records native `PaymentClaimed`, persists the receiver asset balance in `ldk-node`, and no longer logs the zero-HTLC post-claim partial-signature failure. | The live Path B scripts settle an asset payment over fork-backed `ldk-node`, verify force-close witnesses, and record post-settlement balances without a broken fallback. |
| 2 | #57 | Live `tap-ldk` pays Lightning Labs asset payment | Harness reaches live `tapd` proof binding, ordered native asset-payment session readiness, standalone current-balance observation, integrated `litd` readiness, and fork-backed `ldk-node` peer connection/API preflight to `litd` with opt-in asset-channel negotiation enabled and remote feature support observed. | `tap-ldk` pays an independent `litd` receiver over the fork-backed live asset-channel path and records observed post-settlement Lightning Labs receiver balance plus updated sender state. |
| 3 | #58 | Live Lightning Labs pays `tap-ldk` asset payment | Receiver-side artifacts exist for buy-direction RFQ, quote-bound receive invoice, final-hop HTLC metadata, expected deltas, restart, and rejection cases. | Independent Lightning Labs node pays `tap-ldk`; `tap-ldk` validates the asset metadata, persists the received balance/proof reference, survives restart, and both sides report expected live balances. |
| 4 | #59 | Replace Path B documented gaps with observed live balance checks | Reports distinguish fixture-backed expected balances from live gates and still keep `live_daemon_gaps_remaining=true`. | Path B cannot report completion without observed post-settlement balances and compatible payment/proof state in both live directions. |
| 5 | #60 | Full semantic Taproot Assets proof ancestry validation | MS-SMT, TapCommitment, TAP VM, TAPF transport validation, and raw proof preservation exist; full proof ancestry does not. | Proof ancestry, anchors, owner transitions, funding, HTLC, close, and recovery proofs validate semantically through one shared boundary. |
| 6 | #61 | BOLT simple taproot channels in `rust-lightning` epic | Fork issues #62 through #70 and #75 are implemented and pinned, with vector/lifecycle smoke coverage. | BTC-only simple-taproot LDK channels open, pay, reestablish, close, force-close, and leave legacy channels unaffected. |
| Done | #62 | Simple-taproot feature bits and channel type | Implemented in `OpenAgentsInc/rust-lightning` at `90054d8fc512eb9506955f27806b496e33d2b346`. | Closed. |
| Done | #63 | Simple-taproot wire TLVs and message validation | Implemented in `OpenAgentsInc/rust-lightning` at `c237a0ae1189c0c59e27bdc8e8b99fd2bb018bcb`. | Closed. |
| Done | #64 | MuSig2 signer and nonce state | Implemented in `OpenAgentsInc/rust-lightning` at `6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941`; key aggregation, counter/JIT nonce generation, partial-signature verification, final Schnorr aggregation, persisted nonce-use rejection, and signer-facing `InMemorySigner` helpers are covered. | Closed. |
| Done | #65 | Simple-taproot P2TR funding flow | Implemented in `OpenAgentsInc/rust-lightning` at `1602ac9e1e7454d39612e126c24a098e276d605a`; BIP86 P2TR funding script generation, BOLT funding vector coverage, P2TR output scripts, wrong-script rejection, and P2TR monitor registration are covered. | Closed. |
| Done | #66 | Simple-taproot commitment outputs and control blocks | Implemented in `OpenAgentsInc/rust-lightning` at `b0b952531329a31265f8de28752ee5334d9d9d4f`; P2TR to-local, to-remote, and anchor scripts match BOLT vectors, with tap tweaks, tapscript roots, and control blocks reconstructable. | Closed. |
| Done | #67 | Simple-taproot commitment update and reestablish state | Implemented in `OpenAgentsInc/rust-lightning` at `1176e837e5aacac7d1a3237c2bb00910989dbd93`; channel-ready, commitment-signed, revoke-and-ack, and channel-reestablish nonce/signature state is persisted and fail-closed. | Closed. |
| Done | #68 | Simple-taproot RBF cooperative close | Implemented in `OpenAgentsInc/rust-lightning` at `26346a56af75eadf60763eb1e32a740656d4e384`; close nonce/signature state is persisted and malformed close state fails closed. | Closed. |
| Done | #69 | Simple-taproot HTLC scripts and second-level transactions | Implemented in `OpenAgentsInc/rust-lightning` at `6af69ad385b864d7666edebbbbb668dab485bdde`; offered/accepted HTLC P2TR outputs, second-level outputs, and BIP342 signing helpers are covered. | Closed. |
| Done | #70 | BOLT simple-taproot vector replay | Implemented in `OpenAgentsInc/rust-lightning` at `983c4385ff66105ab70d766d34f49c1bd547a81a`; vector replay covers implemented TLVs, funding, commitments, close, HTLC, second-level, and trimming surfaces. | Closed. |
| 8 | #71 | Full Taproot Assets protocol support for LDK epic | Native primitives, bounded channel state, fork hooks, and live interop scaffolding exist. | Real Taproot Assets primitives and channel state are layered onto simple-taproot LDK channels, #57 through #60 pass, and normal BTC behavior remains unaffected. |
| Done | #72 | MS-SMT hash-sum tree | Implemented in `tap-ldk-core::mssmt`; Lightning Labs root/proof fixtures, inclusion/exclusion proofs, compressed proof round trips, conservation, and overflow rejection pass. | Closed. |
| Done | #73 | `AssetCommitment` and `TapCommitment` layers | Implemented in `tap-ldk-core::taproot_commitment`; funding roots consume TapCommitment data, tap leaf fixture parsing passes, and wrong output roots fail closed. | Closed. |
| Done | #74 | Virtual transaction and TAP VM validation | Implemented in `tap-ldk-core::tap_vm`; TAP BIP generated valid/error vectors pass, channel funding and commitment updates consume native virtual transition validation, and invalid witnesses/amounts fail closed. | Closed. |
| Done | #75 | Full Taproot Asset channel state in simple-taproot LDK channels | Implemented in `OpenAgentsInc/rust-lightning` at `99fee582d4061af4b0a030353b0a409ee542e064` and extended through `98e25016540ed98b450a2bf270d8d50c846f1d18` for live HTLC blob validation, HTLC blob channel-state persistence, outbound HTLC blob re-emission, HTLC aux-leaf output plumbing, live `commitment_signed` asset-signature blob decoding, Lightning Labs commitment aux-leaf script decoding, proof-derived single-asset channel-template persistence, first full-channel HTLC aux-leaf derivation, transcript diagnostics/fixture coverage for the rejected live HTLC signature path, second-level virtual-lock asset-leaf encoding, full counterparty commitment monitor persistence, and exact previous-output-bound second-level HTLC aux leaves; funding, commitments, HTLCs, close, monitor, recovery, restart, and BTC-only isolation pass through the fork lifecycle state. | Closed. |
| Done | #76 | Lightning Labs `tapd`/`litd` vectors for simple-taproot asset channels | Implemented in `tap-ldk-core::lightning_labs_interop_checks`; consolidated checks cover funding, HTLC RFQ metadata, RFQ message types, TAPF proof vectors, lifecycle state, close/proof recovery, restart round trips, and observed-balance gates. | Closed. |

The parent Path B epic #19 closes last, after #81, #57, #58, #59, and #60
are complete and Path B reports `live_daemon_gaps_remaining=false`.

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
  artifact path. The report now includes explicit live observed-balance gates,
  and those gates remain incomplete until #57 and #58 record post-settlement
  balances from live counterparties.
- `scripts/path-b-lightning-labs-demo.sh` captures the current Track B harness
  into `target/path-b-lightning-labs-demo/<timestamp>` and records an explicit
  runtime/counterparty dependency gap when the independent Lightning Labs
  target cannot be started. The live outgoing-payment gate now reaches proof
  binding, native payment-session readiness, integrated `litd` readiness,
  fork-backed `ldk-node` to `litd` peer connection, and a pre-settlement
  Lightning Labs current-balance observation before blocking at fork-backed
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
- one native wallet can interoperate with a Lightning Labs LND/tapd node for an
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

- Whether to use final `option_simple_taproot` bits, staging bits, or an
  OpenAgents-only experimental namespace until the BOLT draft settles.
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

1. Finish #81 and #57: run native Rust/LDK asset-channel funding/payment over
   the connected independent `litd` peer and record the Lightning Labs
   receiver's post-settlement observed balance.
2. Finish #58: drive the reverse live payment from Lightning Labs into
   `tap-ldk`, validate the asset HTLC metadata through the LDK/fork boundary,
   persist the received balance and proof reference, and verify restart.
3. Finish #59: make the Path B report fail unless both live directions have
   observed post-settlement balances and matching payment/proof state.
4. Finish #60: replace the remaining proof-envelope boundary with semantic
   Taproot Assets proof ancestry validation and wire it through funding, HTLC,
   close, and recovery.
6. Close #61, #71, and #19 only after their acceptance criteria match the
   implemented live behavior. The detailed issue-by-issue path is in
   `docs/remaining-issue-closure-plan.md`.

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
