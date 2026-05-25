# Asset Channel Counterexamples

Counterexamples committed to this directory must be synthetic or redacted. Do
not store real wallet seeds, keys, preimages, proofs, macaroons, certs, bearer
tokens, local absolute paths, private repo contents, raw shell logs, or real
payment/customer/issuer data.

Use this format for every meaningful trace:

```md
# CE-0001: short title

- counterexample_id:
- model:
- invariant_violated:
- minimal_trace:
- production_contract:
- rust_test_name:
- expected_behavior:
- implementation_gap:
- fix_ref:
- status:
- boundary_exception_rationale:
```

A counterexample is resolved only when a Rust regression test fails before the
fix and passes after the fix, or when the trace is documented as outside the
model boundary with rationale.

## Current Status

No counterexamples are currently committed for the Phase 1 asset-channel
model.
