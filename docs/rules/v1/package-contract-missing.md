# v1/package-contract-missing

`SC9005` · **error** · uncertifiable

An imported package integrates with Solid but has no reactivity contract.

## What it does

Flags imported packages whose own manifest declares a dependency on `solid-js` or
`@solidjs/*` but for which no contract could be found. Contracts are discovered in
this order:

1. explicit `--contract <PATH>` arguments,
2. a local override at `.solid-checker/contracts/<package>/solid-reactivity.json`,
3. `solid-reactivity.json` shipped in the package's own root,
4. the contract bundled with solid-checker (`solid-js`).

The bundled contract describes `solid-js@1.x` and covers the `solid-js`,
`solid-js/web` and `solid-js/store` subpaths together, because discovery resolves
every subpath to its package root.

General-purpose packages that do not depend on Solid are deliberately exempt — they
cannot participate in reactivity, so they need no contract.

## Why is this analysis-limiting?

A Solid-integrating package can read reactive values, take tracked callbacks, and
return accessors. Without a contract describing its exports, solid-checker cannot see
through any of them: every value that flows into or out of the package is a blind
spot, and each use becomes uncertifiable rather than certified or proven wrong.

## How to fix

Pick the tier that matches your situation:

- **You consume the package** — create a local contract at
  `.solid-checker/contracts/<package>/solid-reactivity.json` (the path the finding
  names), or pass one explicitly with `--contract <PATH>`.
- **You maintain the package** — ship `solid-reactivity.json` in the package root
  so every consumer gets it automatically.

See [package-contracts.md](../../package-contracts.md) for the contract format and
authoring workflow.

## Related

- [v1/package-contract-export-missing](./package-contract-export-missing.md) — a contract exists but misses an export
