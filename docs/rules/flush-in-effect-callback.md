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
| in a function only effects call (not exported, every reference a call from a proven layer), an immediately invoked function, an `untrack`/`latest`/`isPending`/`flush(fn)`/`createRoot`/`runWithOwner` callback (ADR 0944: `isPending` catches a throw, and this is a warning), or an inline array-method callback written there | **yes** |
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
  passed as an argument or exported is not claimed as a violation: the flush may
  run outside the effect (an exported one an apply calls is an uncertifiable row,
  ADR 0963). Unresolved callees, generators (calling one runs none of its
  body), methods, parameter initializers, a `let` helper, a callback handed to
  a timer, listener or promise, and an async function's code after its first
  `await` are not claimed. A named function handed to an inline carrier
  (`[1].forEach(f)`, ``f`x` ``, `f.call(…)`, `Reflect.apply(f, …)`) runs at
  the carrier, which is a layer; a callback prop of a project component that
  only calls it (`props.run()`, a destructured `run()`) runs at those calls
  (ADR 0538). One handed to a project helper, written as a member's value or
  an object slot, or a method of a class written in the apply is reported as
  uncertifiable, anchored at the code around it.
- **A handler a dispatch may reach from an apply.** The guard takes no event, so
  a flush in a listener an apply's `click()` fires warns. A flush in a function
  that is no component is uncertifiable while any dispatch of the program has no
  placed caller (ADR 0639); one after an await, in a timer or in a handler costs
  nothing.
- **A function no call edge places.** A flush in a module-level object method
  or slot, a class member, a bundle's `effect`, a `Map` slot, a tag, or a
  function handed where nothing follows it (or in a named helper one of those
  calls) is uncertifiable, blocker `function-run-unplaced` (ADR 0709), when
  the project names a primitive that defers a callback: an apply may be what
  runs it, and Chrome warns when one does. A proven apply position stays a
  violation.
- **A walk past its bound** (13 helpers, 400 sites, 4 helpers in the net):
  uncertifiable, `search-depth-limit` (ADR 0750).
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
  reports nothing there, ADR 0302, nor in a function only such an apply
  calls, ADR 0590). `createRenderEffect`'s apply is
  uncertifiable on rc.17, never a violation (ADR 0541; see the table).
- **Returned cleanups.** A cleanup function the apply returns reports only on
  the run that replaces it, so no single answer holds: uncertifiable (ADR 0541).
- **`createReaction`.** On rc.17 its callback is a deferred user effect's apply
  and runs only from the draining flush: claimed as an apply is (ADR 0541).

## Generators and parameter initializers (ADR 0961)

- **A generator stepped from an effect's apply.** The origin walk continues through a generator literal handed to a call
  and through a named generator whose exact project call stepping it sits in the apply; a flush in its body is an
  uncertifiable row (`generator-step-unplaced`) where the stepping is not followed, and a violation where the body is
  visible and stepped in place. A call a statement discards runs none of the body.
- **A parameter initializer.** A `flush` written in a default value, a destructuring default, an IIFE or closure in one,
  or in a helper the default calls, of a function the program calls or references, is an uncertifiable row
  (`parameter-initializer-unplaced`): it runs when a call omits the argument, and no walk reads which calls do.

## Exported helpers (ADR 0963)

