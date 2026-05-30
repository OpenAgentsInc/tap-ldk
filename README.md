# tap-ldk

`tap-ldk` is an experimental Rust/LDK wallet demo for Taproot Assets. The goal
is simple: show that a stablecoin-like asset can be issued, checked, sent, and
received by an LDK-based wallet without running LND or `tapd` as a wallet
sidecar.

This is demo software. It is not production-ready wallet infrastructure.

## Status

Last updated: 2026-05-30

- Official BOLT Simple Taproot status: the pinned OpenAgentsInc
  `rust-lightning` fork implements the Bitcoin channel base from
  `bolt-simple-taproot.md`. That means final feature negotiation, required
  nonce/signature messages, MuSig2 signing and verification, P2TR funding and
  commitment outputs, cooperative close with RBF, reconnect nonce maps, BTC
  splice nonce maps, HTLC and second-level outputs, unilateral spend metadata,
  and BOLT vector replay are covered. This is not an upstream LDK release
  claim, and the BOLT does not cover the Taproot Assets proof layer below.
- The native wallet-to-wallet demo works: issue demo `OPENUSD`, exchange
  proofs, open one asset channel, pay, restart, close, export proofs, and
  recover the recorded asset state.
- The `lnd`/`tapd`/`litd` compatibility demo works for the first demo against
  integrated `litd`: `litd` funds an asset channel and pays native LDK; native
  LDK records the received balance; native LDK then pays the asset back to
  `litd`; both sides report the expected balance after settlement.
- The wallet code includes the core pieces needed for this proof of concept:
  asset proof checks, asset commitments, simple taproot channel support, asset
  payment metadata, restart-safe storage, close handling, and recovery records.
- Proof import is intentionally strict. The wallet rejects shallow or mismatched
  proofs before balance or channel state can change. Wallet balance/export
  checks now also distinguish confirmed, pending, stale, and reorged proof
  anchors at the bounded replay boundary.
- The first proof-engine hardening sequence is complete: #96 through #106 added
  typed proof-history replay, negative vectors, TLA+ proof-validation checks,
  Rust-native property/fuzz/Kani harnesses, close/recovery replay, anchor-state
  policy, a local proof-engine check wrapper, and GitHub Actions coverage.
- The local proof-courier hardening sequence is complete: #107 through #110
  added a typed bundle that moves proof bytes, optional TAPF bytes,
  proof-history IDs, anchor state, asset fields, and digests together. The
  wallet and CLI can import/export those bundles, and negative tests cover
  malformed transport, mismatched fields, bad digests, stale/reorged anchors,
  and proof-history mismatches.
- The first demo is complete. The closed issues #81, #57, #58, #59, #60, #61,
  #71, and #19 are the checks we keep running for live settlement, payments in
  both directions, observed balances, proof checks, simple taproot channels,
  and `lnd`/`tapd`/`litd` software compatibility.
- The typed chain/sweeper observation gate is now in the normal proof-engine
  wrapper. It validates bounded close, unilateral, HTLC, sweep, refusal, and
  restart observations through `chain-watcher-lifecycle-smoke`, while still
  refusing live watcher or production-ready claims.
- The live regtest callback bridge now exists in the extended proof-engine
  path. It binds the observation report to actual Bitcoin Core regtest
  height/block data and typed watcher/sweeper callback records. It is still
  not a live close or force-close claim because the close/sweep anchors remain
  bounded synthetic demo anchors.

Still not done:

- network proof courier/universe service;
- full production proof-history coverage beyond the current bounded replay
  surfaces;
- grouped assets and multi-asset channels;
- split/change proof replay;
- live chain watcher and reorg integration for real close/sweep transactions;
- live force-close and sweep recovery;
- live post-close proof and balance checks;
- live `lnd`/`tapd`/`litd` RFQ signature checks;
- concurrent splice/RBF asset-channel support.

LND, `tapd`, and `litd` are test peers only. They are not sidecars inside the
`tap-ldk` wallet.

## Development

Run the main checks from the repo root:

```bash
./scripts/proof-engine-check.sh
./scripts/onchain-lifecycle-smoke.sh
./scripts/chain-watcher-lifecycle-smoke.sh
./scripts/live-regtest-chain-watcher-lifecycle-smoke.sh
cargo fmt --check
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked
./scripts/check-btc-simple-taproot-conformance.sh
./scripts/check-simple-taproot-cooperative-close.sh
./scripts/check-simple-taproot-splice-policy.sh
./scripts/rust-verification-check.sh
./scripts/path-a-native-demo.sh
./scripts/path-b-lightning-labs-demo.sh
```

Useful CLI entry points:

```bash
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- first-demo-scope
cargo run -p tap-ldk-cli -- simple-taproot-negotiation-report
cargo run -p tap-ldk-cli -- wallet-init target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-export-proof-bundle target/demo-wallet.json '<proof-id>' target/proof-bundle.json
cargo run -p tap-ldk-cli -- wallet-import-proof-bundle target/receiver-wallet.json target/proof-bundle.json
cargo run -p tap-ldk-cli -- onchain-lifecycle-smoke
cargo run -p tap-ldk-cli -- chain-watcher-lifecycle-smoke
cargo run -p tap-ldk-cli -- live-regtest-chain-watcher-lifecycle-smoke 100 101 1 '<best-block-hash>'
```

## Docs

- [Roadmap](ROADMAP.md)
- [Architecture](ARCHITECTURE.md)
- [Invariants](INVARIANTS.md)
- [Public Demo Runbook](docs/public-demo-runbook.md)
- [Native Wallet-To-Wallet Demo](docs/path-a-native-demo.md)
- [`lnd`/`tapd`/`litd` Compatibility Demo](docs/path-b-lightning-labs-demo.md)
- [Taproot Assets LDK Issue 71 Closure Audit](docs/taproot-assets-ldk-issue-71-closure-audit-2026-05-28.md)
- [`lnd`/`tapd`/`litd` Compatibility Closure Audit](docs/path-b-issue-19-closure-audit-2026-05-28.md)
- [BOLT Simple-Taproot Production Compliance Audit](docs/bolt-simple-taproot-production-compliance-audit-2026-05-28.md)
- [Current Status And Production Readiness Audit](docs/current-status-production-readiness-audit-2026-05-29.md)
- [Proof Courier Bundles](docs/proof-courier-bundles.md)
- [On-Chain Lifecycle Readiness](docs/onchain-lifecycle-readiness.md)
- [Chain-Watcher Lifecycle Readiness](docs/chain-watcher-lifecycle-readiness.md)
- [Negative Proof Vector Coverage](docs/negative-proof-vector-coverage.md)
- [Rust-Native Verification](docs/rust-native-verification.md)
- [Proof Engine CI](docs/proof-engine-ci.md)
