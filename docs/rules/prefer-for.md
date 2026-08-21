# prefer-for

`SC8014` · **error** · violation

This preference is enabled by default. Opt out with an explicit rule disable.

A reactively updating list is rendered with `Array#map` directly as JSX
children instead of using Solid 2.0's list control flow.

## What it does

Reports a one-callback `.map()` only when the call itself occupies a JSX child
expression and evaluating its receiver there performs a proven reactive read.
Exact signal/accessor and memo calls, store paths, derived helper summaries,
contract-derived reads, and caller-proven reactive prop or accessor-prop reads
qualify. Type Facts must also prove that the receiver is an array or tuple.
Resolved-call facts must also select the standard-library `map` declaration.
Maps assigned to variables or used in attributes stay clean, as do static
arrays, overridden or unresolved same-name methods, non-array `.map`
collections, and values captured once from a reactive source. Async callbacks
can report in Solid 2.0 because its published JSX types accept that result, but
they never receive the synchronous-callback autofix.

## Why is this bad?

A raw mapped array recreates its output when the surrounding expression runs.
`<For>` makes list identity explicit. Solid 2.0 removed `<Index>`. Its default
`<For>` callback receives the item value, matching an ordinary one-parameter
`Array#map` callback; `keyed={false}` instead receives an item accessor and is
not an automatic semantics-preserving rewrite.

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
<ul><For each={items()}>{(item) => <li>{item.name}</li>}</For></ul>
```

## How to fix

The checker offers a safe `<For>` rewrite only when TypeScript proves the
receiver is an array or tuple, the callback is a synchronous arrow with exactly
one non-rest parameter, and it can reuse or add an unshadowed runtime import.
Regular functions report without a fix because they can observe Array#map's
additional callback arguments through `arguments`.

Native projects opt out with `"prefer-for": { "enabled": false }` in
`.solid-checker/rule-options.json`. ESLint users set
`"solid-checker/prefer-for": "off"` after the generated dialect config.

## Related

- [prefer-show](prefer-show.md) — explicit conditional control flow
