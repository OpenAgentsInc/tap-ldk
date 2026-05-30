# Current Status And Production Readiness Audit

Date: 2026-05-30

## Official BOLT Simple Taproot Status

Against the official BOLTs repository's `bolt-simple-taproot.md` specification,
the current project should be read this way: the Bitcoin channel base is
implemented in the pinned OpenAgentsInc `rust-lightning` fork, while the
Taproot Assets stablecoin layer remains experimental production-hardening work.
The BOLT base claim covers final `option_simple_taproot` negotiation, required
nonce and partial-signature messages, MuSig2 key/signature handling, BIP86 P2TR
funding, simple-taproot commitment outputs, cooperative close and close RBF,
reconnect nonce maps, BTC-level splice nonce maps, HTLC and second-level
outputs, unilateral spend metadata, restart reconstruction, and the BOLT vector
replay suite. It does not claim that upstream LDK has merged the work, and it
does not by itself make the Taproot Assets wallet proof engine production
ready.

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
live chain-watcher policy, and verification hardening.

The remaining caveat inside the BOLTs document is a known accepted-HTLC draft
inconsistency: the JSON script fields disagree with the prose and executable
transaction vectors. The fork follows the prose and transaction vectors for
final BOLT mode and keeps the Lightning Labs staging behavior explicit.

What is not implemented by the BOLT, and therefore not completed merely by
satisfying the BOLT, is the production Taproot Assets layer: network proof
transport, stablecoin issuance policy, grouped-asset behavior, asset-channel
splice/RBF state, live close/sweep proof recovery, live reorg-watcher
integration, and full asset-history recovery beyond the current bounded replay
surfaces.

The proof-of-concept issues are closed. The first production-hardening
sequence for proof replay and formal verification is also closed in #96
through #106. The local proof-transport hardening sequence is now closed in
#107 through #110: accepted proofs, raw `TAPF` files, proof-history metadata,
anchor state, asset fields, and digests move together as a typed
proof-courier bundle instead of being passed around as loose local files. The
remaining proof-transport gap is the future network/universe service layer,
not the local bundle boundary.

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
are no longer blocked by the issues that were created for them. The #96
through #106 proof-replay and formal-verification hardening phase is also
closed. Future production phases should now be filed as new issue sets, not
reopened first-demo issues.

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
cases where those matter, and policy for reorg watchers. The local
proof-courier bundle now defines how the wallet imports and exports proof
material without loose files, but production still needs a network proof
universe/courier service that defines how proofs are found, retained,
repaired, and synchronized without relying on local demo fixtures.

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

The formal and automated verification stack is bounded, not complete.
`scripts/proof-engine-check.sh` now runs formatting, locked tests, formal
checks, Rust-native verification, and the native demo, while optional fuzz and
Kani paths skip visibly when the tools are absent. This should keep expanding
with each production epic. The practical next step is not to try to prove the
entire Lightning and Taproot Assets stack. The next step is to pick narrow,
high-risk pure functions and state transitions and make them mechanically
checked.

Upstreaming is also unresolved. The OpenAgentsInc forks are the right place
for fast iteration, but production readiness should not depend forever on a
large private divergence from upstream LDK. BTC simple-taproot support should
be separated from Taproot Assets overlay work where possible, reviewed on its
own merits, and reduced into upstreamable pieces. The Taproot Assets overlay
may stay experimental longer, but the boundaries between upstreamable channel
machinery and OpenAgents-specific demo glue should become clearer.

## Production Readiness Work That Should Come Next

The proof-engine hardening phase and local proof-courier phase are closed for
the current bounded wallet surfaces. The wallet now has typed proof-history
replay across issuance, split, transfer, channel funding, commitment update,
close, and bounded sweep/recovery outputs, plus negative vectors, formal
checks, Rust-native verification, CI wiring, and a typed local proof-courier
bundle. That does not finish production proof handling.

The local proof-courier epic added a typed bundle and policy. It exports only
proofs that the wallet can explain through replayed history, records the proof
and optional `TAPF` digests, carries anchor state explicitly, refuses pending,
stale, or reorged spendable export claims, imports bundles through the same
semantic and proof-history gates as direct proof import, and exposes CLI
commands that make local courier behavior visible. This is still local
transport, not a decentralized universe service, but it closes the loose-file
gap before network proof discovery is added. The remaining production
proof-transport gap is network proof discovery, retention, repair, and
synchronization.

