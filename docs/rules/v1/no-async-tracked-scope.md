# v1/no-async-tracked-scope

`SC5004` · **warning** · violation

An async function is passed as a tracked scope, but Solid tracks dependencies
only synchronously.

## What it does

Flags `async` functions passed to tracking positions — `createEffect`,
`createMemo`, and other computations. Part of the fine-grained decomposition of
eslint-plugin-solid's monolithic `reactivity` rule.

## Why is this bad?

A computation collects dependencies only until its first `await`: everything read
after that point registers nothing, so the computation silently stops responding
to those inputs. The scope's shape promises reactivity that the `async` keyword
takes away.

## Examples

Examples of **incorrect** code for this rule:

```tsx
createEffect(async () => {
  const data = await fetchData();
  // Tracking ended at the await: theme() is never a dependency.
  render(data, theme());
});
```

Examples of **correct** code for this rule:

```tsx
// Keep the tracked scope synchronous; let createResource own the async work.
const [data] = createResource(theme, async (currentTheme) => {
  return fetchData(currentTheme);
});

createEffect(() => {
  const value = data();
  if (value) render(value, theme());
});
```

## How to fix

Keep tracked scopes synchronous. Move async work into `createResource` — its
source function stays tracked and re-triggers the fetcher — and read the
resulting accessor from synchronous computations.

## Related

- [v1/reactive-read-after-await](./reactive-read-after-await.md) — the per-read variant
