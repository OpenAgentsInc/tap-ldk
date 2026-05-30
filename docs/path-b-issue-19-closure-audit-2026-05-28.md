# Path B Issue 19 Closure Audit

Date: 2026-05-28

Issue #19 is closed for the first-demo `lnd`/`tapd`/`litd` interop scope. The
live interop peer is the alternate target from the issue body: integrated
`litd` with Taproot Asset channels enabled. `litd`, `tapd`, and LND remain
interop peers, not sidecars inside the `tap-ldk` wallet runtime.

## Acceptance Mapping

| #19 criterion | Evidence |
| --- | --- |
| `tap-ldk` pays the `lnd`/`tapd`/`litd` counterparty. | `target/path-b-lightning-labs-demo-issue71/live-lightning-labs-outgoing-payment.json` reports `issue_57_acceptance_met=true`, `native_asset_sender_payment_status=settled`, `native_asset_sender_amount=125`, `native_asset_sender_local_balance_after=0`, `native_asset_sender_remote_balance_after=125`, and `lightning_labs_receiver_channel_balance_observed=true`. |
| Both sides agree on asset ID, payment state, and balance. | `target/path-b-lightning-labs-demo-issue71/path-b-completion-report.json` reports the same asset ID for the native receiver and `litd` receiver, observed native receiver balance `125`, observed `litd` receiver channel balance `125`, `path_b_live_observed_balance_gate_met=true`, and `live_daemon_gaps_remaining=false`. |
| `litd` pays `tap-ldk`, or the only unreached blocker is documented. | `litd` pays `tap-ldk`: the same live report records `issue_58_acceptance_met=true`, `integrated_litd_asset_payment_wire_status=SUCCEEDED`, `native_asset_receiver_payment_status=settled`, `native_asset_receiver_local_balance_after=125`, and `native_asset_receiver_restart_state_matches=true`. |
| Proof compatibility is enforced by native code. | The Path B wrapper report has `semantic_proof_ancestry_complete=true`, and `live-tapd-proof-binding.json` records `verification_scope=semantic_ancestry`, `semantic_ancestry_validation=tap_ldk_core_semantic_ancestry`, `fixture_only_path=false`, `tapd_proof_count=1`, and native wallet balance `1000000`. |
| Fixture-only or expected-only reports cannot complete Path B. | `path-b-completion-report.json` records `fixture_only_reports_can_complete_path_b=false` and `expected_only_balances_can_complete_path_b=false`. The nested fixture interop report still documents fixture-only close/payment gaps, but the top-level Path B completion gate is driven by live observed balances and semantic proof validation. |

## Verification

Run on the final #19 closure path:

```bash
cargo fmt --check
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked
git diff --check
```

Previously run live/regression gates used for this closure:

```bash
TAP_LDK_PATH_B_ARTIFACT_DIR=target/path-b-lightning-labs-demo-issue71 ./scripts/path-b-lightning-labs-demo.sh
OPENAGENTS_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-btc-simple-taproot-conformance.sh
OPENAGENTS_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-simple-taproot-cooperative-close.sh
TAP_LDK_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-simple-taproot-splice-policy.sh
./scripts/live-tapd-proof-bind.sh target/live-tapd-proof-binding-issue60-final/report.json target/live-tapd-proof-binding-issue60-final/wallet.json
```

The `../.worktrees/rust-lightning` checkout has unrelated local modifications,
so fork gates were run against Cargo's clean pinned checkout at
`OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`.

## Scope Boundary

This closure does not claim production-complete Taproot Assets support. Future
hardening remains for full proof-history replay, grouped/multi-asset paths,
STXO/split/change proof replay, reorg watchers, network proof universe/courier
service, live force-close/sweep recovery, live post-close proof and balance
observation, live daemon RFQ accept-signature verification, and concurrent
simple-taproot splice/RBF asset-channel candidates.
