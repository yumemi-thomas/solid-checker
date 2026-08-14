# reactive-write-in-owned-scope

`SC2001` · **error** · violation

A signal or store setter (or `refresh()`) is called inside an owned scope — a
component body or a computation's tracking phase.

## What it does

Flags calls to setters returned by `createSignal`/`createStore` and to `refresh()`
when they execute inside a component body, a memo, or an effect's compute function.
Writes are allowed in event handlers, actions, `onSettled`, effect apply callbacks,
directive apply callbacks, and `untrack` blocks. Internal reactive sources created
with `{ ownedWrite: true }` in their source-creation options are exempt.

This is the static counterpart of Solid's dev-mode `REACTIVE_WRITE_IN_OWNED_SCOPE`
error.

## Why is this bad?

Writing during the tracking phase creates feedback loops: the write invalidates
state the surrounding graph may depend on, which re-runs the scope that performed
the write. Solid 2.0 makes this a dev-mode error because such loops are almost
always a derivation expressed imperatively — and the 1.x behavior of silently
tolerating them hid real bugs.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [doubled, setDoubled] = createSignal(0);
// A derivation written imperatively — throws in dev.
createMemo(() => setDoubled(count() * 2));

function Counter() {
  setCount(0); // Write in a component body.
  return <span>{count()}</span>;
}
```

Examples of **correct** code for this rule:

```tsx
// Derive instead of writing back:
const doubled = createMemo(() => count() * 2);

// Imperative writes belong in imperative scopes:
<button onClick={() => setCount((c) => c + 1)}>+1</button>;

// Internal reactive sources that must be written from owned scope opt in narrowly:
const [element, setElement] = createSignal(null, { ownedWrite: true });
```

## How to fix

First ask whether the write is a derivation in disguise — if the new value is
computed from other reactive values, replace compute-then-set with a `createMemo`.
Genuinely imperative writes move to an event handler, an `action`, `onSettled`, or
the apply function of `createEffect(compute, apply)`. Reserve the source creation
option `{ ownedWrite: true }` for internal reactive sources such as element refs;
using it on application state reintroduces the feedback loops this rule prevents.

## Related

- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the same constraint for actions
- [invalid-refresh-target](invalid-refresh-target.md) — what `refresh()` accepts
