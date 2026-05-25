# Asset Channel Funding

Date: 2026-05-25

`tap-ldk` now has a bounded native asset-channel funding store for the first
demo path. It verifies Taproot Asset proof inputs, enforces one asset ID per
channel, merges multiple same-asset inputs, derives the funding Taproot Asset
root hash+sum, and persists initial local/remote balances with a monitor blob
before the channel is treated as funded.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- asset-channel-funding-smoke target/asset-channels.json
cargo run -p tap-ldk-cli -- asset-channel-list target/asset-channels.json
cargo run -p tap-ldk-cli -- asset-channel-balances target/asset-channels.json '<channel-id>'
```

## Current Scope

- Single asset ID per channel.
- Same-asset multi-input merge.
- Local and remote initial balance derivation from verified proof amounts.
- Funding root mismatch, wrong asset, incomplete proof, duplicate proof, and
  reused proof attempts fail before durable state advances.
- The stored monitor blob starts at commitment number `0` and must be marked
  persisted before the channel store validates.

## Boundaries

This is the native funding state and persistence boundary. It does not yet
implement asset commitment updates, asset HTLCs, close/recovery, on-chain
sweeps, or rust-lightning channel monitor integration. Those are the next
issues in sequence.
