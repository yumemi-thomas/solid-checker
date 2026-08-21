# v1/prefer-show

`SC8015` · **warning** · violation

This preference is enabled by default. Opt out with an explicit rule disable.

Expensive JSX content is controlled by a reactive `&&` or `?:` condition
instead of Solid's `<Show>` component.

## What it does

Checks logical-and and conditional expressions used directly as JSX children.
At least one rendered branch must be an element, fragment, or bare identifier;
small scalar expressions and conditions inside JSX attributes are left alone.
The left operand of `&&`, or the test of `?:`, must itself contain a proven
signal/accessor, memo, store-path, derived-helper, or package-contract read.

Static locals, literals, once-captured values, unknown calls, and reactive
reads confined to a branch remain clean. The 1.x catalog's historical
all-props-reactive compatibility model is not sufficient proof for this style
preference: an unenumerated or exported component prop remains clean unless a
separate exact reactive read establishes the condition.

## Why is this bad?

`<Show>` makes the condition and fallback explicit and gives conditional DOM a
stable Solid control-flow boundary. Solid's compiler already handles the
reported JavaScript forms correctly, so this is deliberately a warning and a
readability preference, not a runtime-correctness error.

## Examples

Incorrect:

```tsx
<main>{ready() && <Dashboard />}</main>
<main>{user() ? <Profile /> : <Login />}</main>
```

Correct:

```tsx
<main><Show when={ready()}><Dashboard /></Show></main>
<main><Show when={user()} fallback={<Login />}><Profile /></Show></main>
```

## How to fix

The ternary form has a safe `<Show fallback>` rewrite when an unshadowed runtime
import can be reused or added. The `&&` form is manual because falsy values such
as `0` render differently through JavaScript `&&` and `<Show>`, and Type Facts
do not yet prove a Boolean-only condition.

Native projects opt out with `"v1/prefer-show": { "enabled": false }` in
`.solid-checker/rule-options.json`. ESLint users set
`"solid-checker/v1/prefer-show": "off"` after the generated v1 config.

## Related

- [prefer-for](prefer-for.md) — explicit list control flow
