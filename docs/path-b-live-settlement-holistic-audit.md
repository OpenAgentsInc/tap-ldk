# Path B Live Settlement Holistic Audit

Date: 2026-05-28

This audit records the root-cause work that cleared the live Lightning Labs
Path B #81 blocker and the work path that remains after it. The failure was not
a Docker, peer-connection, funding, or basic feature-negotiation problem. The
live path now reaches a real Lightning Labs `litd` asset channel, settles a
Lightning Labs to native asset keysend, and records the native receiver-side
asset payment and balance through the fork-backed `ldk-node` runtime. The
post-claim partial-signature blocker and the force-close/on-chain Taproot
witness fallback blocker are fixed and fixture-backed.

The more detailed file-level audit and implementation map is
`docs/path-b-live-settlement-system-audit-2026-05-28.md`. Use that document as
historical context and keep the completed #81 live gate green while working
#57 and later Path B issues.

## Current Live Result

Latest completed #81 artifact:

- `target/live-lightning-labs-outgoing-payment-issue81-rerun/report.json`
- `status`: `completed`
- `blocked_step`: `null`
- `issue_81_acceptance_met`: `true`
- `integrated_litd_asset_payment_wire_status`: `SUCCEEDED`
- `native_asset_receiver_payment_status`: `settled`
- `native_asset_receiver_local_balance_after`: `125`
- invalid commitment, partial-signature, control-block, and counterparty
  force-close logs: all `false`
- `openagents_rust_lightning_rev`:
  `8a54739ac030ba3e439496eacb7e1c1216e11c6f`

Previous diagnostic artifact:

- `target/live-lightning-labs-outgoing-payment-diagnostic/report.json`
- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `openagents_rust_lightning_rev`:
  `85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1`
- `live_node_runtime`: `ldk-node 0.8.0+git`
  from `https://github.com/OpenAgentsInc/ldk-node`
- `integrated_litd_asset_channel_fund_status`: `completed`
- `integrated_litd_asset_channel_usable_for_keysend`: `true`
- `integrated_litd_asset_channel_local_balance`: `125`
- `integrated_litd_asset_payment_status`: `timed_out`
- `integrated_litd_asset_payment_wire_status`: `IN_FLIGHT`
- `integrated_litd_post_payment_balance`: `999875`
- `asset_channel_settlement_ready`: `false`

The transcript from this run is recorded in
`docs/path-b-live-settlement-diagnostic-run-2026-05-28.md`.

Follow-up after this artifact: the current code is now pinned to
`OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` and
`OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`. That line
keeps the failing transcripts as regression fixtures, adds the Lightning Labs
second-level virtual `lock_time`/`relative_lock_time` asset-leaf fields, full
counterparty commitment monitor persistence, exact previous-output-bound
second-level HTLC aux leaves before signing outgoing HTLC transactions, native
receiver-side asset payment accounting, zero-HTLC asset commitment-sig blob
handling for post-claim `commitment_signed`, dynamic post-claim balance-output
aux-leaf placement for claimed full-amount asset HTLCs, and a live zero-HTLC
post-claim partial-signature regression fixture.

Post-claim-fix live artifact:

- `target/live-lightning-labs-outgoing-payment-post-claim-fix/report.json`
- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `integrated_litd_asset_payment_status`: `completed`
- `integrated_litd_asset_payment_wire_status`: `SUCCEEDED`
- `native_asset_receiver_payment_status`: `settled`
- `native_asset_receiver_local_balance_after`: `125`
- `native_ldk_invalid_simple_taproot_commitment_partial_sig_logged`: `false`
- `native_ldk_invalid_taproot_control_block_logged`: `false`

Historical outgoing-signature rerun artifact:

- `target/live-lightning-labs-outgoing-payment-full-commitment/report.json`
- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `openagents_rust_lightning_rev`:
  `acce215e1ca284fa45f1c13e13760de459d410d4`
- `integrated_litd_asset_channel_fund_status`: `completed`
- `integrated_litd_asset_channel_usable_for_keysend`: `true`
- `integrated_litd_asset_channel_local_balance`: `125`
- `integrated_litd_asset_payment_status`: `timed_out`
- `integrated_litd_asset_payment_wire_status`: `IN_FLIGHT`
- `integrated_litd_asset_payment_hash`:
  `fe213ee74516cdb29eda6a57fc6ed9cdf8b7f6e8f77de0f4eef29de3e3804239`

