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
| an `untrack(() => flush())` callback in the body | **throws** | runs |
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
- **Nested functions.** The proof is lexical: a `flush` inside a helper,
  an `untrack` callback, or any other function written in the body is not
  claimed, although the probes show the synchronous ones throw.
- **After a suspension.** In an async generator, a call that any `await`,
  `for await`, `yield*` or `await using` of the body can run before — or that
  a loop can reach again after one — is not claimed. A later `yield` re-enters
  a step, so some of those calls do throw; the proof does not follow the
  generator's control flow far enough to say which.
- **Arguments that are not the generator.** `action(wrap(function* () { … }))`
  steps whatever `wrap` returns, so nothing inside is claimed.

## Related

- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the other
  action guard
- [until-in-tracked-scope](until-in-tracked-scope.md) — a release-keyed dev
  throw in the same family
