# uncalled-accessor

`SC1005` · **warning** · violation

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
  and is not flagged. The 1.x catalog keeps its upstream-parity behavior in
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
- [expected-function-got-expression](expected-function-got-expression.md) — the inverse defect
