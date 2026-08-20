# v1/missing-owner

`SC4001` · **warning** · violation (uncertifiable, reported as an error, when ownership or runtime entry is unproven)

An effect, `onCleanup` registration, or `Suspense`/`ErrorBoundary` is created
without a Solid 1.x reactive owner, so its lifetime cannot be disposed.

## What it does

The rule reports owner-requiring operations that no component, computation, or
root dominates. Module scope, bare helpers called outside a component tree, and
detached callbacks are common sources. An exported helper with unseen callers
is uncertifiable because a downstream caller might provide an owner. An
uppercase function name is not ownership proof: JSX use or the exact Solid
`Component` type must establish component execution.

Calls covered by `"use server"` are also uncertifiable when the client entry
would allocate. The directive is a framework convention; Solid 1.x's client
entry allocates while its server entry may be a no-op.

## Examples

Incorrect:

```tsx
createEffect(() => applyTheme(theme()));
onCleanup(() => window.removeEventListener("resize", resize));
const orphan = <Suspense fallback={<Spinner />}><Profile /></Suspense>;
```

Correct:

```tsx
function App() {
  createEffect(() => applyTheme(theme()));
  onCleanup(() => window.removeEventListener("resize", resize));
  return <Suspense fallback={<Spinner />}><Profile /></Suspense>;
}

const dispose = createRoot((dispose) => {
  createEffect(() => applyTheme(theme()));
  return dispose;
});
```

## How to fix

Create effects, cleanup registrations, and boundaries inside a component or
computation. For standalone setup, use `createRoot`, retain the dispose handle,
and call it when the setup ends; Solid 1.x roots are detached, so nothing
disposes one automatically. For exported helpers, record the ownership
precondition in the package's reactivity contract.

## Related

- [v1/reactive-write-in-owned-scope](./reactive-write-in-owned-scope.md) — writes whose owner context changes their semantics
- [v1/package-contract-incomplete](./package-contract-incomplete.md) — missing external ownership facts
