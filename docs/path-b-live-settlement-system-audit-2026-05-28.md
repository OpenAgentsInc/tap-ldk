# Path B Live Settlement System Audit

Date: 2026-05-28

This audit is the current working map for getting Path B from "funded but not
settled" to a completed live Lightning Labs interop demo. It exists because the
last several fixes were useful but too local: they moved the harness from peer
readiness to funding, then from funding to payment-time HTLC delivery, then
past the first HTLC signature mismatch, through monitor update completion, and
to an outgoing local commitment response. The live payment now requires the
native runtime to produce the exact Lightning Labs-compatible outgoing HTLC
signature, claim the incoming HTLC, and record real asset balances.

The current failure is not Docker, not `litd` startup, not peer connection, not
feature negotiation, not funding, not signature encoding, not the basic anchor
HTLC sighash policy, not the monitor/message-release boundary, and no longer
payment settlement. The previous peer-signature transcript mismatch is
fixture-backed, the virtual-lock and exact previous-output-bound HTLC aux-leaf
fixes get the live run through `commitment_signed`, and fork-backed `ldk-node`
now records the native receiver-side Taproot Asset balance. The active blocker
from the latest completed live run is post-success: after native claim and
fulfill, Lightning Labs force-closes with `invalid commitment`. The claimed
full-amount HTLC balance-output fix did not remove that rejection. The on-chain
fallback is also still broken: native LDK races a local commitment broadcast
and then the HTLC claim against the counterparty commitment fails with
`Invalid Taproot control block size`.

## Current Update

Latest live artifact:
`target/live-lightning-labs-outgoing-payment-claimed-balance-output/`.

Observed state:

- `litd` asset keysend status: `completed`
- LND payment wire status: `SUCCEEDED`
- native receiver event: `Received payment from payment hash ...`
- native claim event: `Claimed payment with ID ...`
- fork-backed `ldk-node` native asset payment status: `settled`
- native receiver local asset balance after claim: `125`
- native receiver remote asset balance after claim: `0`
- remaining failure: `CounterpartyForceClosed` with peer message
  `invalid commitment`
- on-chain fallback still needs work: native LDK attempts a local commitment
  and HTLC-success spend, the local commitment loses to the counterparty
  commitment, and the later HTLC claim against the counterparty commitment
  fails with `Invalid Taproot control block size`

## Current Live Gate

Previous diagnostic run:

- artifact directory:
  `target/live-lightning-labs-outgoing-payment-diagnostic/`
- `OpenAgentsInc/rust-lightning`:
  `85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1`
- `OpenAgentsInc/ldk-node`:
  `c5ae040bf84225922c5213d9acb077e031076a9c`
- live report status: `blocked`
- blocked step: `live_asset_channel_payment_settlement`
- `litd` asset channel fund status: `completed`
- `litd` reports channel usable for keysend: `true`
- `litd` local asset balance before keysend: `125`
- asset keysend status: `timed_out`
- LND payment wire status: `IN_FLIGHT`
- Rust Lightning close reason:
  `Invalid simple-taproot HTLC signature from peer`
- force-close broadcast also fails later with:
  `Invalid Taproot control block size`

What this proves:

- the OpenAgentsInc `ldk-node` fork is being used by the live harness;
- that fork is pinned to the OpenAgentsInc `rust-lightning` fork;
- the native node connects to the independent integrated `litd` peer;
- `litd` advertises simple-taproot and Taproot Asset channel support;
- `litd` mints the demo asset;
- `litd fundchannel` completes against the native LDK peer;
- the channel reaches `channel_ready`;
- the first live asset keysend reaches Rust Lightning as an `UpdateAddHTLC`;
- Rust Lightning decodes the Taproot Asset HTLC blob and the
  `commitment_signed` asset-signature blob structurally;
- Rust Lightning now treats the 64-byte HTLC signature as BIP340 Schnorr and
  uses `SIGHASH_SINGLE|ANYONECANPAY` with sequence `1` for the HTLC
  transaction.

What it does not prove:

- payment settlement;
- receiver-side native asset state;
- post-settlement Lightning Labs balance agreement;
- force-close recovery;
- semantic proof ancestry validation.

## Progress After This Audit

The current implementation has advanced beyond the diagnostic revision listed
above:

