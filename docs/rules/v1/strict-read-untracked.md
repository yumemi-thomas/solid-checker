# v1/strict-read-untracked

`SC1001` · **warning** · violation

A reactive value (signal accessor, store path, or component prop) is read in a scope
that does not track dependencies.

## What it does

Flags reads of reactive values that happen outside every tracking scope: at the top
level of a component body, inside a plain helper function called from an untracked
position, or through a chain of calls that ends in one of those places. The
finding's evidence trail shows where the value was declared and how the read
reached the untracked scope.

For an uppercase function with neither a JSX call site nor an exact
`Component` type, the execution model is unresolved. Reads that would be
untracked in component setup are reported as **uncertifiable**, because the
same function may instead be an ordinary helper invoked under tracking.

Solid 1.x context providers are an important runtime-defined case:
`createContext(...).Provider` reads its `value` getter inside `untrack`, so a
reactive read in that prop is untracked even though ordinary component props
are lazy. The checker proves this from the resolved `createContext` symbol
across direct imports, re-exports, and namespace imports; a component merely
named `Provider` does not receive context semantics. Function-valued context
values are stored, not invoked, so reads inside those function bodies are not
reported at the provider site.

## Why is this bad?

Solid's reactivity is dependency tracking: a read only subscribes when it happens
inside a tracking scope (JSX, a `createMemo`, or the callback of
`createEffect(fn)`). An untracked read sees the current value exactly once and
never re-runs — the UI silently shows stale data after the first update.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Profile(props) {
  // Top-level read: `name` is captured once and never updates.
  const name = props.name;
  return <h1>{name}</h1>;
}

function currentTheme() {
  return store.settings.theme;
}

function Header() {
  // The helper reads a store path, and the call happens outside every
  // tracking scope — the theme is captured once.
  const theme = currentTheme();
  return <header class={theme} />;
}
```

Examples of **correct** code for this rule:

```tsx
function Profile(props) {
  // The property access inside JSX is what tracks.
  return <h1>{props.name}</h1>;
}

function Header() {
  // Called inside JSX, the helper's read is tracked.
  return <header class={currentTheme()} />;
}

// Reads inside the callback of createEffect(fn) subscribe.
createEffect(() => {
  console.log(store.settings.theme);
});

// An intentional one-time snapshot, made explicit:
const initial = untrack(() => count());
```

## How to fix

Move the read into a tracking scope: JSX, a `createMemo`, or the callback of
`createEffect(fn)`. If a one-time snapshot is intended, wrap the read in
`untrack()` so the intent is visible to both readers and the analyzer.

For component props specifically, read `props.name` directly in JSX or derive it
with `createMemo(() => props.name)`. `untrack(() => props.name)` is only an
explicit one-time snapshot; it does not make the prop reactive.

## Related

- [v1/no-destructure](./no-destructure.md) — the destructuring special case
- [v1/reactive-read-after-await](./reactive-read-after-await.md) — reads that lose tracking at an `await`
