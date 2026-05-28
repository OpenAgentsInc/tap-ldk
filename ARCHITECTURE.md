# tap-ldk Architecture

Date: 2026-05-26

`tap-ldk` is an experimental native Rust/LDK proof of concept for Taproot
Assets over Lightning-style channels. The point is not to wrap `tapd` and call
that an LDK wallet. The point is to make the asset proof, asset-channel,
quote, HTLC, payment, persistence, and recovery logic live in Rust/LDK code.

This document explains what exists now, how the pieces fit together, what has
been wired against `rust-lightning`, and what still has to be built before the
Lightning Labs interop demo is real.

## Current State

The native demo works as a bounded local smoke:

- Two local `tap-ldk` wallets can issue and transfer a demo `OPENUSD` asset.
- The demo can create a mocked single-asset channel with local and remote
  balances.
- It can move asset balance through a quote-bound HTLC/payment flow.
- It can restart and recover the same channel/payment state.
- It can cooperatively close and export final proof artifacts for both sides.
- It can start a localhost `tap-ldk` peer listener, connect a second local
  peer over TCP, negotiate the experimental single-asset capability through the
  rust-lightning fork, and round-trip an encoded native RFQ custom message.

The Lightning Labs path now has a live bidirectional payment regression:

- The repo can decode Lightning Labs funding, HTLC, commitment, RFQ, and proof
  fixtures.
- It can build fixture-backed reports for both payment directions.
- It can prove that expected balances are conserved in those reports.
- It writes a live outgoing-payment gate that links live `tapd` proof binding
  to the native outgoing RFQ/invoice/HTLC artifact, integrated `litd`
  channel state, fork-backed native LDK receiver/sender state, and observed
  Lightning Labs channel balance.
- It can start integrated Lightning Labs `litd`, confirm the asset-channel RPC
  surface is reachable, then start the fork-backed `ldk-node` runtime and
  connect it to the `litd` Lightning P2P address.
- The current #57/#81 gate reaches `proof_binding_status=bound`,
  `native_asset_payment_session_ready=true`,
  `integrated_litd_counterparty_ready=true`,
  `native_litd_peer_connected=true`, both remote taproot feature observations,
  live `litd` asset-channel funding, `channel_ready`, a keysend-usable `litd`
  asset-channel balance, Lightning Labs to native asset keysend success,
  native `PaymentClaimed`, and durable native receiver balance recording in
  `ldk-node`. It then sends the asset back from native LDK to `litd` with a
  canonical Taproot Asset HTLC blob and a dust-covering BTC amount.
- It now uses fork-backed `ldk-node`, so the live peer path reaches the
  OpenAgentsInc `rust-lightning` simple-taproot and Taproot Asset channel
  hooks.
- It completes live funding, Lightning Labs to native settlement, native to
  Lightning Labs settlement, and the returned `litd` channel asset-balance
  observation. The latest live report has `issue_81_acceptance_met=true`,
  `issue_57_acceptance_met=true`, no invalid commitment, no invalid
  simple-taproot signature, no invalid Taproot control-block, and no
  counterparty force-close marker.
- The 2026-05-28 BOLT simple-taproot audit confirms the fork is not production
  spec-complete. The current first-demo scope covers base BTC simple-taproot
  open/pay/reestablish/cooperative-close/force-close and explicitly excludes
  concurrent splicing until bounded nonce-map vectors are added.
- The ordered local asset-payment message session remains useful for bounded
  negative-path checks, but #57 is now proved by the live integrated `litd`
  channel run instead of by that loopback session.

The `rust-lightning` fork is wired with the first asset-channel hooks, but not
the full asset channel implementation:

- `tap-ldk-core` depends on `OpenAgentsInc/rust-lightning` at a pinned fork
  revision.
- The repo records which asset-channel hooks must live inside the fork.
- The fork now includes feature/channel-type gates, a bounded funding approval
  hook, a channel monitor aux blob surface for asset commitments, an HTLC
  metadata/final-hop validation surface, and a cooperative close allocation
  surface, and a proof-ownership recovery surface for force-close,
  second-level HTLC, and final sweep paths.
- The fork still needs full live channel-manager, resolver, and sweeper
  call-site integration before the open epics can close.

## Repository Layout

The root repo contains code, docs, fixtures, formal models, and demo scripts.

- `crates/tap-ldk-core`: all current protocol and state-machine logic.
- `crates/tap-ldk-cli`: thin command-line wrapper around `tap-ldk-core`.
- `fixtures`: synthetic fixtures, imported TAP BIP vectors, and imported
  Lightning Labs fixture data.
- `formal`: TLA+ models and model-boundary notes.
- `scripts`: local regtest, Path A, Path B, and full-demo harness scripts.
- `docs`: narrow implementation notes for each surface.
- `README.md`: public top-level status and development commands.
- `INVARIANTS.md`: safety and correctness rules that future code must keep.
- `ROADMAP.md`: issue sequence and demo plan.

`projects/` and `stablecoins/` are outside this repo. They are reference and
planning material. They are not runtime implementation homes for `tap-ldk`.

## Crates

### `tap-ldk-core`

`tap-ldk-core` owns the protocol logic. It is deliberately split into small
modules so each boundary can be tested without needing a full live Lightning
node.

Important modules:

- `asset`: basic asset IDs, amounts, compressed keys, genesis data, split
  conservation, same-asset input merging, and bounded asset-leaf root summaries
  backed by the native MS-SMT primitive.
- `mssmt`: Taproot Assets-style Merkle Sum Sparse Merkle Tree primitives:
  256-level roots, inclusion/exclusion proofs, Lightning Labs-compatible
  compressed proof encoding, and overflow/malformed-proof rejection.
- `taproot_commitment`: protocol-shaped asset commitment keys,
  `AssetCommitment`, `TapCommitment`, Taproot Asset tap leaf scripts, and
  BIP341 tap leaf/branch binding for output commitments.
- `tap_vm`: native virtual transaction and TAP VM validation for the first
  demo surfaces: issuance, transfer/split fixtures, channel funding, and
  commitment-update conservation and witness rules.
- `tlv`: strict BigSize/TLV encode and decode. It rejects non-canonical
  integers, duplicate records, out-of-order records, truncation, and unknown
  even required types.
- `proof`: bounded native proof format used by the local demo.
- `tapd_proof`: Lightning Labs `TAPP` single proof and `TAPF` proof-file
  envelope parsing.
- `live_tapd_proof`: binds a daemon-exported TAPF proof into native wallet
  state, reports asset id/balance/proof metadata, and fails closed on wrong
  asset id, stale proof digest, or wrong owner script key.
- `wallet`: local JSON wallet state, issuance, proof import/export, local
  proof transfer, tapd proof import/export, balances, and atomic persistence.
