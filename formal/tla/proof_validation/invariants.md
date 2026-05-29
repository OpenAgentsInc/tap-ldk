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
- A replayed proof-history output cannot be accepted unless it descends from
  valid issuance through valid transition records.
- Accepted proof-history state must keep asset ID, amount, owner script key,
  anchor, and root coherent across issuance, split, transfer, channel funding,
  commitment update, close, and sweep.
- Wrong genesis, wrong anchor, wrong owner script key, wrong asset type, wrong
  amount, wrong root hash, wrong root sum, mismatched TapCommitment output
  root, invalid split sum, missing STXO, malformed proof-file transport, stale
  proof, and reorg-sensitive history cannot become accepted wallet or channel
  balances.
- Counterexamples must become Rust regression tests or explicit production
  boundary notes; the model must not weaken runtime validation policy.
