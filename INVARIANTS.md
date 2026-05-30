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
- `lnd`/`tapd`/`litd` compatibility paths;
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

Rust-native verification is part of that contract. Property tests, fuzz
targets, and Kani harnesses should stay mapped to the formal invariants they
cover, and optional tools must skip explicitly rather than silently removing a
verification boundary from the developer flow.
`scripts/proof-engine-check.sh` and `.github/workflows/proof-engine.yml` are
the current umbrella gates for that policy.

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
- The `lnd`/`tapd`/`litd` counterparty harness is an external interop topology only;
  it must not perform native `tap-ldk` wallet duties.
- The public demo must clearly label mocked pieces: issuer identity, price
  oracle, discovery, proof courier, UI, and any compatibility gaps.
- The first public Taproot Assets demo still uses one asset-channel funding
  outpoint. BTC-level BOLT simple-taproot splice nonce maps are covered in the
  fork, but asset-channel splice/RBF claims need separate asset-state and proof
  coverage before they are demoed.

## Native Asset Correctness

- Taproot Assets TLV parsing is strict and rejects malformed, non-canonical, or
  unsupported data rather than silently normalizing it.
- Native asset identity derives from the protocol inputs, not from UI labels or
  local database names.
- Proof import, export, and verification must preserve enough data for restart,
  close, and interop checks.
- Accepted native proof records must use `semantic-ancestry`, strict regtest
  outpoints, the first-demo normal asset type, derived Taproot Asset root
  hash/sum, and fail-closed expected asset, owner, amount, anchor, and stale
  proof checks before wallet or channel state advances.
- A proof-history replay that can explain wallet or channel balance must pass
  the configured anchor policy. Unknown, stale, or reorged anchors cannot be
  treated as spendable; pending anchors must remain explicit unless a caller
  deliberately opts into pending-anchor acceptance for a bounded policy.
- Lightning Labs `TAPF` imports must validate the proof-file envelope, chained
  checksums, strict `TAPP` TLVs, latest asset-leaf genesis-derived asset ID,
  asset type, amount, owner script key, and genesis outpoint before wallet
  state advances.
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
- BOLT simple taproot channel behavior is behind explicit feature bits and an
  explicit channel type. Staging bits stay isolated for Lightning Labs interop,
  while final `option_simple_taproot` bits are advertised only through the
  separate final-mode config with `option_channel_type` and
  `option_simple_close` available.
- A normal BTC channel must not become an asset channel implicitly.
- A Taproot Asset channel cannot be negotiated unless the BOLT simple taproot
  base channel type was also negotiated.
- A peer that requires simple taproot when the local node has not enabled that
  support must fail closed.
- Simple-taproot wire TLVs must remain optional for legacy channels, but once a
  simple-taproot flow requires a nonce or partial signature, the payload must be
  fixed-width, canonical for its TLV type, and fail closed when malformed,
  duplicated, missing, or unsupported.
- BTC-level simple-taproot MuSig2 signing must use sorted aggregate funding
  keys, domain-separated counter/JIT nonce derivation, persisted nonce-use
  state, and duplicate-use rejection. Asset-level signing must not reuse that
  BTC-level nonce material.
- BTC-level simple-taproot channel update state must persist counterparty
  next-local nonces, consumed nonce uses, and sent partial signatures across
  restart/reestablish. Advertised future local-commitment nonces and
  commitment-signed JIT signing nonces are different protocol roles; both must
  stay domain-separated, the advertised nonce state must exist when required,
  and the JIT signing nonce must be accepted only when the MuSig2 partial
  signature verifies for the exact commitment.
- Lightning Labs staging/overlay simple-taproot interop may use the legacy
  scalar next-local nonce for single-funding `revoke_and_ack` and
  `channel_reestablish`; final simple-taproot and every multi-funding or
  splice context must use type-22 nonce-map entries, and scalar nonce fallback
  must fail closed when more than one funding txid is active.
- BTC-level simple-taproot cooperative close state must persist closee nonce
  indexes, counterparty closee nonces, consumed close nonce uses, and sent
  `closing_complete` partials. Shutdown-advertised closee nonces and JIT
  closer nonces must stay domain-separated, and missing, reused, or mismatched
  close nonces/signatures must fail closed. The #93 fork line covers both
  opener-as-closer and accepter-as-closer RBF rounds, reload after sent
  `closing_complete`, and signed close retention until confirmation.
- BTC-level BOLT simple-taproot splice nonce-map support covers current,
  pending splice, and RBF funding txids, including fail-closed checks for
  missing, empty, duplicate, unknown, scalar-with-multiple-funding, and
  nonce-reuse cases. Asset-channel concurrent splice/RBF support remains
  outside the first public demo until asset-state and proof-transition tests
  cover the same funding-candidate set.
- BTC-level simple-taproot funding outputs must use the same BIP86 P2TR script
  derived from the sorted aggregate funding key in event emission, funding
  transaction validation, and monitor/watch registration.
- BTC-level simple-taproot commitment outputs must preserve enough tapscript
  root, tap tweak, leaf script, and control-block data to reconstruct unilateral
  to-local, to-remote, and anchor script-path spends after restart.
- BTC-level simple-taproot HTLC outputs must preserve BOLT-vector-matching
  offered/accepted tapscript leaves, control blocks, second-level P2TR delay
  outputs, and BIP342 `SIGHASH_SINGLE|ANYONECANPAY` witness shape for each
  offered/accepted success and timeout path. Asset-level HTLC state must be
  layered on top of that base without changing the BTC script semantics.
