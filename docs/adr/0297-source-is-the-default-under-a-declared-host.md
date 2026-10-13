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
array, a `browser` map, an oversized closure) keeps its contract, and so does
a namespace import. A run without both a target and a build, including
ESLint's `browser-v2` config, is contract mode as before.

**ESLint.** The adapter's snapshot cache keys on settings, and nothing in
that key moves when an installed package's file does. A persistent editor
session would serve a snapshot of package bytes that are gone. A run that may
read package source (both host fields, or a host or selection in
`commandArgs`, and no opt-out) is therefore reused only within one file's
lint pass, keyed on ESLint's `SourceCode` object, and analyzed again on the
next pass. The opt-out restores the cross-pass cache.

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
