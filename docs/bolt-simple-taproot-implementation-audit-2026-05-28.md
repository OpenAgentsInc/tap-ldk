# BOLT Simple Taproot Implementation Audit

Date: 2026-05-28

Spec source:
https://raw.githubusercontent.com/lightning/bolts/refs/heads/master/bolt-simple-taproot.md

Implementation audited:

- `OpenAgentsInc/rust-lightning@4a3cea6d859d172144e7010a38dc821db7fa5a5b`
- `OpenAgentsInc/ldk-node@eb61dde920493afe1037ec299888c10bc353e33a`
- `tap-ldk` pinned to those forks

Follow-up production audit: #91 through #95 now track the remaining work for a
production-complete BOLT simple-taproot claim. See
`docs/bolt-simple-taproot-production-compliance-audit-2026-05-28.md`.

## Summary

The fork implements a large part of the draft BOLT simple-taproot base: feature
and channel-type bits, fixed-width MuSig2 TLVs, nonce parsing, MuSig2 signing
helpers, BIP86 funding outputs, simple-taproot commitment output construction,
HTLC script helpers, second-level HTLC signing, reestablish/RAA nonce fields, and
cooperative-close message types.

It does not fully implement the current spec yet. Follow-up work through #87
fixes the force-close funding-input failure that produced
`Invalid Taproot control block size`, the private-channel rule for
simple-taproot and Taproot Asset opens, immediate open/accept nonce validation,
and the RAA/reestablish nonce field path:

- local unilateral fallback now persists the aggregate holder MuSig2
  commitment signature and spends the simple-taproot funding output with a
  one-element key-path Schnorr witness;
- outbound simple-taproot and Taproot Asset opens clear `announce_channel`,
  while inbound public simple-taproot/Taproot Asset opens fail closed;
- RAA and reestablish use the Lightning Labs staging scalar nonce for
  single-funding overlay interop, keep type-22 nonce maps for final or
  multi-funding paths, and fail closed on scalar fallback when more than one
  funding txid is active;
- cooperative close exists behind `simple_close` plus
  `simple_taproot_musig2`, and #89 proves the native key-path witness plus
  Taproot Asset close-allocation restart boundary;
- splice nonce maps have some multi-funding plumbing, but concurrent splice
  behavior has not been proven against the draft's full active-funding map
  requirements and is explicitly excluded from the first public demo by #90.

The audit result is therefore: **first-demo scoped, not production splice
complete**. The #81 live settlement blocker is now cleared; keep the #81, #57,
#58, #59, #60, and #61 live/proof/reporting/simple-taproot gates green as
regressions for the completed Path B first-demo work. The
rest of the BOLT
conformance work is split into the issue set in
`docs/bolt-simple-taproot-spec-compliance-issues.md`.

