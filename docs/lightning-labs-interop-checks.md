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

Current live status is more specific: the #57/#81 gate can start the relevant
Lightning Labs stacks, bind a live proof, run the native ordered
asset-payment-session smoke, connect fork-backed `ldk-node` to integrated
`litd`, exercise the fork-backed asset message/channel/payment APIs, settle
Lightning Labs to native asset keysend, and record the native receiver asset
balance. It still does not have the true native-to-Lightning Labs receiver
balance delta, and it still records live cooperative close as a documented gap
until native post-close proof and balance observation exists. #59 should only
flip the Path B completion flag after #57 and #58 both record observed
post-settlement balances.

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
  latest allocation preservation, close-store restart, and proof-ownership
  recovery checks pass through the OpenAgentsInc rust-lightning fork state.
- Wrong, stale, malformed, and replayed payment metadata checks remain true.
- Live observed balance gaps and live Lightning Labs cooperative-close
  post-close observation are recorded as documented gaps, not success.

## Closure Gate

- #57 must provide the live `tap-ldk` pays Lightning Labs observed receiver
  balance.
- #58 must provide the live Lightning Labs pays `tap-ldk` observed durable
  receiver balance.
- #59 must make `live_daemon_gaps_remaining=false` impossible unless both live
  directions agree on asset ID, amount, payment state, proof reference, and
  balances.
- #60 must replace shallow proof acceptance with semantic proof ancestry
  validation before the full protocol epics close.
