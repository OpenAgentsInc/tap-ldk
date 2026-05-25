# Lightning Labs RFQ Invoice Compatibility

`tap-ldk` now has a bounded Lightning Labs RFQ codec for the selected
`taproot-assets` target at commit
`743db21da57b5fdecf5daca9a925f0261ca94e40`. The codec covers RFQ request,
accept, and reject TLV payloads, derives the Lightning Labs RFQ SCID alias from
the last eight bytes of the RFQ ID, and binds the decoded request to the
existing quote-bound invoice path without changing BOLT 11 invoice text.

```bash
cargo run -p tap-ldk-cli -- lightning-labs-rfq-invoice-compat-smoke 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
```

## Wire Surface

Lightning Labs RFQ peer messages use the taproot-assets base offset
`32768 + 20116 = 52884`:

| Message | Type |
| --- | ---: |
| Request | `52884` |
| Accept | `52885` |
| Reject | `52886` |

The existing native `tap-ldk` peer-message shells still use the asset-channel
offset plus `64..66`; those native message IDs intentionally do not match the
Lightning Labs RFQ IDs. Track B uses `tap_ldk_core::lightning_labs_rfq` for
Lightning Labs interop payloads and the native shells for native-to-native
experiments.

## Checks

- Request payloads enforce version `1`, one BTC side, one asset side, non-zero
  `max_in_asset`, bounded oracle metadata, known transfer types, known
  execution policies, and future expiry.
- Accept payloads enforce version `1`, 64-byte signature field preservation,
  non-zero rates, expiry not outliving the request, and optional fill amount
  not exceeding the request max.
- Reject payloads enforce version `1` and known Lightning Labs reject codes.
- Invoice binding fails closed on wrong peer, wrong asset, wrong BTC amount,
  mismatched invoice context, expired request, or replayed quote.

## Remaining Gap

The smoke preserves the 64-byte Lightning Labs accept signature field but does
not yet verify the live peer signature or drive a real LND/`tapd` RFQ exchange.
That belongs to the next Track B payment issues.
