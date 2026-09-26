# The three rc.9 callback forms in a published contract

`solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` add three callback forms
the rc.3 vocabulary answered wrongly, and a contract generated over a package
that forwards a parameter into one of them published the wrong claim. Each
export here forwards its parameter straight into one form.

| Export | form | claim | before this fixture |
| --- | --- | --- | --- |
| `staticSource` | `dynamic(source, { static: true })` | `inline`, same-stack, untracked — the row `untrackedSource` publishes | `tracked` |
| `untrackedSource` | `untrack(source)` | reference for the row above | same |
| `trackedSource` | `dynamic(source)` | `tracked` (the default form is unchanged) | same |
| `optionedSource` | `dynamic(source, options)` | no row; `callbacks` stays open | `tracked` |
| `hiddenBy` | `omit(props, hidden)` | no row; `callbacks` stays open | `callbacks` **closed**, as if `hidden` were never invoked |
| `withoutA` | `omit(props, "a")` | `callbacks` closed (a key list is a value) | same |
| `whenReady` | `until(predicate)` | no row; `callbacks` open, `creates` open | `creates` **closed**: `until` was not in the vocabulary |

Why each answer, from the published bytes:

- **`dynamic`** opens with `if (options?.static) return
  staticDynamic(untrack(source))` (`@solidjs/web` `dist/web.dev.js:2199`,
  `dist/web.js:2034`, `dist/server.js:3729-3730`). A literal `static: true`
  has run the source, untracked, before `dynamic` returns. An options value
  the syntax does not prove may take either path, so no word is published and
  the unknown-callback sentinel opens.
- **`omit(props, hidden)`** stores `hidden` in the view it returns and calls it
  on every read of that view (`@solidjs/signals` `dist/dev.js:3495-3498`,
  `:4178-4241`) — or, without `Proxy`, once per property during the call
  (`:4408-4427`). Neither is a schedule the contract vocabulary can state.
- **`until(predicate)`** runs the predicate as the first compute of a user
  effect under a fresh root, during the call, and again until it is truthy —
  unless `options.signal` is already aborted, when it never runs
  (`dist/dev.js:2717-2785`, `:2734`). That is `resolve`'s shape, and `resolve`
  gets no word either. `until` creates a root and an effect, so `creates` is
  not closed.

The package is TypeScript so the claims rest on declared callability: every
forwarded parameter is `Callable`, and `withoutA`'s `"a"` is a literal. Both
stubs under `node_modules/` are byte-faithful for the signatures the claims
use and say so in their headers. `tsc --noEmit` is clean on `index.ts` against
the stubs and against the published rc.9 typings. Against rc.3's typings the
`until` import is TS2305, both two-argument `dynamic` calls are TS2554, and the
predicate `omit` is TS2345, so no rc.3-valid package forwards into any of them.
