# Wallet Storage

Date: 2026-05-25

`tap-ldk` wallet storage is currently a bounded regtest JSON store for verified
Taproot Asset proof material and spendable asset UTXOs. It is intentionally
small: balances are computed from verified persisted UTXOs, not from cached
display counters, and the wallet rejects tampered or unsupported storage before
showing balances.

## Commands

```bash
cargo run -p tap-ldk-cli -- wallet-init target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-import-proof-fixture target/demo-wallet.json fixtures/synthetic/proof_anchor_valid.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-proofs target/demo-wallet.json
```

Raw encoded proof TLV files can also be imported and exported:

```bash
cargo run -p tap-ldk-cli -- wallet-import-proof-file target/demo-wallet.json target/proof.tlv
cargo run -p tap-ldk-cli -- wallet-export-proof-file target/demo-wallet.json '<proof-id>' target/proof.tlv
```

## Schema Contract

- `version` is `1`; unsupported versions fail closed.
- `proofs` stores encoded proof TLV bytes as hex, keyed by proof ID.
- `spendable_utxos` stores the spendable asset view derived from those proofs.
- `pending_operations` is reserved for later issuance, transfer, channel, and
  RFQ operation markers.

The first proof ID shape is `<asset-id>:<anchor-outpoint>`. This is a demo
identifier, not a production wallet database key.

## Invariants

- A proof is decoded and verified before it can advance wallet state.
- Duplicate proof import is idempotent and does not double count balance.
- Stored UTXO fields must match the encoded verified proof.
- Restarting the CLI and loading the same wallet file must produce the same
  balance view.
- Private signing keys are not stored in this bounded schema.
