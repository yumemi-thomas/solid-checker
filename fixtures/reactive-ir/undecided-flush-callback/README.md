# undecided-flush-callback

ADR 0542: the undecided-component funnel carved out every callback a primitive places, `flush(fn)`'s too, although it runs its callback where the caller is. Row, handed to `map` as a plain function, keeps its `flush(fn)` read an obligation (the old build: a violation; the read there is tracked by the JSX render computation); Rendered, rendered by a tag only, keeps the violation; Quiet's `untrack` read is silent. A component entered only through an exact dynamic factory keeps the carve-out (ADR 0530, `rc17-dynamic-component`).

The stub reduces the published rc.17 declarations. The complete source also type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.
