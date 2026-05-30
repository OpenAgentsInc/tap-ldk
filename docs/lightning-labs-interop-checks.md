# `lnd`/`tapd`/`litd` Interop Checks

The Track B interop check smoke runs the fixture-backed funding state, tapd
proof fixture decode, `tap-ldk` to `litd` payment artifacts, and
`litd` to `tap-ldk` payment artifacts as one report. It also decodes
the Lightning Labs software HTLC metadata fixture and runs the fork-backed
simple-taproot asset-channel lifecycle smoke. It compares asset IDs, amounts,
expected balance deltas, proof availability, RFQ message types, HTLC RFQ
metadata, close/proof recovery, metadata rejection checks, and restart round
trips. Any failed comparison includes the side, field, expected value, actual
value, and related artifact path.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-interop-check-smoke fixtures/lightning-labs/tapchannelmsg/testdata fixtures/lightning-labs/proof/testdata target/lightning-labs-interop-checks.json
```

The fixture report can pass its automated checks while still setting
`live_daemon_gaps_remaining=true`. That remains intentional for the fixture
smoke: outgoing and incoming payment artifacts are expected-delta checks. The
Path B wrapper now writes `path-b-completion-report.json` as the live gate that
consumes observed daemon/channel balances.

Current live status is more specific: the completed #81, #57, and #58 gates
start the relevant `lnd`/`tapd`/`litd` stacks, bind a live proof, connect
fork-backed `ldk-node` to integrated `litd`, exercise the fork-backed asset
message/channel/payment APIs, settle `litd` to native asset keysend, record the
native receiver asset balance, return the asset from native LDK to `litd`, and
record the returned `litd` channel-balance observation. #59 adds the wrapper
completion report that sets `live_daemon_gaps_remaining=false` only from those
live observed balances. Live cooperative close still remains a documented gap
until native post-close proof and balance observation exists.

## Checks

- Funding local plus remote balances equal the decoded funding total.
- Funding, outgoing payment, and incoming payment stores serialize and reload
  unchanged.
- Funding proof material and TAPF proof-file material are present and decode.
- Lightning Labs software HTLC fixture metadata carries an RFQ ID.
- Both `litd` interop directions use taproot-assets RFQ request/accept/reject message
  types.
- Both payment directions use the funding asset ID and conserve expected
  balances.
- The simple-taproot asset-channel lifecycle, cooperative close proof export,
  latest allocation preservation, close-store restart, and proof-ownership
  recovery checks pass through the OpenAgentsInc rust-lightning fork state.
- Wrong, stale, malformed, and replayed payment metadata checks remain true.
- Fixture-only observed balance gaps and live `litd` cooperative-close
  post-close observation are recorded as documented gaps, not success.

## Closure Gate

- #57 provides the live native-to-`litd` observed receiver balance.
- #58 provides the live `litd`-to-native observed durable receiver balance and
  restart snapshot.
- #59 makes `live_daemon_gaps_remaining=false` impossible unless both live
  directions agree on asset ID, amount, payment state, proof reference, and
  balances.
- #60 replaces shallow proof acceptance with semantic proof ancestry validation
  at the first-demo boundary; keep those tests green before closing the full
  protocol epics.
