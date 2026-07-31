# Vocabulary cross-check contracts

Reactivity summaries of the packages each dialect models, one file per
package:

- `solid-js.json` — `solid-js@2.0.0-beta.19`
- `solid-js-1x.json` — `solid-js@1.9.14`
- `solidjs-web.json` — `@solidjs/web@2.0.0-beta.19`

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
exports it).
