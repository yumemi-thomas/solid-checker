# v1/primitive-in-leaf-owner

`SC3002` · **error** · violation

A reactive primitive (`createSignal`, `createMemo`, `createEffect`, …) is created
inside a leaf owner (`createReaction`'s callback).

## What it does

Flags calls that create reactive primitives when they are lexically contained in a
`createReaction` callback.

## Why is this bad?

`createReaction` runs its callback as a leaf owner with no children. A primitive
created inside it has no owner to attach to: it is never tracked into the graph
and never disposed, so its subscriptions leak and its updates go nowhere — once
per run of the reaction.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const track = createReaction(() => {
  // Leaf owner: this signal is never tracked or disposed.
  const [flash, setFlash] = createSignal(true);
  setFlash(false);
});
track(() => count());
```

Examples of **correct** code for this rule:

```tsx
// Create primitives in the component body; read their accessors in the reaction.
const [flash, setFlash] = createSignal(false);

const track = createReaction(() => {
  console.log(flash());
});
track(() => count());
```

## How to fix

Create the primitive in the component body (or another owning scope) and read its
accessor inside the `createReaction` callback. If the primitive's lifetime really
is tied to the reaction, the logic probably belongs in a computation
(`createEffect`, `createMemo`) rather than a reaction.

## Related

- [v1/cleanup-in-forbidden-scope](./cleanup-in-forbidden-scope.md) — the same constraint for `onCleanup`
- [v1/primitive-in-directive-application](./primitive-in-directive-application.md) — the directive analogue
