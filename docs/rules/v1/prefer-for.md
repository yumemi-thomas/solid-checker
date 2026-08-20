# v1/prefer-for

`SC8014` · **error** · violation

This preference is disabled by default. Enable the `preferences` preset or
enable this rule explicitly.

A reactively updating list is rendered with `Array#map` directly as JSX
children instead of using Solid's list control flow.

## What it does

Reports a `.map(function)` call only when the call itself occupies a JSX child
expression and evaluating its receiver there performs a proven reactive read.
Exact signal/accessor and memo calls and store paths qualify. Maps assigned to
variables or used in attributes are outside the rule; static arrays, captured
snapshots, unknown calls, and generic `.map` members remain clean. A reactive
read inside the callback is not evidence that the receiver updates.

Solid 1.x's retained caller model does not certify individual prop getters.
Prop-only receivers therefore remain clean unless another fact domain proves a
reactive source in the receiver expression.

## Why is this bad?

Ordinary array mapping recreates the mapped output whenever the surrounding
expression runs. `<For>` preserves DOM identity by item, while `<Index>` keeps
positions stable and exposes each item reactively. Those primitives encode the
update behavior that a raw mapped array cannot express to Solid.

## Examples

Incorrect:

```tsx
<ul>{items().map((item) => <li>{item.name}</li>)}</ul>
```

Correct:

```tsx
<ul><For each={items()}>{(item) => <li>{item.name}</li>}</For></ul>
```

Use `<Index>` when stable positions matter and the item value should be an
accessor.

## How to fix

Choose `<For>` or `<Index>` based on identity semantics. A safe `<For>` fix is
offered only when TypeScript proves the receiver is array-like and the callback
has exactly one non-rest parameter; index-aware or ambiguous callbacks require a
human choice.

Native checks accept `--preset preferences` or
`--enable-rule v1/prefer-for`. ESLint users can compose
`solidChecker.configs["preferences-v1"]`.

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