- `OpenAgentsInc/rust-lightning@c94f4570587e94e89740f5126a5fa70021b58de2`
  keeps the failing transcript as a regression fixture and preserves the trace
  details needed to compare the Rust and Lightning Labs HTLC signing views.
- `OpenAgentsInc/rust-lightning@5cee3fd83db4822eb7b05a5779aa4149d228238f`
  applies the current concrete fixes from this audit: second-level Taproot
  Asset HTLC aux leaves encode Lightning Labs virtual `lock_time` and
  `relative_lock_time` fields, full Taproot Asset counterparty commitments are
  persisted through monitor updates, outgoing HTLC signatures use exact
  previous-output-bound second-level aux leaves, and post-claim commitments
  move claimed full-amount asset HTLCs into the rightful balance output.
- `OpenAgentsInc/ldk-node@ce6319df7220aa39cd561fee50ea7115a0b7dd73` pins that
  Rust Lightning revision, and `tap-ldk` now consumes the same fork chain.

The latest completed live rerun before the exact-leaf pin produced:

- artifact directory:
  `target/live-lightning-labs-outgoing-payment-full-commitment/`
- `OpenAgentsInc/rust-lightning`:
  `acce215e1ca284fa45f1c13e13760de459d410d4`
- live report status: `blocked`
- blocked step: `live_asset_channel_payment_settlement`
- LND payment wire status: `IN_FLIGHT`
- Rust Lightning result: peer HTLC signature verified, `commitment_signed`
  accepted, monitor update `1` completed, and `revoke_and_ack` emitted.
- Lightning Labs result: rejected our outgoing HTLC signature with
  `invalid_htlc_sig`.

The newer claimed-balance-output rerun moved past that blocker:

- artifact directory:
  `target/live-lightning-labs-outgoing-payment-claimed-balance-output/`
- `OpenAgentsInc/rust-lightning`:
  `5cee3fd83db4822eb7b05a5779aa4149d228238f`
- `OpenAgentsInc/ldk-node`:
  `ce6319df7220aa39cd561fee50ea7115a0b7dd73`
- live report status: `blocked`
- blocked step: `live_asset_channel_payment_settlement`
- LND payment wire status: `SUCCEEDED`
- native receiver asset payment status: `settled`
- native receiver balance: `125`
- remaining post-claim error from `litd`: `invalid commitment`
- fallback error: `Invalid Taproot control block size` when spending the
  counterparty commitment HTLC output

This does not close #81. The claimed-balance-output fix was necessary but not
sufficient. The next honest gate is a fixture-backed comparison of the
post-claim remote commitment transaction, asset allocation, signature, and HTLC
claim witness/control block against the Lightning Labs transcript before
touching unrelated signing policy.

## Failing Transcript

The native side receives this live HTLC:

- channel id:
  `422c9bd5ba245dbec837043b36446a886f690a76f5d80b587f112c7a1343cf64`
- HTLC id: `0`
- amount: `354000 msat`
- CLTV: `218`
- payment hash:
  `9567b1917e38ac6a3f7bb4be862277bf1ef615bed9c20f41876c18933cbf8405`
- Taproot Asset HTLC blob:
  `012c0020c246de3d8fd2ac23f5608a6e787d6cd3601c2d62cdbc3640d1b0dbab084e1df50108000000000000007d`

Rust checks the peer HTLC signature:

- signature:
  `7b9b51b2c0f3b31a3404e53104205060ea2975f4337e982928ebabcc6d3cd7954c311f906f95c5b6c33897bb343c20f8e79f16abe791aab5f59c69e75681e6f2`
- verifying key:
  `03106bc567cdb1fa700acaff1a637dc21d5f8a4a075e4ca25234b5813db0f19c7b`
- HTLC transaction:
  `0200000001565900635bf3cfabb2e715d85b36a348eff38bd3880ed3ac034235926d53e5d502000000000100000001620100000000000022512095dc737bfa556a7ebcd7c6aa2851ec72775a4fa082f580d5d92eddb58dc24db100000000`
- output:
  `0:354:22512095dc737bfa556a7ebcd7c6aa2851ec72775a4fa082f580d5d92eddb58dc24db1`
- previous commitment output:
  `354:22512081c339b9042ea10fc4a2731bc4858c7c30fd522f8b0717b0b1b7fe0c9369ceb3`
