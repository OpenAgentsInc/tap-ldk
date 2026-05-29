# Simple-Taproot Cooperative Close Proof

Date: 2026-05-28

## Current Result

Native BTC simple-taproot cooperative close is now covered in the
OpenAgentsInc `rust-lightning` fork at
`1e7b435a015dafb5cc314c135e2eebab18cf460f`. The focused shutdown test opens a
simple-taproot channel, exchanges `shutdown`, `closing_complete`, and
`closing_sig`, checks that both peers broadcast the same final transaction, and
asserts that the final transaction spends the P2TR funding output with exactly
one 64-byte Schnorr witness element.

Taproot Asset cooperative-close state is covered in `tap-ldk-core` by the
simple-taproot asset-channel smoke. The report now records that the close:

- exports local and remote final-owner proofs;
- validates the close allocation through the rust-lightning fork state;
- preserves the latest local amount, remote amount, total amount, proof root,
  and commitment number from the latest valid channel state;
- survives a close-store round trip and reload with the same allocation.

The consolidated Lightning Labs software interop check treats those native and
fixture-backed close/recovery vectors as passing automated checks.

## `litd` Close Boundary

The `lnd`/`tapd`/`litd` stack exposes the peer side of asset cooperative close
through LND `CloseChannel` and the taproot-assets custom-channel closer. The
relevant reference code is in the local synced Lightning Labs repo:

- `projects/lightninglabs/repos/taproot-assets/tapchannel/aux_closer.go`
- `projects/lightninglabs/repos/taproot-assets/itest/custom_channels/restart_coop_close_test.go`
- `projects/lightninglabs/repos/taproot-assets/itest/custom_channels/helpers.go`

Those references show `AuxChanCloser`, shutdown blob creation, close output
handling, final close handling, and a restart-before-confirmation cooperative
close test.

`tap-ldk` now exposes the corresponding live harness command:

```bash
./scripts/lightning-labs-litd-counterparty.sh close-asset-channel '<txid:index>' false
```

That command calls integrated `litd`/LND `closechannel`, captures stdout,
stderr, exit code, and parsed JSON output when available. This is the correct
live operation to run after `asset-channel-status` returns a channel point.

The live Path B harness still does not claim completed live cooperative close.
The missing observation is native-side post-close Taproot Asset proof and
balance recording after the `litd` close completes and after restart. Until
that observer exists, the interop report records
`live litd cooperative close remains documented gap` rather than
reporting false success.

## Verification

```bash
./scripts/check-simple-taproot-cooperative-close.sh
cargo run -p tap-ldk-cli -- asset-close-smoke
cargo run -p tap-ldk-cli -- simple-taproot-asset-channel-smoke
cargo run -p tap-ldk-cli -- lightning-labs-interop-check-smoke fixtures/lightning-labs/tapchannelmsg/testdata fixtures/lightning-labs/proof/testdata target/lightning-labs-interop-checks.json
```

The BOLT simple-taproot tracker now has #92 BTC-level splice nonce-map coverage.
Asset-channel splice/RBF remains out of the first public demo until the asset
state and proof transitions are covered.
