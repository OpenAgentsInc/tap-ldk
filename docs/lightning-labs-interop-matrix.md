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
| Automated harness | `docs/path-b-lightning-labs-demo.md`; `scripts/lightning-labs-counterparty.sh`; `scripts/lightning-labs-litd-counterparty.sh`; `scripts/path-b-lightning-labs-demo.sh`; `scripts/full-demo-smoke.sh` | Path B writes counterparty config/status, dependency gaps, fixture reports, payment artifacts, integrated `litd` readiness, native LDK peer preflight, and consolidated interop checks to a predictable ignored artifact directory. The counterparty scripts now perform ordered Bitcoin Core/LND/tapd bootstrap plus integrated litd bootstrap when a runtime is reachable. | Partially implemented |
| Live `tap-ldk` peer | `crates/tap-ldk-core/src/live_peer.rs`; `crates/tap-ldk-core/src/live_litd_peer.rs`; `docs/live-tap-ldk-peer.md`; `docs/live-litd-peer-preflight.md` | First localhost peer smoke starts a `tap-ldk` listener, connects a second peer, negotiates asset-channel support through the OpenAgentsInc rust-lightning fork, and round-trips an encoded native RFQ custom message. The native LDK preflight now connects to integrated Lightning Labs `litd`; running the asset-payment messages over that daemon-backed peer remains open. | Partially implemented |
| Funding extension point | `../projects/lightninglabs/repos/taproot-assets/docs/asset-channel-funding.md`; `tapchannel/aux_funding_controller.go` | Map LND `AuxFundingController` behavior to explicit LDK/fork extension surfaces. | Required |
| Channel feature/channel type | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/records.go`; `tapchannelmsg/wire_msgs_test.go` | Add experimental asset-channel negotiation; normal BTC channels remain BTC-only. | Required |
| Funding proof transport | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records_test.go` | Send proof data outside `open_channel`, reconstruct fragments, and reject incomplete proofs before funding advances. | Required |
| Funding message names | `tapchannelmsg/wire_msgs_test.go` | Track `TxAssetInputProof`, `TxAssetOutputProof`, `AssetFundingCreated`, and `AssetFundingAccepted` equivalents. | Required |
| Funding blob shape | `tapchannelmsg/testdata/funding-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `docs/lightning-labs-blob-fixtures.md`; `docs/lightning-labs-funding-interop.md` | Fixture-backed decoder maps decimal display, optional group key, funded asset outputs, proof digests, and raw digest. Funding interop smoke reconciles the funding total with commitment local/remote balances and persists the documented live-funding gap. | Partially implemented |
| Commitment blob shape | `tapchannelmsg/testdata/commitment-blob.hexdump`; `tapchannelmsg/records.go`; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps local/remote/outgoing/incoming asset output sections, aux leaves, optional STXO, and raw digest. Full commitment-number integration remains required. | Partially implemented |
| HTLC blob shape | `tapchannelmsg/testdata/htlc-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `rfqmsg` tests; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps asset balances when present, RFQ id, available RFQ ids, noop flag, and visible optional odd records. Full send/receive enforcement remains required. | Partially implemented |
| Asset signatures | `tapchannelmsg/records.go`; `tapchannel/aux_leaf_signer_test.go` | Maintain asset-level signing/nonce context separate from BTC-level signatures. | Required |
| Proof file import/export | `../projects/lightninglabs/repos/taproot-assets/proof`; `proof/append.go`; `proof/file.go`; `proof/tx.go`; `docs/tapd-proof-import-export.md` | Fixture-backed `TAPF`/`TAPP` decoder validates version/checksum/TLV transport, stores raw proof files across restart, and exports raw proof bytes for Lightning Labs verification tooling. Full semantic proof ancestry remains required. | Partially implemented |
| Address encoding | `../projects/lightninglabs/repos/bips/bip-tap-addr.mediawiki`; `address/address.go`; `address/encoding.go` | Native address encode/decode already passes imported TAP BIP vectors; still needs wallet integration. | Partially implemented |
| Virtual PSBT | `../projects/lightninglabs/repos/bips/bip-tap-psbt.mediawiki`; `tappsbt/interface.go`; `tappsbt/decode.go`; `fixtures/tap-bips/psbt_encoding_generated.json` | Current `tap-ldk` summary validates fixtures; full VPacket signing and state transition construction remains required. | Partially implemented |
| Issuance and proof sync | `itest/assets_test.go`; `itest/mint_fund_seal_test.go`; `proof` package; `scripts/live-tapd-proof-bind.sh`; `crates/tap-ldk-core/src/live_tapd_proof.rs` | Live command path can mint `OPENUSD` through `tapcli`, mine confirmations, export TAPF proof material, and bind it into native `tap-ldk` wallet state when the Lightning Labs daemon is reachable. Local host currently records a runtime prerequisite before minting. | Partially implemented |
| Universe/proof courier | `bip-tap-universe.mediawiki`; `proof/courier.go`; `itest/universe_test.go` | Use local proof/universe courier for demo; do not make it production infrastructure. | Mocked for first demo |
| RFQ message types | `rfqmsg/request_test.go`; `rfqmsg/accept_test.go`; `rfqmsg/reject_test.go`; `rfqmsg/messages_test.go`; `docs/lightning-labs-rfq-invoice.md` | Lightning Labs request/accept/reject payloads round-trip with message types `52884..52886`, RFQ-ID-derived SCID aliases, fixed-point rates, and fail-closed version/expiry checks. Native `tap-ldk` RFQ shell message types remain intentionally separate. | Partially implemented |
| Invoice behavior | `tapchannel/aux_invoice_manager.go`; `itest/custom_channels/decode_invoice_test.go`; `itest/custom_channels/invoice_expiry_test.go`; `docs/lightning-labs-rfq-invoice.md` | BOLT 11 invoice text stays opaque; Lightning Labs RFQ metadata is checked against native quote-bound invoice fields before HTLC/payment state can advance. | Partially implemented |
| Quote expiry | `itest/custom_channels/invoice_expiry_test.go`; `rfq/manager_test.go`; `docs/lightning-labs-rfq-invoice.md` | RFQ request/accept expiry, invoice expiry, quote expiry, and replay checks are enforced in the bounded interop smoke. Live daemon expiry behavior still needs Track B payment execution. | Partially implemented |
| Multi-RFQ and routing | `itest/custom_channels/multi_rfq_test.go`; `itest/custom_channels/multi_channel_pathfinding_test.go` | Out of first-demo scope except as a compatibility note. | Deferred |
| Payment direction: `tap-ldk` pays Lightning Labs | `itest/custom_channels/core_test.go`; `itest/custom_channels/single_asset_multi_input_test.go`; `itest/custom_channels/strict_forwarding_test.go`; `docs/lightning-labs-outgoing-payment.md`; `scripts/live-lightning-labs-outgoing-payment.sh` | Sender-side Track B artifacts are built and persisted: fixture-backed funding state, Lightning Labs RFQ payloads, quote-bound invoice, asset HTLC metadata, expected balance delta, replay rejection, wrong-asset rejection, wrong-amount rejection, and a live outgoing-payment gate that refuses success until a real Lightning Labs receiver balance is observed. | Stopped at live settlement gap |
| Payment direction: Lightning Labs pays `tap-ldk` | `itest/custom_channels/core_test.go`; `tapchannel/aux_invoice_manager.go`; `docs/lightning-labs-incoming-payment.md` | Receiver-side Track B artifacts are built and persisted: fixture-backed funding state, Lightning Labs buy-direction RFQ payloads, native quote-bound receive invoice, final-hop asset HTLC metadata, expected balance delta, stale/wrong-amount/malformed/replay rejection, and documented live-daemon settlement gap. | Stopped at live daemon gap |
| Balance comparison | `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records.go`; `itest/custom_channels/balance_consistency_test.go`; `docs/lightning-labs-interop-checks.md` | Automated interop check report compares funding balances, proof availability, both payment-direction asset IDs, expected balance deltas, metadata rejection checks, and restart round trips. Live observed balance comparison remains a documented gap until a daemon-backed run supplies actual balances. | Partially implemented |
| Cooperative close | `tapchannel/aux_closer.go`; `itest/custom_channels/restart_coop_close_test.go` | Close/proof export is required for a strong demo; may remain after first payment interop if documented. | Stronger-demo gate |
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