The following phase should harden on-chain lifecycle behavior. The project needs
live regtest tests for cooperative close proof export, unilateral close,
second-level HTLC timeout and success spends, sweep success, sweep failure,
restart during sweep, and recovery from only persisted wallet plus monitor
state. The important production question is not merely whether the Bitcoin
transaction is valid. It is whether the asset owner can still prove ownership
of the correct asset output after the transaction confirms.

The next issue wave is the bounded lifecycle gate before the live chain watcher.
The code now exposes one typed lifecycle report through
`tap-ldk onchain-lifecycle-smoke` and the Path A artifact set. Each lifecycle
event names its channel, asset, amount, proof-history output, proof handoff
digest, wallet or monitor evidence when required, and terminal status. Sweep
failure remains a refusal, BTC-only sweep state does not count as asset
recovery, and restart recovery requires both wallet and monitor evidence. Once
that bounded report is green in the normal proof-engine check, which now runs
`scripts/onchain-lifecycle-smoke.sh`, later live-regtest work can feed it from
actual chain notifications and sweeper callbacks.

The next production issue wave should add that feed boundary without claiming
the live daemon path too early. The repo needs a typed chain/sweeper
observation report that can sit next to the lifecycle report. It should record
which lifecycle events have a close anchor, unilateral anchor, second-level
HTLC anchor, final sweep anchor, failed sweep callback, reorg marker, or
wallet/monitor restart observation. Confirmed lifecycle recovery must require
confirmed observations. Stale or reorged anchors must remain refused, and a
BTC-only or failed sweep callback must not be able to turn into Taproot Asset
recovery by naming the right channel. This is the next honest step before
replacing bounded observations with actual live regtest watcher callbacks.

The phase after that should make asset-channel splice and RBF a first-class asset
state machine. The BTC simple-taproot base already knows how to carry nonce
maps for multiple funding candidates. The asset overlay must now prove that
every funding candidate has the right asset allocation, proof transition,
monitor aux blob, and recovery record. If the project does not want to support
asset-channel splice/RBF in the next demo, the wallet should continue to reject
or gate it explicitly instead of silently entering an unmodeled state.

The next interop phase should broaden live coverage. The current integrated `litd`
flow should remain a regression, but production needs more live tests:
native-to-`litd`, `litd`-to-native, close, force-close, proof export, RFQ
acceptance, RFQ rejection, quote expiry, malformed asset HTLC metadata, wrong
asset amount, wrong proof, and restart during interop. The outcome should be a
plain compatibility matrix that says exactly which Lightning Labs software
versions and flows are known-good, known-failing, or intentionally unsupported.

Verification is now part of the normal gate, but it should keep expanding as
new production surfaces land. `scripts/proof-engine-check.sh` runs formatting,
locked tests, formal checks, Rust-native verification, the native demo, and the
bounded on-chain lifecycle report; the extended mode adds the BOLT scripts and
compatibility demo. Future epics should add their own property tests, fuzz
targets, Kani harnesses, and formal notes to that wrapper instead of relying on
ad hoc one-off commands.

The long-running fork-risk phase should reduce divergence. The Rust Lightning changes should be
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

The first proof-engine and formal-verification hardening sequence is complete
for the bounded repo surfaces tracked in #96 through #106. The governing rule
remains the same: the wallet should not accept an asset balance because a demo
fixture, local counter, latest proof leaf, or channel state says the balance
exists. It should accept a balance only when it can replay the relevant proof
chain and explain how that balance was created, moved, split, locked into a
channel, updated through commitments, closed, swept, or rejected. New
production epics should extend that rule rather than bypassing it.

The first concrete step of that sequence is now in code. Issue #97 adds a
typed proof-history replay engine in `tap-ldk-core::proof`, with runtime
states aligned to the planned `formal/tla/proof_validation` vocabulary and
tests for valid lifecycle replay plus missing or contradictory histories. The
engine is a new authority surface, not yet the only wallet gate; #100 through
#104 track wiring it into wallet balances, funding, commitments, close, sweep,
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

Issue #99 adds the corresponding negative-vector checklist and tightens the
formal invalid-transition set for wrong asset type, wrong amount, wrong root
hash, wrong root sum, and mismatched TapCommitment output root. The point is
to keep every bad proof class tied to a state-advance boundary, not just a
parser error.

