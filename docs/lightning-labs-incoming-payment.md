# Lightning Labs Incoming Payment

`tap-ldk` now builds the receiver-side artifacts for the second Track B
payment direction: a Lightning Labs counterparty paying a native `tap-ldk`
wallet. The smoke uses the imported Lightning Labs funding and commitment
fixtures, constructs a native quote-bound receive invoice, emits Lightning Labs
buy-direction RFQ request/accept payload digests, validates final-hop asset
HTLC metadata, computes the expected Lightning Labs sender and `tap-ldk`
receiver balance delta, and persists a restart-safe interop payment state.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-incoming-payment-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-incoming-payment.json
```

The stored status is `stopped_at_live_daemon_gap`. This is intentional: the
current smoke does not drive a live LND/`tapd` sender or observe a durable
`tap-ldk` receiver balance. It records the expected balance change and the
exact remaining gap instead of reporting a successful interop settlement.

This issue follows #57. Once `tap-ldk` can pay Lightning Labs over the live
asset-channel path, the reverse direction must expose the native receive path
to the Lightning Labs sender, validate the received asset HTLC metadata through
the LDK/fork boundary, persist the received proof reference and balance, and
prove restart does not lose that state.

## Checks

- Reuses the fixture-backed Lightning Labs funding state.
- Builds and validates Lightning Labs buy-direction RFQ request/accept
  payloads.
- Binds the RFQ to opaque BOLT 11 invoice text without changing the invoice.
- Encodes, decodes, and validates final-hop asset HTLC custom records.
- Rejects stale, wrong-amount, malformed, and replayed receive metadata.
- Persists the incoming payment gap state and reloads it unchanged.

## Next Step

Run these same RFQ, invoice, and HTLC artifacts through the headless or
Polar-backed Lightning Labs sender, then replace the expected `tap-ldk`
receiver balance with an observed durable settlement balance before claiming
Track B payment success. #59 should only close after both this observed balance
and the #57 Lightning Labs receiver balance are present in the Path B report.
