# Native Asset Close And Proof Export

Date: 2026-05-25

`tap-ldk` now has a bounded cooperative close path for the strong Path A demo.
The close uses the latest durable asset commitment, returns the exact local and
remote asset allocation, exports owner proofs tied to final close anchors, and
verifies those proofs by importing them into fresh wallets. The close path now
also validates the final allocation through the OpenAgentsInc rust-lightning
cooperative-close hook and exports the resulting allocation digest.

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

The close smoke remains cooperative-close focused. Force-close proof ownership
is now covered by `asset-recovery-smoke`, which validates commitment,
second-level HTLC, and final sweep proof-ownership records through the
OpenAgentsInc rust-lightning fork and refuses BTC-only sweep state as asset
recovery. Live on-chain resolver and sweeper integration is still pending.