- `ldk_baseline`: BTC-only baseline LDK smoke and planning state.
- `ldk_fork`: metadata and compile-time touchpoints for the
  `OpenAgentsInc/rust-lightning` fork.
- `asset_channel_boundary`: typed ledger of hooks that belong in `tap-ldk`,
  `ldk-node`, the interop harness, or the rust-lightning fork.
- `asset_channel_negotiation`: experimental asset-channel feature negotiation
  and channel type model.
- `asset_peer_message`: native custom-message shells for funding proofs, RFQ,
  funding accept/reject, and asset HTLC blobs.
- `rfq_quote_store`: fixed-rate quote store, replay-domain tracking, SCID
  alias allocation, expiry, accept/reject, and HTLC authorization.
- `rfq_invoice`: quote-bound invoice logic. BOLT 11 text remains opaque and
  unchanged.
- `asset_channel_funding`: bounded native asset-channel funding store.
- `asset_commitment`: asset balance transition and monitor blob model.
- `asset_htlc`: custom record encode/decode and final-hop validation.
- `asset_payment`: Path A native payment orchestration.
- `asset_recovery`: restart checkpoint model for funding, quote, HTLC,
  commitment, settlement, and close-prep boundaries.
- `asset_close`: cooperative close and final proof export.
- `live_peer`: localhost live peer smoke for asset-channel negotiation and
  native custom-message movement.
- `lightning_labs_blob`: fixture-backed Lightning Labs funding, HTLC, and
  commitment blob decoding.
- `lightning_labs_funding`: fixture-backed funding interop report and store.
- `lightning_labs_rfq`: Lightning Labs RFQ request/accept/reject compatibility.
- `lightning_labs_payment`: fixture-backed outgoing and incoming payment
  reports for Lightning Labs interop.
- `lightning_labs_interop_checks`: consolidated Track B report that says which
  checks passed and which live-daemon gaps remain.
- `regtest`: local regtest and Lightning Labs counterparty config material.
- `address` and `virtual_psbt`: imported TAP BIP fixture support.

### `tap-ldk-cli`

`tap-ldk-cli` is intentionally thin. It parses command arguments, calls
`tap-ldk-core`, and prints JSON or short text. The CLI is useful because the
scripts can treat every step as a command that writes an artifact.

Examples:

- `wallet-init`
- `wallet-issue-openusd`
- `wallet-send-local`
- `wallet-import-proof-file`
- `wallet-import-tapd-proof-file`
- `wallet-export-tapd-proof-file`
- `wallet-balances`
- `asset-negotiation-smoke`
- `asset-peer-message-smoke`
- `asset-channel-funding-smoke`
- `asset-commitment-smoke`
- `asset-htlc-smoke`
- `asset-payment-smoke`
- `asset-recovery-smoke`
- `live-peer-smoke`
- `live-asset-payment-session-smoke`
- `asset-close-smoke`
- `lightning-labs-blob-fixture-smoke`
- `lightning-labs-proof-fixture-smoke`
- `lightning-labs-funding-interop-smoke`
- `lightning-labs-rfq-invoice-compat-smoke`
- `lightning-labs-outgoing-payment-smoke`
- `lightning-labs-incoming-payment-smoke`
- `lightning-labs-interop-check-smoke`

## Asset Primitives

The local demo needs deterministic asset identifiers, amounts, proof summaries,
and balance conservation checks.

`asset.rs` provides:

- `Bytes32` for fixed 32-byte IDs and digests.
- `CompressedKey` for 33-byte compressed keys.
- `Genesis` for deterministic demo asset ID derivation.
- `AssetAmount` with checked add/subtract.
- `AssetLeaf` for asset ownership leaves.
- `derive_hash_sum_root` for deterministic hash+sum summaries backed by the
  native MS-SMT primitive.
- `validate_split_conservation` for transfer/split checks.
- `merge_same_asset_inputs` for funding multiple same-asset inputs.

`mssmt.rs` implements the protocol-shaped tree primitive separately from the
bounded asset helper. It matches the Lightning Labs node hashing shape, bit
order, inclusion/exclusion proof walk, and compressed proof format against
imported `taproot-assets` vectors.

`taproot_commitment.rs` builds on that tree with:

- no-group and group-style asset commitment keys;
- inner `AssetCommitment` trees;
- outer `TapCommitment` trees;
- Taproot Asset commitment script parsing for the upstream Lightning Labs
  script fixture;
- BIP341 tap leaf and branch hashing for output commitment binding.

Asset-channel funding now derives its funding root and output commitment from a
`TapCommitment` instead of the older bounded root placeholder.

`tap_vm.rs` adds the native virtual transition layer on top of those
commitments. It validates generated TAP BIP issuance, transfer, split, hash
lock, and signature witness cases; rejects generated invalid cases; and gives
channel funding and commitment updates deterministic virtual IDs and witness
digests only after amount and witness validation. The first-demo semantic proof
boundary is now enforced by #60; production full-history virtual transaction,
STXO, grouped-asset, and reorg hardening remain future production work.

## TLV Layer

The TLV layer is strict on purpose. A parser that silently accepts malformed
data would make interop results meaningless.

`tlv.rs` enforces:

- canonical BigSize integers;
- sorted TLV records;
- no duplicate record types;
- no truncation;
- rejection of unknown even required types.

Most protocol modules use this TLV layer directly. That includes native proof
files, native peer messages, HTLC custom records, Lightning Labs blob fixtures,
Lightning Labs RFQ payloads, and `tapd` proof-file parsing.

## Proof Handling

There are two proof paths.

### Native semantic proof path

`proof.rs` defines the native proof record accepted by the wallet and channel
state:

- version;
- asset ID;
- genesis outpoint;
- anchor outpoint;
- amount;
- script key;
- root hash;
- root sum;
- verification scope;
- network;
- asset type.

The accepted verification scope is now `semantic-ancestry`. The validator
rejects shallow field matches: it requires regtest scope, normal asset type for
the demo stablecoin path, strict `<txid>:<vout>` outpoints, nonzero asset and
amount fields, root sum conservation, a derived Taproot Asset root hash for the
accepted asset leaf, expected asset/owner/amount checks when supplied by the
call site, and stale-anchor rejection.

HTLC receipt does not accept an independently invented proof root. The
final-hop metadata must validate normally and match the proof root hash and sum
already committed in the channel monitor blob before settlement can advance.

### Lightning Labs proof path

`tapd_proof.rs` parses Lightning Labs proof files:

- `TAPP` single proof magic;
- `TAPF` proof-file magic;
- proof-file version;
- proof count;
- proof lengths;
- chained proof checksums;
- strict inner proof TLVs;
- known required proof records;
- latest asset-leaf TLV fields;
- Taproot Assets genesis-derived asset ID;
- optional unknown odd records.

