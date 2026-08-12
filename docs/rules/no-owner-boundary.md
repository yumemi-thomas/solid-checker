# no-owner-boundary

`SC4003` · **warning** · violation (uncertifiable, reported as an error, for exported functions)

A boundary (`Loading`, `Errored`) is created without a reactive owner, so it can
never be disposed.

## What it does

Flags boundary creations that no component, computation, or root owner dominates —
typically JSX built in module scope or in bare helper functions that are called
outside any component tree.

When the creation sits in an exported function whose call sites are outside the
project, the finding is reported as **uncertifiable** instead: solid-checker cannot
prove callers provide an owner. Like the SC9xxx rules, the uncertifiable form
carries **error** severity; the catalog's **warning** applies to the proven
violation form.

## Why is this bad?

Boundaries manage a subtree: they own the computations under them, catch their
suspensions or errors, and dispose the subtree when they are disposed themselves.
A boundary with no owner is never disposed, so the entire subtree it manages —
every computation and DOM node under it — leaks.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// Module scope: this boundary and everything under it can never be disposed.
const widget = (
  <Loading fallback={<Spinner />}>
    <Profile user={user()} />
  </Loading>
);
```

Examples of **correct** code for this rule:

```tsx
// Boundaries live inside a rooted component tree.
render(
  () => (
    <Loading fallback={<Spinner />}>
      <Profile user={user()} />
    </Loading>
  ),
  root,
);
```

## How to fix

Render boundaries inside a component tree rooted by `render()`/`hydrate()`, or
under an explicit `createRoot`. If a helper builds JSX with boundaries, call it
from a component so the surrounding owner adopts the subtree.

## Related

- [no-owner-effect](no-owner-effect.md) — unowned effects
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — where boundaries are needed
