# A consumer of a `plain` return

Pins ADR 0113's consumer arm: an accepted contract that closes `returns` over
one `return` whose output is `plain` leaves nothing open at the import, where the
same export with `returns` left open raises `SC9005`.

`App.tsx` calls two exports that are identical in everything but that claim:

| component | export | `returns` | verdict |
| --- | --- | --- | --- |
| `Status` | `isReady` | closed over one plain return | clean |
| `StatusOpen` | `isReadyOpen` | open | `SC9005` for `returns` |

The Rust consumer used to reopen the first. `project_return` turns every
operation output it has a return kind for into one, and `plain` has none, so a
closed claim over a single plain return projected to "no shape" and the domain
was marked open again. A plain value carries no reactive capability, which is
exactly what the consumer's own "no reactive return described" answer states,
so that is what it projects to now.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-merged-props-consumer` does and for the same reasons
(`docs/package-contract-v2/phase21/2026-09-15-an-accepted-contract-a-fixture-can-supply.md`).
`scripts/coverage.mjs` mints a policy-2 receipt over the document on a copy and
analyzes the copy with the trust configuration supplied out of band; nothing in
this directory can authorize itself. The document states its claims rather than
proving them, because what is under test is the consumer's reading of an
accepted claim, not the census that would earn it
(`../../package-contracts/implementation-census-primitive-returns` is that half).

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-merged-props-consumer`'s, so the closure digest and package integrity
in the import block are the ones that fixture already pins rather than numbers
invented here. `index.d.ts` is outside the closure and is this fixture's own.
`node_modules/solid-js` is required for the reason that fixture gives: an
authorized fixture is analyzed from a copy at a different depth.
