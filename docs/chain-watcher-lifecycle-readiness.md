# Chain-Watcher Lifecycle Readiness

The next production-readiness step is to stop treating close and sweep state as
only a bounded lifecycle fixture. The repo now has a typed lifecycle report for
cooperative close proof export, unilateral recovery, second-level HTLC
recovery, final sweep recovery, refusal events, and restart evidence. The next
layer should record typed observations that explain where those lifecycle
events came from on the chain or from the sweeper.

This does not mean the wallet is production-ready for live force-close. The
current step is a deterministic observation boundary: every observed lifecycle
event points to a lifecycle event ID, source, anchor state, optional height,
optional outpoint or sweep digest, and deterministic observation digest.
Confirmed recovery needs confirmed chain observation. Failed sweeps, BTC-only
sweeps, stale anchors, and reorged anchors stay refusal states until a
replacement proof path is observed.

The bounded report now exists and is exposed through
`tap-ldk chain-watcher-lifecycle-smoke`. Path A writes the same report as
`chain-watcher-lifecycle.json` next to the close and lifecycle artifacts. The
report validates that every bounded lifecycle event has one matching typed
observation, while still keeping `live_chain_watcher_backed=false` and
`production_ready=false`. The remaining step in this issue wave is adding that
report to the normal proof-engine gate. After that, live regtest code can feed
the same report from real chain notifications, resolver callbacks, sweeper
callbacks, and persisted wallet plus monitor state after restart.

Current implementation status: the core observation model is now present in
`tap_ldk_core::onchain_lifecycle`. It defines observation sources, observation
kinds, deterministic IDs, explicit anchor states, optional height/outpoint or
sweep digest fields, wallet and monitor evidence fields, refusal reasons, and
observation digests. Validation fails closed if a recovered lifecycle event is
not confirmed, if a stale or reorged anchor is counted as recovered, if failed
or BTC-only sweep state is marked recovered, if restart evidence is incomplete,
if the observation digest is tampered, if an observation references an unknown
lifecycle event, or if observation fields do not match the referenced lifecycle
event.
