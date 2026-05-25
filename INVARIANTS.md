# INVARIANTS

This is the invariant ledger for `tap-ldk`. The project is experimental, but
the demo claim is still a protocol claim: a standalone Rust/LDK wallet can
handle Taproot Asset stablecoin functionality without delegating wallet
behavior to an LND/`tapd` sidecar.

## Agent Maintenance Contract

Before changing an invariant-bearing surface, read this file and
`ROADMAP.md`. Invariant-bearing surfaces include:

- native Taproot Assets parsing, validation, proof handling, and persistence;
- asset issuance, transfer, channel funding, commitment, HTLC, close, and
  recovery state;
- RFQ, invoice, expiry, replay, route, and custom-record behavior;
- rust-lightning/LDK fork or extension boundaries;
- Lightning Labs LND/`tapd`/`litd` compatibility paths;
- regtest, Polar, fixture, smoke, and formal-verification harnesses.

When a change adds, removes, relaxes, or materially reinterprets an invariant:

1. Update this file in the same change.
2. Update the roadmap or formal docs when the invariant affects planned work.
3. Add or update the corresponding fixture, unit test, property test, fuzz
   target, formal model, smoke test, or explicit model-boundary note.
4. Treat the invariant change as a protocol or safety-policy change, not as
   incidental documentation.

Do not weaken native runtime policy to make a formal model, interop test, or
demo pass. A meaningful model counterexample must become either a Rust
regression test or a documented model-boundary exception.

## Current Demo Invariants

- `tap-ldk` is the implementation home for native Rust/LDK Taproot Assets
  proof-of-concept work.
- `stablecoins/` contains source notes and planning material, not runtime
  implementation.
- `projects/` contains reference clones only. Do not turn reference clones into
  owned forks.
- Required upstream forks, including any `rust-lightning` fork, live under the
  `OpenAgentsInc` GitHub organization and are referenced explicitly.
- The demo wallet must not require an LND process or `tapd` sidecar inside the
  wallet runtime.
- LND, `tapd`, and `litd` are independent compatibility peers and reference
  implementations.
- Polar may orchestrate a manual regtest/interop network, but it is not the
  native wallet runtime and not the only automated harness.
- The headless Bitcoin regtest harness is infrastructure only; it must not
  become wallet logic or an implicit sidecar dependency.
- The Lightning Labs counterparty harness is an external interop topology only;
  it must not perform native `tap-ldk` wallet duties.
- The public demo must clearly label mocked pieces: issuer identity, price
  oracle, discovery, proof courier, UI, and any compatibility gaps.

## Native Asset Correctness

- Taproot Assets TLV parsing is strict and rejects malformed, non-canonical, or
  unsupported data rather than silently normalizing it.
- Native asset identity derives from the protocol inputs, not from UI labels or
  local database names.
- Proof import, export, and verification must preserve enough data for restart,
  close, and interop checks.
- Local asset balances are derived from verified proofs and committed channel
  state, not from displayed counters alone.
- Normal BTC Lightning behavior remains isolated from experimental asset
  channel behavior behind feature flags or explicit type boundaries.
- Asset-channel state must be persisted before any corresponding Lightning
  commitment state is treated as safe.
- Restart after funding, quote acceptance, HTLC addition, commitment signing,
  settlement, and close must not create or destroy asset balance.

## rust-lightning Integration Invariants

These are the contracts we definitely want around the rust-lightning work.

- Experimental asset-channel behavior is behind explicit features, types, or
  negotiated channel flags.
- A normal BTC channel must not become an asset channel implicitly.
- A peer must not send asset-channel messages before feature negotiation
  succeeds.
- A peer must not accept asset-channel funding unless the asset ID, genesis,
  group key when present, script key, proof root, and funding output
  commitment all agree.
- Asset proof data may be fragmented for Lightning message-size limits, but a
  funding flow cannot proceed until the complete proof set is reconstructed and
  verified.
- Asset-channel commitment state is versioned with the corresponding
  rust-lightning commitment number.
- The asset-channel blob stored with the channel monitor must be written before
  the corresponding Lightning commitment is considered durable.
- A stale asset-channel state cannot be accepted after restart, force-close, or
  peer reconnect.
- Revoked Lightning commitment states remain revoked at the asset layer.
- An asset-channel failure must fail closed without corrupting normal BTC
  channel state.
