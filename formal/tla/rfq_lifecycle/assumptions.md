# Assumptions

- The model has one quote, one invoice, and one SCID alias.
- The quote and invoice expire at the same bounded time.
- `Alias` stands in for an RFQ SCID alias allocated outside the set of real
  local channel SCIDs.
- Payment success abstracts Lightning routing and final-hop validation.

