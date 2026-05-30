# Chain-Watcher Lifecycle Readiness

The next production-readiness step is to stop treating close and sweep state as
only a bounded lifecycle fixture. The repo now has a typed lifecycle report for
cooperative close proof export, unilateral recovery, second-level HTLC
recovery, final sweep recovery, refusal events, and restart evidence. The next
layer should record typed observations that explain where those lifecycle
events came from on the chain or from the sweeper.

This does not mean the wallet is production-ready for live force-close. The
first step is a deterministic observation boundary: every observed lifecycle
event should point to a lifecycle event ID, source, anchor state, optional
height, optional outpoint or sweep digest, and deterministic observation
digest. Confirmed recovery needs confirmed chain observation. Failed sweeps,
BTC-only sweeps, stale anchors, and reorged anchors must stay refusal states
until a replacement proof path is observed.

The next issue wave should implement this as a bounded report first, then add
it to the normal proof-engine gate. After that, live regtest code can feed the
same report from real chain notifications, resolver callbacks, sweeper
callbacks, and persisted wallet plus monitor state after restart.
