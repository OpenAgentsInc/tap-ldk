# Invariants

- The Lightning Labs node is an independent counterparty, not a sidecar.
- Fixture-backed Lightning Labs blob decoding is read-only in the model; a
  decoded funding, HTLC, or commitment blob cannot by itself advance proof,
  channel, RFQ, payment, or balance state.
- Fixture-backed Track B funding state can be persisted only when decoded
  Lightning Labs funding and commitment blobs agree on asset ID, total amount,
  and local/remote allocation; without live funding-outpoint proof binding the
  model remains in a documented-gap state.
- Fixture-backed Lightning Labs `TAPF` proof-file import validates the
  transport envelope before proof sync can become available in the model, and
  raw proof-file export preserves the imported bytes.
- Unsupported required blob fields and malformed blob structure lead to a
  documented gap or rejection state, never a successful interop state.
- A settled interop payment requires proof sync, an open compatible channel,
  accepted RFQ state, and matching balances.
- A compatibility gap is not reported as success.
- The native wallet remains the wallet authority in the model.
