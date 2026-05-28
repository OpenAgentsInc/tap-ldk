# Path B Lightning Labs Demo

Path B is the native `tap-ldk` to independent Lightning Labs interop demo. The
current harness captures version info, counterparty config/status, blob
fixtures, TAPF proof fixtures, the live localhost `tap-ldk` peer smoke, funding
interop, RFQ/invoice compatibility, both payment directions, the live `tapd`
proof-binding report, the integrated `litd` counterparty readiness report, the
fork-backed `ldk-node` to `litd` peer preflight report, the live outgoing-payment
gate, and the consolidated interop check report into an ignored artifact
directory. The
consolidated report now includes the HTLC RFQ metadata vector, Lightning Labs
RFQ message-type vectors, and the fork-backed simple-taproot asset-channel
lifecycle, close, and proof-recovery checks.

```bash
./scripts/path-b-lightning-labs-demo.sh
```

Artifacts are written under `target/path-b-lightning-labs-demo/<timestamp>` by
default. Override with `TAP_LDK_PATH_B_ARTIFACT_DIR=/path/to/artifacts`.

If Docker or Podman is available, the script attempts the independent Bitcoin
Core/LND/`tapd` counterparty smoke with the selected Lightning Labs target.
That smoke now includes daemon readiness, LND wallet init/unlock, regtest
mining, LND funding, LND sync, tapd startup ordering, and tapd readiness. The
live outgoing-payment gate also starts an integrated `litd` counterparty,
because real asset-channel settlement needs Lightning Labs' aux funding
controller with taproot overlay channels enabled. If no runtime is available,
or if the selected daemon/machine is down, the script records the runtime
prerequisite and still runs every fixture-backed Track B check. LND, `tapd`,
and `litd` remain compatibility peers, not sidecars inside the `tap-ldk`
wallet.

The Path B wrapper also runs `scripts/live-tapd-proof-bind.sh`. With a live
daemon, that script mints `OPENUSD`, exports a TAPF proof from `tapd`, and
binds it into native wallet state. Without a reachable runtime it writes a
blocked JSON report at `live-tapd-proof-binding.json`.

The wrapper also runs `scripts/live-lightning-labs-outgoing-payment.sh`. That
gate links live proof binding to the sender-side RFQ/invoice/HTLC artifact,
starts the integrated `litd` counterparty, connects the fork-backed
OpenAgentsInc `ldk-node` runtime to that `litd` peer, and now completes the
bidirectional live payment regression: `litd` pays native LDK, native LDK
records the received asset, native LDK sends the asset back to `litd`, and the
report observes the returned `litd` channel asset balance.

Current #57 status: complete. The latest passing artifact is
`target/live-lightning-labs-outgoing-payment-issue57-final/report.json` with
`issue_57_acceptance_met=true` and `issue_81_acceptance_met=true`. The reverse
native-to-`litd` leg uses a canonical Taproot Asset HTLC blob and a 354,000
msat BTC carrier amount so the asset HTLC is above LND's dust floor. The report
has no invalid-commitment or counterparty force-close markers.

The current consolidated report can pass fixture-backed checks while still
showing `live_daemon_gaps_remaining=true`. That means live daemon settlement
and observed balance replacement are still required before Track B is a settled
interop success.

The live peer smoke is local `tap-ldk` to `tap-ldk`: it starts a real listener,
connects a second peer, negotiates the asset-channel capability through the
OpenAgentsInc rust-lightning fork, and sends an encoded native RFQ custom
message over the socket. It is not yet a Lightning Labs daemon-backed P2P
session. The `litd` peer preflight now proves that the OpenAgentsInc
`ldk-node` fork can connect to integrated `litd`, report the OpenAgentsInc
`rust-lightning` revision, opt into simple-taproot/Taproot Asset channel
negotiation locally, observe remote simple-taproot and Taproot Asset channel
support, reach typed asset-channel message/payment APIs, and complete the #81
live Lightning Labs to native settlement gate plus the #57 native-to-`litd`
return payment gate.

Open issue path:

1. #58: live Lightning Labs pays `tap-ldk` and `tap-ldk` persists the received
   balance across restart.
2. #59: Path B reports require observed live balances in both directions.
3. #60: semantic proof ancestry validation replaces the remaining bounded
   proof boundary.
4. #19 closes only after those live and semantic gates pass.
