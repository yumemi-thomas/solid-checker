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

One store write shape is exempt in 2.0: an assignment through the original
store proxy **lexically inside one of that store's own setter callbacks**.
The rc.0 runtime write-enables the store for the duration of its own setter's
draft callback, so `setStore(draft => { store.value = 7 })` commits exactly
like a draft write (verified against the published runtime). The exemption is
proven, not guessed: the callee must resolve to the setter destructured beside
that exact store. Writing through *another* store's proxy inside a setter is
still silently dropped at runtime and stays a finding, as does any write
outside a setter. 1.x setters never unlock the proxy, so the `v1/` twin has no
such exemption.

Read-modify-write spellings are writes in both dialects: `store.count += 1` and
`store.count++` reach the proxy with a value that is dropped, and both also read
the old value, so [strict-read-untracked](strict-read-untracked.md) reports the
read alongside this write. A plain `=` reads nothing and produces this finding
alone.

The warning tier mirrors eslint-plugin-solid's `reactivity` policy for this
defect and stays consistent across the two dialects. It does not mean the
write is speculative: every finding is a proven mutation that bypasses the
reactive setter.

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

const [profile] = createStore({ name: "Ada" });
const [, setOther] = createStore({ n: 0 });
// Another store's setter does not unlock this proxy; the write is dropped.
setOther(() => {
  profile.name = "Grace";
});
```

Examples of **correct** code for this rule:

```tsx
const [user, setUser] = createSignal({ name: "Ada" });
setUser({ ...user(), name: "Grace" });

// Or use a store and mutate the draft in its own setter:
const [profile, setProfile] = createStore({ name: "Ada" });
setProfile((draft) => {
  draft.name = "Grace";
});

// Inside the store's own setter the original proxy is write-enabled too:
setProfile(() => {
  profile.name = "Grace"; // commits — the store is in its Writing set
});
```

## How to fix

Route every update through the setter returned by `createSignal` or
`createStore` — for stores, mutate the draft (or the original proxy) inside the
store's own setter callback. For state a child needs to change, lift it to the
parent and pass a callback down instead of assigning to props.

## Related

- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — setter calls in the wrong scope
