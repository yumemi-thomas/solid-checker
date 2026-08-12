# v1/prefer-for

`SC8014` · **error** · violation

An `Array#map` call is rendered directly as JSX children instead of using
Solid's list control flow.

## What it does

Reports a `.map(function)` call only when the call itself occupies a JSX child
expression. Maps assigned to variables or used in attributes are outside the
rule. The callback may render JSX or text: the important fact is that the
resulting array becomes a rendered list.

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

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
