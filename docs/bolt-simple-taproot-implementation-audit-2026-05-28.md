# BOLT Simple Taproot Implementation Audit

Date: 2026-05-28

Spec source:
https://raw.githubusercontent.com/lightning/bolts/refs/heads/master/bolt-simple-taproot.md

Implementation audited:

- `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18`
- `OpenAgentsInc/ldk-node@6d44b0bda8305b71544c9996ea23b7ab653b8ce2`
- `tap-ldk` pinned to those forks

## Summary

The fork implements a large part of the draft BOLT simple-taproot base: feature
and channel-type bits, fixed-width MuSig2 TLVs, nonce parsing, MuSig2 signing
helpers, BIP86 funding outputs, simple-taproot commitment output construction,
HTLC script helpers, second-level HTLC signing, reestablish/RAA nonce maps, and
cooperative-close message types.

It does not fully implement the current spec yet. The latest #84 follow-up
fixes the force-close funding-input failure that produced
`Invalid Taproot control block size`, and the latest #85 follow-up fixes the
private-channel rule for simple-taproot and Taproot Asset opens:

- local unilateral fallback now persists the aggregate holder MuSig2
  commitment signature and spends the simple-taproot funding output with a
  one-element key-path Schnorr witness;
- outbound simple-taproot and Taproot Asset opens clear `announce_channel`,
  while inbound public simple-taproot/Taproot Asset opens fail closed;
- cooperative close exists behind `simple_close` plus
  `simple_taproot_musig2`, but it is not yet live-proven for the demo channel;
- splice nonce maps have some multi-funding plumbing, but concurrent splice
  behavior has not been proven against the draft's full active-funding map
  requirements.

The audit result is therefore: **not spec complete**. The next #81 work should
stay limited to the live settlement blocker. The rest of the BOLT conformance
work is split into the issue set in
`docs/bolt-simple-taproot-spec-compliance-issues.md`.

