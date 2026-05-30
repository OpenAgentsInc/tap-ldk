# `lnd`/`tapd`/`litd` Interop Matrix

Date: 2026-05-25

This matrix scopes Track B: native `tap-ldk` talking to an independent
`lnd`/`tapd`/`litd` counterparty. The `lnd`/`tapd`/`litd` daemon is
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
| Live `tap-ldk` peer | `crates/tap-ldk-core/src/live_peer.rs`; `crates/tap-ldk-core/src/live_litd_peer.rs`; `docs/live-tap-ldk-peer.md`; `docs/live-litd-peer-preflight.md`; `docs/openagents-ldk-node-fork.md` | First localhost peer smoke starts a `tap-ldk` listener, connects a second peer, negotiates asset-channel support through the OpenAgentsInc rust-lightning fork, and round-trips an encoded native RFQ custom message. The current `litd` peer preflight uses fork-backed `OpenAgentsInc/ldk-node`, proves connectivity plus fork provenance, reports opt-in simple-taproot/Taproot Asset channel negotiation, exercises typed asset message/channel/payment APIs, and the #81 gate has run those APIs through live `litd` to native settlement. | Partially implemented |
| Funding extension point | `../projects/lightninglabs/repos/taproot-assets/docs/asset-channel-funding.md`; `tapchannel/aux_funding_controller.go` | Map LND `AuxFundingController` behavior to explicit LDK/fork extension surfaces. | Required |
| Channel feature/channel type | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/records.go`; `tapchannelmsg/wire_msgs_test.go` | Add experimental asset-channel negotiation; normal BTC channels remain BTC-only. | Required |
| Funding proof transport | `docs/blip-tap-implementation-note.md`; `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records_test.go` | Send proof data outside `open_channel`, reconstruct fragments, and reject incomplete proofs before funding advances. | Required |
| Funding message names | `tapchannelmsg/wire_msgs_test.go` | Track `TxAssetInputProof`, `TxAssetOutputProof`, `AssetFundingCreated`, and `AssetFundingAccepted` equivalents. | Required |
| Funding blob shape | `tapchannelmsg/testdata/funding-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `docs/lightning-labs-blob-fixtures.md`; `docs/lightning-labs-funding-interop.md` | Fixture-backed decoder maps decimal display, optional group key, funded asset outputs, proof digests, and raw digest. Funding interop smoke reconciles the funding total with commitment local/remote balances and persists the documented live-funding gap. | Partially implemented |
| Commitment blob shape | `tapchannelmsg/testdata/commitment-blob.hexdump`; `tapchannelmsg/records.go`; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps local/remote/outgoing/incoming asset output sections, aux leaves, optional STXO, and raw digest. Full commitment-number integration remains required. | Partially implemented |
| HTLC blob shape | `tapchannelmsg/testdata/htlc-blob.hexdump`; `tapchannelmsg/wire_msgs_test.go`; `rfqmsg` tests; `docs/lightning-labs-blob-fixtures.md` | Fixture-backed decoder maps asset balances when present, RFQ id, available RFQ ids, noop flag, and visible optional odd records. Rust Lightning also decodes the live `litd` asset-id/amount HTLC payload, rejects malformed blobs at `update_add_htlc`, persists them through channel state, and re-emits them on outbound `update_add_htlc`. Full per-commitment asset-state application remains required. | Partially implemented |
| Asset signatures | `tapchannelmsg/records.go`; `tapchannel/aux_leaf_signer_test.go` | Maintain asset-level signing/nonce context separate from BTC-level signatures. | Required |
| Proof file import/export | `../projects/lightninglabs/repos/taproot-assets/proof`; `proof/append.go`; `proof/file.go`; `proof/tx.go`; `docs/tapd-proof-import-export.md` | `TAPF`/`TAPP` decoder validates version/checksum/TLV transport, decodes the latest asset leaf, derives the Taproot Assets asset ID from genesis, checks asset type/amount/owner/genesis against the native proof record, stores raw proof files across restart, and exports raw proof bytes for Lightning Labs verification tooling. Wallet balance/export now also requires replayed proof-history metadata, bounded anchor-state policy, and the local typed proof-courier bundle for normal wallet handoff. Network proof discovery, STXO, split/change, grouped-asset, and live chain-watcher policy remain production work. | Implemented for bounded proof-history boundary |
| Address encoding | `../projects/lightninglabs/repos/bips/bip-tap-addr.mediawiki`; `address/address.go`; `address/encoding.go` | Native address encode/decode already passes imported TAP BIP vectors; still needs wallet integration. | Partially implemented |
| Virtual PSBT / TAP VM | `../projects/lightninglabs/repos/bips/bip-tap-psbt.mediawiki`; `tappsbt/interface.go`; `tappsbt/decode.go`; `fixtures/tap-bips/psbt_encoding_generated.json`; `fixtures/tap-bips/vm_validation_generated.json` | Native `tap_vm` validates generated TAP BIP issuance, transfer, split, hash-lock, signature, and negative vectors, and channel funding/commitment updates derive virtual IDs only after validation. Full VPacket signing and proof-chain ancestry remain required. | Partially implemented |
| Issuance and proof sync | `itest/assets_test.go`; `itest/mint_fund_seal_test.go`; `proof` package; `scripts/live-tapd-proof-bind.sh`; `crates/tap-ldk-core/src/live_tapd_proof.rs` | Live command path can mint `OPENUSD` through `tapcli`, mine confirmations, export TAPF proof material, and bind it into native `tap-ldk` wallet state when the `lnd`/`tapd`/`litd` daemon is reachable. The #57 gate has reached `proof_binding_status=bound` in a live run. | Partially implemented |
| Universe/proof courier | `bip-tap-universe.mediawiki`; `proof/courier.go`; `itest/universe_test.go`; `docs/proof-courier-bundles.md` | Local wallet handoff now uses the typed proof-courier bundle. It carries native proof bytes, optional TAPF bytes, proof-history IDs, anchor state, asset fields, and digests together, and fails closed on malformed transport, mismatched fields, bad digests, stale/reorged spendable export claims, and proof-history mismatch. This is local transport policy, not a network universe service. | Implemented locally; network universe deferred |
| RFQ message types | `rfqmsg/request_test.go`; `rfqmsg/accept_test.go`; `rfqmsg/reject_test.go`; `rfqmsg/messages_test.go`; `docs/lightning-labs-rfq-invoice.md` | taproot-assets request/accept/reject payloads round-trip with message types `52884..52886`, RFQ-ID-derived SCID aliases, fixed-point rates, and fail-closed version/expiry checks. Native `tap-ldk` RFQ shell message types remain intentionally separate. | Partially implemented |
| Invoice behavior | `tapchannel/aux_invoice_manager.go`; `itest/custom_channels/decode_invoice_test.go`; `itest/custom_channels/invoice_expiry_test.go`; `docs/lightning-labs-rfq-invoice.md` | BOLT 11 invoice text stays opaque; taproot-assets RFQ metadata is checked against native quote-bound invoice fields before HTLC/payment state can advance. | Partially implemented |
| Quote expiry | `itest/custom_channels/invoice_expiry_test.go`; `rfq/manager_test.go`; `docs/lightning-labs-rfq-invoice.md` | RFQ request/accept expiry, invoice expiry, quote expiry, and replay checks are enforced in the bounded interop smoke. Live daemon expiry behavior still needs Track B payment execution. | Partially implemented |
| Multi-RFQ and routing | `itest/custom_channels/multi_rfq_test.go`; `itest/custom_channels/multi_channel_pathfinding_test.go` | Out of first-demo scope except as a compatibility note. | Deferred |
| Payment direction: `tap-ldk` pays `litd` | `itest/custom_channels/core_test.go`; `itest/custom_channels/single_asset_multi_input_test.go`; `itest/custom_channels/strict_forwarding_test.go`; `docs/lightning-labs-outgoing-payment.md`; `scripts/live-lightning-labs-outgoing-payment.sh` | Sender-side Track B artifacts are built and persisted, and the live gate now completes the bidirectional integrated-`litd` path: `litd` pays native LDK, native LDK records the received asset, native LDK sends the asset back with the canonical Taproot Asset HTLC blob and dust-covering BTC amount, and observed `litd` channel balance reflects the returned amount. | Implemented as live regression |
| Payment direction: `litd` pays `tap-ldk` | `itest/custom_channels/core_test.go`; `tapchannel/aux_invoice_manager.go`; `docs/lightning-labs-incoming-payment.md` | Receiver-side Track B artifacts are built and persisted, and the live integrated `litd` keysend settles into native LDK with durable receiver payment/balance state. `target/live-lightning-labs-outgoing-payment-issue58-rerun/report.json` has `issue_58_acceptance_met=true` and a completed native restart snapshot. | Implemented as live regression |
| Balance comparison | `tapchannelmsg/wire_msgs_test.go`; `tapchannelmsg/records.go`; `itest/custom_channels/balance_consistency_test.go`; `docs/lightning-labs-interop-checks.md`; `scripts/path-b-lightning-labs-demo.sh` | Automated interop checks still label fixture-only expected deltas as fixture-only, while `path-b-completion-report.json` consumes the live #57/#58 report and sets `path_b_live_observed_balance_gate_met=true` only when both live directions have observed balances and non-secret payment references. Fixture-only and expected-only completion are disabled. | Implemented as live observed-balance gate |
| Cooperative close | `tapchannel/aux_closer.go`; `itest/custom_channels/restart_coop_close_test.go`; `docs/simple-taproot-cooperative-close-2026-05-28.md` | Native simple-taproot cooperative close asserts the final P2TR key-path witness, Taproot Asset close preserves the latest allocation across restart, and the `litd` harness exposes `close-asset-channel`. Live post-close proof/balance observation remains documented, not claimed. | Fixture-backed with live boundary |
| Force close | `tapchannel/aux_sweeper.go`; `itest/custom_channels/force_close_test.go`; `itest/custom_channels/htlc_force_close_test.go`; `docs/onchain-lifecycle-readiness.md`; `docs/chain-watcher-lifecycle-readiness.md` | Bounded proof-ownership recovery records now exist for commitment, second-level HTLC, and final sweep paths, BTC-only sweep state is refused as asset recovery, `onchain-lifecycle.json` consolidates those records with close/restart evidence, `chain-watcher-lifecycle.json` binds the bounded lifecycle records to typed chain/sweeper observations, and the extended live-regtest callback smoke binds those observations to Bitcoin Core regtest height/block data. Live daemon-backed resolver/sweeper interop with real close/sweep tx anchors remains open. | Partially implemented |
| Proof-engine verification | `formal/tla/proof_validation`; `crates/tap-ldk-core/tests/proof_replay_properties.rs`; `fuzz/fuzz_targets`; `scripts/proof-engine-check.sh`; `.github/workflows/proof-engine.yml` | Proof replay, negative vectors, wallet/funding/commitment/close/recovery replay gates, bounded anchor policy, bounded on-chain lifecycle reporting, bounded chain/sweeper observation reporting, live-regtest callback binding, formal checks, Rust property tests, fuzz target compile/smoke paths, and optional Kani harnesses are now covered by local wrapper and CI workflow surfaces. | Implemented for bounded hardening suite |

