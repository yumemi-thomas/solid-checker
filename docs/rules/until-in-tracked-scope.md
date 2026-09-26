# until-in-tracked-scope

`SC2005` · **error** · violation

`until(fn)` is called inside a tracked scope — a memo or effect compute, a
`createTrackedEffect` callback, or tracked JSX.

## What it does

Flags calls to `until` (imported from `solid-js`, exported from `2.0.0-rc.5`
on; see "When it does not fire") whose call site provably runs under an active
observer: directly
inside a tracked, non-deferred callback (`createMemo`'s compute,
`createEffect` / `createRenderEffect`'s compute function,
`createTrackedEffect`, a boundary body) or inside a compiler-proven tracked JSX
region.

**Premise: read from the published bytes** of `@solidjs/signals@2.0.0-rc.9`.
The dev bundle opens `until` with the guard `resolve` has
(`dist/dev.js:2718-2722`):

```js
if (getObserver()) {
  throw new Error(
    "Cannot call until inside a reactive scope; await it from an action or another imperative scope."
  );
}
```

`getObserver()` is `tracking ? context : null` (`dist/dev-shared.js:2826-2829`),
so the scopes that throw and the scopes that are legal are exactly
[resolve-in-tracked-scope](resolve-in-tracked-scope.md)'s, and this rule uses
the same scope proof.

The guard is **dev-only**: `dist/prod/signals.js:530` has none. In production
each run of the scope calls `until` again, and each call builds a new root and
user effect that wait on the predicate.

## Why is this bad?

`until` is the imperative "wait until the world confirms this" primitive: it
returns a promise that settles on the first truthy evaluation of its predicate.
Inside a computation it is backwards — the computation already re-runs when its
inputs change, and a promise it creates is not something it can wait on. The
runtime throws in dev to force the redesign.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const saved = createMemo(() => {
  // Throws in dev: an observer (the memo) is active.
  void until(() => ready());
  return ready();
});

createEffect(
  () => until(() => ready()), // throws in dev
  () => {}
);
```

Examples of **correct** code for this rule:

```tsx
// An action step, as rc.9 documents it:
const send = action(function* (text: string) {
  yield;
  yield until(() => confirmed(), { timeout: 10_000 });
});

// Imperative code:
async function onSubmit() {
  await until(() => ready());
  proceed();
}
```

## How to fix

Inside a computation, read the condition directly — tracked reads re-run the
computation when it changes. Keep `until()` for imperative code: an action
step, an event handler, `onSettled`, an effect's apply function.

## When it does not fire

- **`untrack` callbacks, component bodies, event handlers, effect apply
  callbacks, `createRoot` bodies, module scope** — no observer is active in any
  of them, as for `resolve`.
- **Helpers.** The proof is lexical: an `until()` inside a named helper that a
  memo happens to call is not claimed.
- **Before rc.5.** `until` arrives in `@solidjs/signals@2.0.0-rc.5`, and the
  `solid-js` root re-exports it from the same release. On an installation
  where either is older, or either is a release nobody compared, `until` is not
  a name the vocabulary knows, so the rule cannot fire: on rc.3 and rc.4 the
  import is TS2305 and neither runtime has the export (the rc.1-rc.8 release
  review § 6 measured the rule firing there before this gate;
  `fixtures/reactive-ir/release-triple-until-rc3` pins it silent). A fresh
  install of `solid-js@2.0.0-rc.3` over signals rc.9 is the same case, because
  rc.3's root does not re-export `until`.

## Related

- [resolve-in-tracked-scope](resolve-in-tracked-scope.md) — the same guard on
  `resolve`
- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — the
  owner-keyed counterpart for writes
