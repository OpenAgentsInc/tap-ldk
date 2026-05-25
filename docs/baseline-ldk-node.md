# Baseline LDK Node

Date: 2026-05-25

Issue #23 establishes the BTC-only Lightning boundary that later Taproot Asset
channel work must not weaken. The implementation pins `ldk-node` `0.7.0` as the
first native LDK node surface and keeps asset-channel behavior disabled in the
baseline path.

## Commands

Render the intended two-node regtest plan:

```bash
cargo run -p tap-ldk-cli -- ldk-baseline-plan target/ldk-baseline
```

Run the headless BTC-only smoke abstraction and persist its restart state:

```bash
cargo run -p tap-ldk-cli -- ldk-baseline-smoke target/ldk-baseline-smoke.json
```

The smoke records the required baseline sequence: start Alice/Bob, connect
peers, sync regtest height, fund on-chain wallets, open a normal BTC channel,
settle a normal BTC payment, restart Bob, and reload the same state. It is
deliberately BTC-only: asset-channel features, asset-channel channels, and
asset-payment metadata fail validation.

## Live Regtest Boundary

The code also includes `BaselineLdkNodeConfig::build_node`, which constructs a
real `ldk_node::Node` on regtest with Bitcoin Core RPC and explicit storage and
listening paths. A live end-to-end channel/payment run still requires local
`bitcoind`/`bitcoin-cli` or a Docker/Polar topology. Those binaries were not
available in the shell used for issue #23, so the checked test coverage is the
headless BTC-only state smoke plus compile-time LDK node wiring.

## Invariants

- Baseline channels are BTC-only and cannot be marked as asset channels.
- Baseline payments cannot carry asset metadata.
- Asset-channel feature flags must stay disabled until the later negotiation
  issue explicitly enables them.
- The BTC-only baseline state must persist across restart before asset-channel
  state is layered on.
