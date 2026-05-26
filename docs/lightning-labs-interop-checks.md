# Lightning Labs Interop Checks

The Track B interop check smoke runs the fixture-backed funding state, tapd
proof fixture decode, `tap-ldk` to Lightning Labs payment artifacts, and
Lightning Labs to `tap-ldk` payment artifacts as one report. It also decodes
the Lightning Labs HTLC metadata fixture and runs the fork-backed
simple-taproot asset-channel lifecycle smoke. It compares asset IDs, amounts,
expected balance deltas, proof availability, RFQ message types, HTLC RFQ
metadata, close/proof recovery, metadata rejection checks, and restart round
trips. Any failed comparison includes the side, field, expected value, actual
value, and related artifact path.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-interop-check-smoke fixtures/lightning-labs/tapchannelmsg/testdata fixtures/lightning-labs/proof/testdata target/lightning-labs-interop-checks.json
```

The report can pass its automated fixture checks while still setting
`live_daemon_gaps_remaining=true`. That is intentional: current outgoing and
incoming payment checks still stop at expected balance deltas until a live
LND/`tapd` counterparty reports observed settlement and durable balances.

## Checks

- Funding local plus remote balances equal the decoded funding total.
- Funding, outgoing payment, and incoming payment stores serialize and reload
  unchanged.
- Funding proof material and TAPF proof-file material are present and decode.
- Lightning Labs HTLC fixture metadata carries an RFQ ID.
- Both payment directions use Lightning Labs RFQ request/accept/reject message
  types.
- Both payment directions use the funding asset ID and conserve expected
  balances.
- The simple-taproot asset-channel lifecycle, cooperative close proof export,
  and proof-ownership recovery checks pass through the OpenAgentsInc
  rust-lightning fork state.
- Wrong, stale, malformed, and replayed payment metadata checks remain true.
- Live observed balance gaps are recorded as documented gaps, not success.
