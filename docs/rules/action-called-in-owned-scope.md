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
explicitly exempts leaf imperative scopes. An invocation no fact places (a
helper with no resolved caller, a JSX attribute function, code after an
`await`) is **uncertifiable**. A helper is resolved by its exact binding: a
declaration, or an arrow or function expression bound to a `const` identifier
(`const useMenu = () => ...`), is proven to run where its direct callers run.
A `let` nothing reassigns is exact too (ADR 0516); a written `let` and a
`var` are not, so a helper held in one stays uncertifiable.
An action call in a lazy memo (`createMemo(fn, { lazy: true })`) no proven
read of its accessor computes, or in code no execution reaches (ADRs 0458,
0578), is uncertifiable (ADR 0576).
An action call in a function no call edge reaches but that may run (called
from a getter, `toString`, an iterator, a tag, a `Map` slot, a method, a
constructor or an instance field) is uncertifiable too, never certified (ADR
0715): Chrome throws when a render runs it. Module initialization (outside
every function, a static block or field) has no owner and is silent.
An action handed to a project function calling that parameter in its body is
called at that call; U if only on some paths (ADR 0929).

A `createRoot` body runs during the call with the new root as the ambient
owner, and the action guard has no root exemption on any release: an action
called directly in the body, or in a memo compute nested in it, throws
`ACTION_CALLED_IN_OWNED_SCOPE` (probed on every published `2.0.0-rc.0` to
`rc.9` triple, dev client builds). A function passed to `createRoot` by name
is the root body too, resolved by symbol to its same-file declaration. A
closure bound to a name in a root body runs under the root where it is called,
even when a cleanup or a handler holds the root call (ADR 0471). An
effect apply, `onSettled` or an event listener inside the root keeps its own,
legal, answer.

`untrack` and `flush(fn)` are **not** escape hatches: each runs its callback
inline and keeps the owner context, so an action invoked inside `untrack(...)`
or `flush(() => ...)` within a memo, component body or root body still throws
`ACTION_CALLED_IN_OWNED_SCOPE` at runtime.

An action call that no row, discard or named silence ends is uncertifiable: `action-unaccounted` (ADR 0936).

An action called where it is created, with no binding (`action(function* () {})()`), is that action's call
and is placed as a bound one is (ADR 0972).

A call into code of an installed package the build does not read as source may call `action` inside the
package, where the program shows none (ADR 0972). Each such call, `new`, tagged template, render of the
package's component and function handed on as a value (an import binding passed as an argument, a
`{ shorthand }` property, a namespace member) is a site: where its role may run under an owner or nothing
places it, it is **uncertifiable** (`package-may-call-action:<why>`), never a violation, unless the package's
source, walked from every entry through every import, `require` and literal `import()`, names no Solid
package (a `@solidjs/*` package counts) and loads nothing by a computed name, generates no code (`eval`,
`Function`, a string timer) and reads no global the pinned TypeScript's `lib.es*`/`lib.dom*` files do not
declare. Missing source, a native module, an unparseable file and the WASM `checkSync` entry (no filesystem)
stay uncertifiable. A package the build reads as source is analyzed, and a row in its code stands at the
project's entry (ADR 0949); the dialect's own runtime, the standard library, a reviewed test runner's
registration, a global a project declaration file declares and a function the project itself defines are no
sites.

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

- [no-write-in-pure-computation](no-write-in-pure-computation.md) — the same constraint for setters
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — reactive data loading

Generators are placed as for [no-write-in-pure-computation](no-write-in-pure-computation.md) (ADRs 0851, 0853).

## Endings by code (ADR 0968)

An action call is accounted for, for this rule's Chrome code, only by an ending that answers that code: a row of its own table, an SC9012, the compiler's discard, a finding of a rule the dialect says claims the code, or a named silence that names it. A row of another table or a finding of another rule at the same place answers another code and ends nothing here, so this rule's own uncertifiable row (`action-unaccounted`) stands beside it: never a violation, and it yields to any row of the rule that stands. A site whose rule claims no code here, or whose code has no policy in the dialect, is not ended by what stands at it.
