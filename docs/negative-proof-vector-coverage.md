# Negative Proof Vector Coverage

Issue #99 makes the negative proof cases explicit. The fixture at
`fixtures/synthetic/proof_negative_vectors.json` is not a production proof
corpus. It is a bounded checklist that ties each invalid proof class to a
state-advance boundary and a regression target.

The current runtime policy is fail-closed. Bad proof material can be parsed for
diagnostics, but it must not move into wallet balances, funded channel state,
commitment state, close records, or recovery records.

| Vector | Boundary | Coverage |
| --- | --- | --- |
| Wrong genesis | Proof validation context and TAPF import | `proof::semantic_context_rejects_wrong_fields_and_stale_anchor` rejects mismatched expected genesis before acceptance; TAPF import also checks latest leaf genesis. |
| Wrong anchor | Proof validation context and wallet import | `proof::semantic_context_rejects_wrong_fields_and_stale_anchor` rejects mismatched expected anchor; wallet tests reject malformed anchor import without balance state. |
| Stale proof | Proof validation, close, recovery | Proof context rejects stale anchors; close smoke rejects obsolete close proof; recovery tests reject stale checkpoints and stale proof ownership. |
| Malformed TAPF proof-file transport | Lightning Labs TAPF wallet import | Wallet TAPF test corrupts the proof-file checksum and asserts no proof or spendable UTXO is recorded. |
| Invalid split sums | Proof replay and TAP VM split validation | Proof replay rejects non-conserved split totals; TAP VM fixtures and asset identity tests reject split-sum errors. |
| Wrong owner script key | Proof validation, close proof handoff, TAPF import | Proof context rejects wrong owner; close validation checks owner-specific proof handoff; TAPF ancestry compares latest leaf script key. |
| Missing STXO | Proof replay and channel funding | Proof replay rejects missing inputs before accepted explanations; funding rejects empty or spent funding proofs before channel state advances. |
| Wrong asset type | Proof validation and wallet import | Proof context rejects non-normal first-demo asset type before balance import. |
| Wrong amount | Proof validation and wallet storage validation | Proof context rejects expected amount mismatch; wallet validation rejects tampered UTXO amount against verified proof. |
| Wrong root hash | Proof import and proof replay | Proof validation rejects commitment-root hash mismatch; proof replay rejects mismatched output root. |
| Wrong root sum | Proof import and proof replay | Proof validation rejects root sum mismatch before wallet import; proof replay also checks output root sum. |
| Mismatched TapCommitment output root | Asset-channel funding | Funding rejects wrong expected funding root and LDK output commitment mismatch before channel state advances. |
| Reorg-sensitive history | Proof replay and recovery | Proof replay rejects stale/reorg-sensitive output state; recovery rejects stale checkpoints and proof ownership. |
| BTC-only sweep as asset recovery | On-chain lifecycle and recovery | Recovery refuses BTC-only sweep state as Taproot Asset recovery; the next lifecycle gate must keep that refusal visible. |
| Failed sweep reported recovered | On-chain lifecycle and close | Close and recovery gates must keep failed sweeps distinct from recovered asset proofs. |

The formal model at `formal/tla/proof_validation/ProofValidation.tla` has a
matching invalid-transition vocabulary for these classes. It proves only the
bounded policy that these invalid paths cannot end in accepted balances. It
does not prove Bitcoin inclusion, Taproot hashing, MuSig2, or database crash
consistency.
