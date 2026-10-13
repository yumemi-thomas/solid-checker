# ADR 0291: Every Solid package is analyzed from source

- Status: accepted and implemented; opt-in as `--dependency-source '*'`
  (2026-10-13). The default of an ordinary analysis under a declared target
  and build since ADR 0297 (2026-10-14).
- Owner: `dependency_sources::prepare_all` and its request collection in
  `main.rs`.
- Step 2 of moving dependency analysis from contracts to source (ADR 0284).

## Context

ADR 0284 analyzes the packages a caller names. Naming them per project does
not scale, and it misses what the named packages load. The decision to move
from contracts to source needs the checker to choose the packages itself.

## Decision

`*` selects packages from the project's runtime imports. Each import is an
`(importer, bare specifier)` pair from the project's configured sources. Each
package resolves from where its importer is, so a package's own dependencies
resolve from its own `node_modules`. Selected packages' closures add their
bare runtime imports to the same worklist.

A resolved package is selected when all of these hold:

- **It is a Solid package:** its `dependencies` or `peerDependencies` name
  the dialect's runtime (`Dialect::primitive_defining_packages`) or a package
  in one of `Dialect::ecosystem_scopes`. Any other package stays what it is
  without this mode: an unknown package whose behavior is an obligation.
  Without this filter, a project that includes `vite.config.ts` pulled in its
  whole build toolchain (`@babel/core`, `jiti`, `acorn`).
- **It lives under `node_modules` once its path is resolved.** A linked
  workspace package resolves to the project's own directory. It is project
  code and is never selected. In the first sweep, openbot and app-game lost 28
  findings in their own workspace packages this way.
- **It is not the dialect's own runtime,** which stays vocabulary.
- **It is analyzable.** A package that is not (an `exports` array, a
  `browser` map, an entry or relative import that is not a source file, an
  oversized closure) is skipped with a note naming the reason. Nothing of it
  enters the program, and it keeps its contract. A name with an unsupported
  copy keeps its contract for every copy, because contracts unbind by name.

Relative imports inside a package resolve in Vite's extension order (`.mjs`,
`.js`, `.mts`, `.ts`, `.jsx`, `.tsx`), as the declared Vite host loads them.

`exports` subpath patterns (`./*`) now resolve as Node resolves them: the
longest prefix wins, the matched text replaces every `*` in the target, and a
match that steps out of the package (`..`) selects nothing. That covers
Kobalte, which was previously refused.

## Consequences

- `*` mode matches the hand-picked 11-package list on every violation and
  leaves fewer obligations. Over the rc.13 corpus:

  | Mode | Violations | Uncertifiable |
  | --- | --- | --- |
  | 11 packages | 560 | 3,556 |
  | `*` | 560, same rows | 3,244 |
  | base | 562 | 3,731 |

  There were no errors.
- **Cost:** about 2.9× base mode's total time over the corpus, against about
  2× for the 11-package list. Agent builds ran at the same time, so absolute
  walls are inflated. Step 3 is caching analyses by package bytes.
- Not selected: non-Solid packages, so a callback a non-Solid library calls
  stays an obligation. Also not selected: workspace packages, Solid's own
  runtime, and the packages refused above.

## Evidence

- **Fixture** `reactive-ir/dependency-source-all`:
  - three violations: a selected package, a transitive one resolved from its
    importer's `node_modules`, and a pattern export;
  - two obligations: a non-Solid package and an `exports` array, which is
    skipped with a note.

  `tsc --noEmit` is clean.
- **Unit tests:**
  - a linked workspace package is project code;
  - subpath patterns follow Node;
  - every runtime-imported subpath is selected.
- **Coverage:** no other snapshot moved.
