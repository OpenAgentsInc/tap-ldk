# Live tapd Proof Binding

This is the Path B command path for turning live Lightning Labs `tapd` output
into native `tap-ldk` wallet state.

```bash
./scripts/live-tapd-proof-bind.sh target/live-tapd-proof-binding/report.json target/live-tapd-proof-binding/wallet.json
```

When Docker or Podman is reachable, the script starts the independent
Lightning Labs counterparty, mints an `OPENUSD` normal asset through `tapcli`,
finalizes the minting batch, mines confirmations, exports the raw TAPF proof,
and imports that proof into the `tap-ldk` wallet through:

```bash
cargo run -p tap-ldk-cli -- live-tapd-proof-bind <wallet.json> <tapd-proof-file> <asset-id> <amount> <owner-script-key> <genesis-outpoint> <anchor-outpoint> <report.json>
```

The report records the live asset id, amount, wallet balance, proof id, anchor
outpoint, proof digest, and owner script key. It does not record private keys,
macaroon bytes, wallet passwords, or Bitcoin RPC passwords.

If the daemon runtime is unavailable, the script writes a `blocked` report with
the host prerequisite. Fixture proof import remains available through
`wallet-import-tapd-proof-file`; this live command is the daemon-backed path.
Full semantic proof ancestry validation is still tracked separately in issue
#60.
