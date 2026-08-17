# no-owner-effect

`SC4001` · **warning** · violation (uncertifiable, reported as an error, for exported functions)

An effect is created without a reactive owner, so nothing will ever dispose it.

## What it does

Flags effect creations (`createEffect`, `createRenderEffect`,
`createTrackedEffect`) that no component, computation, or root owner dominates —
module scope, bare helpers, and detached callbacks.

When the unowned creation sits in an exported function whose call sites are outside
the project, the finding is reported as **uncertifiable** instead: solid-checker
cannot prove callers provide an owner. Like the SC9xxx rules, the uncertifiable form
carries **error** severity; the catalog's **warning** applies to the proven
violation form.

For `runWithOwner`, a supplied owner is treated as definite only when TypeScript's
resolved type declaration is the active Solid dialect's exported `Owner` type.
Same-spelled user types and unresolved values remain conditional.

## Why is this bad?

Owners are Solid's disposal mechanism: when a component or root is disposed, every
computation it owns is torn down with it. An effect created with no owner is
immortal — it keeps re-running and holding its subscriptions for the lifetime of
the app, a leak that grows with every call of the creating function.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// Module scope: no owner will ever dispose this effect.
createEffect(
  () => theme(),
  (value) => applyTheme(value),
);
```

Examples of **correct** code for this rule:

```tsx
// Inside a component, the component owns and disposes the effect.
function ThemeProvider(props) {
  createEffect(
    () => theme(),
    (value) => applyTheme(value),
  );
  return props.children;
}

// Deliberate module-scope reactivity keeps an explicit root and its dispose handle.
const dispose = createRoot((dispose) => {
  createEffect(
    () => theme(),
    (value) => applyTheme(value),
  );
  return dispose;
});
```

## How to fix

Create effects inside a component or computation so their owner disposes them. For
deliberate module-scope reactivity, wrap the setup in `createRoot(dispose => ...)`
and keep the dispose handle — in Solid 2.0 a `createRoot` is owned by its creating
parent by default, so even explicit roots are disposed with their surroundings
unless you detach them with `runWithOwner(null, ...)`.

For exported library functions, document the ownership expectation in the package's
reactivity contract so consumers' analyses can certify calls.

## Related

- [no-owner-cleanup](no-owner-cleanup.md), [no-owner-boundary](no-owner-boundary.md) — the same problem for cleanup and boundaries
- [package-contract-missing](package-contract-missing.md) — reactivity contracts
