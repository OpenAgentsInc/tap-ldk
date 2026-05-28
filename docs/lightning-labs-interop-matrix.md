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
| Automated harness | `docs/path-b-lightning-labs-demo.md`; `scripts/lightning-labs-counterparty.sh`; `scripts/lightning-labs-litd-counterparty.sh`; `scripts/path-b-lightning-labs-demo.sh`; `scripts/full-demo-smoke.sh` | Path B writes counterparty config/status, dependency gaps, fixture reports, payment artifacts, integrated `litd` readiness, fork-backed `ldk-node` peer preflight, and consolidated interop checks to a predictable ignored artifact directory. The counterparty scripts now perform ordered Bitcoin Core/LND/tapd bootstrap plus integrated litd bootstrap when a runtime is reachable. | Partially implemented |
| Live `tap-ldk` peer | `crates/tap-ldk-core/src/live_peer.rs`; `crates/tap-ldk-core/src/live_litd_peer.rs`; `docs/live-tap-ldk-peer.md`; `docs/live-litd-peer-preflight.md`; `docs/openagents-ldk-node-fork.md` | First localhost peer smoke starts a `tap-ldk` listener, connects a second peer, negotiates asset-channel support through the OpenAgentsInc rust-lightning fork, and round-trips an encoded native RFQ custom message. The current `litd` peer preflight uses fork-backed `OpenAgentsInc/ldk-node`, proves connectivity plus fork provenance, reports opt-in simple-taproot/Taproot Asset channel negotiation, and exercises typed asset message/channel/payment APIs. Issue #81 must run those APIs through live settlement before settlement can be claimed. | Partially implemented |
| Funding extension point | `../projects/lightninglabs/repos/taproot-assets/docs/asset-channel-funding.md`; `tapchannel/aux_funding_controller.go` | Map LND `AuxFundingController` behavior to explicit LDK/fork extension surfaces. | Required |
| Channel feature/channel type | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/records.go`; `tapchannelmsg/wire_msgs_test.go` | Add experimental asset-channel negotiation; normal BTC channels remain BTC-only. | Required |
| Funding proof transport | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records_test.go` | Send proof data outside `open_channel`, reconstruct fragments, and reject incomplete proofs before funding advances. | Required |
| Funding message names | `tapchannelmsg/wire_msgs_test.go` | Track `TxAssetInputProof`, `TxAssetOutputProof`, `AssetFundingCreated`, and `AssetFundingAccepted` equivalents. | Required |
| Funding blob shape | `tapchannelmsg/testdata/funding-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `docs/lightning-labs-blob-fixtures.md`; `docs/lightning-labs-funding-interop.md` | Fixture-backed decoder maps decimal display, optional group key, funded asset outputs, proof digests, and raw digest. Funding interop smoke reconciles the funding total with commitment local/remote balances and persists the documented live-funding gap. | Partially implemented |
| Commitment blob shape | `tapchannelmsg/testdata/commitment-blob.hexdump`; `tapchannelmsg/records.go`; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps local/remote/outgoing/incoming asset output sections, aux leaves, optional STXO, and raw digest. Full commitment-number integration remains required. | Partially implemented |
| HTLC blob shape | `tapchannelmsg/testdata/htlc-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `rfqmsg` tests; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps asset balances when present, RFQ id, available RFQ ids, noop flag, and visible optional odd records. Rust Lightning also decodes the live `litd` asset-id/amount HTLC payload, rejects malformed blobs at `update_add_htlc`, persists them through channel state, and re-emits them on outbound `update_add_htlc`. Full per-commitment asset-state application remains required. | Partially implemented |
| Asset signatures | `tapchannelmsg/records.go`; `tapchannel/aux_leaf_signer_test.go` | Maintain asset-level signing/nonce context separate from BTC-level signatures. | Required |
| Proof file import/export | `../projects/lightninglabs/repos/taproot-assets/proof`; `proof/append.go`; `proof/file.go`; `proof/tx.go`; `docs/tapd-proof-import-export.md` | Fixture-backed `TAPF`/`TAPP` decoder validates version/checksum/TLV transport, stores raw proof files across restart, and exports raw proof bytes for Lightning Labs verification tooling. Full semantic proof ancestry remains required. | Partially implemented |
| Address encoding | `../projects/lightninglabs/repos/bips/bip-tap-addr.mediawiki`; `address/address.go`; `address/encoding.go` | Native address encode/decode already passes imported TAP BIP vectors; still needs wallet integration. | Partially implemented |
| Virtual PSBT / TAP VM | `../projects/lightninglabs/repos/bips/bip-tap-psbt.mediawiki`; `tappsbt/interface.go`; `tappsbt/decode.go`; `fixtures/tap-bips/psbt_encoding_generated.json`; `fixtures/tap-bips/vm_validation_generated.json` | Native `tap_vm` validates generated TAP BIP issuance, transfer, split, hash-lock, signature, and negative vectors, and channel funding/commitment updates derive virtual IDs only after validation. Full VPacket signing and proof-chain ancestry remain required. | Partially implemented |
| Issuance and proof sync | `itest/assets_test.go`; `itest/mint_fund_seal_test.go`; `proof` package; `scripts/live-tapd-proof-bind.sh`; `crates/tap-ldk-core/src/live_tapd_proof.rs` | Live command path can mint `OPENUSD` through `tapcli`, mine confirmations, export TAPF proof material, and bind it into native `tap-ldk` wallet state when the Lightning Labs daemon is reachable. The #57 gate has reached `proof_binding_status=bound` in a live run. | Partially implemented |
| Universe/proof courier | `bip-tap-universe.mediawiki`; `proof/courier.go`; `itest/universe_test.go` | Use local proof/universe courier for demo; do not make it production infrastructure. | Mocked for first demo |
| RFQ message types | `rfqmsg/request_test.go`; `rfqmsg/accept_test.go`; `rfqmsg/reject_test.go`; `rfqmsg/messages_test.go`; `docs/lightning-labs-rfq-invoice.md` | Lightning Labs request/accept/reject payloads round-trip with message types `52884..52886`, RFQ-ID-derived SCID aliases, fixed-point rates, and fail-closed version/expiry checks. Native `tap-ldk` RFQ shell message types remain intentionally separate. | Partially implemented |
| Invoice behavior | `tapchannel/aux_invoice_manager.go`; `itest/custom_channels/decode_invoice_test.go`; `itest/custom_channels/invoice_expiry_test.go`; `docs/lightning-labs-rfq-invoice.md` | BOLT 11 invoice text stays opaque; Lightning Labs RFQ metadata is checked against native quote-bound invoice fields before HTLC/payment state can advance. | Partially implemented |
| Quote expiry | `itest/custom_channels/invoice_expiry_test.go`; `rfq/manager_test.go`; `docs/lightning-labs-rfq-invoice.md` | RFQ request/accept expiry, invoice expiry, quote expiry, and replay checks are enforced in the bounded interop smoke. Live daemon expiry behavior still needs Track B payment execution. | Partially implemented |
| Multi-RFQ and routing | `itest/custom_channels/multi_rfq_test.go`; `itest/custom_channels/multi_channel_pathfinding_test.go` | Out of first-demo scope except as a compatibility note. | Deferred |
| Payment direction: `tap-ldk` pays Lightning Labs | `itest/custom_channels/core_test.go`; `itest/custom_channels/single_asset_multi_input_test.go`; `itest/custom_channels/strict_forwarding_test.go`; `docs/lightning-labs-outgoing-payment.md`; `scripts/live-lightning-labs-outgoing-payment.sh` | Sender-side Track B artifacts are built and persisted: fixture-backed funding state, Lightning Labs RFQ payloads, quote-bound invoice, asset HTLC metadata, expected balance delta, replay rejection, wrong-asset rejection, wrong-amount rejection, and a live outgoing-payment gate. The current live gate proves the opposite live direction first: integrated `litd` pays native LDK, native LDK claims, and `ldk-node` records receiver balance. The true native-to-Lightning Labs direction remains #57. | Open |
| Payment direction: Lightning Labs pays `tap-ldk` | `itest/custom_channels/core_test.go`; `tapchannel/aux_invoice_manager.go`; `docs/lightning-labs-incoming-payment.md` | Receiver-side Track B artifacts are built and persisted, and the live integrated `litd` keysend now settles into native LDK with durable receiver balance. The post-success zero-HTLC commitment partial-signature mismatch and force-close Taproot control-block failure are fixed in the current fork line; #58 still needs the full named reverse-direction acceptance flow and restart proof. | Partially implemented |
| Balance comparison | `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records.go`; `itest/custom_channels/balance_consistency_test.go`; `docs/lightning-labs-interop-checks.md` | Automated interop check report compares funding balances, HTLC RFQ metadata, RFQ message types, proof availability, both payment-direction asset IDs, expected balance deltas, metadata rejection checks, restart round trips, simple-taproot asset-channel lifecycle state, close/proof recovery, and explicit observed-balance gates. Live native receiver balance is now observed for Lightning Labs to native; two-sided observed balance comparison remains open until the native-to-Lightning Labs direction supplies the Lightning Labs receiver delta. | Partially implemented |
| Cooperative close | `tapchannel/aux_closer.go`; `itest/custom_channels/restart_coop_close_test.go`; `docs/simple-taproot-cooperative-close-2026-05-28.md` | Native simple-taproot cooperative close asserts the final P2TR key-path witness, Taproot Asset close preserves the latest allocation across restart, and the `litd` harness exposes `close-asset-channel`. Live post-close proof/balance observation remains documented, not claimed. | Fixture-backed with live boundary |
| Force close | `tapchannel/aux_sweeper.go`; `itest/custom_channels/force_close_test.go`; `itest/custom_channels/htlc_force_close_test.go` | Bounded proof-ownership recovery records now exist for commitment, second-level HTLC, and final sweep paths, and BTC-only sweep state is refused as asset recovery. Live daemon-backed resolver/sweeper interop remains open. | Partially implemented |

