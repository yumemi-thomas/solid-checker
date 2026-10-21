# jsx-no-duplicate-props

`SC8003` · **error** · violation

An intrinsic element uses multiple competing sources of DOM child content.

## What it does

Reports combinations of JSX children, `children`, `innerHTML`, `innerText`, and
`textContent` when more than one mechanism writes the same intrinsic element's
content. The `children`-prop plus JSX-children pair alone is left to TS2710.

Only the camel-case `innerHTML` is inner HTML. A lowercase `innerhtml` is not a
content source in Solid 2.0: TypeScript rejects it (TS2322), and the runtime
writes it as a plain attribute, so the rule stays silent on it. Upstream's
lowercase alias is deliberately not carried; the divergence is pinned in
`fixtures/ownership-cases/cases.json`.

`innerText` is a content source too. The rc.13 runtime lists it in its property
set (`ChildProperties`), so a write replaces the element's child nodes, the same
clash `textContent` has. Upstream's rule does not read `innerText`; this kept
divergence is pinned as `innertext/solid-v2/content-positive/001` in
`fixtures/ownership-cases/cases.json`. TypeScript accepts the prop, so the
finding is the checker's own claim.

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
<div innerText={label}>{fallback}</div>

// clean
<div>{fallback}</div>
<div innerText={label} />
<Widget innerHTML={markup}>{fallback}</Widget>
```

## How to fix

Choose exactly one content mechanism. No automatic fix is offered because the
checker cannot decide which source is authoritative.

## Related

- [v1/jsx-no-duplicate-props](v1/jsx-no-duplicate-props.md) — the 1.x rule,
  including compiler-slot folding
