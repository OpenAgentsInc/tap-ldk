# Live litd Peer Preflight

Date: 2026-05-26

`tap-ldk` now has a fork-backed native LDK peer preflight for the Lightning
Labs Path B target. The command starts the OpenAgentsInc `ldk-node` fork on
regtest, connects it to the integrated `litd` node ID and P2P
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
artifact as `native-ldk-litd-peer-preflight.json`. During the hold-mode live
run, that report is refreshed while the native node is running, including the
fork-backed `ldk-node` Taproot Asset channel and payment records. This lets the
shell report prove that a live `litd` payment was claimed and persisted
by the native receiver instead of only proving peer connectivity.

This preflight is no longer the whole #57 story by itself; it is the live node
state source used by the #57 regression gate. It proves connectivity, fork
provenance, opt-in asset-channel negotiation config, remote feature
observation, and the #80 typed API surface. With
`OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269` and
`OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`, the
integrated `litd` peer now advertises both simple-taproot and
Taproot Asset channel support, and the native peer advertises the
taproot-assets no-op HTLC aux feature without advertising unimplemented STXO
support. The live
payment harness now moves past readiness into integrated `litd` issuance, live
asset-channel funding, `litd` to native asset keysend, native
`PaymentClaimed`, durable native receiver balance recording, and the reverse
native-to-`litd` asset send. The current pin adds post-claim balance-output
aux-leaf placement for claimed full-amount asset HTLCs, clears the live
zero-HTLC post-claim partial-signature failure, carries the #84 force-close
funding-input key-path witness fix, and uses canonical Lightning Labs HTLC blob
encoding plus a dust-covering BTC amount for the reverse leg. It also carries
#93 cooperative-close RBF nonce rotation in the fork line, though live
post-close proof/balance observation remains a separate documented boundary.
The latest live run reports `issue_57_acceptance_met=true`.
