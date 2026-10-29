# flush-in-effect-callback

`SC2008` · **warning** · violation

`flush()` (or `flush(fn)`) that, whenever it runs, runs on the stack of a
`createEffect` apply callback, inside the flush that is draining effects, where
it cannot drain anything. The dev build usually also warns.

## What it does

Flags calls to `flush` (imported from `solid-js`) that can only run
synchronously inside the apply (second) argument of a
`createEffect(compute, apply)` call, in a client build, on an installation
whose `@solidjs/signals` has the marker below: `2.0.0-rc.1` and later reviewed
releases. The claim is the **no-op**, and it is **universal**: whenever the call
runs it runs inside the flush that drains effects, so the queue is already
running and the call drains nothing; the writes are processed in that flush's
continuation. It holds for every execution, including none, so reachability is
not part of the proof: a `flush()` in dead code (after a `return`, under
`if (false)`, in an effect that is never registered) is reported, because the
call is pointless wherever it runs. The dev warning is the usual symptom, not
the claim.

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

So the call is a **no-op in every client build** (production returns at the same
branch, `dist/prod/core/scheduler.js`), and the development build **usually
warns**; it does not throw. The marker `inEffectCallback` is one global boolean:
a nested synchronous `runEffect` (a `createRenderEffect` created in the apply
runs its first pass at creation, so does JSX there) sets it to `false` in its
`finally` (`dist/dev.js:1906`) without restoring it, and a `flush()` later in
the same apply is still a no-op but raises no warning. Such calls are reported
anyway, because the no-op is provable from source. `flush(fn)` runs `fn` and then ends with a plain `flush()`,
which reports. The marker is in `@solidjs/signals` rc.1 through rc.13 (probed on
the published bytes of rc.1-rc.9 and rc.13); rc.0 has neither the marker nor the
report.

| position of the call | reports |
| --- | --- |
| directly in `createEffect`'s apply | **yes** |
| in a function only effects call (not exported, every reference a call from a proven layer), an immediately invoked function, an `untrack`/`createRoot`/`runWithOwner` callback, or an inline array-method callback written there | **yes** |
| `createEffect`'s compute function | no (marker not raised) |
| a `setTimeout`/microtask/`.then` callback created in the apply, an event handler, a helper that also has such a caller | no (the effect phase is over; `flush()` drains) |
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
- **Anything that may run elsewhere.** Each layer between the call and the
  apply must be proven to run only during it: a call the dialect or the audited
  standard-library rows state runs its callback before returning, an immediately
  invoked function, a project helper that calls the callback in its own body and
  never hands it on or keeps it (ADR 0484), or a project function bound to a
  name nothing reassigns, not exported itself (one nested in an exported
  function is not), with at least one call, whose every reference is an exact
  call that is itself such a layer. A helper that is also called from an event
  handler, a timer, the module body or another non-effect function, aliased,
  passed as an argument or exported is not claimed: the flush may run outside
  the effect. Unresolved callees, generators (calling one runs none of its
  body), methods, parameter initializers, a `let` helper, a callback handed to
  a timer, listener or promise, and an async function's code after its first
  `await` are not claimed.
- **Server builds.** With no runtime selection the client build is the
  default (a documented premise, as the audited release is everywhere else).
  With a Node-family target (`node`, `deno`, `worker`) or
  `string-ssr`/`streaming-ssr` selected, `solid-js`'s server entry never runs an
  apply (`createEffect` is `serverEffect(compute, undefined, options)`) and its
  `flush` is empty, so nothing is claimed. With no selection the client build is
  the default.
- **Two installed copies.** Package identity is decided by runtime module
  resolution, not declarations (TypeScript merges a second copy at the same
  version into the first copy's declarations, while the runtime loads both, each
  with its own scheduler queue). The backend resolves `solid-js` from each
  analyzed file's real directory (nearest `node_modules`, symlinks resolved);
  the file holding the `flush` call and the file holding the `createEffect` call
  must load the same copy, and both calls' declarations must be in one package
  directory. Each import is followed through project re-exports (barrels,
  `export { a as b } from`, `export *`, a namespace member) to the file that
  imports `solid-js` itself; a hop that does not resolve exactly, or a call
  imported through another package (whose `exports` conditions pick the file
  and so the copy), is not claimed. A file whose `solid-js` is not resolved on
  disk (a caller with no disk, such as the WASM adapter) is not claimed. A
  `flush` imported from `@solidjs/signals` itself, which Type Facts do not
  resolve to a declaration, is not claimed.
- **Positions it does not place.** An apply passed by name
  (`createEffect(v, applyFn)`), the `{ effect, error }` bundle form, and an
  apply the engine proves absent (`defer: true` on a constant compute: it
  reports nothing there, ADR 0302). `createRenderEffect`
  is never claimed (see the table).
- **Returned cleanups.** A cleanup function the apply returns reports only on
  the run that replaces it, so no single answer holds.

## Premises

- **Universal claim.** Dead code is reported on purpose; there is no
  reachability, completion or constant-folding analysis. What the rule needs is
  that no execution of the call is outside the effect phase.
- **Same queue.** The apply's queue is the one the flush finds running when the
  two calls load one installed `solid-js` (above).

## Related

- [flush-in-action](flush-in-action.md): the same export refused while an
  action step runs, where it throws
- [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md): the throw for
  `flush` in `createTrackedEffect` and owned `onSettled` callbacks
