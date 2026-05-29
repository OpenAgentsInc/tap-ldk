# Proof Validation Assumptions

- The model boundary starts after TLV decoding has produced a native
  `ProofFile` and, for Lightning Labs imports, a decoded `TAPF`/`TAPP` asset
  leaf summary.
- Cryptographic hash functions, compressed public-key parsing, and Lightning
  Labs proof-file checksum primitives are trusted library boundaries.
- The bounded model has one normal regtest asset, two modeled owners, one
  current output, and a small fixed transition vocabulary.
- `RootFor(amount, owner, anchor)` is a symbolic root relation. It models the
  obligation that accepted roots agree with accepted fields; it does not model
  Taproot hash internals.
- A present STXO means the replay engine has enough prior proof state to spend
  the input. Missing STXO is modeled as a rejection or unresolved history, not
  as a recoverable accepted state.
- Reorg-sensitive history is modeled as a chain-view state change that prevents
  acceptance. The model does not attempt to prove Bitcoin reorg depth policy.
- Grouped assets, collectibles, reissuance policy, multi-asset channels, and
  full network proof transport are outside this model and remain separate
  production work.
