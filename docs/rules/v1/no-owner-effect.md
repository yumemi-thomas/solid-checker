# v1/no-owner-effect

`SC4001` · **warning** · violation (uncertifiable, reported as an error, when ownership or runtime entry is unproven)

An effect is created without a reactive owner, so nothing will ever dispose it.

## What it does

Flags effect creations (`createEffect`, `createRenderEffect`, and friends) that no
component, computation, or root owner dominates — module scope, bare helpers, and
detached callbacks.

When the unowned creation sits in an exported function whose call sites are outside
the project, the finding is reported as **uncertifiable** instead: solid-checker
cannot prove callers provide an owner. Like the SC9xxx rules, the uncertifiable form
carries **error** severity; the catalog's **warning** applies to the proven
violation form.

Calls covered by `"use server"` are also uncertifiable when they would allocate
on the client. The directive is a framework convention, not proof of a server
transform: Solid 1.x's client entry allocates the computation, while its server
entry is a no-op.

An uppercase function name is likewise not owner proof. Without a JSX call
site or an exact `Component` type, the function may execute as a component
under an owner or as an ordinary helper without one; an effect inside it is
therefore **uncertifiable**, with component identity named as the missing fact.

## Why is this bad?

Owners are Solid's disposal mechanism: when a component or root is disposed, every
computation it owns is torn down with it. An effect created with no owner is
immortal — it keeps re-running and holding its subscriptions for the lifetime of
the app, a leak that grows with every call of the creating function.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// Module scope: no owner will ever dispose this effect.
createEffect(() => applyTheme(theme()));
```

Examples of **correct** code for this rule:

```tsx
// Inside a component, the component owns and disposes the effect.
function ThemeProvider(props) {
  createEffect(() => applyTheme(theme()));
  return props.children;
}

// Deliberate module-scope reactivity keeps an explicit root and its dispose handle.
const dispose = createRoot((dispose) => {
  createEffect(() => applyTheme(theme()));
  return dispose;
});
```

## How to fix

Create effects inside a component or computation so their owner disposes them. For
deliberate module-scope reactivity, wrap the setup in `createRoot(dispose => ...)`
and keep the dispose handle — a 1.x root is detached from any surrounding owner,
so nothing ever calls `dispose` for you.

For exported library functions, document the ownership expectation in the package's
reactivity contract so consumers' analyses can certify calls.

## Related

- [v1/no-owner-cleanup](./no-owner-cleanup.md), [v1/no-owner-boundary](./no-owner-boundary.md) — the same problem for cleanup and boundaries
- [v1/package-contract-missing](./package-contract-missing.md) — reactivity contracts