## First Direction To Implement

Implement `tap-ldk` pays Lightning Labs first.

Reason:

- `tap-ldk` can own the sender-side native proof, RFQ, route metadata, and HTLC
  construction work.
- The Lightning Labs side can act as an independent receiver/counterparty.
- Success can be checked by comparing asset ID, payment state, and receiver
  balance through the Lightning Labs daemon APIs.

Lightning Labs pays `tap-ldk` is the second direction because it requires a
native receiver invoice/final-hop path in `tap-ldk`. The bounded receive-side
artifacts now exist, but live LND/`tapd` sender execution and observed durable
`tap-ldk` balance comparison are still required before this direction can be
reported as settled interop.

## Follow-Up Implementation Issues

Close the remaining issues in this order:

1. #81: use `OpenAgentsInc/ldk-node` asset-channel messages and payment APIs
   for live settlement.
2. #57: run asset-channel funding/payment over the fork-backed connected
   independent `litd` peer and record the Lightning Labs receiver balance
   after settlement.
3. #58: drive the reverse Lightning Labs sender flow into `tap-ldk`, persist
   the received balance/proof reference, and verify restart.
4. #59: replace expected-only payment deltas with observed balance comparison
   checks after both live interop payments.
5. #60: extend proof import/export from byte-compatible `TAPF` preservation to
   full semantic proof ancestry validation and wire it into funding, HTLC,
   close, and recovery.
