# Boundaries

This model covers quote accept/reject, invoice binding, expiry, single-use
payment, and alias collision behavior. It does not model exchange-rate
precision, BOLT 11 parsing, HTLC custom records, route blinding, or multi-part
payments.

Counterexamples should become Rust tests for quote stores, alias allocation,
expiry handling, or invoice binding.
