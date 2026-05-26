# OpenAgentsInc LDK Node Fork

Date: 2026-05-26

The live Taproot Assets demo needs an owned `ldk-node` fork:

- Fork: `https://github.com/OpenAgentsInc/ldk-node`
- Upstream: `https://github.com/lightningdevkit/ldk-node`
- Current upstream crate used by `tap-ldk`: `ldk-node 0.7.0`
- Tracking issues: #77, #78, #79, #80, #81

## Why This Fork Exists

The current #57 live preflight uses upstream `ldk-node 0.7.0`. That proves a
native LDK node can connect to integrated Lightning Labs `litd`, but it does
not prove Taproot Asset channel settlement. Upstream `ldk-node` depends on
upstream `lightning`, while the simple-taproot and Taproot Asset channel hooks
live in `OpenAgentsInc/rust-lightning`.

For #57 to settle honestly, the live node runtime must use the OpenAgentsInc
`rust-lightning` fork and expose the forked channel configuration, custom
message path, asset-channel open path, and asset payment path. A direct
lower-level `rust-lightning` node is possible, but a narrow `ldk-node` fork is
the faster route because it preserves node lifecycle, chain sync, persistence,
peer management, wallet plumbing, and normal BTC smoke coverage.

## Fork Scope

1. #77 creates and documents `OpenAgentsInc/ldk-node`.
2. #78 pins that fork to the OpenAgentsInc `rust-lightning` fork revision used
   by `tap-ldk`.
3. #79 exposes simple-taproot and Taproot Asset channel config while keeping
   BTC-only defaults unchanged.
4. #80 wires proof, funding, RFQ, quote, and asset HTLC messages plus typed
   asset-channel open/payment APIs.
5. #81 replaces the current upstream-runtime preflight with fork-backed live
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
cargo metadata --format-version 1
cargo test
cargo test -p tap-ldk-core live_litd_peer -- --nocapture
TAP_LDK_LL_WAIT_TIMEOUT_SECONDS=240 ./scripts/live-lightning-labs-outgoing-payment.sh target/live-lightning-labs-outgoing-payment/report.json target/live-lightning-labs-outgoing-payment/wallet.json
```

The final live report must show that the live node uses the OpenAgentsInc
`rust-lightning` fork and records observed post-settlement balances, not only a
peer connection.
