# pending-async-forbidden-scope

`SC5002` · **warning** · violation

An async accessor that may still be pending is read inside `onSettled` or
`createTrackedEffect`, which cannot suspend.

## What it does

Flags reads of accessors returned by async computations when the read happens
inside a leaf owner (`onSettled`, `createTrackedEffect`).

## Why is this bad?

Leaf owners run after the graph settles; they are the end of the flush cycle, not
part of the tracked graph. If the async value happens to still be pending when the
scope runs — a refetch in flight, a slow first load — the read cannot suspend and
throws at runtime. The failure is timing-dependent, which makes it easy to miss in
development and hit in production.

**Sources with a declared first paint still get this finding, with conditional
wording.** A `loadingValue` (or store-family `seedLoadingValue: true`) node is
born committed: a leaf-owner read during the *first flight* serves the declared
value without warning or throwing. The window ends at the first real answer —
probed against `solid-js@2.0.0-rc.0`, a leaf-owner read during a later re-ask
warns `PENDING_ASYNC_FORBIDDEN_SCOPE` and throws exactly like an undeclared
node. The declaration narrows the timing window; it does not remove it.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

onSettled(() => {
  analytics.identify(user().id); // Throws whenever user() is pending.
});
```

Examples of **correct** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

// Settle the value in the compute phase; the apply phase receives it resolved.
createEffect(
  () => user(),
  (resolved) => analytics.identify(resolved.id),
);
```

## How to fix

Settle the value before it reaches the leaf owner: read the accessor in the compute
function of `createEffect(compute, apply)` and pass the resolved value through, or
guard the scope so it only runs once the data is ready.

## Related

- [pending-async-untracked-read](pending-async-untracked-read.md) — untracked pending reads
- [primitive-in-leaf-owner](primitive-in-leaf-owner.md) — other leaf-owner restrictions