Follow-up update: `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
now serializes 64 zero bytes for the legacy `signature` field when a
simple-taproot MuSig2 partial-signature TLV is present in `funding_created`,
`funding_signed`, or `commitment_signed`, rejects non-zero legacy fields on
decode, derives post-claim Taproot Asset balance script keys with the real CSV
delay, includes a live `litd` zero-HTLC post-claim transcript regression, and
persists the aggregate holder commitment Schnorr signature needed for
simple-taproot key-path force-close broadcast.
`OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4` pins those
fixes for the live runtime.
That first-demo fork line also enforces the draft private-channel rule:
simple-taproot and Taproot Asset outbound opens do not set `announce_channel`,
and inbound public opens for those channel types are rejected.
It also enforces immediate type-4 nonce presence for simple-taproot and
Taproot Asset `open_channel` and `accept_channel` handling, while legacy
channels can still omit the TLV.
It also fixes RAA/reestablish nonce field selection: Lightning Labs
staging/overlay single-funding interop uses the legacy scalar next-local nonce,
while final simple-taproot and multi-funding paths use type-22
`next_local_nonces`; scalar fallback fails closed when more than one funding
txid is active.

## Why This Matters For #81

The latest live post-claim-fix run reaches real settlement:

- `litd` completed asset issuance and asset-channel funding;
- `litd` sent the asset keysend and reported `SUCCEEDED`;
- native LDK recorded `PaymentClaimable` and `PaymentClaimed`;
- fork-backed `ldk-node` recorded native receiver balance `125`;
- `native_ldk_invalid_commitment_logged=false`;
- `native_ldk_invalid_simple_taproot_partial_sig_logged=false`;
- `native_ldk_invalid_simple_taproot_commitment_partial_sig_logged=false`;
- `native_ldk_invalid_taproot_control_block_logged=false`.

The earlier control-block failure came from using a legacy P2WSH 2-of-2 witness
to spend the P2TR funding output. Bitcoin Core interpreted that multi-element
witness as a Taproot script-path spend and treated the legacy funding script as
a malformed control block. The correct holder commitment broadcast path is a
key-path funding spend with only the final aggregate BIP340 signature in the
witness. Output spends still use their own script-path leaves and control
blocks.

## Audit Matrix

| Spec area | Spec requirement | Current implementation | Status | Required work |
| --- | --- | --- | --- | --- |
| Feature bits | Define final `option_simple_taproot` bits 80/81 and staging bits 180/181; use explicit channel type. | `lightning-types/src/features.rs` defines final and staging bits. `ChannelHandshakeConfig::negotiate_simple_taproot_channels` advertises staging only. | Partial | Keep staging for `litd` interop, but document final-bit dependency on `option_simple_close` before enabling final bits. |
| Public-channel prohibition | A simple-taproot opener must not set `announce_channel`. | Outbound simple-taproot and Taproot Asset opens clear the public bit when those channel types are selected; inbound public opens for those channel types fail closed. Regressions cover BTC-only simple-taproot, Taproot Asset, and peer-provided public channel types. | Implemented | Keep legacy public BTC channel behavior covered while later BOLT work proceeds. |
| TLV wire types | Fixed TLV payloads: type 2 partial signature with nonce, type 4 next local nonce, type 6 partial signature, type 8 shutdown nonce, type 22 nonce map. | `lightning/src/ln/simple_taproot.rs` defines the fixed constants and validates 66-byte public nonces as two compressed secp points. `msgs.rs` round-trips these TLVs. | Implemented | Keep vector tests pinned to the upstream BOLT fixture payloads. |
| `open_channel` / `accept_channel` nonces | Messages must include type 4 `next_local_nonce`; receivers fail on absent or invalid nonces. | Open/accept generation derives counter nonces. `channel_type_from_open_channel` and `do_accept_channel_checks` now reject simple-taproot and Taproot Asset opens/accepts when the nonce is absent; legacy channel types can still omit it. Invalid points fail parse. | Implemented | Keep message-level and channel-level missing-nonce regressions. |
| Funding partials | `funding_created` and `funding_signed` legacy `signature` field must be 64 zero bytes; type 2 MuSig2 partial must be present and valid. | Type 2 MuSig2 partials are generated and validated. Current wire serialization emits zero legacy fields when the simple-taproot TLV is present and rejects non-zero peer legacy fields. | Implemented for wire field | Keep live interop and functional coverage around funding message acceptance. |
| `channel_ready` nonce | Message must include a fresh type 4 nonce. | `check_get_channel_ready` sends a nonce and `channel_ready` handling requires it for simple-taproot funding. | Implemented | Add a missing-nonce functional regression if not already covered by message tests. |
| `commitment_signed` partial | Legacy `signature` field must be zero; type 2 partial must verify; HTLC signatures must be BIP340 Schnorr in the existing HTLC field. | Type 2 partial validation and BIP340 HTLC verification exist. Current wire serialization emits a zero legacy field when the simple-taproot TLV is present and rejects non-zero peer legacy fields. The live post-claim `litd` transcript now verifies with fixture coverage. | Implemented for current live path | Keep the live transcript fixture and add more spec vectors as upstream publishes them. |
| `revoke_and_ack` nonce map | Type 22 `next_local_nonces` must include one entry for each active funding txid. | Final or multi-funding simple-taproot paths build and validate a type-22 map. Lightning Labs staging/overlay single-funding interop sends and accepts the legacy scalar nonce. Scalar fallback fails closed when more than one funding txid is active. | First-demo staging interop implemented; final/multi-funding map path retained | Concurrent splice candidates are out of first-demo scope under #90. |
| `channel_reestablish` nonce map | Type 22 must be sent and checked for every active commitment; retransmitted commits must regenerate partials with new nonces. | Final or multi-funding reestablish uses type-22 maps. Lightning Labs staging/overlay single-funding reestablish uses the legacy scalar nonce. | First-demo staging interop implemented; final/multi-funding map path retained | Concurrent splice candidates are out of first-demo scope under #90. |
| Splice coordination | Every active splice/funding txid needs a distinct nonce entry. | Expected txid calculation includes current funding and pending funding. No live or vector proof of concurrent splice maps exists, so the first public demo excludes concurrent simple-taproot splicing. | Explicit first-demo exclusion | Reopen before any production/simple-taproot-complete claim, any public splice demo, or any Taproot Asset channel claim using concurrent splice/RBF candidates. |
| BIP86 funding output | Funding output must be P2TR over MuSig2 KeyAgg(KeySort(funding keys)). | `SimpleTaprootKeyAggContext` builds BIP86 funding scripts and has BOLT vector replay. | Implemented | Keep vector coverage. |
| To-local output | NUMS internal key, delay/revocation leaves, correct parity-bearing control blocks, delay sequence. | `simple_taproot_to_local_spend_info` builds delay and revocation leaves and test coverage checks control-block lengths. Taproot Asset aux leaves alter tree depth. The #84 live invalid-control-block symptom was the funding-input witness, not a to-local output control block. | Partial | Keep to-local output-spend coverage in the broader BTC-only force-close and Taproot Asset recovery gates. |
| To-remote output | The draft prose is internally inconsistent here, but the vectors use the global simple-taproot NUMS point, a single CSV-1 script leaf, and sequence 1 spend. | `simple_taproot_to_remote_spend_info` builds the script and uses the global BOLT NUMS point, matching the vectors. | Implemented for base | Confirm Taproot Asset aux-leaf depth/control-block behavior in live force-close tests. |
| Anchor outputs | Anchor internal key is local delayed key or remote payment key; script is `16 CSV`; omit anchor if corresponding output absent and no HTLCs. | `chan_utils.rs` emits simple-taproot anchors under the BOLT conditions. | Implemented | Add/keep regression for no-output/no-HTLC anchor omission. |
| HTLC outputs | Offered/accepted HTLCs are P2TR with revocation internal key and split timeout/success leaves. | `simple_taproot_htlc_spend_info_with_aux_leaf_for_variant` implements final and staging variants and can include Taproot Asset aux leaves. | Partial | The base is covered, but live asset aux-leaf transcript must remain fixture-backed for both directions. |
| Second-level HTLCs | Version 2, sequence 1, zero-fee semantics, SIGHASH_SINGLE|ANYONECANPAY, one delayed output. | `build_htlc_transaction`, `simple_taproot_htlc_sighash_type`, and package/signing code use sequence 1 and `SinglePlusAnyoneCanPay` for simple-taproot/Taproot Asset HTLCs. | Implemented for current path | Keep previous-output-bound Taproot Asset aux-leaf regressions. |
| Cooperative close | `shutdown`, `closing_complete`, and `closing_sig` carry MuSig2 nonces/partials; aggregate final Schnorr signature; rotate closee nonces for RBF. | Message structs and channel logic exist behind `simple_close` plus `simple_taproot_musig2`; shutdown nonce persistence exists. #89 asserts that native cooperative close broadcasts the same final tx on both peers and spends the P2TR funding input with one 64-byte Schnorr witness. Taproot Asset close checks preserve the latest asset allocation across close-store restart. | Implemented for native and fixture boundary | Live `litd` post-close proof/balance observation remains a Path B documented gap, not a BOLT base blocker. |
| Formal/spec vectors | BOLT vectors should cover TLVs, scripts, commitments, HTLCs, signatures, and trimming. | Vector replay exists for implemented base surfaces. The live post-claim zero-HTLC transcript is fixture-backed. #84 adds a stable holder force-close funding-input witness assertion for the one-element key-path Schnorr witness. #88 adds a BTC-only simple-taproot lifecycle gate covering open, payment, reconnect/reestablish, functional cooperative close, force-close funding witness shape, and legacy P2WSH isolation. #89 adds a cooperative-close gate with `simple_close`. #90 adds a machine-readable first-demo splice exclusion and verification wrapper. | First-demo scoped | Add bounded splice vectors before any production splice claim. |

## Required Work From This Audit

Completed follow-up from this audit:

- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  adds a focused regression for simple-taproot legacy signature zeroing and
  non-zero legacy signature rejection in `funding_created`, `funding_signed`,
  and `commitment_signed`.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  also persists the aggregate simple-taproot holder commitment signature in
  `HolderCommitmentTransaction`, uses it in the on-chain holder funding-output
  package path, and asserts that the latest holder commitment transaction spends
  the P2TR funding output with exactly one 64-byte Schnorr witness element.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  enforces private-only simple-taproot and Taproot Asset channel opens while
  preserving legacy public BTC channel behavior.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  rejects missing type-4 `next_local_nonce` during simple-taproot and Taproot
  Asset `open_channel`/`accept_channel` handling before channel state advances.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  sends Lightning Labs staging scalar RAA/reestablish nonces for single-funding
  overlay interop, keeps type-22 maps for final or multi-funding paths, and
  rejects scalar fallback when multiple funding txids are active.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  adds a BTC-only simple-taproot conformance gate for open, P2TR funding,
  payments across reconnect/reestablish, functional cooperative close,
  force-close key-path funding witness shape, and legacy P2WSH channel
  isolation. `tap-ldk` wraps that gate in
  `./scripts/check-btc-simple-taproot-conformance.sh`.
- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
  also asserts the cooperative-close final transaction's P2TR funding input has
  a single 64-byte key-path witness. `tap-ldk-core` records that Taproot Asset
  cooperative close preserves the latest allocation across close-store restart,
  and `tap-ldk` wraps the native gate in
  `./scripts/check-simple-taproot-cooperative-close.sh`.
- `tap-ldk-core::demo_scope` and `tap-ldk-cli first-demo-scope` explicitly
  exclude concurrent simple-taproot splicing from the first public demo. Run
  `./scripts/check-simple-taproot-splice-policy.sh` to verify that policy while
  still running the pinned fork's simple-taproot and splicing filters.

Remaining work that should be tracked outside #81 before #61 closes:

1. For the first-demo claim, no BOLT audit item remains ambiguous: concurrent
   splicing is out of scope. For production/simple-taproot-complete claims,
   add bounded splice nonce-map coverage before removing that scope qualifier.
2. This is sufficient to close #82 for first-demo scope, but not sufficient to
   claim production splice support.

## Closure Rule

#81 may close from the current pin plus the completed live rerun at
`target/live-lightning-labs-outgoing-payment-issue81-rerun/report.json`. #61 is
closed for first-demo scope, but
must not be described as production-complete simple-taproot support until
bounded splice nonce-map vectors replace the first-demo splice exclusion. Keep
#60, #61, and #71 green as semantic proof, simple-taproot, and first-demo
Taproot Assets-over-LDK regression gates.