- Asset-channel code must not weaken normal channel tests, policy, or
  persistence guarantees in upstream/forked `rust-lightning`.

## RFQ, Invoice, And HTLC Invariants

- A quote binds asset ID, asset amount, BTC amount, peer, expiry, invoice
  context, and replay domain.
- Expired or replayed quotes cannot authorize new asset HTLCs.
- RFQ SCID aliases cannot collide with real local channel SCIDs.
- Asset HTLC metadata must identify the asset, amount, quote binding, and
  final-hop validation context.
- A malformed, stale, wrong-asset, or wrong-amount HTLC blob fails closed.
- BTC amounts exposed to the Lightning layer are quote-derived for asset
  payments.
- Normal BTC payments are unaffected by asset-payment metadata.
- Asset-level signing and nonce contexts must remain separate from BTC-level
  signing and nonce contexts.

## Funding, Commitment, And Balance Invariants

- Issuance is the only demo operation that may create asset supply.
- Split commitments and transfers may move or subdivide assets, but the sum of
  valid outputs must not exceed the verified input amount.
- Channel funding must not create or destroy asset balance relative to the
  verified on-chain proof anchoring the funding output.
- A commitment update must preserve total channel asset balance except for a
  modeled close, sweep, or protocol-defined fee/output edge.
- Both sides must derive the same asset balance view from the same commitment
  and asset-channel blob.
- A malformed, missing, stale, wrong-genesis, wrong-group, or wrong-anchor
  proof fails funding or update validation before any durable channel state is
  advanced.
- Local and remote balances cannot become negative, overflow, or exceed the
  asset amount represented by the verified channel state.
- The wallet must not show a settled asset balance until both the Lightning
  commitment state and the asset-channel state are durable.

## Close, Force-Close, And Recovery Invariants

- Cooperative close returns the exact asset allocation implied by the latest
  mutually valid commitment state.
- Force-close handling preserves asset proof ownership for the rightful owner
  of each spendable output.
- Second-level HTLC outputs that carry asset state must preserve the same asset
  ownership and amount constraints as the parent commitment.
- A failed sweep cannot be reported as recovered.
- A wallet restart cannot discard proof material needed to claim, export, or
  verify the asset after close or force-close.
- A recovered wallet must either reconstruct the latest valid asset-channel
  state or refuse to claim the channel is recovered.
- Proof export after close or sweep must be tied to the final spendable output,
  not to an obsolete commitment view.

## Interop Invariants

- The native-to-native demo and Lightning Labs interop demo are both required.
- Track B must use a Lightning Labs node as an independent counterparty, not as
  the wallet's sidecar.
- Interop claims require a compatibility matrix that says which direction works
  and which protocol or implementation gap remains.
- Lightning Labs funding, HTLC, and commitment blob compatibility claims must
  be fixture-backed. Decoding produces a read-only field map; parsing a blob
  must not mutate wallet state, advance channel state, or silently drop
  unsupported required fields.
- After an interop payment, both sides must report compatible payment state and
  asset balance state, or the mismatch must be documented as a failing gap.

## Formal Verification Stance

Formal verification is worth adding aggressively where it fits the risk:

- Use TLA+ for bounded state machines: asset-channel lifecycle, RFQ
  quote/invoice lifecycle, HTLC settlement, close/recovery, and interop
  handshakes.
- Use Rust unit tests, fixture tests, property tests, fuzzing, and eventually
  Kani for parsers, codecs, pure transition helpers, arithmetic, and bounded
  validation logic.
- Use `loom` only if concurrency or async coordination becomes an actual
  correctness boundary.
- Do not try to formally prove cryptographic primitives, Bitcoin consensus, or
  the full Lightning Network. Depend on audited libraries, protocol vectors,
  and interop tests for those surfaces.
- Every formal model must document assumptions, boundaries, invariants, and
  counterexample handling.
- `scripts/formal-check.sh`, once added, should run checked-in models when TLC
  is available and skip clearly when it is not.
- Formal models inform implementation; they do not authorize runtime behavior
  or replace fixture, interop, persistence, or smoke tests.

## Target Formal Models

Add these formal models as the implementation reaches each surface. Each model
must include `assumptions.md`, `boundaries.md`, `invariants.md`, a `.tla` and
`.cfg` pair when modeled in TLA+, and a counterexample policy.

