# Remaining Issue Closure Plan

Date: 2026-05-26

This is the current path from the open issue list to a fully closed demo
track. Do not close the epics from fixture-backed reports, expected balances,
or local loopback smokes. Close them only when the issue-specific live and
semantic checks below pass.

The holistic settlement audit is
`docs/path-b-live-settlement-holistic-audit.md`. Treat it as the historical
guide for the completed #81 settlement blocker and as regression context for
the remaining Path B work.
The detailed system audit is
`docs/path-b-live-settlement-system-audit-2026-05-28.md`. Treat it as the
file-level implementation map for the #81 regression gate and remaining #57
work.
The 2026-05-28 diagnostic transcript is
`docs/path-b-live-settlement-diagnostic-run-2026-05-28.md`.
The BOLT simple-taproot implementation audit is
`docs/bolt-simple-taproot-implementation-audit-2026-05-28.md`; treat it as the
spec-compliance checklist. The dedicated issue split is
`docs/bolt-simple-taproot-spec-compliance-issues.md`; use it to keep BOLT
conformance work that does not directly block the live settlement gate out of
#81.

## Current State

Path A works as a bounded native-to-native demo. It issues demo `OPENUSD`,
opens a single-asset channel, pays, restarts, cooperatively closes, exports
proof artifacts, and exercises the fork-backed simple-taproot asset-channel
lifecycle state.

The local fork verification script now checks the current pinned
OpenAgentsInc `rust-lightning` revision,
`8a54739ac030ba3e439496eacb7e1c1216e11c6f`, so later issue verification does
not fail against the older proof-ownership-only fork revision.

Path B now has one live settlement direction. The completed #81 gate
reaches:

- live `tapd` proof binding;
- ordered native asset-payment session readiness;
- standalone Lightning Labs current-balance observation;
- integrated `litd` readiness with the asset-channel RPC surface enabled;
- fork-backed `OpenAgentsInc/ldk-node` peer connection to the independent
  `litd` node, with opt-in simple-taproot plus Taproot Asset negotiation
  enabled, remote taproot feature observation, and provenance reporting
  `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`;
- integrated `litd` asset issuance, live asset-channel funding, channel
  confirmation, and a keysend-usable local asset balance on `litd`;
- live Lightning Labs to native asset keysend with `litd` reporting
  `SUCCEEDED`;
- native `PaymentClaimed` with the Taproot Asset HTLC blob preserved; and
- fork-backed `ldk-node` durable receiver accounting with local asset balance
  `125` and remote balance `0` for the live channel.

#81 is complete because the latest live rerun records the native receiver
balance and no longer logs an invalid post-claim partial signature, invalid
commitment, invalid Taproot control block, or counterparty force-close. The
current fork line moves the claimed asset HTLC to the receiver balance output,
fixes the BOLT audit's legacy signature-field zeroing/rejection rule, adds a
live zero-HTLC post-claim transcript regression, fixes the holder force-close
funding-input fallback by persisting the aggregate key-path Schnorr signature,
makes simple-taproot and Taproot Asset opens private by construction, rejects
missing simple-taproot open/accept nonces before channel state advances, and
uses the Lightning Labs staging scalar nonce for single-funding
RAA/reestablish interop while preserving type-22 nonce maps for final or
multi-funding simple-taproot paths. Broader BOLT conformance items are separate
#61/#71 blockers, not #81 closure criteria.

## Closure Sequence

