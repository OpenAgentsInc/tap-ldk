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
OpenAgentsInc `ldk-node` runtime to that `litd` peer, and keeps issue #57
marked incomplete until the native LDK asset payment path settles against the
independent Lightning Labs litd receiver with an observed receiver balance.

Current #57 status is narrower than the old runtime prerequisite. The gate can
reach live proof binding, native asset-payment session readiness, integrated
`litd` readiness, fork-backed `ldk-node` to `litd` peer connection, and a
pre-settlement Lightning Labs current-balance observation. It also records
whether `litd` advertised the taproot features needed for asset channels. It
now completes live asset-channel funding, confirms the channel, and sees
`litd` report a keysend-usable local asset balance. It still stops at
`live_asset_channel_payment_settlement` because the live asset keysend remains
`IN_FLIGHT` after Rust Lightning closes on a later payment-time simple-taproot
commitment partial-signature check. The next work is dynamic Taproot Asset
commitment output construction for payment-time channel states, then
post-settlement balance observation.

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
support, and reach typed asset-channel message/payment APIs. Issue #81 still
has to run those APIs through daemon-backed live funding/payment settlement.

Open issue path:

1. #81: use fork-backed `OpenAgentsInc/ldk-node` asset-channel
   message/payment APIs for live settlement.
2. #57: live `tap-ldk` pays Lightning Labs and records post-settlement receiver
   balance.
3. #58: live Lightning Labs pays `tap-ldk` and `tap-ldk` persists the received
   balance across restart.
4. #59: Path B reports require observed live balances in both directions.
5. #60: semantic proof ancestry validation replaces the remaining bounded
   proof boundary.
6. #19 closes only after those live and semantic gates pass.
