# BOLT Simple-Taproot Production Compliance Audit

Date: 2026-05-28

Source reviewed:

- `https://raw.githubusercontent.com/lightning/bolts/refs/heads/master/bolt-simple-taproot.md`
- #81 final comment: `https://github.com/OpenAgentsInc/tap-ldk/issues/81#issuecomment-4568568448`

Current local line audited:

- `tap-ldk@current main after #91 docs/pin update`
- `OpenAgentsInc/rust-lightning@cac9764f5926b081034b88e4fa1c13cc691335c1`
- `OpenAgentsInc/ldk-node@81e141cf58125fff60771fe023b363ff2b591860`

## Audit Result

#81 is correctly closed as a live `litd` to native LDK settlement regression.
It is not a production-complete BOLT simple-taproot claim. The remaining work
is now tracked in:

- #92: full splice nonce-map compliance;
- #93: cooperative-close RBF nonce rotation;
- #94: full BOLT vector and unilateral spend-path replay;
- #95: production compliance tracker.

#91 is now implemented. The final `option_simple_taproot` bits are a separate
mode from staging/overlay interop, require `option_channel_type` and
`option_simple_close`, keep opens private, and use type-22 nonce maps for final
RAA/reestablish.

The first-demo path remains valid. The gap is between that bounded demo and a
100% simple-taproot BOLT implementation.

## Implemented Or Demo-Covered

These areas do not need new issues from this audit unless regressions are
found while implementing #92 through #94:

- fixed-width TLV parsing and serialization for simple-taproot nonces and
  partial signatures;
- type-4 `next_local_nonce` enforcement in `open_channel`, `accept_channel`,
  and `channel_ready`;
- zero legacy signature fields when MuSig2 partial-signature TLVs are present;
- fail-closed non-zero legacy signature parsing;
- private-only simple-taproot and Taproot Asset channel opens;
- BIP86 P2TR funding output derivation from sorted aggregate funding keys;
- MuSig2 key aggregation, partial verification, final aggregation, nonce-use
  state, and duplicate-use rejection;
- BTC-only simple-taproot open, payment, reconnect, cooperative close, and
  force-close smoke coverage;
- staging scalar nonce interop for the current single-funding Lightning Labs
  overlay path;
- live first-demo `litd` to native LDK asset payment settlement.

## Remaining Production Gaps

| Area | Spec expectation | Current state | Required issue |
| --- | --- | --- | --- |
| Final feature bits | Final `option_simple_taproot` depends on `option_channel_type` and `option_simple_close`, and should use explicit channel type negotiation. | Implemented under `negotiate_final_simple_taproot_channels`, with staging/overlay kept separate. | Done in #91 |
| Public-channel behavior | Simple-taproot channel type must not become a public default channel type. | Implemented for staging, Taproot Asset overlay, and final mode. | Done in #91 |
| RAA nonce maps | Final simple-taproot `revoke_and_ack` must carry type-22 entries for every active funding txid. | Map machinery exists. Staging single-funding scalar interop remains intentionally supported. Multi-funding/final paths need production splice coverage. | #92 |
| Reestablish nonce maps | Final simple-taproot `channel_reestablish` must carry fresh type-22 entries for every active funding txid and regenerate partials from the new peer map. | Map machinery and selected retransmission coverage exist. Production splice/restart coverage is not complete. | #92 |
| Splice coordination | Every active current or splice funding txid needs a distinct nonce map entry. Multiple pending splices need distinct txid/nonce pairs and no reuse. | Concurrent simple-taproot splicing is explicitly excluded from the first public demo. | #92 |
| Cooperative close RBF | Later close proposals must use the latest peer `next_closee_nonce`, fresh closer nonces, and fail closed on stale/missing/mismatched state. | A native one-shot cooperative-close happy path is covered. Multiple RBF rounds and restart-safe nonce rotation are not fully proven. | #93 |
| Full vectors | The BOLT appendix covers scripts, output keys, commitment transactions, HTLC resolution transactions, witnesses, trimming, and deterministic BIP340 HTLC signatures. | The fork has selected vector and smoke coverage. Exact full-corpus replay and every unilateral output spend path are not yet production-complete. | #94 |
| Persistence for spends | Tap tweaks, script roots, leaf scripts, control blocks, and nonce state needed for unilateral spends must survive restart. | Spend-info structures exist and selected monitor paths are covered. Complete output-class spend and persistence tests remain. | #94 |

## Implementation Homes

- Protocol library work belongs in the OpenAgentsInc `rust-lightning` fork.
- Runtime surfacing belongs in the OpenAgentsInc `ldk-node` fork only when the
  library work needs node-level configuration or live runtime access.
- `tap-ldk` should pin the completed fork revisions, expose verification
  scripts, and keep README/roadmap/audit docs honest.

## Closure Rule

#95 can close only after #92 through #94 close, `tap-ldk` pins the completed
fork revisions, and the docs no longer describe any first-demo exclusion as
part of a production-complete BOLT simple-taproot claim.
