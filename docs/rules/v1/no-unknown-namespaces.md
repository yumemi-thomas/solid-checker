# v1/no-unknown-namespaces

`SC8012` · **error** · violation

Reports unknown JSX namespaces from Oxc JSX facts.

## Options

## Scope: components only

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

On an intrinsic element every namespaced prop this rule objects to is already a
type error against the real `solid-js@1.9.14` typings. Solid resolves its
namespaces through mapped types over user-augmentable interfaces (`Directives`,
`ExplicitProperties`, `ExplicitAttributes`, `ExplicitBoolAttributes`,
`CustomEvents`) plus individually declared `on:*` events, so an unrecognised
prefix has nothing to land on:

```
TS2322: Property 'model:value' does not exist on type 'InputHTMLAttributes<HTMLInputElement>'.
```

That covers the `style:`/`class:` steer too: neither prefix is declared at all,
so `<div class:active={true} />` is a type error regardless of the style
preference this rule was expressing. That is a genuine gap in Solid's published
typings — the 1.x compiler does support both — but the type error is already
speaking at that exact span, and compensating for the typings is not this
checker's job.

A **component** keeps the rule. Its props are a plain object, TypeScript is
silent, and the claim is one no type makes: the compiler special-cases
namespaces only on DOM elements it lowers directly, so `<Panel on:click={fn} />`
delivers an inert `"on:click"` key. Upstream's own cases 06 and 07 are exactly
this and still fire.

Both directions are pinned by `fixtures/tsc-oracle/rule-cases.json` and
`fixtures/reactive-ir/eslint-compat`. The upstream cases this narrowing stops
firing for are declared `status: "policy"` in
`fixtures/upstream-parity/deviations.json`.

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/no-unknown-namespaces": { "allowedNamespaces": [] }
  }
}
```

- `allowedNamespaces` (default `[]`) — extra namespace prefixes to accept on
  top of the dialect compiler's own vocabulary.
