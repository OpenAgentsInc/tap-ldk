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
that identity and P2P address to run a fork-backed `ldk-node` peer preflight
against `litd`.
In the current #57 gate this reaches `integrated_litd_counterparty_ready=true`
and `native_litd_peer_connected=true`. It does not mark a `tap-ldk` to
Lightning Labs payment complete; that still requires #80 and #81 to expose
and use the fork-backed asset-channel message/payment APIs, then run
asset-channel funding/payment over the connected `litd` peer and record the
post-settlement receiver balance.

The harness mines a fresh regtest block before the LND sync checks, and again
after the wallet-funding step, so a persisted regtest chain with an old tip
does not leave litd reporting `synced_to_chain=false`.
