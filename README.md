# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Status

Last updated: 2026-05-26

What works today:

- The native `tap-ldk` demo runs end to end between two local `tap-ldk` wallets.
  It issues a demo `OPENUSD` asset, moves proof data between wallets, opens a
  mocked single-asset channel, makes a demo payment, restarts, closes the
  channel, and exports final proof artifacts.
- The Lightning Labs compatibility checks can read the current Taproot Assets
  fixture data we imported from Lightning Labs. That includes funding blobs,
  HTLC blobs, commitment blobs, proof files, RFQ data, invoice binding, and
  both payment directions as stored demo artifacts.
- The demo scripts write reviewable artifacts under `target/`, including
  balances, proof files, restart checks, close checks, and logs.
- `tap-ldk` now has a first live peer smoke. It starts a localhost peer
  listener, connects a second peer over TCP, negotiates the experimental
  single-asset channel capability through the OpenAgentsInc rust-lightning
  fork, and round-trips an encoded native RFQ custom message.
- The native live peer path now also has an ordered asset-payment session
  smoke. It sends input proof chunks, output proof chunks, funding created,
  funding accepted, RFQ request, RFQ accept, and asset HTLC messages over the
  socket and records the acked message sequence.
- The Lightning Labs counterparty harness now performs a full ordered
  bootstrap when Docker or Podman is reachable: Bitcoin Core RPC readiness,
  Bitcoin wallet setup, regtest mining, LND wallet init/unlock, LND funding,
  LND sync, tapd startup after LND credentials exist, tapd RPC readiness, and
  a secret-safe readiness report.
- The integrated Lightning Labs `litd` counterparty harness now starts the
  real asset-channel target topology: Bitcoin Core plus litd with integrated
  LND/taproot-assets, taproot overlay channels, RFQ mock oracle config, and the
  asset-channel RPC surface reachable.
- The live Path B gate now starts a native LDK node and connects it to that
  integrated `litd` node over the Lightning P2P address. This proves the
  Lightning Labs side is no longer only a JSON loopback target; the remaining
  gap is asset-channel funding/payment over that connected peer.
- The live `tapd` proof-binding path is wired. When the Lightning Labs daemon
  is reachable, `scripts/live-tapd-proof-bind.sh` mints `OPENUSD` through
  `tapcli`, mines confirmations, exports the TAPF proof, and binds that proof
  into native `tap-ldk` wallet state. The bounded CLI path and negative checks
  for wrong asset id, stale proof digest, and wrong owner binding are covered
  by tests.
- Path B now writes a live outgoing-payment gate artifact. It ties the live
  `tapd` proof-binding report to the native outgoing RFQ/invoice/HTLC artifact,
  records the ordered native asset-payment wire session, payment id, quote id,
  asset id, amount, expected balances, and the current `tapd` daemon balance
  when the counterparty is reachable. It refuses to mark issue #57 complete
  until the Lightning Labs receiver balance is observed after settlement.
- The OpenAgentsInc `rust-lightning` fork now has the first asset-channel
  feature/channel-type gate, bounded funding approval hook, and channel monitor
  aux blob surface for asset commitment state. It now also has BOLT simple
  taproot final/staging feature-bit definitions, explicit staging channel-type
  negotiation, fail-closed unsupported-peer tests, and a rule that the
  experimental Taproot Asset channel type must sit on the simple taproot
  staging base. It also has the first HTLC metadata/final-hop validation hook,
  cooperative close allocation hook, and force-close/sweep proof-ownership
  recovery hook. The native `tap-ldk` funding, commitment, HTLC, close, and
  recovery stores call those fork hooks before writing funded channel state,
  treating asset commitment state as restart-safe, settling asset HTLC
  metadata, exporting final close proofs, or reporting recovered asset proof
  ownership.

What does not work yet:

- `tap-ldk` does not yet complete a real live asset payment with the
  independent Lightning Labs `litd` node.
- The current Lightning Labs payment path connects a native LDK node to `litd`,
  but still stops before driving asset-channel funding/payment over that peer
  and before observed balance comparison from both sides.
- The standalone LND container starts without `simple-taproot-overlay-chans`;
  the Lightning Labs asset-channel payment path still needs the aux-controller
  overlay path from the Taproot Assets/Lit stack or equivalent integration.
- The ordered asset-payment message smoke is still localhost `tap-ldk` to
  `tap-ldk`. The separate native LDK peer preflight now connects to `litd`, but
  the message flow is not yet running over that connected daemon-backed peer.
- The live Lightning Labs checks run through Docker or Podman. The scripts now
  bound image pull and container startup time and write blocked reports when
  the independent regtest Bitcoin Core, LND, and `tapd` counterparty cannot
  start.
- Full semantic Taproot Assets proof ancestry validation is still open; the
  live proof binding preserves and binds TAPF material with bounded anchor
  checks until issue #60 lands.
- Live on-chain force-close and sweeper integration is not implemented yet. The
  bounded recovery smoke now proves that `tap-ldk` refuses to call an asset
  recovered when only BTC sweep state exists, but it is not a live chain spend.
- LND, `tapd`, and `litd` are only test counterparties for interoperability.
  They are not sidecars inside the `tap-ldk` wallet.

What is being worked on now:

- Issues #48 through #56 have landed the first Rust Lightning fork gates for
  asset-channel negotiation, bounded funding approval, monitor aux blob
  persistence tied to asset commitment numbers, and HTLC metadata/final-hop
  validation, plus cooperative close allocation export and force-close/sweep
  proof-ownership recovery, plus the first live localhost `tap-ldk` peer smoke
  and the hardened Lightning Labs counterparty bootstrap harness, plus the
  live `tapd` mint/export/bind command path.
- Issue #57 is active: the repo now has the live outgoing-payment gate, the
  local failure checks, the proof-binding handoff, and the ordered native
  asset-payment wire session, plus the current `tapd` balance observer, an
  integrated `litd` asset-channel counterparty, and a native LDK peer
  connection to that `litd` node. The remaining #57 work is to run the asset
  funding/payment flow over that connected peer and record the observed
  receiver-balance check after settlement.
- Issues #57 through #60 cover the remaining live demo path: payments in both
  directions, observed live balance checks, and full proof ancestry validation.
- Issue #62 is implemented and pinned in `tap-ldk`: the fork now negotiates
  simple taproot staging channel types explicitly and rejects unsupported
  required simple taproot channels. The next simple-taproot fork work is issue
  #63, the wire TLV codecs and message validation.
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
