# rc21-flush-in-effect-unplaced

ADR 0709: `flush()` in a function no call edge places (object or class method, `Map` slot, a slot's helper, a bundle's `effect`) an apply reaches is an SC2008 obligation, blocker `function-run-unplaced` (Chrome warns). Kept: `flush()` in the apply. Silent: unreached members.