The new result changes the live diagnosis. Rust Lightning now logs:

```text
Received valid commitment_signed from peer
Completed off-chain monitor update 1
Enqueueing message RevokeAndACK
```

It then signs the remote HTLC transaction and sends the local response. `litd`
rejects that response:

```text
rejected commitment: commit_height=1, invalid_htlc_sig=...
```

The important fact is that funding succeeds and `litd` believes the channel is
usable for asset keysend. The earlier payment-time commitment update reached
Rust Lightning, but Rust Lightning rejected the peer HTLC Schnorr signature:

```text
Invalid simple-taproot HTLC signature from peer
```

After the close, the force-close path also exposed a separate funding-input
witness construction problem:

```text
Invalid Taproot control block size
```

The full-commitment rerun proved that the monitor/update release path was no
longer the active blocker, and the follow-up exact previous-output-bound
second-level HTLC aux-leaf work moved the live path past the outgoing
HTLC-signature rejection. The #84 follow-up identified the control-block
failure as a legacy P2WSH witness being used on the P2TR funding input; the
holder fallback now uses the persisted aggregate key-path Schnorr signature.

Latest completed live rerun artifact:

- `target/live-lightning-labs-outgoing-payment-owner-state/report.json`
- `status`: `blocked`
- `blocked_step`: `live_asset_channel_payment_settlement`
- `openagents_rust_lightning_rev`:
  `8a54739ac030ba3e439496eacb7e1c1216e11c6f`
- `integrated_litd_asset_channel_fund_status`: `completed`
- `integrated_litd_asset_channel_usable_for_keysend`: `true`
- `integrated_litd_asset_payment_status`: `completed`
- `integrated_litd_asset_payment_wire_status`: `SUCCEEDED`
- `native_asset_receiver_payment_recorded`: `true`
- `native_asset_receiver_payment_status`: `settled`
- `native_asset_receiver_amount`: `125`
- `native_asset_receiver_local_balance_after`: `125`
- `native_asset_receiver_remote_balance_after`: `0`
- `native_ldk_payment_claimable_logged`: `true`
- `native_ldk_payment_claimed_logged`: `true`
- `native_ldk_empty_asset_commit_sig_blob_logged`: `true`
- `native_ldk_invalid_commitment_logged`: `false`
- `native_ldk_invalid_simple_taproot_partial_sig_logged`: `false`
- `native_ldk_invalid_taproot_control_block_logged`: `false`

The newer #57 result changes the live diagnosis again. The bounded Lightning
Labs to native direction settles and persists receiver balance, the native
return payment settles into integrated `litd`, and the post-claim
partial-signature transcript verifies. The #84 follow-up also fixes the stale
funding-input control-block fallback symptom. The #58 issue-specific
receive/restart proof and #59 completion report are now complete; the next live
proof gate is #60 semantic proof ancestry validation, which is now implemented
and must stay green as a regression.

## What Works

- `tap-ldk` consumes the OpenAgentsInc `ldk-node` fork.
- `ldk-node` consumes the OpenAgentsInc `rust-lightning` fork.
- After the latest pin update, all `lightning*` packages in `tap-ldk` resolve
  to
  `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`.
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
- The latest live rerun accepts the peer `commitment_signed`.
- The latest rust-lightning pin treats simple-taproot HTLC second-level
  signing like the Lightning Labs anchor path:
  `SIGHASH_SINGLE|ANYONECANPAY` and input sequence `1`, and encodes
  Lightning Labs virtual lock fields in second-level Taproot Asset HTLC aux
  leaves.
- The current rust-lightning pin mutates the commitment transaction's nondust
  HTLC state to the exact previous-output-bound second-level aux leaf before
  outgoing HTLC signatures are produced.
- `litd` reports the asset keysend as `SUCCEEDED`.
- Native LDK logs the received payment and claimed payment for the live asset
  payment.
- `ldk-node` records the receiver-side Taproot Asset payment as `settled` and
  persists local asset balance `125`.
- The code path includes zero-HTLC asset commitment-sig blob handling for the
  post-claim `commitment_signed`; the post-claim fixture now asserts the exact
  `litd` sighash and partial-signature path.
- The holder force-close fallback persists the aggregate simple-taproot
  commitment signature and spends the funding input with a one-element key-path
  Schnorr witness.

## What Is Still Broken

