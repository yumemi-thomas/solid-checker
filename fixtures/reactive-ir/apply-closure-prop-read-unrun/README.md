# A prop call in an apply closure (ADR 1229)

The closure is handed on, never called in place, so its read is no read of the apply: it keeps its obligation (no read runs on rc.21).

Chrome: `fixtures/runtime-oracle/apply-closure-prop`.
