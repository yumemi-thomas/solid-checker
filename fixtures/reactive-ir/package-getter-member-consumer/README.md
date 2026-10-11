# Returned plain object getter members

ADR 0272's authorized synthetic package owns its own declarations. Its exact
object has reactive plain-valued pathname/search/hash/state getters, inert key,
and a query data property holding a store proxy. It is not a router stub.
Real router pairs typecheck separately against published next.26/rc.13 types.
The manifest and authorization identity match package-lazy-getter-consumer.

Body reads, destructuring, object spread, Object.assign and JSON.stringify
must prove strict-read-untracked violations. JSX/memo reads, key, retrieving
query and Object.keys remain clean. Query property reads retain store semantics.
Computed members, mutations, unknown escapes, unsupported reflection, defaults,
rest, aliases and wrappers without exact targets must retain uncertifiable
dispatch obligations. Transparent TypeScript receivers preserve exact Gets.
The same-named local factory must remain plain; namespace imports use exact
Type Facts targets. Existing store/accessor fixtures must not move.

`cases/TypedBuiltinAlias.tsx` and `TypedKeysAlias.tsx` are isolated native
controls, excluded by the default include. They require zero violations and
one dispatch obligation: structural builtin method typings do not identify
the runtime callee. Isolation also prevents missing unrelated type-reference
facts from withholding the whole-project builtin proof in the base controls.
