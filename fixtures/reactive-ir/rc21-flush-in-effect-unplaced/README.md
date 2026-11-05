# rc21-flush-in-effect-unplaced

ADR 0709: `flush()` in a function no call edge places (module object or class method, `Map` slot, a slot's helper, a bundle's `effect`) that an apply reaches is an SC2008 obligation (Chrome: `FLUSH_IN_EFFECT_CALLBACK`). Kept: `flush()` in the apply. Silent: an unreached member.
