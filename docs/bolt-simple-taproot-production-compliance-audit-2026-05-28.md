# BOLT Simple-Taproot Production Compliance Audit

Date: 2026-05-28

Source reviewed:

- `https://raw.githubusercontent.com/lightning/bolts/refs/heads/master/bolt-simple-taproot.md`
- #81 final comment: `https://github.com/OpenAgentsInc/tap-ldk/issues/81#issuecomment-4568568448`

Current local line audited:

- `tap-ldk@current main after #94/#95 docs/pin update`
- `OpenAgentsInc/rust-lightning@3db3229733b724f45e7a356d923715213cb4f269`
- `OpenAgentsInc/ldk-node@1e439b10c94a6e42442d245f95945a906dd6221e`

## Audit Result

#81 is correctly closed as a live `litd` to native LDK settlement regression.
It is separate from the production-complete BTC simple-taproot BOLT claim.
The remaining BOLT base work was tracked and closed in:

- #94: full BOLT vector and unilateral spend-path replay;
- #95: production compliance tracker.

#91, #92, and #93 are now implemented. The final `option_simple_taproot` bits are a
separate mode from staging/overlay interop, require `option_channel_type` and
`option_simple_close`, keep opens private, and use type-22 nonce maps for final
RAA/reestablish. The splice nonce-map work now covers current, pending splice,
and RBF funding txids with fail-closed tests for malformed, missing, duplicate,
unknown, scalar-with-multiple-funding, and nonce-reuse cases.
Cooperative-close RBF now rotates closee and closer nonces across later close
proposals, persists sent and received close state across reload, retains signed
close transactions until confirmation, and fails closed on missing or reused
close state.

#94 closes the remaining BTC simple-taproot BOLT base gap in the pinned fork
line. The remaining project gaps are Taproot Assets overlay hardening and live
interop boundaries, not missing simple-taproot BOLT base surfaces.

## Implemented Or Demo-Covered

These areas do not need new BOLT-base issues unless regressions are found:

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
- BTC-level simple-taproot splice nonce maps for current, pending splice, and
  RBF funding txids;
- staging scalar nonce interop for the current single-funding Lightning Labs
  overlay path;
- exact BOLT no-HTLC, five-HTLC, and trimmed-HTLC commitment transaction
  vectors;
- exact BOLT HTLC resolution transaction hex, complete witness stacks, and
  deterministic remote HTLC signatures;
- consensus-verified to-local, to-remote, anchor, HTLC, and second-level
  unilateral spend paths;
- restart/serialization reconstruction for tap tweaks, script roots, leaf
  scripts, and control blocks needed by spend paths;
- live first-demo `litd` to native LDK asset payment settlement.

## BOLT Base Production Status

| Area | Spec expectation | Current state | Required issue |
| --- | --- | --- | --- |
| Final feature bits | Final `option_simple_taproot` depends on `option_channel_type` and `option_simple_close`, and should use explicit channel type negotiation. | Implemented under `negotiate_final_simple_taproot_channels`, with staging/overlay kept separate. | Done in #91 |
| Public-channel behavior | Simple-taproot channel type must not become a public default channel type. | Implemented for staging, Taproot Asset overlay, and final mode. | Done in #91 |
| RAA nonce maps | Final simple-taproot `revoke_and_ack` must carry type-22 entries for every active funding txid. | Implemented for current, pending splice, and RBF funding txids. Staging single-funding scalar interop remains intentionally supported. | Done in #92 |
| Reestablish nonce maps | Final simple-taproot `channel_reestablish` must carry fresh type-22 entries for every active funding txid and regenerate partials from the new peer map. | Implemented with serialized channel-state coverage for the pending splice and counterparty nonce map. | Done in #92 |
| Splice coordination | Every active current or splice funding txid needs a distinct nonce map entry. Multiple pending splices need distinct txid/nonce pairs and no reuse. | Implemented for BTC-level BOLT simple-taproot nonce maps. Asset-channel splice/RBF remains separate hardening. | Done in #92 |
| Cooperative close RBF | Later close proposals must use the latest peer `next_closee_nonce`, fresh closer nonces, and fail closed on stale/missing/mismatched state. | Implemented in #93 with opener-as-closer and accepter-as-closer RBF tests, persisted close state, reload coverage, signed close retention until confirmation, and fail-closed checks for missing/reused close nonce or partial-signature state. | Done in #93 |
| Full vectors | The BOLT appendix covers scripts, output keys, commitment transactions, HTLC resolution transactions, witnesses, trimming, and deterministic BIP340 HTLC signatures. | Implemented in #94: exact no-HTLC, five-HTLC, trimmed-HTLC, HTLC resolution, witness, and deterministic remote-signature vectors are replayed in the fork. | Done in #94 |
| Persistence for spends | Tap tweaks, script roots, leaf scripts, control blocks, and nonce state needed for unilateral spends must survive restart. | Implemented in #94: spend metadata reconstructs after commitment serialization and unilateral output spends are consensus-verified. | Done in #94 |

## Draft Conflict Note

The current `bolt-simple-taproot.md` draft has an internal accepted-HTLC
key-order conflict: the JSON script fields disagree with the prose and the
executable HTLC resolution transaction vectors. The implementation follows the
prose and transaction vectors for final BOLT mode. Lightning Labs staging
behavior remains explicit and separate.

## Implementation Homes

- Protocol library work belongs in the OpenAgentsInc `rust-lightning` fork.
- Runtime surfacing belongs in the OpenAgentsInc `ldk-node` fork only when the
  library work needs node-level configuration or live runtime access.
- `tap-ldk` should pin the completed fork revisions, expose verification
  scripts, and keep README/roadmap/audit docs honest.

## Closure Rule

#95 can close because #94 is implemented, `tap-ldk` pins the completed fork
revision, and the docs now separate the BTC simple-taproot BOLT base from
Taproot Assets overlay hardening.
