# Proof Engine CI

The proof-engine hardening checks are wired through one local wrapper and one
GitHub Actions workflow.

Run the normal local suite with:

```bash
./scripts/proof-engine-check.sh
```

That command runs formatting, `cargo test --locked`, all checked TLA+ configs
through `scripts/formal-check.sh`, the Rust-native property/fuzz/Kani wrapper,
the native wallet-to-wallet demo, and the bounded on-chain lifecycle report
gate. Optional tools are visible: TLA+ skips when no `tlc` or `TLA_TOOLS_JAR`
is available, fuzz smoke skips when `cargo-fuzz` is missing, and Kani skips
when `cargo kani` is missing.

Run the extended local suite with:

```bash
TAP_LDK_EXTENDED_CHECKS=1 ./scripts/proof-engine-check.sh
```

The extended mode adds the BTC simple-taproot conformance script, cooperative
close script, splice-policy script, and the `lnd`/`tapd`/`litd` compatibility
demo. Those checks need the pinned OpenAgentsInc `rust-lightning` checkout for
the BOLT scripts and a Docker-capable environment for the live/demo harnesses.

The GitHub workflow at `.github/workflows/proof-engine.yml` runs the normal
suite on pushes to `main` and pull requests. It also exposes a manual
`workflow_dispatch` extended run that checks out the pinned OpenAgentsInc
`rust-lightning` fork and runs the same wrapper with
`TAP_LDK_EXTENDED_CHECKS=1`.

The CI claim is intentionally bounded. It verifies the current proof-engine
hardening surfaces: semantic proof validation, typed proof-history replay,
wallet/funding/commitment/close/recovery replay gates, bounded anchor-state
policy, local proof-courier bundle validation, formal models, Rust property
tests, fuzz target compile/smoke paths, and optional Kani harnesses. It does
not claim a production network proof universe/courier service, grouped-asset,
live force-close/sweep, live chain-watcher, or asset-channel splice/RBF
completeness. The bounded on-chain lifecycle report now runs through
`scripts/onchain-lifecycle-smoke.sh` inside the normal wrapper. It ties
cooperative close, unilateral recovery, second-level HTLC recovery, final
sweep, failed sweep refusal, and restart evidence into one checked surface.
The next proof-engine expansion is live chain-watcher and sweeper callback
coverage feeding that same event vocabulary.
