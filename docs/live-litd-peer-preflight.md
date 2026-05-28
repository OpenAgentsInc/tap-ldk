# Live litd Peer Preflight

Date: 2026-05-26

`tap-ldk` now has a fork-backed native LDK peer preflight for the Lightning
Labs Path B target. The command starts the OpenAgentsInc `ldk-node` fork on
regtest, connects it to the integrated Lightning Labs `litd` node ID and P2P
address, records the native node ID, and verifies that LDK reports the `litd`
peer as connected. The report also records the OpenAgentsInc `rust-lightning`
revision reported by `ldk_node::provenance` and the live node's opt-in
simple-taproot plus Taproot Asset channel negotiation flags. It also records
whether the connected `litd` peer advertised simple-taproot staging and
Taproot Asset channel support, then exercises the fork-backed typed Taproot
Asset custom-message, asset-channel open, and asset-payment APIs with
synthetic regtest state before attempting the live peer connection.

```bash
cargo run -p tap-ldk-cli -- live-litd-peer-preflight target/live-litd-peer-preflight.json target/live-litd-peer-preflight-state '<litd-node-id>' '127.0.0.1:29735'
```

The live outgoing-payment gate fills the `litd` node ID and address from
`scripts/lightning-labs-litd-counterparty.sh start` and writes the preflight
artifact as `native-ldk-litd-peer-preflight.json`.

This is still not issue #57 completion. It proves connectivity, fork
provenance, opt-in asset-channel negotiation config, remote feature
observation, and the #80 typed API surface. With
`OpenAgentsInc/rust-lightning@85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1` and
`OpenAgentsInc/ldk-node@c5ae040bf84225922c5213d9acb077e031076a9c`, the
integrated Lightning Labs `litd` peer now advertises both simple-taproot and
Taproot Asset channel support, and the native peer advertises Lightning Labs
no-op HTLC aux support without advertising unimplemented STXO support. The live
outgoing-payment harness now moves past
readiness into integrated `litd` issuance, live asset-channel funding,
channel confirmation, and a keysend-usable local asset balance on `litd`.
Issue #81 still remains open because the live asset keysend stays `IN_FLIGHT`
after Rust Lightning closes on `Invalid simple-taproot HTLC signature from
peer`. The current fork pin attempts the first full-channel HTLC aux-leaf path
and treats the peer HTLC signature bytes as BIP340 Schnorr, but the latest live
run proves that the selected Lightning Labs HTLC signature leaf, sighash, or
key still does not match the peer's signed view. The current #57 report treats
this as a readiness and partial-live gate, not as live settlement.
