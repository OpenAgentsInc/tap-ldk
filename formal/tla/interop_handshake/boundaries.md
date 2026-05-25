# Boundaries

This model covers the Track B interop handshake shape: proof sync, compatible
channel setup, RFQ negotiation, payment settlement, balance comparison, and
documented compatibility gaps. Blob decoding is abstracted as either
`DecodedReadOnly`, `RejectedMalformed`, or `RejectedUnsupported`; the TLA model
does not prove byte-level TLV parsing. It does not model Lightning Labs daemon
internals, macaroon/TLS auth, Bitcoin consensus, Docker/Polar lifecycle, or
full protocol serialization.

Counterexamples should become Rust or harness tests for interop state
classification, balance comparison, proof compatibility, or gap reporting.
