# primitive-in-effect-callback

`SC4005` · **error** · violation

A `createRoot`, `createMemo` or `createDeferred` call that, whenever it runs,
runs on the stack of a `createEffect` apply callback, which has no owner. On
`2.0.0-rc.17` the dev build throws `PRIMITIVE_IN_EFFECT_CALLBACK` before the
primitive is created; the production build creates a computation that belongs
to nobody and is never disposed.

## What it does

Flags calls to `createRoot`, `createMemo` and `createDeferred` (imported from
`solid-js`) that can only run synchronously inside the apply (second) argument
of a `createEffect(compute, apply)` call, in a client build, on an installation
whose `solid-js` and `@solidjs/signals` are both `2.0.0-rc.17`. The claim is
**universal**, as [flush-in-effect-callback](flush-in-effect-callback.md)'s:
whenever the call runs it runs in the effect phase, where the ambient owner is
none, so a computation created there belongs to nobody. A call in dead code
(after a `return`, under `if (false)`, in an effect never registered) is
reported too; reachability is no part of the proof.

**Premise: read from the published bytes and probed.** `runEffect` raises a
global marker around the apply (rc.17 `@solidjs/signals` `dist/dev.js:1994`,
cleared in its `finally` at `:2024`); it runs for the render and the user queue
alike (`dist/dev-shared.js:2150-2151`). A root is made by `createOwner`
(`:1496`), a memo or effect by `setupComputedNode` (`:4927`), and both call:

```js
function assertNotInEffectCallback() {
  if (inEffectCallback && context === null) {
    emitDiagnostic({ code: "PRIMITIVE_IN_EFFECT_CALLBACK", kind: "lifecycle", severity: "error", … });
    throw new Error(PRIMITIVE_IN_EFFECT_CALLBACK_MESSAGE);
  }
}
```

(`dist/dev-shared.js:3947-3957`). The check precedes the node's link and first
run, `lazy` or not, so the primitive is never created. `createRoot` is
`createOwner` and a `runWithOwner` (`:1537-1542`); `createMemo` and
`createDeferred` end in `computed` (`dist/dev.js:2433-2434`, `:8897-8898`;
`dist/dev-shared.js:4626-4689`). `solid-js`'s client wrappers, hydration
included, all call signals' functions (`dist/solid.dev.js:752-809`, `:884-885`,
`:962-988`). The production and observe builds have no such code (0 hits), so
they create the computation and nothing owns it. rc.13 has none in any build.

