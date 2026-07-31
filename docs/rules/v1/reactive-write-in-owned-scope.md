# v1/reactive-write-in-owned-scope

`SC2001` · **error** · violation

A signal or store setter is called inside an owned scope — a component body or a
computation's tracking phase.

## What it does

Flags calls to setters returned by `createSignal`/`createStore` when they execute
inside a component body, a memo, or another tracked scope. Writes are allowed in
event handlers, `onMount`, and other callbacks that run outside the current
computation.

## Why is this bad?

Writing during the tracking phase creates feedback loops: the write invalidates
state the surrounding graph may depend on, which re-triggers the computation that
made the write and can loop the reactive graph. Solid 1.x tolerates these writes
at runtime, so the loop hides real bugs — and such a write is almost always a
derivation expressed imperatively.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [doubled, setDoubled] = createSignal(0);
// A derivation written imperatively — the write re-triggers the graph
// that produced it.
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

// One-time setup writes run after render, outside the tracking phase:
onMount(() => setReady(true));
```

## How to fix

First ask whether the write is a derivation in disguise — if the new value is
computed from other reactive values, replace compute-then-set with a `createMemo`.
Genuinely imperative writes move to an event handler, `onMount`, or another
callback that runs outside the current computation.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — the read-side counterpart
- [v1/no-owner-effect](./no-owner-effect.md) — effects created without an owner
