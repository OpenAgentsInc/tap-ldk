# Lightning Labs litd Counterparty

`scripts/lightning-labs-litd-counterparty.sh` starts the Lightning Labs
counterparty topology needed for real Taproot Asset channel interop: Bitcoin
Core plus integrated `litd`, where `litd` runs LND and taproot-assets in the
same process with the aux funding controller and `simple-taproot-overlay-chans`
enabled.

```bash
./scripts/lightning-labs-litd-counterparty.sh start
./scripts/lightning-labs-litd-counterparty.sh balance '<asset-id>'
```

This is different from the standalone LND/`tapd` harness. The standalone
harness is still useful for proof export/import and current `tapd` balance
checks, but standalone LND refuses the taproot overlay flag without an aux
controller. The integrated `litd` harness is the live asset-channel target for
issue #57.

The readiness report records the litd identity pubkey, LND sync state,
taproot-assets sync state, wallet balance, subserver status, and whether the
asset-channel RPC surface is reachable. The live outgoing-payment gate now uses
that identity and P2P address to run a native LDK peer preflight against `litd`.
It does not mark a `tap-ldk` to Lightning Labs payment complete; that still
requires running asset-channel funding/payment over the connected litd peer and
recording the post-settlement receiver balance.
