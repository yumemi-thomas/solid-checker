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
and `flush()` cannot re-enter the flush cycle. Solid throws for all three
operation variants in its development build only: the `@solidjs/signals`
rc.17 production bundle (`dist/prod`) carries no such guard, so there the
call does not throw (ADR 0434). Value-form state such as `createSignal(0)` and
`createStore(object)` is allowed because it attaches no child work.

The callback must be a directly written function literal, an exact function
reference, or a closed local callback return. For the last form, the adapter
must have one unconditional return of a function literal or its exact callback
parameter; the returned function is then checked in its own synchronous extent.
Exactly resolved project helpers are followed transitively: a function or
`const` arrow, an import a path or alias resolves inside the program, and a
method declared in `C` of a `const` bound to `new C(...)`. Conditional,
aliased, wrapper-built, package, or otherwise unresolved callbacks produce
`reactive-dispatch-unresolved` instead of being guessed. A nested function
merely created by the callback executes outside the leaf extent.

A standard-library call in that extent is judged by its exact declaration,
optional (`el?.scrollIntoView()`) or not (ADR 0439). One that dispatches an
event synchronously (`focus`, `click`, `dispatchEvent`, dialog and popover
toggles, `AbortController.abort`) keeps the obligation, since a listener it
reaches may call `onCleanup`; so does an optional `getContext`.

`onSettled` becomes a leaf owner only when its call is proven owner-backed.
Out-of-band calls enqueue a plain callback and remain outside this rule; an
unprovable caller or nullable owner is uncertifiable. An `onSettled` called inside
another leaf body is always queued out of band, because that body's owner is
children-forbidden: it is never claimed here, and its callback is judged as
out-of-band (`missing-owner`).

An `onCleanup` in an owned leaf is this rule's claim: the runtime throws
`CLEANUP_IN_FORBIDDEN_SCOPE` there, and it does so wherever the cleanup sits in
the leaf's extent, a helper the leaf calls included. A `createTrackedEffect`
body runs under the tracked node whatever the effect's owner, so its cleanup is
this rule's claim even when the effect itself has no owner. The same site is not
also reported as `missing-owner` (SC4001), which claims only a cleanup that never
runs: one in an `onSettled` callback queued with no owner, or with an owner that
may be absent (ADR 0347).

A site no execution reaches is uncertifiable (ADR 0458). Not yet: a site in a
tracked-effect body whose every creation is inside an owned leaf body, where
the creation itself throws first (`docs/open-work.md`).

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