`wallet.rs` imports a Lightning Labs `TAPF` proof file only after the latest
`TAPP` asset leaf agrees with the local proof record's asset ID, normal asset
type, amount, owner script key, and genesis outpoint. The raw proof-file bytes
and digest are still preserved alongside the native proof record so export
returns the exact accepted bytes.

Remaining production hardening after #60 is narrower: full Bitcoin anchor
transaction/merkle validation, full proof-chain virtual transaction replay,
grouped/collectible/reissuance paths, STXO/split/change proof replay, reorg
watcher integration, and production proof-courier policy remain future
production work.

## Wallet Storage

`WalletState` is JSON-backed and schema-versioned. It stores:

- metadata;
- proofs;
- spendable asset UTXOs;
- pending operations.

The wallet validates itself before saving. Saves are atomic: write a temp file,
then rename it into place.

Important behavior:

- Proof import validates before wallet state advances.
- Duplicate proof import is idempotent when the proof bytes match.
- Conflicting proof import fails.
- Proof balances are derived from spendable UTXOs, not from a cached counter.
- Tapd raw proof files are stored exactly when imported.
- Unsupported schema versions fail closed.

The local transfer path spends one spendable asset UTXO, creates a receiver
proof, optionally creates change, and validates split conservation before
mutating state.

## Baseline LDK

The project keeps a BTC-only baseline so asset work does not accidentally
weaken normal Lightning behavior.

`ldk_baseline.rs` models:

- two local nodes;
- a normal BTC channel;
- one BTC payment;
- restart state;
- asset-channel features disabled.

The baseline is not the full live LDK node runtime. It is the current local
smoke and policy check that BTC-only behavior remains separate from
asset-channel experiments.

## Live tap-ldk Peer

`live_peer.rs` is the first live process surface. It starts a real localhost
TCP listener, connects a client, frames JSON control messages over the socket,
and sends an encoded native asset peer message as the payload. The peer does
not just trust a fixture: it calls the OpenAgentsInc rust-lightning fork
negotiation surface before it accepts an asset-channel custom message.

The current smoke proves:

- the server process starts and accepts a local connection;
- the client can connect over TCP;
- the asset-channel feature bits and channel type are negotiated through the
  fork-backed `asset_channel_negotiation` path;
- an encoded `AssetPeerMessage::RfqRequest` crosses the live socket;
- the receiver decodes the payload and checks it is allowed only after asset
  negotiation succeeds.

Command:

```bash
cargo run -p tap-ldk-cli -- live-peer-smoke target/live-peer-smoke.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
cargo run -p tap-ldk-cli -- live-asset-payment-session-smoke target/live-asset-payment-session.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 125
cargo run -p tap-ldk-cli -- live-litd-peer-preflight target/live-litd-peer-preflight.json target/live-litd-peer-preflight-state '<litd-node-id>' '127.0.0.1:29735'
```

Current boundary:

- the ordered asset-payment message exchange is not yet a Lightning Labs
  daemon-backed session;
- the `live-litd-peer-preflight` command uses the OpenAgentsInc `ldk-node`
  fork to connect to integrated `litd`, prove fork provenance, observe remote
  simple-taproot and Taproot Asset channel support, and reach the fork-backed
  asset custom-message, channel-open, and payment APIs;
- the live outgoing-payment script now drives integrated `litd` asset issuance
  and real asset-channel funding against the fork-backed peer. #81 remains
  open until the payment-time monitor update completes, held commitment
  messages are released, the native receiver claims the HTLC, the witness path
  works, and post-settlement balances are observed end to end;
- it does not yet send Lightning wire custom messages to LND;
- it is the runnable `tap-ldk` peer process and ordered native payment-session
  exchange that must be moved onto the connected counterparty peer.

## rust-lightning Fork Wiring

The workspace points at:

- fork: `https://github.com/OpenAgentsInc/rust-lightning.git`
- upstream: `https://github.com/lightningdevkit/rust-lightning.git`
- base revision: `0c37f08a55c0f7738f2691dc3690166fd42f851d`
- current revision: `057d0e7c524f7b1255cabf22ae9f7fc261256aea`

`crates/tap-ldk-core/Cargo.toml` has a direct dependency:

```toml
lightning = { git = "https://github.com/OpenAgentsInc/rust-lightning.git", rev = "057d0e7c524f7b1255cabf22ae9f7fc261256aea", package = "lightning", features = ["simple_taproot_musig2"] }
```

`ldk_fork.rs` checks that the fork is reachable and that important
rust-lightning feature types are available:

- `lightning::types::features::ChannelTypeFeatures`
- `lightning::types::features::InitFeatures`

The current fork integration exposes the first real asset-channel gate:

- `ChannelHandshakeConfig::negotiate_simple_taproot_channels`
- `ChannelTypeFeatures::simple_taproot`
- `ChannelTypeFeatures::simple_taproot_staging`
- `lightning::ln::taproot_asset::TaprootAssetChannelDescriptor`
- `lightning::ln::taproot_asset::negotiate_single_asset_channel`
- `lightning::ln::taproot_asset::validate_single_asset_channel_open`
- `lightning::ln::taproot_asset::TaprootAssetFundingRequest`
- `lightning::ln::taproot_asset::validate_asset_channel_funding`
- `lightning::ln::taproot_asset::TaprootAssetChannelState`
- `lightning::ln::taproot_asset::TaprootAssetMonitorAuxBlob`
- `lightning::ln::taproot_asset::TaprootAssetMonitorAuxBlobExpectation`
- `lightning::ln::taproot_asset::TaprootAssetHtlcBlob`
- `lightning::ln::taproot_asset::decode_taproot_asset_htlc_blob`
- `lightning::ln::taproot_asset::TaprootAssetHtlcMetadata`
- `lightning::ln::taproot_asset::TaprootAssetHtlcMetadataExpectation`
- `lightning::ln::taproot_asset::TaprootAssetCloseAllocation`
- `lightning::ln::taproot_asset::TaprootAssetCloseAllocationExpectation`
- `lightning::ln::taproot_asset::TaprootAssetProofOwnershipState`
- `lightning::ln::taproot_asset::TaprootAssetProofOwnershipExpectation`
- `lightning::ln::taproot_asset::prepare_asset_htlc_metadata`
- `lightning::ln::taproot_asset::validate_asset_htlc_final_hop`
- `lightning::ln::taproot_asset::prepare_cooperative_close_asset_allocation`
- `lightning::ln::taproot_asset::validate_cooperative_close_asset_allocation`
- `lightning::ln::taproot_asset::prepare_asset_proof_ownership_recovery`
- `lightning::ln::taproot_asset::validate_asset_proof_ownership_recovery`
- `lightning::ln::simple_taproot::SimpleTaprootKeyAggContext`
- `lightning::ln::simple_taproot::SimpleTaprootHtlcSpendInfo`
- `lightning::ln::simple_taproot::SimpleTaprootHtlcSpendPath`
- `lightning::ln::simple_taproot::SimpleTaprootNonceState`
- `lightning::ln::simple_taproot::derive_simple_taproot_counter_nonce_seed`
- `lightning::ln::simple_taproot::derive_simple_taproot_jit_nonce_seed`
- `lightning::ln::simple_taproot::simple_taproot_htlc_spend_info`
- `lightning::ln::simple_taproot::simple_taproot_second_level_htlc_spend_info`
- `lightning::ln::simple_taproot::simple_taproot_sign_htlc_spend`
- `lightning::sign::SimpleTaprootChannelSigner`
- `ChannelMonitorUpdate::taproot_asset_aux_update`
- `ChannelMonitorUpdate::require_taproot_asset_aux_blob`
- `ChannelHandshakeConfig::negotiate_taproot_asset_channels`
- `ChannelTypeFeatures::taproot_asset_single_asset`

