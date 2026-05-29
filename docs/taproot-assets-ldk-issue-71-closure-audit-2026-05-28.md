# Taproot Assets LDK Issue 71 Closure Audit

Date: 2026-05-28

Issue #71 is closed for the first-demo LDK Taproot Assets scope described in
the issue body. It is not a production-complete claim for every Taproot Assets
feature.

## Acceptance Mapping

| #71 acceptance item | Evidence |
| --- | --- |
| Bounded hash+sum placeholders are replaced by real protocol primitives. | `tap-ldk-core::mssmt`, `taproot_commitment`, `tap_vm`, and `proof` provide native MS-SMT roots/proofs, `AssetCommitment`, `TapCommitment`, virtual transition validation, and semantic proof ancestry validation. |
| Asset state is persisted before the matching Lightning commitment is safe. | `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f` carries `TaprootAssetChannelState`, monitor aux blobs, HTLC blobs, close allocation, proof ownership recovery, and restart/reestablish coverage. |
| Native-to-native asset channel payments work on the simple-taproot base. | `cargo test --locked` covers `simple_taproot_asset_channel`, `asset_payment`, `asset_recovery`, and `asset_close`; the Path A native demo remains green. |
| `lnd`/`tapd`/`litd` interop records observed balances and compatible proofs in both directions. | `target/path-b-lightning-labs-demo-issue71/path-b-completion-report.json` reports `path_b_complete=true`, `live_daemon_gaps_remaining=false`, `issue_57_acceptance_met=true`, `issue_58_acceptance_met=true`, `semantic_proof_ancestry_complete=true`, observed native receiver balance `125`, and observed `litd` receiver channel balance `125`. |
| Normal BTC channels remain unaffected. | #61 gates passed against the pinned fork: BTC-only simple-taproot conformance, cooperative close, splice policy, and legacy P2WSH isolation. |

## Verification

Run on this closure:

```bash
cargo fmt --check
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked
OPENAGENTS_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-btc-simple-taproot-conformance.sh
OPENAGENTS_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-simple-taproot-cooperative-close.sh
TAP_LDK_RUST_LIGHTNING_DIR=/Users/christopherdavid/.cargo/git/checkouts/rust-lightning-aff72b554919ce0e/8a54739 ./scripts/check-simple-taproot-splice-policy.sh
./scripts/live-tapd-proof-bind.sh target/live-tapd-proof-binding-issue60-final/report.json target/live-tapd-proof-binding-issue60-final/wallet.json
TAP_LDK_PATH_B_ARTIFACT_DIR=target/path-b-lightning-labs-demo-issue71 ./scripts/path-b-lightning-labs-demo.sh
git diff --check
```

The `../.worktrees/rust-lightning` checkout has unrelated local modifications,
so the #61/#71 fork gates were intentionally run against Cargo's clean pinned
checkout at `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`.

## Scope Boundary

The following are future production hardening items outside this first-demo
#71 closure:

- production proof-history replay for every historical virtual transaction;
- grouped assets, collectibles, reissuance, and multi-asset paths;
- full STXO, split, and change-output proof replay;
- reorg watcher integration;
- production proof courier/universe policy;
- concurrent simple-taproot splice/RBF asset-channel candidates.

Those gaps must stay documented and fail closed, but they are not blockers for
the issue #71 acceptance criteria as written.
