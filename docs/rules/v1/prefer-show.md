# v1/prefer-show

`SC8015` · **warning** · violation

This preference is disabled by default. Enable the `preferences` preset or
enable this rule explicitly.

Expensive JSX content is conditionally rendered with `&&` or `?:` instead of
Solid's `<Show>` component.

## What it does

Checks logical-and and conditional expressions used directly as JSX children.
At least one rendered branch must be an element, fragment, or bare identifier;
small scalar expressions and conditions inside JSX attributes are left alone.
Both the no-fallback and fallback forms have safe structural rewrites.

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

Apply the safe `<Show>` rewrite or make the same transformation manually. Keep
the original JavaScript conditional when its compact form communicates the
intent better; this rule can be disabled project-wide without weakening the
checker’s reactive correctness guarantees.

Native checks accept `--preset preferences` or
`--enable-rule v1/prefer-show`. ESLint users can compose
`solidChecker.configs["preferences-v1"]`.

## Related

- [prefer-for](prefer-for.md) — explicit list control flow
