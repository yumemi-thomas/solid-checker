# v1/event-handlers

`SC8001` · **warning** · violation

Validates Solid JSX event-handler spelling and values from Oxc JSX facts, where
TypeScript does not.

## What it does

Three claims, with different scopes since the narrowing below.

- **Readability rename**, on any element: a *declared* handler written in its
  all-lowercase alias (`onclick`, `ondblclick`) should use the canonical
  camelCase spelling.
- **Static value**, and **ambiguous name**, on a hyphenated (custom-element)
  tag: a statically valued `on*` prop is frozen into the template as a plain
  attribute rather than attached as a listener, and an unrecognised `on*` name
  is ambiguous between a word beginning "on" and a misspelled handler.
- **`warnOnSpread`** (off by default), on any element: a handler-named property
  carried in through a JSX spread is not attached as a listener, because Solid
  attaches listeners from attributes its compiler can see.

For a value that is neither a directly written string nor an obviously static
string local, the static-value branch follows the pinned 1.x compiler: only a
`StringLiteral` or `NumericLiteral` expression is frozen into the template.
Thus `onClick={-1}` and `onClick={NaN}` are not treated as static merely because
TypeScript renders both as `number`; radix and separator numeric literal syntax
is still a numeric literal.

## Scope: standard elements are TypeScript's

Narrowed on 2026-08-17 under the absolute rule in
[AGENTS.md](../../../AGENTS.md): never report what TypeScript already reports.

Solid 1.x declares every standard handler under **both** its camelCase and its
all-lowercase spelling, and `HTMLAttributes` has no `on*` index signature — its
only index signature is `` [key: `-${string}`] ``. So on a standard element
TypeScript already answers two of the three arms, against the real
`solid-js@1.9.14` typings:

```
TS2322: Property 'onFoo' does not exist on type 'HTMLAttributes<HTMLDivElement>'.
TS2322: Property 'onClIcK' does not exist ... Did you mean 'onClick'?
TS2322: Type 'string' is not assignable to type 'EventHandlerUnion<…>'.
```

The unknown-name arm is covered in **every** value form, including the boolean
shorthand; and no static value is ever assignable to `EventHandlerUnion`, so the
static-value arm has no residue on a standard element either.

Three things are not covered, and the rule keeps all three:

- **A declared spelling written in lowercase.** `onclick` and `ondblclick`
  type-check, so the remaining objection is readability and it is this rule's.
  A mis-cased (`onClIcK`) or non-standard (`ondoubleclick`) name is *not*
  declared, is TS2322, and is no longer reported.
- **A hyphenated tag.** `<my-widget />` is TS2339 against stock typings, so any
  project that uses one has augmented `JSX.IntrinsicElements` with its own
  declaration — commonly a permissive one. There TypeScript is silent about the
  attributes and this rule's claims are the only ones available.
- **`warnOnSpread`.** `<div {...{ onClick: handler }} />` type-checks; that
  Solid does not attach the handler is a compiler-lowering fact.

Both directions are pinned by `fixtures/tsc-oracle/rule-cases.json` and
`fixtures/reactive-ir/eslint-compat`. The upstream cases this narrowing stops
firing for are declared `status: "policy"` in
`fixtures/upstream-parity/deviations.json`, each naming its diagnostic.

## Options

Configured in the project's `.solid-checker/rule-options.json` (see
[the rules index](../README.md#rule-options)):

```json
{
  "schemaVersion": 1,
  "rules": {
    "v1/event-handlers": { "ignoreCase": false, "warnOnSpread": false }
  }
}
```

- `ignoreCase` (default `false`) — accept handler names as written; the
  canonical-spelling and ambiguous-name advice is off.
- `warnOnSpread` (default `false`) — report handler-named properties carried
  into a DOM element through a JSX spread, which Solid does not attach as
  listeners.