Current checked-in model map:

- `formal/tla/asset_conservation/`: proof, split, and local balance state.
- `formal/tla/asset_channel/`: negotiation, funding proof, and persistence
  gate.
- `formal/tla/rfq_lifecycle/`: RFQ quote, invoice binding, expiry, alias, and
  single-use lifecycle.
- `formal/tla/asset_commitment/`: commitment-numbered balance transitions,
  revocation, nonce reuse, signing-domain separation, and durability.
- `formal/tla/asset_htlc/`: quote-bound asset HTLC offer, settle, fail, revoke,
  and durability lifecycle.
- `formal/tla/close_recovery/`: cooperative close, force-close recovery,
  sweep, refusal, and proof export lifecycle.
- `formal/tla/interop_handshake/`: Path B proof sync, compatible channel, RFQ,
  payment, balance agreement, and documented-gap lifecycle.

### Core Asset Conservation And Proof State

Model:

- asset genesis;
- asset amount;
- split commitments;
- proof state;
- spendable, spent, pending, and invalid outputs;
- local wallet balance view.

Invariants:

- asset supply is created only by a valid issuance transition;
- transfer and split transitions conserve asset amount;
- an output cannot be both spendable and spent;
- a proof cannot make an output spendable unless its ancestry and anchor are
  valid inside the bounded model;
- local balance equals the sum of verified spendable outputs plus valid
  channel balances, not pending or invalid proofs.

### Asset Channel Negotiation And Funding

Model:

- peers;
- feature negotiation;
- asset proof fragments;
- proof verification;
- funding output construction;
- channel monitor persistence;
- accept/reject states.

Invariants:

- an asset channel cannot open before both peers negotiated support;
- an asset channel cannot open with incomplete or invalid proof data;
- funding cannot advance with mismatched asset ID, genesis, group key, script
  key, or anchor commitment;
- funding either creates one mutually agreed initial asset allocation or fails
  without durable asset-channel state;
- no funding transition creates asset balance beyond verified proof input.

### rust-lightning Aux Hook Boundary

Model:

- channel negotiator;
- funding controller;
- commitment leaf store;
- asset signer;
- HTLC modifier;
- traffic shaper;
- final-hop validator;
- close handler;
- on-chain resolver/sweeper;
- blob parser and persistence codec.

Invariants:

- an asset-channel hook cannot run on a normal BTC channel unless the asset
  feature was negotiated;
- the funding controller cannot approve funding before proof verification and
  commitment construction succeed;
- the HTLC modifier cannot attach asset metadata without an accepted quote;
- the final-hop validator cannot accept an asset HTLC with missing, stale, or
  mismatched metadata;
- the close handler and resolver cannot spend or export proof data for a state
  older than the latest durable channel monitor view;
- the blob parser fails closed on unknown required fields, malformed lengths,
  or version/state mismatches;
- a hook failure leaves the channel in a modeled rejected, failed, or normal
  BTC-only state rather than a partially upgraded asset state.

### Asset Commitment, HTLC, And Revocation Lifecycle

Model:

- commitment number;
- local and remote asset balances;
- offered and received asset HTLCs;
- quote-bound BTC HTLC amount;
- revoked states;
- settle/fail transitions;
- persistence checkpoints.

Invariants:

- total channel asset balance is conserved across commitment updates;
- asset HTLCs cannot settle without valid asset metadata and quote binding;
- expired or replayed quotes cannot create active HTLCs;
- revoked commitment states cannot become valid settlement states;
- asset state persistence happens before the matching Lightning commitment is
  treated as safe;
- restart cannot move from an older persisted asset state to a newer Lightning
  state without the matching asset blob.

### RFQ, Invoice, And SCID Alias Lifecycle

Model:

- quote request;
- quote accept/reject;
- expiry;
- replay domain;
- invoice binding;
- SCID alias allocation and garbage collection.

Invariants:

- a quote can be used at most once unless explicitly modeled as reusable;
- an accepted quote binds the exact asset ID, asset amount, BTC amount, peer,
  invoice, expiry, and route context;
- expired quotes cannot authorize new payments;
- SCID aliases cannot collide with real local channel SCIDs or live quote
  aliases;
