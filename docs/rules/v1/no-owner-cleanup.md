# v1/no-owner-cleanup

`SC4002` · **warning** · violation (uncertifiable for exported functions)

`onCleanup` is called without a reactive owner, so the cleanup function will never
run.

## What it does

Flags `onCleanup` calls that no component, computation, or root owner dominates.
When the call sits in an exported function whose call sites are outside the
project, the finding is reported as **uncertifiable** instead: solid-checker cannot
prove callers provide an owner.

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
component, `onMount(() => { setup(); onCleanup(teardown); })` is the idiomatic
shape.

## Related

- [v1/no-owner-effect](./no-owner-effect.md) — unowned effects
- [v1/cleanup-in-forbidden-scope](./cleanup-in-forbidden-scope.md) — `onCleanup` inside a leaf owner
