# v1/cleanup-in-forbidden-scope

`SC3001` · **error** · violation

`onCleanup` is called inside a leaf owner (`createReaction`'s callback), which has
no owner to register cleanup on.

## What it does

Flags `onCleanup` calls that are lexically contained in a `createReaction`
callback.

## Why is this bad?

`createReaction` runs its callback as a leaf: it owns no scope of its own, so
there is nothing for `onCleanup` to register on — the cleanup function will never
run, and whatever resource it was meant to release leaks. Returning the cleanup
from the callback does not help either: Solid 1.x never reads a callback's return
value as a cleanup (an effect's return value becomes the next run's `prev`).
Cleanup in 1.x is `onCleanup` in an owned scope, and only that.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const track = createReaction(() => {
  const id = setInterval(tick, 1000);
  onCleanup(() => clearInterval(id)); // Never runs: the callback is a leaf.
});
track(() => count());
```

Examples of **correct** code for this rule:

```tsx
function Ticker() {
  const track = createReaction(() => console.log("count changed"));
  track(() => count());

  // Register the cleanup in the computation that owns the reaction.
  const id = setInterval(tick, 1000);
  onCleanup(() => clearInterval(id));
  return <Clock />;
}

// Or give module-scope setup an explicit root so disposal exists:
createRoot(() => {
  const id = setInterval(tick, 1000);
  onCleanup(() => clearInterval(id));
});
```

## How to fix

Register the cleanup in the computation that owns the `createReaction` instead,
or create the surrounding scope with `createRoot` so disposal exists. Do not
return the cleanup function from the callback — that is not a cleanup mechanism
in Solid 1.x.

## Related

- [v1/primitive-in-leaf-owner](./primitive-in-leaf-owner.md) — the same constraint for primitives
- [v1/no-owner-cleanup](./no-owner-cleanup.md) — `onCleanup` with no owner at all