- BTC-level simple-taproot vector tests must cite the BOLT draft surface they
  replay and distinguish exact script-vector assertions from transaction-case
  assertions when the draft's generated transaction JSON is internally
  inconsistent.
- The current BOLT draft's accepted-HTLC JSON script fields conflict with the
  prose and executable transaction vectors; the final-mode implementation
  follows the prose/transaction vectors, while Lightning Labs staging behavior
  stays behind explicit staging channel-type selection.
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

- The native-to-native demo and `lnd`/`tapd`/`litd` interop demo are both required.
- Track B must use an `lnd`/`tapd`/`litd` node as an independent counterparty, not as
  the wallet's sidecar.
- Interop claims require a compatibility matrix that says which direction works
  and which protocol or implementation gap remains.
- Lightning Labs funding, HTLC, and commitment blob compatibility claims must
  be fixture-backed. Decoding produces a read-only field map; parsing a blob
  must not mutate wallet state, advance channel state, or silently drop
  unsupported required fields.
- Track B funding interop may persist fixture-backed compatibility state only
  when the Lightning Labs funding and commitment blobs agree on asset ID,
  funded amount, and local/remote allocation. Until a live funding outpoint is
  bound to a fully verified proof chain, the state must be marked as a
  documented gap rather than funded success.
- Lightning Labs `TAPF` proof-file compatibility claims must be fixture-backed.
  Import validates the proof-file envelope, chained checksums, and strict
  `TAPP` proof TLV transport before wallet state advances, and export must
  preserve the exact raw proof-file bytes accepted on import.
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
- the rust-lightning `TaprootAssetChannelState` lifecycle must not advance a
  commitment unless the matching asset monitor aux blob is present and valid;
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

### `lnd`/`tapd`/`litd` Interop Handshake

Model:

- native `tap-ldk` peer;
- independent LND/`tapd` or `litd` peer;
- proof sync/import/export;
- channel setup or compatible pre-funded channel state;
- RFQ/invoice exchange;
- payment send/receive;
- balance comparison.

Invariants:

- the `lnd`/`tapd`/`litd` peer is an external counterparty, not a wallet sidecar;
- interop cannot claim success unless both sides agree on asset ID, amount,
  payment state, and resulting balance;
- a compatibility gap must terminate as a documented failure state, not a
  partial success;
- Track B must not use `lnd`/`tapd`/`litd` daemons to perform native wallet duties
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

- No first-demo issue sequence remains open. #81, #57, #58, #59, #60, #61,
  #71, and #19 are complete and remain live settlement, bidirectional payment,
  restart, observed-balance, semantic-proof, first-demo simple-taproot,
  first-demo Taproot Assets-over-LDK, and Path B interop regression gates.
- Native strict BigSize/TLV primitives, native MS-SMT root/proof/compressed
  proof primitives, protocol-shaped `AssetCommitment`/`TapCommitment`
  construction, bounded synthetic asset identity/hash+sum conservation helpers,
  native virtual transition/TAP VM validation for generated TAP BIP fixture
  cases and demo channel funding/commitment updates, and semantic proof import,
  export, and verification helpers exist. Typed proof-history replay now gates
  wallet balances, proof export, channel funding, commitment updates,
  cooperative close, bounded recovery, and anchor-state policy for the
  implemented surfaces. Bounded Taproot Asset address encode/decode and virtual
  PSBT summary validation exist for the first-demo fixture surface, but
  network proof courier/universe service behavior, live chain-watcher
  integration, every historical virtual transaction witness, STXO/split/change
  paths, and grouped assets are not implemented.
- Lightning Labs `TAPF` proof-file transport validation, latest asset-leaf
  semantic validation, genesis-derived asset ID checks, and exact raw proof
  preservation exist for imported fixtures and live proof binding. The local
  proof-courier bundle now moves native proof bytes, optional TAPF bytes,
  proof-history metadata, anchor state, asset fields, and digests together
  through wallet and CLI import/export. Live/network proof discovery remains
  future hardening.
- Lightning Labs funding interop fixture reconciliation exists for asset ID,
  funded amount, and initial local/remote allocation. The integrated `litd`
  first-demo live path now funds and settles in both directions, but broader
  live close, force-close, proof export, and RFQ-signature coverage remain
  future hardening.
- Lightning Labs RFQ request, accept, and reject TLV payload compatibility
  exists for the bounded first-demo message surface. Live daemon RFQ session
  execution and accept-signature verification remain future hardening.
- The consolidated Lightning Labs vector report decodes the funding,
  commitment, HTLC, RFQ, and proof fixtures and is paired with the fork-backed
  first-demo lifecycle and live observed-balance gates. It must stay fail-closed
  if a future live-daemon gap reappears.
- Bounded native asset-channel funding, commitment-numbered asset balance
  transitions, asset HTLC custom-record validation, and native asset
  send/receive, restart-recovery, cooperative-close, and proof-export smoke
  coverage exist for the first demo. Native live force-close, full production
  Lightning dispatch, live proof-courier transport, and live close/sweep
  interop remain future hardening.
- `proptest`, fuzz targets, and Kani harnesses are configured for the current
  proof-engine surfaces through optional local wrappers. `loom`, Miri, Verus,
  Prusti, and Creusot are not configured as production gates.
- No public stablecoin issuance, redemption, compliance, or reserve guarantee
  is implied by this proof-of-concept.
