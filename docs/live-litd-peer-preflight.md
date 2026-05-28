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
`OpenAgentsInc/rust-lightning@acce215e1ca284fa45f1c13e13760de459d410d4` and
`OpenAgentsInc/ldk-node@fefcc5747b84145b1c85a76fde13848b445ffc3a`, the
integrated Lightning Labs `litd` peer now advertises both simple-taproot and
Taproot Asset channel support, and the native peer advertises Lightning Labs
no-op HTLC aux support without advertising unimplemented STXO support. The live
outgoing-payment harness now moves past
readiness into integrated `litd` issuance, live asset-channel funding,
channel confirmation, and a keysend-usable local asset balance on `litd`.
Issue #81 still remains open because the live asset keysend stays `IN_FLIGHT`.
The current fork pin treats the peer HTLC signature bytes as BIP340 Schnorr,
keeps the earlier failing transcript as a regression fixture, and adds the
Lightning Labs second-level virtual-lock asset-leaf fields. The latest rerun
accepts `commitment_signed`, then stalls while monitor update `1` remains
incomplete and the held `revoke_and_ack`/local commitment response is not
released. The current #57 report treats this as a readiness and partial-live
gate until the monitor/message path, receiver claim, force-close witness path,
and observed balances pass.
