# Invariants

- Recovered balance requires current proof data.
- Exported proof data must correspond to the recovered output.
- Failed sweeps do not produce recovered balances.
- Refused recovery is not reported as recovered.
- The latest durable commitment number is preserved through close/recovery
  transitions.
