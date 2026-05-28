# OpenAgentsInc Rust-Lightning Fork

Date: 2026-05-25

The required `rust-lightning` fork for Taproot Asset channel work lives at:

- Fork: `https://github.com/OpenAgentsInc/rust-lightning`
- Upstream: `https://github.com/lightningdevkit/rust-lightning`
- Base revision: `0c37f08a55c0f7738f2691dc3690166fd42f851d`
- Current `tap-ldk` revision: `85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1`

This fork was created for issue #25 after the extension-boundary issue (#24)
identified hooks that must sit inside channel negotiation, funding,
commitment, HTLC, monitor persistence, close, and on-chain recovery logic.

## Current Wiring

`tap-ldk-core` has a direct git dependency on the forked `lightning` crate at
the pinned current revision. The dependency is intentionally narrow for this
phase: it proves CI can fetch and build against the OpenAgentsInc fork while
keeping fork touchpoints explicit.

Workspace metadata records the same fork in `Cargo.toml`:

```toml
[workspace.metadata.tap-ldk.rust-lightning-fork]
url = "https://github.com/OpenAgentsInc/rust-lightning.git"
upstream = "https://github.com/lightningdevkit/rust-lightning.git"
base_rev = "0c37f08a55c0f7738f2691dc3690166fd42f851d"
rev = "85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1"
```

Revision `99ddb8b7033b3b5d056005c00ba650e716ed37da` added the first forked
asset-channel gate: an experimental Taproot Asset channel feature bit, config
opt-in, explicit single-asset channel type, and a public descriptor API that
binds asset ID plus protocol version.

Revision `84032b87d05a157ee9ef247102767bc100d84ed6` adds the bounded funding
controller hook. It validates pending channel ID, peer identities, asset ID,
genesis/group identity, proof-fragment completeness, proof root, funding
outpoint, output commitment, and local/remote allocation before asset-channel
funding state is allowed to advance.

Revision `4394c0e350dd5faf34ca37fc6bde5cc14497e3f9` adds the first channel
monitor aux blob surface for asset commitments. It exposes
`TaprootAssetMonitorAuxBlob`, `TaprootAssetMonitorAuxBlobExpectation`,
`ChannelMonitorUpdate::taproot_asset_aux_update`, and
`ChannelMonitorUpdate::require_taproot_asset_aux_blob`, with validation for
missing, stale, malformed, or digest-mismatched asset state.

Revision `ef2538fe181025231c1f2a946df713b3109fa9ef` adds the first asset HTLC
metadata and final-hop validation surface. It exposes
`TaprootAssetHtlcMetadata`, `TaprootAssetHtlcMetadataExpectation`,
`prepare_asset_htlc_metadata`, and `validate_asset_htlc_final_hop`, with
validation for missing, stale, malformed, wrong-asset, wrong-amount,
wrong-root, wrong-quote, wrong-payment, or digest-mismatched metadata.

Revision `d6862145b43225d5002445c3733e70293bb0646e` adds the first cooperative
close allocation surface. It exposes `TaprootAssetCloseAllocation`,
`TaprootAssetCloseAllocationExpectation`,
`prepare_cooperative_close_asset_allocation`, and
`validate_cooperative_close_asset_allocation`, with validation for missing,
stale, malformed, wrong-asset, wrong-amount, wrong-root, or digest-mismatched
close allocations.

Revision `0f442683da45af47daff313fefcfaef1ac7b82d7` adds the first
force-close and sweep proof-ownership recovery surface. It exposes
`TaprootAssetProofOwnershipState`, `TaprootAssetProofOwnershipExpectation`,
`prepare_asset_proof_ownership_recovery`, and
`validate_asset_proof_ownership_recovery`, with validation for missing, stale,
wrong-path, malformed, partial BTC-only, missing-proof-material, or
digest-mismatched recovery state.

Revision `90054d8fc512eb9506955f27806b496e33d2b346` adds BOLT simple taproot
feature and channel-type plumbing. It defines the final `80/81` bits and the
draft staging `180/181` bits, advertises the staging bits behind
`ChannelHandshakeConfig::negotiate_simple_taproot_channels`, selects an
explicit staging channel type, fails closed when a peer requires unsupported
simple taproot, and requires the simple taproot staging base before the
experimental Taproot Asset channel type can be negotiated.

Revision `c237a0ae1189c0c59e27bdc8e8b99fd2bb018bcb` adds native
simple-taproot wire TLV support. It exposes fixed-width MuSig2 public nonce,
partial signature, partial-signature-with-nonce, and next-local-nonce payload
types; wires those optional TLVs into the open/accept, funding,
channel-ready, commitment, revoke/reestablish, shutdown, and RBF cooperative
close messages; and adds roundtrip plus malformed, duplicate, missing, and
unsupported TLV tests.

