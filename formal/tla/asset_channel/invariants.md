# Asset Channel Phase 1 Invariants

Initial invariants for the first asset-channel model:

- An asset channel never reaches `open` unless both peers negotiated support.
- An asset channel never reaches `open` unless proof material is valid.
- Durable asset-channel state is only recorded for an open channel.
- The modeled channel balance never exceeds the verified maximum asset amount.

When TLC reports a counterexample, reduce it to the smallest meaningful trace
and record it in `counterexamples/` before translating it into a Rust
regression test or a documented model-boundary exception.

## Phase 1 Model

`AssetChannel.tla` models one local peer, one remote peer, symbolic proof
states, and a fixed asset amount. It intentionally keeps the state space small
so the model remains easy to understand and cheap to run.

Minimum path coverage:

- the channel can negotiate support;
- proof material can be received;
- valid proof material can allow opening;
- invalid proof material can reject opening;
- terminal states stutter so TLC does not treat intended terminal states as
  deadlocks.

Local status on 2026-05-25: TLC 2.19 ran through `scripts/formal-check.sh` on
this machine and reported no errors for `AssetChannel.cfg`. The checked state
graph generated 8 states, found 6 distinct states, and reached depth 5.
