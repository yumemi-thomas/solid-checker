# no-await-primitive-in-tracked-scope

`SC2004` (`resolve`) · `SC2005` (`until`) · **error** · violation

`resolve(fn)` or `until(fn)` is called inside a tracked scope: a memo or effect
compute, a `createTrackedEffect` callback, or tracked JSX. One rule, one
configurable name (ADR 0426); a finding carries the code of the primitive it
names. Formerly `resolve-in-tracked-scope` (`SC2004`), `resolve-in-reactive-scope`
and `until-in-tracked-scope` (`SC2005`). Each old name is an alias limited to its own code, in `rule-options.json` and as an ESLint key: disabling `resolve-in-tracked-scope` leaves `SC2005` on, and the new name governs both.

## What it does

Flags calls to `resolve` (imported from `solid-js` / `@solidjs/signals`) and to
`until` (imported from `solid-js`, exported from `2.0.0-rc.5` on; see "When it
does not fire") whose call site provably runs under an active observer:
directly inside a tracked, non-deferred callback (`createMemo`'s compute,
`createEffect` / `createRenderEffect`'s compute function,
`createTrackedEffect`, a boundary body) or inside a compiler-proven tracked JSX
region. Both primitives open with the same guard, `getObserver()`, so one scope
proof serves both.

### `resolve` (`SC2004`)

**Premise: probe-confirmed** on the `@solidjs/signals@2.0.0-rc.0` dev bundle
(`dev.js:4738`), whose guard is exactly `if (getObserver()) throw new
Error("Cannot call resolve inside a reactive scope; it only resolves the
current value and does not track updates.")`. Probed results:

| call site | dev behavior |
| --- | --- |
| `createMemo` compute | **throws** |
| `createEffect` compute | **throws** |
| `createTrackedEffect` callback | **throws** |
| `untrack(...)` — even inside a memo | allowed (observer cleared) |
| component body (`solid-js` rc.0 dev `createComponent`) | allowed (`getObserver()` is `null` there) |
| event handler / plain call | allowed |
| effect *apply* callback | allowed |
| `createRoot` body | allowed |
| `onSettled` callback | passes this guard (see below) |

The guard is **dev-only**: the production bundle
(`dist/prod/signals.js`) has no observer check, so the same call silently
resolves a one-shot value that never tracks — the classic stale-data hazard.
The catalog mirrors the dev throw as an error, like the other tracked/owned
scope throws.

**Doc divergence, runtime wins:** RFC 05 says `resolve` "cannot be called
inside a reactive scope". The runtime enforces something narrower — an active
*observer* — so `untrack` callbacks and component bodies (both inside what
the docs might call a reactive scope) are runtime-legal and this rule leaves
them alone.

### `until` (`SC2005`)

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
`resolve`'s. The guard is **dev-only**: `dist/prod/signals.js:530` has none. In
production each run of the scope calls `until` again, and each call builds a new
root and user effect that wait on the predicate.

## Why is this bad?

`resolve(fn)` reads the expression once and settles a `Promise`; it
subscribes to nothing. `until(fn)` is the imperative "wait until the world
confirms this" primitive: it returns a promise that settles on the first truthy
evaluation of its predicate. Inside a computation both are exactly backwards —
the computation wants tracked reads that re-run when inputs change, and a
promise it creates is not something it can wait on. The runtime throws in dev
to force the redesign, and in production the computation keeps running with a
value frozen at first resolution (`resolve`) or builds a fresh waiting root on
every run (`until`).

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

const label = createMemo(() => {
  // Throws in dev: an observer (the memo) is active.
  return resolve(() => user());
});

createEffect(
  () => resolve(() => user()), // throws in dev
  value => console.log(value)
);

const saved = createMemo(() => {
  void until(() => ready()); // throws in dev
  return ready();
});
```

Examples of **correct** code for this rule:

```tsx
// Tracked scopes read the accessor directly — pending reads suspend and the
// computation re-runs when the value settles:
const label = createMemo(() => user().name);

// Imperative code awaits the settled value:
async function onExport() {
  const current = await resolve(() => user());
  download(current);
}

// An action step, as rc.9 documents `until`:
const send = action(function* (text: string) {
  yield;
  yield until(() => confirmed(), { timeout: 10_000 });
});
```

## How to fix

Inside a computation, read the accessor or the condition directly — that is
what tracked reads are for. Keep `resolve()` and `until()` for imperative code:
event handlers, an action step, `onSettled`, an effect's apply function, tests.
A deliberate one-shot `resolve` inside a computation can be wrapped in
`untrack()`, which clears the observer the runtime guards on (runtime-legal;
the value still will not update).

## When it does not fire

- **`untrack` callbacks** — probed legal; the guard keys on tracking, and
  `untrack` clears it (unlike the write guard, which keys on the owner — see
  [no-write-in-pure-computation](no-write-in-pure-computation.md)).
- **Component bodies, event handlers, effect apply callbacks, `createRoot`
  bodies, module scope** — probed: no observer is active in any of them.
- **Helpers.** A call in function layers that run in place inside a tracked
  callback (an IIFE, a project helper's callback, a function bound to a name
  whose only run is its one call there; not in a parameter list, not after an
  `await`) runs with the observer active, so it is a violation (ADR 0494). A
  helper that may also be called from an event handler is not such a layer.
  Where nothing places the call at all, the finding is **uncertifiable**.
- **`onSettled` / leaf owners.** `resolve()` passes the observer guard there,
  so this rule is silent — but note that in an *owner-backed* `onSettled` the
  promise still rejects in dev with `PRIMITIVE_IN_FORBIDDEN_SCOPE` (probed):
  `resolve` creates a `createRoot` internally, and leaf owners forbid
  primitive creation. That failure belongs to the leaf-owner defect class
  ([leaf-owner-forbidden-call](leaf-owner-forbidden-call.md)), which does not
  currently model `resolve`'s internal root; the rejection is asynchronous
  (inside the returned promise), unlike the synchronous throws that rule
  describes.
- **Before rc.5** (`until` only). `until` arrives in
  `@solidjs/signals@2.0.0-rc.5`, and the `solid-js` root re-exports it from the
  same release. On an installation where either is older, or either is a release
  nobody compared, `until` is not a name the vocabulary knows, so the rule
  cannot fire: on rc.3 and rc.4 the import is TS2305 and neither runtime has the
  export (the rc.1-rc.8 release review § 6 measured the rule firing there
  before this gate; `fixtures/reactive-ir/release-triple-until-rc3` pins it
  silent). A fresh install of `solid-js@2.0.0-rc.3` over signals rc.9 is the
  same case, because rc.3's root does not re-export `until`.
- The rule belongs to the 2.0 catalog only: Solid 1.x does not export
  `resolve`.

## Related

- [no-write-in-pure-computation](no-write-in-pure-computation.md) — the
  owner-keyed counterpart for writes; note the inverted `untrack` semantics
- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the same
  family for actions
- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — how
  pending values are meant to be read
