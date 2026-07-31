# v1/uncalled-accessor

`SC1005` · **warning** · violation

A signal or memo accessor is used as a value without being called.

## What it does

Flags uses of a reactive accessor where the function object itself is rendered or
compared instead of the value it holds — `{count}` in JSX where `{count()}` was
meant. Part of the fine-grained decomposition of eslint-plugin-solid's monolithic
`reactivity` rule; untracked and after-await reads land on
[v1/strict-read-untracked](./strict-read-untracked.md) and
[v1/reactive-read-after-await](./reactive-read-after-await.md) instead.

## Why is this bad?

An accessor returned by `createSignal` or `createMemo` is a function; the *call*
is both the read and the subscription. Passing the uncalled accessor renders or
operates on the function object, which is almost never the intent.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [count, setCount] = createSignal(0);
// Renders the function object, not the number.
return <span>{count}</span>;
```

Examples of **correct** code for this rule:

```tsx
const [count, setCount] = createSignal(0);
// The call reads the value and subscribes.
return <span>{count()}</span>;
```

## How to fix

Call the accessor at the point of use: `count()`, not `count`. If you really mean
to pass the accessor itself — to a helper that will call it inside a tracking
scope — pass it to a position the analysis knows receives a function.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — reads that happen outside tracking
- [v1/expected-function-got-expression](./expected-function-got-expression.md) — the inverse defect
