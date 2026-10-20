# flush-in-effect-callback

`SC2008` · **warning** · violation

`flush()` (or `flush(fn)`) runs on the stack of a `createEffect` apply callback,
inside the flush that is draining effects, where it cannot drain anything. The
dev build usually also warns.

## What it does

Flags calls to `flush` (imported from `solid-js` or `@solidjs/signals`) that
provably run synchronously inside the apply (second) argument of a
`createEffect(compute, apply)` call, in a client build, on an installation
whose `@solidjs/signals` has the marker below: `2.0.0-rc.1` and later reviewed
releases. The claim is the **no-op**: the queue is already running, so the call
drains nothing and the writes are processed in that flush's continuation. The
dev warning is the usual symptom, not the claim.

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
- **Unreachable code.** A positive allowlist: the call, and the
  `createEffect` registration itself, are claimed only when every construct on
  the path from the enclosing function's entry (or the module body) is an
  allowlisted shape: blocks; `if`/`else`, `while` and `for` whose test does not
  fold to the constant that skips the branch; `for`-`of`/`in` over a non-literal
  iterable; `switch` over a non-constant discriminant; `try` and `finally`
  blocks (never a `catch`, since no throw is proven to reach it); and, inside an
  expression, call/`new`/`await`/unary/binary/sequence operands, simple
  assignments, and the right of `&&`/`||`/`??` or an arm of `?:` unless the left
  side or test folds to the constant that skips it. Anything else on the path
  is not claimed. Every statement and expression evaluated earlier on the path
  must show no proven abrupt completion: `return`, `throw`, `break`,
  `continue`, a call that never returns (a `never` signature, an exactly
  resolved project function that only throws, an inline callback that only
  throws over a literal array with a first element), an infinite loop without
  `break`, a `switch` whose arms all leave, or a read of a name a same-file
  getter that cannot return is declared under (any computed read, spread or
  destructuring while one exists). Constant folding is exact (literals, `!`,
  `typeof`, `void`, unary minus, `&&`/`||`/`??`, `?:`, comparisons of folded
  primitives); anything it cannot fold is non-constant. Array methods are
  claimed only on a receiver that is not an array literal (it may be empty,
  sparse or one element), and never `sort`, `toSorted`, or `reduce` and
  `reduceRight` without an initial value. A function the analysis cannot parse
  or find is not claimed.
- **A compute that never returns.** When the effect's compute is a function
  written inline, or an exactly resolved project function, it must be able to
  return a value; one that only throws (directly, through `never`, or through a
  helper that only throws) leaves the apply unrun. Any other compute (a signal
  accessor, a member, a package function) is assumed able to return, like every
  unknown callee. An `async` or generator compute is refused: the apply is not
  claimed. So is one whose body reads a throwing same-file getter first.
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
  analyzed file's directory (nearest `node_modules`, symlinks resolved); the file
  holding the `flush` call and the file holding the `createEffect` call must load
  the same copy, and both calls' declarations must be in one package directory.
  A file whose `solid-js` is not resolved on disk (a caller with no disk, such as
  the WASM adapter) is not claimed. A `flush` imported from `@solidjs/signals`
  itself, which Type Facts do not resolve to a declaration, is not claimed.
- **Positions it does not place.** An apply passed by name
  (`createEffect(v, applyFn)`), the `{ effect, error }` bundle form, and an
  apply of a `defer: true` effect whose compute never changes. `createRenderEffect`
  is never claimed (see the table).
- **Returned cleanups.** A cleanup function the apply returns reports only on
  the run that replaces it, so no single answer holds.

## Premises

- **Feasible paths.** Non-constant conditions may take either branch, and calls
  not proven never to return return normally. `if (next > 100) flush()` is
  reported on that premise; it is the only assumption left under the allowlist
  above. Constants are folded exactly and honored: a test that folds to the
  constant that skips a branch withholds the branch, one that folds to the
  constant that runs it keeps it.
- **The apply runs.** `defer: true` effects whose compute never changes, or
  whose options the syntax cannot read, are not claimed.

## Related

- [flush-in-action](flush-in-action.md): the same export refused while an
  action step runs, where it throws
- [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md): the throw for
  `flush` in `createTrackedEffect` and owned `onSettled` callbacks