Those gates cover BOLT simple taproot staging feature negotiation, explicit
simple-taproot channel type handling, native simple-taproot lifecycle wire TLV
codecs, feature-gated MuSig2 key aggregation/nonce/signature helpers, BIP86
P2TR funding script handling, P2TR to-local/to-remote/anchor commitment output
scripts, HTLC P2TR output scripts, second-level HTLC output scripts,
taproot-sighash signing helpers, tap tweak and control-block reconstruction
data, BOLT-vector replay coverage for the implemented simple-taproot surfaces,
and fail-closed malformed/duplicate/unsupported TLV, wrong funding script, and
nonce-reuse tests.
They also cover experimental Taproot Asset channel type handling layered on
that base and the bounded funding-controller approval surface. They provide
the first versioned channel monitor aux blob hook for asset commitment state,
the first HTLC metadata/final-hop validation and cooperative close allocation
hooks, plus the first proof-ownership recovery hook for force-close,
second-level HTLC, and final sweep paths. The current revision also exposes a
bounded `TaprootAssetChannelState` lifecycle state that requires explicit
simple-taproot asset-channel negotiation, proof-backed funding, monitor aux
blob persistence before commitment advancement, HTLC metadata validation,
cooperative close allocation validation, and proof-ownership recovery checks.
It now also strictly decodes the live Lightning Labs Taproot Asset HTLC blob,
persists that blob through inbound/outbound HTLC state and holding-cell
serialization, re-emits it on outbound `update_add_htlc`, and carries optional
asset aux leaves into simple-taproot HTLC output construction. The current
fork pin also encodes Lightning Labs second-level HTLC virtual lock fields.
Live #81 still has to prove the full transcript against `litd`, then fix any
remaining per-commitment asset-state or witness/control-block deltas.

Rust Lightning uses `bitcoin::secp256k1`, the rust-bitcoin wrapper around
libsecp256k1. The fork does not call raw libsecp APIs directly. The #63 TLV
work only defined and validated the wire payloads for simple-taproot MuSig2
nonces and partial signatures; it did not sign, aggregate, or verify them.
Issue #64 added the feature-gated Rust `musig2` crate integration and signer
state helpers. Issue #67 routes those helpers through the first live LDK
channel-ready, commitment-signed, revoke-and-ack, and reestablish call sites.

## What Must Be Added To rust-lightning

`asset_channel_boundary.rs` is the typed list of rust-lightning/LDK extension
surfaces. It exists so we do not hand-wave where behavior belongs.

The following surfaces must move into the OpenAgentsInc rust-lightning fork for
a real live demo:

- Feature negotiation: asset-channel behavior must be behind explicit feature
  bits or negotiated channel flags. Initial fork support landed in
  `99ddb8b7033b3b5d056005c00ba650e716ed37da`.
- BOLT simple taproot negotiation: BTC-only simple taproot support must have
  its own feature bits and explicit channel type before the asset overlay can
  claim a real channel. Initial staging-bit support landed in
  `90054d8fc512eb9506955f27806b496e33d2b346`.
- BOLT simple taproot wire messages: native lifecycle messages must carry the
  MuSig2 nonce and partial-signature TLVs without changing legacy messages.
  Initial TLV codec and message validation support landed in
  `c237a0ae1189c0c59e27bdc8e8b99fd2bb018bcb`. The current audit still
  requires zeroing legacy ECDSA signature fields for simple-taproot
  `funding_created`, `funding_signed`, and `commitment_signed`, and rejecting
  non-zero peer legacy fields.
- BOLT simple taproot MuSig2 signer state: simple-taproot funding keys must
  aggregate with BIP-327 sorting, public nonces and partial signatures must
  verify, final Schnorr signatures must aggregate, and nonce-use state must
  survive serialization while rejecting reuse. Initial feature-gated support
  landed in `6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941`; first channel update
  and reestablish wiring landed in
  `1176e837e5aacac7d1a3237c2bb00910989dbd93`.
- BOLT simple taproot P2TR funding: simple-taproot channels must derive BIP86
  P2TR funding scripts from the sorted aggregate funding key, expose that
  script in `FundingGenerationReady`, reject funding transactions with the
  wrong script, and register the same script with channel monitors. Initial
  support landed in `1602ac9e1e7454d39612e126c24a098e276d605a`; commitment
  output/control-block work landed in #66 and first channel
  signing/reestablish wiring landed in #67.
- BOLT simple taproot commitment outputs: simple-taproot commitments must use
  P2TR to-local, to-remote, and anchor outputs with tapscript roots, tap
  tweaks, and control blocks that can be reconstructed after restart. Initial
  support landed in `b0b952531329a31265f8de28752ee5334d9d9d4f`; MuSig2
  commitment signing/reestablish moved in #67 and HTLC scripts landed in #69.
  The current live force-close path still fails with an invalid Taproot
  control block, so this surface is not complete for #81 closure.
- BOLT simple taproot commitment update/reestablish state: channel-ready,
  commitment-signed, revoke-and-ack, and channel-reestablish paths must carry
  next-local nonces, preserve sent partial signatures for retransmission, and
  fail closed on missing or cryptographically invalid simple-taproot nonce
  state. The live LND staging path treats the `commitment_signed` nonce as a
  JIT signing nonce, while next-local nonces remain future verification state.
  First support landed in `1176e837e5aacac7d1a3237c2bb00910989dbd93`; LND
  JIT nonce compatibility landed in
  `0d6ac878453bcc108f315d69aae0bda625c1f871`, and live asset HTLC blob
  persistence landed in `5bd5992ac7f7625f254e5df67eec66d085fe7c7d`.
- BOLT simple taproot cooperative close: shutdown must carry closee nonces,
  `closing_complete`/`closing_sig` must validate and aggregate MuSig2 close
  partials, closee nonce rotation must be persisted, and malformed close state
  must fail closed. First support landed in
  `26346a56af75eadf60763eb1e32a740656d4e384`; #69 unignored the functional
  close harness after fixing simple-taproot anchor/output accounting during
  channel open. Vector replay landed in #70.
