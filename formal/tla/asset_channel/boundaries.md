# Asset Channel Phase 1 Boundaries

This phase covers:

- feature negotiation;
- receiving proof material;
- accepting valid proof material;
- rejecting invalid proof material;
- opening a durable channel only after negotiation and proof validation;
- preserving the bounded no-inflation property.

This phase does not model:

- full Taproot Assets proof ancestry;
- MS-SMT internals;
- Taproot Assets VM validation;
- Bitcoin confirmation handling;
- MuSig2 signing;
- HTLCs;
- revocation;
- close or force-close;
- persistence crash windows;
- Lightning Labs software interop;
- real network transport;
- external APIs;
- production data.

Those omissions are boundaries, not guarantees. Later models or Rust tests must
cover them before the project treats those surfaces as verified.
