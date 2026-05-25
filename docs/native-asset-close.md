# Native Asset Close And Proof Export

Date: 2026-05-25

`tap-ldk` now has a bounded cooperative close path for the strong Path A demo.
The close uses the latest durable asset commitment, returns the exact local and
remote asset allocation, exports owner proofs tied to final close anchors, and
verifies those proofs by importing them into fresh wallets.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-close-smoke
```

The smoke pays `125` `OPENUSD` from Alice to Bob, cooperatively closes the
channel at `alice=575` and `bob=425`, exports both proofs, imports them into
wallets, and round-trips the close store to model restart after close. It also
rejects an obsolete proof from the prior commitment view.

`./scripts/path-a-native-demo.sh` captures the close evidence as standalone
artifacts:

- `native-close.json`: full cooperative close smoke report.
- `native-close-local-proof.hex`: local final-output proof export.
- `native-close-remote-proof.hex`: remote final-output proof export.
- `close-recovery-status.json`: restart, stale-proof, failed-sweep, and
  force-close gate status.

## Force Close Gate

Force-close and sweep recovery are explicitly deferred. Failed sweep state is
not reported as recovered, and the demo writes `force_close_supported=false`
until native force-close evidence exists.
