# flush-in-forbidden-scope

`SC3003` · **error** · violation

`flush()` is called inside `createTrackedEffect` or an owner-backed
`onSettled`, which run as part of the flush cycle itself.

## What it does

Flags `flush()` calls that are lexically contained in a `createTrackedEffect`
callback, or in an `onSettled` callback whose call is proven to execute under a
live children-capable owner.

"Lexically contained" means the leaf callback is a function **literal written
directly in the owner's argument**, and the `flush()` sits in that literal's own
synchronous extent. `createTrackedEffect(wrap(() => flush()))` and
`createTrackedEffect(makeCallback())` hand the owner a callback this analysis
cannot see, and a call inside a nested function the callback merely builds runs
later, in that function's scope. The nested function is proven outside the leaf
extent and stays clean. Opaque wrapper/factory callbacks instead produce SC9012
`reactive-dispatch-unresolved`; they are not silently certified.

`onSettled` called out-of-band (from an event handler, with no owner, or inside
another leaf scope) runs its callback as a plain queued function where `flush()`
is a silent no-op rather than a throw, so this rule stays silent there; an
unprovable call site (exported helper) is reported as **uncertifiable**.
`createTrackedEffect` is a leaf owner unconditionally.

The rule also follows the dynamic extent through exactly-resolved helpers: a
project function that calls `flush()` in its own synchronous extent throws
just the same when called from a leaf scope, so that call site is flagged,
naming the helper. Unresolved, ambiguous, or package callees contribute
nothing here; package behavior stays owned by the contract obligation
surface.

## Why is this bad?

Solid 2.0 batches all writes on microtasks; `flush()` drains that queue
synchronously. `onSettled` and `createTrackedEffect` execute *during* the flush
cycle, so calling `flush()` from inside them would re-enter the scheduler. Solid
throws in dev instead of risking re-entrant flushes.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Widget() {
  onSettled(() => {
    setReady(true); // the write itself is fine here — leaf scopes may write
    flush(); // Throws: already inside the flush cycle.
    measure(element);
  });
  return <div />;
}
```

Examples of **correct** code for this rule:

```tsx
// Inside onSettled the graph has already settled — just read.
onSettled(() => {
  measure(element);
});

// If you need to observe a write synchronously, do it at the imperative boundary:
button.onclick = () => {
  setReady(true);
  flush();
  measure(element);
};
```

## How to fix

Inside these scopes the graph has already settled: signal values and the DOM are
current, so the `flush()` is usually unnecessary — delete it. If you need to
observe a write you just made, move both the write and the `flush()` out to the
event handler or imperative boundary that triggered the scope.

## Related

- [cleanup-in-forbidden-scope](cleanup-in-forbidden-scope.md) — other leaf-owner restrictions