## Live Payment Direction Status

The bidirectional integrated-`litd` live regression now covers both payment
directions in one channel run:

- `litd` funds the asset channel and pays native LDK.
- Native LDK records the received asset balance.
- Native LDK sends the asset back with the canonical Taproot Asset HTLC blob
  and a dust-covering BTC amount.
- `litd` settles the receive invoice and reports the returned channel asset
  balance.

`litd` pays `tap-ldk` is now covered by the same integrated-`litd`
live regression. The bounded receive-side artifacts still exist separately, and
the live report now proves native receiver settlement plus restart persistence.

## Follow-Up Implementation Issues

The first-demo issue path is closed. Keep #19 green only when Path B reports
live settlement in both directions and any mismatch is a failing compatibility
gap, not a partial success.

## Current Known Gaps

- `tap-ldk` implements bounded native asset-channel funding, native first-demo
  virtual transition validation, and a fork-backed simple-taproot
  asset-channel lifecycle smoke. The #81 and #57 gates now settle both live
  directions over fork-backed `ldk-node` and integrated `litd`.
- `tap-ldk` semantically validates imported `tapd` proof asset leaves against
  native proof metadata, preserves raw proof files, and gates wallet/funding/
  commitment/close/recovery state through bounded proof-history replay.
  Production courier, full STXO/split/change paths, grouped assets, and live
  chain-watcher integration remain future production hardening.
- `tap-ldk` parses fixture-backed Lightning Labs funding/HTLC/commitment blob
  field maps. The fork validates the live HTLC blob at `update_add_htlc`, but
  does not yet apply the full blob set to live interop channel state.
- `tap-ldk` implements bounded taproot-assets RFQ request/accept/reject payload
  compatibility, but does not yet run the live daemon RFQ session or verify the
  Lightning Labs accept signature.
- `tap-ldk` builds sender-side artifacts for the `tap-ldk` pays `litd`
  direction and now proves the live native-to-`litd` return payment through
  fork-backed `ldk-node`.
- `tap-ldk` builds receiver-side artifacts for the `litd` pays
  `tap-ldk` direction, and the integrated `litd` live keysend now observes a
  durable native receiver balance. The post-success zero-HTLC commitment
  partial-signature mismatch and force-close Taproot control-block path are
  fixed in the current fork line.
- `tap-ldk` emits a consolidated Track B interop check report with structured
  mismatch diagnostics, simple-taproot asset-channel vector coverage, and
  explicit observed-balance gates, and #59 adds the broader Path B completion
  report that fails closed without live observed-balance/proof state.
