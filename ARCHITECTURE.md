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

The Lightning Labs path is not a live payment yet:

- The repo can decode Lightning Labs funding, HTLC, commitment, RFQ, and proof
  fixtures.
- It can build fixture-backed reports for both payment directions.
- It can prove that expected balances are conserved in those reports.
- It does not yet drive a healthy live LND/`tapd` counterparty through asset
  funding and payment settlement.
- It does not yet query real live balances from both nodes after settlement.

The `rust-lightning` fork is wired but not yet patched with the full asset
channel implementation:

- `tap-ldk-core` depends on `OpenAgentsInc/rust-lightning` at the pinned base
  revision.
- The repo records which asset-channel hooks must live inside the fork.
- The native code models and tests those boundaries.
- The actual fork still needs the real feature, channel, funding, HTLC,
  monitor, close, and recovery hooks implemented.

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
  conservation, same-asset input merging, and bounded MS-SMT-style root
  summaries.
- `tlv`: strict BigSize/TLV encode and decode. It rejects non-canonical
  integers, duplicate records, out-of-order records, truncation, and unknown
  even required types.
- `proof`: bounded native proof format used by the local demo.
- `tapd_proof`: Lightning Labs `TAPP` single proof and `TAPF` proof-file
  envelope parsing.
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
- `derive_hash_sum_root` for deterministic hash+sum summaries.
- `validate_split_conservation` for transfer/split checks.
- `merge_same_asset_inputs` for funding multiple same-asset inputs.

This is not a full Taproot Assets VM. It is a bounded native model sufficient
for the current demo and tests. The full protocol still needs complete proof
ancestry, virtual transaction, anchor, and script validation.

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

### Native bounded proof path

`proof.rs` defines a small bounded proof file:

- version;
- asset ID;
- genesis outpoint;
- anchor outpoint;
- amount;
- script key;
- root hash;
- root sum;
- verification scope.

The only current verification scope is `bounded-anchor-only`. It checks that
the proof is structurally valid, nonzero, sum-conserving, and tied to plausible
outpoint strings. This is enough for the local demo. It is not full Taproot
Assets proof ancestry verification.

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
- optional unknown odd records.

`wallet.rs` can import a Lightning Labs `TAPF` proof file by preserving the raw
proof-file bytes and digest alongside a bounded local proof record. Export
returns the exact raw bytes when present.

Current limitation: the code preserves and checks the `TAPF` envelope and TLV
transport, but it does not yet verify complete semantic proof ancestry,
virtual transactions, or on-chain anchors.

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

## rust-lightning Fork Wiring

The workspace points at:

- fork: `https://github.com/OpenAgentsInc/rust-lightning.git`
- upstream: `https://github.com/lightningdevkit/rust-lightning.git`
- base revision: `0c37f08a55c0f7738f2691dc3690166fd42f851d`

`crates/tap-ldk-core/Cargo.toml` has a direct dependency:

```toml
lightning = { git = "https://github.com/OpenAgentsInc/rust-lightning.git", rev = "0c37f08a55c0f7738f2691dc3690166fd42f851d", package = "lightning" }
```

`ldk_fork.rs` checks that the fork is reachable and that important
rust-lightning feature types are available:

- `lightning::types::features::ChannelTypeFeatures`
- `lightning::types::features::InitFeatures`

That is the current fork integration. It proves `tap-ldk` can build against
the OpenAgentsInc fork and gives us a pinned place to land LDK changes.

It does not mean the fork already implements Taproot Assets channels. The
actual asset-channel hooks still have to be added to the fork.

## What Must Be Added To rust-lightning

`asset_channel_boundary.rs` is the typed list of rust-lightning/LDK extension
surfaces. It exists so we do not hand-wave where behavior belongs.

The following surfaces must move into the OpenAgentsInc rust-lightning fork for
a real live demo:

- Feature negotiation: asset-channel behavior must be behind explicit feature
  bits or negotiated channel flags.
- Channel type: normal BTC channels must not become asset channels implicitly.
- Funding controller: funding must be blocked until asset ID, proof root,
  funding output, and allocation checks pass.
- Commitment blob: asset-channel state must be versioned with the Lightning
  commitment number.
- HTLC metadata modifier: asset metadata must only be attached after an
  accepted quote.
- Final-hop validator: missing, stale, malformed, wrong-asset, or wrong-amount
  final-hop metadata must fail before settlement.
- Close handler: cooperative close must return the latest mutually valid asset
  allocation.
- On-chain resolver/sweeper: force-close and sweep handling must preserve proof
  ownership.
- Monitor persistence: asset-channel state must be durable before the
  corresponding Lightning commitment is treated as safe.

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

- asset ID;
- asset amount;
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

