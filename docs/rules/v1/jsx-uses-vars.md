# v1/jsx-uses-vars

`SC8006` · **error** · compatibility-only; emits no finding

This catalog entry preserves eslint-plugin-solid's rule identity, but the
checker never needs to report it.

## What it does

The upstream ESLint rule marks identifiers referenced by JSX as used so a
separate `no-unused-vars` pass does not mistake component tags for dead
bindings. solid-checker builds on TypeScript reference facts, where JSX tag and
expression references already point to their declarations. There is no missing
usage to repair and therefore no standalone SC8006 diagnostic.

## Example

```tsx
import { Card } from "./Card";

export function Page() {
  return <Card />;
}
```

`Card` is a normal TypeScript reference. No `v1/jsx-uses-vars` finding is
expected, and unused-variable diagnostics remain the responsibility of the
project's TypeScript or lint configuration.

## Why keep the rule?

Migration configurations often name every eslint-plugin-solid rule. Retaining
the exact `v1/jsx-uses-vars` identity means such configuration can be translated
or disabled without producing an unknown-rule error, while documenting that
silence is intentional rather than an implementation gap.

## Configuration

The rule accepts the shared `enabled` option, although toggling it cannot change
findings because none are emitted. It remains listed with upstream's catalog
severity solely for identity compatibility.

## Related

- [jsx-no-undef](jsx-no-undef.md) — JSX identifiers that truly cannot resolve
