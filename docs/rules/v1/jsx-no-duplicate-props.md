# v1/jsx-no-duplicate-props

`SC8003` · **error** · violation

The same effective JSX property is written more than once, or an element uses
multiple competing sources of child content.

## What it does

Compares direct attributes with properties from inline object-literal spreads.
It applies Solid's prop normalization, so equivalent event spellings and
`attr:`/`prop:` aliases cannot evade the check. It also rejects combinations of
JSX children, a `children` prop, `innerHTML`, and `textContent`, because each
writes the element's content.

## Why is this bad?

JSX write order makes the last value win. Earlier values are dead, and changing
attribute order or spread contents can silently change which value reaches the
DOM. Duplicate `class` props are especially fragile; independent conditional
classes belong in `classList`.

## Examples

Incorrect:

```tsx
<button class="base" {...{ class: active() ? "active" : "" }} />
<div innerHTML={markup}>{fallback}</div>
```

Correct:

```tsx
<button class="base" classList={{ active: active() }} />
<div>{fallback}</div>
```

## How to fix

Remove the dead occurrence or combine the values into one prop. Choose exactly
one mechanism for element content. No automatic fix is offered because deciding
which value should survive is a semantic choice.

## Related

- [prefer-classlist](prefer-classlist.md) — conditional class composition
- [no-innerhtml](no-innerhtml.md) — unsafe markup injection