Issue #100 starts wiring the replay engine into wallet authority. Imported
wallet proofs now carry deterministic proof-history metadata, and wallet
balances plus proof exports must replay that metadata to an accepted balance
explanation before returning state. This is still the bounded first-demo
wallet path, not the full channel/close/sweep replay path; #101 through #103
extend the same authority boundary across those later states.

Issue #101 extends replay authority to asset-channel funding. The funding path
now builds a proof-history replay from accepted input proofs into a
channel-locked funding output before durable channel state is recorded. The
stored channel also carries deterministic proof-history metadata so a tampered
funding-history pointer fails validation.

Issue #102 extends replay authority through asset commitment updates. The
commitment store now starts from the channel-locked funding proof-history
output, consumes the previous channel-locked output on each commitment update,
and records a new channel-locked proof-history output tied to the TapVM virtual
transition. Restart validation rebuilds that chain alongside the monitor aux
blob, so a newer commitment state without matching proof-history metadata is
refused.

Issue #103 extends replay authority through close, recovery, and close-proof
export. Cooperative close now consumes the latest channel-locked
proof-history output, produces closed local and remote outputs, and then
records proof-export transitions for the exact final proofs imported and
exported by the wallet. The recovery matrix also records replayed
proof-history output metadata for unilateral commitment, second-level HTLC,
and final sweep spend kinds; those reports end in closed or swept state rather
than a generic recovered flag.

Issue #104 adds a bounded chain-state boundary to the proof replay engine and
wallet. Proof replay can now run with an explicit anchor policy whose states
are unknown, pending, confirmed, stale, and reorged. The existing bounded
regtest `replay()` entry point still assumes confirmed anchors, but wallet
balance and proof-export authority now use stored anchor state. Confirmed
anchors are spendable. Pending anchors are retained as explicit pending state
and are not counted by default. Stale or reorged anchors are retained as
rejected wallet state and cannot produce accepted balances or exported proofs
until a replacement proof path is imported.

The replay engine baseline is now in place. It builds a typed history graph
from issuance, split, transfer, channel funding, commitment update,
cooperative close, unilateral close, second-level HTLC, sweep, and
proof-export records. Each accepted output points to the virtual transition,
Taproot Asset root, owner script key, anchor outpoint, amount, asset ID, and
prior state that justify it. If a required link is missing or contradictory,
the wallet should refuse to advance state instead of presenting a balance.

The matching `formal/tla/proof_validation/` work is also now checked in. It
models a bounded asset universe with issued, pending, spendable, spent,
channel-locked, closed, swept, stale, and rejected states. Its main invariant
is explainability: every accepted wallet or channel balance must be reachable
from a valid issuance path through transitions whose asset ID, owner, amount,
anchor, and root all agree.

Negative proof vectors are first-class now. The repo has fixtures for wrong
genesis, wrong anchor, stale proof, malformed `TAPF` proof-file transport,
invalid split sums, wrong owner script key, missing STXO, wrong asset type,
wrong amount, wrong root hash, wrong root sum, mismatched TapCommitment output
root, and reorg-sensitive history. The next useful expansion is to keep
threading those vectors into close, sweep, recovery, and CI verification
boundaries as those surfaces move from bounded demo behavior to production
readiness.

Proof replay is now connected to wallet balances, proof export, channel
funding, commitment updates, cooperative close, bounded unilateral recovery,
second-level HTLC recovery, final sweep recovery, and bounded anchor-state
policy. The #96 proof-engine hardening sequence now has local and CI wiring;
live chain-watcher integration remains a production boundary above the current
synthetic regtest policy. The
already-wired funding path proves that the asset input being locked into the
channel is spendable, owned by the expected key, tied to the expected asset ID
and genesis, and conserved into the channel allocation. The already-wired
commitment path consumes the previous channel-locked proof-history output and
records the new channel-locked output for the latest commitment. The
close/recovery path now ties exported close proofs and recovered unilateral
outputs to replayed closed or swept proof states.

The matching models are `asset_channel`, `asset_commitment`, and
`asset_conservation`. They now make proof replay part of the checked state
rather than an external assumption. Funding requires accepted replay state,
commitment updates preserve total channel balance from the prior durable
state, and accepted restart requires matching proof replay and persisted
asset-channel state. These models do not know Bitcoin script semantics; they
prove the narrower wallet contract that funding, commitment, rejection, and
restart do not create or forget asset balance.

