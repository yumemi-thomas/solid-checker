# prefer-for

`SC8014` · **error** · violation

This preference is disabled by default. Enable the `preferences` preset or
enable this rule explicitly.

A reactively updating list is rendered with `Array#map` directly as JSX
children instead of using Solid 2.0's list control flow.

## What it does

Reports a one-callback `.map()` only when the call itself occupies a JSX child
expression and evaluating its receiver there performs a proven reactive read.
Exact signal/accessor and memo calls, store paths, and caller-proven reactive
prop reads qualify. Maps assigned to variables or used in attributes stay
clean, as do static arrays and values captured once from a reactive source.

## Why is this bad?

A raw mapped array recreates its output when the surrounding expression runs.
`<For>` makes list identity explicit. Solid 2.0 removed `<Index>`; use
`<For keyed={false}>` for index-stable rendering.

The preference is intentionally silent when source provenance is unresolved.
An unknown call or generic `.map` member is not evidence of reactive updates,
and reactive reads inside the callback do not make a static receiver reactive.

## Examples

Incorrect:

```tsx
<ul>{items().map((item) => <li>{item.name}</li>)}</ul>
```

Correct:

```tsx
<ul><For keyed={false} each={items()}>{(item) => <li>{item.name}</li>}</For></ul>
```

## How to fix

The checker offers a safe `<For keyed={false}>` rewrite only when TypeScript
proves the receiver is array-like and the callback has exactly one non-rest
parameter.

Native checks accept `--preset preferences` or `--enable-rule prefer-for`.
ESLint users can compose `solidChecker.configs["preferences-v2"]`.

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
