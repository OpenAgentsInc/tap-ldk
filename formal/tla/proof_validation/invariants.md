# Proof Validation Invariants

- A proof with a shallow or unsupported scope cannot advance wallet or channel
  state.
- A proof for the wrong network, wrong asset type, wrong asset ID, wrong owner,
  wrong amount, malformed outpoint, stale anchor, or mismatched root cannot
  advance wallet or channel state.
- A Lightning Labs proof file cannot advance wallet state unless the decoded
  latest asset leaf agrees with the native proof record's asset ID, asset type,
  amount, owner script key, and genesis outpoint.
- Recovery handoff cannot claim proof recovery unless the latest committed
  proof root and commitment number match the expected asset-channel state.
- Counterexamples must become Rust regression tests or explicit #71 boundary
  notes; the model must not weaken runtime validation policy.
