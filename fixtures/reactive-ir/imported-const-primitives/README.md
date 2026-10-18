# Imported constants (ADR 0325)

A named import of another module's exported `const` holds a primitive when
its initializer is proven one, so it settles the unmodeled use a callee makes
of the prop or argument it feeds (ADR 0320's primitive proofs, which read
only a file-local, unexported `const` before). The import must land on the
declaring module itself (the compiler's attested resolution, no re-export
hop), the module must be a module (not a script), and the declaration a
module-level `const` of one identifier exported under exactly the imported
name (`export const X`, or `export { N as X }`). A `const` cannot be
reassigned and a module namespace is frozen.

`App.tsx`. Settled (no `SC9012`): `Cluster` (string), `Limit` (number),
`Aliased` (`export { HIDDEN as ALIASED }`), and `Direct` (the argument itself,
no component). Refused, still open: an object (`Object_`), a call initializer
(`Called`), a type assertion (`Asserted`), `export let` (`Mutable`), a default
import (`Default`), a re-export hop through `barrel.ts` (`Barrel`), an
`export *` (`Star`), and a constant initialized from another exported constant
of its own module (`Chained`: the file-local proof refuses an exported
constant, so the chain is not followed).

Types: the published rc.13 declarations of `solid-js` and `@solidjs/signals`,
copied unchanged from `standard-runtime-configuration`; the tree passes
`tsc --noEmit`.