- BOLT simple taproot HTLC outputs and second-level spends: simple-taproot
  commitments must emit BOLT-vector-matching offered/accepted HTLC P2TR
  outputs, construct P2TR second-level HTLC outputs, use sequence `1` for
  second-level spends, sign BIP342 `SIGHASH_SINGLE|ANYONECANPAY` tapscript
  spends, and build the correct witness stack for each offered/accepted
  success/timeout path. Initial support landed in
  `6af69ad385b864d7666edebbbbb668dab485bdde`; #75 now layers the bounded
  single-asset lifecycle state on top of these surfaces, while live
  channel-manager and interop exercises continue in the Path B epic #19, and
  #61 plus #71 stay as first-demo simple-taproot and Taproot Assets-over-LDK
  regression gates.
- BOLT simple taproot vector replay: the fork must keep fixture tests tied to
  `bolt-simple-taproot.md` for TLV payloads, nonce and partial-signature wire
  shapes, funding scripts, commitment output scripts and leaf hashes, close
  behavior, HTLC scripts, second-level outputs, and multi-HTLC value/trimming
  cases. Initial coverage landed in
  `983c4385ff66105ab70d766d34f49c1bd547a81a`. The BOLT draft transaction JSON
  currently disagrees with its script-vector section for some multi-HTLC
  output keys, so the fork asserts unambiguous script vectors exactly and uses
  transaction cases for output count, value/order, P2TR shape, and trimming.
- Taproot Asset channel lifecycle state: funding, commitments, HTLC metadata,
  cooperative close, monitor aux persistence, and proof-ownership recovery must
  be tied to one simple-taproot asset-channel state object instead of loose
  helper calls. Initial support landed in
  `99fee582d4061af4b0a030353b0a409ee542e064`; `tap-ldk` drives it through
  `simple-taproot-asset-channel-smoke`. The same revision also pins the live
  Lightning Labs CSV finding: the Taproot Asset allocation/script-key
  derivation keeps Lightning Labs' zero-CSV `AuxChanState` behavior, while the
  actual Bitcoin commitment to-local aux output uses the negotiated channel CSV
  delay. That matches the live `litd` commitment script vector instead of the
  earlier zero-CSV commitment-output attempt.
- Taproot Asset commitment output sorting: Taproot Asset aux leaves change the
  final P2TR script, but Lightning Labs sorts commitment outputs by the base
  simple-taproot script before the asset aux leaf is merged. Revision
  `a7cb50c64ba589e1171526f04f199d09cac35812` carries a separate base-script
  sort key for simple-taproot asset outputs so the live funding commitment can
  match `litd` ordering without changing the final output scripts. Revision
  `4761230b3d8a2732d379087a5510456a13b86c29` adds preservation and strict
  decoding of Lightning Labs `commitment_signed` TLV 65537 asset-signature
  blobs plus Lightning Labs commitment aux-leaf scripts, including HTLC-count
  validation for Taproot Asset channels. Revision
  `c94f4570587e94e89740f5126a5fa70021b58de2` adds trace diagnostics and a
  regression fixture for the rejected simple-taproot HTLC signature
  transcript. Follow-up revisions encode Lightning Labs second-level virtual
  lock fields in Taproot Asset HTLC aux leaves, persist full Taproot Asset
  counterparty commitments through monitor updates, derive exact
  previous-output-bound second-level HTLC aux leaves before outgoing HTLC
  signing, move claimed full-amount asset HTLCs into the receiver balance
  output, and finally make simple-taproot funding/commitment messages write
  zero legacy signature fields while rejecting non-zero peer legacy fields.
- Channel type: normal BTC channels must not become asset channels implicitly.
  Initial fork support landed in
  `99ddb8b7033b3b5d056005c00ba650e716ed37da`.
- Funding controller: funding must be blocked until asset ID, proof root,
  funding output, and allocation checks pass. Initial fork support landed in
  `84032b87d05a157ee9ef247102767bc100d84ed6`.
- Commitment blob: asset-channel state must be versioned with the Lightning
  commitment number. Initial monitor aux blob support landed in
  `4394c0e350dd5faf34ca37fc6bde5cc14497e3f9`; later issues still need to wire
  that through live channel-manager call sites.
- Monitor persistence: asset-channel state must be durable before the
  corresponding Lightning commitment is treated as safe. Initial fork support
  landed in `4394c0e350dd5faf34ca37fc6bde5cc14497e3f9`.
- HTLC metadata modifier: asset metadata must only be attached after an
  accepted quote. Initial fork support landed in
  `ef2538fe181025231c1f2a946df713b3109fa9ef`; later issues still need live
  channel-manager call sites.
- Final-hop validator: missing, stale, malformed, wrong-asset, or wrong-amount
  final-hop metadata must fail before settlement. Initial fork support landed
  in `ef2538fe181025231c1f2a946df713b3109fa9ef`.
- Close handler: cooperative close must return the latest mutually valid asset
  allocation. Initial fork support landed in
  `d6862145b43225d5002445c3733e70293bb0646e`.
- On-chain resolver/sweeper: force-close and sweep handling must preserve proof
  ownership. Initial fork support landed in
  `0f442683da45af47daff313fefcfaef1ac7b82d7`; later issues still need live
  channel-manager, resolver, and sweeper call sites.
The following surfaces stay in `tap-ldk`:

- proof parsing and proof storage;
- asset wallet state;
- asset peer-message codecs;
- proof chunking and reassembly;
- RFQ quote store;
- invoice binding policy;
- asset-level signing context;
- Lightning Labs fixture and interop codecs;
- demo scripts and artifact reporting.

The fork should expose bounded hooks and persistence surfaces. It should not
own wallet policy, proof courier policy, stablecoin business rules, or a `tapd`
sidecar.

## Asset-Channel Negotiation

`asset_channel_negotiation.rs` models the first experimental negotiation
surface:

- protocol version: `1`;
- required feature bit: `54032`;
- optional feature bit: `54033`;
- channel request: BTC-only or single-asset;
- negotiated type: BTC-only or `SingleAsset`.

Rules:

- BTC-only channels work without asset features.
- Single-asset channels require both peers to support asset channels.
- Asset ID cannot be zero.
- Asset messages are rejected before successful asset-channel negotiation.

These feature bits are local experimental values. They are not final BLIP or
BOLT assignments.

## Native Peer Messages

`asset_peer_message.rs` defines the first native message layer for the proof of
concept.

Messages include:

- `TxAssetInputProof`
- `TxAssetOutputProof`
- `AssetFundingCreated`
- `AssetFundingAccepted`
- `RfqRequest`
- `RfqAccept`
- `RfqReject`
- `AssetHtlcBlob`