Follow-up update: `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18`
now serializes 64 zero bytes for the legacy `signature` field when a
simple-taproot MuSig2 partial-signature TLV is present in `funding_created`,
`funding_signed`, or `commitment_signed`, rejects non-zero legacy fields on
decode, derives post-claim Taproot Asset balance script keys with the real CSV
delay, includes a live `litd` zero-HTLC post-claim transcript regression, and
persists the aggregate holder commitment Schnorr signature needed for
simple-taproot key-path force-close broadcast.
`OpenAgentsInc/ldk-node@6d44b0bda8305b71544c9996ea23b7ab653b8ce2` pins those
fixes for the live runtime.
The same current fork line also enforces the draft private-channel rule:
simple-taproot and Taproot Asset outbound opens do not set `announce_channel`,
and inbound public opens for those channel types are rejected.

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
| `open_channel` / `accept_channel` nonces | Messages must include type 4 `next_local_nonce`; receivers fail on absent or invalid nonces. | Open/accept generation derives counter nonces. Missing peer nonces are stored as `None` and later cause signing/validation failure; invalid points fail parse. | Partial | Fail immediately during open/accept validation for simple-taproot channels when the nonce is absent. |
| Funding partials | `funding_created` and `funding_signed` legacy `signature` field must be 64 zero bytes; type 2 MuSig2 partial must be present and valid. | Type 2 MuSig2 partials are generated and validated. Current wire serialization emits zero legacy fields when the simple-taproot TLV is present and rejects non-zero peer legacy fields. | Implemented for wire field | Keep live interop and functional coverage around funding message acceptance. |
| `channel_ready` nonce | Message must include a fresh type 4 nonce. | `check_get_channel_ready` sends a nonce and `channel_ready` handling requires it for simple-taproot funding. | Implemented | Add a missing-nonce functional regression if not already covered by message tests. |
| `commitment_signed` partial | Legacy `signature` field must be zero; type 2 partial must verify; HTLC signatures must be BIP340 Schnorr in the existing HTLC field. | Type 2 partial validation and BIP340 HTLC verification exist. Current wire serialization emits a zero legacy field when the simple-taproot TLV is present and rejects non-zero peer legacy fields. The live post-claim `litd` transcript now verifies with fixture coverage. | Implemented for current live path | Keep the live transcript fixture and add more spec vectors as upstream publishes them. |
| `revoke_and_ack` nonce map | Type 22 `next_local_nonces` must include one entry for each active funding txid. | `simple_taproot_next_local_nonces` builds a map across current and pending funding; receipt validates expected txids. A legacy scalar type 4 compatibility path is still accepted for a single funding txid. | Partial | Make type 22 the authoritative path for spec mode; keep scalar compatibility only under an explicit `litd` compatibility note or remove it after interop is stable. |
| `channel_reestablish` nonce map | Type 22 must be sent and checked for every active commitment; retransmitted commits must regenerate partials with new nonces. | Reestablish sends the nonce map and stores received maps. Sent commitment signatures are persisted by funding txid and nonce index. | Partial | Add a reconnect test that forces retransmission and proves the partial is regenerated against the newly received nonce map. |
| Splice coordination | Every active splice/funding txid needs a distinct nonce entry. | Expected txid calculation includes current funding and pending funding. No live or vector proof of concurrent splice maps exists. | Partial | Add bounded splice nonce-map tests or mark splicing out of the first demo's acceptance criteria. |
| BIP86 funding output | Funding output must be P2TR over MuSig2 KeyAgg(KeySort(funding keys)). | `SimpleTaprootKeyAggContext` builds BIP86 funding scripts and has BOLT vector replay. | Implemented | Keep vector coverage. |
| To-local output | NUMS internal key, delay/revocation leaves, correct parity-bearing control blocks, delay sequence. | `simple_taproot_to_local_spend_info` builds delay and revocation leaves and test coverage checks control-block lengths. Taproot Asset aux leaves alter tree depth. The #84 live invalid-control-block symptom was the funding-input witness, not a to-local output control block. | Partial | Keep to-local output-spend coverage in the broader BTC-only force-close and Taproot Asset recovery gates. |
| To-remote output | The draft prose is internally inconsistent here, but the vectors use the global simple-taproot NUMS point, a single CSV-1 script leaf, and sequence 1 spend. | `simple_taproot_to_remote_spend_info` builds the script and uses the global BOLT NUMS point, matching the vectors. | Implemented for base | Confirm Taproot Asset aux-leaf depth/control-block behavior in live force-close tests. |
| Anchor outputs | Anchor internal key is local delayed key or remote payment key; script is `16 CSV`; omit anchor if corresponding output absent and no HTLCs. | `chan_utils.rs` emits simple-taproot anchors under the BOLT conditions. | Implemented | Add/keep regression for no-output/no-HTLC anchor omission. |
| HTLC outputs | Offered/accepted HTLCs are P2TR with revocation internal key and split timeout/success leaves. | `simple_taproot_htlc_spend_info_with_aux_leaf_for_variant` implements final and staging variants and can include Taproot Asset aux leaves. | Partial | The base is covered, but live asset aux-leaf transcript must remain fixture-backed for both directions. |
| Second-level HTLCs | Version 2, sequence 1, zero-fee semantics, SIGHASH_SINGLE|ANYONECANPAY, one delayed output. | `build_htlc_transaction`, `simple_taproot_htlc_sighash_type`, and package/signing code use sequence 1 and `SinglePlusAnyoneCanPay` for simple-taproot/Taproot Asset HTLCs. | Implemented for current path | Keep previous-output-bound Taproot Asset aux-leaf regressions. |
| Cooperative close | `shutdown`, `closing_complete`, and `closing_sig` carry MuSig2 nonces/partials; aggregate final Schnorr signature; rotate closee nonces for RBF. | Message structs and channel logic exist behind `simple_close` plus `simple_taproot_musig2`; shutdown nonce persistence exists. | Partial | Run native and `litd` cooperative close coverage before closing #61 or #71. |
| Formal/spec vectors | BOLT vectors should cover TLVs, scripts, commitments, HTLCs, signatures, and trimming. | Vector replay exists for implemented base surfaces. The live post-claim zero-HTLC transcript is fixture-backed. #84 adds a stable holder force-close funding-input witness assertion for the one-element key-path Schnorr witness. | Partial | Add broader BTC-only force-close and output-spend assertions before closing #61. |

## Required Work From This Audit

Completed follow-up from this audit:

- `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18`
  adds a focused regression for simple-taproot legacy signature zeroing and
  non-zero legacy signature rejection in `funding_created`, `funding_signed`,
  and `commitment_signed`.
- `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18`
  also persists the aggregate simple-taproot holder commitment signature in
  `HolderCommitmentTransaction`, uses it in the on-chain holder funding-output
  package path, and asserts that the latest holder commitment transaction spends
  the P2TR funding output with exactly one 64-byte Schnorr witness element.
- `OpenAgentsInc/rust-lightning@98e25016540ed98b450a2bf270d8d50c846f1d18`
  enforces private-only simple-taproot and Taproot Asset channel opens while
  preserving legacy public BTC channel behavior.

Remaining work that should be tracked outside #81 before #61 closes:

1. Fail simple-taproot `open_channel` / `accept_channel` immediately when the
   required type-4 `next_local_nonce` is missing.
2. Make type 22 `next_local_nonces` the spec path for RAA and reestablish,
   then prove reconnect/retransmission regenerates partial signatures from the
   newly received nonces.
3. After the live #81 run is clean, run BTC-only simple-taproot open/pay,
   reestablish, cooperative close, and force-close checks before closing #61.
4. Live-prove cooperative close for simple-taproot channels.
5. Add bounded splice nonce-map coverage or explicitly mark concurrent splicing
   out of the first demo's acceptance criteria.

## Closure Rule

Do not close #81, #61, #71, or #19 from the current pin. The current pin is an
important settlement milestone, but this audit finds simple-taproot spec gaps
that are still observable in the live Lightning Labs path.
