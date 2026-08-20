# prefer-show

`SC8015` · **warning** · violation

This preference is enabled by default. Disable it explicitly when compact
JavaScript control flow is preferred.

Expensive JSX content is controlled by a reactive `&&` or `?:` condition
instead of Solid's `<Show>` component.

## What it does

Checks logical-and and conditional expressions used directly as JSX children.
At least one branch must be an element, fragment, or bare identifier. Scalar
expressions and conditions inside attributes stay clean. The left operand of
`&&`, or the test of `?:`, must itself contain a proven accessor/memo, store
path, or caller-proven reactive-prop read at that JSX position.

Static and once-captured conditions remain clean. So do unknown calls and
conditions whose only reactive read occurs in the right, consequent, or
alternate branch; those reads do not make the governing test update.

## Why is this bad?

`<Show>` makes the condition and fallback explicit and gives conditional DOM a
stable control-flow boundary. This is a style preference: the compiler already
handles the reported JavaScript forms correctly.

## Examples

```tsx
// reported
<main>{ready() && <Dashboard />}</main>

// preferred
<main><Show when={ready()}><Dashboard /></Show></main>
```

## How to fix

Apply the safe `<Show>` rewrite or keep the compact conditional and disable the
preference. Disabling it does not weaken reactive correctness checks.

Native projects opt out with `"prefer-show": { "enabled": false }` in
`.solid-checker/rule-options.json`. ESLint users set
`"solid-checker/prefer-show": "off"`.

## Related

- [prefer-for](prefer-for.md) — explicit list control flow
