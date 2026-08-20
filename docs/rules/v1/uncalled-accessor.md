# v1/uncalled-accessor

`SC1005` · **warning** · violation

## Scope: three of the six value positions are TypeScript's

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

The rule enumerated six value positions. Three are ones the type system closes,
in **both** dialects:

| Position | TypeScript |
| --- | --- |
| a native JSX attribute | TS2322 — an accessor is never assignable to a DOM attribute's type |
| a class object value | TS2322 against 2.0's `Record<string, boolean>` (the only dialect where the position was enabled) |
| a computed property access | TS2538 "cannot be used as an index type" |

Three are positions TypeScript **permits**, and they are the most common real
spellings of the bug, so the rule keeps them:

- **A string-concatenation binary operand.** `"hello " + label` type-checks,
  because `+` with a string operand accepts anything — and renders the
  accessor's own source text. The *arithmetic and bitwise* binary positions are
  separated out and stay silent: `count + 1` is TS2365, and `count - 1`,
  `count * 2`, and `count | 0` are TS2362, so they are TypeScript's. The fact
  that distinguishes them is the operand kind recorded on
  `AstFacts::coercive_operands`, which now carries only the coercions the type
  system accepts.
- **A unary operand.** `!count` is legal on any value and always truthy, and so
  are the unary numeric coercions `-count`, `+count`, and `~count` — probed
  against the published typings, TypeScript reports nothing for a function
  operand in any of the four, so an accessor coerced there (silently `NaN`) is
  this checker's to report.
- **A template-literal interpolation.** Stringifies whatever it is given.

Both directions are pinned by `fixtures/tsc-oracle/rule-cases.json` and
`fixtures/reactive-ir/uncalled-accessor-v2`. The upstream cases this narrowing
stops firing for are declared `status: "policy"` in
`fixtures/upstream-parity/deviations.json`.

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
- [v1/reactive-handler-frozen](./reactive-handler-frozen.md) — the inverse defect
