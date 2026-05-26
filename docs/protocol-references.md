# Protocol References

Date: 2026-05-25

This manifest records the source material used for early `tap-ldk` planning.
The local paths assume the broader workspace is checked out at
`/Users/christopherdavid/work`; the GitHub repo should not vendor these
reference repos.

| Area | Upstream | Local workspace path | Local commit |
| --- | --- | --- | --- |
| BLIP-TAP PR #29 capture | https://github.com/lightning/blips/pull/29 | `../stablecoins/blip-tap-pr-29.md` | workspace doc |
| Stablecoins transcript | local transcript source | `../stablecoins/stablecoins-may25-transcript.md` | workspace doc |
| Tap-LDK analysis | local planning source | `../stablecoins/tap-ldk-proof-of-concept-analysis.md` | workspace doc |
| BOLT simple taproot channels | https://github.com/lightning/bolts/blob/master/bolt-simple-taproot.md | upstream BOLTs draft | reviewed 2026-05-26 |
| TAP BIPs draft source | https://github.com/Roasbeef/bips | `../projects/lightninglabs/repos/bips` | `bd3cdc153bea` |
| BLIPs repo | https://github.com/lightning/blips | `../projects/lightninglabs/repos/blips` | `27c0ea5be60b` |
| Taproot Assets reference implementation | https://github.com/lightninglabs/taproot-assets | `../projects/lightninglabs/repos/taproot-assets` | `743db21da57b` |
| LND reference implementation | https://github.com/lightningnetwork/lnd | `../projects/lightninglabs/repos/lnd` | `9f03672bdaba` |
| Rust Lightning / LDK | https://github.com/lightningdevkit/rust-lightning | `../projects/ldk/repos/rust-lightning` | `0c37f08a55c0` |
| OpenAgentsInc rust-lightning fork | https://github.com/OpenAgentsInc/rust-lightning | owned fork | `6af69ad385b8` |
| LDK Node crate | https://github.com/lightningdevkit/ldk-node | crates.io `ldk-node` | `0.7.0` |
| Polar regtest reference | https://github.com/jamaljsr/polar | `../projects/repos/polar` | `ee3ae493d613` |

## Reference Rules

- Use `projects/` and `stablecoins/` as source material only.
- Do not vendor large chunks of reference code into `tap-ldk`.
- If a fork is required, create it under `OpenAgentsInc` and wire it
  explicitly from this repo.
- Keep local reference commits fresh enough to explain interop behavior before
  making a compatibility claim.
- Record any BLIP/TAP draft mismatch in docs before encoding it as runtime
  behavior.

## First Fixture Targets

Import or reference fixture material in this order:

1. BOLT simple taproot feature, channel-type, wire TLV, MuSig2, funding,
   commitment, close, HTLC, and reestablish vectors.
2. TAP TLV encoding and strict-decoding vectors.
3. MS-SMT hash+sum and split-commitment vectors.
4. `AssetCommitment` and `TapCommitment` vectors.
5. Proof file and anchor proof vectors, including semantic proof ancestry.
6. Address and virtual PSBT vectors.
7. LND/`tapd` asset-channel funding and payment traces.
8. RFQ request/accept/reject and SCID alias examples.

Fixture tests should fail because implementation is missing, not because the
source of truth is ambiguous.
