# primitive-in-effect-callback

`SC6002` · **error** · violation

A memo, effect, root or other computation is created, with no owner set in
between, on the stack of a `createEffect` apply callback.

## What it does

Flags `createMemo`, `createEffect`, `createRenderEffect`, `createRoot`,
`createProjection`, the function forms of `createSignal` and `createStore`,
`Loading`/`Errored` called as functions, and `resolve` and `until`, which create
a root and a computation for their thunk (ADR 0614), when the call runs synchronously inside the apply (second) argument of a
`createEffect(compute, apply)` in a client build, reached directly or through
an IIFE, an inline standard-library callback, `untrack`, or project helpers,
a named function handed to such a callback position or a callback prop the
child component only calls (ADR 0538; a hand-off nothing places is
uncertifiable), and never through `runWithOwner`, `createRoot` or another call
that sets an owner (ADR 0483). A `createReaction` callback, the apply of a
deferred user effect, is claimed the same way (ADR 0541).
An apply `defer: true` provably never runs (ADR 0302) claims nothing, in itself
or in a function only it calls (ADR 0590).

**Premise: read from the published bytes.** `@solidjs/signals@2.0.0-rc.17`
runs an effect's apply from the flush with the `inEffectCallback` marker up and
no owner, and both `createOwner` and `setupComputedNode` call
`assertNotInEffectCallback` (`dist/dev-shared.js:1496`, `:4927`, `:3947-3957`),
which throws `PRIMITIVE_IN_EFFECT_CALLBACK` when `inEffectCallback &&
context === null`. A production build creates the computation with no owner, so
nothing ever disposes it. Value-form state (`createSignal(0)`,
`createStore({})`) creates no computation and is not flagged.

**The flush veto (ADR 0613, shared with `missing-owner` and SC2001).** A
`createEffect` apply runs from the drain of the effect queue with no owner. A
`flush()` called synchronously under an owner (in a component or root body, a
memo compute, under `runWithOwner`) drains the queue there and runs the apply
under that owner: then nothing throws and the computation is owned. So the claim
is a violation only while every `flush` of the program is proven to run with no
owner (a handler, a timer, past an await, module scope, `runWithOwner(null,
…)`) or to drain nothing (in an effect apply); any other call, or a reference
to `flush` that is not a call, makes it uncertifiable program-wide. A flush in
dependency code is outside the facts (premise, docs/open-work.md).

## When it is uncertifiable

The marker is one global: an effect run synchronously inside the callback
(a render effect created under `runWithOwner`) lowers it when it returns. So the
claim is a violation only when no call runs in the apply before the creating
call (or tagged template): in the apply and in each layer the walk crosses, and
in a helper's own body before it calls its callback (ADR 0495); after one it is
uncertifiable. The tag of a template the walk climbs through
(``hf`x` ``) is the call that runs the path, not an earlier one; a call in its
substitutions is (ADR 0586). A `createRenderEffect` apply (its first run is owned, its later
runs are not) and a cleanup an apply returns (a rerun runs it in the phase, a
disposal outside it) are uncertifiable, never violations (ADR 0541). Only
rc.17's bytes were read: other releases are not claimed.

A creating call in a function no call edge places (a module-level object or
class method, a getter, a slot, a tag, a function handed where nothing follows
it, or a named helper one of those calls) is uncertifiable too, blocker
`function-run-unplaced` (ADR 0710), when the project names a primitive that
defers a callback: an apply may be what runs it (Chrome throws for each, and a
custom element's `connectedCallback` is one). Not asked: a creation inside
`runWithOwner` or another primitive's synchronous callback, a body that may be a
component's, and a role that runs off every apply (a handler with no
synchronous dispatch in the program, a timer, rendering, module code).

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
