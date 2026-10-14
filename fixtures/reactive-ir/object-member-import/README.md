# Imported object members (ADR 0299)

Claim: a direct call of a stable const object's own function has that exact
function's owner edge and read summary, including across a named import.
`MissingOwner` reaches `onCleanup` after await. `Reads` covers an arrow,
Object.freeze/function value, quoted key, shorthand declaration, anonymous default object,
and redundant optional links on a proven non-nullish receiver and callable.
The ordinary synchronous helper retains the anonymous member's source-location
proof; conditional and detached async helpers remain uncertifiable.
`Owned` stays clean. Negatives refuse let/written bindings, replaced members,
Object.assign, computed keys, getters, trailing spreads, opaque escapes,
namespace re-exports, a possibly-null receiver, a cross-file named write,
homonymous freeze, and receiver escape through this in direct and sibling methods.
A written const identifier cannot be tsc-clean; the binder write veto is tested
with synthetic IR facts, while the fixture's written let remains type-correct.

Claim-bearing Solid declarations are copied verbatim from dependency-source-owner,
which records their published 2.0.0-rc.13 source. JSX scaffolding is reduced.
