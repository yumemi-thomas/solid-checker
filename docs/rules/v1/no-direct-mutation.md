# v1/no-direct-mutation

`SC2003` · **warning** · violation

A reactive variable is reassigned or mutated directly instead of through its
setter.

## What it does

Flags direct writes to reactive values — reassigning a signal accessor binding,
or mutating state a signal or store holds without going through `setSignal`/
`setStore`. Part of the fine-grained decomposition of eslint-plugin-solid's
monolithic `reactivity` rule.

Read-modify-write spellings count as writes: `store.count += 1` and
`store.count++` both reach the readonly proxy with a value that is dropped.
Upstream reports the compound form (its props branch tests for an ESTree
`AssignmentExpression`, which covers every compound operator) and reports an
accessor binding's `++` through the write reference ESLint's scope analysis
records, but never sees a *member* `++`, which is an `UpdateExpression`. No
upstream test case covered either spelling, so the product-owned fixture pins
this additional evidence-backed behavior directly.

The warning tier deliberately mirrors that upstream rule's advisory policy.
It does not mean the write is speculative: every finding is a proven mutation
that bypasses the reactive setter.

## Why is this bad?

Solid updates the graph only through setters: the setter is what notifies
subscribers. A direct assignment or in-place mutation changes the underlying
value silently — no computation re-runs, no JSX updates, and later setter-driven
updates may clobber the change.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [user, setUser] = createSignal({ name: "Ada" });
// Mutates the held object without notifying anyone.
user().name = "Grace";
```

Examples of **correct** code for this rule:

```tsx
const [user, setUser] = createSignal({ name: "Ada" });
setUser({ ...user(), name: "Grace" });

// Or use a store for fine-grained nested updates:
const [profile, setProfile] = createStore({ name: "Ada" });
setProfile("name", "Grace");
```

## How to fix

Route every update through the setter returned by `createSignal` or
`createStore`. For nested state, prefer `createStore` and its path-based setter
over spreading whole objects through a signal.

## Related

- [v1/reactive-write-in-owned-scope](./reactive-write-in-owned-scope.md) — setter calls in the wrong scope
