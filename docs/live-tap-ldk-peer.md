# Live tap-ldk Peer Smoke

Date: 2026-05-26

`tap-ldk` now has a first live peer smoke for the Path B side of the demo. The
smoke starts a localhost `tap-ldk` listener, connects a second peer over TCP,
negotiates the experimental single-asset channel capability through the
OpenAgentsInc rust-lightning fork, and sends an encoded native asset RFQ custom
message across the live socket.

Smoke command:

```bash
cargo run -p tap-ldk-cli -- live-peer-smoke target/live-peer-smoke.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
```

The report records the listener address, client connection, negotiated feature
bits, negotiated asset channel type, custom message type, decoded message kind,
payload digest, and round-trip status.

## Boundary

This is a live `tap-ldk` peer smoke, not a completed Lightning Labs daemon
interop run. It proves the native peer process can run, accept a connection,
use the rust-lightning fork negotiation surface, and move encoded asset custom
messages over a socket. The remaining Path B work is to bind this to real
Lightning wire peer management, the Lightning Labs LND/`tapd` counterparty,
and observed live balance checks.
