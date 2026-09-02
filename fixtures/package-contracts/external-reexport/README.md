# Re-exporting a dependency's surface needs that dependency's accepted identity

`index.ts` publishes `dependency-package` three ways: an `export *`, a renamed
named re-export, and a namespace import called from a local function. The
dependency is installed with real declarations, so nothing here is unresolvable
in the TypeScript sense.

Generation still refuses the entrypoint. Publishing `dependencyValue` under
this package's name is a claim about a module this package does not contain,
and the only thing that can bind that name to an exact module identity is an
accepted contract for `dependency-package`. Without one there is no exact
runtime binding, and the whole artifact case is refused rather than described
from the dependency's source.

This is the `dependency-contract-obligation` refusal, and it is by a wide margin
the most common refusal on real registry packages -- see
`benchmarks/ecosystem/report.md`, where `@corvu/*`, `@tanstack/*` and
`motion-solidjs` all stop here. `solid-reexport` pins that `solid-js` gets no
exemption from it.
