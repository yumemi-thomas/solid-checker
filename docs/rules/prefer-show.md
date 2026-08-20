# prefer-show

`SC8015` · **warning** · violation

Expensive JSX content is conditionally rendered with `&&` or `?:` instead of
Solid's `<Show>` component.

## What it does

Checks logical-and and conditional expressions used directly as JSX children.
At least one branch must be an element, fragment, or bare identifier. Scalar
expressions and conditions inside attributes stay clean.

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

## Related

- [prefer-for](prefer-for.md) — explicit list control flow
