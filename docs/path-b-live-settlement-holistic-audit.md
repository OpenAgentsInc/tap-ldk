# Path B Live Settlement Holistic Audit

Date: 2026-05-28

This audit records the current root cause for the live Lightning Labs Path B
blocker and the complete work path needed to stop making one-off settlement
patches. The current failure is not a Docker, peer-connection, funding, or
basic feature-negotiation problem. The live path now reaches a real
Lightning Labs `litd` asset channel and attempts a real asset keysend. It
fails because the Rust implementation is still approximating parts of the
Lightning Labs Taproot Asset commitment and second-level HTLC transcript.

## Current Live Result

Latest live run artifact:

- `target/live-lightning-labs-outgoing-payment-anchor/report.json`
- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `openagents_rust_lightning_rev`:
  `4761230b3d8a2732d379087a5510456a13b86c29`
- `live_node_runtime`: `ldk-node 0.8.0+git`
  from `https://github.com/OpenAgentsInc/ldk-node`
- `integrated_litd_asset_channel_fund_status`: `completed`
- `integrated_litd_asset_channel_usable_for_keysend`: `true`
- `integrated_litd_asset_channel_local_balance`: `125`
- `integrated_litd_asset_payment_status`: `timed_out`
- `integrated_litd_asset_payment_wire_status`: `IN_FLIGHT`
- `integrated_litd_post_payment_balance`: `999875`
- `asset_channel_settlement_ready`: `false`

The important fact is that funding succeeds and `litd` believes the channel is
usable for asset keysend. The first payment-time commitment update reaches
Rust Lightning, but Rust Lightning rejects the peer HTLC Schnorr signature:

```text
Invalid simple-taproot HTLC signature from peer
```

After that close, the force-close path also exposes a separate witness
construction problem:

```text
Invalid Taproot control block size
```

These are distinct blockers. The signature mismatch prevents live settlement;
the control-block failure prevents correct unilateral recovery once the
payment-time commitment path gets farther.

## What Works

- `tap-ldk` consumes the OpenAgentsInc `ldk-node` fork.
- `ldk-node` consumes the OpenAgentsInc `rust-lightning` fork.
- All `lightning*` packages in `tap-ldk` resolve to
  `OpenAgentsInc/rust-lightning@4761230b3d8a2732d379087a5510456a13b86c29`.
- The live harness starts an integrated Lightning Labs `litd` counterparty.
- The native LDK node connects to `litd`.
- The peer feature path observes simple-taproot and Taproot Asset channel
  support.
- `litd` issues a real regtest asset.
- `litd` completes `fundchannel` with the fork-backed native LDK peer.
- The asset channel reaches `channel_ready`.
- `litd` reports a keysend-usable asset balance.
- Rust Lightning receives the live payment-time `UpdateAddHTLC`.
- Rust Lightning decodes the live Taproot Asset HTLC blob and
  `commitment_signed` asset-signature blob structurally.
- Rust Lightning verifies the peer HTLC signature as BIP340 Schnorr bytes,
  not as an ECDSA wrapper.
- The latest rust-lightning pin treats simple-taproot HTLC second-level
  signing like the Lightning Labs anchor path:
  `SIGHASH_SINGLE|ANYONECANPAY` and input sequence `1`.

## What Is Still Broken

The current Rust path is still not deriving the exact same payment-time HTLC
view that Lightning Labs signs. The anchor-style sighash and sequence fix was
necessary, but not sufficient.

The current mismatch is likely in one or more of these transcript inputs:

- second-level HTLC output script;
- second-level Taproot Asset aux leaf;
- HTLC taproot spend info and control block;
- output value or output script selected for the HTLC signature;
- script-tree construction order once the Taproot Asset sibling is present;
- exact Lightning Labs allocation and commitment construction for the
  payment-time asset movement.

The latest native log shows Rust Lightning now checks the peer signature
against a v2 HTLC transaction with input sequence `1`, output value `354`
sats, `SIGHASH_SINGLE|ANYONECANPAY`, and the expected offered/accepted HTLC
leaf script. Lightning Labs logs show `litd` builds Taproot Asset allocations
for both first-level and second-level HTLC outputs and tweaks asset-layer
script keys before producing the aux leaves. Rust still reconstructs this from
a bounded single-asset/no-split template rather than porting the exact
Lightning Labs allocation path.

## Relevant Files Audited

### `tap-ldk`

- `README.md`
- `ROADMAP.md`
- `ARCHITECTURE.md`
- `INVARIANTS.md`
- `docs/remaining-issue-closure-plan.md`
- `docs/openagents-rust-lightning-fork.md`
- `docs/openagents-ldk-node-fork.md`
- `docs/live-litd-peer-preflight.md`
- `docs/lightning-labs-litd-counterparty.md`
- `docs/lightning-labs-outgoing-payment.md`
- `docs/path-b-lightning-labs-demo.md`
- `scripts/live-lightning-labs-outgoing-payment.sh`
- `scripts/lightning-labs-litd-counterparty.sh`
- `scripts/check-openagents-rust-lightning.sh`
- `crates/tap-ldk-core/src/live_litd_peer.rs`
- `crates/tap-ldk-core/src/lightning_labs_blob.rs`
- `crates/tap-ldk-core/src/lightning_labs_payment.rs`
- `crates/tap-ldk-core/src/lightning_labs_interop_checks.rs`
- `crates/tap-ldk-core/src/ldk_fork.rs`

