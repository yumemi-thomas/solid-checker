# v1/reactive-write-in-owned-scope

`SC2001` · **error** · violation

A signal or store setter is called during a computation's tracking phase.

## What it does

Flags calls to setters returned by `createSignal`/`createStore` when they execute
inside a memo, effect computation, render effect, or tracked JSX expression.
One-shot component bodies, ordinary helpers, event handlers, and `onMount`
callbacks are outside this rule.

## Why is this bad?

Writing during the tracking phase creates feedback loops: the write invalidates
state the surrounding graph may depend on, which re-triggers the computation that
made the write and can loop the reactive graph. Solid 1.x has no component-body
write guard; SC2001 therefore reports only genuinely tracked execution, where a
write is usually a derivation expressed imperatively.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [doubled, setDoubled] = createSignal(0);
// A derivation written imperatively — the write re-triggers the graph
// that produced it.
createMemo(() => setDoubled(count() * 2));

function Counter() {
  return <span>{setCount(0), count()}</span>; // Tracked JSX expression.
}
```

Examples of **correct** code for this rule:

```tsx
// Derive instead of writing back:
const doubled = createMemo(() => count() * 2);

// Imperative writes belong in imperative scopes:
<button onClick={() => setCount((c) => c + 1)}>+1</button>;

// One-time setup writes run after render, outside the tracking phase:
onMount(() => setReady(true));

// Component setup and ordinary helpers also run once rather than tracking:
function Counter() {
  setCount(0);
  return <span>{count()}</span>;
}
```

## How to fix

First ask whether the write is a derivation in disguise — if the new value is
computed from other reactive values, replace compute-then-set with a `createMemo`.
Genuinely imperative writes move to an event handler, `onMount`, or another
callback that runs outside the current computation.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — the read-side counterpart
- [v1/missing-owner](./missing-owner.md) — effects created without an owner
