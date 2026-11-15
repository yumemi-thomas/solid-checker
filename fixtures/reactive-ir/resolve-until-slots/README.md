# resolve and until: every call ends

ADR 0985 (batch 11, slice 7). SC2004 (`resolve`) and SC2005 (`until`) mirror the dev guard `if (getObserver()) throw`. The site universe lists every call the dialect
answers as `Primitive::Resolve` or `Primitive::Until` (whatever the callee is spelled), and each ends in a row (a tracked scope: a violation; an unplaced one: an
obligation), in a typed silence that names why no observer is active where it runs (`static_api.resolve_scope.*`, each with the span it rests on), or in the
uncertifiable row `resolve-scope-unaccounted`.

Each component is one cell, `...R` for `resolve` and `...U` for `until`:

- violations: `latest(() => resolve(..))` in a memo (Chrome rc.21 throws), a const alias of the import, the namespace member;
- an obligation: an async callback of `isPending` (the rejection is not what `isPending` catches);
- silences: `isPending` with a synchronous callback (it takes what the callback throws), `untrack`, `runWithOwner(getOwner(), ..)` and `createRoot` (they clear the
  observer), a timer in a component body, code past an `await` every path reaches, an action step resumed by a `yield` of a promise, an action only handlers invoke
  (called in a handler, or handed to one as a value);
- no site: a local function named `resolve` or `until`.

`SOLID_CHECKER_IR_ABLATE_SILENCE=all` drops the silences: each silent cell then carries `resolve-scope-unaccounted` (an obligation, never a violation); the
`site_accounting_process` test pins that.
