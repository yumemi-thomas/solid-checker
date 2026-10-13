# A consumer of a registered computation

Pins ADR 0114's consumer arm and ADR 0161's reading of its count: an accepted
contract that states a `compute` operation in `computations` tells a consumer
the export registers a computation on its caller's owner, so an unowned call is
`SC4001 missing-owner`. The count decides the finding kind: `min: 1` says every
call registers, which is the proven violation an unowned `createEffect` would
be; `min: 0` says a call may register, and an unowned call is then a proof
obligation, reported uncertifiable.

`App.tsx` calls three exports that are identical in everything but that item:

| call | export | `computations` | verdict |
| --- | --- | --- | --- |
| module scope | `startTicker` | one `compute`, `min: 0` | `SC4001`, uncertifiable: it may register (ADR 0161) |
| module scope | `startTickerAlways` | one `compute`, `min: 1` | `SC4001`, a proven violation: every call registers |
| inside `Ticker` | both | one `compute` | clean: the component's owner is there |
| module scope | `startTickerSilent` | none | clean: `creates` is closed, so "no owner requirement" |

All three summaries close `callbacks`, `reads`, `creates` and `returns`, so nothing
else is open at either import and the one finding is about the owner.

A generated contract could not say this before ADR 0114: the generator withheld
an `Effect` requirement by name because version 1 had no domain for it, and a
document carrying `computations` or `kind: compute` was refused as it decoded.
Analyzed without the authorization, this fixture reports nothing at all.

## Why this fixture needs an accepted contract

Everything above is downstream of a contract the checker *accepted*, so this
fixture ships `.solid-checker/authorize-contract.json` rather than a catalog,
exactly as `../package-plain-return-consumer` does and for the same reasons.
The document states its claims rather than proving them: what is under test is
the consumer's reading of an accepted `compute`, not the certification that
would earn it (`owner_requirement_operation`'s witness,
`a_compute_operation_is_witnessed_by_an_effect_role_call_only`).

## Stubs

`node_modules/reactive-package/package.json` is byte-identical to
`package-plain-return-consumer`'s, so the closure digest and package integrity
in the import block are the ones that fixture already pins. `index.d.ts` is
this fixture's own; both signatures are `() => void` and `tsc`-clean wherever
they are called. `node_modules/solid-js` selects the Solid 2.0 dialect for the
authorized copy, which is analyzed at a different depth and inherits nothing.
