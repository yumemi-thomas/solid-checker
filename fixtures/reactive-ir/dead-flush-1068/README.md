# dead-flush-1068

**Claim (ADR 1068).** FLUSH_IN_EFFECT_CALLBACK (SC2008) was exempt from the site-reach rule ("a no-op wherever it runs", ADR 0458); under ADR 1018 a flush that provably never runs is uncertifiable:

- `AfterReturn`: after a `return` (`unreached-dead-code`).
- `TryReturn`: after a `try` that always returns, its `finally` completing normally (new: `try` in the facts' dead-code reading).
- `ThrowingCompute`, `NamespaceThrowingCompute`: in the apply of an effect whose compute starts with a `throw` (new: never entered, `function-unentered`).
- `EmptyLiteral`: in a callback of `[].forEach` (ADR 1019, `function-unentered`).

Violations kept: `Plain`, `CatchCompletes` (the `catch` completes, so the flush runs), `ComputeMayReturn` (the compute may return).

Chrome: `fixtures/runtime-oracle/flush-in-effect-callback` (the same five shapes, their ledger rows removed). Stubs: `rc21-flush-in-effect-unplaced`'s.