The fourth implementation step, close and sweep replay, is now covered for the
bounded demo surfaces. Cooperative close exports proof material tied to actual
local and remote close outputs. Unilateral commitment recovery, second-level
HTLC recovery, and final sweep recovery preserve proof-history output metadata
for the recovered spend kind. A failed sweep is not reported as recovered.
The live remaining work is wiring the same records through real
channel-manager, resolver, and sweeper call sites.

The formal companion is `close_recovery`. It now models close output,
second-level output, sweep attempt, sweep success, sweep failure, proof export,
and restart. Its key invariant is that close and sweep do not create, destroy,
or reassign assets outside the modeled recovery path, and that proof export
references either the cooperative close output or final sweep output rather
than an obsolete commitment view.

The fifth implementation step, reorg-sensitive history, is now implemented at
the bounded replay and wallet-policy layer. Production proof acceptance needs a
chain-state boundary, and the current code now has one: an anchor can be
unknown, pending, confirmed, stale, or reorged. Confirmed anchors satisfy the
default wallet policy. Pending anchors remain explicit and unspendable by
default. Stale and reorged anchors move dependent wallet records to rejected
state, and balance/export calls cannot use them. A confirmed replacement proof
path can recover accepted state. The remaining production work is connecting
this policy to a live chain watcher instead of only synthetic regtest/demo
updates.

The formal model keeps reorg handling bounded. It does not attempt to prove
Bitcoin consensus. It now models unknown, pending, confirmed, stale, and
reorged anchors and proves that accepted balances can only depend on anchors
that satisfy the configured policy. Reorged and stale anchors cannot be
accepted, and pending anchors remain explicit rather than silently counted as
spendable.

The sixth implementation step, Rust-native verification, is now in place as
an explicit harness layer. `proptest` coverage checks proof-history amount
conservation, split inflation rejection, proof graph topology, anchor-policy
acceptance, wallet restart and reorg-state behavior, and RFQ/invoice binding.
The fuzz target set covers TLV parsing, `TAPF` proof files, virtual PSBT
summaries, Taproot commitment leaf parsing, and imported Lightning Labs
funding/HTLC/commitment blobs. Kani harnesses cover pure helpers for
`AssetAmount` checked arithmetic, strict and pending anchor policy, and
proof-state transition input rules. `Miri` remains unwired because the current
proof-engine code does not use unsafe Rust, and `loom` remains unwired because
the proof-engine state does not yet use shared concurrent mutation.

The seventh implementation step, developer and CI wiring, is now in place.
`scripts/formal-check.sh` discovers checked-in TLA+ configs and skips cleanly
when no checker is installed. `scripts/rust-verification-check.sh` runs the
property tests and visibly skips optional fuzz or Kani checks when those tools
are not installed. `scripts/proof-engine-check.sh` is the local umbrella
command: it runs formatting, `cargo test --locked`, formal checks,
Rust-native verification, and the native demo by default, and it runs the BOLT
simple-taproot scripts plus the compatibility demo when
`TAP_LDK_EXTENDED_CHECKS=1` is set. `.github/workflows/proof-engine.yml` wires
the same normal suite into GitHub Actions for pushes and pull requests, with a
manual extended workflow for the heavier BOLT and compatibility checks.

The eighth implementation step is documentation and issue hygiene. That work
is now represented by the #96 through #106 sequence rather than being buried
under the closed first-demo issues. The sequence tracks the proof replay
engine, negative vectors, proof-validation TLA+ model, channel-funding proof
replay, commitment proof replay, close/sweep proof replay, reorg-sensitive
policy, property/fuzz/Kani harnesses, CI integration, and compatibility-matrix
updates. The remaining open production items should be filed as new issues
rather than reopening the first-demo closure scope.

The current exit bar is precise. The repo can now claim bounded proof-history
replay and formal-verification wiring for the wallet, funding, commitment,
close, recovery, and anchor-policy surfaces that have been implemented. It
still cannot claim production-complete Taproot Assets proof handling until
network proof discovery, live chain-watcher integration, grouped assets,
STXO/split/change history, and live close/force-close/sweep proof recovery are
implemented and verified.
