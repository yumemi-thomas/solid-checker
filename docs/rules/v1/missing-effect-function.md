# v1/missing-effect-function

`SC7001` · **error** · violation

`createEffect` is called without an effect function as its first argument.

## What it does

Flags `createEffect` calls whose first argument is not a function — either
missing entirely, or a value that only type-checks because a cast defeated
TypeScript.

## Why is this bad?

`createEffect(fn, value?, options?)` takes the effect callback as its **first**
argument. That callback is what Solid runs, and re-runs whenever the reactive
values it reads change. Without it there is nothing to track and nothing to
re-run: the call is inert, and whatever reactivity it was supposed to drive
silently never happens.

TypeScript catches the plain no-argument case on its own. It does not catch a
value laundered through a cast, which is where this rule earns its place.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// No callback at all.
createEffect();

// First argument is not a function — the cast hides it from TypeScript.
createEffect(123 as unknown as () => void);
```

Examples of **correct** code for this rule:

```tsx
// The effect function is the first argument.
createEffect(() => {
  console.log(name());
});

// The optional second argument is the initial value passed to the callback
// as `prev` on its first run.
createEffect((prev: number) => prev + 1, 0);

// Cleanup is registered with onCleanup, not returned.
createEffect(() => {
  const id = setInterval(() => console.log(name()), 1000);
  onCleanup(() => clearInterval(id));
});
```

## How to fix

Pass the effect function as the first argument. If you need a starting value for
the `prev` parameter, pass it as the second argument — it is a value, not a
second callback.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — reads that do not track
- [v1/no-owner-effect](./no-owner-effect.md) — effects created without an owner
