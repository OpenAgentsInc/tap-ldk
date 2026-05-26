# BOLT Simple Taproot LDK Analysis

Date: 2026-05-26

Source reviewed:

- https://github.com/lightning/bolts/blob/master/bolt-simple-taproot.md

## Did We Use This Already?

No. The current `tap-ldk` repo and OpenAgentsInc `rust-lightning` fork do not
show direct implementation or direct roadmap dependency on
`bolt-simple-taproot.md`.

The existing docs mention that BLIP-TAP treats Taproot Asset channels as a
variant of simple taproot channels, and the Lightning Labs counterparty docs
mention `simple-taproot-overlay-chans`. Those references are directionally
right, but they are not the same as implementing the BOLT simple taproot
channel protocol inside LDK.

The current fork work is a useful bounded hook layer:

- experimental asset-channel feature and channel-type gates;
- funding controller validation hooks;
- monitor auxiliary blob persistence;
- asset HTLC metadata/final-hop validation hooks;
- cooperative close allocation validation;
- proof ownership recovery validation.

That work does not yet implement the BTC-only simple taproot channel state
machine. It also does not yet implement full Taproot Assets protocol semantics
such as the real MS-SMT, `AssetCommitment`, `TapCommitment`, TAP VM, virtual
transaction validation, or semantic proof ancestry.

## Why This Matters

Taproot Asset channels should not be built as asset metadata bolted onto the
legacy segwit-v0 Lightning commitment format. The BOLT simple taproot draft is
the intended lower layer for taproot-native Lightning channels. Asset-channel
work should extend that lower layer by adding the Taproot Assets commitment
leaf and asset-state rules.

The implementation dependency should be:

1. Implement BTC-only BOLT simple taproot channels in `rust-lightning`.
2. Implement full Taproot Assets protocol primitives in native Rust.
3. Integrate Taproot Asset channel state into the simple taproot LDK channel
   state machine.

Skipping step 1 risks building an asset-channel abstraction against the wrong
base transaction, signing, nonce, close, and monitor model.

## BOLT Simple Taproot Surfaces To Pull Into LDK

The BOLT draft mechanically translates existing Lightning funding and
commitment behavior onto Taproot, MuSig2, and tapscript. For LDK, that means
the following surfaces are core implementation work, not optional interop
notes.

### Feature Bits And Channel Type

LDK needs explicit negotiation for simple taproot channels. The draft defines
`option_simple_taproot` and a staging feature bit allocation. The
implementation must fail closed when the peer does not negotiate the required
feature or channel type.

Open questions:

- whether the fork uses final bits, staging bits, or an experimental
  OpenAgents-only feature namespace while the draft is unsettled;
- how to isolate this from current normal channel behavior;
- how the future Taproot Asset feature composes with the simple taproot
  channel-type bit.

### Wire TLVs

The draft adds TLVs for partial signatures, nonces, shutdown nonce handling,
and reestablishment nonce state. These need native LDK message codecs,
validation, retransmission behavior, and tests.

Required message coverage:

- `open_channel`;
- `accept_channel`;
- `funding_created`;
- `funding_signed`;
- `channel_ready`;
- `commitment_signed`;
- `revoke_and_ack`;
- `channel_reestablish`;
- `shutdown`;
- `closing_complete`;
- `closing_sig`.

### MuSig2 Nonce And Signature State

Simple taproot channels replace the current 2-of-2 funding multisig shape with
MuSig2 aggregate keys and Schnorr signatures. LDK needs a signer interface and
monitor persistence model that can:

- generate and verify public nonces;
- produce partial signatures;
- aggregate partial signatures where required;
- rotate nonces for commitments and cooperative close;
- prevent nonce reuse across commitment numbers, force-close paths, and asset
  signing sessions;
- persist enough verification nonce or counter-derived nonce state for
  unilateral close and restart recovery.

