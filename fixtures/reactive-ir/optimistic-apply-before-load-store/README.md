# SC2009: a user effect writes an optimistic store before load (ADR 1175)

The store form of `optimistic-apply-before-load`: the compute `async () => ({ v: 1 })` answers a closed object literal of
primitive literals, which runs no code, so ADR 0505's mount flush holds as for a primitive literal. A violation.
