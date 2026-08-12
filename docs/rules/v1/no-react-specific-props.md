# v1/no-react-specific-props

`SC8011` · **warning** · violation

JSX uses React compatibility props where Solid has native spellings or no need
for the prop at all.

## What it does

Reports `className` in favor of `class`, `htmlFor` in favor of `for`, and `key`
on lowercase DOM, SVG, or custom elements. Solid's `<For>` and `<Index>` manage
list identity from their data; they do not consume React-style keys placed on
the rendered DOM node.

## Why is this bad?

The compatibility aliases obscure which semantics Solid actually implements
and may disappear from future releases. A DOM `key` is worse than redundant: it
is passed through as an inert attribute and can mislead readers into believing
it controls reconciliation.

## Examples

Incorrect:

```tsx
<label className="field" htmlFor="email">Email</label>
<For each={items()}>{(item) => <li key={item.id}>{item.name}</li>}</For>
```

Correct:

```tsx
<label class="field" for="email">Email</label>
<For each={items()}>{(item) => <li>{item.name}</li>}</For>
```

## How to fix

Rename `className` and `htmlFor`, and remove DOM `key`. The checker supplies safe
fixes, but withholds a rename if the destination prop already exists because
that would create a duplicate-property defect.

## Related

- [jsx-no-duplicate-props](jsx-no-duplicate-props.md) — conflicting prop names
