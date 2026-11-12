# rc21-flush-unplaced-no-action

Claim (ADR 0708): the unplaced arm asks only when the project names a primitive that steps a callback. This stub declares only `flush`, so the members a click handler calls stay silent. Real typings always declare `action`: this pins the gate, not a project shape.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
