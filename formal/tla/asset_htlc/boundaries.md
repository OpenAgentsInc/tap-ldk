# Boundaries

This model covers quote-gated asset HTLC add, settle, fail, revocation, and
asset-balance conservation. It does not model onion routing, full Lightning
commitment construction, preimage revelation, CLTV/CSV timing, or multi-part
payments.

Counterexamples should become Rust tests for asset HTLC custom-record
validation, quote binding, amount conservation, revocation handling, or
persistence ordering.
