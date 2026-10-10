# Published RC.13 compiler-facts candidate

## Outcome

The live npm registry checked on 2026-10-03 reports `2.0.0-rc.13` as `next`
for `solid-js`, `@solidjs/compiler`, `@solidjs/web` and `@solidjs/signals`.
They were published on September 30 and identify upstream Git head
`5efaf260becb32293f2bcb4d32f8be72be6de674`.
The registry's `latest` tag still names older releases; it does not select
the current Solid 2 RC.

The compiler-facts fork has been merged onto that exact release in an isolated
local candidate. The compiler delta contains trace models, recording hooks,
tests, documentation and the semantic SHA-256 dependency. Files outside
`packages/compiler` match upstream exactly. Production checker pins, contracts,
runtime packages and historical research seals remain at their previous inputs.

The candidate passes output comparisons against both untouched upstream source
and the actual published darwin-arm64 compiler. This establishes a reproducible
compiler base for the next integration experiment; it does not establish new
package feedback accuracy or certify application behavior.

## Exact identities

| Input | Revision |
| --- | --- |
| Published upstream RC.13 | `5efaf260becb32293f2bcb4d32f8be72be6de674` |
| Candidate implementation merge | `c04c48779812d3d87166da3741c625748458c62f` |
| Candidate distribution | `3ad4bbec37ae30f325a803cdb4271a71c86a2a2d` |
| Previously retained research distribution | `16f0988e0313c9ce1a06ed479799b45262eaca43` |
| Production distribution, unchanged | `9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4` |

The distribution commit has one parent, the implementation merge. Its only
change is the implementation identity constant in `semantic_trace.rs`.
Trace version remains 3; normalized compiler-facts protocol remains 2.
The candidate source-manifest digest is
`sha256:35f4874f646f9482d0549fdf052f0d37bd02ec47b5c024d5732b6d9796423a9a`.

The checked upstream `next` head was
`9e85a092dd6d1a183a50148b526c1c650e7cab22`. Its post-release compiler changes
include Android WASI fallback, computed SSR style keys, lowercase `on*`
attribute semantics and coverage pragma preservation. Those changes are
outside the published RC.13 candidate. A move to unreleased `next` requires
another exact source/output audit.

## Reconciliation work

The merge resolved nine files touched by both the compiler and fact recording.
It preserves RC.13's SSR `hoistProps` pass, component/binding source labels,
attributes-before-ref ordering, spread source arrays and tails, server-function
export validation and TSRX frontend. The fork does not restore retired lowering.

Focused tests pin the added fact paths:

- `hoist_props`, `source_names_components` and `source_names_bindings` are
  included in effective trace configuration. Sixteen DOM/SSR option combinations
  check configuration binding and trace neutrality.
- DOM spread `$key` expressions are recorded as discarded.
- SSR spread tails record deferred rendering where the emitter builds a thunk.
- Server component spread refs/events retain conditional claim semantics.
  The claim does not assert a server event invocation or ref application.
- Server component class/style holes reconcile the selected evaluation or thunk.

The old Rust corpus reader partially unescaped JavaScript template literals.
Four escape-sensitive probes consequently represented different inputs from
the JavaScript suite. It now reads the Oxc parser's cooked literal value.
The independent comparison driver reads TypeScript's cooked literal value.
The replacement 365-entry baseline comes from untouched upstream, and both
readers agree against it. No baseline was regenerated from the modified fork.

## Validation

| Check | Result |
| --- | --- |
| 95 fixture sources + 270 probes, nine profiles | 3,285/3,285 upstream/fork matches |
| DOM/SSR trace on/off, seven profiles | 2,555/2,555 output/diagnostic matches |
| Universal/Dynamic fact requests | 730 explicit refusals |
| Published RC.13 native artifact vs untouched source | 3,285/3,285 matches |
| Rust without default features | 53 passed, baseline writer ignored |
| Rust with only TSRX | 116 passed, baseline writer ignored |
| Rust default features | 63 passed |
| JavaScript compiler suite | 41 files, 5,938 tests passed |
| Isolated checker adapter with candidate pins | 13 passed |
| Comparison integrity tests | 17 passed |

The 3,285 comparisons are combinations of 365 sources and nine configurations,
not 3,285 independent programs. They compare complete JavaScript, source maps,
CSS result fields and diagnostic strings. Compiled traces additionally bind
source/output/map digests and exact compiler provenance. The published wrapper
and native tarballs were verified against registry SHA-512 integrity before
being loaded from an isolated directory. No existing installation was replaced.

Initial JavaScript runs lacked the generated Babel plugin and dependencies
needed by SSR runtime tests. Building the plugin from candidate source and
reusing existing local dependencies fixed that setup. The final complete run
uses the rebuilt candidate native library and passes all 5,938 tests.

Strict all-feature Clippy fails on two unchanged RC.13 branches:
`collapsible_if` in `shared/validate.rs:101` and `if_same_then_else` in
`tsrx/project.rs:168`. Untouched upstream produces the same failures.
Excluding only those two named lints passes the remaining candidate checks.
Candidate-added formatting differences were fixed; upstream formatting
differences in directives, DOM dynamics, refresh and TSRX files remain.

The main checker universal set passed: workspace formatting and Clippy through
`make verify-fast BUN=node` with certification environment intact, whitespace,
schema JSON and dialect manifests. Its Type Facts binary already matched its
local source stamp and was reused.

## Artifacts and remaining adoption work

[Candidate files](../../../benchmarks/compiler-facts/rc13/README.md) include
the complete semantic delta, a 224 KiB Git bundle preserving exact commits,
comparison tools, an isolated adapter and a hash-bound evidence summary.
The adapter is a research copy of the production projection with candidate
identity literals. It exercises the same normalized fact interface; production
adapter behavior and pins have not changed.

Raw outcomes and logs live under `rust/target/compiler-rc13-*`. The evidence
summary records their hashes and completed gates. The patch and bundle are
outside that disposable build tree. Earlier failing runs are retained as
investigation history, not counted as successful validation.

Before production adoption:

1. Publish or otherwise make the exact candidate distribution available to
   Cargo. The branch is local; no remote push occurred.
2. Move Cargo pins, adapter provenance/cache identity, source manifest,
   identity document, notices and conformance records atomically. Keep the
   Type Facts producer/client ownership unchanged.
3. Run armed compiler process tests, finding/coverage and ownership comparisons,
   contract conformance/corpus and `make verify`. Review actual finding changes
   before any scoped snapshot update. These gates were deferred because the
   production compiler dependency was not moved in this preparation.
4. Align the experiment's compiler options and runtime packages with the exact
   real application build. The current normalized request does not expose the
   new hoisting/source-name options; adapter tests exercise compiler defaults.

Universal/Dynamic and authored TSRX trace requests still refuse. Files skipped
by `requireImportSource` do not claim fact coverage. Missing census decisions
refuse tracing. Generated operation enumeration remains partial, so an absent
recorded wrapper cannot prove that a wrapper was absent. Published native
artifact validation covers darwin-arm64, not every release platform.

These boundaries remain explicit; an RC.13 rebase does not make all packages
or all rules analyzable and does not remove the prior experiment's noisy controls.
