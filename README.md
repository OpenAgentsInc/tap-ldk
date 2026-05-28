# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Status

Last updated: 2026-05-28

Path A works as a bounded native demo: issue demo `OPENUSD`, exchange proofs,
open a single-asset channel, pay, restart, close, export proofs, and exercise
the OpenAgentsInc `rust-lightning` asset-channel lifecycle state.

Path B now settles the Lightning Labs to native direction: integrated `litd`
funds an asset channel, sends asset keysend, native LDK claims the HTLC, and
`ldk-node` records the native receiver balance. The current pins add the
post-claim balance-output fix, the live zero-HTLC post-claim signature
regression, Taproot HTLC script-path claim witnesses, and the simple-taproot
holder force-close funding input as a one-element key-path Schnorr witness.
They also keep simple-taproot and Taproot Asset opens private by construction.
Missing required simple-taproot open/accept nonces now fail immediately.
Reestablish and revoke-and-ack now use type-22 nonce maps as the authoritative
path and regenerate retransmitted partial signatures from fresh nonce-map
material. A BTC-only simple-taproot conformance gate now covers open, payment,
reconnect/reestablish, functional cooperative close, force-close, P2TR funding
witnesses, and legacy P2WSH isolation.
The latest live run no longer logs the post-claim partial-signature failure or
invalid Taproot control-block failure. Path B still needs the true native
`tap-ldk` to Lightning Labs payment direction.

Spec-compliance work is now split out of #81. #81 stays focused on the live
settlement gate; broader BOLT simple-taproot conformance is tracked in #82 and
#89 through #90 before #61 can close. Path B is not done until both payment
directions settle against Lightning Labs with observed post-settlement balances.

LND, `tapd`, and `litd` are interop peers, not wallet sidecars.

## Development

Run the current setup checks from the repo root:

```bash
cargo fmt --check
cargo test
./scripts/check-btc-simple-taproot-conformance.sh
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- regtest-bitcoin-config
cargo run -p tap-ldk-cli -- lightning-labs-counterparty-config
./scripts/lightning-labs-counterparty.sh connection
./scripts/lightning-labs-counterparty.sh smoke
./scripts/lightning-labs-counterparty.sh tapd-balance '<asset-id>'
./scripts/lightning-labs-litd-counterparty.sh start
./scripts/lightning-labs-litd-counterparty.sh balance '<asset-id>'
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
