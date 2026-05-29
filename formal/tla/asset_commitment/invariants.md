# Invariants

- Commitment updates preserve total asset balance.
- Commitment numbers are monotonic in the bounded model.
- The latest commitment cannot be marked revoked.
- Asset nonces are single-use.
- Accepted restart state is durable only when the proof replay state and
  persisted proof commitment match the latest commitment number.
- A restart with newer Lightning commitment state but stale proof state is
  refused rather than treated as recovered.
- BTC-domain signatures cannot advance asset state.
