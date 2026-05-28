# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Status

Last updated: 2026-05-28

- Path A means native-to-native: two `tap-ldk`/LDK wallets talking to each
  other without a Lightning Labs daemon. This works for the bounded demo:
  issue demo `OPENUSD`, exchange proofs, open a single-asset channel, pay,
  restart, cooperatively close, export proofs, and exercise the
  OpenAgentsInc `rust-lightning` asset-channel lifecycle state.
- Path B means interop: `tap-ldk`/LDK talks to an independent Lightning Labs
  peer. This works for the first-demo scope against integrated `litd`: `litd`
  funds a Taproot Asset channel and pays native LDK, native LDK records and
  persists the received balance, then native LDK pays the same asset back to
  `litd` and the `litd` channel balance is observed after settlement.
- Native Taproot Assets pieces are now present for the demo path: MS-SMT
  roots/proofs, `AssetCommitment`, `TapCommitment`, TAP VM transition checks,
  semantic proof ancestry validation, simple-taproot channel support, asset
  HTLC metadata, close allocation, monitor persistence, restart checks, and
  recovery ownership records.
- Proof import is fail-closed. Native proof records must use
  `semantic-ancestry`, strict regtest outpoints, normal demo asset type,
  derived Taproot Asset roots, expected owner/amount/asset checks, and stale
  anchor rejection. Lightning Labs `TAPF` import decodes the latest `TAPP`
  asset leaf and checks asset ID, type, amount, owner script key, and genesis
  before wallet state advances.
- The first-demo issue queue is closed. #81, #57, #58, #59, #60, #61, #71,
  and #19 are regression gates for live settlement, bidirectional payment,
  observed balances, semantic proof validation, first-demo simple-taproot, and
  first-demo Taproot Assets-over-LDK interop.
- This is not production-complete Taproot Assets support. Still future work:
  full proof-history replay, grouped and multi-asset paths, STXO/split/change
  proof replay, reorg watchers, proof courier policy, live force-close/sweep
  recovery, live post-close proof and balance observation, daemon RFQ
  accept-signature verification, and concurrent simple-taproot splice/RBF
  asset-channel candidates.

LND, `tapd`, and `litd` are interop peers only. They are not wallet sidecars
inside `tap-ldk`.

## Development

Run the current setup checks from the repo root:

```bash
cargo fmt --check
cargo test
./scripts/check-btc-simple-taproot-conformance.sh
./scripts/check-simple-taproot-cooperative-close.sh
./scripts/check-simple-taproot-splice-policy.sh
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- first-demo-scope
cargo run -p tap-ldk-cli -- regtest-bitcoin-config
cargo run -p tap-ldk-cli -- lightning-labs-counterparty-config
./scripts/lightning-labs-counterparty.sh connection
./scripts/lightning-labs-counterparty.sh smoke
./scripts/lightning-labs-counterparty.sh tapd-balance '<asset-id>'
./scripts/lightning-labs-litd-counterparty.sh start
./scripts/lightning-labs-litd-counterparty.sh balance '<asset-id>'
./scripts/lightning-labs-litd-counterparty.sh close-asset-channel '<txid:index>' false
./scripts/live-tapd-proof-bind.sh target/live-tapd-proof-binding/report.json target/live-tapd-proof-binding/wallet.json
cargo run -p tap-ldk-cli -- ldk-baseline-plan target/ldk-baseline
cargo run -p tap-ldk-cli -- ldk-baseline-smoke target/ldk-baseline-smoke.json
cargo run -p tap-ldk-cli -- live-peer-smoke target/live-peer-smoke.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- live-asset-payment-session-smoke target/live-asset-payment-session.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 125
cargo run -p tap-ldk-cli -- live-litd-peer-preflight target/live-litd-peer-preflight.json target/live-litd-peer-preflight-state '<litd-node-id>' '127.0.0.1:29735'
cargo run -p tap-ldk-cli -- asset-negotiation-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- asset-peer-message-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- rfq-request target/rfq-quotes.json alice 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 250000 200 1111111111111111111111111111111111111111111111111111111111111111 path-a-demo-1 100
cargo run -p tap-ldk-cli -- rfq-invoice-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- asset-channel-funding-smoke target/asset-channels.json
cargo run -p tap-ldk-cli -- asset-commitment-smoke target/asset-commitments.json
cargo run -p tap-ldk-cli -- asset-htlc-smoke
cargo run -p tap-ldk-cli -- asset-payment-smoke
cargo run -p tap-ldk-cli -- asset-recovery-smoke
cargo run -p tap-ldk-cli -- asset-close-smoke
cargo run -p tap-ldk-cli -- simple-taproot-asset-channel-smoke
cargo run -p tap-ldk-cli -- lightning-labs-blob-fixture-smoke fixtures/lightning-labs/tapchannelmsg/testdata
cargo run -p tap-ldk-cli -- lightning-labs-proof-fixture-smoke fixtures/lightning-labs/proof/testdata
cargo run -p tap-ldk-cli -- lightning-labs-funding-interop-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-funding-interop.json
cargo run -p tap-ldk-cli -- lightning-labs-rfq-invoice-compat-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- lightning-labs-outgoing-payment-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-outgoing-payment.json
cargo run -p tap-ldk-cli -- lightning-labs-incoming-payment-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-incoming-payment.json
cargo run -p tap-ldk-cli -- lightning-labs-interop-check-smoke fixtures/lightning-labs/tapchannelmsg/testdata fixtures/lightning-labs/proof/testdata target/lightning-labs-interop-checks.json
./scripts/path-a-native-demo.sh
./scripts/path-b-lightning-labs-demo.sh
./scripts/full-demo-smoke.sh
cargo run -p tap-ldk-cli -- wallet-init target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-issue-openusd target/demo-wallet.json 1000000 02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f
cargo run -p tap-ldk-cli -- wallet-import-proof-fixture target/demo-wallet.json fixtures/synthetic/proof_anchor_valid.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
```

