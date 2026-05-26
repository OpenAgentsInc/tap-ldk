# Live litd Peer Preflight

Date: 2026-05-26

`tap-ldk` now has a native LDK peer preflight for the Lightning Labs Path B
target. The command starts an upstream `ldk-node 0.7.0` node on regtest,
connects it to the integrated Lightning Labs `litd` node ID and P2P address,
records the native node ID, and verifies that LDK reports the `litd` peer as
connected.

```bash
cargo run -p tap-ldk-cli -- live-litd-peer-preflight target/live-litd-peer-preflight.json target/live-litd-peer-preflight-state '<litd-node-id>' '127.0.0.1:29735'
```

The live outgoing-payment gate fills the `litd` node ID and address from
`scripts/lightning-labs-litd-counterparty.sh start` and writes the preflight
artifact as `native-ldk-litd-peer-preflight.json`.

This is still not issue #57 completion. It proves connectivity only. The live
runtime is not yet fork-backed `ldk-node`, so it cannot reach the
OpenAgentsInc `rust-lightning` simple-taproot and Taproot Asset channel hooks.
Issues #77 through #81 track the required `ldk-node` fork, forked
`rust-lightning` pin, config exposure, custom message/payment APIs, and
replacement of this upstream-runtime preflight with fork-backed live
settlement. The current #57 report treats this as a readiness gate, not as
live settlement.
