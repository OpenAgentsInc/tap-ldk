# Current Status And Production Readiness Audit

Date: 2026-05-29

`tap-ldk` is an experimental Rust/LDK implementation of Taproot Assets over
simple-taproot Lightning-style channels. The question it is testing is narrow
and concrete: can an LDK-based wallet issue, verify, send, and receive a
stablecoin-like Taproot Asset without embedding LND or `tapd` as its wallet
runtime, while still interoperating with the existing Lightning Labs software
stack?

The current answer is yes for the bounded regtest proof of concept. The live
interop demonstration has two independent sides. One side is the Lightning Labs
stack, run through integrated `litd`, which exposes LND and Taproot Assets
behavior for the test. The other side is the native LDK wallet path in
`tap-ldk`, using the OpenAgentsInc `ldk-node` and `rust-lightning` forks. In
that setup, integrated `litd` issued the demo Taproot Asset, funded an asset
channel with the native LDK peer, sent that asset to native LDK, and native LDK
recorded the received asset balance. The native LDK side then sent the same
Taproot Asset back to integrated `litd`, and both sides reported the expected
post-settlement balances. That is the central completed demo claim.

There is also a native-to-native proof of concept where two `tap-ldk` wallets
exchange proofs, open a bounded single-asset channel, pay, restart, close,
export proof records, and recover the recorded asset state. Separately, the
BTC simple-taproot BOLT base is now implemented in the pinned Rust Lightning
fork line. Those facts make this a working experimental implementation, not
just a design sketch. They still do not make it production wallet
infrastructure.

