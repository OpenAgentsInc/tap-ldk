# BOLT Simple-Taproot Spec Compliance Issue Plan

Date: 2026-05-28

This document splits the remaining BOLT simple-taproot work out of #81. Issue
#81 should stay focused on the live Lightning Labs to native settlement gate
and broadcast-clean force-close fallback. Broader BOLT conformance belongs in a separate issue set
tracked under #61 before the project claims simple-taproot support is complete.

## Current Fork Line

- `OpenAgentsInc/rust-lightning@90212e54066a35ad982b338e7c2c152bf4fe0b0b`
- `OpenAgentsInc/ldk-node@3264d96ee6dcbd37cec24473eac5982b1678a560`

The current fork line fixes one audit gap: simple-taproot `funding_created`,
`funding_signed`, and `commitment_signed` now serialize the legacy `signature`
field as 64 zero bytes when the MuSig2 TLV is present, and reject non-zero peer
legacy fields.

It also closes the #83 post-claim transcript gap: the live `litd`
zero-HTLC post-claim partial now verifies against the native transcript, with a
fixture asserting sighash
`53c50e50be029ef494407087714245ad42e50c8bf7ae39f9f8589568f705841c`.

## Issue Map

| Issue | Role | Scope | Blocks |
| --- | --- | --- | --- |
| #82 | Master tracker | Track all BOLT simple-taproot spec-compliance work split out of #81 | #61, #71, #19 |
| #83 | Done | Fixture and fix the live post-claim zero-HTLC commitment transcript mismatch | Closed after live verification |
| #84 | #81 blocker | Fix the live simple-taproot force-close control-block/witness path | #81 |
| #85 | BOLT compliance | Enforce the no-public-simple-taproot-channel rule | #61 |
| #86 | BOLT compliance | Fail `open_channel` / `accept_channel` immediately on missing simple-taproot nonces | #61 |
| #87 | BOLT compliance | Make type-22 nonce maps authoritative and prove reconnect retransmission | #61 |
| #88 | BOLT compliance | Add a BTC-only simple-taproot end-to-end conformance gate | #61 |
| #89 | BOLT compliance | Live-prove cooperative close for simple-taproot channels | #61, #71 |
| #90 | BOLT compliance | Cover splice nonce maps or explicitly gate splicing out of the first demo | #61, #71 |

## Before #81 Can Close

This is the only BOLT audit item that should remain in #81's critical path:

1. Fix the local unilateral force-close witness/control-block path so the
   fallback transaction is broadcast-clean.

## Before #61 Can Close

These are required for BOLT simple-taproot completion but do not need to be
stuffed into #81:

1. Enforce that simple-taproot channels are private: clear or reject
   `announce_channel` when selecting the simple-taproot channel type.
2. Fail `open_channel` and `accept_channel` immediately when a simple-taproot
   channel omits the type-4 `next_local_nonce`.
3. Make type-22 `next_local_nonces` the spec path for RAA and
   `channel_reestablish`, then prove retransmitted commitments regenerate
   partial signatures from newly received nonce maps.
4. Add a BTC-only simple-taproot conformance gate covering open, payment,
   reconnect/reestablish, cooperative close, and force-close.
5. Prove cooperative close in the live/simple-taproot path before using it as a
   demo claim.
6. Either add bounded splice nonce-map coverage or explicitly mark concurrent
   splicing out of the first demo's acceptance criteria.

## Closure Policy

- Do not close #81 because a generic BOLT item was fixed. Close #81 only when
  the live Path B settlement and fallback are clean.
- Do not close #61 until the master spec-compliance tracker is closed.
- Do not close #71 or #19 until the simple-taproot base is clean enough for the
  Taproot Asset overlay claims made by the demo.
