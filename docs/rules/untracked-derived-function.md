# untracked-derived-function

`SC1006` · **warning** · violation

A function that reads reactive values is only ever called from untracked
positions, so the reactivity it contains is ignored.

## What it does

Flags derived functions — functions whose body reads signals, store paths, or
props — whose every use is provably untracked. Shared with the 1.x catalog as
[v1/untracked-derived-function](v1/untracked-derived-function.md) under the
same code, so a suppression comment survives a migration.

Deliberately narrow, because it proves a negative: the rule fires only when
the function is bound inside another function and every reference to it is a
direct call in that function's own body, outside JSX. A function that is
passed as an argument, returned, called from a nested callback, or rendered
anywhere is left alone rather than guessed about — any of those may hand it
to a tracking scope the analysis cannot enumerate.

## Why is this bad?

Wrapping reactive reads in a function defers them, but deferral only helps if the
function is eventually called somewhere that tracks. A derived function that only
runs in untracked positions reads its inputs once per call and never subscribes —
the derivation looks reactive and is not.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [count] = createSignal(0);
const doubled = () => count() * 2;
// Called once during setup: the derivation never updates anything.
console.log(doubled());
```

Examples of **correct** code for this rule:

```tsx
const [count] = createSignal(0);
const doubled = () => count() * 2;
// Called from JSX, the derived function's reads are tracked.
return <span>{doubled()}</span>;
```

## How to fix

Call the derived function from a tracking scope — JSX, a `createMemo`, or the
compute function of `createEffect(compute, apply)` — or from an event handler
if a fresh value at event time is all you need.

## Related

- [strict-read-untracked](strict-read-untracked.md) — direct untracked reads
