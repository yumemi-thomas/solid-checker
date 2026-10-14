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
any runtime project or selected package file reaches through a namespace import,
namespace re-export, literal bare dynamic import, `require`, or import-equals
binding is not selected, in any selection, and keeps its contract too (review
rounds 1 and 2). Named/default imports and named/default-as/star re-exports
bind exact runtime exports instead. Nonliteral loads bind no contract in either
mode; relative literal loads join the package closure. Type-only edges, dialect
runtime edges and build-config imports do not retain packages. A run without both a target and
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

The boundary is a contract binding that source selection would drop. The first
implementation retained every re-export target and every candidate beside a
relative/nonliteral load; its 488 extra uncertifiable results and loss of the
three probus-hk source proofs were over-retention. This revision replaces those
blanket rules with the following selection.

Named, default, default-as and star re-exports follow the same exact
compiler-symbol/runtime-target joins as imports, in project barrels and selected
package files. Explicit selection expands its rows into package importers too.
Project barrels outside `files`-only roots enter through the program inventory.
Explicit exports precede stars; distinct star bindings or an unknown competing
star provider refuse the join. Known absence and unavailable export facts are
distinct outcomes. An
exact re-export can end at an unselected external declaration; requiring every
export to have a selected runtime body would incorrectly retain its entire
exporting package. That boundary follows the accepted-contract test below.

Namespace imports, `export * as ns`, import-equals, literal bare `require` and
literal bare `import()` retain the target package for all copies and subpaths.
Nonliteral loads bind nothing in contract mode and retain nothing. Relative
literal loads are walked inside the selected closure. Erased type-only edges,
the dialect's primitive-defining packages (including runtime subpaths), and
`vite.config.*`/`vitest.config.*` edges do not retain packages or seed selection.
An unreadable project AST still refuses selection conservatively.

A selected package A importing an unselected Q retains A's contract only when
the run's admitted catalogs/tiers accept Q and a Q binding or derived value
reaches A's exports. Internal calls with discarded results retain nothing.
This check runs after acquisition, closes retention over overlapping roots and
selection aliases, removes retained source files/redirects, and restores the
ordinary external-package obligations omitted during initial acquisition.
It is shared by one-shot and retained analysis.

The ten primary barrel process variants now require a source-proven `SC1001` read
through the re-export, stronger than contract mode's open `SC9005` claim, under
default, explicit `*`, and explicit package selections. Selecting only a barrel
whose target has an accepted contract preserves contract mode's findings.
Dynamic-load and config controls distinguish the ignored edges from namespace
bindings. Five more star controls cover identical bindings, explicit precedence,
ambiguity refusal, an unselected provider whose declarations omit a runtime
export, and an opaque CommonJS provider. The last two are `tsc`-clean and report
`SC9011`, never a guessed read violation. Binder-resolved initializer, assignment,
return and shadowing controls
exercise exported values separately from internal external-package uses.
An accepted source package re-exporting an unaccepted external helper stays
selected; passing a reactive source through that helper reports `SC9011`
(uncertifiable), rather than retaining the entire package or silently certifying
the call.

Still open independently: contract mode's `resolve_contract_imports_inner`
projects explicit re-export specifiers only. A bare star has none, so its open
accepted return claim raises no `SC9005` there. Source mode's star regression
now proves the runtime read; this change does not repair contract-mode star
projection. Namespace/module-object redirects remain unimplemented and retain
contracts; missing external acceptance remains source analysis's obligation.

Revised rc.13 corpus (49 projects, release binary, browser/development/CSR):
562 violations at exactly the baseline sites, including probus-hk
`RouteDetail.tsx:2214:7`, `Search.tsx:302:42` and `Search.tsx:317:11`;
3,275 uncertifiable against 3,248 (+27), with no analysis errors. There are
28 added and one removed uncertifiable sites: 24 added sites are Vite-config
imports, which now stay in ordinary contract mode instead of selecting build
tools, and four are runtime imports.

The complete retention table has 12 package entries across nine apps (six
distinct names): eight namespace cases, three source refusals (`browser` map,
raw-query import, JSON import), and one accepted external re-export:
`@rigelbuild/solid-virtual` exports `@tanstack/virtual-core`. Neither router nor
TanStack's Solid packages are retained; app-game and build tools have zero
entries. Every entry names its requesting file, line and binding form; source
refusals also name the offending closure edge. Per-site deltas and reasons are
in the corpus's `experiments/sweep-sol-reexport2-reasons.json`, beside
`sweep-sol-reexport2.json`.
