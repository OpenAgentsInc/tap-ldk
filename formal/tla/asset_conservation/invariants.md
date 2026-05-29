# Invariants

- Issued supply is never greater than the bounded maximum supply.
- Visible spendable plus channel balance never exceeds issued supply.
- A spent output is not spendable.
- Channel balance requires a spent verified input in the bounded funding model.
- A commitment update cannot change the channel's total asset amount.
- Pending or invalid proof state is not counted as visible balance.
