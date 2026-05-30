# On-Chain Lifecycle Readiness

The next production-hardening surface is the asset owner lifecycle after a
channel stops being an ordinary live payment channel. The current project can
prove the bounded native path through cooperative close, proof export, and
proof-ownership recovery records for unilateral commitment, second-level HTLC,
and final sweep cases. It now has a typed lifecycle report that says which
on-chain lifecycle events were explained, which were refused, which are only
bounded simulations, and which remain live-regtest or chain-watcher work.

The immediate goal is a bounded on-chain lifecycle gate. That gate should bind
cooperative close proof export, unilateral close proof ownership, second-level
HTLC success and timeout ownership, sweep success, sweep failure, restart
recovery, wallet digest, monitor digest, proof-history output, and proof handoff
digest into typed records. A report should fail closed when a sweep failure is
reported as recovered, when a recovery record has no proof-history output, when
restart recovery lacks wallet or monitor evidence, or when a BTC-only sweep is
counted as Taproot Asset recovery.

This is still not the live chain watcher. The bounded gate makes the current
proof-ownership and close/recovery evidence explicit, deterministic, and
testable, and the normal proof-engine wrapper now runs it. The production work
after this gate is to drive the same event vocabulary from actual regtest
transactions, chain notifications, sweeper callbacks, and persisted wallet plus
monitor state after crash/restart.

Current completed pieces:

- Cooperative close builds local and remote final proofs, imports them into
  wallets, exports them again, and verifies the close proof-history export
  records.
- Native recovery builds proof-ownership records for unilateral commitment,
  second-level HTLC, and final sweep spend kinds.
- Recovery refuses missing proof ownership, stale proof ownership, and BTC-only
  sweep state pretending to be asset recovery.
- `tap-ldk onchain-lifecycle-smoke` emits one typed report with cooperative
  close, unilateral commitment, second-level HTLC success/timeout, final sweep,
  refusal, and restart evidence.
- `tap-ldk chain-watcher-lifecycle-smoke` emits the bounded chain/sweeper
  observation report for those lifecycle events while explicitly refusing live
  chain-watcher or production-readiness claims.
- `scripts/path-a-native-demo.sh` writes that report as `onchain-lifecycle.json`
  next to the native close/recovery artifacts and now writes
  `chain-watcher-lifecycle.json` beside it.
- `scripts/onchain-lifecycle-smoke.sh` validates the lifecycle report and
  writes the checked artifact under `target/onchain-lifecycle-smoke/`.
- `scripts/proof-engine-check.sh` runs the lifecycle smoke as part of the
  normal proof-engine gate.
- The local proof-courier bundle can move accepted wallet proofs with
  proof-history metadata, anchor state, asset fields, optional TAPF bytes, and
  digests.

Remaining after this bounded gate:

- normal proof-engine wrapper coverage for the bounded chain/sweeper
  observation report;
- live chain-watcher integration for pending, confirmed, stale, and reorged
  close/sweep anchors;
- live post-close proof export and balance observation against integrated
  `litd`;
- live unilateral close and second-level HTLC success/timeout spends;
- live sweeper success/failure callbacks tied to asset proof ownership;
- restart from persisted wallet plus monitor state while a sweep is pending;
- production backup, restore, and partial-recovery refusal policy.
