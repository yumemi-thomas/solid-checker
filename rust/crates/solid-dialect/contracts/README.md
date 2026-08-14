# Vocabulary cross-check contracts

Reactivity summaries of the packages each dialect models, grouped by the
stable dialect id and then package slug:

- `solid-v1/solid-js.json` — `solid-js@1.9.14`
- `solid-v2/solid-js.json` — `solid-js@2.0.0-rc.0`
- `solid-v2/solidjs-web.json` — `@solidjs/web@2.0.0-rc.0`

They exist for one purpose: the crate's tests hold the hand-written dialect
tables to them. `callback_executions` must not contradict a contract's
`callbacks` column, and the generated export index in `src/exports/` is the
same extraction in table form. A name the two disagree about means one of them
was edited alone.

These are **not** the contracts the checker bundles for analysis — those live
with the backend and follow the loader's entrypoint layout. These are the
review artifacts the vocabulary was transcribed from: generated from the
published packages' declarations (see `docs/solid-1x-api-surface.md` for the
1.x extraction rule: no name enters the vocabulary unless the package itself
exports it). When an ambient namespace or type re-export is written in
value-position syntax but the exact package runtime exports no binding, the
generator's reviewed runtime correction keeps it in the type index and out of
the callable contract; Solid 2's renderer-owned `JSX` namespace is the current
example.

The corresponding `rust/dialects/<id>/dialect.json` owns these paths and their
generated `src/exports/` indexes. Regenerate every declared pair with
`make contracts`, or verify them against installed package declarations with
`make contracts-check`. Generator target ids use the same path-shaped identity,
for example `solid-v2/solidjs-web`.

The separate runtime-embedded documents are described in
`pkg/contracts/bundled/README.md`. Do not copy edits between the two sets: a
review contract is a flat vocabulary cross-check, while a bundled contract is
the normalized entrypoint-aware document the backend loads during analysis.
