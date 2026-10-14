# ADR 0297: Source is the default under a declared host

- Status: accepted and implemented (2026-10-14).
- Owner: `defaults_to_dependency_sources` and its call at the top of `run` in
  `main.rs`; `loadSnapshot` in `packages/cli/eslint.cjs`.
- Step 4 of moving dependency analysis from contracts to source (ADRs 0284,
  0291, 0296).

## Context

`--dependency-source '*'` (ADR 0291) analyzes every Solid package from its
installed runtime source. Over the rc.13 corpus it reports the same 562
violations as contract mode with about 450 fewer uncertifiable results, and
the daemon retains it (ADR 0296). It needs `--runtime-target` and
`--runtime-build`, because the file a package loads depends on both. Until
now a caller had to ask for it.

## Decision

**Default.** An ordinary analysis that declares both a target and a build,
and names no `--dependency-source`, runs as `--dependency-source '*'`. The
selection is decided once, at the top of `run`, before any mode or the
daemon branch reads it, so a retained check is the explicit `*` daemon.

**Premise (review round 1).** Source selection reads `exports` as Vite with
the Solid plugin does: `solid`, `import` and `module` are always set, beside
the host's conditions. A target and a build do not say the project is built
that way, so the default also needs the plugin: `@solidjs/vite-plugin` or
`vite-plugin-solid` installed at the nearest `node_modules` above the
project's tsconfig (walked as dialect detection walks for `solid-js`), or one
an installed `@solidjs/start` resolves from its own directory. Whether
`vite.config` registers the plugin is not observed. The default also needs
the tsconfig's directory writable, because source analysis writes its
sibling tsconfig there and contract mode writes nothing. Without either, the
run is contract mode and prints a note naming the reason; a one-shot write
that fails anyway falls back the same way. An explicit `--dependency-source`
states the premise itself and keeps both its selection and its write error.

**Ordinary only.** Every other mode keeps the selection it was given:
contract emission and its batch, the module inventory and runtime-module
resolutions, contract resolution and proposal plans, the package root and
proposal dependencies, declaration probe plans, runtime probe and
certification planning and execution, `--check-contracts`, contract
validation, merging and review, `--feedback-facts`, `--runtime-resolution`
(which runs project code per check), caller-supplied sources and the daemon
server, which serves its client's selection verbatim. None of these has been
shown to mean the same thing under a source selection. `solid-checker
feedback` passes `--no-dependency-source` with its runtime, so it keeps
contracts too.

**Opt-out.** `--no-dependency-source` restores contract mode. It is refused
beside `--dependency-source`, which asks for the opposite. A fixture says
`"dependencySource": false` in `.solid-checker/runtime.json`; ESLint says
`settings.solidChecker.dependencySource: false`.

**Fallback.** Nothing is deleted. A package `*` does not select (non-Solid
packages, workspace packages, Solid's own runtime) or refuses (an `exports`
array, a `browser` map, an oversized closure) keeps its contract. A package
any project or selected package file reaches through a namespace import,
runtime re-export, dynamic import, `require`, or import-equals binding is
not selected, in any selection, and keeps its contract too (review rounds 1
and 2): those bindings are never redirected to source. A run without both a target and
a build, including ESLint's `browser-v2` config, is contract mode as before.

**ESLint.** The adapter's snapshot cache keys on settings, and nothing in
that key moves when an installed package's file does. A persistent editor
session would serve a snapshot of package bytes that are gone. A run that may
read package source (both host fields, or a host or selection in
`commandArgs`, and no opt-out) is therefore reused only within one lint pass
over a file, keyed on ESLint's per-pass file context, and analyzed again on
the next pass. The opt-out restores the cross-pass cache.

## Consequences

- Over the rc.13 corpus (release binary, `--runtime-target browser
  --runtime-build development --rendering csr`, no other flag): 562
  violations and 3,287 uncertifiable, no errors. Every finding equals the
  same binary's explicit `--dependency-source '*'` run, and every violation
  row equals the saved `*` sweep's (`experiments/sweep-k-all.json`). The three
  uncertifiable findings that sweep lacks are `derp-media-server` store reads
  from ADR 0294's review, which landed after it; the explicit run has them
  too.
- A host-declared run costs what `*` costs (ADR 0291): about 2.9× contract
  mode one-shot, 0.24 s warm under the daemon.
- A host-declared ESLint session analyzes once per linted file instead of
  once per process.
- The `--runtime-realm window` premise (ADR 0295) now applies to the default
  selection as well; it is still refused when nothing is selected.
- The three rows the retirement plan lists as lost to source mode against
  contract mode (kui `TopicsRoute.tsx:425`, spotify-desk-thing
  `PlayerControlIcon.tsx:26` and `PlayerControls.tsx:133`) are now lost by
  default; `--no-dependency-source` recovers them. See
  docs/precision-backlog.md.

## Evidence

- **Process tests** (`tests/dependency_daemon_process.rs`, armed) on
  `dependency-source-all`:
  - target and build alone equal `--dependency-source '*'` in JSON and notes,
    and print `dep-array`'s refusal note;
  - `--no-dependency-source` prints no source note and answers differently;
  - a target alone, a build alone and neither equal their opted-out runs;
  - `--check-contracts` under the host equals its opted-out run, with no
    source note;
  - `--no-dependency-source --dependency-source '*'` is refused;
  - the default's cold and warm daemon answers equal the one-shot one, the
    explicit `*` check hits the same daemon's cache, and an opted-out
    retained check equals its one-shot one.
- **Unit tests** (`default_dependency_source_tests`): only an ordinary
  analysis with both host fields defaults; each of the 35 other modes and
  caller-supplied sources does not.
