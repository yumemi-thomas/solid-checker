# flush-in-action

`SC2006` · **error** · violation

`flush()` (or `flush(fn)`) is called in the body of an `action`, while one of
its steps is running.

## What it does

Flags calls to `flush` (imported from `solid-js` or `@solidjs/signals`) that
sit directly in the generator passed to `action(...)`, at a position that
provably runs inside one of the action's steps, on an installation whose
`@solidjs/signals` throws there: `2.0.0-rc.8` and later reviewed releases.

**Premise: read from the published bytes and probed.** From
`@solidjs/signals@2.0.0-rc.8`, `flush` opens with a guard on a counter only
`action` raises (rc.9 `dist/dev-shared.js:2210-2219`; rc.8 `:1904-1913`):

```js
function flush(fn) {
  if (actionStepDepth > 0) {
    throw new Error("[FLUSH_IN_ACTION] flush() inside an action body is not allowed. …");
  }
  if (fn) { … }
```

`action`'s `step` raises that counter around exactly one call into the
generator (rc.9 `dist/dev.js:2016-2024`; rc.8 `:1682-1690`):

```js
enterActionStep();
try {
  r = err ? it.throw(v) : it.next(v);
} catch (e) { exitActionStep(); … }
exitActionStep();
```

So "inside an action body" means *inside the synchronous slice of the
generator that one `next()` runs*. Probed on rc.0-rc.9, dev and production
bundles:

| position in the action | rc.8, rc.9 dev | rc.0-rc.7, and every production build |
| --- | --- | --- |
| sync generator, anywhere in its body: before or after `yield`, `yield promise`, `yield*` | **throws** | runs |
| async generator, before its first `await` | **throws** | runs |
| async generator, after a `yield` that follows an `await` | **throws** | runs |
| async generator, after an `await`, a `for await` or a `yield*`, with no `yield` since | runs | runs |
| `await flush(fn)` (the call is the awaited operand) | **throws** | runs |
| `flush(await x)` (the await runs first) | runs | runs |
| a helper function the body calls synchronously | **throws** | runs |
| an `untrack`, `latest`, `createRoot` or `runWithOwner` callback in the body that calls `flush()` (ADR 0944) | **throws** | runs |
| an `isPending(() => flush())` callback (it catches the throw, no diagnostic precedes it) | runs | runs |
| `flush(() => flush())` | **throws** at the outer call; the inner never runs | runs |
| the inner body of a nested action the outer body invokes | **throws** | runs |
| a parameter initializer, `function* (x = flush())` | runs (before the first step) | runs |
| `setTimeout` / `.then` callbacks created in the body | runs | runs |
| an event handler that calls the action and then `flush()` | runs | runs |

`flush()` and `flush(fn)` behave the same: the guard comes before `fn` is read.

The throw is **dev-only**. The production and `observe` builds take the same
branch and return `fn?.()` without draining (rc.9
`dist/prod/core/scheduler.js:1182-1184`), so production does not throw, and the
flush reveals nothing.

## Why is this bad?

