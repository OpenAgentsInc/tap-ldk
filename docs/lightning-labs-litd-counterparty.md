# `litd` Counterparty

`scripts/lightning-labs-litd-counterparty.sh` starts the `litd` counterparty
topology needed for real Taproot Asset channel interop: Bitcoin
Core plus integrated `litd`, where `litd` runs LND and taproot-assets in the
same process with the aux funding controller and `simple-taproot-overlay-chans`
enabled.

```bash
./scripts/lightning-labs-litd-counterparty.sh start
./scripts/lightning-labs-litd-counterparty.sh balance '<asset-id>'
./scripts/lightning-labs-litd-counterparty.sh close-asset-channel '<txid:index>' false
```

This is different from the standalone LND/`tapd` harness. The standalone
harness is still useful for proof export/import and current `tapd` balance
checks, but standalone LND refuses the taproot overlay flag without an aux
controller. The integrated `litd` harness is the live asset-channel target for
the completed first-demo Path B settlement regressions.

The readiness report records the litd identity pubkey, LND sync state,
taproot-assets sync state, wallet balance, subserver status, and whether the
asset-channel RPC surface is reachable. The live outgoing-payment gate now uses
that identity and P2P address to run a fork-backed `ldk-node` peer preflight
against `litd`.
The completed #81, #57, and #58 gates use this harness for live asset-channel
funding and both payment directions. Integrated `litd` can pay native LDK,
native LDK records the received balance, native LDK can return the asset to
`litd`, and the report records observed balances on both sides.

`close-asset-channel` wraps the integrated LND `closechannel` RPC for a channel
point returned by `asset-channel-status`. It records exit status, stdout,
stderr, and parsed JSON output when available. This is the live peer operation
needed for cooperative-close testing, but the Path B report still needs native
post-close Taproot Asset proof and balance observation before claiming live
cooperative close success.

The harness mines a fresh regtest block before the LND sync checks, and again
after the wallet-funding step, so a persisted regtest chain with an old tip
does not leave litd reporting `synced_to_chain=false`.
