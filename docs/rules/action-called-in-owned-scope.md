# action-called-in-owned-scope

`SC2002` · **error** · violation

An `action` is invoked inside an owned scope — a component body, a
`createRoot` body, or a children-capable computation.

## What it does

Flags calls to functions created with `action()` when they execute under a live
children-capable owner: a component body, a `createRoot` body, a memo, or an
effect's compute function. Actions may be invoked from event handlers, effect apply callbacks,
and the children-forbidden leaf scopes `onSettled` and `createTrackedEffect` —
the runtime's action guard uses the same owner test as the write guard and
explicitly exempts leaf imperative scopes.

A `createRoot` body runs during the call with the new root as the ambient
owner, and the action guard has no root exemption on any release: an action
called directly in the body, or in a memo compute nested in it, throws
`ACTION_CALLED_IN_OWNED_SCOPE` (probed on every published `2.0.0-rc.0` to
`rc.9` triple, dev client builds). A function passed to `createRoot` by name
is the root body too, resolved by symbol to its same-file declaration. An
effect apply, `onSettled` or an event listener inside the root keeps its own,
legal, answer.

`untrack` and `flush(fn)` are **not** escape hatches: each runs its callback
inline and keeps the owner context, so an action invoked inside `untrack(...)`
or `flush(() => ...)` within a memo, component body or root body still throws
`ACTION_CALLED_IN_OWNED_SCOPE` at runtime.

## Why is this bad?

Invoking an action starts a write transaction: optimistic writes apply, async work
runs inside the transition, and `refresh` re-derives state when it settles. Started
under a children-capable owner, that transaction invalidates the very graph that is
being tracked — re-triggering the scope that called it, exactly the feedback loop
Solid 2.0 forbids for plain setters. Leaf scopes have no children to re-trigger,
which is why the runtime leaves them legal.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const save = action(function* (todo) {
  setOptimisticTodos((s) => {
    s.push(todo);
  });
  yield api.add(todo);
  refresh(todos);
});

function TodoList() {
  save(defaultTodo); // Called during component setup — starts a transaction under a live owner.
  return <For each={todos()}>{(todo) => <Row todo={todo} />}</For>;
}

// A root body runs under the root, a children-capable owner.
createRoot(() => save(defaultTodo));
```

Examples of **correct** code for this rule:

```tsx
function TodoList() {
  return (
    <>
      <button onClick={() => save(defaultTodo)}>Add</button>
      <For each={todos()}>{(todo) => <Row todo={todo} />}</For>
    </>
  );
}

// Leaf scopes are legal action sites — the guard exempts them:
createTrackedEffect(() => {
  if (shouldAutosave()) save(draft());
});
```

## How to fix

Call the action from an event handler, `onSettled`, `createTrackedEffect`, or
another imperative boundary; wrapping the call in `untrack` does not help.
If the goal is loading data reactively rather than mutating it, an action is the
wrong tool: return the Promise from a computation (`createMemo(() => fetchX())`)
and read it under a `<Loading>` boundary.

## Related

- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — the same constraint for setters
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — reactive data loading
