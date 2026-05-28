# Live Asset Payment Session

Date: 2026-05-26

`tap-ldk` now has a live localhost asset-payment session smoke. It starts two
native peers, negotiates the experimental single-asset channel capability, then
sends the ordered peer messages needed by the outgoing payment path:

- input proof chunks;
- output proof chunks;
- funding created;
- funding accepted;
- RFQ request;
- RFQ accept;
- asset HTLC blob.

Run it from the repo root:

```bash
cargo run -p tap-ldk-cli -- live-asset-payment-session-smoke target/live-asset-payment-session.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93 125
```

The report records the listener address, negotiated feature bits, negotiated
channel type, each message type, each decoded kind, payload lengths, payload
digests, ack status, reassembled proof lengths, and the session payment id.

## Boundary

This is not issue #57 completion by itself. It proves the native ordered
asset-payment wire session can run over a live socket without a `tapd` sidecar.
Issue #57 is now completed by the separate integrated-`litd` live gate in
`scripts/live-lightning-labs-outgoing-payment.sh`, which drives asset-channel
funding/payment and records the returned `litd` channel asset balance.
