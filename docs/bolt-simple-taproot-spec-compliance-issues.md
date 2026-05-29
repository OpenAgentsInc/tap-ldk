# BOLT Simple-Taproot Spec Compliance Issue Plan

Date: 2026-05-28

This document records the BOLT simple-taproot work that was split out of #81.
#81 is complete and should stay a live `litd` to native settlement
regression gate.
Broader production BOLT conformance after the first-demo claim belongs in a
new focused issue set before the project claims production-complete
simple-taproot support.

## Current Fork Line

- `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`
- `OpenAgentsInc/ldk-node@0964b3d0cce5753a0ff42166ea4686702faf93b4`

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
output, and the latest live `litd` to native run no longer logs
`Invalid Taproot control block size`.

It also closes #85: outbound simple-taproot and Taproot Asset opens clear
`announce_channel`, inbound public opens for those channel types fail closed,
and legacy public BTC channel behavior remains unchanged.

It also closes #86: simple-taproot and Taproot Asset `open_channel` and
`accept_channel` handling now reject missing type-4 `next_local_nonce`
immediately, while legacy channel types can still omit the TLV.

It also closes #87 for first-demo interop: RAA and `channel_reestablish` use
the Lightning Labs staging scalar nonce for single-funding overlay channels,
keep type-22 nonce maps for final or multi-funding paths, and reject scalar
fallback when more than one funding txid is active.

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
and a live `litd` close command. The live `litd` post-close proof and
balance observer is still a documented Path B boundary, not a claimed success.

It also closes #90 for the first-demo scope: concurrent simple-taproot splicing
is explicitly excluded from the first public demo by
`tap_ldk_core::demo_scope::first_demo_protocol_scope` and
`tap-ldk-cli first-demo-scope`. Production splice claims still require bounded
tests for every active splice/funding txid's type-22 nonce-map entry.

## Issue Map

| Issue | Role | Scope | Blocks |
| --- | --- | --- | --- |
| #82 | Done | Track all BOLT simple-taproot spec-compliance work split out of #81 | Closed for first-demo scope |
| #83 | Done | Fixture and fix the live post-claim zero-HTLC commitment transcript mismatch | Closed after live verification |
| #84 | Done | Fix the live simple-taproot force-close control-block/witness path | Closed after fixture and live verification |
| #85 | Done | Enforce the no-public-simple-taproot-channel rule | Closed after fork pin and docs update |
| #86 | Done | Fail `open_channel` / `accept_channel` immediately on missing simple-taproot nonces | Closed after fork pin and docs update |
| #87 | Done | Select correct RAA/reestablish nonce fields for staging and final paths | Closed after fork pin and docs update |
| #88 | Done | Add a BTC-only simple-taproot end-to-end conformance gate | Closed after fork pin and docs update |
| #89 | Done | Prove native/fixture cooperative close and document the live Lightning Labs close boundary | Closed after fork pin and docs update |
| #90 | Done | Cover splice nonce maps or explicitly gate splicing out of the first demo | Closed with machine-readable first-demo splice exclusion |

## #61 Closure Result

The first-demo simple-taproot claim is closed by #61. The closure depends on
these facts:

1. Keep the concurrent-splicing exclusion visible in `README.md`,
   `ROADMAP.md`, `ARCHITECTURE.md`, this plan, and the BOLT implementation
   audit.
2. `./scripts/check-btc-simple-taproot-conformance.sh`,
   `./scripts/check-simple-taproot-cooperative-close.sh`, and
   `./scripts/check-simple-taproot-splice-policy.sh` pass against
   `OpenAgentsInc/rust-lightning@8a54739ac030ba3e439496eacb7e1c1216e11c6f`.

These are still required before any production/simple-taproot-complete claim:

1. Replace the first-demo splice exclusion with bounded splice nonce-map tests
   for missing, stale, duplicate, and wrong-funding-txid entries across every
   active current or splice funding candidate.

## Closure Policy

- Do not reopen or broaden #81 because a generic BOLT item remains. Keep #81's
  live Path B settlement gate clean and track broader production BOLT work in a
  focused child issue.
- #82 is closeable for first-demo scope because all child issues #83 through
  #90 are closed and the splice exclusion is explicit.
- #61 is closed for first-demo scope. Do not describe it as
  production-complete simple-taproot support until splice nonce-map vectors
  replace the first-demo exclusion.
- Keep #19 closed only while the simple-taproot base is clean enough for the
  Taproot Asset overlay claims made by the demo.
