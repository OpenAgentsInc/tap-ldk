# Headless Bitcoin Regtest Harness

`scripts/regtest-bitcoin.sh` starts a disposable Bitcoin Core regtest node for
local and CI smoke tests. It uses Docker when available and skips clearly when
Docker is missing or the daemon is unavailable.

Default connection material:

```bash
cargo run -p tap-ldk-cli -- regtest-bitcoin-config
```

Lifecycle commands:

```bash
./scripts/regtest-bitcoin.sh start
./scripts/regtest-bitcoin.sh mine 1
./scripts/regtest-bitcoin.sh status
./scripts/regtest-bitcoin.sh stop
```

Smoke command:

```bash
./scripts/regtest-bitcoin.sh smoke
```

Local state lives under `.tap-ldk/regtest/bitcoin` by default and is ignored by
Git. Override the image, container name, RPC material, or state directory with
the `TAP_LDK_BITCOIN_*` and `TAP_LDK_REGTEST_DIR` environment variables listed
by `./scripts/regtest-bitcoin.sh --help`.

This harness is infrastructure only. It does not implement wallet behavior and
does not make Bitcoin Core, LND, `tapd`, or Polar a sidecar for `tap-ldk`.