- invoice expiry cannot outlive the quote in a way that allows stale asset
  settlement.

### Close, Force-Close, Sweep, And Proof Export

Model:

- cooperative close;
- unilateral close;
- second-level HTLC outputs;
- sweep success/failure;
- proof export;
- restart checkpoints.

Invariants:

- close allocation equals the latest valid asset-channel state;
- force-close does not create, destroy, or reassign assets outside the modeled
  penalty or timeout path;
- a swept output cannot be exported with proof data for a different output;
- failed sweeps do not produce recovered balances;
- restart after close or force-close preserves enough state to either recover
  or explicitly refuse recovery.

### Lightning Labs Interop Handshake

Model:

- native `tap-ldk` peer;
- independent LND/`tapd` or `litd` peer;
- proof sync/import/export;
- channel setup or compatible pre-funded channel state;
- RFQ/invoice exchange;
- payment send/receive;
- balance comparison.

Invariants:

- the Lightning Labs peer is an external counterparty, not a wallet sidecar;
- interop cannot claim success unless both sides agree on asset ID, amount,
  payment state, and resulting balance;
- a compatibility gap must terminate as a documented failure state, not a
  partial success;
- Track B must not use Lightning Labs daemons to perform native wallet duties
  that the demo claims are handled by Rust/LDK.

### Persistence Atomicity

Model:

- asset wallet database version;
- channel monitor version;
- proof store version;
- commitment number;
- crash/restart points.

Invariants:

- there is no reachable recovered state where the Lightning commitment number
  is newer than the persisted asset-channel blob required to interpret it;
- there is no reachable recovered state where a proof is known to be spent but
  the wallet still presents it as spendable;
- crash between asset-state write and channel-monitor write is either atomic,
  recoverable, or explicitly refused on restart;
- persistence repair cannot invent balances.

## Rust-Native Verification Targets

The formal TLA+ models should be paired with Rust-native checks:

- fixture tests for TAP BIP vectors, `tapd` vectors, and LND/`tapd` interop
  traces;
- `proptest` for TLV round trips, non-canonical encodings, amount boundaries,
  SCID alias allocation, quote expiry, and persistence state transitions;
- fuzzing for TLV, proof files, addresses, virtual PSBTs, custom records, and
  imported interop payloads;
- Kani for pure helpers that validate quotes, balance transitions, HTLC
  metadata, persistence versions, and no-overflow amount arithmetic;
- `loom` only for wallet/channel monitor concurrency once the implementation
  has shared mutable state or async coordination worth modeling;
- regression tests for every actionable TLA+ counterexample.

Do not accept a model as useful unless it produces at least one of:

- a checked invariant with a recorded TLC result;
- a regression test derived from a counterexample;
- a documented boundary saying why the model intentionally does not cover a
  production behavior;
- a concrete implementation simplification that makes the invariant easier to
  enforce.

## Formal Artifact Privacy

Formal artifacts, counterexamples, fixtures, and traces must be synthetic or
redacted. Do not commit:

- real wallet seeds, keys, preimages, macaroons, certs, or bearer tokens;
- private repo contents;
- local absolute paths;
- raw shell logs with secrets or private environment data;
- production customer, issuer, payment, reserve, or compliance data.

## Current Open Gaps That Are Not Invariants Yet

Do not claim these guarantees until implementation and verification exist:

- Native strict BigSize/TLV primitives, bounded synthetic asset
  identity/hash+sum conservation helpers, and bounded proof-anchor import,
  export, and verification helpers exist. Bounded Taproot Asset address
  encode/decode and virtual PSBT summary validation exist for the first-demo
  fixture surface, but full TAP BIP MS-SMT, TAP VM, virtual transaction
  signing, and full-history proof validation are not implemented.
- Bounded native asset-channel funding, commitment-numbered asset balance
  transitions, asset HTLC custom-record validation, and native asset
  send/receive, restart-recovery, cooperative-close, and proof-export smoke
  coverage exist for the first demo. Native force-close, full Lightning
  dispatch, and interop payment execution are not implemented.
- `proptest`, fuzzing, Kani, `loom`, Miri, Verus, Prusti, and Creusot are not
  configured.
- No public stablecoin issuance, redemption, compliance, or reserve guarantee
  is implied by this proof-of-concept.
