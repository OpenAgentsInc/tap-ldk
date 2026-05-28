# Proof Validation Assumptions

- The model boundary starts after TLV decoding has produced a native
  `ProofFile` and, for Lightning Labs imports, a decoded `TAPF`/`TAPP` asset
  leaf summary.
- Cryptographic hash functions, compressed public-key parsing, and Lightning
  Labs proof-file checksum primitives are trusted library boundaries.
- The bounded first-demo model has one normal regtest asset and one latest
  proof state.
- Production reorg handling, grouped assets, collectibles, reissuance, and
  full STXO/split/change replay are outside this model and remain future
  production work.
