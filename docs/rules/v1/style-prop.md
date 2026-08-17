# v1/style-prop

`SC8017` · **warning** · violation

Validates Solid's JSX `style` property representation, and its CSS property
names where TypeScript does not.

## What it does

Two claims, with different scopes since the narrowing below.

**The string form**, on every element: a string-valued `style` is replaced
wholesale on every update, where an object lets Solid patch individual
properties in place. Two of this arm's claims are ones no type can make at all —
a declaration with a missing value (`"font-size: 10px; missing-value: ;"`) and a
value that is not CSS.

**Object keys**, on a component, or for a `-`-prefixed key on any element: a
camelCase key is not a CSS property, an unknown key is a typo, and a unitless
number for a length is passed to the DOM as-is.

## Scope: object keys are TypeScript's on an intrinsic element

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

`JSX.IntrinsicElements` types a `style` object as `csstype`'s `CSSProperties`,
and TypeScript's excess-property check has exactly the same subject this rule
inspects — a fresh object literal written in place. So against the real
`solid-js@1.9.14` typings:

```
TS2561: 'maxWidth' does not exist in type 'CSSProperties'. Did you mean to write 'max-width'?
TS2353: 'unknownStyleProp' does not exist in type 'CSSProperties'.
TS2322: Type '-10' is not assignable to type 'MarginTop<0 | (string & {})>'.
```

Two shapes are not covered, and the rule keeps both:

- **A `-`-prefixed key on any element.** `CSSProperties` carries
  `` [key: `-${string}`]: string | number | undefined ``, so the index signature
  absorbs a vendor-prefixed key whatever it is spelled —
  `-webkitAlignContent`, `-webkit-align-content`, and `-fooBar` are all silent
  to `tsc`. This is upstream's own case 02, and gating the rule on
  component-ness alone would have dropped it.
- **Any key on a component.** Its props are whatever it declares, so
  `<Panel style={{ fontSize: 10 }} />` is a permitted key when `Panel` takes
  `Record<string, unknown>` and a type error when it declares
  `JSX.CSSProperties`. Where TypeScript is silent, the key is still wrong the
  moment the component forwards it to the DOM.

`--` custom properties remain CSS's own escape hatch and are never reported, and
only a *direct* object literal is inspected — an object built by a helper is left
alone.

Both directions are pinned by `fixtures/tsc-oracle/rule-cases.json` and
`fixtures/reactive-ir/upstream-divergences/style-cases.tsx`. The upstream cases
this narrowing stops firing for are declared `status: "policy"` in
`fixtures/upstream-parity/deviations.json`, each naming its diagnostic.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/style-prop": { "styleProps": ["style"], "allowString": false }
  }
}
```

- `styleProps` (default `["style"]`) — the prop names the rule inspects.
  Naming props replaces the default, so `["css"]` leaves `style` alone.
- `allowString` (default `false`) — accept string-valued style props instead
  of asking for an object.
