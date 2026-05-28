# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Status

Last updated: 2026-05-28

Path A works as a bounded native demo: issue demo `OPENUSD`, exchange proofs,
open a single-asset channel, pay, restart, close, export proofs, and exercise
the OpenAgentsInc `rust-lightning` asset-channel lifecycle state.

Path B now settles both live payment directions over an integrated Lightning
Labs `litd` asset channel. `litd` can fund the channel and pay native LDK;
native LDK claims the asset HTLC and records the receiver balance. Native LDK
can then send the same asset back to `litd` with the canonical Taproot Asset
HTLC blob, a dust-covering BTC amount, and observed `litd` channel asset
balance. The current pins also cover the post-claim balance-output fix,
Taproot HTLC script-path claim witnesses, simple-taproot key-path force-close
witnesses, private-only simple-taproot/Taproot Asset opens, immediate missing
nonce rejection, Lightning Labs staging scalar nonce interop, and the BTC-only
simple-taproot lifecycle conformance gate.
Native cooperative-close coverage now also asserts the final close transaction
uses a one-element 64-byte Taproot key-path witness and the asset-channel smoke
proves the latest asset allocation survives close-store restart. The live
Lightning Labs cooperative-close command is available, but Path B still needs
native post-close proof and balance observation before claiming live close.
Concurrent simple-taproot splicing is explicitly excluded from the first public
demo until bounded splice nonce-map tests are added.
The latest live run no longer logs the post-claim partial-signature failure,
invalid Taproot control-block failure, invalid commitment failure, or
counterparty force-close. #81, #57, #58, #59, #60, #61, #71, and #19 are
completed regression gates for the first-demo scope. The Path B wrapper now
writes a completion report that sets `path_b_live_observed_balance_gate_met=true`
only from live observed balances and marks `path_b_complete=true` when the live
observed-balance gate and semantic proof ancestry validation are both green.

Proof import no longer accepts shallow field matches. Native proof records must
use the `semantic-ancestry` scope, strict regtest outpoints, normal-asset demo
type, derived Taproot Asset root, expected owner/amount/asset checks, and stale
anchor rejection. Lightning Labs `TAPF` import decodes the latest `TAPP` asset
leaf, derives the Taproot Assets asset ID from genesis, and checks asset ID,
type, amount, owner script key, and genesis before wallet state advances.

Spec-compliance work is split out of #81. #61 is complete for the first-demo
BOLT simple-taproot scope, and #71 is complete for the first-demo native
Taproot Assets-over-LDK scope. Remaining production hardening includes full
proof-history replay, grouped/multi-asset paths, STXO/split/change proof
replay, reorg watchers, proof courier policy, live force-close/sweep recovery,
and concurrent simple-taproot splice/RBF asset-channel candidates. The
first-demo issue queue is closed; future work should be opened as production
hardening, not as a claim that the first-demo interop path is still incomplete.

LND, `tapd`, and `litd` are interop peers, not wallet sidecars.

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
