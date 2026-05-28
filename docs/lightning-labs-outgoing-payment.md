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

The fixture-only bounded artifact still records the expected sender/receiver
balance changes. The live gate now runs the integrated `litd` counterparty,
opens a live asset channel to the OpenAgentsInc `ldk-node` fork, proves the
Lightning Labs to native direction, then has native LDK send the asset back to
`litd` over the same channel. The reverse leg uses the canonical Lightning
Labs Taproot Asset HTLC blob shape and a 354,000 msat BTC carrier amount so the
asset HTLC clears LND's dust check.

Current #57 state: `target/live-lightning-labs-outgoing-payment-issue57-final/report.json`
completed with `issue_57_acceptance_met=true`. The report shows integrated
`litd` fundchannel, litd-to-native settlement, native receiver accounting,
native-to-litd settlement, returned `litd` channel asset balance, replay and
wrong-metadata failures, and no invalid commitment or counterparty force-close
markers.

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
  outgoing payment artifact, native payment-session artifact, integrated `litd`
  readiness, fork-backed `ldk-node` peer preflight artifacts, post-payment
  channel status, and post-native-payment channel status.
- `issue_57_acceptance_met=true` requires #81 to remain green, native
  local-to-remote accounting to settle, observed `litd` channel asset balance
  to reflect the returned amount, replay and wrong-metadata checks to pass, and
  no invalid commitment or counterparty force-close markers.

## Next Step

#81, #57, and #58 are complete live regression gates. Keep
`./scripts/live-lightning-labs-outgoing-payment.sh` green, then implement the
#59 observed-balance completion gate and #60 semantic proof ancestry validation
before closing the Path B epic.
