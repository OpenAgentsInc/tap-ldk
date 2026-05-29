# tap-ldk

`tap-ldk` is an experimental Rust/LDK wallet demo for Taproot Assets. The goal
is simple: show that a stablecoin-like asset can be issued, checked, sent, and
received by an LDK-based wallet without running LND or `tapd` as a wallet
sidecar.

This is demo software. It is not production-ready wallet infrastructure.

## Status

Last updated: 2026-05-28

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
  proofs before balance or channel state can change.
- The first demo is complete. The closed issues #81, #57, #58, #59, #60, #61,
  #71, and #19 are the checks we keep running for live settlement, payments in
  both directions, observed balances, proof checks, simple taproot channels,
  and `lnd`/`tapd`/`litd` software compatibility.

Still not done:

- production-grade proof history checks;
- grouped assets and multi-asset channels;
- split/change proof replay;
- reorg handling;
- production proof courier policy;
- live force-close and sweep recovery;
- live post-close proof and balance checks;
- live `lnd`/`tapd`/`litd` RFQ signature checks;
- concurrent splice/RBF asset-channel support.

LND, `tapd`, and `litd` are test peers only. They are not sidecars inside the
`tap-ldk` wallet.

## Development

Run the main checks from the repo root:

```bash
cargo fmt --check
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --locked
./scripts/check-btc-simple-taproot-conformance.sh
./scripts/check-simple-taproot-cooperative-close.sh
./scripts/check-simple-taproot-splice-policy.sh
./scripts/path-a-native-demo.sh
./scripts/path-b-lightning-labs-demo.sh
```

Useful CLI entry points:

```bash
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- first-demo-scope
cargo run -p tap-ldk-cli -- wallet-init target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
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
