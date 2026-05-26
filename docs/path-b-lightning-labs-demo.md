# Path B Lightning Labs Demo

Path B is the native `tap-ldk` to independent Lightning Labs interop demo. The
current harness captures version info, counterparty config/status, blob
fixtures, TAPF proof fixtures, the live localhost `tap-ldk` peer smoke, funding
interop, RFQ/invoice compatibility, both payment directions, the live `tapd`
proof-binding report, and the consolidated interop check report into an
ignored artifact directory.

```bash
./scripts/path-b-lightning-labs-demo.sh
```

Artifacts are written under `target/path-b-lightning-labs-demo/<timestamp>` by
default. Override with `TAP_LDK_PATH_B_ARTIFACT_DIR=/path/to/artifacts`.

If Docker or Podman is available, the script attempts the independent Bitcoin
Core/LND/`tapd` counterparty smoke with the selected Lightning Labs target.
That smoke now includes daemon readiness, LND wallet init/unlock, regtest
mining, LND funding, LND sync, tapd startup ordering, and tapd readiness. If no
runtime is available, or if the selected daemon/machine is down, the script
records the runtime prerequisite and still runs every fixture-backed Track B
check. LND and `tapd` remain compatibility peers, not sidecars inside the
`tap-ldk` wallet.

The Path B wrapper also runs `scripts/live-tapd-proof-bind.sh`. With a live
daemon, that script mints `OPENUSD`, exports a TAPF proof from `tapd`, and
binds it into native wallet state. Without a reachable runtime it writes a
blocked JSON report at `live-tapd-proof-binding.json`.

The current consolidated report can pass fixture-backed checks while still
showing `live_daemon_gaps_remaining=true`. That means live daemon settlement
and observed balance replacement are still required before Track B is a settled
interop success.

The live peer smoke is local `tap-ldk` to `tap-ldk`: it starts a real listener,
connects a second peer, negotiates the asset-channel capability through the
OpenAgentsInc rust-lightning fork, and sends an encoded native RFQ custom
message over the socket. It is not yet a Lightning Labs daemon-backed P2P
session.
