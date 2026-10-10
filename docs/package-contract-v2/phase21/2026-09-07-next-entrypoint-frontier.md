# Next executable entrypoint frontier

Investigation baseline: `rust/target/ecosystem-investigations/2026-09-07-exact-subject-full.json`,
finished 2026-09-07 19:30:33 JST. Its catalog-bound result is 322 complete,
52 partial, 24 refused and 20 not advanced. This investigation adds no
certification and changes no denominator or accepted catalog. The 768 accepted
artifact selections remain the latest measured set; there is no new full run.

## Floating UI: initialized parameter member input

Three refused rows share demand
`sha256:cc5ec9b62a8a07a6607bb898c9f2b82d35fba747bc0413a5c66430efd55002eb`:
`@corvu-next/popover@0.1.5|solid2|only`,
`@corvu/popover@0.2.0|solid1|only`, and `corvu@0.7.2|solid1|only`.
Their dependency is `@floating-ui/utils@0.2.12`, `./dom`, export
`getOverflowAncestors`. The refusal is recursive-value-shape, complete=true,
presence=Absent, callability=Unknown. Absence is not permission.

Following the Corvu Popover row's retained project, fresh generation with the
matching pinned verify binary succeeds, but produces an **unaccepted** proposal.
Its only operation for this export is `read-0`: input parameter 1, path
`["concat"]`, untracked, same-stack, count 0..many. This is a member input,
not the root parameter input handled by ADR 0056.

Exact generated case:

- Runtime `./dist/floating-ui.utils.dom.mjs`, SHA-256
  `b6658b1eac47e2917e6d10ebbeefb9c4ac9a166f0052bed6e08c871ed848b680`.
- Declarations `./dist/floating-ui.utils.dom.d.mts`, SHA-256
  `5ecea63968444d55f7c3cf677cbec9525db9229953b34f06be0386a24b0fffd2`.
- Resolution `/exports/.~1dom/import/default` and
  `/exports/.~1dom/import/types`.
- Lock integrity
  `sha512-HpCo8tmWzLVad5s2d19EhAz5zqrrQ6s69qd6moPMQvkOuSwDT1YgRfWSVuc4ennqrgv3OHppiOGMQ7oC13yIww==`.

The published implementation assigns `list = []` when `list === void 0`,
then reads `list.concat` in both return branches. The declaration makes `list`
optional. A TypeScript 5.9.3 diagnostic program using the retained published
declarations, strict mode, bundler resolution and skipLibCheck=false produces
zero diagnostics for:

```ts
import { getOverflowAncestors } from "@floating-ui/utils/dom";
function f(list: Parameters<typeof getOverflowAncestors>[1]) {
  if (list === undefined) list = [];
  return list.concat;
}
```

That observation demonstrates TypeScript's branch narrowing; it does not
certify the package runtime or authorize importing a type premise into a
different source context. ADR 0056 explicitly excludes both reassigned
parameters and member paths, so extending it mechanically would be unsound.

The next proof needs exact declaration/slot identity, the undefined guard and
assignment, a control-flow premise binding each relevant read to its reaching
value, and the exact member path. It must distinguish caller-provided arrays
from the fresh local fallback. Reassignment after initialization, aliases,
captured writes, another guard, another fallback and another member must not
inherit permission. The source archive, importer/resolution, producer evidence,
and dependency trust remain consumer-bound. Do not claim that array typing
alone proves member runtime behavior or a closed call domain.

Measure the dependency's next refusal before implementing additional rules.
Three rows is only a first-blocker ceiling, not a forecast of complete rows.

## Declaration blockers excluded from semantic recovery

Read-only TypeScript 5.9.3 diagnostic programs used each latest row's retained
project and real published declarations, strict bundler resolution, and
skipLibCheck=false. No packages or declaration substitutes were installed.

- `@solid-devtools/ui@0.10.3`: importing `SignalContextProvider` exposes TS2307
  in its declarations (and shared graph declarations): cannot find
  `solid-js/types/reactive/signal`. The callable refusal has an unresolved
  declaration dependency; a name-based ContextProvider shortcut is not proof.
- `solid-devtools@0.34.5`: importing `namePlugin` from `solid-devtools/babel`
  exposes TS7016 for `@babel/core`. The retained project lacks
  `@types/babel__core`. Do not treat the resulting open `babel.PluginObj<any>`
  as compiler-proved noncallable. Adding authenticated declaration inputs would
  be a separate resolution-context change, requiring fresh measurement.

These are certification blockers, not new checker diagnostics duplicating
TypeScript's findings.

## Validation and artifacts

Fresh Floating UI proposal generation exited 0. The three diagnostic programs
completed: zero Floating UI diagnostics, three UI TS2307 diagnostics, and one
Babel TS7016 diagnostic. Local diagnostic files are
`/private/tmp/floating-frontier.json` with generated sidecars,
`/private/tmp/next-entrypoint-types.cjs`, and
`/private/tmp/next-entrypoint-types.jsonlog`.

No implementation, receipt format, protocol, fixture snapshot, or published
catalog changed. Full verification and corpus measurement were not repeated
for this documentation-only investigation. The preceding implementation's full
`make verify` remains exit 0, TOTAL 116.12s, with no failure marker; that result
is historical, not a new verification claim. No commit or push was made.
