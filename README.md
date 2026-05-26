# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Status

Last updated: 2026-05-26

What works today:

- The native `tap-ldk` demo runs end to end between two local wallets: it
  issues demo `OPENUSD`, moves proof data, opens a mocked single-asset channel,
  pays, restarts, closes, and exports final proof artifacts.
- Lightning Labs fixture checks read the imported funding, HTLC, commitment,
  proof, RFQ, invoice, and payment artifacts. Demo scripts write reviewable
  outputs under `target/`.
- Live peer plumbing exists: a localhost `tap-ldk` peer smoke, an ordered
  localhost asset-payment session, and a native LDK preflight connection to
  integrated `litd` when the container topology is running.
- The Lightning Labs harnesses can bootstrap Bitcoin Core, LND, `tapd`, and
  `litd` when Docker or Podman is reachable. The live `tapd` proof-binding
  path can mint `OPENUSD`, export TAPF proof material, and bind it into native
  wallet state.
- Path B writes outgoing-payment gate artifacts and current `tapd` balance
  observations, but still refuses completion until the Lightning Labs receiver
  balance is observed after settlement.
- The OpenAgentsInc `rust-lightning` fork is pinned at
  `6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941`. It includes asset-channel
  feature gates and hooks, BOLT simple-taproot staging negotiation, lifecycle
  wire TLV codecs, and feature-gated MuSig2 signer/nonce-state helpers with
  fail-closed validation tests.

What does not work yet:

- `tap-ldk` does not yet complete a real live asset payment with the
  independent Lightning Labs `litd` node. The peer can connect, but
  asset-channel funding/payment and two-sided balance comparison remain open.
- Standalone LND still lacks `simple-taproot-overlay-chans`; the interop path
  needs the Taproot Assets/Lit aux-controller overlay path or an equivalent
  integration.
- BTC-only BOLT simple-taproot channels do not yet open, pay, reestablish,
  close, or force-close end to end. P2TR funding, commitments, close, HTLCs,
  channel/reestablish wiring, and vector replay remain open.
- Full semantic Taproot Assets proof ancestry validation and live on-chain
  force-close/sweeper integration are not implemented yet.
- LND, `tapd`, and `litd` are only test counterparties for interoperability.
  They are not sidecars inside the `tap-ldk` wallet.

What is being worked on now:

- Issues #57 through #60 remain open for live Path B payments, observed
  Lightning Labs balances, and full proof ancestry validation.
- Issues #62 through #64 are implemented and pinned. Issue #65 is next: P2TR
  funding flow for BTC-only simple-taproot channels.
- Issues #65 through #70 complete the BTC-only BOLT simple-taproot base.
  Issues #71 through #76 then layer real Taproot Assets support on top:
  MS-SMT, asset commitments, virtual transactions, TAP VM validation, LDK
  channel integration, and Lightning Labs `tapd`/`litd` vectors.
- Issue #19 remains the parent Path B epic and should stay open until those
  implementation issues are actually done.

## Development

Run the current setup checks from the repo root:

```bash
cargo fmt --check
cargo test
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