- selected HTLC leaf:
  `5f82012088a9148928255a546cf82436ee513f08b488902c4e36d48820d06cf3f101da15d0743c5c6c42f7d2e86546a000aa7a35d152061c050825efdfad20106bc567cdb1fa700acaff1a637dc21d5f8a4a075e4ca25234b5813db0f19c7bac`
- first-level aux leaf:
  `496a47e8543f2ab163faf693971a653e54efe3d8056c52164c6b8bbf597b6ddb2802c7686af75a89232d922ca2f47ff29c7e5a6897480213ec8acb56d8b2b8f7e8cd000000000000007d`
- second-level aux leaf Rust derived:
  `496a47e8543f2ab163faf693971a653e54efe3d8056c52164c6b8bbf597b6ddb280288dbd45e53728ca954abfa2e522f44d22f6b74f5b7dd460bb800ee58ddc99876000000000000007d`
- computed sighash:
  `a03dc2c5816d1b158b5966351690f4e49b4e937cd90bfaaef974eb36665afa42`

Lightning Labs logs for the same payment-time second-level allocation show:

- internal key:
  `03770ea7c98b45d4fc078b2ec1024eb48490219d2a4bca3097cdc3f9827ebf9495`
- tweaked internal key:
  `0370e5d041406c7b40a8044ec1e38e1e1501c18c4b08d38b15842e6cd3fa06ed83`
- base second-level asset script key:
  `e0a9208bc65205adb7b192cc7515b946aeb98a3de0ae037e777d31a343ffda97`
- tweaked second-level asset script key:
  `f12638d93f43df7ce215f8669a3db27f37ab155b9591fc68f400655666815f96`
- asset commitment key:
  `cfc3784d78ad19d8fab58aa122f9f38f574620b278d75d2c290107733224d79c`
- asset commitment leaf:
  `03bd8afc62c03a029fbc32db9250772092d0dd0fb4fa2b6a067b5c3913886ea6`
- inclusion Taproot Asset root:
  `01b4fa40e88d8a63754ee74d0eaa72b1e6fd101d114590cbe7f51b444d78e8fd`
- output commitment root:
  `879e7097820ec7ed9f065c91d6482bdefaec33ed5d30f70fbf4cff5d5d7cf12f`
- virtual signing output script:
  `5120994993a3810a1990bcccf848e3d76a5e612312c56abc74d05903374d13c92c07`
- virtual signing output value: `125`
- asset-layer hash type: `131`

This is the exact shape of the bug: Rust is deriving a local no-split aux leaf
from a compressed view of the output key. Lightning Labs is deriving a full
Taproot Asset output commitment through allocation, virtual packet, output
commitment, and assignment steps, with HTLC-index script-key tweaks.

## Cross-Repo Runtime Path

### `tap-ldk`

Primary live command:

- `scripts/live-lightning-labs-outgoing-payment.sh`

Responsibilities:

- create and clean artifact paths;
- run bounded outgoing-payment fixtures;
- run native asset payment-session smoke;
- bind live `tapd` proof data;
- start integrated Lightning Labs `litd`;
- mint a live asset;
- start the fork-backed native LDK peer hold process;
- let `litd` fund an asset channel to that peer;
- wait until `litd` says the channel is keysend-usable;
- ask `litd` to send the asset keysend;
- write the consolidated Path B report.

Important point: this script is now doing real interop. It is not the core
bug. It should not be patched around the signature failure except to improve
diagnostics and acceptance reporting.

Primary native peer wrapper:

- `crates/tap-ldk-core/src/live_litd_peer.rs`

Responsibilities:

- build `ldk_node::Node` with regtest bitcoind RPC;
- enable `ExperimentalChannelConfig::taproot_assets_regtest()`;
- set filesystem logging so Rust Lightning traces can be inspected;
- connect to the independent `litd` peer;
- report provenance, peer feature bits, and experimental runtime API reach.

Important point: this wrapper proves the fork-backed runtime is connected. It
does not yet expose a durable receiver asset-balance API for the live payment.
That must come after the commitment transcript is correct.

Planning and status files:

- `README.md`
- `ROADMAP.md`
- `ARCHITECTURE.md`
- `INVARIANTS.md`
- `docs/remaining-issue-closure-plan.md`
- `docs/path-b-live-settlement-holistic-audit.md`
- `docs/path-b-live-settlement-diagnostic-run-2026-05-28.md`

Responsibilities:

- keep the public state honest;
- prevent fixture-backed expected balances from being described as live
  balances;
- keep #81, #57, #58, #59, #60, #61, #71, and #19 in the correct closure
  order.

### OpenAgentsInc `ldk-node`

Primary files:

- `Cargo.toml`
- `src/config.rs`
- `src/builder.rs`
- `src/provenance.rs`
- `src/taproot_asset.rs`

Responsibilities:

- pin the OpenAgentsInc `rust-lightning` fork;
- keep BTC-only defaults unchanged;
- expose explicit opt-in simple-taproot and Taproot Asset channel flags;
- fail build config when Taproot Asset channels are enabled without
  simple-taproot channels;
- create the `TaprootAssetManager` and custom message handler;
- parse live Lightning Labs `AssetFundingCreated`;
- bind pending channel template and initial aux leaves into Rust Lightning;
- expose bounded asset message/channel/payment APIs used by `tap-ldk`;
- report provenance so the harness can fail if it is not running the fork.

Important point: `ldk-node` is now part of the active blocker because the live
run stalls at the async monitor persistence/message-release boundary. Keep
protocol transcript construction in Rust Lightning, but audit the `ldk-node`
persister/background-processor wiring before making another signature patch.
Do not fake settlement here.

### OpenAgentsInc `rust-lightning`

Primary files:

- `lightning/src/ln/channel.rs`
- `lightning/src/ln/chan_utils.rs`
- `lightning/src/ln/simple_taproot.rs`
- `lightning/src/ln/taproot_asset.rs`
- `lightning/src/ln/msgs.rs`
- `lightning/src/ln/peer_handler.rs`
- `lightning/src/chain/channelmonitor.rs`

Current relevant behavior:

- `channel.rs` carries Taproot Asset HTLC blobs in pending HTLC state;
- `channel.rs` builds commitment transactions with optional simple-taproot
  asset aux leaves;
- `channel.rs` verifies peer HTLC signatures as Schnorr for simple-taproot;
- `channel.rs` has helpers named
  `taproot_asset_htlc_asset_script_key`,
  `taproot_asset_htlc_aux_leaf_script`,
  `taproot_asset_second_level_htlc_aux_leaf_script_for_commitment_output`,
  and `taproot_asset_second_level_htlc_aux_leaf_script`;
- `taproot_asset.rs` decodes the live HTLC blob;
- `taproot_asset.rs` decodes Lightning Labs `commitment_signed` asset
  signature blobs;
- `taproot_asset.rs` decodes Lightning Labs commitment aux-leaf blobs;
- `taproot_asset.rs` has `derive_no_split_taproot_asset_aux_leaf_script`,
  which is a bounded local helper, not a port of the Lightning Labs allocation
  pipeline;
- `simple_taproot.rs` constructs base simple-taproot HTLC scripts, second-level
  scripts, tapscript roots, control blocks, and BIP342 HTLC sighashes.

Current core mismatch:

- Rust computes the asset script key by turning a locally selected P2TR output
  key into a compressed even key and feeding that into the no-split helper.
- Lightning Labs computes asset script keys from the base simple-taproot HTLC
  script tree, then tweaks the asset-level internal key by `htlc_index + 1`;
  the Bitcoin-level HTLC output is not tweaked that way.
- Rust uses a compressed no-split template path for second-level HTLC aux leaf
  derivation.
- Lightning Labs creates second-level HTLC allocations, distributes coins,
  prepares output assets, creates output commitments, assigns them back to the
  allocation, and uses the allocation's `AuxLeaf()`.

This must be fixed in Rust Lightning first. The exact allocation transcript is
part of the channel state machine, not a CLI or harness concern.

### Lightning Labs References

Primary files:

- `projects/lightninglabs/repos/taproot-assets/tapchannel/aux_leaf_signer.go`
- `projects/lightninglabs/repos/taproot-assets/tapchannel/commitment.go`
- `projects/lightninglabs/repos/taproot-assets/tapchannel/aux_leaf_creator.go`
- `projects/lightninglabs/repos/taproot-assets/tapsend/allocation.go`
- `projects/lightninglabs/repos/taproot-assets/tapsend/send.go`
- LND module files under the local Go module cache:
  `lnwallet/channel.go`, `lnwallet/transactions.go`,
  `lnwallet/commitment.go`, and `input/script_utils.go`