The post-claim partial-signature transcript is no longer the active blocker.
The current Rust path accepts the payment-time HTLC commitment, settles the
Lightning Labs to native keysend, records receiver balance, and no longer logs
the invalid post-claim partial signature or invalid Taproot control-block
fallback. The remaining live-settlement work is the true
native-to-Lightning Labs payment direction and two-sided observed balance
reporting.

The earlier 2026-05-28 diagnostic run still matters as a regression fixture:
Rust's second-level aux leaf contained local root/script-key material that did
not match the Lightning Labs `tapchannel`/`tapsend` second-level allocation
trace. Later pins fixed the missing virtual lock fields and the exact
previous-output-bound second-level aux leaf. If another live rerun exposes a
new signature or force-close transcript delta, the remaining work is a
fixture-backed port of the bounded single-asset allocation path, not another
isolated sighash, TLV, or key guess.

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
- `scripts/check-btc-simple-taproot-conformance.sh`
- `scripts/check-simple-taproot-cooperative-close.sh`
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

The fork previously derived the second-level HTLC aux leaf from a generic
bounded single-asset template before the commitment txid and HTLC output index
were known. That was useful to get past earlier open/funding failures, but it
was not an adequate source of truth for live payment-time HTLC signing. The
current pin now rewrites nondust HTLC state to an exact previous-output-bound
second-level aux leaf after the commitment transaction is built, and the live
rerun proved that this is enough to move the bounded Lightning Labs to native
payment path through `SUCCEEDED` plus native receiver accounting.

The right fix remains fixture-backed transcript work, not another isolated
change to a sighash flag, output value, TLV, or key order. Because `litd` still
rejects the post-claim commitment after native settlement, the next step is to
port the remaining relevant Lightning Labs Taproot Asset post-claim commitment
semantics into Rust and wire that through rust-lightning and ldk-node as the
only accepted Path B settlement path.

## Required Work

### Phase 1: Capture The Exact Transcript

Keep deterministic diagnostics and fixtures from the live runs so the Rust side
records every input to rejected post-claim commitment and HTLC-success fallback
checks:

- pre-claim and post-claim commitment transaction bytes;
- post-claim commitment number and output set;
- post-claim commitment signature bytes and verifying public key;
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

The current pin includes a regression for exact previous-output-bound
second-level aux leaves. Add the post-claim zero-HTLC commitment partial
signature transcript, including explicit zero-HTLC asset commitment-sig blob
presence, as the next checked fixture before changing more behavior.

### Phase 2: Port Lightning Labs Allocation Semantics

Implement the bounded but exact single-asset subset of the Lightning Labs
`tapchannel` and `tapsend` pipeline in Rust for the post-claim commitment and
reverse-direction payment path:

- channel-view input model;
- asset allocation model;
- first-level HTLC allocation;
- second-level HTLC allocation;
- post-claim no-HTLC local/remote asset allocation;
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

The current force-close attempt now happens after a successful Lightning Labs
to native payment and exposes the remaining HTLC-success fallback weakness.
This must be fixed while the post-claim commitment transcript is made exact:

- preserve spend info for first-level and second-level outputs;
- rebuild the right control block for the selected leaf;
- verify script-path spends in tests;
- persist enough monitor data to recover after restart;
- export proof data only for the actual final spendable output.

### Phase 6: Settle Both Live Directions

#19 is closed from live observed balances and semantic proof validation, not
from fixture-only or readiness reports. Keep the closed #81, #57, #58, #59,
#60, #61, #71, and #19 gates running as regressions.

Required live evidence for closure:

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

### Phase 7: Complete Broader Protocol Work

After the bounded live payment path and #60 proof boundary work, finish the
remaining protocol surface:

- partial split and change-output support;
- STXO commitment leaves;
- multi-input same-asset funding where required;
- production full-history Taproot Asset proof-chain validation;
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

The next step is to keep the #84 funding-input fallback regression in place and
move the remaining live-demo work forward:

1. Preserve the latest bidirectional #57/#58 artifact with no post-claim
   signature, invalid-control-block, invalid-commitment, or force-close marker.
2. Preserve the #59 Path B observed-balance completion report.
3. Keep #60 semantic proof ancestry validation and #61 simple-taproot coverage
   green as #19's regression evidence.

This keeps the project aligned with the invariant that asset-channel failures
fail closed and that interop success requires live, observed settlement.
