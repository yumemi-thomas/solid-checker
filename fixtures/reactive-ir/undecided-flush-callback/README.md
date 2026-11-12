# undecided-flush-callback

ADR 0542: the undecided-component funnel carved out every callback a primitive places, `flush(fn)`'s too, although it runs its callback where the caller is. Row, handed to `map` as a plain function, keeps its `flush(fn)` read an obligation (the old build: a violation; the read there is tracked by the JSX render computation); Rendered, rendered by a tag only, keeps the violation; Quiet's `untrack` read is silent. A component entered only through an exact dynamic factory keeps the carve-out (ADR 0530, `rc17-dynamic-component`).

The stub reduces the published rc.17 declarations. The complete source also type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