6. Close #19 only when Path B reports live settlement in both directions and
   any mismatch is a failing compatibility gap, not a partial success.

## Current Known Gaps

- `tap-ldk` implements bounded native asset-channel funding, native first-demo
  virtual transition validation, and a fork-backed simple-taproot
  asset-channel lifecycle smoke. The live Lightning Labs peer still needs to
  run through fork-backed `ldk-node` and settle asset-channel funding/payment
  over the connected `litd` session.
- `tap-ldk` preserves and exports `tapd` proof files, but does not yet verify
  full proof ancestry, proof-chain virtual transactions, or on-chain anchor
  semantics.
- `tap-ldk` parses fixture-backed Lightning Labs funding/HTLC/commitment blob
  field maps. The fork validates the live HTLC blob at `update_add_htlc`, but
  does not yet apply the full blob set to live interop channel state.
- `tap-ldk` implements bounded Lightning Labs RFQ request/accept/reject payload
  compatibility, but does not yet run the live daemon RFQ session or verify the
  Lightning Labs accept signature.
- `tap-ldk` builds sender-side artifacts for the `tap-ldk` pays Lightning Labs
  direction and reaches the connected integrated `litd` peer through
  fork-backed `ldk-node`. The live harness currently proves the Lightning Labs
  to native direction first; the remaining sender-side blocker is implementing
  true native-to-Lightning Labs payment execution and recording the Lightning
  Labs receiver balance delta.
- `tap-ldk` builds receiver-side artifacts for the Lightning Labs pays
  `tap-ldk` direction, and the integrated `litd` live keysend now observes a
  durable native receiver balance. The post-success zero-HTLC commitment
  partial-signature mismatch and force-close Taproot control-block path are
  fixed in the current fork line.
- `tap-ldk` emits a consolidated Track B interop check report with structured
  mismatch diagnostics, simple-taproot asset-channel vector coverage, and
  explicit observed-balance gates, but live observed balance checks still
  require the headless or Polar-backed Lightning Labs counterparty to settle
  real asset payments.