The proof messages support chunking. A proof is split into chunks with a
SHA-256 digest. The receiver must reconstruct all chunks and verify the digest
before funding can advance.

This keeps bulky proof data out of `open_channel`. That matches the design
direction captured from BLIP-TAP review: funding proof transport is a separate
flow, not a bloated base channel-open message.

## RFQ And Invoice Binding

Asset payments need a BTC amount for Lightning while preserving asset semantics
outside the BOLT 11 invoice text.

The local RFQ flow works like this:

1. A peer requests a quote for asset ID, asset amount, peer, invoice context,
   expiry, and replay domain.
2. The fixed regtest oracle maps asset units to millisats.
3. The quote store assigns an SCID alias that cannot collide with registered
   real local SCIDs.
4. The receiver accepts, rejects, or expires the quote.
5. An accepted quote can bind to an invoice.
6. A payment can authorize one HTLC.
7. Replays and expired quotes fail.

The current fixed rate is:

- `100` millisats per `OPENUSD` unit.

The BOLT 11 string stays opaque. The code does not parse or change BOLT 11 for
the demo. The RFQ and HTLC metadata carry the asset meaning.

## HTLC Custom Records

`asset_htlc.rs` defines asset HTLC custom records under a local experimental
record base.

Records include:

- protocol version;
- asset ID;
- asset amount;
- proof/root reference hash and sum;
- quote ID;
- invoice context;
- BTC millisats;
- SCID alias;
- payment hash;
- final-hop digest.

Final-hop validation checks that the metadata matches the quote-bound invoice
and payment context. Wrong, stale, malformed, missing, or mismatched records
fail closed.

Normal BTC records are unaffected by this module.

## Native Asset-Channel Funding

`asset_channel_funding.rs` models the bounded native funding path.

It verifies:

- local and remote input proofs;
- same asset ID on all funding inputs;
- shared genesis across both sides;
- local and remote amount sums;
- funding root hash+sum;
- optional expected funding root;
- OpenAgentsInc rust-lightning funding-hook approval before durable state is
  written;
- proof reuse prevention;
- monitor blob persistence.

The store records:

- channel ID;
- local peer;
- remote peer;
- asset ID;
- genesis outpoint;
- funding outpoint;
- funding script key;
- funding Taproot Asset root;
- local and remote balances;
- total amount;
- input proof IDs;
- funding status;
- monitor blob.

This is still a bounded model. The fork hook now provides the approval
boundary, and later live channel plumbing must call it with proof material from
the custom funding messages before the live demo can be called complete.

## Asset Commitment State

`asset_commitment.rs` tracks the asset state coupled to Lightning commitment
numbers.

It records:

- latest commitment number;
- local and remote balances;
- total amount;
- commitment snapshots;
- revoked commitment numbers;
- used asset nonces;
- asset signing key;
- monitor blob.

Updates check:

- correct next commitment number;
- no asset nonce reuse;
- no balance underflow;
- no overflow;
- total balance conservation;
- separate BTC and asset signature domains;
- monitor digest consistency;
- matching LDK monitor aux blob digest and commitment number.

The module now builds an LDK `ChannelMonitorUpdate` carrying the fork's asset
monitor aux blob. Restart validation refuses missing or tampered aux blob
digests before treating the asset commitment state as recovered.

## Native Asset Payment Flow

`asset_payment.rs` wires funding, RFQ, invoice binding, HTLC records,
commitment updates, and payment storage into one native Path A payment.

The happy path:

1. Receiver accepts an RFQ.
2. The quote is bound to an invoice.
3. Sender pays the quote-bound invoice.
4. Asset HTLC custom records are built.
5. Final-hop metadata is validated.
6. The HTLC is added.
7. The asset commitment state moves balance from sender to receiver.
8. The HTLC is settled.
9. Payment state is stored as settled.

The smoke uses:

- 125 asset units;
- 12,500 millisats;
- starting channel balance `alice=700`, `bob=300`;
- ending channel balance `alice=575`, `bob=425`.

Negative checks cover:

- wrong quote;
- wrong invoice;
- wrong metadata;
- failed payments that must not advance durable settlement fields.

## Restart Recovery

`asset_recovery.rs` creates explicit checkpoints for:

- funding;
- quote accepted;
- HTLC added;
- commitment signed;
- settled;
- close prepared.

Each checkpoint records:

- stage;
- channel ID;
- asset ID;
- commitment number;
- local balance;
- remote balance;
- total balance;
- optional quote ID;
- optional HTLC ID;
- optional payment ID;
- optional close preparation;
- checkpoint digest.

Recovery refuses stale checkpoints. Restart after every modeled boundary must
recover the same asset state or fail clearly.

## Close And Force-Close

`asset_close.rs` implements bounded cooperative close.

Cooperative close:

- reads the latest commitment state;
- produces local and remote final proof files;
- records local and remote amounts;
- records the commitment number;
- validates the close allocation through the OpenAgentsInc rust-lightning fork;
- records the fork close allocation digest for proof handoff review;
- imports the final proofs into local wallets;
- rejects obsolete proof views;
- round-trips the close store to model restart.

The Path A script extracts:

- `native-close.json`;
- `native-close-local-proof.hex`;
- `native-close-remote-proof.hex`;
- `close-recovery-status.json`.

The bounded recovery smoke now validates proof-ownership records for
commitment force-close, second-level HTLC, and final sweep paths through the
OpenAgentsInc rust-lightning fork. A failed or BTC-only sweep cannot be
reported as asset recovery:

```json
"btc_sweep_without_asset_proof_refused": true
```

This is still not a live on-chain force-close. The remaining work is to wire
the bounded records through real channel-manager, resolver, and sweeper call
sites.

## Lightning Labs Compatibility

Track B is the Lightning Labs interop path. It uses Lightning Labs as an
independent counterparty, not as a wallet sidecar.

Current target:

- Bitcoin Core `30.0`;
- LND `0.19.0-beta`;
- `tapd` `0.7.0-alpha`.

Live asset-channel settlement currently targets integrated `litd`
`0.16.0-alpha`, because it runs LND plus taproot-assets with the aux funding
controller and taproot overlay channel support enabled. Standalone LND/`tapd`
remains useful for proof import/export and balance checks.

### Blob fixtures

`lightning_labs_blob.rs` decodes imported fixture hexdumps:

- funding blob;
- HTLC blob;
- commitment blob.

It extracts:

- decimal display;
- group key when present;
- funded asset outputs;
- asset balances;
- RFQ IDs;
- noop flags;
- local and remote asset outputs;
- outgoing and incoming HTLC asset outputs;
- aux leaves;
- STXO markers;
- raw digests.

Blob decoding is read-only. It must not mutate wallet state or silently skip
unsupported required fields.

### Funding interop

`lightning_labs_funding.rs` compares Lightning Labs funding and commitment
fixtures and persists a fixture-backed interop state.