That TLV revision only defines and validates wire payload shapes. It does not
perform MuSig2 signing, nonce aggregation, partial-signature verification, or
final signature aggregation.

Revision `6e6b6c7b0407cd4cb0833228cfeb75ba5ccbb941` adds the first
feature-gated simple-taproot MuSig2 signer state. It wires the `musig2` Rust
crate behind `simple_taproot_musig2`, adds BIP-327 key sorting and aggregation,
counter/JIT nonce derivation, public nonce validation, partial-signature
generation and verification, final Schnorr aggregation, serializable nonce-use
state, nonce-reuse rejection tests, and `InMemorySigner` helper methods through
`SimpleTaprootChannelSigner`.

Revision `1602ac9e1e7454d39612e126c24a098e276d605a` adds BIP86 P2TR funding
script handling for BTC-only simple-taproot channels. It derives the funding
script from the sorted aggregate funding key, matches the BOLT funding vector,
emits P2TR scripts in `FundingGenerationReady`, rejects funding transactions
with the wrong script, and registers the P2TR funding script with channel
monitors. This is funding-output plumbing, not full live simple-taproot channel
completion; commitment output/control-block work and live MuSig2
channel-signing/reestablish wiring remain separate issues.

Revision `b0b952531329a31265f8de28752ee5334d9d9d4f` adds the first
simple-taproot commitment output model. It builds BOLT-vector-matching P2TR
to-local, to-remote, and anchor outputs; exposes tapscript roots, tap tweaks,
leaf scripts, and control blocks for deterministic spend reconstruction; emits
those outputs from `CommitmentTransaction` for simple-taproot channels; and
keeps legacy commitment outputs unchanged when simple taproot is not enabled.
This is still not complete live channel operation: MuSig2 commitment update
and reestablish state moved in issue #67, and HTLC scripts landed in issue
#69.

Revision `1176e837e5aacac7d1a3237c2bb00910989dbd93` adds the first
simple-taproot commitment update and reestablish state wiring. It persists
counterparty next-local nonces, consumed nonce uses, and sent commitment
partial signatures; emits next-local nonces in `channel_ready`,
`revoke_and_ack`, and `channel_reestablish`; verifies peer
`commitment_signed` partial signatures with the included JIT signing nonce;
reuses sent partials for retransmission; and fails closed when required
simple-taproot nonce/signature state is missing or cryptographically invalid.
Revision `99fee582d4061af4b0a030353b0a409ee542e064` corrects the LND staging
interop semantics: the advertised next-local nonce is future verification
state, not the commitment-signed signing nonce. This does not complete
cooperative close, force-close, HTLC second-level scripts, or vector replay.

Revision `26346a56af75eadf60763eb1e32a740656d4e384` adds simple-taproot
cooperative close wiring. It persists closee nonce state and sent
`closing_complete` partials, carries shutdown close nonces, handles
`closing_complete` and `closing_sig` under `simple_close`, aggregates close
partials into a P2TR key-path cooperative-close transaction, and fails closed
on missing or mismatched close nonce/signature state.

Revision `6af69ad385b864d7666edebbbbb668dab485bdde` adds simple-taproot HTLC
script and second-level transaction support. It emits offered and accepted
HTLC P2TR outputs from simple-taproot commitments, matches the BOLT HTLC and
second-level output vectors, treats simple-taproot HTLC transactions as
zero-fee second-level spends with sequence `1`, signs BIP342 tapscript spends
with `SIGHASH_SINGLE|ANYONECANPAY`, builds separate witness stacks for each
offered/accepted success and timeout path, forwards MuSig2 signer methods
through test/dynamic signer wrappers, and unignores the cooperative close
functional harness.

Revision `983c4385ff66105ab70d766d34f49c1bd547a81a` adds the BOLT
simple-taproot vector replay pass for the surfaces implemented so far. The
tests now pin BOLT TLV payload shapes, nonce and partial-signature wire
payloads, funding scripts, commitment output scripts and leaf hashes, close
harness behavior, HTLC scripts, second-level outputs, and multi-HTLC
transaction value/trimming cases. The draft transaction JSON currently differs
from the script-vector section for some multi-HTLC output keys, so exact script
assertions stay on the unambiguous script vectors while transaction coverage
checks output count, values, ordering, P2TR shape, and trimming.

Revision `99fee582d4061af4b0a030353b0a409ee542e064` adds
`TaprootAssetChannelState`, the bounded rust-lightning-side lifecycle state
for a single-asset channel layered on simple taproot. It ties explicit
simple-taproot asset-channel negotiation, proof-backed funding, monitor aux
blob persistence, asset commitment advancement, HTLC metadata validation,
cooperative close allocation, and proof-ownership recovery to one state object.
It also aligns the experimental Taproot Asset overlay negotiation with
Lightning Labs `taproot-overlay-chans` feature bits and adds outgoing Init
custom TLV support so `ldk-node` can advertise the Taproot Assets aux feature
record used by `litd`. The live `litd` funding trace also showed that
Lightning Labs uses zero CSV for Taproot Asset allocation/script-key
derivation, but uses the negotiated channel CSV delay for the actual Bitcoin
commitment to-local aux output. This revision preserves that split and adds the
matching script-vector regression coverage.
`tap-ldk` exercises this lifecycle with
`simple-taproot-asset-channel-smoke`.