### OpenAgentsInc `ldk-node`

- `Cargo.toml`
- `Cargo.lock`
- `src/builder.rs`
- `src/config.rs`
- `src/provenance.rs`
- `src/taproot_asset.rs`

Important live surfaces:

- asset opt-in config;
- provenance reporting;
- simple-taproot plus Taproot Asset feature negotiation;
- custom asset message API;
- asset-channel open API;
- asset-payment API;
- pending simple-taproot tapscript root;
- pending Taproot Asset channel template;
- pending simple-taproot asset output keys;
- pending commitment aux leaves.

### OpenAgentsInc `rust-lightning`

- `lightning/src/ln/channel.rs`
- `lightning/src/ln/chan_utils.rs`
- `lightning/src/ln/simple_taproot.rs`
- `lightning/src/ln/taproot_asset.rs`
- `lightning/src/ln/msgs.rs`
- `lightning/src/ln/peer_handler.rs`
- `lightning/src/chain/channelmonitor.rs`

Important live surfaces:

- `simple_taproot_validate_commitment_partial`;
- HTLC transaction construction through `build_htlc_transaction`;
- simple-taproot HTLC signature verification;
- BIP340 Schnorr handling;
- Taproot Asset HTLC blob decode;
- Taproot Asset `commitment_signed` blob decode;
- pending channel template persistence;
- pending aux leaf persistence;
- second-level HTLC aux leaf derivation;
- force-close witness/control-block reconstruction.

### Lightning Labs LND Module

Local module path audited:

- `github.com/lightningnetwork/lnd@v0.20.0-beta.rc4.0.20260421084739-a8a3e13120eb`

Relevant files:

- `lnwallet/channel.go`
- `lnwallet/transactions.go`
- `lnwallet/commitment.go`
- `input/script_utils.go`

Important source behavior:

- `genRemoteHtlcSigJobs` signs HTLC transactions with
  `CreateHtlcSuccessTx` or `CreateHtlcTimeoutTx`;
- the signature path uses `CommitAuxLeaves` for the second-level leaf;
- `HtlcSigHashType(chanType)` returns
  `SigHashSingle|SigHashAnyOneCanPay` for anchor-like channels;
- `HtlcSecondLevelInputSequence(chanType)` returns `1` for anchor-like
  channels;
- Taproot HTLC signing uses `TaprootScriptSpendSignMethod`;
- staging simple-taproot second-level scripts differ from final/prod scripts.

### Lightning Labs `taproot-assets`

Reference repo audited:

- `projects/lightninglabs/repos/taproot-assets`

Relevant files:

- `tapchannel/aux_leaf_creator.go`
- `tapchannel/commitment.go`
- `tapsend/allocation.go`
- `tapsend/send.go`

Important source behavior:

- `FetchLeavesFromView` and `FetchLeavesFromCommit` are the source of the
  Taproot Asset aux leaves LND uses for channel commitments.
- `GenerateCommitmentAllocations` flows through `ComputeView`,
  `CreateAllocations`, `tapsend.DistributeCoins`,
  `signCommitVirtualPackets`, `tapsend.CreateOutputCommitments`,
  `tapsend.AssignOutputCommitments`, and proof suffix generation.
- `CreateAllocations` first builds the base Lightning simple-taproot HTLC
  script without the aux leaf, extracts the sibling/tree, then creates
  asset-layer allocations and tweaked asset script keys.
- `CreateSecondLevelHtlcTx` rebuilds the second-level asset commitment output
  and returns the aux leaf used when LND signs the HTLC transaction.

## Root Cause

The current native implementation has enough local pieces to reach the live
payment path, but it does not yet have an exact Rust implementation of the
Lightning Labs Taproot Asset channel allocation and output commitment pipeline.

The fork currently derives a bounded full-channel HTLC aux leaf from local
single-asset template data. That was useful to get past earlier open/funding
failures, but it is not an adequate source of truth for live payment-time HTLC
signing. Lightning Labs signs a transcript produced by its `tapchannel` and
`tapsend` allocation machinery. Rust must build the same transcript byte for
byte before the peer HTLC signature can verify.

The right fix is not another isolated change to a sighash flag, output value,
or key order. The right fix is to port and test the relevant Lightning Labs
Taproot Asset commitment semantics into Rust, then wire that through
rust-lightning and ldk-node as the only accepted Path B settlement path.

## Required Work

