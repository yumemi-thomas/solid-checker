# no-direct-mutation

`SC2003` · **warning** · violation

A reactive variable is reassigned or mutated directly instead of through its
setter.

## What it does

Flags direct writes to reactive values — reassigning a signal accessor binding,
or writing through a proven signal, store, or props proxy without going through
its setter. Shared with the 1.x catalog as
[v1/no-direct-mutation](v1/no-direct-mutation.md) under the same code, so a
suppression comment survives a migration.

## Why is this bad?

Solid updates the graph only through setters: the setter is what notifies
subscribers. A direct assignment or in-place mutation changes the underlying
value silently — no computation re-runs, no JSX updates, and later setter-driven
updates may clobber the change. Props are readonly by design: the parent owns
the value.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [user, setUser] = createSignal({ name: "Ada" });
// Mutates the held object without notifying anyone.
user().name = "Grace";

function Title(props) {
  // Writes through the readonly props proxy; the write is dropped.
  props.text = "untitled";
  return <h1>{props.text}</h1>;
}
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
`createStore`. For state a child needs to change, lift it to the parent and
pass a callback down instead of assigning to props.

## Related

- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — setter calls in the wrong scope
