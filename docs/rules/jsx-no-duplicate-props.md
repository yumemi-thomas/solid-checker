# jsx-no-duplicate-props

`SC8003` · **error** · violation

An intrinsic element uses multiple competing sources of DOM child content.

## What it does

Reports combinations of JSX children, `children`, `innerHTML`, and
`textContent` when more than one mechanism writes the same intrinsic element's
content. The `children`-prop plus JSX-children pair alone is left to TS2710.

Which source the DOM keeps, as observed in Chrome on Solid 2.0.0-rc.13
(`oracle-behaviour-jsx-no-duplicate-props`):

- JSX children beside `textContent`: the children win, and the `textContent` is lost.
- JSX children beside `innerHTML`: the `innerHTML` wins, and the children are lost.
- `innerHTML` beside `textContent`: the `innerHTML` wins, and the `textContent` is lost.
- A `children` prop beside `textContent`: the `textContent` wins, and the prop is lost.
- A `children` prop beside `innerHTML`: the `innerHTML` wins, and the prop is lost.
- A static `textContent` beside JSX children: the children win and the static text never shows. A write cannot reach a static source, so the oracle cannot confirm this case at runtime.

Solid 2.0 has no Solid 1.x lowercase `on*` or `attr:` alias domain, so this
rule deliberately does not port the 1.x compiler-slot folding arm. Component
props are ordinary object keys and remain outside the rule.

## Why is this bad?

The writes compete for one DOM content slot, so one source is silently lost. A
`textContent` beside JSX children is dropped by the compiler: the children always
win, and the `textContent` value never reaches the DOM. An `innerHTML` beside JSX
children wins: the element shows the `innerHTML` on first render and still after
the children update, so the children never show (observed on Solid 2.0.0-rc.13).

## Examples

```tsx
// reported
<div innerHTML={markup}>{fallback}</div>
<div textContent={label}>{fallback}</div>

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
