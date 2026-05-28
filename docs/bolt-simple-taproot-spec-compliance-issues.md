# BOLT Simple-Taproot Spec Compliance Issue Plan

Date: 2026-05-28

This document splits the remaining BOLT simple-taproot work out of #81. Issue
#81 should stay focused on the live Lightning Labs to native settlement gate.
Broader BOLT conformance belongs in a separate issue set tracked under #61
before the project claims simple-taproot support is complete.

## Current Fork Line

- `OpenAgentsInc/rust-lightning@0a89b49bf1e822353e0e7c482c5630d5dff22c5c`
- `OpenAgentsInc/ldk-node@17b27661990db823f082a56c026492ccb6f217b0`

The current fork line fixes the completed audit gaps so far: simple-taproot
`funding_created`, `funding_signed`, and `commitment_signed` now serialize the
legacy `signature` field as 64 zero bytes when the MuSig2 TLV is present, and
reject non-zero peer legacy fields.

It also closes the #83 post-claim transcript gap: the live `litd`
zero-HTLC post-claim partial now verifies against the native transcript, with a
fixture asserting sighash
`53c50e50be029ef494407087714245ad42e50c8bf7ae39f9f8589568f705841c`.

It also closes the #84 funding-input force-close witness gap: holder commitment
transactions now persist the aggregate simple-taproot MuSig2 Schnorr signature,
the on-chain fallback uses a one-element key-path witness for the P2TR funding
output, and the latest live Lightning Labs to native run no longer logs
`Invalid Taproot control block size`.

It also closes #85: outbound simple-taproot and Taproot Asset opens clear
`announce_channel`, inbound public opens for those channel types fail closed,
and legacy public BTC channel behavior remains unchanged.

It also closes #86: simple-taproot and Taproot Asset `open_channel` and
`accept_channel` handling now reject missing type-4 `next_local_nonce`
immediately, while legacy channel types can still omit the TLV.

It also closes #87: RAA and `channel_reestablish` now send type-22 nonce maps
as the authoritative path, reject legacy scalar fallback once simple-taproot
funding exists, and regenerate retransmitted commitment partials from fresh
nonce-map material.

It also closes #88: the fork has a BTC-only simple-taproot conformance gate
that opens a simple-taproot channel, verifies P2TR funding, pays across
reconnect/reestablish, covers functional cooperative close, force-closes with a
one-element key-path funding witness, and proves legacy P2WSH channels remain
unaffected. Run it from `tap-ldk` with
`./scripts/check-btc-simple-taproot-conformance.sh`.

It also closes #89: native simple-taproot cooperative close now asserts a
single 64-byte Taproot key-path funding witness, the Taproot Asset
asset-channel smoke confirms the latest close allocation survives restart, and
`tap-ldk` exposes both `./scripts/check-simple-taproot-cooperative-close.sh`
and a live `litd` close command. The live Lightning Labs post-close proof and
balance observer is still a documented Path B boundary, not a claimed success.

## Issue Map

| Issue | Role | Scope | Blocks |
| --- | --- | --- | --- |
| #82 | Master tracker | Track all BOLT simple-taproot spec-compliance work split out of #81 | #61, #71, #19 |
| #83 | Done | Fixture and fix the live post-claim zero-HTLC commitment transcript mismatch | Closed after live verification |
| #84 | Done | Fix the live simple-taproot force-close control-block/witness path | Closed after fixture and live verification |
| #85 | Done | Enforce the no-public-simple-taproot-channel rule | Closed after fork pin and docs update |
| #86 | Done | Fail `open_channel` / `accept_channel` immediately on missing simple-taproot nonces | Closed after fork pin and docs update |
| #87 | Done | Make type-22 nonce maps authoritative and prove reconnect retransmission | Closed after fork pin and docs update |
| #88 | Done | Add a BTC-only simple-taproot end-to-end conformance gate | Closed after fork pin and docs update |
| #89 | Done | Prove native/fixture cooperative close and document the live Lightning Labs close boundary | Closed after fork pin and docs update |
| #90 | BOLT compliance | Cover splice nonce maps or explicitly gate splicing out of the first demo | #61, #71 |

## Before #61 Can Close

These are required for BOLT simple-taproot completion but do not need to be
stuffed into #81:

1. Either add bounded splice nonce-map coverage or explicitly mark concurrent
   splicing out of the first demo's acceptance criteria.

## Closure Policy

- Do not close #81 because a generic BOLT item was fixed. Close #81 only when
  the live Path B settlement gate is clean and its report no longer depends on
  stale force-close/control-block blockers.
- Do not close #61 until the master spec-compliance tracker is closed.
- Do not close #71 or #19 until the simple-taproot base is clean enough for the
  Taproot Asset overlay claims made by the demo.