This is still a bounded model. Live rust-lightning funding hooks must enforce
the same checks inside the channel funding path before the live demo can be
called complete.

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
- monitor digest consistency.

The module models the asset-channel state that must eventually be stored with,
and updated alongside, rust-lightning channel monitor state.

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
- imports the final proofs into local wallets;
- rejects obsolete proof views;
- round-trips the close store to model restart.

The Path A script extracts:

- `native-close.json`;
- `native-close-local-proof.hex`;
- `native-close-remote-proof.hex`;
- `close-recovery-status.json`.

Force-close is not implemented. It is explicitly reported as deferred:

```json
"force_close_supported": false
```

That is intentional. A failed sweep cannot be reported as recovered, and the
demo must not claim force-close recovery until proof ownership and sweep state
are real.

## Lightning Labs Compatibility

Track B is the Lightning Labs interop path. It uses Lightning Labs as an
independent counterparty, not as a wallet sidecar.

Current target:

- Bitcoin Core `30.0`;
- LND `0.19.0-beta`;
- `tapd` `0.7.0-alpha`.

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
compatibility. Full semantic proof ancestry remains open.

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
- proof fixture checks;
- outgoing payment report;
- incoming payment report;
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
11. builds incoming payment artifacts;
12. writes consolidated interop checks;
13. prints the dependency gap and interop report.

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

- default: prefer Docker, then Podman;
- override: `TAP_LDK_CONTAINER_RUNTIME=docker`;
- override: `TAP_LDK_CONTAINER_RUNTIME=podman`.

Current limitation: the script starts containers, but it does not yet perform
the full daemon bootstrap needed for a live payment demo:

- wait for bitcoind RPC;
- create or unlock the LND wallet;
- mine blocks;
- fund LND;
- wait for LND certificates and macaroon files;
- start `tapd` after LND is usable;
- wait for `tapd` RPC/REST;
- mint/import a live asset;
- export a live proof;
- open or attach an asset channel;
- run the live payment.

On the current machine Docker is not installed. Podman exists, but its machine
was not reachable when last checked. The harness records that plainly as a
dependency gap.

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
- Force-close must not be claimed until proof ownership survives the on-chain
  path.
- Lightning Labs mismatches are compatibility failures, not partial success.

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
- exporting final proof artifacts.

It does not prove:

- production stablecoin issuance;
- reserve management;
- compliance;
- live Lightning routing;
- full Taproot Assets VM validation;
- force-close recovery.

## What Does Not Work Yet

The live Lightning Labs demo is incomplete.

Missing pieces:

1. A healthy live regtest runtime for Bitcoin Core, LND, and `tapd`.
2. Full daemon bootstrap in `scripts/lightning-labs-counterparty.sh`.
3. Live asset mint/import/export through `tapd`.
4. Live proof binding from `tapd` into `tap-ldk`.
5. Real asset-channel funding between `tap-ldk` and the Lightning Labs
   counterparty.
6. Real custom-message exchange through LDK/rust-lightning.
7. Real RFQ exchange with Lightning Labs peer/session semantics.
8. Real payment from `tap-ldk` to Lightning Labs.
9. Real payment from Lightning Labs to `tap-ldk`.
10. Observed balance checks from both live sides.
11. Full semantic proof ancestry validation.
12. Force-close and sweep recovery.

Until those are done, Track B must keep saying:

```json
"live_daemon_gaps_remaining": true
```

## How To Make Path B Fully Work

The shortest path is:

1. Fix local container runtime.
   - Install/start Docker Desktop, or fix/start Podman machine.
   - Confirm `docker info` or `podman info` works.

2. Harden `scripts/lightning-labs-counterparty.sh`.
   - Add readiness loops.
   - Create/unlock LND wallet.
   - Mine blocks.
   - Fund LND.
   - Wait for `tapd` credentials and RPC.
   - Record usable connection material.

3. Add live `tapd` asset setup.
   - Mint or import `OPENUSD`.
   - Mine confirmations.
   - Export proof files.
   - Record live asset ID and anchor outpoint.

4. Add a real `tap-ldk` peer process.
   - The current code is mostly smoke functions and JSON stores.
   - Live Path B needs a running LDK peer that can send and receive custom
     messages.

5. Patch the OpenAgentsInc rust-lightning fork.
   - Add feature/channel type gates.
   - Add funding controller hooks.
   - Add asset commitment monitor persistence hooks.
   - Add HTLC metadata hooks.
   - Add final-hop validation hooks.
   - Add close and recovery hooks.

6. Replace fixture-backed reports with live reports.
   - Keep fixture tests as regression coverage.
   - Add live artifact paths for the daemon run.
   - Query both sides after payment.
   - Only mark Track B complete when asset ID, payment state, and balances
     match.

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
