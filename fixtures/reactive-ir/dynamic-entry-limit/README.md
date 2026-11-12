# dynamic-entry-limit

ADR 0695: a climb that stops at its bound names no entry. Thirty nested `flush(fn)` calls in a dynamic-only body stay the no-window read of ADR 0530; thirty-three are uncertifiable, not a rendering read with a warning Chrome does not give.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
