# BOLT Simple-Taproot Spec Compliance Issue Plan

Date: 2026-05-28

This document records the BOLT simple-taproot work that was split out of #81.
#81 is complete and should stay a live `litd` to native settlement
regression gate.
Broader production BOLT conformance after the first-demo claim belongs in a
new focused issue set before the project claims production-complete
simple-taproot support.

## Current Fork Line

- `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269`
- `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`

Production-complete BOLT simple-taproot work after the first-demo closure is
now closed by #94 and #95 and audited in
`docs/bolt-simple-taproot-production-compliance-audit-2026-05-28.md`.

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

It also closes #90 for the first-demo scope: before #92, concurrent
simple-taproot splicing was kept out of the first public demo by
`tap_ldk_core::demo_scope::first_demo_protocol_scope` and
`tap-ldk-cli first-demo-scope`.

It also closes #91 and #92 for production hardening: final `option_simple_taproot`
negotiation is implemented separately from the staging/overlay path, and
BTC-level splice nonce maps now cover current, pending splice, and RBF funding
txids with fail-closed tests for malformed or reused nonce state. The first
Taproot Assets demo still does not claim asset-channel splice/RBF behavior.

It also closes #93 for production hardening: cooperative-close RBF now keeps
the signed close channel state alive until confirmation, rotates closee nonces
after each `closing_sig`, uses fresh closer nonces for later proposals, persists
sent and received close state across reload, and fails closed on missing or
reused shutdown, close partial, and close nonce state.

It also closes #94 for production hardening: the fork replays exact no-HTLC,
five-HTLC, trimmed-HTLC, and HTLC-resolution BOLT vectors, including complete
witness stacks and deterministic remote HTLC signatures. It consensus-verifies
to-local, to-remote, anchor, HTLC, and second-level unilateral spends and
checks restart-safe reconstruction of tap tweaks, script roots, leaf scripts,
and control blocks.

It also closes #95: the production tracker now points at the pinned fork line
that contains #91 through #94. Remaining project work is Taproot Assets overlay
hardening, not the BTC simple-taproot BOLT base.

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
| #90 | Done | Cover splice nonce maps or explicitly gate splicing out of the first demo | Closed the historical first-demo boundary; #92 now supersedes the BTC-level gap |
| #91 | Done | Enable final `option_simple_taproot` production negotiation | Implemented in the OpenAgentsInc forks and exposed through `tap-ldk-cli simple-taproot-negotiation-report` |
| #92 | Done | Implement full simple-taproot splice nonce-map compliance | BTC-level production splice nonce-map support |
| #93 | Done | Complete simple-taproot cooperative-close RBF nonce rotation | Production close RBF support |
| #94 | Done | Replay full simple-taproot BOLT vectors and unilateral spend paths | Production vector/spend proof |
| #95 | Done | Track production BOLT simple-taproot compliance | Production tracker closed after #94 |

## #61 Closure Result

The first-demo simple-taproot claim is closed by #61. The closure depends on
these facts:

1. Keep the asset-channel splice/RBF boundary visible in `README.md`,
   `ROADMAP.md`, `ARCHITECTURE.md`, this plan, and the BOLT implementation
   audit.
2. `./scripts/check-btc-simple-taproot-conformance.sh`,
   `./scripts/check-simple-taproot-cooperative-close.sh`, and
   `./scripts/check-simple-taproot-splice-policy.sh` passed against the
   first-demo fork pin used for #61 closure.

The production/simple-taproot-complete BTC base claim is now pinned in
`tap-ldk`. Taproot Assets overlay splice/RBF and live close/recovery hardening
remain separate.

## Closure Policy

- Do not reopen or broaden #81 because a generic BOLT item remains. Keep #81's
  live Path B settlement gate clean and track broader production BOLT work in a
  focused child issue.
- #82 is closed for first-demo scope because all child issues #83 through #90
  are closed, and #91/#92 now add final negotiation plus BTC-level splice
  nonce-map support.
- #61 is closed for first-demo scope; #94/#95 now close the later production
  BTC simple-taproot BOLT-base tracker.
- Keep #19 closed only while the simple-taproot base is clean enough for the
  Taproot Asset overlay claims made by the demo.
- #95 can close because #94 is complete and `tap-ldk` pins the fork revision
  that contains that work.
