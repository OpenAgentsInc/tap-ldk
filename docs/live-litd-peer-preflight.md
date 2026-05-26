# Live litd Peer Preflight

Date: 2026-05-26

`tap-ldk` now has a fork-backed native LDK peer preflight for the Lightning
Labs Path B target. The command starts the OpenAgentsInc `ldk-node` fork on
regtest, connects it to the integrated Lightning Labs `litd` node ID and P2P
address, records the native node ID, and verifies that LDK reports the `litd`
peer as connected. The report also records the OpenAgentsInc `rust-lightning`
revision reported by `ldk_node::provenance` and the live node's opt-in
simple-taproot plus Taproot Asset channel negotiation flags. It also exercises
the fork-backed typed Taproot Asset custom-message, asset-channel open, and
asset-payment APIs with synthetic regtest state before attempting the live
peer connection.

```bash
cargo run -p tap-ldk-cli -- live-litd-peer-preflight target/live-litd-peer-preflight.json target/live-litd-peer-preflight-state '<litd-node-id>' '127.0.0.1:29735'
```

The live outgoing-payment gate fills the `litd` node ID and address from
`scripts/lightning-labs-litd-counterparty.sh start` and writes the preflight
artifact as `native-ldk-litd-peer-preflight.json`.

This is still not issue #57 completion. It proves connectivity, fork
provenance, opt-in asset-channel negotiation config, and the #80 typed API
surface. #81 replaces this pre-settlement gate with live asset-channel
funding/payment against the independent `litd` counterparty. The current #57
report treats this as a readiness gate, not as live settlement.
