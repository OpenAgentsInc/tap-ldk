# Remaining Issue Closure Plan

Date: 2026-05-26

This is the current path from the open issue list to a fully closed demo
track. Do not close the epics from fixture-backed reports, expected balances,
or local loopback smokes. Close them only when the issue-specific live and
semantic checks below pass.

## Current State

Path A works as a bounded native-to-native demo. It issues demo `OPENUSD`,
opens a single-asset channel, pays, restarts, cooperatively closes, exports
proof artifacts, and exercises the fork-backed simple-taproot asset-channel
lifecycle state.

The local fork verification script now checks the current pinned
OpenAgentsInc `rust-lightning` revision,
`ff572b99ff6de2aa1e1a9c425b1a80a01bb7581e`, so later issue verification does
not fail against the older proof-ownership-only fork revision.

Path B is live-funded but not live-settled. The current #57/#81 gate reaches:

- live `tapd` proof binding;
- ordered native asset-payment session readiness;
- standalone Lightning Labs current-balance observation;
- integrated `litd` readiness with the asset-channel RPC surface enabled;
- fork-backed `OpenAgentsInc/ldk-node` peer connection to the independent
  `litd` node, with opt-in simple-taproot plus Taproot Asset negotiation
  enabled, remote taproot feature observation, and provenance reporting
  `OpenAgentsInc/rust-lightning@ff572b99ff6de2aa1e1a9c425b1a80a01bb7581e`;
- integrated `litd` asset issuance, live asset-channel funding, channel
  confirmation, and a keysend-usable local asset balance on `litd`.

The gate now stops at `live_asset_channel_payment_settlement`. The current live
blocker is not peer readiness or funding: the live asset keysend stays
`IN_FLIGHT` after Rust Lightning closes on a later payment-time simple-taproot
commitment partial-signature check. Dynamic Taproot Asset commitment
output/aux-leaf construction is still needed for payment-time channel states,
and this issue set must remain open until both directions observe
post-settlement balances from real live Lightning Labs funding/payment.

## Closure Sequence

