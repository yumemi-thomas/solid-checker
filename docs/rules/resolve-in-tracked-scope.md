# resolve-in-tracked-scope

`SC2004` · **error** · violation

`resolve(fn)` is called inside a tracked scope — a memo or effect compute, a
`createTrackedEffect` callback, or tracked JSX.

## What it does

Flags calls to `resolve` (imported from `solid-js` / `@solidjs/signals`) whose
call site provably runs under an active observer: directly inside a tracked,
non-deferred callback (`createMemo`'s compute, `createEffect` /
`createRenderEffect`'s compute function, `createTrackedEffect`, a boundary
body) or inside a compiler-proven tracked JSX region.

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

## Why is this bad?

`resolve(fn)` reads the expression once and settles a `Promise`; it
subscribes to nothing. Inside a computation that is exactly backwards — the
computation wants tracked reads that re-run when inputs change. The runtime
throws in dev to force the redesign, and in production the computation keeps
running with a value frozen at first resolution.

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
```

## How to fix

Inside a computation, read the accessor directly — that is what tracked reads
are for. Keep `resolve()` for imperative code: event handlers, `onSettled`,
an effect's apply function, tests. A deliberate one-shot read inside a
computation can be wrapped in `untrack()`, which clears the observer the
runtime guards on (runtime-legal; the value still will not update).

## When it does not fire

- **`untrack` callbacks** — probed legal; the guard keys on tracking, and
  `untrack` clears it (unlike the write guard, which keys on the owner — see
  [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md)).
- **Component bodies, event handlers, effect apply callbacks, `createRoot`
  bodies, module scope** — probed: no observer is active in any of them.
- **Helpers.** The proof is lexical: a `resolve()` inside a named helper that
  a memo happens to call is not claimed (the helper may equally be called
  from an event handler).
- **`onSettled` / leaf owners.** `resolve()` passes the observer guard there,
  so this rule is silent — but note that in an *owner-backed* `onSettled` the
  promise still rejects in dev with `PRIMITIVE_IN_FORBIDDEN_SCOPE` (probed):
  `resolve` creates a `createRoot` internally, and leaf owners forbid
  primitive creation. That failure belongs to the leaf-owner defect class
  ([leaf-owner-forbidden-call](leaf-owner-forbidden-call.md)), which does not
  currently model `resolve`'s internal root; the rejection is asynchronous
  (inside the returned promise), unlike the synchronous throws that rule
  describes.
- The rule belongs to the 2.0 catalog only: Solid 1.x does not export
  `resolve`.

## Related

- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — the
  owner-keyed counterpart for writes; note the inverted `untrack` semantics
- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the same
  family for actions
- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — how
  pending values are meant to be read
