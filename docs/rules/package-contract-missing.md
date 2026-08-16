# package-contract-missing

`SC9005` · **error** · uncertifiable

An imported package integrates with Solid but has no reactivity contract.

## What it does

Flags imported packages whose own manifest declares a dependency on `solid-js` or
`@solidjs/*` but for which no contract could be found. Contracts are discovered in
this order:

1. explicit `--contract <PATH>` arguments,
2. a local override at `.solid-checker/contracts/<package>/solid-reactivity.json`,
3. `solid-reactivity.json` shipped in the package's own root,
4. the contracts bundled with solid-checker (`solid-js`, `@solidjs/web`).

General-purpose packages that do not depend on Solid are deliberately exempt — they
cannot participate in reactivity, so they need no contract.

**The bundled contracts are version-pinned.** Each bundled contract names the exact
audited release (`solid-js@2.0.0-rc.0`, `@solidjs/web@2.0.0-rc.0` for the Solid 2.0
dialect; `solid-js@1.9.14` for 1.x) and is matched by exact version equality against
the installed package's manifest. Installing **any other version — including a newer
RC** — silently skips the bundled contract, so `solid-js` itself becomes a package
with no contract and this rule fires, making the whole project uncertifiable. That is
deliberate: a new RC must be re-audited before the checker can vouch for it. If you
hit SC9005 on `solid-js` or `@solidjs/web` right after upgrading, this is why — pin
back to the audited version or provide a reviewed local contract override. The Solid
2 runtime closure is pinned too: `@solidjs/signals` is resolved and integrity-pinned
alongside `solid-js`, so package-manager drift beneath the same top-level RC also
fails contract conformance rather than being certified. See
[package-contracts.md](../package-contracts.md#bundled-contracts) for details.

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

See [package-contracts.md](../package-contracts.md) for the contract format and
authoring workflow.

## Related

- [package-contract-export-missing](package-contract-export-missing.md) — a contract exists but misses an export
