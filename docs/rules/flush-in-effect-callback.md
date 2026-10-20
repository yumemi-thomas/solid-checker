# flush-in-effect-callback

`SC2008` · **warning** · violation

`flush()` (or `flush(fn)`) runs on the stack of a `createEffect` apply callback,
inside the flush that is draining effects, where it cannot drain anything.

## What it does

Flags calls to `flush` (imported from `solid-js` or `@solidjs/signals`) that
provably run synchronously inside the apply (second) argument of a
`createEffect(compute, apply)` call, on an installation whose
`@solidjs/signals` reports it: `2.0.0-rc.1` and later reviewed releases.

**Premise: read from the published bytes and probed.** An effect's apply is
queued, and `runEffect` runs it between two marker writes (rc.13
`dist/dev.js:1876-1906`):

```js
setEffectCallback(true);
…
const nextCleanup = node._effectFn(node._value, node._prevValue);
…
setEffectCallback(false);
```

`flush` checks the marker once a flush is running (rc.13
`dist/dev-shared.js:2680-2698`):

```js
if (globalQueue._running) {
  if (inTrackedQueueCallback) { throw new Error("Cannot call flush() from inside onSettled or createTrackedEffect. …"); }
  if (inEffectCallback) {
    … emitDiagnostic({ code: "FLUSH_IN_EFFECT_CALLBACK", kind: "lifecycle", severity: "warn", message }) …
  }
  return;
}
```

So the call is a **no-op in every build** (production returns at the same
branch, `dist/prod/core/scheduler.js`), and the development build **warns**; it
does not throw. `flush(fn)` runs `fn` and then ends with a plain `flush()`,
which reports. The marker is in `@solidjs/signals` rc.1 through rc.13 (probed on
the published bytes of rc.1-rc.9 and rc.13); rc.0 has neither the marker nor the
report.

| position of the call | reports |
| --- | --- |
| directly in `createEffect`'s apply | **yes** |
| in a function the apply calls (same file or another), an immediately invoked function, an `untrack`/`createRoot`/`runWithOwner` callback, or an inline array-method callback written there | **yes** |
| `createEffect`'s compute function | no (marker not raised) |
| a `setTimeout`/microtask/`.then` callback created in the apply, an event handler | no (the effect phase is over; `flush()` drains) |
| `createRenderEffect`'s apply | first run: no, it is synchronous at creation with no flush running (`flush()` drains); later runs: yes |
| a cleanup the apply returns | next run's cleanup: yes (it runs inside the marker); disposal: no |
| `createTrackedEffect` and an owned `onSettled` | not this code: `flush` **throws** there in dev (`Cannot call flush() from inside onSettled or createTrackedEffect`); see [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md) |

## Why is this bad?

`flush()` exists to observe writes synchronously. Inside an effect the flush
that runs effects is already in progress, so there is nothing to start: the
writes made in the callback are processed in that flush's continuation, after
the callback returns, never at the `flush()` call. Code that reads state
after the call expecting the update sees the old value.

## Examples

Examples of **incorrect** code for this rule:

```tsx
createEffect(count, () => {
  setLog(log() + 1);
  flush(); // warns FLUSH_IN_EFFECT_CALLBACK in dev, drains nothing
});

function commit() {
  flush();
}
createEffect(count, () => commit()); // the same call, one function away
```

Examples of **correct** code for this rule:

```tsx
createEffect(count, () => {
  setLog(log() + 1); // processed in this flush's continuation
});

createEffect(count, () => {
  queueMicrotask(() => flush()); // after the effect phase: drains normally
});

function onClick() {
  setCount(count() + 1);
  flush(); // an event handler is outside the flush that runs effects
}
```

## How to fix

Remove the `flush()`: writes made in an effect are processed by the flush that
is already running. To force a drain afterwards, defer it
(`queueMicrotask(() => flush())`) or call it from code that runs outside the
effect.

## When it does not fire

- **Releases without the report.** The rule reads the resolved
  `@solidjs/signals` (the one the installed `solid-js` resolves). rc.0 has no
  marker, so it is silent there; a signals release no review read, or one that
  does not resolve, is not claimed (its `SC9014` notice says so).
- **Anything not proven on the apply's stack.** Each layer between the call and
  the apply must be proven to run during it: a call the dialect or the audited
  standard-library rows state runs its callback before returning, an immediately
  invoked function, or a project function bound to a name nothing reassigns that
  one resolved call from the apply reaches. Unresolved callees, generators
  (calling one runs none of its body), methods, parameter initializers, a
  `let` helper, a callback handed to a timer, listener or promise, and an async
  function's code after its first `await` are not claimed.
- **Positions it does not place.** An apply passed by name
  (`createEffect(v, applyFn)`), the `{ effect, error }` bundle form, and an
  apply of a `defer: true` effect whose compute never changes. `createRenderEffect`
  is never claimed (see the table).
- **Returned cleanups.** A cleanup function the apply returns reports only on
  the run that replaces it, so no single answer holds.

## Related

- [flush-in-action](flush-in-action.md): the same export refused while an
  action step runs, where it throws
- [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md): the throw for
  `flush` in `createTrackedEffect` and owned `onSettled` callbacks
