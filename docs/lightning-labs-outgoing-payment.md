# Lightning Labs Outgoing Payment

`tap-ldk` now builds the sender-side artifacts for the first Track B payment
direction: a native `tap-ldk` wallet paying a Lightning Labs counterparty. The
smoke uses the imported Lightning Labs funding and commitment fixtures,
constructs the RFQ-bound invoice path, emits Lightning Labs RFQ request/accept
payload digests, validates asset HTLC metadata, computes the expected sender
and receiver balance delta, and persists a restart-safe interop payment state.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-outgoing-payment-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-outgoing-payment.json
./scripts/live-lightning-labs-outgoing-payment.sh target/live-lightning-labs-outgoing-payment/report.json target/live-lightning-labs-outgoing-payment/wallet.json
```

The stored status is `stopped_at_live_daemon_gap`. This is intentional: the
current smoke does not drive a live Lightning Labs receiver or observe the
Lightning Labs receiver balance after settlement. The live gate now also runs
the ordered native asset-payment wire session over a localhost `tap-ldk`
socket, asks standalone `tapd` for the current asset balance when reachable,
and starts the integrated `litd` counterparty that exposes the asset-channel
RPC surface. It then starts the OpenAgentsInc `ldk-node` fork and connects it
to that `litd` node over the Lightning P2P address. It records the expected
balance change and the exact remaining gap instead of reporting a successful
interop settlement.

Current #57 state: the gate reaches proof binding, native payment-session
readiness, integrated `litd` readiness, fork-backed `ldk-node` to `litd` peer
connection, and a pre-settlement Lightning Labs balance observation. That
balance is not the close condition, and the peer preflight still cannot settle
asset channels until #80 exposes the asset-channel message/payment APIs. #57
closes only after the live asset-channel
funding/payment flow runs over the connected `litd` peer and the report
records the Lightning Labs receiver balance after settlement.

## Checks

- Reuses the fixture-backed Lightning Labs funding state.
- Builds and validates Lightning Labs RFQ request/accept payloads.
- Binds the RFQ to opaque BOLT 11 invoice text without changing the invoice.
- Encodes and validates asset HTLC custom records before payment state can
  advance.
- Rejects quote replay, wrong asset metadata, and wrong amount metadata.
- Persists the outgoing payment gap state and reloads it unchanged.
- Runs the ordered native payment-session peer exchange for input proofs,
  output proofs, funding, RFQ, quote, and HTLC messages.
- Queries the current Lightning Labs `tapd` asset balance by asset ID when a
  live counterparty is available.
- Starts the integrated Lightning Labs `litd` counterparty with LND,
  taproot-assets, the aux funding controller, and asset-channel RPCs enabled.
- Starts a fork-backed `ldk-node` preflight node and connects it to the
  integrated `litd` node ID and P2P address.
- The live gate report links the live `tapd` proof-binding artifact to the
  outgoing payment artifact, native payment-session artifact, and integrated
  `litd` readiness and fork-backed `ldk-node` peer preflight artifacts, and keeps
  `issue_57_acceptance_met=false` until a real Lightning Labs receiver balance
  is observed after settlement.

## Next Step

Finish #80 and #81 first: wire asset messages/payment APIs and run the
asset-channel funding/payment flow over the connected independent Lightning
Labs `litd` peer. Then replace the
expected receiver balance with an observed post-settlement daemon balance,
implement #58, and let #59 turn both observed balances into the Path B
completion gate.