Relevant Lightning Labs behavior:

- `ScriptKeyTweakFromHtlcIndex(index)` maps `max_u64` to `1`, otherwise
  uses `index + 1`, encoded as a secp256k1 scalar.
- `TweakHtlcTree(tree, index)` tweaks the asset-level internal key only:
  `internal_key + (index + 1) * G`; it keeps the tapscript root unchanged and
  recomputes the asset-level Taproot key.
- `CreateAllocations` builds base HTLC scripts with no aux leaf, extracts the
  non-asset sibling/tree, applies the HTLC-index tweak only to the asset-level
  script key, and uses the untweaked Taproot key for sorting.
- `createSecondLevelHtlcAllocations` does the same for the second-level HTLC
  script.
- `CreateSecondLevelHtlcPackets` builds virtual packets from the parent HTLC
  asset outputs and allocation.
- `CreateSecondLevelHtlcTx` calls `tapsend.CreateOutputCommitments`,
  `tapsend.AssignOutputCommitments`, then `allocations[0].AuxLeaf()`.
- `tapsend.CreateOutputCommitments` builds the Taproot Asset output
  commitment from prepared virtual outputs and trims split witnesses.
- `Allocation.FinalPkScript` computes the final Bitcoin output script from the
  allocation internal key, tapscript sibling, and output commitment root.
- LND signs the second-level HTLC transaction using anchor-style
  `SIGHASH_SINGLE|ANYONECANPAY`, input sequence `1`, and
  `TaprootScriptSpendSignMethod`.

## What Not To Do

- Do not bypass signature verification.
- Do not accept the peer signature by trying multiple scripts or keys until
  one passes.
- Do not mark the payment settled from `litd`'s `IN_FLIGHT` status.
- Do not move settlement accounting into `tap-ldk` or `ldk-node` without the
  Rust Lightning commitment transcript being correct.
- Do not depend on `tapd`, `litd`, or LND as a sidecar inside the wallet.
- Do not close #81, #57, #58, #59, #60, #61, #71, or #19 from fixture-only
  reports.
- Do not weaken runtime fail-closed policy to make a live harness report
  better.

## Required Implementation Plan

### Phase 1: Make The Failure A Regression Fixture

Add a deterministic Rust Lightning test fixture for the 2026-05-28 payment
transcript.

The fixture should preserve:

- HTLC blob;
- HTLC id, amount, CLTV, payment hash;
- peer signature and verifying key;
- HTLC transaction bytes;
- previous output value and script;
- selected leaf and control block;
- first-level aux leaf;
- Rust-derived second-level aux leaf;
- Lightning Labs second-level script-key allocation data;
- Lightning Labs output commitment root and asset commitment leaf;
- Rust-computed sighash.

Acceptance for this phase:

- a normal test must pass without Docker;
- it must assert the current mismatch explicitly;
- it must document which value is wrong and which Lightning Labs value is the
  target;
- it must not be an ignored failing test.

### Phase 2: Port The Bounded Lightning Labs Allocation Semantics

Add Rust helpers in `lightning/src/ln/taproot_asset.rs` and/or
`lightning/src/ln/simple_taproot.rs` for the single-asset, one-HTLC path:

- HTLC-index scalar tweak with the Lightning Labs `index + 1` rule;
- asset-level internal-key tweak without changing the Bitcoin-level HTLC
  output;
- base first-level HTLC script-tree material with no asset aux leaf;
- base second-level HTLC script-tree material with no asset aux leaf;
- non-asset sibling preimage for the aux-leaf commitment;
- asset script key from the tweaked asset-level tree;
- output commitment root and Taproot Asset aux leaf construction using the
  same key material and amount semantics as Lightning Labs.

Acceptance for this phase:

- a fixture test must derive the Lightning Labs observed tweaked second-level
  asset script key
  `f12638d93f43df7ce215f8669a3db27f37ab155b9591fc68f400655666815f96`;
