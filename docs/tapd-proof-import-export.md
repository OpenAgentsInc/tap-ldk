# tapd Proof Import/Export

`tap-ldk` can now parse Lightning Labs `TAPF` proof files, validate the
version-0 proof-file envelope, verify each chained checksum, decode each
contained `TAPP` proof as a strict TLV stream with known required proof record
types, and parse the latest Taproot Assets asset leaf.

Wallet import now rejects shallow caller-supplied proof metadata. The latest
`TAPP` asset leaf must derive the same Taproot Assets asset ID from genesis and
must match the local proof record's asset type, amount, owner script key, and
genesis outpoint before wallet state advances. Imported `TAPF` bytes are still
stored byte-for-byte with the wallet proof record, survive restart, and can be
exported as raw proof-file bytes for Lightning Labs `tapcli proofs verify
--proof_file <file>` or equivalent API tooling.

Malformed proof files, unsupported proof-file versions, checksum failures,
malformed supplied anchors/genesis values, wrong asset, wrong owner, wrong
amount, wrong asset type, stale proof digests, and storage digest mismatches
fail before wallet state advances. Production full-history proof replay,
including every Bitcoin anchor transaction, virtual transaction witness,
STXO/split/change path, grouped asset path, and reorg watcher policy remains
future production hardening.

For the first demo and local wallet handoff, the proof courier is now a typed
bundle rather than a loose proof file. The `lnd`/`tapd`/`litd` side can mint or
receive the asset, export a proof file, and hand that file to `tap-ldk`;
`tap-ldk` can preserve the raw TAPF bytes, wrap the accepted native proof and
proof-history metadata into a local proof-courier bundle, and export the raw
proof file back for verification by Lightning Labs tooling. This is local
interop plumbing, not production proof-discovery infrastructure.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-proof-fixture-smoke fixtures/lightning-labs/proof/testdata
cargo run -p tap-ldk-cli -- wallet-import-tapd-proof-file target/tapd-wallet.json fixtures/lightning-labs/proof/testdata/proof-file.hex 941c6b88de2e5c66797831545adabac0b55f8adb836e921c25d2963c65d15bd1 600 0285a7e2dfcad008f54094005db2424aa23431cfb62535950a590957fa6c7cdb27 c181733565d1ddc83fbdc36d7ad630f0b1a497a5f4f4d57a0bf664bb95d59905:0 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1
cargo run -p tap-ldk-cli -- live-tapd-proof-bind target/live-tapd-proof-wallet.json fixtures/lightning-labs/proof/testdata/proof-file.hex 941c6b88de2e5c66797831545adabac0b55f8adb836e921c25d2963c65d15bd1 600 0285a7e2dfcad008f54094005db2424aa23431cfb62535950a590957fa6c7cdb27 c181733565d1ddc83fbdc36d7ad630f0b1a497a5f4f4d57a0bf664bb95d59905:0 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1 target/live-tapd-proof-binding.json
cargo run -p tap-ldk-cli -- wallet-export-proof-bundle target/tapd-wallet.json '<proof-id>' target/proof-bundle.json
cargo run -p tap-ldk-cli -- wallet-import-proof-bundle target/receiver-wallet.json target/proof-bundle.json
cargo run -p tap-ldk-cli -- wallet-export-tapd-proof-file target/tapd-wallet.json '<proof-id>' target/exported.tapf
./scripts/live-tapd-proof-bind.sh target/live-tapd-proof-binding/report.json target/live-tapd-proof-binding/wallet.json
```