This audit records the current state after the bounded interop issue sequence
and the BOLT simple-taproot production tracker were closed. It is written with
the repository pinned to
`OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and
`OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`. The
remaining production work is mostly above and around the completed demo:
complete Taproot Assets proof-history validation, live close and recovery
behavior, asset-channel splice/RBF state, broader interop, network operations,
and verification hardening.

The codebase now has a strong, but specific, implementation claim against the
BOLTs repository's official `bolt-simple-taproot.md` Simple Taproot Channels
spec. The pinned OpenAgentsInc Rust Lightning fork implements the BTC
simple-taproot channel base described by that document. In practical terms,
that means the fork covers final `option_simple_taproot` feature and
channel-type negotiation, keeps the staging feature bits isolated for current
Lightning Labs software interop, requires private channels for this channel
type, parses and serializes the new nonce and partial-signature TLVs,
implements MuSig2 key aggregation, nonce handling, partial-signature
verification, and final Schnorr aggregation, derives BIP86 P2TR funding
outputs from sorted aggregate funding keys, handles simple-taproot funding and
commitment signing, implements the to-local, to-remote, anchor, HTLC, and
second-level HTLC output scripts, supports cooperative close with close nonce
handling and RBF nonce rotation, carries type-22 nonce maps through final
revoke-and-ack and reestablish flows, covers BTC-level splice nonce maps for
current, pending splice, and RBF funding transaction IDs, and persists the tap
tweaks, script roots, leaf scripts, control blocks, nonces, and signatures
needed for unilateral spends and restart.

The implementation is not just a shape match. The fork replays the BOLTs
simple-taproot transaction vectors for the no-HTLC, five-HTLC, trimmed-HTLC,
and HTLC-resolution cases; checks deterministic BIP340 HTLC signatures and
witness stacks; and consensus-verifies to-local, to-remote, anchor, HTLC, and
second-level spend paths. The remaining caveat inside the BOLTs document is a
known accepted-HTLC draft inconsistency: the JSON script fields disagree with
the prose and executable transaction vectors. The fork follows the prose and
transaction vectors for final BOLT mode and keeps the Lightning Labs staging
behavior explicit.

That BOLT implementation claim is deliberately narrow. The BOLTs
simple-taproot document specifies the BTC channel base: feature bits, TLVs,
MuSig2 signing, P2TR funding, commitment outputs, close behavior, HTLC
transactions, unilateral spend data, splice nonce maps, and vectors. Within
that BTC channel-base scope, this project should be treated as implementing
the spec through the pinned OpenAgentsInc Rust Lightning fork. What is not
implemented by the BOLT, and therefore not completed merely by satisfying the
BOLT, is the production Taproot Assets layer: proof-chain replay as the wallet
balance authority, stablecoin issuance policy, proof courier behavior,
grouped-asset behavior, asset-channel splice/RBF state, live close/sweep proof
recovery, reorg handling, and full asset-history recovery. Those are the next
production-hardening items for `tap-ldk`, tracked separately in #96 through
#106. This is also a pinned OpenAgentsInc Rust Lightning fork implementation,
not a claim that the work has already been upstreamed into LDK.

The proof-of-concept issues are closed, and the current open work is the
production-hardening sequence for proof replay and formal verification. That
new sequence is tracked in #96 through #106.

## What This Project Currently Is

`tap-ldk` is an experimental Rust/LDK wallet demo for Taproot Assets. Its
purpose is to show that an LDK-based wallet can hold and move a stablecoin-like
Taproot Asset without delegating wallet duties to LND or `tapd`. LND, `tapd`,
and `litd` are used as independent compatibility peers and reference
implementations. They are not part of the native wallet runtime.

The repo is intentionally split across a few layers. Native asset semantics
live in `tap-ldk-core`. The CLI and scripts are demo and operator surfaces.
Protocol-level Lightning channel changes live in the OpenAgentsInc
`rust-lightning` fork. Runtime surfacing that belongs in a node wrapper lives
in the OpenAgentsInc `ldk-node` fork. External reference repos under
`projects/` are reference material only.

That split matters for the current status. Some of the strongest claims are
not in the `tap-ldk` crate alone; they are in the pinned Rust Lightning fork
and in the pinned `ldk-node` fork. The `tap-ldk` repo is the coordination
point that pins those revisions, exposes verification scripts, holds fixtures,
and runs the demo flows.

The project has two completed proof-of-concept surfaces. The first is native
wallet-to-wallet: two native `tap-ldk` wallets issue or receive a demo asset,
exchange proof material, open a bounded single-asset channel, pay, restart,
close, export proof records, and recover the recorded state. The second is
interop with the Lightning Labs stack: integrated `litd` issues the asset and
funds an asset channel against the fork-backed native LDK peer, then the same
Taproot Asset is paid in both directions and both sides report the expected
balances after settlement.

Those flows are enough for the experimental claim: native Rust/LDK code can
drive Taproot Asset channel behavior and can trade the same Taproot Asset back
and forth with the Lightning Labs `litd`/LND/Taproot Assets stack in a bounded
regtest setup. They are not enough for a production stablecoin wallet.

## Current Fork And Issue State

The current `tap-ldk` line consumes `OpenAgentsInc/rust-lightning` at
`3db3229733b724f45e7a356d923715213cb4f269`. That fork revision contains the
simple-taproot feature negotiation, MuSig2 signing path, P2TR funding,
commitment output scripts, HTLC output scripts, cooperative close handling,
RBF close nonce rotation, splice nonce maps, BOLT vector replay, unilateral
spend checks, and restart metadata reconstruction for the BTC simple-taproot
base.

The current `tap-ldk` line also consumes `OpenAgentsInc/ldk-node` at
`1e439b10c94a6e42442d245f95945a906dd6221e`. That runtime fork is used by the
live compatibility harness. It exposes the fork provenance, the opt-in
simple-taproot and Taproot Asset channel configuration, the typed Taproot
Asset message and payment surfaces needed by the demo, and the fork-backed
asset payment accounting used by the live `litd` checks.

Issues #19 and #57 through #95 are closed. The bounded proof-of-concept
regression gates are #81, #57, #58, #59, #60, #61, #71, and #19. The BTC
simple-taproot production tracker is #95, closed after #94 added the full
vector, unilateral spend, and restart metadata checks. The useful way to read
that closed issue state is not "there is nothing left to do." The useful
reading is that the interop proof of concept and the BTC simple-taproot base
are no longer blocked by the issues that were created for them. The next phase
is now tracked separately in #96 through #106 for production proof replay and
formal verification hardening.

## What Works Now

The native wallet-to-wallet demo works for the bounded proof-of-concept shape.
It can issue a demo `OPENUSD` asset, store proof-backed balances, exchange proof
material locally, build a single-asset channel, bind an RFQ quote to the asset
payment context, create and validate asset HTLC metadata, settle the payment,
restart through important state boundaries, cooperatively close, and export
the recorded proof state. The native demo is not a full production network
wallet, but it is no longer a loose collection of placeholders. The main
primitives now connect through a coherent demo lifecycle.

The native asset primitives are substantially stronger than the early
placeholder design. The code includes native MS-SMT root and proof handling,
compressed proof encoding, asset commitment and TapCommitment construction,
tap leaf parsing, output-root binding, TAP VM-style validation for issuance,
transfer and split, channel funding, and commitment update conservation.
Semantic proof validation is strict for the current demo surface. It checks
asset identity, owner, amount, anchor, stale proof state, and Lightning Labs
`TAPF` proof-file structure before wallet or channel state advances.

The proof import posture is deliberately fail-closed. A proof cannot be used
just because the UI or a local label says it represents an asset. The wallet
requires protocol-derived identity and semantic agreement with the expected
asset and owner data. That is the right posture for this project. The parts
that remain incomplete are not because the project accepts loose proof data;
they are because full production proof history is larger than the first-demo
surface.

The BTC simple-taproot base now has a credible local production claim inside
the pinned Rust Lightning fork line. Final `option_simple_taproot` negotiation
is separate from Lightning Labs staging interop. Final mode requires the
related channel-type and simple-close support, keeps opens private, and uses
type-22 nonce maps for final RAA and reestablish. BTC-level splice nonce maps
cover current, pending splice, and RBF funding transaction IDs, including
fail-closed tests for missing, empty, duplicate, unknown, scalar fallback, and
nonce-reuse cases. Cooperative-close RBF rotates closee and closer nonces,
persists sent and received close state, retains signed close transactions
until confirmation, and rejects missing or reused close state.

The BOLT simple-taproot vector work is also in place. The fork replays the
no-HTLC, five-HTLC, trimmed-HTLC, and HTLC-resolution transaction vectors. It
checks complete witness stacks and deterministic remote HTLC signatures. It
consensus-verifies to-local, to-remote, anchor, HTLC, and second-level spend
paths. It also reconstructs tap tweaks, script roots, leaf scripts, and
control blocks after commitment serialization. There is still a known draft
conflict in `bolt-simple-taproot.md`: the accepted-HTLC JSON script fields do
not agree with the prose and executable transaction vectors. The
implementation follows the prose and transaction vectors for final BOLT mode,
while keeping Lightning Labs staging behavior explicit.

The Lightning Labs compatibility demo works for the first-demo scope. The
integrated `litd` counterparty can issue the demo asset, fund the asset
channel against the fork-backed native peer, send the asset to native LDK,
and report the Lightning payment as succeeded. The native side records the
received asset payment and the local receiver balance. The native side can
then pay the asset back to `litd` with the canonical Taproot Asset HTLC blob
and a BTC amount large enough for LND's dust checks. The wrapper-level
compatibility completion report now depends on observed balances instead of
fixture-only or expected-only data.

The live failure modes that used to block the demo have been turned into
regression surfaces. The post-claim zero-HTLC partial-signature mismatch is
fixed. The stale force-close fallback that tried to spend a P2TR funding input
with a legacy P2WSH witness is fixed by persisting and using the aggregate
key-path Schnorr signature. Missing simple-taproot open and accept nonces fail
before channel state advances. Public simple-taproot and Taproot Asset opens
are rejected for the current private-channel policy. Legacy BTC channel
behavior remains isolated from the experimental asset-channel behavior.

Persistence is good enough for the demo claim. The repo has restart checks for
wallet state, receiver payment state, asset-channel state, monitor aux blobs,
cooperative close allocation, and proof-ownership recovery records. The
important invariant is already explicit: asset-channel state must be durable
before the matching Lightning commitment is treated as safe. The current
tests and models support that claim for the first-demo flows.

Formal modeling exists as a design and regression discipline. The repo has
checked-in TLA+ model directories for asset conservation, asset channel
negotiation and funding, RFQ lifecycle, asset commitment, asset HTLC,
close/recovery, and interop handshake. The `scripts/formal-check.sh` runner
executes checked-in TLA+ configs when `tlc` or `TLA_TOOLS_JAR` is available
and skips clearly otherwise. These models do not prove the whole system, but
they make the important bounded invariants explicit and give future work a
place to put counterexamples.

## What Is Still Experimental

The project is not a production stablecoin issuer, wallet, or compliance
system. It does not claim reserve management, redemption, sanctions screening,
issuer governance, custody policy, accounting controls, audit trails, or
consumer-facing operational readiness. `OPENUSD` is a demo asset name, not a
production asset promise.

The Taproot Assets proof engine is not production complete. The current proof
checks are intentionally strict for the bounded demo, but production still
needs full proof-history replay for every historical virtual transaction
witness. It needs complete STXO, split, and change-output proof replay. It
needs grouped assets, multi-asset paths, reissuance and collectible edge
cases where those matter, and policy for reorg watchers. It also needs a
production proof courier or universe policy that defines how proofs are found,
validated, retained, exported, and repaired without relying on local demo
fixtures.

The asset-channel overlay is not production complete just because the BTC
simple-taproot base is now covered. BTC-level simple-taproot splice nonce maps
are implemented, but concurrent splice/RBF for asset channels still needs its
own asset-state and proof-transition logic. Every active funding candidate
must have matching asset-channel state, proof ownership, and recovery data.
Until that exists, the first public demo correctly stays with one
asset-channel funding outpoint.

Live close and recovery are also not production complete. Native cooperative
close is covered for the bounded demo, and the `litd` harness exposes a live
close command. What is still missing is live post-close proof and balance
observation through the independent counterparty path. Live force-close,
second-level HTLC sweep, proof export after sweep, failed-sweep behavior, and
recovery after restart need to be driven against actual on-chain regtest
spends, not only fixture and unit surfaces.

Interop is good enough for the first demo, not for broad compatibility. The
current live path uses integrated `litd` and the exact bounded flow that was
closed by #19, #57, #58, #59, #60, #71, and #81. Production readiness needs
more than that. It needs live LND/`tapd` RFQ sessions, RFQ accept-signature
verification, proof sync and proof export across more flows, independent
balance reconciliation after close and force-close, negative-path interop
tests, and a compatibility matrix that is kept current as Lightning Labs
software changes.

Routing and network behavior are still early. The first demo uses a direct
asset channel and controlled regtest topology. Production needs route
discovery, quote discovery, SCID alias collision prevention at larger scale,
expiry semantics that cannot create stale settlement paths, and eventually
multi-hop or MPP behavior if the wallet is meant to route across multiple
asset channels. BOLT 12 offer or invoice work is still future work.

The storage layer is not yet a production database design. It has the
restart-safe states needed for the demo, but production needs a clear atomic
write story across wallet records, proof records, channel monitor updates,
asset-channel blobs, and recovery indexes. It needs backup and restore policy,
migration policy, corruption handling, operator observability, and explicit
refusal states for partial recovery.

The formal and automated verification stack is incomplete. TLA+ models exist,
but not every model has been run in the local environment on every commit.
Rust-native verification should grow beyond unit and fixture tests into more
property testing, fuzzing, and bounded model checking. `proptest`, fuzzing,
Kani, Miri, loom, Verus, Prusti, and Creusot are not configured as production
gates. The practical next step is not to try to prove the entire Lightning and
Taproot Assets stack. The next step is to pick narrow, high-risk pure
functions and state transitions and make them mechanically checked.

Upstreaming is also unresolved. The OpenAgentsInc forks are the right place
for fast iteration, but production readiness should not depend forever on a
large private divergence from upstream LDK. BTC simple-taproot support should
be separated from Taproot Assets overlay work where possible, reviewed on its
own merits, and reduced into upstreamable pieces. The Taproot Assets overlay
may stay experimental longer, but the boundaries between upstreamable channel
machinery and OpenAgents-specific demo glue should become clearer.

## Production Readiness Work That Should Come Next

The next production-hardening phase should start with the proof engine. The
wallet needs full proof-history replay across issuance, split, transfer,
channel funding, commitment update, close, and sweep outputs. The current
semantic proof boundary is useful, but a production wallet must be able to
explain every accepted asset balance from a valid proof chain, not just from a
bounded first-demo fixture. This work should include negative vectors for
wrong genesis, wrong anchor, stale proof, malformed proof-file transport,
invalid split sums, wrong owner script key, missing STXO, and reorg-sensitive
history.

The second phase should harden on-chain lifecycle behavior. The project needs
live regtest tests for cooperative close proof export, unilateral close,
second-level HTLC timeout and success spends, sweep success, sweep failure,
restart during sweep, and recovery from only persisted wallet plus monitor
state. The important production question is not merely whether the Bitcoin
transaction is valid. It is whether the asset owner can still prove ownership
of the correct asset output after the transaction confirms.

The third phase should make asset-channel splice and RBF a first-class asset
state machine. The BTC simple-taproot base already knows how to carry nonce
maps for multiple funding candidates. The asset overlay must now prove that
every funding candidate has the right asset allocation, proof transition,
monitor aux blob, and recovery record. If the project does not want to support
asset-channel splice/RBF in the next demo, the wallet should continue to reject
or gate it explicitly instead of silently entering an unmodeled state.

The fourth phase should broaden live interop. The current integrated `litd`
flow should remain a regression, but production needs more live tests:
native-to-`litd`, `litd`-to-native, close, force-close, proof export, RFQ
acceptance, RFQ rejection, quote expiry, malformed asset HTLC metadata, wrong
asset amount, wrong proof, and restart during interop. The outcome should be a
plain compatibility matrix that says exactly which Lightning Labs software
versions and flows are known-good, known-failing, or intentionally unsupported.

The fifth phase should turn verification into a normal gate. `cargo test
--locked`, formatting, the BOLT simple-taproot scripts, the native demo, and
the compatibility demo should continue to run. On top of that, the project
should add property tests for TLV parsing, amount conservation, quote expiry,
SCID alias allocation, proof-state transitions, and asset-channel persistence.
It should add fuzz targets for TLVs, proof files, virtual PSBTs, custom
records, and Lightning Labs blobs. It should add Kani or another bounded Rust
checker for pure amount arithmetic and state-transition helpers. It should run
TLA+ models in CI when the runner is available and record explicit skip
behavior when it is not.

The sixth phase should reduce fork risk. The Rust Lightning changes should be
split into reviewable units: BTC simple-taproot base, MuSig2 signer and nonce
state, simple-taproot close/RBF, splice nonce maps, Taproot Asset channel
state hooks, HTLC asset metadata, monitor aux persistence, and recovery hooks.
The more that can be upstreamed or at least kept as small, well-tested fork
patches, the less brittle the wallet becomes.

## Release Bar For A Production Claim

A production claim should require more than the first-demo scripts passing.
The wallet should not present production stablecoin readiness until the native
proof engine can replay complete asset history, the on-chain close and
recovery paths are live-tested, interop has a maintained compatibility matrix,
and asset-channel persistence has an atomicity and recovery story that holds
under crash and restart.

The BTC simple-taproot base can be described as implemented in the pinned fork
line, with the known BOLT draft conflict documented. The Taproot Assets wallet
cannot yet be described as production ready. It can be described as a working
experimental proof of concept with a completed first-demo path, live
`litd` compatibility for bounded asset payments in both directions, and a
clear remaining hardening path.

That wording matters. The project has crossed the line from architecture plan
to working experimental implementation. It has not crossed the line from
working experimental implementation to production financial software.

## Proof Engine And Formal Verification Roadmap

The next production-hardening sequence should treat proof history as the core
wallet authority. The wallet should not accept an asset balance because a demo
fixture, local counter, latest proof leaf, or channel state says the balance
exists. It should accept a balance only when it can replay the relevant proof
chain and explain how that balance was created, moved, split, locked into a
channel, updated through commitments, closed, swept, or rejected. The formal
verification work should be added at the same time as the implementation work,
because the model is the clearest way to keep the acceptance rule narrow.

The first concrete step of that sequence is now in code. Issue #97 adds a
typed proof-history replay engine in `tap-ldk-core::proof`, with runtime
states aligned to the planned `formal/tla/proof_validation` vocabulary and
tests for valid lifecycle replay plus missing or contradictory histories. The
engine is a new authority surface, not yet the only wallet gate; #100 through
#103 track wiring it into wallet balances, funding, commitments, close, sweep,
and recovery.

The second step is now represented in the checked formal harness. Issue #98
adds `formal/tla/proof_validation/ProofValidation.tla` and `.cfg`, modeling a
bounded proof-history path through import, issuance, split, transfer, channel
funding, commitment update, close, and sweep. The model checks that accepted
balances have valid issuance history, coherent asset fields, a well-formed
proof file, present STXO, stable chain view, and no bad proof reason. It also
models wrong genesis, wrong anchor, wrong owner, invalid split sum, malformed
proof-file transport, missing STXO, stale proof, and reorg-sensitive history
as paths that cannot end in accepted balances.

The first implementation step is to replace the current bounded proof-history
surface with an explicit proof-chain replay engine. That engine should live
around `proof.rs`, `tapd_proof.rs`, `mssmt.rs`, `taproot_commitment.rs`, and
`tap_vm.rs`, with call sites in funding, commitment, close, recovery, wallet
balance, and Lightning Labs interop paths. The replay engine should build a
typed history graph from issuance, split, transfer, channel funding,
commitment update, cooperative close, unilateral close, second-level HTLC,
sweep, and proof-export records. Each accepted output should point to the
exact virtual transition, TapCommitment root, owner script key, anchor
outpoint, amount, asset ID, and prior state that justify it. If any link is
missing or contradictory, the wallet should refuse to advance state instead
of presenting a balance.

The matching formal work should start by completing
`formal/tla/proof_validation/`. That directory already has assumptions,
boundaries, and invariants, but it does not yet have a checked `.tla` and
`.cfg` pair. The model should represent a bounded asset universe with issued,
pending, spendable, spent, channel-locked, closed, swept, stale, and rejected
states. It should model valid and invalid proof transitions for issuance,
split, transfer, funding, commitment update, close, and sweep. The main
invariant should be explainability: every accepted wallet or channel balance
must be reachable from a valid issuance path through valid transitions whose
asset ID, owner, amount, anchor, and root all agree. The model should also
prove that wrong genesis, wrong anchor, wrong owner script key, invalid split
sum, missing STXO, malformed proof-file transport, stale proof, and
reorg-sensitive history all terminate in rejected or unresolved states, never
in accepted balance.

The second implementation step is to make negative proof vectors first-class.
The repo should add fixtures for wrong genesis, wrong anchor, stale proof,
malformed `TAPF` proof-file transport, invalid split sums, wrong owner script
key, missing STXO, wrong asset type, wrong amount, wrong root hash, wrong root
sum, mismatched TapCommitment output root, and reorg-sensitive history. These
fixtures should exercise both native proof import and Lightning Labs proof-file
interop. They should not be isolated parser tests only. At least one path must
try to move each invalid proof into wallet balance state, channel funding
state, commitment state, close state, or recovery state, and the state advance
must fail closed.

The formal companion for those vectors should extend the proof-validation TLA+
model with explicit invalid transitions, then map every meaningful TLC
counterexample to a Rust regression test. If TLC finds a sequence where a
malformed proof becomes accepted because the model failed to carry one field
through a split or close transition, the Rust test should assert that the same
field is checked in the real replay engine. If a behavior is intentionally
outside the model, the exception should be written into
`formal/tla/proof_validation/boundaries.md` before the code relies on it.

The third implementation step is to connect proof replay to channel funding
and commitment updates. A Taproot Asset channel should not open from a latest
leaf alone. The funding path should prove that the asset input being locked
into the channel is spendable, owned by the expected key, tied to the expected
asset ID and genesis, and conserved into the channel allocation. Each
commitment update should then be able to derive the new local and remote asset
allocation from the previous accepted channel state plus the HTLC or balance
transition being applied. This belongs in `asset_channel_funding.rs`,
`asset_commitment.rs`, `simple_taproot_asset_channel.rs`, and the
OpenAgentsInc Rust Lightning asset-channel state hooks.

The matching models are `asset_channel`, `asset_commitment`, and
`asset_conservation`. They should be tightened so proof replay is not an
external assumption. Funding should require a verified input proof state.
Commitment updates should require conservation from the prior durable state.
A restart should not recover a newer Lightning commitment number unless the
matching proof replay state and asset-channel blob are also present. These
models do not need to know Bitcoin script semantics; they do need to prove
that the wallet never creates or forgets asset balance through funding,
commitment, rejection, or restart.

The fourth implementation step is close and sweep replay. Cooperative close
should export proof material tied to the actual close outputs. Unilateral
close should preserve proof ownership for every spendable output, including
second-level HTLC success and timeout paths. Sweep should either produce an
accepted proof state for the final spendable output or a clear unrecovered
state. A failed sweep must not be reported as recovered. This work belongs in
`asset_close.rs`, `asset_recovery.rs`, `wallet.rs`, the Rust Lightning proof
ownership recovery hooks, and the live regtest close/sweep scripts.

The formal companion is `close_recovery`. That model should stop treating
proof recovery as a simple terminal state and instead model close output,
second-level output, sweep attempt, sweep success, sweep failure, proof export,
and restart. Its key invariant should be that close and sweep do not create,
destroy, or reassign assets outside the modeled penalty or timeout path. A
recovered balance must correspond to the latest accepted proof state for the
actual spendable output.

The fifth implementation step is reorg-sensitive history. Production proof
acceptance needs a chain-state boundary. The wallet should distinguish a proof
whose anchor is confirmed and stable enough for the configured policy from a
proof whose anchor is missing, stale, reorged, or still pending. For regtest
and demo mode this can start as a small synthetic chain-state adapter, but the
core replay engine should already carry enough status to avoid treating a
reorged output as spendable. Reorg handling should be added before the wallet
claims production-grade proof import or recovery.

The formal model should keep reorg handling bounded. It should not attempt to
prove Bitcoin consensus. It should model anchor states such as unknown,
pending, confirmed, stale, and reorged, then prove that accepted balances can
only depend on anchors that satisfy the configured policy. A reorged anchor
should move dependent balances to rejected or unresolved until a valid
replacement proof path is replayed.

The sixth implementation step is to make Rust-native verification a gate. The
TLA+ models should describe the intended state space, but the parser,
arithmetic, serialization, and validation code must be checked in Rust. Add
`proptest` coverage for amount conservation, split sums, proof graph
topology, quote/proof binding, and restart state transitions. Add fuzz targets
for TLV parsing, `TAPF` proof files, virtual PSBT summaries, TapCommitment
records, and imported Lightning Labs blobs. Add Kani targets for pure helpers
that validate asset amount arithmetic, owner/asset matching, proof-state
transitions, and no-overflow conservation. Miri can be used for unsafe or
FFI-sensitive paths if those appear; loom should wait until proof or channel
state uses shared concurrent mutation that actually needs interleaving
coverage.

The seventh implementation step is to wire the checks into normal developer
and CI flow. `scripts/formal-check.sh` already discovers checked-in TLA+
configs and skips cleanly when no checker is installed. Once
`ProofValidation.tla` and `ProofValidation.cfg` exist, that script should run
the proof-validation model with the rest of the checked specs. A future CI job
should run formatting, `cargo test --locked`, proof negative vectors, the
native demo, the compatibility demo, the BOLT simple-taproot scripts, formal
checks when TLC is available, property tests, fuzz smoke targets, and Kani
targets. If a tool is optional locally, the skip behavior must be explicit and
visible rather than silently ignored.

The eighth implementation step is documentation and issue hygiene. This work
should be split into a new GitHub issue sequence rather than buried under the
closed first-demo issues. The sequence should track the proof replay engine,
negative vectors, proof-validation TLA+ model, channel-funding proof replay,
commitment proof replay, close/sweep proof replay, reorg-sensitive policy,
property/fuzz/Kani harnesses, CI integration, and compatibility-matrix updates.
Each issue should state the implementation files, the model files, the
negative vectors, and the verification command that closes it.

The exit bar for this roadmap is precise. A production proof-engine claim can
be made only when every accepted wallet balance, channel balance, close output,
sweep output, and exported proof can be explained from a replayed proof chain;
when the listed negative vectors fail closed at every state-advance boundary;
when the proof-validation, conservation, channel, commitment, and close models
have checked configs; when meaningful model counterexamples have corresponding
Rust regressions; and when the normal check suite includes both model-level
and Rust-native verification for the replay engine. Until then, the correct
claim remains that `tap-ldk` has a strict first-demo semantic proof boundary,
not a production-complete Taproot Assets proof engine.
