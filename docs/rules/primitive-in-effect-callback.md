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
an IIFE, an inline standard-library callback, a callback of `untrack`,
`latest`, `isPending` or `flush(fn)` (ADR 0944: each runs the callback in place
and sets no owner), `runWithOwner(null, fn)` or `runWithOwner(owner, fn)` with
the `owner` that `getOwner()` returned in the apply itself (both `context ===
null`), or project helpers,
a named function handed to such a callback position or a callback prop the
child component only calls (ADR 0538; a hand-off nothing places is
uncertifiable), and never through `createRoot` or another call that sets an
owner, nor through `runWithOwner(undefined, fn)` (the guard tests `=== null`),
an owner captured in a component body, or an owner the walk cannot follow
(ADR 0483). `getOwner()` is no earlier call. A `createReaction` callback, the apply of a
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

Written JSX creates too (ADR 0883): an intrinsic's dynamic child, attribute or spread, a
fragment's dynamic child and a component element, on an apply's stack with no owner set.
Static JSX creates nothing.

## When it is uncertifiable

A walk past its bound (13 helpers, ADR 0750) is uncertifiable, `search-depth-limit`.

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

## Generators and parameter initializers (ADR 0961)

- **A generator stepped from an effect's apply.** A creating call in a generator body an apply steps is an uncertifiable
  row (`generator-step-unplaced`) where the stepping is not followed; a generator the apply calls and discards runs none of
  its body.
- **A parameter initializer.** A creating call written in a default value, a destructuring default, an IIFE or closure in
  one, or a constructor's default, of a function the program calls or references, is an uncertifiable row
  (`parameter-initializer-unplaced`), never silence: the component that merely declares the function does not run its
  defaults, and the role veto (`runs off every apply`) cannot clear a default, which no role places.

## Exported helpers (ADR 0963)

An exported function is entered by calls outside the project as well, so the claim that its code only ever runs in an
effect's apply is not made; the calls the walk sees are proven runs there. A helper an apply calls creates a computation
with no owner (Chrome: `PRIMITIVE_IN_EFFECT_CALLBACK`), exported or not. For an exported function the origin is `mixed`
(ADR 0516): the creation is an uncertifiable row, never a violation and never silence; the unexported twin stays a
violation, and an exported helper only an event handler calls stays silent.
A call the call graph withheld (ADR 0339: a `const` arrow is not hoisted, and a call through an import cycle, a self-import included,
may run before the binding's declarator) is followed as a run the walk does not prove: the origin it reaches is `mixed`, an
uncertifiable row, where the helper was certified clean for want of a call edge.

## Endings of the creation sites (ADR 0969)

Every creation the dialect says throws on an effect's apply (`createMemo`, `createEffect`, `createRenderEffect`, `createRoot`, `render`, `createProjection`, `Loading`, `Errored`, `resolve`, `until`, a function-form `createSignal` or `createStore`) is a site on a client build. It ends in a row of this rule, an SC9012, the compiler's discard, or a silence that names the fact the throw needs and the code lacks (`inEffectCallback && context === null`); with none it gets an uncertifiable `SC6002` row, `effect-creation-unaccounted`, never a violation. The silences: the site never runs; it is at module scope; the role engine places it off every apply (rendering, a deferred callback, a directive apply, module code, discarded code, an event handler where the program dispatches no event synchronously); an exact primitive whose callback creates an owner wraps it (`createRoot`, a computation's compute: not `runWithOwner`, whose owner may be a value that is null); or the owner graph gives the owning function an owner on every entry (no unowned bit), the site is plain and no layer around it is entered by a reference that is no direct call. The graph's answer is given only for a function no package the build does not read may run (ADR 0967). A creation in a helper a timer and a component body both call, a handler in a program that dispatches an event synchronously, and a function kept in a collection keep the row.

## How to fix

Create the computation in the effect's compute function or in the component
body, or parent it explicitly with `runWithOwner(getOwner(), …)` captured
outside the callback.