It checks:

- funding total amount;
- local balance;
- remote balance;
- asset ID;
- funding proof digest;
- output digest;
- commitment blob digest;
- balance conservation;
- restart round trip.

It intentionally stores status as a documented gap until a live funding
outpoint and live proof chain are bound to the run.

### RFQ compatibility

`lightning_labs_rfq.rs` implements the Lightning Labs RFQ wire shape:

- message types `52884..52886`;
- transfer type for pay-invoice and receive-payment directions;
- RFQ ID;
- fixed-point rates;
- expiry;
- max/min asset fields;
- oracle metadata;
- execution policy;
- accept signature field;
- reject code.

It can convert between local quote-bound invoice state and Lightning Labs RFQ
payloads. It also derives the Lightning Labs SCID alias from the RFQ ID.

Signature validation against a live Lightning Labs peer is not implemented yet.
That belongs in the live daemon path.

### Proof compatibility

`tapd_proof.rs` and `wallet.rs` preserve Lightning Labs proof-file bytes.
Single `TAPP` proofs can be wrapped into `TAPF` proof files for tooling
compatibility. Proof import now requires the #60 semantic boundary: latest
Lightning Labs `TAPF` asset leaf, asset ID, normal demo asset type, amount,
script key, genesis outpoint, anchor staleness, and the native proof root must
agree before wallet state advances. Production full-history proof replay,
STXO/grouped-asset handling, and reorg hardening remain future production work.

### Payment reports

`lightning_labs_payment.rs` builds reports for both directions:

- `tap-ldk` pays Lightning Labs;
- Lightning Labs pays `tap-ldk`.

The reports include:

- channel ID;
- peer;
- RFQ ID;
- quote ID;
- asset ID;
- asset amount;
- BTC amount;
- payment hash;
- invoice context;
- Lightning Labs SCID alias;
- native SCID alias;
- before balances;
- expected after balances;
- message type IDs;
- request/accept data digests;
- asset HTLC digest;
- replay rejection;
- wrong-asset rejection;
- restart state match;
- documented live-daemon gap.

The reports deliberately reject any claim that an observed live balance exists
while the state is still fixture-backed. That is why they store
`observed_*_balance_after: null` today.

### Consolidated interop report

`lightning_labs_interop_checks.rs` combines:

- funding interop;
- Lightning Labs HTLC RFQ metadata fixture checks;
- proof fixture checks;
- RFQ request/accept/reject message-type checks;
- outgoing payment report;
- incoming payment report;
- fork-backed simple-taproot asset-channel lifecycle checks;
- cooperative close and proof-ownership recovery checks;
- restart round trips;
- mismatch diagnostics;
- documented gaps.

The current report can have:

```json
"all_automated_checks_passed": true,
"live_daemon_gaps_remaining": true
```

That means the fixture-backed checks passed, but live daemon settlement is not
done.

## Demo Scripts

### Path A

`scripts/path-a-native-demo.sh` runs the native-to-native demo.

It:

1. starts Bitcoin regtest;
2. mines a block;
3. creates Alice and Bob wallets;
4. issues `OPENUSD` to Alice;
5. sends a local proof file to Bob;
6. imports Bob's proof;
7. records wallet balances;
8. runs asset-channel funding smoke;
9. runs asset commitment smoke;
10. runs native payment smoke;
11. runs native recovery smoke;
12. runs native close smoke;
13. extracts close proof hex files;
14. writes a close/recovery status file;
15. copies Bob's wallet and checks restart balances;
16. prints a summary.

Artifacts land in:

```text
target/path-a-native-demo/<timestamp>/
```

### Path B

`scripts/path-b-lightning-labs-demo.sh` runs the Lightning Labs compatibility
demo as far as it can go today.

It:

1. records version information;
2. writes Lightning Labs counterparty config;
3. tries to print counterparty status;
4. tries to run the Lightning Labs counterparty smoke if Docker or Podman is
   available and healthy;
5. records an explicit dependency gap if the runtime is unavailable;
6. decodes blob fixtures;
7. decodes proof fixtures;
8. runs fixture-backed funding interop;
9. runs RFQ/invoice compatibility;
10. builds outgoing payment artifacts;
11. writes the live outgoing-payment gate artifact;
12. builds incoming payment artifacts;
13. writes consolidated interop checks;
14. prints the dependency gap and interop report.

