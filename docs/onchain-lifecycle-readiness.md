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
testable. The production work after this gate is to drive the same event
vocabulary from actual regtest transactions, chain notifications, sweeper
callbacks, and persisted wallet plus monitor state after crash/restart.

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
- `scripts/path-a-native-demo.sh` writes that report as `onchain-lifecycle.json`
  next to the native close/recovery artifacts.
- The local proof-courier bundle can move accepted wallet proofs with
  proof-history metadata, anchor state, asset fields, optional TAPF bytes, and
  digests.

Remaining before the live chain-watcher phase:

- add the lifecycle smoke to the normal proof-engine verification wrapper;
- finish the lifecycle docs and status notes around what the bounded report
  proves and what it does not prove.

Remaining after this bounded gate:

- live chain-watcher integration for pending, confirmed, stale, and reorged
  close/sweep anchors;
- live post-close proof export and balance observation against integrated
  `litd`;
- live unilateral close and second-level HTLC success/timeout spends;
- live sweeper success/failure callbacks tied to asset proof ownership;
- restart from persisted wallet plus monitor state while a sweep is pending;
- production backup, restore, and partial-recovery refusal policy.
