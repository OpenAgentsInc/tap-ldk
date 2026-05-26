# Lightning Labs Counterparty Harness

`scripts/lightning-labs-counterparty.sh` starts the Track B external
counterparty topology with Docker or Podman:

- Bitcoin Core via `polarlightning/bitcoind:30.0`
- LND via `polarlightning/lnd:0.19.0-beta`
- `tapd` via `polarlightning/tapd:0.7.0-alpha`

These versions match the first Track B target in
`docs/lightning-labs-interop-matrix.md` and Polar's compatibility data.

Commands:

```bash
cargo run -p tap-ldk-cli -- lightning-labs-counterparty-config
./scripts/lightning-labs-counterparty.sh start
./scripts/lightning-labs-counterparty.sh status
./scripts/lightning-labs-counterparty.sh ready
./scripts/lightning-labs-counterparty.sh connection
./scripts/lightning-labs-counterparty.sh stop
```

Smoke:

```bash
./scripts/lightning-labs-counterparty.sh smoke
```

The script prefers Docker, including the Docker Desktop app bundle CLI, and
falls back to Podman. Set `TAP_LDK_CONTAINER_RUNTIME=docker`,
`TAP_LDK_CONTAINER_RUNTIME=podman`, or a runtime binary path to force one.
Generated state and credentials live under `.tap-ldk/regtest/lightning-labs`
by default and are ignored by Git.

`start` now performs the ordered bootstrap needed before Path B can talk to a
live Lightning Labs counterparty:

- start Bitcoin Core and wait for RPC;
- create or load the regtest mining wallet;
- mine enough blocks for spendable regtest funds;
- start LND and wait for TLS material;
- initialize or unlock the LND wallet through the wallet-unlocker REST API;
- wait for LND admin macaroon and chain sync;
- fund the LND wallet and mine confirmations;
- start `tapd` only after LND credentials exist;
- wait for `tapd` TLS, macaroon, and `getinfo`.

`ready` and `start` print JSON with container names, images, node pubkeys,
chain heights, sync flags, wallet balance, and cert/macaroon paths. The report
does not print RPC or wallet password values. If the selected runtime is not
reachable, the command exits with a direct prerequisite message.

This harness is an interop counterparty only. It must not be wired into
`tap-ldk` as a wallet sidecar.
