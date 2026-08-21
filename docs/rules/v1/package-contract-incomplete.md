# v1/package-contract-incomplete

`SC9005` · **error** · uncertifiable

An external package used by the Solid 1.x analysis has no exact, reviewed
reactivity contract, or its contract omits an imported export or callback
execution fact. The checker reports the missing evidence instead of assuming
that the package is transparent or owner-preserving.

## What it does

The message distinguishes a wholly missing/stale contract, a missing or
environment-dependent export summary, and an external callback whose timing,
tracking, or ownership mode is unspecified. Contracts are tied to the exact
installed package version; evidence for another release cannot certify it.

Unknown callback diagnostics provide an editable schema-v1 contract stub. A
project-owned contract overrides package and bundled evidence, allowing an
application to audit a dependency even when its maintainer does not publish
one.

## How to fix

Generate and review a contract for the installed artifact:

```sh
solid-checker contract generate \
  --package-root node_modules/example \
  --output .solid-checker/contracts/example/solid-reactivity.json
```

Complete the relevant export or callback entry, verify it against the runtime
implementation, then validate the result. Library maintainers should ship a
reviewed `solid-reactivity.json` at the package root. Do not add blanket trust:
describe only the exact exports and execution behavior the artifact proves.

## Related

- [v1/reactive-source-uncaptured](./reactive-source-uncaptured.md) — an external call with an incomplete source summary
- [v1/reactive-dispatch-unresolved](./reactive-dispatch-unresolved.md) — unresolved project dispatch
- [Package contracts](../../package-contracts.md) — schema and evidence workflow
