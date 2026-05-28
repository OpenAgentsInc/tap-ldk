# BOLT Simple Taproot Implementation Audit

Date: 2026-05-28

Spec source:
https://raw.githubusercontent.com/lightning/bolts/refs/heads/master/bolt-simple-taproot.md

Implementation audited:

- `OpenAgentsInc/rust-lightning@0d587fbe4259145dd576fd5255ac9acc4b06a0f4`
- `OpenAgentsInc/ldk-node@38f53969c90f0f3178d0617a212d77b7ea2316f1`
- `tap-ldk` pinned to those forks

## Summary

The fork implements a large part of the draft BOLT simple-taproot base: feature
and channel-type bits, fixed-width MuSig2 TLVs, nonce parsing, MuSig2 signing
helpers, BIP86 funding outputs, simple-taproot commitment output construction,
HTLC script helpers, second-level HTLC signing, reestablish/RAA nonce maps, and
cooperative-close message types.

It does not fully implement the current spec yet. The most important gaps for
#81 are:

- native simple-taproot `funding_created`, `funding_signed`, and
  `commitment_signed` still carry non-zero legacy ECDSA signature fields from
  existing LDK signing paths; the spec requires the legacy field to be a
  64-byte zero array;
- native receivers ignore the legacy signature field for simple-taproot
  commitments instead of failing if it is non-zero;
- the latest live #81 run rejects `litd`'s zero-HTLC post-claim commitment with
  `Invalid simple-taproot commitment partial signature`, so the implemented
  post-claim transaction, nonce, aggregate key, or Taproot Asset tapscript-root
  transcript still differs from the peer's transcript;
- local unilateral fallback then fails bitcoind policy/consensus checks with
  `Invalid Taproot control block size`, so the force-close witness path is not
  broadcast-clean;
- cooperative close exists behind `simple_close` plus
  `simple_taproot_musig2`, but it is not yet live-proven for the demo channel;
- splice nonce maps have some multi-funding plumbing, but concurrent splice
  behavior has not been proven against the draft's full active-funding map
  requirements.

The audit result is therefore: **not spec complete**. The next #81 work should
be a spec-aligned fix pass, not another isolated Taproot Assets overlay patch.

## Why This Matters For #81

The latest live owner-state run reached real settlement before failing:

- `litd` completed asset issuance and asset-channel funding;
- `litd` sent the asset keysend and reported `SUCCEEDED`;
- native LDK recorded `PaymentClaimable` and `PaymentClaimed`;
- fork-backed `ldk-node` recorded native receiver balance `125`;
- `native_ldk_invalid_commitment_logged=false`;
- `native_ldk_invalid_simple_taproot_partial_sig_logged=true`;
- `native_ldk_invalid_taproot_control_block_logged=true`.

The failing `ldk_node.log` transcript shows a BOLT simple-taproot failure, not
only a Taproot Assets failure:

- native LDK rejects `litd`'s post-claim `commitment_signed` because the MuSig2
  partial signature does not verify over native's zero-HTLC commitment
  transcript;
- the local force-close commitment transaction cannot be broadcast because the
  Taproot script-path control block is malformed for the spend being attempted.

The BOLT requires `commitment_signed` partial signatures to verify against the
exact commitment transaction and requires Taproot control blocks to reconstruct
the committed script path. The current #81 failure directly maps to those
requirements.

## Audit Matrix

