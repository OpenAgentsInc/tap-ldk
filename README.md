# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Development

Run the current setup checks from the repo root:

```bash
cargo fmt --check
cargo test
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- regtest-bitcoin-config
cargo run -p tap-ldk-cli -- lightning-labs-counterparty-config
cargo run -p tap-ldk-cli -- ldk-baseline-plan target/ldk-baseline
cargo run -p tap-ldk-cli -- ldk-baseline-smoke target/ldk-baseline-smoke.json
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
./scripts/path-a-native-demo.sh
cargo run -p tap-ldk-cli -- wallet-init target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-issue-openusd target/demo-wallet.json 1000000 02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f
cargo run -p tap-ldk-cli -- wallet-import-proof-fixture target/demo-wallet.json fixtures/synthetic/proof_anchor_valid.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
```

## Planning Docs

- [Roadmap](ROADMAP.md)
- [Invariants](INVARIANTS.md)
- [Protocol References](docs/protocol-references.md)
- [BLIP-0029 Implementation Note](docs/blip-0029-implementation-note.md)
- [LDK Asset-Channel Extension Boundary](docs/ldk-asset-channel-extension-boundary.md)
- [OpenAgentsInc Rust-Lightning Fork](docs/openagents-rust-lightning-fork.md)
- [Polar Regtest Topology](docs/polar-regtest-topology.md)
- [Headless Bitcoin Regtest Harness](docs/headless-regtest-harness.md)
- [Baseline LDK Node](docs/baseline-ldk-node.md)
- [Lightning Labs Interop Matrix](docs/lightning-labs-interop-matrix.md)
- [Lightning Labs Blob Fixtures](docs/lightning-labs-blob-fixtures.md)
- [Lightning Labs Counterparty Harness](docs/lightning-labs-counterparty-harness.md)
- [Wallet Storage](docs/wallet-storage.md)
- [Path A Native-To-Native Demo](docs/path-a-native-demo.md)
