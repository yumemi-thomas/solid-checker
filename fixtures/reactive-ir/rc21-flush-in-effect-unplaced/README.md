# rc21-flush-in-effect-unplaced

Claim (ADR 0709): `flush()` in a function no call edge places (module-level object or class method, `Map` slot, a helper a slot calls, a bundle's `effect`) that an apply reaches drains nothing (Chrome, rc.21: `FLUSH_IN_EFFECT_CALLBACK`, witnesses SC2008 b05, d07): an SC2008 obligation, blocker `function-run-unplaced`. Kept: `flush()` in the apply is a violation. Silent: a member nothing reaches. No `action` is declared.
