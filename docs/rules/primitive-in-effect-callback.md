# primitive-in-effect-callback

`SC6002` · **error** · violation

A memo, effect, root or other computation is created, with no owner set in
between, on the stack of a `createEffect` apply callback.

## What it does

Flags `createMemo`, `createEffect`, `createRenderEffect`, `createRoot`,
`createProjection`, and the function forms of `createSignal` and `createStore`,
when the call runs synchronously inside the apply (second) argument of a
`createEffect(compute, apply)` in a client build, reached directly or through
an IIFE, an inline standard-library callback, `untrack`, or project helpers,
and never through `runWithOwner`, `createRoot` or another call that sets an
owner (ADR 0483).

**Premise: read from the published bytes.** `@solidjs/signals@2.0.0-rc.17`
runs an effect's apply from the flush with the `inEffectCallback` marker up and
no owner, and both `createOwner` and `setupComputedNode` call
`assertNotInEffectCallback` (`dist/dev-shared.js:1496`, `:4927`, `:3947-3957`),
which throws `PRIMITIVE_IN_EFFECT_CALLBACK` when `inEffectCallback &&
context === null`. A production build creates the computation with no owner, so
nothing ever disposes it. Value-form state (`createSignal(0)`,
`createStore({})`) creates no computation and is not flagged.

**Premise (shared with `missing-owner`).** A `createEffect` apply runs from
the drain of the effect queue with no owner. A `flush()` called synchronously
under an owner (in a component or root body, or under `runWithOwner`) drains the
queue there and runs the apply under that owner: then nothing throws and the
computation is owned. Neither rule proves that no such call exists, including
in dependency code (docs/open-work.md).

## When it is uncertifiable

The marker is one global: an effect run synchronously inside the callback
(a render effect created under `runWithOwner`) lowers it when it returns. So the
claim is a violation only when no call runs in the apply before the creating
call (or tagged template); after one, or through a helper, it is uncertifiable. Only
rc.17's bytes were read: other releases are not claimed.

## Examples

```tsx
createEffect(() => count(), (value) => {
  const doubled = createMemo(() => value * 2); // throws in dev
});

const owner = getOwner();
createEffect(() => count(), (value) => {
  runWithOwner(owner, () => createMemo(() => value * 2)); // owned: fine
});
```

## How to fix

Create the computation in the effect's compute function or in the component
body, or parent it explicitly with `runWithOwner(getOwner(), …)` captured
outside the callback.
