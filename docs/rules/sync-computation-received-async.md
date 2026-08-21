# sync-computation-received-async

`SC7002` · **error** · violation

A computation is marked `sync: true`, but its callback can return a Promise or
AsyncIterable.

## What it does

Flags the signal-family constructors — `createMemo`, `createSignal(fn)`,
`createOptimistic(fn)`, `createEffect`, and related computation nodes — whose
options include `sync: true` while the computation's callback is async or
statically returns a Promise/AsyncIterable.

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
