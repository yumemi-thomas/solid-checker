# v1/missing-effect-function

`SC7001` · **error** · violation

`createEffect` is called without an effect function as its first argument.

## What it does

Flags `createEffect` calls whose first argument is provably not a function but
only type-checks because a TypeScript assertion defeated the published
signature. Missing and raw invalid arguments are TypeScript's diagnostics.
An `any`, unresolved value, or type-correct spread is **uncertifiable**: its
runtime callback may be callable or non-callable, so neither a violation nor
safety is proven. A compiler-proven callable identifier stays silent.
When such a call is covered by a `"use server"` directive, the finding is
**uncertifiable**. The directive is a framework and bundler convention that no
core Solid package reads, so it proves neither client nor server execution.
The call fails on the client, while 1.x's server entry compiles `createEffect`
to a bare no-op. The 2.0 rule preserves the same uncertainty.

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
// First argument is not a function — the cast hides it from TypeScript.
createEffect(123 as unknown as () => void);
```

Examples whose outcome is **uncertifiable**:

```tsx
declare const callback: any;
createEffect(callback);

declare const args: Parameters<typeof createEffect>;
createEffect(...args);
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
- [v1/missing-owner](./missing-owner.md) — effects created without an owner
