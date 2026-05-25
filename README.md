# tap-ldk

This is an experimental effort to explore native Taproot Assets support in Rust Lightning/LDK, with the goal of proving that stablecoin-style assets can be issued, validated, routed, and transacted through an LDK-based wallet without depending on an LND/tapd sidecar. The work here is early research and implementation planning, focused on interoperability, protocol fit, and the engineering needed to make a real native LDK proof of concept possible.

## Development

Run the current setup checks from the repo root:

```bash
cargo fmt --check
cargo test
cargo run -p tap-ldk-cli -- --help
cargo run -p tap-ldk-cli -- regtest-bitcoin-config
```

## Planning Docs

- [Roadmap](ROADMAP.md)
- [Invariants](INVARIANTS.md)
- [Protocol References](docs/protocol-references.md)
- [BLIP-0029 Implementation Note](docs/blip-0029-implementation-note.md)
- [Polar Regtest Topology](docs/polar-regtest-topology.md)
- [Headless Bitcoin Regtest Harness](docs/headless-regtest-harness.md)
- [Lightning Labs Interop Matrix](docs/lightning-labs-interop-matrix.md)
