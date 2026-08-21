# uncalled-accessor

`SC1005` · **warning** · violation

## Scope: three of the six value positions are TypeScript's

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../AGENTS.md): never report what TypeScript already reports.

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
the product-owned cases in `fixtures/ownership-cases/cases.json`.

A signal or memo accessor is used as a value without being called.

## What it does

Flags proven accessor references in positions that necessarily consume a value:
an untagged template interpolation, a coercive binary or unary operator, a
computed property key, a native JSX value attribute, or — in the 2.0 catalog —
an object value inside a `class={...}` attribute. Component props are not
included because Solid compiles them to lazy getters; function-valued native
bindings such as event handlers, `ref`, and (in the 2.0 catalog) the
`children` attribute are excluded too. Shared with the 1.x catalog as
[v1/uncalled-accessor](v1/uncalled-accessor.md) under the same code, so a
suppression comment survives a migration.

Two positions are decided per dialect against the pinned
`@solidjs/web@2.0.0-rc.0` runtime:

- **`class` object values (2.0, flagged — probe-confirmed).** The object and
  array forms of `class` coerce each property value by truthiness (RFC 07);
  probed: `ssrClassName({ active: () => false })` renders `"active"`, and the
  client's `className` applies the same `!!value[key]` after
  `classListToObject`. An uncalled accessor there is a function object —
  always truthy — so the class is permanently on and never updates.
- **`children` attribute on a native element (2.0, exempt — code-read).**
  The children slot goes through child insertion, which *invokes*
  zero-argument functions: `insert()` wraps a function value in an effect and
  calls it (`dist/dev.js`), and the spread path normalizes through
  `flatten()`, which unwraps by calling. `<div children={count} />` is
  therefore live, called usage — the same contract as function JSX children —
  and is not flagged. The 1.x catalog keeps that behavior in
  both positions.

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

// A class-object value is truthiness-coerced: the function object is always
// truthy, so "active" can never turn off (2.0 catalog).
const row = <div class={{ active: isActive }} />;
const entry = <div class={["base", { selected: isSelected }]} />;
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

// The children *attribute* on a native element rides the same insertion
// contract and is called reactively (2.0 catalog).
return <div children={count} />;

// Call the accessor to make the class reactive.
return <div class={{ active: isActive() }} />;
```

## How to fix

Call the accessor at a value-consuming use: `` `${count()}` `` or
`values[count()]`. If you mean to pass the accessor itself—to a helper or Solid's
JSX child insertion—leave it uncalled.

## Related

- [strict-read-untracked](strict-read-untracked.md) — reads that happen outside tracking
- [reactive-handler-frozen](reactive-handler-frozen.md) — the inverse defect