Revision `0d6ac878453bcc108f315d69aae0bda625c1f871` adds strict decoding for
the live Lightning Labs Taproot Asset HTLC blob and an HTLC aux-leaf output
hook. That lets Rust Lightning reject malformed live asset HTLC payloads and
carry asset-derived aux leaves into HTLC output construction. The remaining
#81 work is exact Lightning Labs-compatible per-commitment HTLC and change aux
leaf construction from asset-channel state.

Revision `5bd5992ac7f7625f254e5df67eec66d085fe7c7d` persists the live Taproot
Asset HTLC blob through inbound/outbound HTLC state and holding-cell
serialization, writes optional blob vectors under channel TLVs `95`, `97`, and
`99`, and re-emits the stored blob on outbound `update_add_htlc`. This closes
the blob-loss gap but does not yet derive the Lightning Labs-compatible
Taproot Asset HTLC and change output scripts that `litd` signs.

Revision `a7cb50c64ba589e1171526f04f199d09cac35812` sorts Taproot Asset
simple-taproot commitment outputs by the base no-aux P2TR script while keeping
the final aux-leaf P2TR script in the transaction. This matches the Lightning
Labs allocation/custom-commit sort rule for the initial funding commitment.
Revision `4761230b3d8a2732d379087a5510456a13b86c29` preserves and decodes
Lightning Labs `commitment_signed` TLV 65537 asset-signature blobs and
requires Taproot Asset channels with non-dust HTLCs to carry one decoded
signature group per HTLC. The same revision persists a proof-derived
single-asset channel template in `ChannelTransactionParameters` and derives the
first full-channel HTLC aux leaf for payment-time commitment outputs. It also
interprets the existing 64-byte BOLT HTLC signature field as a raw BIP340
Schnorr signature for simple-taproot HTLC transaction verification.
Revision `85189ebe7d3c3b0cf92d504c06e0e3b192a5e5c1` keeps the same fail-closed
policy and adds trace diagnostics for the rejected HTLC signature transcript:
previous output, HTLC tx outputs, aux leaves, control block, sighash type,
computed sighash, signature, and verifying key.

With `ldk-node@c5ae040bf84225922c5213d9acb077e031076a9c`, the current pin is
ready for the next live run to capture that transcript. The previous live run
confirmed that `litd` `fundchannel` completes, the channel confirms, and
`litd` reports a keysend-usable local asset balance. The live asset keysend
still stays `IN_FLIGHT` after Rust Lightning closes on `Invalid
simple-taproot HTLC signature from peer`, so #81 remains open until the fork
matches Lightning Labs' exact HTLC signature leaf, sighash, key selection, and
witness/control block construction and records observed balances. Partial
split/change-output support remains later #71/#60 work after the bounded live
path settles.

Issue #61 remains open even though #62 through #70 and #75 are implemented.
The epic closes only after BTC-only simple-taproot LDK channels open, pay,
reestablish, cooperatively close, force-close, and prove legacy channels are
unaffected in live channel-manager paths. Issue #71 remains open until the
Taproot Assets overlay is wired through those live paths and Path B interop
records observed live balances.

As broader forked code lands, the dependency strategy may need to move from a
direct touchpoint dependency to explicit `[patch.crates-io]` entries for the
LDK crates affected by the fork. Do that only when the fork changes require
patching transitive LDK crates, so normal BTC behavior remains easy to compare
against upstream.

## Sync Process

1. Add upstream locally when working inside an owned fork clone:

   ```bash
   git remote add upstream https://github.com/lightningdevkit/rust-lightning.git
   git fetch upstream
   ```

2. Rebase or merge only after confirming normal upstream BTC-only tests still
   pass.
3. Keep asset-channel changes feature-gated and isolated to the boundaries in
   `docs/ldk-asset-channel-extension-boundary.md`.
4. Update this document, `Cargo.toml` workspace metadata, and the pinned Cargo
   dependency revision whenever the fork base changes.

## Drift Rules

- Reference clones under `projects/` remain read-only source material.
- The OpenAgentsInc fork is the implementation home for forked
  rust-lightning code.
- Do not weaken normal channel policy, monitor durability, or BTC-only tests to
  make asset-channel work pass.
- Any upstream conflict in monitor, HTLC, close, or recovery code is a protocol
  review event.
