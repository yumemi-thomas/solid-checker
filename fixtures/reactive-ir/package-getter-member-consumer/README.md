# Returned plain object getter members

ADR 0272's authorized synthetic package owns its own declarations. Its exact
object has reactive plain-valued pathname/search/hash/state getters, inert key,
and a query data property holding a store proxy. It is not a router stub.
Real router pairs typecheck separately against published next.26/rc.13 types.
The manifest and authorization identity match package-lazy-getter-consumer.
Every getter leaf asserts an own enumerable accessor. A synthetic package
using Object.defineProperty with enumerable omitted (or false) must not
describe that non-enumerable getter with this leaf: direct Get behavior alone
does not authorize spread/assign/values/entries traversal claims.

Body reads, destructuring, object spread and Object.assign
must prove strict-read-untracked violations. JSX/memo reads, key, retrieving
query and Object.keys remain clean. Query property reads retain store semantics.
Values/entries follow the same placement rule, with existing copied-value
provenance obligations retained. JSON.stringify always remains SC9012;
serialization is not modeled. makeWithToJSON describes an own enumerable
pathname getter and an own inert toJSON method returning null, so serialization
must never be guessed to read pathname.
makeWithAccessor retains a reactive accessor data member: spread/assign/values/
entries must remain SC9012, including directly tracked traversal.
Computed members, mutations, unknown escapes, unsupported reflection, defaults,
rest, aliases and wrappers without exact targets must retain uncertifiable
dispatch obligations. Transparent TypeScript receivers preserve exact Gets.
The same-named local factory must remain plain; namespace imports use exact
Type Facts targets. Existing store/accessor fixtures must not move.

AsyncCompute (a Get after `await`), GeneratorCompute and NamedInlineSelfEscape
must each retain one `returned getter execution` SC9012 and no violation.
Returned/stored/escaped nested callbacks cannot inherit an enclosing memo,
effect, untrack or onSettled execution role. They must retain `returned getter
execution` SC9012 obligations. Named functions, even memo-only, onSettled and
inline wrappers such as flush also remain uncertifiable. This consumer proves
only direct body/JSX and exact inline compute/event/untrack placements.
Direct memo/compute, untrack and event callback Gets remain controls.
Direct effect apply retains its existing strict-read violation.
Isolated `cases/PrototypeToJSON.tsx` retains the inherited-toJSON regression;
both own and inherited toJSON now fall under unmodeled serialization.

`cases/TypedBuiltinAlias.tsx` and `TypedKeysAlias.tsx` are isolated native
controls, excluded by the default include. They require zero violations and
one dispatch obligation: structural builtin method typings do not identify
the runtime callee. Isolation also prevents missing unrelated type-reference
facts from withholding the whole-project builtin proof in the base controls.
