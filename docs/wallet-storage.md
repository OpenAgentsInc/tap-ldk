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
cargo run -p tap-ldk-cli -- wallet-issue-openusd target/demo-wallet.json 1000000 02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f
cargo run -p tap-ldk-cli -- wallet-import-proof-fixture target/demo-wallet.json fixtures/synthetic/proof_anchor_valid.json
cargo run -p tap-ldk-cli -- wallet-balances target/demo-wallet.json
cargo run -p tap-ldk-cli -- wallet-proofs target/demo-wallet.json
```

Raw encoded proof TLV files can also be imported and exported:

```bash
cargo run -p tap-ldk-cli -- wallet-import-proof-file target/demo-wallet.json target/proof.tlv
cargo run -p tap-ldk-cli -- wallet-export-proof-file target/demo-wallet.json '<proof-id>' target/proof.tlv
cargo run -p tap-ldk-cli -- wallet-verify-proof-file target/proof.tlv
```

For normal local handoff between wallets, prefer a proof-courier bundle. It
keeps the proof bytes, replayed proof-history metadata, anchor state, asset
fields, digests, and optional TAPF proof file bytes together:

```bash
cargo run -p tap-ldk-cli -- wallet-export-proof-bundle target/demo-wallet.json '<proof-id>' target/proof-bundle.json
cargo run -p tap-ldk-cli -- wallet-import-proof-bundle target/receiver-wallet.json target/proof-bundle.json
```

`tapd` proof files can be imported from raw `TAPF` bytes or
hex fixture files and exported back as raw bytes:

```bash
cargo run -p tap-ldk-cli -- wallet-import-tapd-proof-file target/demo-wallet.json fixtures/lightning-labs/proof/testdata/proof-file.hex '<asset-id>' 1000000 '<owner-script-key>' '<genesis-outpoint>' '<anchor-outpoint>'
cargo run -p tap-ldk-cli -- wallet-export-tapd-proof-file target/demo-wallet.json '<proof-id>' target/exported.tapf
```

## Local Regtest Transfer

Before asset channels exist, the bounded local transfer command models a
single on-chain asset split: the sender spends one verified proof, exports a
receiver proof, and stores sender change as a new verified proof. The receiver
then imports the exported proof file.

```bash
cargo run -p tap-ldk-cli -- wallet-init target/alice-wallet.json
cargo run -p tap-ldk-cli -- wallet-init target/bob-wallet.json
cargo run -p tap-ldk-cli -- wallet-issue-openusd target/alice-wallet.json 1000000 02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f
cargo run -p tap-ldk-cli -- wallet-send-local target/alice-wallet.json '<asset-id>' 250000 03a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f target/bob-openusd-proof.tlv
cargo run -p tap-ldk-cli -- wallet-verify-proof-file target/bob-openusd-proof.tlv
cargo run -p tap-ldk-cli -- wallet-import-proof-file target/bob-wallet.json target/bob-openusd-proof.tlv
cargo run -p tap-ldk-cli -- wallet-balances target/alice-wallet.json
cargo run -p tap-ldk-cli -- wallet-balances target/bob-wallet.json
```

The issuance identity, issuer policy, mining/funding mechanics, and proof
courier are mocked for this pre-channel regtest path. Issuance is the only
operation that creates supply; local transfer uses split-conservation checks
and fails if the requested amount exceeds a verified spendable UTXO.

## Schema Contract

- `version` is `1`; unsupported versions fail closed.
- `proofs` stores encoded proof TLV bytes as hex, keyed by proof ID.
- `proofs[*].tapd_raw_proof_file_hex`, when present, stores exact imported
  `tapd` proof-file bytes and must match its stored digest on restart.
- `spendable_utxos` stores the spendable asset view derived from those proofs.
- `pending_operations` is reserved for later issuance, transfer, channel, and
  RFQ operation markers.

The first proof ID shape is `<asset-id>:<anchor-outpoint>`. This is a demo
identifier, not a production wallet database key.

## Invariants

- A proof is decoded and verified before it can advance wallet state.
- Duplicate proof import is idempotent and does not double count balance.
- Stored UTXO fields must match the encoded verified proof.
- Proof-courier export is allowed only for currently accepted, spendable wallet
  proofs. Pending, stale, reorged, obsolete, or unexplained proofs fail closed.
- Restarting the CLI and loading the same wallet file must produce the same
  balance view.
- Private signing keys are not stored in this bounded schema.
