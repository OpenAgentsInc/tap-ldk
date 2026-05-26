# OpenAgentsInc LDK Node Fork

Date: 2026-05-26

The live Taproot Assets demo needs an owned `ldk-node` fork:

- Fork: `https://github.com/OpenAgentsInc/ldk-node`
- Upstream: `https://github.com/lightningdevkit/ldk-node`
- Current fork commit used by `tap-ldk`:
  `4b7d8de974a8b08ee8bfee94450dc5c332fe596c`
- Current `rust-lightning` fork commit:
  `cbc508b8ae972fd1134b0c5f1dc1792139276268`
- Tracking issues: #77, #78, #79, #80, #81

## Why This Fork Exists

The current #57 live preflight uses the OpenAgentsInc `ldk-node` fork. That
proves a native LDK node can connect to integrated Lightning Labs `litd` and
that the runtime is built against the OpenAgentsInc `rust-lightning` fork, but
it does not prove Taproot Asset channel settlement yet.

For #57 to settle honestly, the live node runtime must now expose the forked
channel configuration, custom message path, asset-channel open path, and asset
payment path. A direct lower-level `rust-lightning` node is possible, but the
narrow `ldk-node` fork is the chosen route because it preserves node lifecycle,
chain sync, persistence, peer management, wallet plumbing, and normal BTC smoke
coverage.

## Fork Scope

1. #77 created and documented `OpenAgentsInc/ldk-node`.
2. #78 pinned that fork to the OpenAgentsInc `rust-lightning` fork revision
   used by `tap-ldk` and added `ldk_node::provenance`.
3. #79 exposes simple-taproot and Taproot Asset channel config while keeping
   BTC-only defaults unchanged.
4. #80 wires proof, funding, RFQ, quote, and asset HTLC messages plus typed
   asset-channel open/payment APIs.
5. #81 replaces the current provenance-only preflight with fork-backed live
   settlement against independent integrated Lightning Labs `litd`.

## Invariants

- `ldk-node` is a live node runtime dependency, not a wallet sidecar.
- LND, `tapd`, and `litd` remain independent interop peers.
- Taproot Asset channel negotiation cannot be enabled without the
  simple-taproot base.
- BTC-only channel behavior must remain unchanged by default.
- Asset-channel state must be durable before the matching Lightning commitment
  is treated as safe.

## Verification

The fork path is not done until these checks pass:

```bash
gh repo view OpenAgentsInc/ldk-node
cargo tree -i lightning@0.3.0+git -e no-dev
cargo metadata --format-version 1
cargo test
cargo test -p tap-ldk-core live_litd_peer -- --nocapture
TAP_LDK_LL_WAIT_TIMEOUT_SECONDS=240 ./scripts/live-lightning-labs-outgoing-payment.sh target/live-lightning-labs-outgoing-payment/report.json target/live-lightning-labs-outgoing-payment/wallet.json
```

The final live report must show that the live node uses the OpenAgentsInc
`rust-lightning` fork and records observed post-settlement balances, not only a
peer connection.
