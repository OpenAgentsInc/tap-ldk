# Proof Engine CI

The proof-engine hardening checks are wired through one local wrapper and one
Google Cloud Build config.

Run the normal local suite with:

```bash
./scripts/proof-engine-check.sh
```

That command runs formatting, `cargo test --locked`, all checked TLA+ configs
through `scripts/formal-check.sh`, the Rust-native property/fuzz/Kani wrapper,
the native wallet-to-wallet demo, the bounded on-chain lifecycle report gate,
and the bounded chain/sweeper observation report gate. Optional tools are
visible: TLA+ skips when no `tlc` or `TLA_TOOLS_JAR` is available, fuzz smoke
skips when `cargo-fuzz` is missing, and Kani skips when `cargo kani` is
missing.

Run the extended local suite with:

```bash
TAP_LDK_EXTENDED_CHECKS=1 ./scripts/proof-engine-check.sh
```

The extended mode adds the BTC simple-taproot conformance script, cooperative
close script, splice-policy script, the live-regtest chain-watcher lifecycle
callback script, and the `lnd`/`tapd`/`litd` compatibility demo. Those checks
need the pinned OpenAgentsInc `rust-lightning` checkout for the BOLT scripts
and a Docker-capable environment for the live/demo harnesses.

The remote runner path is Google Cloud Build, not GitHub-hosted Actions. The
GitHub workflow was removed because GitHub was refusing to start jobs before
any repo step ran. `cloudbuild.yaml` runs the proof-engine wrapper inside a
Rust 1.85 container with Docker socket access for the regtest and demo
containers.

Submit the fast remote suite with:

```bash
./scripts/gcloud-proof-engine-submit.sh fast
```

Submit the extended remote suite with:

```bash
./scripts/gcloud-proof-engine-submit.sh extended
```

The submit wrapper uses `GOOGLE_CLOUD_PROJECT` or the active `gcloud` project.
It uses `GOOGLE_CLOUD_REGION`, `builds/region`, or `compute/region` when a
region is configured. The Cloud Build config can also be submitted directly:

```bash
gcloud builds submit . \
  --config cloudbuild.yaml \
  --substitutions _TAP_LDK_EXTENDED_CHECKS=0
```

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
The bounded chain-watcher lifecycle report now runs through
`scripts/chain-watcher-lifecycle-smoke.sh` beside it. That second report
checks typed chain, sweeper, and wallet/monitor observations for every
lifecycle event while refusing live watcher and production-ready claims. The
extended proof-engine path now also runs
`scripts/live-regtest-chain-watcher-lifecycle-smoke.sh`, which binds that
observation vocabulary to Bitcoin Core regtest height/block data and typed
callback records. The next proof-engine expansion is real close/sweep
transaction anchoring and reorg-stream coverage feeding that same event and
observation vocabulary.
