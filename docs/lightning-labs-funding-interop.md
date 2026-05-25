# Lightning Labs Funding Interop

The current Track B funding interop step is fixture-backed. `tap-ldk` decodes
the Lightning Labs `tapchannelmsg` funding and commitment blobs, confirms they
agree on asset ID, funded amount, and initial local/remote allocation, then
persists that state in a restart-safe interop store.

This deliberately stops at a documented gap: the static fixtures prove the
funding blob shape and commitment allocation, but they do not perform the live
LND/`tapd` custom-channel funding handshake or bind a fresh funding outpoint
to a fully verified proof chain. The live headless or Polar-backed counterparty
flow must close that gap before Track B can claim a funded channel.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-funding-interop-smoke fixtures/lightning-labs/tapchannelmsg/testdata target/lightning-labs-funding-interop.json
```

Current fixture values:

- asset id: `5bbcbdf00f8e1065384efef9286646ca3b9765458df9a22baa1b1bd3bb75bf71`
- funded amount: `100000000000`
- local balance: `56700021068`
- remote balance: `43299978932`

The stored status is `stopped_at_documented_gap`, not a live channel-open
success.
