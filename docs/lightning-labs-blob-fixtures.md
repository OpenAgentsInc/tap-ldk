# Lightning Labs Blob Fixtures

Date: 2026-05-25

This note records the first fixture-backed decoder for Lightning Labs
`tapchannelmsg` blobs. The imported fixtures come from
`lightninglabs/taproot-assets@743db21da57b5fdecf5daca9a925f0261ca94e40`:

- `tapchannelmsg/testdata/funding-blob.hexdump`
- `tapchannelmsg/testdata/htlc-blob.hexdump`
- `tapchannelmsg/testdata/commitment-blob.hexdump`

## Field Map

| Blob | Lightning Labs type | Native map |
| --- | --- | --- |
| Funding | `OpenChannel` | raw digest, decimal display, optional group key, funded asset outputs, output proof digests |
| HTLC | `rfqmsg.Htlc` custom records | raw digest, asset balance list when present, RFQ id, available RFQ ids when present, noop flag when present, visible optional unknown odd records |
| Commitment | `Commitment` | raw digest, local asset outputs, remote asset outputs, outgoing HTLC asset outputs, incoming HTLC asset outputs, aux leaves, optional STXO flag |

The current fixture values decode to one funded asset output of
`100000000000`, then a commitment split of `56700021068` local and
`43299978932` remote for the same asset id. The HTLC fixture carries RFQ id
`cbe41e5c1bbe711d9edf3245c6d8484cc5a339fa3082a400f550ebe846373a3d` and one
odd optional custom record, which is preserved in the decoded summary instead
of being normalized away.

## Boundaries

The decoder is intentionally read-only. Parsing a Lightning Labs blob produces
a native field map and digests, but it does not mutate wallet state, advance a
channel, accept funding, settle an HTLC, or claim proof validity. Those actions
still require the later funding, proof, RFQ, payment, and balance-check issues.

Malformed, truncated, non-canonical, unsupported required, or semantically
wrong fields fail closed in fixture tests. Unknown odd HTLC records are retained
as explicit optional records so interop work can decide whether a later phase
needs to support them.

## Verification

Run:

```bash
cargo test -p tap-ldk-core --test lightning_labs_blob_fixture
cargo run -p tap-ldk-cli -- lightning-labs-blob-fixture-smoke fixtures/lightning-labs/tapchannelmsg/testdata
```