### Phase 1: Capture The Exact Transcript

Add deterministic diagnostics and a fixture from the latest live run so the
Rust side records every input to the rejected HTLC signature check:

- commitment txid and HTLC output index;
- previous output amount and script;
- HTLC transaction bytes;
- all HTLC transaction outputs;
- selected tapleaf script;
- tapleaf hash;
- sighash type;
- computed BIP341 sighash;
- verifying public key;
- peer signature bytes;
- Taproot Asset HTLC blob bytes;
- Taproot Asset `commitment_signed` blob bytes;
- first-level and second-level aux leaf bytes when available;
- control-block bytes used for force-close attempts.

The acceptance test for this phase is not "payment settles"; it is a checked
transcript fixture that makes the current mismatch explicit and stable.

### Phase 2: Port Lightning Labs Allocation Semantics

Implement the bounded but exact single-asset subset of the Lightning Labs
`tapchannel` and `tapsend` pipeline in Rust:

- channel-view input model;
- asset allocation model;
- first-level HTLC allocation;
- second-level HTLC allocation;
- asset script-key tweak by HTLC index;
- output commitment creation;
- Taproot Asset aux leaf construction;
- proof suffix fields needed by the demo path;
- deterministic output ordering matching Lightning Labs.

This must replace the current full-channel/no-split approximation for live
Path B. The first scope can remain single-asset, single-HTLC, no-change-output
only, but it must be exact for that scope.

### Phase 3: Wire The Exact Model Through `rust-lightning`

Rust Lightning must use the exact model for:

- pending channel asset template state;
- commitment aux leaf generation;
- HTLC add/update validation;
- peer HTLC signature verification;
- local HTLC signature generation;
- channel monitor persistence;
- reestablish after restart;
- failure paths that must not advance stale asset state.

The verification path must keep failing closed when any required asset data is
missing, stale, malformed, or inconsistent with the Lightning commitment.

### Phase 4: Wire The Runtime Through `ldk-node`

`ldk-node` must expose the live runtime hooks without pretending the daemon
side is wallet logic:

- explicit experimental config;
- remote feature reporting;
- typed asset channel open request;
- typed asset payment request;
- event/provenance reporting;
- native receiver balance persistence;
- non-secret report artifacts for live runs.

`litd`, LND, and `tapd` stay independent interop peers, not sidecars.

### Phase 5: Fix Force-Close And Recovery

The current force-close attempt exposes an invalid Taproot control block. This
must be fixed after the payment-time transcript is exact enough to accept
peer HTLC signatures:

- preserve spend info for first-level and second-level outputs;
- rebuild the right control block for the selected leaf;
- verify script-path spends in tests;
- persist enough monitor data to recover after restart;
- export proof data only for the actual final spendable output.

### Phase 6: Settle Both Live Directions

Do not close #81, #57, #58, #59, #60, #61, #71, or #19 from fixture-only or
readiness reports.

Required live evidence before closure:

- `tap-ldk` pays Lightning Labs and the payment reaches a terminal successful
  state;
- Lightning Labs pays `tap-ldk` and the payment reaches a terminal successful
  state;
- both sides report the same asset ID and amount;
- both sides report compatible pre/post asset balances;
- native receiver state survives restart;
- proof references are non-secret and tied to the settled output;
- wrong-asset, wrong-amount, stale-quote, and malformed-blob cases still fail
  closed.

### Phase 7: Complete Proof Ancestry And Broader Protocol Work

After the bounded live payment path works, finish the remaining protocol
surface:

- full semantic proof ancestry validation;
- partial split and change-output support;
- STXO commitment leaves;
- multi-input same-asset funding where required;
- additional Taproot Asset proof-chain validation;
- broader property/fuzz/formal coverage for amount conservation,
  persistence, quote lifecycle, and close/recovery.

## Stop Conditions For Issues

Do not close the remaining live Path B issues merely because:

- funding succeeds;
- a channel reaches `channel_ready`;
- `litd` reports a keysend-usable balance;
- a fixture-backed report passes;
- the latest payment gets farther than the previous run;
- a daemon log shows a sender-side balance moved before receiver settlement;
- an implementation emits expected balances without observing them live.

Close live issues only from observed settlement and observed balance state.

## Immediate Next Implementation Step

The next code change should make the live mismatch measurable before changing
more behavior:

1. Add rust-lightning diagnostics or a checked fixture helper around the
   simple-taproot HTLC signature verification path.
2. Capture the exact payment-time transcript listed in Phase 1.
3. Add a regression test that fails against the current bounded aux-leaf path.
4. Port the exact single-asset `CreateSecondLevelHtlcTx` equivalent from
   Lightning Labs `tapchannel/commitment.go`.
5. Replace the bounded second-level aux-leaf approximation with that exact
   model.
6. Rerun the live `litd` settlement harness.

This keeps the project aligned with the invariant that asset-channel failures
fail closed and that interop success requires live, observed settlement.
