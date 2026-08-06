# v1/uncalled-accessor

`SC1005` · **warning** · violation

A signal or memo accessor is used as a value without being called.

## What it does

Flags proven accessor references in positions that necessarily consume a value:
an untagged template interpolation, a coercive binary or unary operator, a
computed property key, or a native JSX value attribute. Component props are not
included because Solid compiles them to lazy getters; function-valued native
bindings such as event handlers and `ref` are excluded too. Part of the
fine-grained decomposition of eslint-plugin-solid's monolithic `reactivity` rule;
untracked and after-await reads land on
[v1/strict-read-untracked](./strict-read-untracked.md) and
[v1/reactive-read-after-await](./reactive-read-after-await.md) instead.

## Why is this bad?

An accessor returned by `createSignal` or `createMemo` is a function; the *call*
is both the read and the subscription. Using the uncalled accessor in one of
these value positions operates on the function object rather than its current
value.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [count, setCount] = createSignal(0);
// Interpolates the function's source, not the number.
const label = `Count: ${count}`;

// Uses the function object as a property key.
const selected = values[count];

// Coerces the function object instead of the number.
const previous = count - 1;

// A native attribute receives the accessor function itself.
const input = <input value={count} />;
```

Examples of **correct** code for this rule:

```tsx
const [count, setCount] = createSignal(0);
const label = `Count: ${count()}`;
const selected = values[count()];
const previous = count() - 1;
const input = <input value={count()} />;

// Solid's JSX insertion contract calls function children reactively, so
// passing an accessor as a child is also correct.
return <span>{count}</span>;
```

## How to fix

Call the accessor at a value-consuming use: `` `${count()}` `` or
`values[count()]`. If you mean to pass the accessor itself—to a helper or Solid's
JSX child insertion—leave it uncalled.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — reads that happen outside tracking
- [v1/expected-function-got-expression](./expected-function-got-expression.md) — the inverse defect
