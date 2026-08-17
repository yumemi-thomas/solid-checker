# v1/no-react-specific-props

`SC8011` · **warning** · violation

A component is passed a React compatibility prop where Solid has a native
spelling.

## What it does

Reports `className` in favor of `class`, and `htmlFor` in favor of `for`, **on a
component** — a JSX element whose tag is not lowercase-led.

## Scope: components only

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

On an intrinsic element these names are not in `JSX.IntrinsicElements`, so
TypeScript already rejects them against the real `solid-js@1.9.14` typings:

```
TS2322: Type '{ className: string; }' is not assignable to type
  'LabelHTMLAttributes<HTMLLabelElement>'.
    Property 'className' does not exist on type 'LabelHTMLAttributes<HTMLLabelElement>'.
```

`htmlFor` and `key` behave identically, so the `key` arm — which was gated to
lowercase-led elements and had no other domain — was removed entirely.

A component is the case no type answers. Its props are whatever it declares, so
`<Panel className="x" />` is a permitted key when `Panel` takes
`Record<string, unknown>` and a type error when it declares
`{ class?: string }`. Where TypeScript is silent, this rule makes the migration
claim: Solid forwards `class`, not `className`, so a component written for
Solid will not see the prop it expects.

Both directions are pinned by `fixtures/tsc-oracle/rule-cases.json` and enforced
by `scripts/tsc-oracle-gate.mjs`, so neither the dropped arm nor the surviving
one can move without failing CI. The upstream cases this narrowing stops firing
for are declared `status: "policy"` in
`fixtures/upstream-parity/deviations.json`.

## Why is this bad?

The compatibility aliases obscure which semantics Solid actually implements and
may disappear from future releases. On a component the mistake is also silent at
runtime: nothing renames the prop on the way in, so a component reading
`props.class` sees `undefined`.

## Examples

Incorrect:

```tsx
<Panel className="field" />
<Field htmlFor="email" />
```

Correct:

```tsx
<Panel class="field" />
<Field for="email" />
```

Not this rule's business — TypeScript reports both:

```tsx
<label className="field" htmlFor="email">Email</label>
<li key={item.id} />
```

## How to fix

Rename `className` to `class` and `htmlFor` to `for`. The checker supplies safe
fixes, but withholds a rename if the destination prop already exists, because
that would create a duplicate-property defect.

## Related

- [jsx-no-duplicate-props](jsx-no-duplicate-props.md) — conflicting prop names
