# tapd Proof Import/Export

`tap-ldk` can now parse Lightning Labs `TAPF` proof files, validate the
version-0 proof-file envelope, verify each chained checksum, and decode each
contained `TAPP` proof as a strict TLV stream with known required proof record
types. Imported `TAPF` bytes are stored byte-for-byte with the wallet proof
record, survive restart, and can be exported as raw proof-file bytes for
Lightning Labs `tapcli proofs verify --proof_file <file>` or equivalent API
tooling.

The current wallet import remains deliberately bounded: the caller supplies the
demo asset ID, genesis outpoint, anchor outpoint, amount, and owner script key,
and `tap-ldk` binds those fields to the verified `TAPF` file digest in its
local proof record. Malformed proof files, unsupported proof-file versions,
checksum failures, malformed supplied anchors/genesis values, and storage
digest mismatches fail before wallet state advances. Full semantic validation
of the asset leaf, virtual transaction, Taproot proof, and full proof ancestry
is still later Track B work.

For the first demo, the local proof/universe courier is mocked infrastructure:
the Lightning Labs side can mint or receive the asset, export a proof file,
and hand that file to `tap-ldk`; `tap-ldk` can then export the same raw proof
file back for verification by Lightning Labs tooling. This is interop plumbing,
not production proof-discovery infrastructure.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-proof-fixture-smoke fixtures/lightning-labs/proof/testdata
cargo run -p tap-ldk-cli -- wallet-import-tapd-proof-file target/tapd-wallet.json fixtures/lightning-labs/proof/testdata/proof-file.hex 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 1000000 02a0afeb165f0ec36880b68e0baabd9ad9c62fd1a69aa998bc30e9a346202e078f 9673b7a0ff70658b94b29c7719af53ba52fe624c330f1db166a221898f343a7d:0 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1
cargo run -p tap-ldk-cli -- wallet-export-tapd-proof-file target/tapd-wallet.json '<proof-id>' target/exported.tapf
```
