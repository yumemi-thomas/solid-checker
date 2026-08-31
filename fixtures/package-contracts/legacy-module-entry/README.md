# A legacy manifest whose `module` build is the analyzable runtime artifact

This is the published shape of most legacy Solid ecosystem packages -- for
example `@solid-primitives/until@0.1.1`: no `exports` map, a CJS `main`, an ESM
`module`, and a `types` sibling. `main` is what Node's own resolver loads;
`module` is the bundler's ESM entry, and the two are builds of the same source.

Resolving only `main` on the runtime axis lands on the CJS file, which has no
runtime ESM export, and refuses the entire package. So legacy runtime
resolution prefers the `module` target when it is declared and present, and the
resolution trace records `legacy:module` -- the certifier replays the same
choice, so which field won stays attributable.

The declarations axis is unchanged: it reads `types`, then `typings`, then
`main`. `module` never names a typing, so it is not consulted there, and this
case's declaration branch stays `legacy:types`.

`legacy-module-absent` pins the fallback when `module` names no file, and
`legacy-dual-root` pins a dual package that declares no typings at all.