| position of the call | reports |
| --- | --- |
| directly in `createEffect`'s apply | **yes** |
| in a function only effects call (not exported, every reference a call from a proven layer), an immediately invoked function, an `untrack` callback, or an async helper before its first `await` | **yes** |
| `createRoot` or `createMemo` inside a callback of one of these | the outer call only (a root's callback never runs; a memo's runs under its own owner) |
| `runWithOwner(owner, fn)`, `createRoot` callback | no |
| `createEffect`'s compute function | no (an owner) |
| a `setTimeout`/microtask/`.then` callback created in the apply, an event handler, a helper that also has such a caller, after an `await` | no |
| `createRenderEffect`'s apply | no (first run is synchronous at creation, under the caller's owner; the rule cannot tell runs apart) |
| `createSignal`, `onCleanup` | no (inert, or SC4001's) |
| `createEffect`, `createRenderEffect`, `createTrackedEffect` | no, [missing-owner](missing-owner.md) |
| any of the above, when the program may flush from an owned scope | uncertifiable ([below](#when-the-program-may-flush-from-an-owned-scope)) |

### The split with `missing-owner`

The effect constructors reach the same assertion, but SC4001 already claims them
as the owner-requirement `Effect` role (`NO_OWNER_EFFECT` in an apply). Keeping
both would be two findings for one call, so SC4001 keeps `createEffect`,
`createRenderEffect` and `createTrackedEffect`, and this rule claims what SC4001
treats as legal or self-owning there: a root, a memo and a deferred memo.
On rc.17 dev the effect SC4001 reports also throws; its message does not say so.

The message says where the call sits: directly in the apply, in an inline
callback written in it (`[1].forEach(() => createMemo(…))`, one layer or
several), or in a function the apply calls (with the number of calls, and any
inline callbacks it is also reached through).

## Why is this bad?

The effect phase runs with no ambient owner, and the runtime says why it
refuses: "a computation created here belongs to nobody: it is never disposed
and never held". A root made there is a leaked root, a memo a leaked
computation, and in dev the create call throws, so the effect's apply errors.

## Examples

Examples of **incorrect** code for this rule:

```tsx
createEffect(count, () => {
  createRoot(() => mount()); // throws PRIMITIVE_IN_EFFECT_CALLBACK in dev
});

const label = () => createMemo(() => format(count()));
createEffect(count, () => label()); // the same call, one function away
```

Examples of **correct** code for this rule:

```tsx
function Panel() {
  const owner = getOwner();
  createEffect(count, () => {
    runWithOwner(owner, () => createMemo(() => format(count()))); // parented explicitly
  });
}

createEffect(count, () => {
  queueMicrotask(() => createRoot(() => mount())); // outside the effect phase
});
```

## How to fix

Create the primitive in the effect's compute function or in the component
body, which are owned, or parent it explicitly: capture the owner with
`getOwner()` outside the effect and create the primitive inside
`runWithOwner(owner, …)`.

## When it does not fire

- **Other releases.** The throw is in rc.17 only. On rc.13 and every earlier
  read release the same code runs there silently and nothing is reported (no
  assertion exists to mirror); a release no review read, a mixed install and an
  unresolved package state nothing (the dialect answers
  `throws_inside_effect_phase` for the rc.17 pair alone, and `SC9014` names the
  gap).
- **Anything that may run elsewhere or with an owner.** Each layer between the
  call and the apply must be proven to run only during it, with the ambient
  owner still none: a project function bound to a name nothing reassigns, not
  exported, with at least one call, whose every reference is an exact call that
  is itself such a layer; an immediately invoked function; an `untrack`
  callback; an audited inline standard-library callback (`forEach`, `map`; the
  climb is SC2008's, not pinned by this rule's fixture). A helper also called
  from an event handler, a timer or the module body, an alias, an argument, an
  export, a generator, a method, a parameter initializer and code after an
  `await` are not claimed. `runWithOwner` and `createRoot` callbacks are not
  climbed: the first installs an owner, the second's callback never runs.
- **Server builds** (`node`, `deno`, `worker`, `string-ssr`, `streaming-ssr`):
  `solid-js`'s server entry never runs an apply.
- **Two installed copies.** Both calls must import `solid-js` and load the same
  installed copy at runtime (the marker and the context are that copy's
  module-level state); a call imported through another package or a copy the
  backend cannot resolve is not claimed.
- **Positions it does not place.** An apply passed by name, the `{ effect,
  error }` bundle form, a returned cleanup (the previous cleanup also runs
  inside the marker), `createRenderEffect` in every run.
- **Primitives not claimed.** Only what the bytes above were read for:
  `createProjection`, `createStore(fn)`, `createOptimistic(fn)`, `mapArray` and
  `repeat` also reach `computed` or `createOwner` but take argument shapes this
  rule does not tell apart; nothing is claimed for them.

## When the program may flush from an owned scope

A program-wide check, as [missing-owner](missing-owner.md)'s event-dispatch
veto (ADR 0356): if some call may run `flush` synchronously from an owned
scope, every SC4005 candidate becomes an **uncertifiable** result, not a
violation. The calls that count:

- an exact call of `flush`; of web's `render` (it ends `createRoot(...)` with
  `flush()`: `@solidjs/web` rc.17 `dist/web.dev.js:567`, `dist/web.js:417`,
  rc.13 `dist/web.dev.js:504`) or `hydrate` (it reaches that `render`: rc.17
  `dist/web.dev.js:1644`, `:1732`); of `action(...)`, whose invocation flushes
  on the invoker's stack after a synchronous `yield` (`@solidjs/signals` rc.17
  `dist/dev.js:1455-1465`, `:1636`) and cannot be placed from the call that
  builds the wrapper, so that call counts wherever it is written;
- a call spelled `flush` (`flush()`, `x.flush()`) whose callee does not resolve
  exactly: an `any`-typed declaration, an import that does not resolve.

Exempt, because no owner is current: a statement at module level (the usual
`render(() => <App/>, root)` entry), and a call on a `createEffect` apply's
stack (the [flush-in-effect-callback](flush-in-effect-callback.md) placement;
only a `solid-js` import is placed there, so a `render` in an apply counts).
Anywhere else counts: a `createRoot` body, a component, a helper, a handler.
Every release answers alike (rc.13 and rc.17 were read; any other is taken to
reach `flush`). The check is one program-wide flag: the flush site is not
joined to the effects it drains.

## Premises

- **Universal claim.** Dead code is reported on purpose; there is no
  reachability or completion analysis.
- **The apply is ownerless, unless the program may flush from an owned scope.**
  `runEffect` raises the marker but never resets the context (`dist/dev.js:1990-2024`),
  and `flush` does not either (`dist/dev-shared.js:2129`, `:2646`), so a
  `flush()` that runs synchronously in an owned scope (a `createRoot` body, a
  component) drains the queued applies under that owner and the primitive is
  created owned, without the throw. The rule therefore withholds (see the next
  section) instead of assuming it away.
- **The dev throw is the usual symptom, the ownerless computation the claim.**
  The marker is one global boolean: a nested synchronous `runEffect` (a render
  effect created under `runWithOwner` earlier in the same apply) lowers it in
  its `finally` (`dist/dev.js:2024`) without restoring it, and a later primitive
  in that apply is created without the throw, still unowned. An action step
  clears and restores it (`dist/dev.js:1584-1600`).
- **No type expresses it.** `createRoot`, `createMemo` and `EffectFunction`
  accept the call, so `tsc` is silent (the oracle case `primitive-in-effect-callback`).

## Related

- [missing-owner](missing-owner.md): the effect constructors and `onCleanup` in
  an apply (`NO_OWNER_EFFECT`, `NO_OWNER_CLEANUP`)
- [flush-in-effect-callback](flush-in-effect-callback.md): the same stack, the
  `flush` no-op
- [primitive-in-directive-application](primitive-in-directive-application.md):
  the ref apply, which runs with no owner and does not throw
