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
connection/API preflight, remote taproot feature observation, live
asset-channel funding, and a settled Lightning Labs to native asset keysend.
That proves the receiver side can claim and persist asset balance, but #57 is
still false because the live sender in this script is `litd`. #57 closes only
after native `tap-ldk` pays the connected Lightning Labs peer and the report
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

Finish #81 first: the live Lightning Labs to native payment now reports
`SUCCEEDED`, native LDK claims it, and fork-backed `ldk-node` records the
native receiver asset balance. The current fork handles the post-claim
zero-HTLC asset commitment-sig blob and derives the claimed asset HTLC's
post-claim balance-output aux leaf with the correct CSV-bound script key. The
latest live rerun no longer logs the post-claim partial-signature failure or an
invalid Taproot control block; the #84 funding-input force-close witness path
is now key-path and fixture-backed. Next, keep #81's live settlement report
green, then implement the true native `tap-ldk` to Lightning Labs payment
direction for #57, then #58, and the #59 observed-balance completion gate.
