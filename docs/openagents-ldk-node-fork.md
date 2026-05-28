# OpenAgentsInc LDK Node Fork

Date: 2026-05-26

The live Taproot Assets demo needs an owned `ldk-node` fork:

- Fork: `https://github.com/OpenAgentsInc/ldk-node`
- Upstream: `https://github.com/lightningdevkit/ldk-node`
- Current fork commit used by `tap-ldk`:
  `3264d96ee6dcbd37cec24473eac5982b1678a560`
- Current `rust-lightning` fork commit:
  `90212e54066a35ad982b338e7c2c152bf4fe0b0b`
- Tracking issues: #77, #78, #79, #80, #81

## Why This Fork Exists

The current #57 live preflight uses the OpenAgentsInc `ldk-node` fork. That
proves a native LDK node can connect to integrated Lightning Labs `litd`, that
the runtime is built against the OpenAgentsInc `rust-lightning` fork, that the
live node opts into simple-taproot plus Taproot Asset channel negotiation, and
that the fork exposes typed Taproot Asset message/channel/payment APIs. It
now also records the native receiver-side Taproot Asset payment and balance
for the live Lightning Labs to native keysend path.

For #57 to settle honestly, the remaining work is the reverse direction:
native `tap-ldk` must pay the independent Lightning Labs peer and record the
Lightning Labs receiver balance delta. A direct lower-level `rust-lightning`
node is possible, but the
narrow `ldk-node` fork is the chosen route because it preserves node lifecycle,
chain sync, persistence, peer management, wallet plumbing, and normal BTC smoke
coverage.

## Fork Scope

1. #77 created and documented `OpenAgentsInc/ldk-node`.
2. #78 pinned that fork to the OpenAgentsInc `rust-lightning` fork revision
   used by `tap-ldk` and added `ldk_node::provenance`.
3. #79 exposed simple-taproot and Taproot Asset channel config while keeping
   BTC-only defaults unchanged and failing closed when Taproot Asset
   negotiation is enabled without simple taproot.
4. #80 wires proof, funding, RFQ, quote, and asset HTLC messages plus typed
   asset-channel open/payment APIs. Follow-up fork commits through
   `3264d96ee6dcbd37cec24473eac5982b1678a560` pin
   `OpenAgentsInc/rust-lightning@90212e54066a35ad982b338e7c2c152bf4fe0b0b`,
   advertise the Taproot Assets aux Init TLV `65545` with the Lightning Labs
   aux feature bit for no-op HTLCs, align Taproot Asset overlay negotiation
   with Lightning Labs `taproot-overlay-chans`, and expose connected-peer
   taproot feature support to the live preflight. The current pin also carries
   live `commitment_signed` asset-signature blob preservation, validation,
   HTLC transcript fixture coverage, and second-level virtual-lock asset-leaf
   encoding, full counterparty commitment monitor persistence, and exact
   previous-output-bound second-level HTLC aux leaves from the Rust Lightning
   fork. It also reports the claimed-HTLC balance-output fix in the runtime
   provenance path and carries BOLT simple-taproot zero legacy signature-field
   serialization/rejection for funding and commitment messages. The fork does
   not advertise STXO support until native STXO commitment leaves are
   implemented and verified.
5. #81 now uses fork-backed live settlement against independent integrated
   Lightning Labs `litd`. The latest completed live run settled the Lightning
   Labs to native direction and recorded native receiver balance. The current
   pin adds post-claim balance-output aux-leaf placement for claimed
   full-amount asset HTLCs, fixes the legacy signature-field wire rule, and
   clears the live zero-HTLC post-claim partial-signature failure with a
   regression fixture. #81 remains open until the force-close fallback path is
   fixture-backed and clean.

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