- Close the live funding gap recorded by
  `docs/lightning-labs-funding-interop.md`: drive a headless or Polar-backed
  LND/`tapd` channel funding attempt and bind the live funding outpoint to the
  proof and funding blob.
- Drive a live LND/`tapd` RFQ exchange and verify Lightning Labs accept
  signatures against the peer/session once payment execution begins.
- Extend proof import/export from byte-compatible `TAPF` preservation to full
  semantic proof ancestry validation.
- Use the hardened `scripts/lightning-labs-counterparty.sh` readiness report as
  the first external counterparty smoke path once a Docker or Podman runtime is
  reachable.
- Replace expected-only payment deltas with observed balance comparison checks
  after each live interop payment.
- Document any mismatch as a failing compatibility gap, not a partial success.

## Current Known Gaps

- `tap-ldk` does not yet implement native asset-channel funding.
- `tap-ldk` does not yet construct full virtual PSBT state transitions.
- `tap-ldk` preserves and exports `tapd` proof files, but does not yet verify
  full proof ancestry, virtual transactions, or on-chain anchor semantics.
- `tap-ldk` parses fixture-backed Lightning Labs funding/HTLC/commitment blob
  field maps, but does not yet apply them to live interop channel state.
- `tap-ldk` implements bounded Lightning Labs RFQ request/accept/reject payload
  compatibility, but does not yet run the live daemon RFQ session or verify the
  Lightning Labs accept signature.
- `tap-ldk` builds sender-side artifacts for the `tap-ldk` pays Lightning Labs
  direction, but does not yet drive a live LND/`tapd` receiver or observe the
  Lightning Labs receiver balance.
- `tap-ldk` builds receiver-side artifacts for the Lightning Labs pays
  `tap-ldk` direction, but does not yet drive a live LND/`tapd` sender or
  observe a durable `tap-ldk` receiver balance.
- `tap-ldk` emits a consolidated Track B interop check report with structured
  mismatch diagnostics, but live observed balance checks still require the
  headless or Polar-backed Lightning Labs counterparty.
