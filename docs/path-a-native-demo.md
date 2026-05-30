# Path A Native-To-Native Demo

Date: 2026-05-25

Run the bounded native-to-native demo from the repo root:

```bash
./scripts/path-a-native-demo.sh
```

The script writes artifacts under `target/path-a-native-demo/<timestamp>/` by
default. Override with `TAP_LDK_PATH_A_ARTIFACT_DIR=/path/to/dir`.

Expected abbreviated output:

```text
path-a-native-demo: artifacts=...
Path A native-to-native demo artifacts: ...
- local wallets issue and courier OPENUSD proof material
- native asset channel funds at alice=700 bob=300
- native payment settles 125 OPENUSD to bob
- cooperative close exports final proofs at alice=575 bob=425
- on-chain lifecycle report records close, bounded recovery, refusal, and
  restart evidence
- chain observation report binds that lifecycle evidence to bounded
  chain/sweeper observations
```

The command makes mocked pieces visible in `summary.txt`: bounded local issuer,
fixed regtest oracle, local proof handoff, and headless CLI UI. The receiver
proof is also exported as `bob-openusd-proof-bundle.json`, the typed local
proof-courier bundle used by the wallet import/export path. It does not use LND
or `tapd` for wallet duties.

Close/recovery artifacts are captured separately:

- `native-close-local-proof.hex`
- `native-close-remote-proof.hex`
- `close-recovery-status.json`
- `onchain-lifecycle.json`
- `chain-watcher-lifecycle.json`

`close-recovery-status.json` is the machine-visible force-close gate. It keeps
`force_close_supported=false` until a real force-close/sweep smoke exists.
`onchain-lifecycle.json` is the typed bounded report that explains the current
close/recovery evidence and marks the live chain-watcher boundary.
`chain-watcher-lifecycle.json` is the typed bounded observation report. It
binds lifecycle events to chain-watcher, sweeper, and wallet/monitor evidence,
but keeps live watcher and production readiness flags false.
