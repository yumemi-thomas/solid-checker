# The strict window and the current DOM event

Measured in Chrome on rc.21 (ADR 0639). A body's window takes any event; an effect's
records its queue's event, so a handler dispatched from an apply, a timer and a microtask
read in none, and the apply's own read warns, after a dispatch too.
The body's `click()` withholds every handler's read proof in this program; `flush()` in a
handler a helper dispatches stays an obligation.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