- a fixture test must derive the observed output commitment root
  `879e7097820ec7ed9f065c91d6482bdefaec33ed5d30f70fbf4cff5d5d7cf12f`
  or document the precise missing input if the root cannot be derived yet;
- no normal BTC simple-taproot test may regress.

### Phase 3: Wire The Allocation Model Into Commitment And HTLC Signing

Replace the no-split approximation inside:

- `taproot_asset_htlc_aux_leaf_script`;
- `taproot_asset_second_level_htlc_aux_leaf_script`;
- `taproot_asset_second_level_htlc_aux_leaf_script_for_commitment_output`.

The replacement must use the exact allocation model when a live Taproot Asset
HTLC blob and proof-derived channel template exist. If required inputs are
missing, it must fail closed rather than returning a plausible placeholder.

Acceptance for this phase:

- Rust Lightning verifies the same payment-time HTLC signature that `litd`
  sent in the fixture;
- the live harness no longer closes on
  `Invalid simple-taproot HTLC signature from peer`;
- the live keysend progresses past the first payment-time commitment update.

Status: the virtual-lock and full-commitment reruns have satisfied the
signature-verification part of this phase and progressed to a valid
`commitment_signed`, monitor update completion, and held-message release. The
keysend has not progressed past the outgoing HTLC signature accepted by
Lightning Labs.

### Phase 3B: Complete Monitor Update And Release Held Commitment Messages

Status: implemented in
`OpenAgentsInc/rust-lightning@acce215e1ca284fa45f1c13e13760de459d410d4`.
The full-commitment rerun logs monitor update `1` completion and
`revoke_and_ack` emission. Keep the BTC-only behavior checks in the regression
set.

The runtime/protocol boundary requirements were:

- inspect `ldk-node`'s async monitor persister and background processor setup;
- inspect Rust Lightning's `ChannelMonitorUpdateStatus::InProgress` completion
  path for payment-time updates on simple-taproot/Taproot Asset channels;
- add logging or tests that prove update id `1` returns a completion event;
- ensure `ChannelManager` receives the monitor completion event and releases
  held `revoke_and_ack` and local `commitment_signed` messages;
- keep persistence fail-closed if the monitor update cannot be durably stored;
- rerun the live harness and confirm `litd` no longer leaves the payment
  stuck at the first commitment exchange.

Acceptance for this phase:

- the live log contains completion for monitor update `1`;
- held commitment messages are emitted after completion;
- `litd` progresses beyond `IN_FLIGHT` or exposes the next receiver-claim
  blocker with a concrete transcript;
- BTC-only monitor update behavior remains unchanged.

### Phase 3C: Produce Lightning Labs-Compatible Outgoing HTLC Signatures

An earlier live run failed after Phase 3B when `litd` rejected our outgoing
HTLC signature. The current pin fixed that path by rewriting nondust HTLC state
to the exact previous-output-bound Taproot Asset second-level aux leaf after
commitment txid/output index assignment, and it also moves claimed full-amount
asset HTLCs into the correct post-claim balance output.

Acceptance for this phase:

- completed for the latest `5cee3fd83...` rerun: the live log no longer
  contains `invalid_htlc_sig` for our outgoing HTLC signature;
- the next fixture is now the post-claim `invalid commitment` transcript, not
  the outgoing HTLC signature transcript;
- BTC-only and fixture-backed Taproot Asset tests remain unchanged.

### Phase 4: Fix Post-Claim Force-Close And HTLC-Success Fallback

After the successful Lightning Labs to native settlement, fix the current
post-claim unilateral recovery failure:

- compare the native post-claim commitment/fulfill transcript against the
  Lightning Labs expectation that currently returns `invalid commitment`;
- inspect control-block construction for HTLC outputs with asset aux siblings;
- ensure the control block matches the selected script-path leaf and tree;
- persist enough HTLC aux-leaf state through monitor serialization;
- add fixture tests for control-block length, HTLC-success witness content,
  feerate/RBF behavior, and spend path;
- rerun the live force-close path.

Acceptance for this phase:

- the post-claim live transcript does not force-close with `invalid commitment`;
- if a force-close happens, HTLC-success fallback broadcasts cleanly and does
  not fail with missing/spent inputs, insufficient-fee replacement, or control
  block errors;
- monitor restart can reconstruct the same control-block data;
- BTC-only force-close tests still pass.

