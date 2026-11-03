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