This is directly relevant to Taproot Assets because the asset layer also needs
asset-level signing and nonce discipline. The asset layer must not reuse the
BTC-level nonce material.

### P2TR Funding Transaction Flow

LDK must construct and verify funding outputs as P2TR outputs controlled by
the aggregate funding key. This includes BIP86-style handling where the output
has no script path and script-root handling where a script path exists.

Taproot Asset channels will later need to commit asset data into the output
tree. That work depends on the simple taproot funding output abstraction
already existing.

### Commitment Transactions

The simple taproot commitment format changes the script and witness model for:

- `to_local`;
- `to_remote`;
- anchor outputs;
- offered HTLC outputs;
- accepted HTLC outputs;
- second-level HTLC success transactions;
- second-level HTLC timeout transactions.

LDK must persist control-block material or enough reconstruction data for
script-path spends. This is also the right place to decide where Taproot Assets
commitment leaves are attached for asset channels.

### Channel Operation And Reestablish

`commitment_signed`, `revoke_and_ack`, and `channel_reestablish` need simple
taproot nonce and partial-signature behavior. This is a real channel state
machine change. Taproot Asset channel state must later be persisted before the
matching Lightning commitment is considered safe.

### RBF Cooperative Close

The draft defines taproot close extensions and nonce rotation for RBF
cooperative close. LDK asset close work must build on this rather than use a
parallel close path.

### HTLC Scripts And Second-Level Transactions

Asset HTLC semantics need to map onto the simple taproot HTLC output model. For
the single-asset first implementation, the asset commitment and asset proof
state should follow the same HTLC lifecycle as the BTC commitment state:
offer, sign, revoke, fulfill, fail, timeout, force-close, and sweep.

## Taproot Assets Layers Still Needed Above Simple Taproot

The current bounded implementation has a hash+sum root and asset-state smoke
coverage, but full Taproot Assets support needs the real protocol layers:

- strict TAP TLV parsing and canonical encoding;
- MS-SMT with hash+sum conservation and fixture compatibility;
- split commitments;
- `AssetCommitment`;
- `TapCommitment`;
- asset leaf inclusion and exclusion proofs;
- Taproot output commitment binding;
- virtual transaction construction and validation;
- TAP VM validation for state transitions;
- proof file parsing plus semantic ancestry validation;
- proof anchor import/export compatible with `tapd`;
- full owner proof recovery after cooperative close, force-close, HTLC success,
  HTLC timeout, and sweep paths.

## How To Include This In The Roadmap

The roadmap should make BOLT simple taproot an explicit prerequisite for full
Taproot Asset support in LDK. The current bounded asset hooks should remain,
but they should be treated as scaffolding until the simple taproot state
machine exists underneath them.

Implementation should be split into two epics:

1. BOLT simple taproot channels in OpenAgentsInc `rust-lightning`.
2. Full Taproot Assets support layered onto those channels.

The first epic should be able to pass with BTC-only channels before any asset
logic is enabled. The second epic should only claim completion once the asset
state is part of the real LDK funding, commitment, HTLC, close, monitor, and
recovery paths.

## Acceptance Bar

The project should not claim full native Taproot Asset support for LDK until:

- a BTC-only simple taproot channel can open, pay, reestablish, cooperatively
  close, and force-close in the `rust-lightning` fork;
- BOLT simple taproot test vectors or equivalent fixture tests cover wire
  TLVs, key aggregation, nonces, signatures, funding outputs, commitment
  outputs, close, and HTLC scripts;
- normal non-taproot LDK channels continue to pass existing tests;
- real MS-SMT, `AssetCommitment`, and `TapCommitment` fixtures pass;
- proof ancestry validation is semantic, not just checksum or root-field
  validation;
- asset funding, commitment, HTLC, close, and recovery state are persisted
  through LDK monitors before the matching Lightning commitment is considered
  safe;
- Lightning Labs `tapd` or `litd` interop shows observed balance changes and
  proof compatibility in both payment directions.