An action's writes are held by its transaction until the action settles, so a
flush inside a step cannot reveal them. The comment on the guard gives the
reason it was added (solidjs/solid#3333): the drain loop only exits once no
transition is active, so a drain mid-step would park the action's transaction
and every write after it in the body would land as a plain committed write,
outside the action. From rc.8 the runtime refuses it in dev and skips the
drain in production.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const save = action(function* (text: string) {
  setDraft(text);
  flush(); // throws FLUSH_IN_ACTION in dev
  yield api.save(text);
});

const submit = action(async function* (form: Form) {
  flush(() => setStatus("sending")); // throws: still the first step
  const id = await api.submit(form);
  yield;
  setStatus(id);
});
```

Examples of **correct** code for this rule:

```tsx
const save = action(function* (text: string) {
  setDraft(text); // held by the action, committed when it settles
  yield api.save(text);
});

async function onClick() {
  await save("hello");
  flush(); // outside the action: drains as usual
}
```

## How to fix

Remove the `flush()`. The action's writes commit when it settles; to observe
the result, read after the action resolves (`await save(); …`).

## When it does not fire

- **Releases without the throw.** The rule reads the resolved
  `@solidjs/signals` (the one the installed `solid-js` resolves). rc.0-rc.7 have
  no `actionStepDepth` at all and drain normally (the rc.1-rc.8 release review
  § 3, probe R), so on them — including the audited rc.3 — the rule is silent.
  A signals release no review read, or one that does not resolve, is not
  claimed to throw (its `SC9014` notice says so).
- **Nested functions.** A `flush` in function layers that run in place in a
  step is claimed (ADR 0478): an IIFE, an inline standard-library callback
  (`[x].forEach`), and a helper (in any module) that calls its callback in its
  own body, nested or not; the layers' outermost call is the position the step
  proof reads. A callback of a primitive that runs in place and lets a throw out
  (`untrack`, `latest`, `createRoot`, `runWithOwner`) is a layer too (ADR 0944);
  `isPending` catches the throw and `flush(fn)` throws before it runs its
  callback, so neither is crossed. Not claimed: a named closure called in
  place, a call after an await in its layer, a layer in a parameter list, and
  any other function written in the body, although the probes show the
  synchronous ones throw. Uncertifiable (ADRs 0537, 0538): a named function
  run at an inline carrier (`[1].forEach(f)`) or called from a step, one
  handed to a project helper, written as a member's value or an object slot,
  a method of a class written in the body, and a chain of in-place layers
  past the climb's bound of 16, re-asked from where it stopped. Also
  uncertifiable (ADR 0708, blocker `function-run-unplaced`): a `flush` in a
  function no call edge places (an object method, slot or getter, a class
  member, a `Map` or array slot, a tag, `toString`, a function handed where
  nothing follows it), or in a named helper one of those calls, when the
  project names `action`. Chrome throws for each when an action's body
  reaches it; module-level members are no longer silent.
  A closure written as an argument of a project call in a step (a chain of
  forwarding helpers, `hc1(() => flush())`) is uncertifiable too, with the
  same blocker (ADR 0832): no layer follows the forward.
- **A generator no walk places (ADR 0961).** A `flush` written in a generator, or in a helper a generator calls, is an
  uncertifiable row (`function-run-unplaced`) when the project, or a package analyzed as source, names `action`:
  a generator literal handed to a helper that makes the action (a project helper or a dependency), a `yield*` into
  another generator, a generator a handler steps. Chrome throws FLUSH_IN_ACTION for each when it is an action's step;
  the exact stepper is not followed. A generator that is the action's own argument keeps the walk's proof (a
  violation); a program that names no `action` has no such site.
- **A callback with a parameter or an async callback of a primitive that runs it in place (ADR 0959).** The climb crosses
  it as an obligation (`untrack((p0) => flush())`, `latest(async () => { flush(); })` in a step): a parameter the primitive
  may never pass, and an async callback's code before its first await, are shapes no proof reads, so the row is an
  obligation, never a violation.
- **After a suspension.** In an async generator, a call that any `await`,
  `for await`, `yield*` or `await using` of the body can run before — or that
  a loop can reach again after one — is not claimed. A later `yield` re-enters
  a step, so some of those calls do throw; the proof does not follow the
  generator's control flow far enough to say which.
- **Arguments that are not the generator.** `action(wrap(function* () { … }))`
  steps whatever `wrap` returns, so no violation is claimed inside; the flush is an uncertifiable row (ADR 0961: a
  generator handed on that no walk places), since `wrap` may return the generator itself.

- **A generator whose iterator the program keeps (ADR 0961, R1).** A named generator's call whose result a statement does
  not discard may be stepped anywhere (`Array.from(g())`, `new Set(g())`, `g().next()`, `const it = g(); it.next()`, a
  loop over a stored iterator, `yield*`): a `flush` in its body is an uncertifiable row (`function-run-unplaced`) when the
  program names `action`, never silence. Chrome throws FLUSH_IN_ACTION at each when the generator is stepped in a step. A
  call a statement discards runs none of the body, and a generator nothing calls or steps has no site.
- **A parameter initializer (ADR 0961, PD).** A `flush` written in a default value, a destructuring default, an IIFE or a
  closure in one, or in a helper the default calls, of a function the program calls or references
  (`function h(x = flush())` and `action(function* () { h(); })`) runs when a call omits the argument, which no walk
  reads: an uncertifiable row (`parameter-initializer-unplaced`), never a violation. The default of the action's own step
  generator is evaluated when the action creates the generator, before the first step (ADR 0454): no row. A method's
  default is the same row.

## Accounting (ADR 0966)

Every flush the flush census lists ends, for this code, in a row of this rule at the site, an SC9012, the compiler's discard, or a typed reason (the program names no primitive that could put the flush where this code applies, or the flush's caller is on an empty stack: a scheduled callback, code past an `await`, module evaluation, a test runner's callback written directly, or a site nothing enters). A flush none of those ends gets an uncertifiable `SC2006` row, `flush-in-action-unaccounted`: the walks that place a flush also answer "placed off" for a function no call edge reaches, a body that may be a component and an owner set around the call, none of which is a proof, so those sites stay uncertifiable (never a violation). The program-wide reason (the program names no primitive that could put the flush where this code applies) is not a proof for a flush in a function an installed package the build does not read as source may run: the package calls the primitive itself, on a function the project hands it (`runStep(() => { flush(); })`), and nothing in the build's table shows it. Such a flush gets no program-wide reason and keeps its `SC2006` row, `flush-in-action-unaccounted`: a function written as an argument or inside one (an object's member, an array's element), a reference to it through any copy, assignment or import, a JSX prop, spread or child of a component the program does not read, a project function's parameter that goes on to one of these, a function a project function returns (`pkg.run(make())`), a reflective call that forwards its arguments (`pkg.run.call(null, cb)`, `Reflect.apply`, `bind`), and a helper the handed callback calls (ADR 0967). A callee is known, and so exempt, by its resolved declaration: a dialect primitive, a reviewed test runner's registration, the standard library, a project function, a DOM element, or a package the build reads as source; `expect`, `vi.*` and `waitFor` are not exempt. The default is the flip's (the review of #216): a value counts as handed unless a fact of the program proves where it goes, so a function kept in a collection or an iterable's element, an assignment into an object a package exports or `this`, a class's functions, a project component's props that go on to the package, `<Dynamic component={…}/>` of a package's component, a function a handed function returns, a callback run inline by known code (the standard library, a project parameter, a dialect primitive) on the stack of a step a package runs, a reflective call whose target is a project function that goes on, a namespace that escapes whole and an export of an open program keep the row; a value that is only called, compared, registered with the host (`addEventListener`), assigned to a local element or handed to a project function that does none of these is not handed. A program that installs fake timers (`vi.useFakeTimers()`, `jest.useFakeTimers()`: a test advances the clock on the stack it is on, which may be an action's step) has no scheduled-callback reason either: a flush written in the callback of `setTimeout`, `queueMicrotask` or `Promise.then` keeps its row of each code (F1 of the review of #213, ADR 0967; the clock is global to the run, so the question is the program's). A test runner's callback, the code past an `await` and module evaluation are unchanged.

## Related

- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the other
  action guard
- [no-await-primitive-in-tracked-scope](no-await-primitive-in-tracked-scope.md) — a release-keyed dev
  throw in the same family

ADR 0967, review causes A–E: containment alone does not place a callback. A receiver inside an initializer, an iterable, a template part or an operand of `yield` or `await` stays handed; a project function naming `arguments` hands every argument, and a callback's returned value follows its receiving call whether it is given literally or by an exact current named reference (an unresolved member remains uncertifiable). An intrinsic element's attributes other than compiler-recorded event handlers, its children and its spreads (also `Dynamic` with a string tag) run on the creating stack. Opening and closing JSX names ask the same render owner, and code in an element's prop or child expression (a nested element, a call, an inline callback) runs where that element evaluates it: under a component nothing reads, when the package reads the prop; a project component's props and an object literal's getter run where they are read. An assignment expression's value is followed like any value. Unproved hand-offs retain the uncertifiable row.
