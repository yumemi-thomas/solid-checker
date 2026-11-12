# rc21-flush-in-effect-unplaced

ADR 0709: `flush()` in a function no call edge places (object or class method, `Map` slot, a slot's helper, a bundle's `effect`) an apply reaches is an SC2008 obligation, blocker `function-run-unplaced` (Chrome warns). Kept: `flush()` in the apply. Silent: unreached members.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
