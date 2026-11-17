# returned-by-reference

**Claim (ADR 1062).** A function an effect's compute returns is a returned value (ADR 1020) whether it is returned directly, by
reference (`const f = () => …; return f`, `R1`, `R5`) or as a returned literal's member (`({ run: … })`, `R2`; `{ run: f }`, `R3`):
the apply calls it on its own stack, where creating the sync memo throws PRIMITIVE_IN_EFFECT_CALLBACK first (Chrome rc.21 reports
only that code). Each SC7002 there is `creation-throws-first`, never a violation. A second path to the site does not hide the first:
`R4`'s helper is also run by an exported function, `R5`'s closure is also named by a local nothing calls.

`C1`: a memo's value, a closure returned by reference and called in the component body, does throw SYNC_NODE_RECEIVED_ASYNC in
Chrome; it stays uncertifiable (whether a call of a returned value is on an apply's stack is not decided per call, a known miss).
`C2`: a sync memo created directly in the body is a violation.

Before ADR 1062, `R1`, `R2`, `R3`, `R5` and `C1` were violations. Runtime: oracle case `fixtures/runtime-oracle/returned-by-reference`.
