# cleanup-in-forbidden-scope

`SC3001` · **error** · violation · 🛠️ safe fix available

`onCleanup` is called inside a leaf owner (`createTrackedEffect`, or an
owner-backed `onSettled`), which manages cleanup through its return value
instead.

## What it does

Flags `onCleanup` calls that are lexically contained in a `createTrackedEffect`
callback, or in an `onSettled` callback whose call is proven to execute under a
live children-capable owner (a component body, memo, `createRoot`, …). When the
`onCleanup` call is the trailing statement of the callback, solid-checker offers
a safe fix that rewrites it to a `return`.

"Lexically contained" means the leaf callback is a function **literal written
directly in the owner's argument**, and the `onCleanup` sits in that literal's
own synchronous extent. `onSettled(wrap(() => onCleanup(dispose)))` and
`onSettled(makeCallback())` hand the owner a callback this analysis cannot
see — a wrapper may stash it and run it out-of-band, where there is no leaf
scope and no throw — and a call inside a nested function the callback merely
builds runs later, in that function's scope. The nested function is proven clean
for this leaf extent. Opaque wrapper/factory callbacks stay silent for this
specific SC3001 claim but produce SC9012 `reactive-dispatch-unresolved`; a
genuinely unowned cleanup is still reported by
[missing-owner](missing-owner.md).

`onSettled` is only a leaf owner when it is called *owner-backed*. Called
out-of-band — from an event handler, with no owner at all, or inside another
leaf scope — the rc.0 runtime enqueues the callback as a plain function instead:
`onCleanup` inside it does not throw (it warns, which
[missing-owner](missing-owner.md) reports), so this rule stays silent
there. Where the call site's ownership cannot be proven (an exported helper,
a conditionally supplied owner) the finding is reported as **uncertifiable**
rather than a proven violation. `createTrackedEffect` is a leaf owner
unconditionally.

The rule also follows the **dynamic extent** through exactly-resolved
helpers: a project function that calls `onCleanup` in its own synchronous
extent (its body minus nested function bodies) throws just the same when it
is called from a leaf scope, so that call site is flagged, naming the helper.
The resolution is the exact TypeScript identity, transitively through further
exact helper calls; an unresolved, ambiguous, or package callee contributes
nothing here — package behavior stays owned by the contract obligation
surface.

This is the static counterpart of Solid's dev-mode `CLEANUP_IN_FORBIDDEN_SCOPE`
error.

## Why is this bad?

Leaf owners own no child scopes, so there is nothing for `onCleanup` to register
on. Their cleanup contract is the return value — returning a function schedules
it for the next run or disposal. Calling `onCleanup` inside them throws in dev.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Widget() {
  onSettled(() => {
    // Owner-backed: the component body proves a live owner, so this
    // onSettled is a leaf owner and the call throws in dev.
    const id = setInterval(tick, 1000);
    onCleanup(() => clearInterval(id));
  });
  return <div />;
}
```

Examples of **correct** code for this rule:

```tsx
function Widget() {
  onSettled(() => {
    const id = setInterval(tick, 1000);
    return () => clearInterval(id); // The return value is the cleanup.
  });
  return <div />;
}

// Out-of-band onSettled runs a plain queued callback, not a leaf owner —
// this does not throw (the cleanup will never run; missing-owner warns).
<button onClick={() => onSettled(() => onCleanup(dispose))} />;
```

## How to fix

Return the cleanup function from the callback: do the setup, then
`return () => teardown()`. `onCleanup` remains the right tool inside computations
and component bodies — just not inside leaf owners.

## Related

- [primitive-in-leaf-owner](primitive-in-leaf-owner.md) — the same constraint for primitives
- [missing-owner](missing-owner.md) — `onCleanup` with no owner at all
- [missing-owner](missing-owner.md) — the out-of-band `onSettled` defect (a returned cleanup that is dropped)