| Spec area | Spec requirement | Current implementation | Status | Required work |
| --- | --- | --- | --- | --- |
| Feature bits | Define final `option_simple_taproot` bits 80/81 and staging bits 180/181; use explicit channel type. | `lightning-types/src/features.rs` defines final and staging bits. `ChannelHandshakeConfig::negotiate_simple_taproot_channels` advertises staging only. | Partial | Keep staging for `litd` interop, but document final-bit dependency on `option_simple_close` before enabling final bits. |
| Public-channel prohibition | A simple-taproot opener must not set `announce_channel`. | `get_initial_channel_type` can select simple-taproot based on config and peer features; `get_open_channel` still copies `announce_for_forwarding` into `channel_flags`. | Gap | Reject or clear public announcement when selecting simple-taproot or Taproot Asset channel types. Add a regression. |
| TLV wire types | Fixed TLV payloads: type 2 partial signature with nonce, type 4 next local nonce, type 6 partial signature, type 8 shutdown nonce, type 22 nonce map. | `lightning/src/ln/simple_taproot.rs` defines the fixed constants and validates 66-byte public nonces as two compressed secp points. `msgs.rs` round-trips these TLVs. | Implemented | Keep vector tests pinned to the upstream BOLT fixture payloads. |
| `open_channel` / `accept_channel` nonces | Messages must include type 4 `next_local_nonce`; receivers fail on absent or invalid nonces. | Open/accept generation derives counter nonces. Missing peer nonces are stored as `None` and later cause signing/validation failure; invalid points fail parse. | Partial | Fail immediately during open/accept validation for simple-taproot channels when the nonce is absent. |
| Funding partials | `funding_created` and `funding_signed` legacy `signature` field must be 64 zero bytes; type 2 MuSig2 partial must be present and valid. | Type 2 MuSig2 partials are generated and validated. Existing LDK ECDSA signature fields are still populated in native messages. | Gap | Serialize zero legacy signatures for simple-taproot funding messages and reject non-zero peer legacy signatures. |
| `channel_ready` nonce | Message must include a fresh type 4 nonce. | `check_get_channel_ready` sends a nonce and `channel_ready` handling requires it for simple-taproot funding. | Implemented | Add a missing-nonce functional regression if not already covered by message tests. |
| `commitment_signed` partial | Legacy `signature` field must be zero; type 2 partial must verify; HTLC signatures must be BIP340 Schnorr in the existing HTLC field. | Type 2 partial validation and BIP340 HTLC verification exist. Native outgoing messages still carry non-zero legacy ECDSA signatures, and incoming non-zero legacy signatures are ignored for simple-taproot. | Gap | Zero outgoing legacy fields and fail on non-zero incoming legacy fields. Add tests for funding and commitment messages. |
| `revoke_and_ack` nonce map | Type 22 `next_local_nonces` must include one entry for each active funding txid. | `simple_taproot_next_local_nonces` builds a map across current and pending funding; receipt validates expected txids. A legacy scalar type 4 compatibility path is still accepted for a single funding txid. | Partial | Make type 22 the authoritative path for spec mode; keep scalar compatibility only under an explicit `litd` compatibility note or remove it after interop is stable. |
| `channel_reestablish` nonce map | Type 22 must be sent and checked for every active commitment; retransmitted commits must regenerate partials with new nonces. | Reestablish sends the nonce map and stores received maps. Sent commitment signatures are persisted by funding txid and nonce index. | Partial | Add a reconnect test that forces retransmission and proves the partial is regenerated against the newly received nonce map. |
| Splice coordination | Every active splice/funding txid needs a distinct nonce entry. | Expected txid calculation includes current funding and pending funding. No live or vector proof of concurrent splice maps exists. | Partial | Add bounded splice nonce-map tests or mark splicing out of the first demo's acceptance criteria. |
| BIP86 funding output | Funding output must be P2TR over MuSig2 KeyAgg(KeySort(funding keys)). | `SimpleTaprootKeyAggContext` builds BIP86 funding scripts and has BOLT vector replay. | Implemented | Keep vector coverage. |
| To-local output | NUMS internal key, delay/revocation leaves, correct parity-bearing control blocks, delay sequence. | `simple_taproot_to_local_spend_info` builds delay and revocation leaves and test coverage checks control-block lengths. Taproot Asset aux leaves alter tree depth. | Partial | Fix the live invalid control-block path and add a broadcastable force-close regression for the exact #81 commitment. |
| To-remote output | The draft prose is internally inconsistent here, but the vectors use the global simple-taproot NUMS point, a single CSV-1 script leaf, and sequence 1 spend. | `simple_taproot_to_remote_spend_info` builds the script and uses the global BOLT NUMS point, matching the vectors. | Implemented for base | Confirm Taproot Asset aux-leaf depth/control-block behavior in live force-close tests. |
| Anchor outputs | Anchor internal key is local delayed key or remote payment key; script is `16 CSV`; omit anchor if corresponding output absent and no HTLCs. | `chan_utils.rs` emits simple-taproot anchors under the BOLT conditions. | Implemented | Add/keep regression for no-output/no-HTLC anchor omission. |
| HTLC outputs | Offered/accepted HTLCs are P2TR with revocation internal key and split timeout/success leaves. | `simple_taproot_htlc_spend_info_with_aux_leaf_for_variant` implements final and staging variants and can include Taproot Asset aux leaves. | Partial | The base is covered, but live asset aux-leaf transcript must remain fixture-backed for both directions. |
| Second-level HTLCs | Version 2, sequence 1, zero-fee semantics, SIGHASH_SINGLE|ANYONECANPAY, one delayed output. | `build_htlc_transaction`, `simple_taproot_htlc_sighash_type`, and package/signing code use sequence 1 and `SinglePlusAnyoneCanPay` for simple-taproot/Taproot Asset HTLCs. | Implemented for current path | Keep previous-output-bound Taproot Asset aux-leaf regressions. |
| Cooperative close | `shutdown`, `closing_complete`, and `closing_sig` carry MuSig2 nonces/partials; aggregate final Schnorr signature; rotate closee nonces for RBF. | Message structs and channel logic exist behind `simple_close` plus `simple_taproot_musig2`; shutdown nonce persistence exists. | Partial | Run native and `litd` cooperative close coverage before closing #61 or #71. |
| Formal/spec vectors | BOLT vectors should cover TLVs, scripts, commitments, HTLCs, signatures, and trimming. | Vector replay exists for implemented base surfaces. Live #81 shows missing transcript coverage for post-claim zero-HTLC state and force-close witnesses. | Partial | Import the failing live transcript as a non-secret regression fixture and add exact BOLT transcript assertions. |

## Required #81 Work From This Audit

1. Add a focused rust-lightning regression for simple-taproot legacy signature
   zeroing and non-zero legacy signature rejection in `funding_created`,
   `funding_signed`, and `commitment_signed`.
2. Convert the latest live post-claim zero-HTLC transcript into a fixture test:
   funding txid, nonce index, local nonce, counterparty nonce, partial
   signature, sighash, tapscript root, transaction hex, and channel balances.
3. Use that fixture to determine whether native is deriving the wrong
   transaction, wrong Taproot Asset tapscript root, wrong aggregate nonce, or
   wrong aggregate key for `litd`'s post-claim `commitment_signed`.
4. Fix the force-close witness builder so the local commitment transaction
   spends the funding output with a valid Taproot control block or with the
   correct simple-taproot key-path signature where the BOLT expects key-path
   spending.
5. Make type 22 `next_local_nonces` the spec path for RAA and reestablish,
   then prove reconnect/retransmission regenerates partial signatures from the
   newly received nonces.
6. After the live #81 run is clean, run BTC-only simple-taproot open/pay,
   reestablish, cooperative close, and force-close checks before closing #61.

## Closure Rule

Do not close #81, #61, #71, or #19 from the current pin. The current pin is an
important settlement milestone, but this audit finds simple-taproot spec gaps
that are still observable in the live Lightning Labs path.
