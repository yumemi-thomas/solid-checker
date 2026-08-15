# reactive-write-in-owned-scope

`SC2001` · **error** · violation

A signal or store setter (or `refresh()`) is called inside an owned scope — a
component body or a children-capable computation.

## What it does

Flags calls to setters returned by `createSignal`/`createStore` and to `refresh()`
when they execute under a live children-capable owner: a component body, a memo,
or an effect's compute function. Writes are allowed in event handlers, actions,
effect apply callbacks, directive apply callbacks, and the children-forbidden
leaf scopes `onSettled` and `createTrackedEffect` — the runtime's write guard
explicitly exempts leaf imperative scopes.

`untrack` is **not** an allowed write region. The `2.0.0-rc.0` guard keys on the
ambient *owner*, not on tracking: `untrack` clears the tracking listener but
keeps the owner context, so a write inside `untrack(...)` within a component
body, memo, or effect compute still throws `REACTIVE_WRITE_IN_OWNED_SCOPE` at
runtime, while the same `untrack` write inside an event handler is fine because
no owner is live there. (The upstream RFC text claims writes in `untrack` blocks
are allowed; the published rc.0 runtime contradicts it — solid-checker follows
the runtime and reports these writes.)

Internal reactive sources created with `{ ownedWrite: true }` in their
source-creation options are exempt.

This is the static counterpart of Solid's dev-mode `REACTIVE_WRITE_IN_OWNED_SCOPE`
error.

## Why is this bad?

Writing under a children-capable owner creates feedback loops: the write
invalidates state the surrounding graph may depend on, which re-runs the scope
that performed the write. Solid 2.0 makes this a dev-mode error because such
loops are almost always a derivation expressed imperatively — and the 1.x
behavior of silently tolerating them hid real bugs. Leaf scopes are exempt
because they own no children to re-trigger: `createTrackedEffect` and
owner-backed `onSettled` run after the graph settles and are the intended home
for imperative writes.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [doubled, setDoubled] = createSignal(0);
// A derivation written imperatively — throws in dev.
createMemo(() => setDoubled(count() * 2));

function Counter() {
  setCount(0); // Write in a component body.
  // untrack does not help: the owner context survives it, and this
  // still throws REACTIVE_WRITE_IN_OWNED_SCOPE at runtime.
  untrack(() => setCount(0));
  return <span>{count()}</span>;
}
```

Examples of **correct** code for this rule:

```tsx
// Derive instead of writing back:
const doubled = createMemo(() => count() * 2);

// Imperative writes belong in imperative scopes:
<button onClick={() => setCount((c) => c + 1)}>+1</button>;

// Leaf scopes are legal write regions — the guard exempts them:
createTrackedEffect(() => {
  setLastSeen(count());
});
onSettled(() => setReady(true));

// Internal reactive sources that must be written from owned scope opt in narrowly:
const [element, setElement] = createSignal(null, { ownedWrite: true });
```

## How to fix

First ask whether the write is a derivation in disguise — if the new value is
computed from other reactive values, replace compute-then-set with a `createMemo`.
Genuinely imperative writes move to an event handler, an `action`, the apply
function of `createEffect(compute, apply)`, or a leaf scope (`onSettled`,
`createTrackedEffect`). Wrapping the write in `untrack` does not fix it — the
guard keys on the owner, not on tracking. Reserve the source creation option
`{ ownedWrite: true }` for internal reactive sources such as element refs;
using it on application state reintroduces the feedback loops this rule prevents.

## Related

- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the same constraint for actions
- [invalid-refresh-target](invalid-refresh-target.md) — what `refresh()` accepts
