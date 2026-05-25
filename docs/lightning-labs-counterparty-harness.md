# Lightning Labs Counterparty Harness

`scripts/lightning-labs-counterparty.sh` starts the Track B external
counterparty topology:

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
./scripts/lightning-labs-counterparty.sh connection
./scripts/lightning-labs-counterparty.sh stop
```

Smoke:

```bash
./scripts/lightning-labs-counterparty.sh smoke
```

The script skips with a clear message when Docker is not installed or the
Docker daemon is unavailable. Generated state and credentials live under
`.tap-ldk/regtest/lightning-labs` by default and are ignored by Git.

This harness is an interop counterparty only. It must not be wired into
`tap-ldk` as a wallet sidecar.
