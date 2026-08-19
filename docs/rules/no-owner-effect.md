# no-owner-effect

`SC4001` · **warning** · violation (uncertifiable, reported as an error, when ownership or allocation is unproven)

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

Under a `"use server"` directive, a constructor that would allocate on the
client is reported as **uncertifiable**. No core Solid package reads that
directive — it is a framework and bundler convention — so source text cannot
prove whether the client or server entry executes. Solid's server entry emits
no lifecycle diagnostic and allocates no corresponding client computation;
the client entry does. Calls that allocate on neither possible path stay
silent.

A spread whose runtime tuple arity decides whether `createEffect` allocates is
also uncertifiable. A visible absent or nullish apply argument remains proven
to throw before allocation and stays silent for this ownership rule.

The same rule applies to a non-literal apply value. A compiler-proven callable
identifier reaches allocation and can produce a violation. `any`, a nullable
value hidden by `!`, or another value whose callability is unavailable produces
an uncertifiable obligation because it may hit the runtime's pre-allocation
nullish guard. This does not weaken ordinary typed callbacks.

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