When Docker is reachable, the live outgoing-payment gate also runs the
standalone proof-binding/current-balance path, starts integrated `litd`, and
runs the fork-backed `ldk-node` to `litd` path. It now reaches live
asset-channel funding, confirms the channel, sees `litd` report the channel
usable for asset keysend, settles a Lightning Labs to native asset keysend, and
records the native receiver balance through fork-backed `ldk-node`. It still
blocks at `live_asset_channel_payment_settlement` because after success native
LDK rejects `litd`'s zero-HTLC post-claim commitment with `Invalid
simple-taproot commitment partial signature`, and the local force-close
commitment broadcast fails with Taproot control-block errors.

Artifacts land in:

```text
target/path-b-lightning-labs-demo/<timestamp>/
```

### Full wrapper

`scripts/full-demo-smoke.sh` runs Path A and Path B into one artifact tree:

```text
target/full-demo-smoke/<timestamp>/
```

## Counterparty Harness

`scripts/lightning-labs-counterparty.sh` is supposed to start an independent
Lightning Labs topology:

- Bitcoin Core;
- LND;
- `tapd`.

It now detects Docker or Podman:

- default: prefer Docker, including the Docker Desktop app bundle CLI, then
  Podman;
- override: `TAP_LDK_CONTAINER_RUNTIME=docker`;
- override: `TAP_LDK_CONTAINER_RUNTIME=podman`.

The script performs the counterparty bootstrap needed before live Path B work
can attach to the Lightning Labs side:

- wait for bitcoind RPC;
- create or unlock the LND wallet;
- mine blocks;
- fund LND;
- wait for LND certificates and macaroon files;
- start `tapd` after LND is usable;
- wait for `tapd` RPC/REST;

The readiness report includes container names, image tags, LND and tapd node
pubkeys, chain heights, LND wallet balance, and credential file paths. It does
not print the Bitcoin RPC password or the LND wallet password.

The live `tapd` proof-binding command path now sits on top of that bootstrap:

- mint `OPENUSD` through `tapcli`;
- finalize the batch;
- mine confirmations;
- export the daemon TAPF proof;
- bind the proof into native `tap-ldk` wallet state.

Remaining live Path B work sits above the proof-binding path:

- open or attach an asset channel;
- run the live payment.

## Formal Models

The repo includes TLA+ models for bounded protocol surfaces:

- `asset_conservation`;
- `asset_channel`;
- `rfq_lifecycle`;
- `asset_commitment`;
- `asset_htlc`;
- `close_recovery`;
- `interop_handshake`.

Each model has:

- assumptions;
- boundaries;
- invariants;
- TLA+ spec;
- TLC config;
- counterexample handling note.

`scripts/formal-check.sh` runs checked-in models when TLC is available and
skips clearly when it is not.

Formal verification here is bounded. It does not prove Bitcoin consensus,
cryptographic primitives, or the full Lightning Network. It is used to keep
local state machines honest and to turn counterexamples into tests or explicit
model-boundary notes.

## Persistence Rules

Most stores follow the same pattern:

- schema version;
- metadata;
- map of records;
- validation before save;
- JSON pretty serialization;
- temp file write;
- rename into place;
- round-trip tests.

This applies to:

- wallet state;
- asset-channel funding store;
- asset commitment store;
- RFQ quote store;
- asset HTLC store;
- native payment store;
- close store;
- Lightning Labs funding interop store;
- Lightning Labs outgoing payment store;
- Lightning Labs incoming payment store.

The design goal is simple: if a state transition matters for the demo claim,
the artifact should survive restart and be inspectable.

## Safety Rules

The main safety rules are in `INVARIANTS.md`. The most important practical
rules are:

- No LND or `tapd` sidecar inside the `tap-ldk` wallet runtime.
- Normal BTC channels stay BTC-only unless asset support is explicitly
  negotiated.
- Malformed protocol data fails closed.
- Unknown even required TLV records fail.
- Asset-channel funding cannot advance without verified proof material.
- Asset balances cannot underflow, overflow, or drift from the committed
  total.
- Quotes are single-use and expiry-bound.
- Asset HTLCs require valid quote-bound metadata.
- Restart must not create, destroy, or hide asset balance.
- Cooperative close must export the latest valid owner proofs.
- Force-close asset recovery must not be claimed unless proof ownership
  survives the relevant commitment, second-level HTLC, or final sweep path.
- Lightning Labs mismatches are compatibility failures, not partial success.

## First-Demo Splice Policy

Concurrent simple-taproot splicing is outside the first public demo. The demo
opens a channel, pays through it, reconnects/reestablishes, cooperatively
closes or force-closes, and never changes the channel funding outpoint through
a splice.

This is deliberate. The OpenAgentsInc `rust-lightning` fork now validates
type-22 nonce maps for current and pending funding txids, but the repo does not
yet have bounded simple-taproot splice vectors proving that missing, stale,
duplicate, or wrong-funding-txid nonce-map entries fail closed for every
concurrent splice candidate. The first demo therefore makes no splice claim.

The machine-readable source for this boundary is
`tap_ldk_core::demo_scope::first_demo_protocol_scope`, exposed through:

```bash
cargo run -p tap-ldk-cli -- first-demo-scope
./scripts/check-simple-taproot-splice-policy.sh
```

Before any production simple-taproot claim, public splice demo, or Taproot
Asset channel path with concurrent splice/RBF candidates, #90 must be reopened
or superseded by tests that cover each active funding txid's type-22 nonce-map
entry.

## What Works End To End

Path A works end to end as a bounded native demo.

It proves that the repo currently has native Rust code for:

- issuing a demo asset;
- storing proof-backed balances;
- transferring proof material locally;
- funding a bounded single-asset channel;
- updating asset commitment state;
- binding RFQ to invoice/payment context;
- creating and validating asset HTLC metadata;
- settling a demo asset payment;
- restarting through key boundaries;
- cooperatively closing;
- exporting final proof artifacts;
- starting a local live `tap-ldk` peer and round-tripping an asset custom
  message after fork-backed negotiation.
- driving the bounded simple-taproot asset-channel lifecycle state in the
  OpenAgentsInc rust-lightning fork across funding, monitor persistence, HTLC
  settlement, cooperative close, proof-ownership recovery, restart roundtrip,
  and BTC-only isolation.

It does not prove:

- production stablecoin issuance;
- reserve management;
- compliance;
- live Lightning Labs routing;
- production-complete Taproot Assets proof history replay for grouped assets,
  STXO/split/change paths, reorg watcher handling, and every historical virtual
  transaction witness;
- live on-chain force-close recovery.
- concurrent simple-taproot splicing or splice/RBF asset-channel candidates.

## What Does Not Work Yet

The live Lightning Labs demo is incomplete.

Missing pieces:

1. Audit #19 against the Path B completion report. The report may only pass
   when the live gate stays green; fixture-only or expected-only values still
   cannot complete Path B.
2. Live force-close and sweep recovery remain future hardening outside the
   first public demo claim.

## How To Make Path B Fully Work

The shortest path is now the open issue sequence:

1. Keep #81, #57, #58, #59, #60, #61, and #71 green as regressions.
   - #81 proves Lightning Labs to native settlement.
   - #57 proves native to Lightning Labs settlement over the same integrated
     `litd` asset channel.
   - #58 proves the native receiver payment/balance checkpoint survives
     restart.
   - #59 proves the wrapper-level Path B completion report cannot pass from
     fixture-only or expected-only balances.
   - #60 proves imported proof material passes the semantic proof boundary
     before wallet or channel state advances.
   - #61 proves the first-demo BOLT simple-taproot base opens, pays,
     reestablishes, cooperatively closes, force-closes, and leaves legacy P2WSH
     channels unaffected.
   - #71 proves first-demo Taproot Assets-over-LDK primitives and channel state
     are layered onto the simple-taproot base while Path A, Path B, semantic
     proof validation, and BTC-only behavior stay green.
   - The live script is
     `./scripts/live-lightning-labs-outgoing-payment.sh`.
   - The wrapper completion report is
     `target/path-b-lightning-labs-demo-issue59/path-b-completion-report.json`.

2. Close #19 only after its acceptance criteria are actually met. The
   issue-by-issue closeout table lives in `docs/remaining-issue-closure-plan.md`.

## Design Boundaries

The project intentionally separates four layers:

1. Native asset semantics in `tap-ldk-core`.
2. LDK/rust-lightning channel integration in the OpenAgentsInc fork.
3. Lightning Labs compatibility as an external interop target.
4. Scripts and CLI as demo/operator surfaces.

That separation matters. If the Lightning Labs daemon performs wallet duties
for `tap-ldk`, the demo fails its own goal. If `tap-ldk` implements asset
semantics but never integrates with live LDK channel state, the demo is only a
state-machine smoke. The final proof of concept needs both: native asset logic
and real LDK channel integration.

## Review Checklist

When changing the architecture:

- Update `INVARIANTS.md` if a safety rule changes.
- Update `ROADMAP.md` if the issue sequence or scope changes.
- Update this file if a module, hook, or live-demo boundary changes.
- Add or update tests for every new protocol behavior.
- Keep fixture-backed, smoke-backed, and live-backed claims separate.
- Never claim live Lightning Labs success from expected-only balances.
- Never hide mocked pieces in public demo docs.
