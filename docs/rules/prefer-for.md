# prefer-for

`SC8014` · **error** · violation

An `Array#map` call is rendered directly as JSX children instead of using
Solid 2.0's list control flow.

## What it does

Reports a one-callback `.map()` only when the call itself occupies a JSX child
expression. Maps assigned to variables or used in attributes stay clean.

## Why is this bad?

A raw mapped array recreates its output when the surrounding expression runs.
`<For>` makes list identity explicit. Solid 2.0 removed `<Index>`; use
`<For keyed={false}>` for index-stable rendering.

## Examples

Incorrect:

```tsx
<ul>{items().map((item) => <li>{item.name}</li>)}</ul>
```

Correct:

```tsx
<ul><For keyed={false} each={items()}>{(item) => <li>{item.name}</li>}</For></ul>
```

## How to fix

The checker offers a safe `<For keyed={false}>` rewrite only when TypeScript
proves the receiver is array-like and the callback has exactly one non-rest
parameter.

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
