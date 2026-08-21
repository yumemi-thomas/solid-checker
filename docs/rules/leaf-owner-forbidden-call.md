# leaf-owner-forbidden-call

`SC3001` · **error** · violation · 🛠️ safe fix available for trailing `onCleanup`

A call that requires child ownership or scheduler re-entry occurs inside a
Solid 2.0 leaf owner. The rule covers `onCleanup`, owner-attaching reactive
primitives, and `flush()` inside `createTrackedEffect` or an owner-backed
`onSettled` callback.

## What it does

Leaf owners manage work through their callback's return value and cannot own
children. Consequently, `onCleanup` must be returned, derived primitives such
as `createMemo`, `createRoot`, and function-form `createSignal` cannot attach,
and `flush()` cannot re-enter the flush cycle. Solid throws in development for
all three operation variants. Value-form state such as `createSignal(0)` and
`createStore(object)` is allowed because it attaches no child work.

The callback must be a directly written function literal, an exact function
reference, or a closed local callback return. For the last form, the adapter
must have one unconditional return of a function literal or its exact callback
parameter; the returned function is then checked in its own synchronous extent.
Exactly resolved project helpers are followed transitively. Conditional,
aliased, wrapper-built, package, or otherwise unresolved callbacks produce
`reactive-dispatch-unresolved` instead of being guessed. A nested function
merely created by the callback executes outside the leaf extent.

`onSettled` becomes a leaf owner only when its call is proven owner-backed.
Out-of-band calls enqueue a plain callback and remain outside this rule; an
unprovable caller or nullable owner is uncertifiable.

## Examples

Incorrect:

```tsx
function Widget() {
  onSettled(() => {
    onCleanup(dispose);
    const label = createMemo(() => count());
    flush();
    console.log(label());
  });
  return <div />;
}
```

Correct:

```tsx
function Widget() {
  const label = createMemo(() => count());
  onSettled(() => {
    console.log(label());
    return dispose;
  });
  return <div />;
}
```

## How to fix

Return cleanup from the leaf callback; a trailing `onCleanup(fn)` has a safe
rewrite to `return fn`. Move owner-attaching primitives to the component body
or another children-capable scope and read their accessors inside the leaf.
Remove `flush()` because leaf callbacks already run during settlement, or move
the write and flush together to the event handler or imperative boundary that
triggered the work.

## Related

- [missing-owner](missing-owner.md) — operations with no owner at all
- [primitive-in-directive-application](primitive-in-directive-application.md) — owner-attaching work in a directive application callback
- [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) — callback bodies whose exact execution cannot be certified
