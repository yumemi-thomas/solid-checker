# prefer-show

`SC8015` · **warning** · violation

This preference is opt-in through the `preferences` preset or an explicit rule
enable. It does not participate in default certification.

Expensive JSX content is controlled by a reactive `&&` or `?:` condition
instead of Solid's `<Show>` component.

## What it does

Checks logical-and and conditional expressions used directly as JSX children.
At least one branch must be an element, fragment, or bare identifier. Scalar
expressions and conditions inside attributes stay clean. The left operand of
`&&`, or the test of `?:`, must itself contain a proven accessor/memo, store
path, derived or package-contract read, or caller-proven reactive-prop read at
that JSX position.

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

The ternary form has a safe `<Show fallback>` rewrite when the checker can reuse
or add an unshadowed runtime import. The `&&` form has no automatic fix: for
example, `0 && child` renders `0`, while `<Show when={0}>` renders nothing, and
the available Type Facts do not yet prove a Boolean-only condition.

Native projects opt in with `--preset preferences`,
`--enable-rule prefer-show`, or `"prefer-show": { "enabled": true }` in
`.solid-checker/rule-options.json`. ESLint users enable the rule or compose the
generated `preferences-v2` config.

## Related

- [prefer-for](prefer-for.md) — explicit list control flow
