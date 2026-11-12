# Deferred effect apply: callees

A function called only from code that never runs never runs (ADR 0590): a
`defer: true` apply whose compute has no source never runs, so what a wrapper
does when that apply calls it (directly, through an IIFE, a helper's callback,
a chain or recursion) is not reported: not `primitive-in-effect-callback`,
`sync-computation-received-async` (a sync async memo) or
`flush-in-effect-callback`. A compute reading a retained
writable source proves the later run (violation in the wrapper), one live caller
keeps the wrapper reported, and an unknown `defer` value leaves it
uncertifiable. Package stubs as in `primitive-in-effect-callback` (reduced rc.17
typings); every case also checks against published rc.17.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
