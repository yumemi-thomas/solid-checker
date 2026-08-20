# jsx-no-duplicate-props

`SC8003` · **error** · violation

An intrinsic element uses multiple competing sources of DOM child content.

## What it does

Reports combinations of JSX children, `children`, `innerHTML`, and
`textContent` when more than one mechanism writes the same intrinsic element's
content. The `children`-prop plus JSX-children pair alone is left to TS2710.

Solid 2.0 has no Solid 1.x lowercase `on*` or `attr:` alias domain, so this
rule deliberately does not port the 1.x compiler-slot folding arm. Component
props are ordinary object keys and remain outside the rule.

## Why is this bad?

The writes compete for one DOM content slot, so one source overwrites another
and the visible result depends on write order.

## Examples

```tsx
// reported
<div innerHTML={markup}>{fallback}</div>

// clean
<div>{fallback}</div>
<Widget innerHTML={markup}>{fallback}</Widget>
```

## How to fix

Choose exactly one content mechanism. No automatic fix is offered because the
checker cannot decide which source is authoritative.

## Related

- [v1/jsx-no-duplicate-props](v1/jsx-no-duplicate-props.md) — the 1.x rule,
  including compiler-slot folding