An exported function is entered by calls outside the project as well, so the claim that its flush only ever runs in an
effect's apply is not made; the calls the walk sees are proven runs there. For an exported function the origin is `mixed`
(ADR 0516): a `flush()` in a helper an apply calls is an uncertifiable row, never a violation and never silence; the
unexported twin stays a violation.
A call the call graph withheld (ADR 0339: a `const` arrow is not hoisted, and a call through an import cycle, a self-import
included, may run before the binding's declarator) is followed the same way, as a run the walk does not prove.

## Premises

- **Universal claim.** Dead code is reported on purpose; there is no
  reachability, completion or constant-folding analysis. What the rule needs is
  that no execution of the call is outside the effect phase.
- **Same queue.** The apply's queue is the one the flush finds running when the
  two calls load one installed `solid-js` (above).

## Accounting (ADR 0966)

Every flush the flush census lists ends, for this code, in a row of this rule at the site, an SC9012, the compiler's discard, or a typed reason (the program names no primitive that could put the flush where this code applies, or the flush's caller is on an empty stack: a scheduled callback, code past an `await`, module evaluation, a test runner's callback written directly, or a site nothing enters). A flush none of those ends gets an uncertifiable `SC2008` row, `flush-in-effect-unaccounted`: the walks that place a flush also answer "placed off" for a function no call edge reaches, a body that may be a component and an owner set around the call, none of which is a proof, so those sites stay uncertifiable (never a violation). The program-wide reason (the program names no primitive that could put the flush where this code applies) is not a proof for a flush in a function an installed package the build does not read as source may run: the package calls the primitive itself, on a function the project hands it (`runStep(() => { flush(); })`), and nothing in the build's table shows it. Such a flush gets no program-wide reason and keeps its `SC2008` row, `flush-in-effect-unaccounted`: a function written as an argument or inside one (an object's member, an array's element), a reference to it through any copy, assignment or import, a JSX prop, spread or child of a component the program does not read, a project function's parameter that goes on to one of these, a function a project function returns (`pkg.run(make())`), a reflective call that forwards its arguments (`pkg.run.call(null, cb)`, `Reflect.apply`, `bind`), and a helper the handed callback calls (ADR 0967). A callee is known, and so exempt, by its resolved declaration: a dialect primitive, a reviewed test runner's registration, the standard library, a project function, a DOM element, or a package the build reads as source; `expect`, `vi.*` and `waitFor` are not exempt. The default is the flip's (the review of #216): a value counts as handed unless a fact of the program proves where it goes, so a function kept in a collection or an iterable's element, an assignment into an object a package exports or `this`, a class's functions, a project component's props that go on to the package, `<Dynamic component={…}/>` of a package's component, a function a handed function returns, a callback run inline by known code (the standard library, a project parameter, a dialect primitive) on the stack of a step a package runs, a reflective call whose target is a project function that goes on, a namespace that escapes whole and an export of an open program keep the row; a value that is only called, compared, registered with the host (`addEventListener`), assigned to a local element or handed to a project function that does none of these is not handed. A program that installs fake timers (`vi.useFakeTimers()`, `jest.useFakeTimers()`: a test advances the clock on the stack it is on, which may be an action's step) has no scheduled-callback reason either: a flush written in the callback of `setTimeout`, `queueMicrotask` or `Promise.then` keeps its row of each code (F1 of the review of #213, ADR 0967; the clock is global to the run, so the question is the program's). A test runner's callback, the code past an `await` and module evaluation are unchanged.

## Related

- [flush-in-action](flush-in-action.md): the same export refused while an
  action step runs, where it throws
- [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md): the throw for
  `flush` in `createTrackedEffect` and owned `onSettled` callbacks

Through a project module that re-exports Solid, the called function's package is that of its exact symbol's declarations (ADR 0517).

ADR 0967, review causes A–E: containment alone does not place a callback. A receiver inside an initializer, an iterable, a template part or an operand of `yield` or `await` stays handed; a project function naming `arguments` hands every argument, and a callback's returned value follows its receiving call whether it is given literally or by an exact current named reference (an unresolved member remains uncertifiable). An intrinsic element's attributes other than compiler-recorded event handlers, its children and its spreads (also `Dynamic` with a string tag) run on the creating stack. Opening and closing JSX names ask the same render owner, and code in an element's prop or child expression (a nested element, a call, an inline callback) runs where that element evaluates it: under a component nothing reads, when the package reads the prop; a project component's props and an object literal's getter run where they are read, and a method call on them binds `this` to the object. `this` and `super` name an object no reference of it shows: a value placed directly in an object literal that writes one (a method, a getter, a function it names, a prototype, a spread's members) is handed, as is a prototype installed by a write of `__proto__` or `prototype` (a `const` literal that writes no receiver takes the write as a `__proto__:` key). A function stored under a key a receiver reads (`this.k`, `super.k`, `const { k } = this`; any key when the receiver is used as a value or the key is not named), in code a package may run, is handed unless its holder provably cannot be a receiver (a plain `const` literal every use of which keeps it so, and what those uses give out asked in turn): one assigned after the literal, a derived literal, an `Object.create` result, a class's instance. In a program that can name `Object.prototype` (a route to it under any name, a read under a key no expression proves, code it builds as a string, an installed package whose closure names it, builds code or is not read to its end) no holder is proved plain, and where code is built as a string or a package is not read to its end any key is read through any receiver: a function stored in an object keeps its row. A key is the property key the runtime gives it (a unicode escape is decoded, `0x1` is `1`), a parameter that is a pattern or a rest reads every key where its function runs, and `Object.assign(this, …)` is a write of any key. A function stored through a parameter's member (`t.m = f` in `function put(t) { … }`) is stored into the object the callers pass: it is followed to each caller's argument, under the name written at the call, when every use of the function is a direct call the build resolves, and handed otherwise (an alias of the object made before the call, a copy of the parameter, a default or a spread argument, a method of an object literal and a call from another file are not followed: `docs/open-work.md`). An assignment expression's value is followed like any value. Unproved hand-offs retain the uncertifiable row.