- **Adapter test:** one analysis per pass under the host, none reused across
  passes, the opt-out flag reaches native and restores the cache, a target
  alone keeps the cache, a host or selection in `commandArgs` counts as
  source.
- **Coverage:** no snapshot moved. One fixture declared a target and a build
  without a selection: `package-variant-precedence-consumer`, whose claim is
  contract variant precedence. Its package is not a Solid package, so the
  default selects nothing there; it is pinned with `"dependencySource":
  false` anyway.

## Review round 1

Five findings, all fixed:
- **The resolver premise.** Selection reads `exports` with `solid`,
  `import` and `module` set, as Vite with the Solid plugin does, and a target
  and build alone do not establish that build: a Node host never sets
  `solid`. The default now needs a Solid Vite plugin resolvable from the
  project (see Premise above); without one it is contract mode with a note.
- **A `SourceCode` linted again.** `Linter.verify` accepts a `SourceCode`
  object and lints it again, so keying the pass on it served the previous
  pass's snapshot. The pass is now ESLint's per-pass file context (one
  `FileContext` per `runRules` call, the prototype of every rule context of
  that pass), and each adapter rule drops the entry at `Program:exit`, after
  every `Program` read of the traversal.
- **A flag-shaped value.** `--runtime-condition --no-dependency-source` is a
  condition, but the adapter took it as the opt-out and cached across passes.
  `commandArgs` are now read at native's option positions, from a table a
  test checks against `request_from_args`; an option the table does not know
  counts as reading source.
- **Namespace imports.** This ADR promised a namespace import its contract;
  selection removed it, and nothing redirects a namespace member to source.
  With an accepted contract whose `returns` was open, the call was then
  reported nowhere. A package any project or selected package file imports
  through a runtime namespace binding is now not selected, in any selection,
  and keeps its contract exactly as in contract mode. Keeping the contract
  beside a selected package was not chosen: the contract index binds by
  importer and specifier, which a redirected named import of the same
  package shares, and nothing shows the two answers cannot both apply.
- **Read-only projects.** The default no longer adds a write requirement:
  without a writable tsconfig directory, or when the one-shot write fails, it
  is contract mode with a note. An explicit selection keeps the error.

rc.13 corpus (release binary, same flags): 562 violations at the same sites,
3,248 uncertifiable (3,287 before). en-passant has no plugin, and no install,
so it is contract mode; `*` selected nothing there before either.
`@kobalte/core` in four projects and `@solid-primitives/controlled-signal`
in four are kept on contracts for a namespace import, which turns 44 openbot
source-mode obligations into contract mode's answer and adds 5 `SC9005`
that contract mode reports too.

Found in this round: a re-export of a selected package (`export { x } from
"pkg"`, in a project barrel or between packages) is not redirected either,
and loses an accepted contract the same way. Review round 2 below fixes it.

Evidence: process tests `the_default_needs_a_solid_vite_plugin`,
`a_read_only_project_keeps_contracts_by_default`,
`a_namespace_import_keeps_its_accepted_contract` and
`a_namespace_import_in_package_source_keeps_that_package_on_contracts`, each
failing on the reviewed commit; adapter tests for a re-linted `SourceCode`,
option positions and the option table.

## Review round 2

Runtime named, star, default-as and namespace re-exports are not redirected by
`symbol_redirects`. Their target package now keeps its contract for every copy
and subpath, including beside a named import of the same package. Imported
project barrels are scanned even outside a `files`-only tsconfig's roots.

A source package re-exporting a non-runtime package keeps its own contract too:
retaining only the target still removed the caller's obligation, while the
target's open claim was filtered as a package-located finding. The dialect's
primitive runtime re-exports already bind exact vocabulary.
Pruning also removes overlapping installed roots and every copy sharing their
manifest or resolved selection names, including npm aliases. It leaves no
selected root without its source files; a re-exporter without a manifest
identity retains all candidate contracts.

The audit also retains contracts for literal bare dynamic imports, unshadowed
`require` and runtime import-equals. A nonliteral or relative load, or unreadable
project AST facts, cannot be scoped to one package, so all candidate contracts
stay with a note. Erased type-only edges do not block selection; mixed imports
retain their runtime named/default redirect. `export =` follows its value's
binding, and a locally bound `require` is not guessed to load a module.

Ten process variants compare project and package barrels with contract mode,
under the default, explicit `*` and explicit package selection; all are
`tsc --noEmit` clean. Named/default-as project claims and package-barrel
requirements remain `SC9005` (uncertifiable). Dynamic-load regressions and
focused unit controls cover the other binding forms.

Still open: contract mode itself projects explicit re-export specifiers only.
A bare star has none, so its accepted open `returns` claim raises no `SC9005`.
The star-only regression pins contract retention and baseline equality; a star
beside a direct accepted import retains that import's `SC9005`. The independent
IR-owned star projection gap remains in `resolve_contract_imports_inner`.

rc.13 corpus, release binary and the same browser/development/CSR flags:
562 violations (unchanged count), 3,736 uncertifiable (3,248 before), no errors.
Three contract-mode sites return: kui `TopicsRoute.tsx:425:75` and spotify's
`PlayerControlIcon.tsx:26:5` and `PlayerControls.tsx:133:5`. Three source-only
probus-hk sites disappear: `RouteDetail.tsx:2214:7`, `Search.tsx:302:42` and
`Search.tsx:317:11`. All added violations already occur in the saved contract
sweep. Opaque-load fallback accounts for most of the 488 extra obligations.
74 package names across 37 apps newly retain contracts; per-app deltas and
package names are in `experiments/sweep-sol-reexport-delta.json`, with full
notes in `sweep-sol-reexport-selection.json` beside the sweep.