| Order | Issue | Current state | Required before close |
| --- | --- | --- | --- |
| Done | #77 Fork `ldk-node` | `OpenAgentsInc/ldk-node` exists and is documented as the owned live node implementation home. | Closed. |
| Done | #78 Pin forked `ldk-node` to forked `rust-lightning` | `OpenAgentsInc/ldk-node` is pinned to `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`; `tap-ldk` consumes the OpenAgentsInc fork line and reports provenance. | Closed. |
| Done | #79 Expose simple-taproot/Taproot Asset config | Implemented in `OpenAgentsInc/ldk-node@0faa999235050a17b198e6bbfa63c2f19aac4cc6`; BTC-only defaults remain unchanged, Taproot Asset negotiation fails closed without simple taproot, and `tap-ldk` live preflight reports both opt-in flags. | Closed. |
| Done | #80 Wire asset messages and payment APIs | Implemented in `OpenAgentsInc/ldk-node@da05c714be061706806bc8757ee74b4709d5a8ef`, with litd-compatible Init feature cleanup, peer taproot feature reporting, and proof-derived channel-template binding through `0964b3d0cce5753a0ff42166ea4686702faf93b4`; `tap-ldk` pins the latest revision and the live preflight reaches typed asset custom-message, asset-channel open, asset-payment APIs, Lightning Labs aux Init feature bits, and remote feature reporting. The fork advertises Lightning Labs no-op HTLC aux support and does not advertise STXO until native STXO commitment leaves are implemented. | Closed. |
| Done | #83 Post-claim zero-HTLC transcript | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` derives the Taproot Asset balance script key with the real CSV delay, includes a live `litd` transcript regression with sighash `53c50e50be029ef494407087714245ad42e50c8bf7ae39f9f8589568f705841c`, and the latest live run no longer logs invalid post-claim partial signatures. | Closed. |
| Done | #84 Simple-taproot force-close funding-input witness | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` persists the aggregate holder commitment Schnorr signature, uses it in `HolderFundingOutput`, asserts the holder force-close transaction has a one-element 64-byte key-path witness, and the latest live run no longer logs `Invalid Taproot control block size`. | Closed. |
| Done | #85 Private-only simple-taproot channels | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` clears `announce_channel` for outbound simple-taproot and Taproot Asset opens, rejects inbound public opens for those channel types, and keeps legacy public BTC channel behavior unchanged. | Closed. |
| Done | #86 Immediate simple-taproot nonce validation | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` rejects missing type-4 `next_local_nonce` during simple-taproot and Taproot Asset `open_channel`/`accept_channel` handling before channel state advances; legacy channels can still omit the TLV. | Closed. |
| Done | #87 RAA/reestablish nonce fields | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` uses the Lightning Labs staging scalar nonce for single-funding RAA/reestablish interop, keeps type-22 nonce maps for final or multi-funding paths, and fails closed on scalar fallback when more than one funding txid is active. | Closed. |
| Done | #88 BTC-only simple-taproot conformance gate | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` adds a BTC-only simple-taproot lifecycle test for open, payment, reconnect/reestablish, functional cooperative close, force-close key-path funding witness shape, and legacy P2WSH isolation; `tap-ldk` exposes it through `./scripts/check-btc-simple-taproot-conformance.sh`. | Closed. |
| Done | #89 Cooperative close proof | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` asserts the cooperative-close final transaction has a one-element 64-byte P2TR key-path funding witness; `tap-ldk-core` verifies latest Taproot Asset close allocation and close-store restart preservation; the live `litd` close command exists and the missing native post-close observer is documented instead of claimed. | Closed. |
| Done | #90 Splice nonce-map policy | `tap-ldk-core::demo_scope`, `tap-ldk-cli first-demo-scope`, and `./scripts/check-simple-taproot-splice-policy.sh` explicitly exclude concurrent simple-taproot splicing from the first public demo while still running the pinned fork's simple-taproot and splicing filters. Production/simple-taproot-complete claims must replace this with bounded splice nonce-map coverage. | Closed. |
| Done | #81 Fork-backed Lightning Labs settlement | `target/live-lightning-labs-outgoing-payment-issue81-rerun/report.json` completed with `issue_81_acceptance_met=true`: integrated `litd` funded the asset channel, sent the asset keysend, reported `SUCCEEDED`, native LDK claimed the HTLC, `ldk-node` recorded local receiver balance `125`, and no invalid commitment, partial-signature, control-block, or counterparty force-close logs were observed. | Closed; keep this command green as a regression while completing the remaining Path B issues. |
| Done | #57 Live `tap-ldk` pays Lightning Labs | `target/live-lightning-labs-outgoing-payment-issue57-final/report.json` completed with `issue_57_acceptance_met=true`: native LDK sends the returned asset to integrated `litd` with the canonical Taproot Asset HTLC blob, 354,000 msat BTC carrier amount, settled local-to-remote accounting, observed `litd` channel asset balance, and no invalid-commitment or counterparty force-close markers. | Closed; keep the live script green as the bidirectional regression gate. |
| Done | #58 Live Lightning Labs pays `tap-ldk` | `target/live-lightning-labs-outgoing-payment-issue58-rerun/report.json` completed with `issue_58_acceptance_met=true`: integrated `litd` paid native LDK, native LDK recorded the settled remote-to-local asset payment, bounded incoming metadata failures stayed fail-closed, and the restart snapshot reloaded the received payment/balance checkpoint. | Closed; keep the live script green as the receiver/restart regression gate. |
| Done | #59 Observed live balance reporting | `target/path-b-lightning-labs-demo-issue59/path-b-completion-report.json` completed with `path_b_live_observed_balance_gate_met=true`, `live_daemon_gaps_remaining=false`, fixture-only completion disabled, and expected-only balance completion disabled. | Closed; keep the Path B wrapper completion report green as the observed-balance regression. |
| Done | #60 Semantic proof ancestry validation | `tap-ldk-core::proof` now rejects shallow proof matches with `semantic-ancestry`, strict regtest outpoints, normal asset type, derived Taproot Asset root, expected asset/owner/amount checks, stale-anchor rejection, and Lightning Labs `TAPF` latest-asset-leaf validation. | Closed; production full-history virtual transaction, STXO, grouped-asset, and reorg-watcher proof replay remains #71 hardening. |
| Done | #82 BOLT simple-taproot spec-compliance tracker | The audit has been split into focused issues so #81 stays narrow. Legacy signature-field zeroing/rejection, #83 post-claim transcript, #84 force-close funding-input witness, #85 public-channel rejection, #86 immediate nonce validation, #87 RAA/reestablish nonce-field selection, #88 BTC-only lifecycle gate, #89 cooperative-close proof, and #90 first-demo splice exclusion are fixed. | Closed for first-demo scope; production splice claims remain out of scope. |
| 2 | #61 BTC simple-taproot LDK epic | Fork surfaces #62 through #70 and #75 are implemented and pinned, with vector and lifecycle smoke coverage. #88 proves BTC-only open, pay, reestablish, functional cooperative close, force-close, and legacy isolation; #89 strengthens cooperative-close close/restart evidence; #90 gates concurrent splicing out of first-demo scope; #82 is closed for first-demo scope. | Close only if the issue title/body/comment clearly avoid production splice claims and legacy channel behavior is unaffected. |
| 3 | #71 Full Taproot Assets LDK epic | Native primitives, bounded channel state, fork hooks, and live interop scaffolding exist. | Close only after #57 through #60 pass and asset funding, commitment, HTLC, close, monitor, and recovery state are wired into the real simple-taproot LDK state machine without weakening BTC-only behavior. |
| 4 | #19 Path B Lightning Labs interop epic | Fixture-backed interop checks and live readiness gates exist. | Close only after both live payment directions settle against Lightning Labs, observed balances match in both directions, semantic proof validation is enforced, and Path B reports `live_daemon_gaps_remaining=false`. |

## Engineering Path

1. Keep #81, #57, #58, #59, and #60 green as live settlement,
   receiver/restart, observed-balance, and semantic-proof regressions.
2. Audit #61 with the first-demo splice exclusion explicit and production
   splice claims outside the close criteria.
3. Audit #71 and #19 against their acceptance criteria. Do not close
   them until the live, semantic, and BOLT conformance checks above are
   complete.

## Verification Before Closing Issues

For every issue, run the issue-specific live command, `cargo fmt --check`,
`cargo test`, and `git diff --check`. For #57, #58, and #59, attach the
non-secret live report summary showing asset ID, payment ID, amount, balance
before/after, proof reference, and whether each side observed the same final
state. Keep #60 covered by `cargo test --locked`, the live tapd proof binding
path, and the semantic negative fixture classes.
