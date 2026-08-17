# no-owner-cleanup

`SC4002` · **warning** · violation (uncertifiable, reported as an error, for exported functions)

`onCleanup` is called without a reactive owner, so the cleanup function will never
run.

## What it does

Flags `onCleanup` calls that no component, computation, or root owner dominates.
When the call sits in an exported function whose call sites are outside the
project, the finding is reported as **uncertifiable** instead: solid-checker cannot
prove callers provide an owner. Like the SC9xxx rules, the uncertifiable form
carries **error** severity; the catalog's **warning** applies to the proven
violation form. An owner-backed `onSettled` callback is a leaf scope, so its
`onCleanup` call is owned by `cleanup-in-forbidden-scope` (SC3001), not duplicated
here. That de-duplication needs the callback to be the function literal written
directly in the owner's argument: `onSettled(wrap(() => { onCleanup(fn) }))` hands
the callback to a wrapper that may run it out-of-band, so it keeps SC4002, as do
genuinely out-of-band and module-scope calls.

## Why is this bad?

`onCleanup` registers its function on the current owner, to run when that owner is
disposed or re-executes. With no owner there is nothing to register on: the call
silently does nothing, and whatever resource the cleanup was meant to release
leaks.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// A bare helper called from module scope: the cleanup never registers.
function listen(target, type, handler) {
  target.addEventListener(type, handler);
  onCleanup(() => target.removeEventListener(type, handler));
}
listen(window, "resize", onResize);
```

Examples of **correct** code for this rule:

```tsx
// Called during component setup, the component owns the cleanup.
function Tracker() {
  listen(window, "resize", onResize);
  return <Chart />;
}

// Or give module-scope setup an explicit root:
createRoot(() => listen(window, "resize", onResize));
```

## How to fix

Call `onCleanup` inside a component or computation, or create the surrounding scope
with `createRoot` so disposal exists. For one-time setup with teardown in a
component, `onSettled(() => { setup(); return teardown; })` is the idiomatic
Solid 2.0 shape.

## Related

- [cleanup-in-forbidden-scope](cleanup-in-forbidden-scope.md) — `onCleanup` inside leaf owners
- [no-owner-settled-cleanup](no-owner-settled-cleanup.md) — the `onSettled` analogue
- [no-owner-effect](no-owner-effect.md) — unowned effects
