# `litd` to `tap-ldk` Incoming Payment

`tap-ldk` now builds the receiver-side artifacts for the second Track B
payment direction: a `lnd`/`tapd`/`litd` counterparty paying a native `tap-ldk`
wallet. The smoke uses the imported Lightning Labs software funding and
commitment fixtures, constructs a native quote-bound receive invoice, emits
taproot-assets buy-direction RFQ request/accept payload digests,
validates final-hop asset
HTLC metadata, computes the expected `litd` sender and `tap-ldk`
receiver balance delta, and persists a restart-safe interop payment state.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-incoming-payment-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-incoming-payment.json
```

The fixture smoke still stores `stopped_at_live_daemon_gap` because it is a
bounded artifact builder, not a live daemon test. The live incoming direction
now runs through `./scripts/live-lightning-labs-outgoing-payment.sh`: integrated
`litd` funds the asset channel, pays native LDK, native LDK records the settled
remote-to-local asset payment, then the harness restarts the native storage and
verifies the received payment/balance checkpoint reloads. The latest #58 run is
`target/live-lightning-labs-outgoing-payment-issue58-rerun/report.json` with
`issue_58_acceptance_met=true`.

## Checks

- Reuses the fixture-backed Lightning Labs software funding state.
- Builds and validates taproot-assets buy-direction RFQ request/accept
  payloads.
- Binds the RFQ to opaque BOLT 11 invoice text without changing the invoice.
- Encodes, decodes, and validates final-hop asset HTLC custom records.
- Rejects stale, wrong-amount, malformed, and replayed receive metadata.
- Persists the incoming payment gap state and reloads it unchanged.
- The live gate includes
  `native-ldk-litd-peer-restart-snapshot.json`, which proves the received
  `litd` payment survives a native-node restart.

## Next Step

#58 and #59 are complete. The consolidated Path B completion report consumes
the observed live balances and non-secret proof/payment references from #57 and
#58 instead of allowing expected-only fixture values to read as live interop
success. #60 now enforces the semantic proof ancestry boundary and must stay
green as a regression.