### Phase 5: Complete Balance Reporting In Both Directions

The Lightning Labs to native direction now records native receiver balance. The
remaining balance work is to wire the reverse direction and tighten reports:

- query Lightning Labs post-settlement balance for the true native-to-Lightning
  Labs payment direction;
- update Path B reports so #57 and #58 only pass with observed balances;
- keep negative checks for wrong quote, wrong asset, wrong amount, stale proof,
  missing metadata, and restart recovery.

Acceptance for this phase:

- #57 can report `issue_57_acceptance_met=true`;
- #58 can report durable native receiver balance after restart;
- #59 can require observed live balances instead of expected-only balances;
- #81 can close only after this phase passes.

### Phase 6: Semantic Proof Validation

After live settlement, #60 still needs the full proof-ancestry boundary:

- asset identity and genesis validation;
- group key validation when present;
- anchor and TapCommitment root validation;
- virtual transaction and previous-witness validation;
- amount conservation across split/transfer/channel states;
- failure fixtures for wrong asset, wrong proof, wrong anchor, and stale
  ancestry.

This is intentionally after live transcript matching because the current
blocker is not proof parsing; it is payment-time commitment construction.

## File-Level Next Edits

Immediate edits should start in the Rust Lightning fork:

- add transcript constants and tests near the existing live `litd` tests in
  `lightning/src/ln/taproot_asset.rs`;
- add an internal helper for the Lightning Labs HTLC-index tweak;
- expose enough base simple-taproot HTLC/second-level tree data from
  `lightning/src/ln/simple_taproot.rs` to avoid reconstructing the wrong
  internal key in `channel.rs`;
- replace the local P2TR-output-key based script-key derivation in
  `lightning/src/ln/channel.rs` after the helper has fixture coverage.

Then pin upward:

- update `ldk-node/Cargo.toml` and `src/provenance.rs`;
- run `ldk-node` focused tests;
- update `tap-ldk/Cargo.toml`, `Cargo.lock`, docs, and provenance checks;
- rerun `tap-ldk` focused tests;
- rerun the live harness.

## Verification Ladder

Rust Lightning:

```bash
cargo fmt --check
cargo test -p lightning taproot_asset --features simple_taproot_musig2 -- --nocapture
cargo test -p lightning simple_taproot --features simple_taproot_musig2 -- --nocapture
cargo check -p lightning --features simple_taproot_musig2
git diff --check
```

`ldk-node`:

```bash
cargo fmt --check
cargo test --locked provenance -- --nocapture
cargo test --locked taproot_asset -- --nocapture
git diff --check
```

`tap-ldk`:

```bash
cargo fmt --check
./scripts/check-openagents-rust-lightning.sh
cargo test --locked ldk_fork -- --nocapture
cargo test --locked live_litd -- --nocapture
git diff --check
```

Live gate:

```bash
TAP_LDK_LL_LITD_LND_DEBUG_LEVEL=trace \
TAP_LDK_LL_LITD_TAPROOT_ASSETS_DEBUG_LEVEL=trace \
TAP_LDK_LIVE_LITD_LDK_LOG_LEVEL=trace \
TAP_LDK_LL_WAIT_TIMEOUT_SECONDS=600 \
TAP_LDK_LL_CONTAINER_RUN_TIMEOUT_SECONDS=180 \
./scripts/live-lightning-labs-outgoing-payment.sh \
  target/live-lightning-labs-outgoing-payment-diagnostic/report.json \
  target/live-lightning-labs-outgoing-payment-diagnostic/wallet.json
```

## Issue Closure Rules

Current open issue order remains:

1. #81: fork-backed live Lightning Labs settlement;
2. #57: live `tap-ldk` pays Lightning Labs;
3. #58: live Lightning Labs pays `tap-ldk`;
4. #59: observed live balance reporting;
5. #60: semantic proof ancestry validation;
6. #61: BOLT simple-taproot LDK epic;
7. #71: full Taproot Assets LDK epic;
8. #19: Path B Lightning Labs interop epic.

Do not close #81 until the live harness settles and records observed balances.
Do not close #57 or #58 until their direction-specific live balance checks are
real. Do not close #59 until expected-only fields cannot satisfy Path B. Do
not close #61, #71, or #19 while any concrete child issue above remains open.
