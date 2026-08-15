# prefer-component-syntax

`SC8018` · **warning** · violation

A lowercase local function returns JSX and is called as an ordinary expression
inside JSX.

## What it does

The rule requires JSX-producing UI units to use component syntax. It uses
resolved symbol identity, so a shadowed same-named function is not confused
with the JSX-returning declaration, and it ignores helpers that return data.

```tsx
// Incorrect: imperative call syntax hides that this is a UI component.
const renderHeader = () => <header>Header</header>;
const Panel = () => <article>{renderHeader()}</article>;

// Correct.
const Header = () => <header>Header</header>;
const Panel = () => <article><Header /></article>;
```

Besides making component boundaries explicit, component syntax preserves the
runtime and compiler conventions around props, ownership, and setup execution.

## Scope: convention, not proven misbehavior

Calling a lowercase JSX-returning function inside JSX **works at runtime** — the
call executes inside the enclosing tracked scope and renders; no Solid 2.0
diagnostic exists for it. This rule enforces a convention (component identity,
dev-tools naming, ownership granularity), unlike the checker's semantic rules,
which prove runtime misbehavior. Like every finding it participates in
`--certify`; a project that prefers render-helper calls should disable the rule
in `.solid-checker/rule-options.json` rather than suppress individual findings.
