# v1/prefer-for

`SC8014` · **error** · violation

This preference is opt-in through the `preferences` preset or an explicit rule
enable. It does not participate in default certification.

A reactively updating list is rendered with `Array#map` directly as JSX
children instead of using Solid's list control flow.

## What it does

Reports a `.map(function)` call only when the call itself occupies a JSX child
expression and evaluating its receiver there performs a proven reactive read.
Exact signal/accessor and memo calls, store paths, derived helper summaries,
and package-contract reads qualify. Type Facts must also prove that the
receiver is an array or tuple, and resolved-call facts must select the
standard-library `map` declaration. Maps assigned to variables or used in
attributes are outside the rule; static arrays, captured snapshots, unknown
or overridden same-name calls, non-array `.map` members, and uncertain prop
backing remain clean. A reactive read inside the callback is not evidence that
the receiver updates.

## Why is this bad?

Ordinary array mapping recreates the mapped output whenever the surrounding
expression runs. `<For>` preserves DOM identity by item, while `<Index>` keeps
positions stable and exposes each item reactively. Those primitives encode the
update behavior that a raw mapped array cannot express to Solid.

## Examples

Incorrect:

```tsx
<ul>{items().map((item) => <li>{item.name}</li>)}</ul>
```

Correct:

```tsx
<ul><For each={items()}>{(item) => <li>{item.name}</li>}</For></ul>
```

Use `<Index>` when stable positions matter and the item value should be an
accessor.

## How to fix

Choose `<For>` or `<Index>` based on identity semantics. A safe `<For>` fix is
offered only when TypeScript proves the receiver is an array or tuple, the
callback is a synchronous arrow with exactly one non-rest parameter, and an
unshadowed runtime import can be reused or added. Regular functions can observe
Array#map's additional callback arguments, so they report without a fix.
Index-aware, async, or ambiguous callbacks require a human choice.

Native projects opt in with `--preset preferences`,
`--enable-rule v1/prefer-for`, or
`"v1/prefer-for": { "enabled": true }` in
`.solid-checker/rule-options.json`. ESLint users enable the rule or compose the
generated `preferences-v1` config.

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
