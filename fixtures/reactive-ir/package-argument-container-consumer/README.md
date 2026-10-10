# A consumer of returns of argument containers

Pins ADR 0115's and ADR 0116's consumer arm: an accepted contract that closes
`returns` over returns that each hand back the caller's own argument, a fresh
array of the caller's arguments, or what an invocation of the argument
returned leaves nothing open at the import, where the same export with
`returns` left open raises `SC9005`.

`App.tsx` calls two pairs of exports, each pair identical in everything but
that claim; the first is declared as `@solid-primitives/utils` declares
`asArray`:

| function | export | `returns` | verdict |
| --- | --- | --- | --- |
| `count` | `asArray` | closed over `parameter 0`, `argument-array []` and `argument-array [0]` | clean |
| `countOpen` | `asArrayOpen` | open | `SC9005` for `returns` |
| `greeting` | `accessWith` | closed over `invocation-result 0` and `parameter 0` (ADR 0116) | clean |
| `greetingOpen` | `accessWithOpen` | open | `SC9005` for `returns` |

`accessWith` and `accessWithOpen` are declared exactly as
`@solid-primitives/utils@7.0.0-next.4` declares `accessWith`, and both close
`callbacks` over the one call-time invocation of their argument. What the
invocation returned names no leaf the consumer can project, so the union reads
as describing no reactive return; that reading is pinned by
`contracts.rs`'s
`argument_containers_project_as_their_one_leaf_or_as_no_reactive_return`,
and this fixture pins that the closed claim leaves nothing open at the import.

The consumer's return is one reactive leaf, and a union of the argument and an
array holding it has none they all share, so a closed claim over it projects as
describing no reactive return, as the consumer reads a local conditional whose
branches disagree. Contract returns are only ever read to find a reactive
leaf, so the reading can hide a finding and never invent one. Built without
that projection arm, `count` raises `SC9005` too; analyzed without the
authorization, the fixture reports nothing at all.

## Why this fixture needs an accepted contract

As `../package-plain-return-consumer` explains: everything above is downstream
of a contract the checker accepted, so the fixture ships
`.solid-checker/authorize-contract.json` and no catalog, and the document states
its claims rather than proving them
(`../../package-contracts/implementation-census-argument-returns` is the
census half).

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-plain-return-consumer`'s, so the closure digest and package integrity
in the import block are the ones that fixture already pins. `index.d.ts` is
this fixture's own and `tsc`-clean for both uses. `node_modules/solid-js`
selects the Solid 2.0 dialect for the authorized copy.
