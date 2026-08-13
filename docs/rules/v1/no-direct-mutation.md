# v1/no-direct-mutation

`SC2003` · **warning** · violation

A reactive variable is reassigned or mutated directly instead of through its
setter.

## What it does

Flags direct writes to reactive values — reassigning a signal accessor binding,
or mutating state a signal or store holds without going through `setSignal`/
`setStore`. Part of the fine-grained decomposition of eslint-plugin-solid's
monolithic `reactivity` rule.

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
