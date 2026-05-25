# Assumptions

- The model has one asset channel and one latest durable commitment view.
- Cooperative close and force close both derive from the latest durable asset
  state.
- Sweep success/failure abstracts Bitcoin confirmation, fee, and mempool
  behavior.
- Proof data is represented as current or stale; full proof ancestry is
  covered by parser and proof-validation tests outside this model.