| Order | Issue | Current state | Required before close |
| --- | --- | --- | --- |
| Done | #77 Fork `ldk-node` | `OpenAgentsInc/ldk-node` exists and is documented as the owned live node implementation home. | Closed. |
| Done | #78 Pin forked `ldk-node` to forked `rust-lightning` | `OpenAgentsInc/ldk-node` is pinned to `OpenAgentsInc/rust-lightning@ff572b99ff6de2aa1e1a9c425b1a80a01bb7581e`; `tap-ldk` consumes the OpenAgentsInc fork line and reports provenance. | Closed. |
| Done | #79 Expose simple-taproot/Taproot Asset config | Implemented in `OpenAgentsInc/ldk-node@0faa999235050a17b198e6bbfa63c2f19aac4cc6`; BTC-only defaults remain unchanged, Taproot Asset negotiation fails closed without simple taproot, and `tap-ldk` live preflight reports both opt-in flags. | Closed. |
| Done | #80 Wire asset messages and payment APIs | Implemented in `OpenAgentsInc/ldk-node@da05c714be061706806bc8757ee74b4709d5a8ef`, with litd-compatible Init feature cleanup and peer taproot feature reporting through `001ec96071ec5943dce42ac2dead8ec2f103f640`; `tap-ldk` pins the latest revision and the live preflight reaches typed asset custom-message, asset-channel open, asset-payment APIs, Lightning Labs aux Init feature bits, and remote feature reporting. The fork advertises Lightning Labs no-op HTLC aux support and does not advertise STXO until native STXO commitment leaves are implemented. | Closed. |
| 1 | #81 Fork-backed Lightning Labs settlement | Current live gate connects to `litd`, observes both taproot feature sets, issues an asset, completes live asset-channel funding, confirms the channel, sees `litd` report it usable for asset keysend, and now preserves/decodes the live `commitment_signed` asset-signature blob. It still fails during payment settlement because Rust Lightning has not derived the payment-time Taproot Asset output scripts that `litd` signs, so the keysend remains `IN_FLIGHT`. | Rust Lightning derives dynamic aux leaves/output scripts for payment-time channel states, verifies the decoded `litd` asset signatures, and `tap-ldk` settles a live payment with observed balances. |
| 2 | #57 Live `tap-ldk` pays Lightning Labs | Harness, proof binding, live current-balance query, integrated `litd`, and fork-backed `ldk-node` peer/API preflight are in place with opt-in asset-channel negotiation enabled. | Run asset-channel funding/payment over the fork-backed connected independent `litd` peer, settle the payment, record post-settlement Lightning Labs receiver balance, record `tap-ldk` sender state, and keep wrong-quote/wrong-asset/wrong-amount failures covered. |
| 3 | #58 Live Lightning Labs pays `tap-ldk` | Receiver-side fixtures, buy-direction RFQ artifacts, quote-bound receive invoice, final-hop metadata, expected balance deltas, and negative checks exist. | Drive a Lightning Labs sender through the live path, have `tap-ldk` receive and validate the asset HTLC metadata through the LDK/fork boundary, persist the received balance and proof reference, restart `tap-ldk`, and compare observed balances on both sides. |
| 4 | #59 Observed live balance reporting | Reports distinguish fixture-backed expected balances from live gates, and `live_daemon_gaps_remaining` remains true. | Make Path B completion impossible unless #57 and #58 both have observed post-settlement balances, compatible asset IDs, compatible payment state, and non-secret proof/payment references. Update README, ROADMAP, ARCHITECTURE, and public runbook after the reports pass. |
| 5 | #60 Semantic proof ancestry validation | MS-SMT, `AssetCommitment`, `TapCommitment`, TAP VM, `TAPF` transport validation, and raw proof preservation exist; full semantic proof ancestry does not. | Validate asset identity, type, anchors, Taproot commitment roots, owner transitions, amount conservation, virtual transaction history, split/previous-witness ancestry, and failure cases. Route the same validation boundary through wallet import, funding, HTLC receipt, cooperative close, and recovery. |
| 6 | #61 BTC simple-taproot LDK epic | Fork surfaces #62 through #70 and #75 are implemented and pinned, with vector and lifecycle smoke coverage. | Close only after BTC-only simple-taproot LDK channels open, pay, reestablish, cooperatively close, force-close, and prove legacy channel behavior is unaffected. |
| 7 | #71 Full Taproot Assets LDK epic | Native primitives, bounded channel state, fork hooks, and live interop scaffolding exist. | Close only after #57 through #60 pass and asset funding, commitment, HTLC, close, monitor, and recovery state are wired into the real simple-taproot LDK state machine without weakening BTC-only behavior. |
| 8 | #19 Path B Lightning Labs interop epic | Fixture-backed interop checks and live readiness gates exist. | Close only after both live payment directions settle against Lightning Labs, observed balances match in both directions, semantic proof validation is enforced, and Path B reports `live_daemon_gaps_remaining=false`. |

## Engineering Path

1. Finish #81 so the fork-backed live runtime uses asset messages and payment
   APIs for live settlement.
2. Finish #57 by replacing the loopback native asset-payment session with the
   fork-backed connected `litd` peer path. The run must use native Rust/LDK
   asset-channel funding/payment logic; `litd` is the counterparty, not a
   `tap-ldk` sidecar.
3. Finish #58 by adding the reverse live sender flow from Lightning Labs into
   `tap-ldk`, including durable receiver balance and restart validation.
4. Finish #59 by tightening report schemas and docs so no expected-only value
   can be read as live interop success.
5. Finish #60 by replacing the remaining proof-envelope boundary with semantic
   proof ancestry validation, then wire that boundary into every path that can
   accept or move asset state.
6. Audit #61, #71, and #19 against their acceptance criteria. Do not close
   them until the live and semantic checks above are complete.

## Verification Before Closing Issues

For every issue, run the issue-specific live command, `cargo fmt --check`,
`cargo test`, and `git diff --check`. For #57, #58, and #59, attach the
non-secret live report summary showing asset ID, payment ID, amount, balance
before/after, proof reference, and whether each side observed the same final
state. For #60, include the semantic proof-validation coverage and negative
fixture classes.
