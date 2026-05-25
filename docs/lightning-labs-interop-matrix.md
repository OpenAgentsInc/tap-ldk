# Lightning Labs Interop Matrix

Date: 2026-05-25

This matrix scopes Track B: native `tap-ldk` talking to an independent
Lightning Labs LND/`tapd` or `litd` counterparty. The Lightning Labs daemon is
a compatibility peer, not a sidecar inside the `tap-ldk` wallet runtime.

## Version Target

First Track B target:

- Bitcoin Core `30.0`
- LND `0.19.0-beta`
- `tapd` `0.7.0-alpha`

Alternate manual target:

- Bitcoin Core `30.0`
- `litd` `0.16.0-alpha`

Sources:

- `docs/polar-regtest-topology.md`
- `../projects/repos/polar/docker/nodes.json`

The explicit LND plus `tapd` target is preferred because it proves the daemon
split and lets `tap-ldk` implement the Taproot Assets logic natively.

## Matrix

| Surface | Lightning Labs source | Native `tap-ldk` implication | Status |
| --- | --- | --- | --- |
| Regtest versions | `docs/polar-regtest-topology.md`; `../projects/repos/polar/docker/nodes.json` | Use Bitcoin Core `30.0`, LND `0.19.0-beta`, `tapd` `0.7.0-alpha` for first LND/`tapd` interop. | Selected |
| Manual topology | `docs/polar-regtest-topology.md` | Polar may operate Bitcoin/LND/`tapd` for manual Track B, while `tap-ldk` runs outside Polar. | Selected |
| Funding extension point | `../projects/lightninglabs/repos/taproot-assets/docs/asset-channel-funding.md`; `tapchannel/aux_funding_controller.go` | Map LND `AuxFundingController` behavior to explicit LDK/fork extension surfaces. | Required |
| Channel feature/channel type | `docs/blip-0029-implementation-note.md`; `tapchannelmsg/records.go`; `tapchannelmsg/wire_msgs_test.go` | Add experimental asset-channel negotiation; normal BTC channels remain BTC-only. | Required |
| Funding proof transport | `docs/blip-0029-implementation-note.md`; `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records_test.go` | Send proof data outside `open_channel`, reconstruct fragments, and reject incomplete proofs before funding advances. | Required |
| Funding message names | `tapchannelmsg/wire_msgs_test.go` | Track `TxAssetInputProof`, `TxAssetOutputProof`, `AssetFundingCreated`, and `AssetFundingAccepted` equivalents. | Required |
| Funding blob shape | `tapchannelmsg/testdata/funding-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go` | Add fixture decoding before claiming funding interop. | Required |
| Commitment blob shape | `tapchannelmsg/testdata/commitment-blob.hexdump`; `tapchannelmsg/records.go` | Preserve local/remote/outgoing/incoming asset outputs with the matching commitment number. | Required |
| HTLC blob shape | `tapchannelmsg/testdata/htlc-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `rfqmsg` tests | Encode asset ID, amount, quote binding, and final-hop validation context in custom records. | Required |
| Asset signatures | `tapchannelmsg/records.go`; `tapchannel/aux_leaf_signer_test.go` | Maintain asset-level signing/nonce context separate from BTC-level signatures. | Required |
| Proof file import/export | `../projects/lightninglabs/repos/taproot-assets/proof`; `proof/append.go`; `proof/file.go`; `proof/tx.go` | Import/export compatible proof data and preserve anchor/proof material across restart. | Required |
| Address encoding | `../projects/lightninglabs/repos/bips/bip-tap-addr.mediawiki`; `address/address.go`; `address/encoding.go` | Native address encode/decode already passes imported TAP BIP vectors; still needs wallet integration. | Partially implemented |
| Virtual PSBT | `../projects/lightninglabs/repos/bips/bip-tap-psbt.mediawiki`; `tappsbt/interface.go`; `tappsbt/decode.go`; `fixtures/tap-bips/psbt_encoding_generated.json` | Current `tap-ldk` summary validates fixtures; full VPacket signing and state transition construction remains required. | Partially implemented |
| Issuance and proof sync | `itest/assets_test.go`; `itest/mint_fund_seal_test.go`; `proof` package | First interop harness can mint/import on Lightning Labs side, export proof, then import into `tap-ldk`. | Required |
| Universe/proof courier | `bip-tap-universe.mediawiki`; `proof/courier.go`; `itest/universe_test.go` | Use local proof/universe courier for demo; do not make it production infrastructure. | Mocked for first demo |
| RFQ message types | `rfqmsg/request_test.go`; `rfqmsg/accept_test.go`; `rfqmsg/reject_test.go`; `rfqmsg/messages_test.go` | Implement request/accept/reject and bind asset amount, BTC amount, peer, invoice context, route context, and expiry. | Required |
| Invoice behavior | `tapchannel/aux_invoice_manager.go`; `itest/custom_channels/decode_invoice_test.go`; `itest/custom_channels/invoice_expiry_test.go` | Keep BOLT 11 invoice format; select asset semantics through RFQ and metadata. | Required |
| Quote expiry | `itest/custom_channels/invoice_expiry_test.go`; `rfq/manager_test.go` | Invoice expiry must not outlive quote validity in a way that permits stale settlement. | Required |
| Multi-RFQ and routing | `itest/custom_channels/multi_rfq_test.go`; `itest/custom_channels/multi_channel_pathfinding_test.go` | Out of first-demo scope except as a compatibility note. | Deferred |
| Payment direction: `tap-ldk` pays Lightning Labs | `itest/custom_channels/core_test.go`; `itest/custom_channels/single_asset_multi_input_test.go`; `itest/custom_channels/strict_forwarding_test.go` | Implement first because `tap-ldk` can construct RFQ/payment from native state and compare Lightning Labs receiver balance. | First direction |
| Payment direction: Lightning Labs pays `tap-ldk` | `itest/custom_channels/core_test.go`; `tapchannel/aux_invoice_manager.go` | Implement after native receive invoice and final-hop validation are real; document as gap until then. | Second direction |
| Balance comparison | `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records.go`; `itest/custom_channels/balance_consistency_test.go` | Track both sides' asset ID, amount, payment state, and resulting balances after each interop payment. | Required |
| Cooperative close | `tapchannel/aux_closer.go`; `itest/custom_channels/restart_coop_close_test.go` | Close/proof export is required for a strong demo; may remain after first payment interop if documented. | Stronger-demo gate |
| Force close | `tapchannel/aux_sweeper.go`; `itest/custom_channels/force_close_test.go`; `itest/custom_channels/htlc_force_close_test.go` | Do not claim force-close recovery until proof ownership and sweep state are implemented. | Deferred |

## First Direction To Implement

Implement `tap-ldk` pays Lightning Labs first.

Reason:

- `tap-ldk` can own the sender-side native proof, RFQ, route metadata, and HTLC
  construction work.
- The Lightning Labs side can act as an independent receiver/counterparty.
- Success can be checked by comparing asset ID, payment state, and receiver
  balance through the Lightning Labs daemon APIs.

Lightning Labs pays `tap-ldk` is the second direction because it requires a
native receiver invoice/final-hop path in `tap-ldk`.

## Follow-Up Implementation Issues

- Decode Lightning Labs funding, HTLC, and commitment blob fixtures from
  `tapchannelmsg/testdata`.
- Implement native RFQ request/accept/reject messages against `rfqmsg` vectors.
- Implement proof import/export compatibility with `tapd`.
- Extend the headless harness to start Bitcoin Core plus LND/`tapd` using the
  selected versions.
- Add balance comparison checks after each interop payment.
- Document any mismatch as a failing compatibility gap, not a partial success.

## Current Known Gaps

- `tap-ldk` does not yet implement native asset-channel funding.
- `tap-ldk` does not yet construct full virtual PSBT state transitions.
- `tap-ldk` does not yet parse Lightning Labs funding/HTLC/commitment blobs.
- `tap-ldk` does not yet implement RFQ wire compatibility.
- `tap-ldk` does not yet perform either Track B payment direction.

