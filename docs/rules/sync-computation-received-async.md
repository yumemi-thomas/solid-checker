# sync-computation-received-async

`SC7002` · **error** · violation

A computation is marked `sync: true`, but its callback can return a Promise or
AsyncIterable.

## What it does

Flags the signal-family constructors — `createMemo`, `createSignal(fn)`,
`createOptimistic(fn)`, `createEffect`, and related computation nodes — whose
options include `sync: true` while the computation's callback is async or
statically returns a Promise/AsyncIterable. Options the syntax cannot read
(`createMemo(fn, opts)`, a spread, a non-literal `sync:` value) beside an async
computation are **uncertifiable**: they may set `sync: true`. An async generator's
iterator is an AsyncIterable however it is named, and so is a return type with a
`Symbol.asyncIterator` member (ADR 0865); a sync generator's iterator is a plain value.
A site that provably never runs is silent, such as a function only a
`defer: true` apply with no compute source calls (ADR 0590).

The store-family constructors (`createStore(fn, …)`, `createProjection`,
`createOptimisticStore`) are deliberately not checked: the runtime rebuilds
their node options with only `loadingValue`/`name`, so `options.sync` never
reaches their node — the option is inert there, not dangerous.

## Why is this bad?

`sync: true` is a performance assertion: it tells the runtime the computation
settles synchronously, so the async-shape probe on its result can be skipped.
An async result breaks that assertion. In development the runtime still probes
and throws `SYNC_NODE_RECEIVED_ASYNC`; in production the probe is skipped and
the unawaited Promise is stored as the node's value — readers see a Promise
object instead of the settled value, with no diagnostic.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(async () => fetchUser(id()), { sync: true });
```

Examples of **correct** code for this rule:

```tsx
// Let the async value suspend to a Loading boundary:
const user = createMemo(() => fetchUser(id()));

// Or keep the sync node synchronous and read the settled async value from it:
const user = createMemo(() => fetchUser(id()));
const initials = createMemo(() => initialsOf(user().name), { sync: true });
```

## How to fix

Drop `sync: true` and let the graph suspend to a `<Loading>` boundary, or make the
computation synchronous by moving the async work into its own computation and
reading the settled accessor here.

## Related

- [async-outside-loading-boundary](async-outside-loading-boundary.md) — consuming async computations

## Uncertifiable

A violation needs an async function, or a return type that is a Promise on every path.
"Can return" is not "returns": a return that may be sync, or typed `any`, `unknown` or a
type parameter (Type Facts `returnOpen`), and a value no fact describes are uncertifiable,
blocker `compute-return-unproven` (ADR 0655). Options the syntax cannot read are
uncertifiable beside any computation not proven synchronous.

## Every call ends, and `sync` is read last-wins (ADR 0989)

Each call of a primitive that takes `sync` is a site (`sync-option`) and ends in a row or a typed silence: the options are proven not to assert `sync: true` (`not-asserted`), a duplicate `sync` key TypeScript reports (`tsc-reports`, TS1117), or a computation proven synchronous (`computation-not-async`, never for an open answer). The runtime reads the LAST `sync`, so the rule is a violation only where the exact options literal's last `sync` is `true`: `{ sync: true, ...over }` and `{ sync: true, sync: false }` are not (Chrome rc.21 is quiet); options the syntax cannot read stay an obligation. `createTrackedEffect` takes no `sync` here: its node's compute returns `undefined` (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js:2878-2892`), so the diagnostic is never raised there.