## Planning Docs

- [Roadmap](ROADMAP.md)
- [Architecture](ARCHITECTURE.md)
- [Invariants](INVARIANTS.md)
- [Path B Live Settlement Holistic Audit](docs/path-b-live-settlement-holistic-audit.md)
- [Path B Live Settlement System Audit](docs/path-b-live-settlement-system-audit-2026-05-28.md)
- [Path B Live Settlement Diagnostic Run](docs/path-b-live-settlement-diagnostic-run-2026-05-28.md)
- [BOLT Simple Taproot Implementation Audit](docs/bolt-simple-taproot-implementation-audit-2026-05-28.md)
- [BOLT Simple Taproot Spec Compliance Issue Plan](docs/bolt-simple-taproot-spec-compliance-issues.md)
- [Simple Taproot Cooperative Close Proof](docs/simple-taproot-cooperative-close-2026-05-28.md)
- [Simple Taproot Splice Policy](docs/simple-taproot-splice-policy-2026-05-28.md)
- [Remaining Issue Closure Plan](docs/remaining-issue-closure-plan.md)
- [OpenAgentsInc LDK Node Fork](docs/openagents-ldk-node-fork.md)
- [Protocol References](docs/protocol-references.md)
- [BLIP-TAP Implementation Note](docs/blip-tap-implementation-note.md)
- [LDK Asset-Channel Extension Boundary](docs/ldk-asset-channel-extension-boundary.md)
- [OpenAgentsInc Rust-Lightning Fork](docs/openagents-rust-lightning-fork.md)
- [Polar Regtest Topology](docs/polar-regtest-topology.md)
- [Headless Bitcoin Regtest Harness](docs/headless-regtest-harness.md)
- [Baseline LDK Node](docs/baseline-ldk-node.md)
- [Live tap-ldk Peer Smoke](docs/live-tap-ldk-peer.md)
- [Live Asset Payment Session](docs/live-asset-payment-session.md)
- [Live litd Peer Preflight](docs/live-litd-peer-preflight.md)
- [Live tapd Proof Binding](docs/live-tapd-proof-binding.md)
- [Semantic Proof Ancestry Validation](docs/semantic-proof-ancestry-validation.md)
- [Taproot Assets LDK Issue 71 Closure Audit](docs/taproot-assets-ldk-issue-71-closure-audit-2026-05-28.md)
- [Path B Issue 19 Closure Audit](docs/path-b-issue-19-closure-audit-2026-05-28.md)
- [Lightning Labs Interop Matrix](docs/lightning-labs-interop-matrix.md)
- [Lightning Labs Blob Fixtures](docs/lightning-labs-blob-fixtures.md)
- [Lightning Labs Funding Interop](docs/lightning-labs-funding-interop.md)
- [Lightning Labs RFQ Invoice Compatibility](docs/lightning-labs-rfq-invoice.md)
- [Lightning Labs Outgoing Payment](docs/lightning-labs-outgoing-payment.md)
- [Lightning Labs Incoming Payment](docs/lightning-labs-incoming-payment.md)
- [Lightning Labs Interop Checks](docs/lightning-labs-interop-checks.md)
- [tapd Proof Import/Export](docs/tapd-proof-import-export.md)
- [Lightning Labs Counterparty Harness](docs/lightning-labs-counterparty-harness.md)
- [Lightning Labs litd Counterparty](docs/lightning-labs-litd-counterparty.md)
- [Wallet Storage](docs/wallet-storage.md)
- [Public Demo Runbook](docs/public-demo-runbook.md)
- [Web Demo App Spec](docs/web-demo-app-spec.md)
- [Path A Native-To-Native Demo](docs/path-a-native-demo.md)
- [Path B Lightning Labs Demo](docs/path-b-lightning-labs-demo.md)
