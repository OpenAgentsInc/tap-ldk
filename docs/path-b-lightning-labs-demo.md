# Path B Lightning Labs Demo

Path B is the native `tap-ldk` to independent Lightning Labs interop demo. The
current harness captures version info, counterparty config/status, blob
fixtures, TAPF proof fixtures, funding interop, RFQ/invoice compatibility, both
payment directions, and the consolidated interop check report into an ignored
artifact directory.

```bash
./scripts/path-b-lightning-labs-demo.sh
```

Artifacts are written under `target/path-b-lightning-labs-demo/<timestamp>` by
default. Override with `TAP_LDK_PATH_B_ARTIFACT_DIR=/path/to/artifacts`.

If Docker or Podman is available, the script attempts the independent
Bitcoin Core/LND/`tapd` counterparty smoke with the selected Lightning Labs
target. If no runtime is available, or if the selected daemon/machine is down,
the script records an explicit dependency gap and still runs every
fixture-backed Track B check. LND and `tapd` remain compatibility peers, not
sidecars inside the `tap-ldk` wallet.

The current consolidated report can pass fixture-backed checks while still
showing `live_daemon_gaps_remaining=true`. That means live daemon settlement
and observed balance replacement are still required before Track B is a settled
interop success.
