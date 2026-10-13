# Reviewed package behavior experiment

This prototype asks whether small, source-reviewed premises can deliver useful
static warnings without running package certification. It uses eight exact
published Solid Primitives packages with Solid/signals/web 2.0.0-rc.9 and
TypeScript 5.9.3. It performs no package installs, network acquisition, receipt
issuance, accepted-tier publication, or production analyzer changes.

The follow-up also includes a deterministic extractor over 97 retained
packages. It produces positive source premises without per-export models,
explicitly labeled `source-extracted-assumption`. Both modes are experimental.

`specs.mjs` contains the reviewed premises: ambient lifecycle ownership,
returned accessors, a returned tuple's first accessor, immediate invocation of
a stable zero-arity accessor, and a tracked inline callback. Browser and server
behavior are explicit. `models.json` binds these assumptions to released source
references and the bytes/versions of the installed dependency inputs. These
checks detect input drift; they do not establish behavioral proof authority.

## How the prototype works

`lower.mjs` resolves imports through the TypeScript program and matches exact
export symbols. It expands supported calls into analysis-only core primitive
operations in a temporary copy of the consumer. The ordinary checker then
classifies ownership and tracking from its existing semantic facts and compiler
execution facts. Generated code is never executed. Real published declarations
check both the original consumer and its analysis copy, including inferred
generic return types. Original argument evaluations are retained; modeled
tracked callbacks stay in a tracked compute phase.

`projectWarning` maps relevant results back to the original caller and labels
them `basis: reviewed-source-assumption`, `severity: warning`, and
`certification: false`. The native verdict over synthetic code is not a proof
about the actual package. No experimental model becomes an `AcceptedContract`.
Existing native uncertifiable findings are retained in the raw observations.
An absence of model warnings is not certification or complete package coverage.

A separate-module trial missed the returned tuple accessor and attributed
ownership to the synthetic helper. Call-site expansion makes those behaviors
visible to the current analyzer and gives usable caller locations. This is an
experimental adapter, not the proposed production model-loading interface.

## Reproduce offline

Use an installed Node runtime. The retained checkpoint and published installs
must exist; missing or changed inputs stop the experiment. The output directory
must be fresh. Existing evidence is never removed.

```sh
node benchmarks/reviewed-package-models/experiment.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/reviewed-models-new-run

node --test benchmarks/reviewed-package-models/*.test.mjs

node benchmarks/reviewed-package-models/runtime-controls.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/reviewed-models-new-runtime-controls
```

`SOLID_CHECKER_NATIVE_BIN` and `SOLID_TYPEFACTS_BIN` override the existing release
checker and producer. The runner records their digests. An optional third
argument filters comma-separated case IDs for a focused replay. `results.json`
records baseline and modeled results, timings, model assumptions, typing checks,
unsupported uses, and assertions. Each analysis copy and raw native output is
saved beside it. Unexpected feedback makes the run fail after saving evidence.

`build-catalog.mjs <retained-run.json> <models.json>` is an explicit authoring
operation. Re-pinning input bytes requires source review; it is not part of
normal analysis or an automatic response to a mismatch.

## Automatic extraction

The extractor follows released runtime bodies and local/imported helpers with
bounded control flow. It records direct signal/memo returns and mandatory
cleanup/effect registrations, with exact source locations and explicit gaps.
It never supplies an empty behavior claim just because it observed nothing.
Opaque options, mutation, unknown callback timing, loops and richer return
shapes remain gaps. Native semantics require the audited rc.9 environment.

```sh
node benchmarks/reviewed-package-models/extract-catalog.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/source-models-new.json

REVIEWED_MODEL_CATALOG="$PWD/rust/target/source-models-new.json" \
REVIEWED_MODEL_EXPLORATORY=1 \
node benchmarks/reviewed-package-models/experiment.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/source-models-new-feedback
```

The default case selection stays the original reviewed export set, so missing
automatic premises are measured as misses. `REVIEWED_MODEL_CASE_CATALOG` can
select a different export set; use the case-ID filter to bound that experiment.
Exploratory mode allows missed target observations, while warnings on negative
controls still fail. It does not change their certification status.

`REVIEWED_MODEL_BASELINE_REPORT` optionally reuses historical baseline outputs
after checking specimens, compiler options, binary digests and runtime versions.
Reused outputs are identified in the saved report. Modeled analyses remain
fresh. `check-results.mjs <results.json>...` replays saved warning projections.
`REVIEWED_MODEL_EQUIVALENT_CATALOG` additionally authenticates a newer catalog
and checks that every exercised premise is unchanged before replay.

`app-demand.mjs <metric.json> <app-corpus.json> <catalog.json> <fresh-output.json>`
audits cached app demand and installed input pins. It performs no fresh app
analysis and does not equate an export-name match with usable feedback.

## Scope and controls

The development group is utils, RAF, event-listener, and timer. The extension
group is media, date, lifecycle, and memo. The memo trial required adding a
general tracked-callback behavior. The final implementation uses the same
lowering code for every package, with no package-specific Rust implementation.
This is not a blinded generalization study: the researcher also reviewed the
extension implementations.

Misuse/correct-use twins come from the existing ledger. Additional controls test
aliased and namespace imports, a shadowed local factory, computed namespace
dispatch, positive function arity, a changed `length` getter, accessor rebinding,
a local wrapper return, writes in a modeled tracked callback, and reads
inside deferred RAF/timer callbacks. Copied argument bodies never inherit an
unrelated model premise. Server
controls cover exports with explicit inert branches. Unmodeled host behavior
remains unknown.

Source review and an independent runtime sample found no owner requirement for
`lifecycle.createIsMounted`: `onSettled` returns no cleanup, and its unowned call
settles to `true` with no diagnostic. The experiment records that ledger row as
a negative control. The original ledger expectation is unchanged and remains
a separate correction to review.

The follow-up also corrects media ownership: browser development code uses
conditional cleanup, so an unowned media listener can deliberately persist
without a missing-owner diagnostic. Both unconditional owner premises are
removed. The original 30/30 headline included that mislabeled owner target;
the corrected target set contains 29.

Runtime controls execute published RAF, lifecycle, media and scheduled code
under browser/development resolution in Node. RAF cancellation and media queries are mocked; this
is not real-browser conformance. RAF reproduces the no-owner cleanup warning and
the untracked read warning; its owned tracked twin is clean. These finite
samples support the review and confer no certification authority.
Media's browser globals are initialized before import, because utils chooses
its client/development behavior during module initialization.

## Remaining limitations

- Returned accessors need an immutable direct variable binding or a direct
  wrapper return. Tuple accessor positions support direct destructuring without
  defaults/rest. Stable local wrappers need zero arguments and one synchronous
  return; branches, escaping functions, cross-file wrappers, mutable bindings,
  object members and arbitrary expression placement remain unsupported.
- `utils.access` lowers only an unescaped, locally established native accessor.
  A general callable signature does not establish JavaScript function `length`.
  Positive arity, a changed getter, aliases/escapes, and unknown targets remain
  unsupported.
- A tracked callback currently needs a single inline function argument.
  Additional options, scheduling, callback cardinality, returned callback
  behavior, async readiness, stores, and arbitrary resource disposal are not
  described by this vocabulary.
- An owner operation is a surrogate for the reviewed lifetime requirement,
  not a transcription of all package operations. A wrong premise can produce
  wrong feedback. Samples and source pins do not make that premise certified.
- The experiment analyzes single-file consumer specimens. It does not establish
  editor latency, behavior across full applications, deep package graphs,
  future package versions, or coverage of all 123 ledger cases.

The initial report, with the follow-up correction marked at its top, is
[`2026-10-01-reviewed-model-experiment.md`](../../docs/package-contract-v2/phase22/2026-10-01-reviewed-model-experiment.md).
The automatic extraction, runtime controls and application-demand results are
in [`2026-10-01-source-model-followup.md`](../../docs/package-contract-v2/phase22/2026-10-01-source-model-followup.md).

## Installed models and live feedback

`demand-models.mjs` derives a catalog from the actual consumer's installed
runtime entries and requested exports, including the dependency closure. Its
call specializer interprets literal arguments, closed plain objects and inline
function identity. Identifiers, getters, spreads and evaluated fields stay
unknown. Cached profiles are scoped to package bytes, host and extractor hash;
the cache is in-process, not a persistent editor cache. Native runtime lookup
follows Solid's dependency graph, including pnpm's transitive signal package.
The audited rc.9 requirement remains explicit.

```sh
REVIEWED_MODEL_ON_DEMAND=1 \
REVIEWED_MODEL_EXTRA_CASES=benchmarks/reviewed-package-models/further-cases.mjs \
REVIEWED_MODEL_CASE_CATALOG=rust/target/reviewed-models-further-case-catalog.json \
REVIEWED_MODEL_CATALOG=rust/target/reviewed-models-further-catalog-final.json \
node benchmarks/reviewed-package-models/experiment.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/installed-models-new-feedback \
  further-pagination-second-member,further-timer-function-delay,control-wrapper-return

node benchmarks/reviewed-package-models/installed-app-demand.mjs \
  rust/target/app-import-metric/metric.json \
  scripts/ecosystem-benchmark/app-import-corpus.json \
  rust/target/installed-app-demand-new.json

node benchmarks/reviewed-package-models/runtime-feedback-experiment.mjs \
  rust/target/primitives-checkpoint/run-browser.json \
  rust/target/live-diagnostics-new
```

The case catalog in this command is a selection-only union of the reviewed
catalog and the source scan, preserving reviewed inert-server controls. The
on-demand path derives its premises afresh; it does not trust those reviewed
behaviors. `REVIEWED_MODEL_REUSE_MODELED=1` with a checked historical baseline
also reuses native outputs when generated source and compiler options match
exactly. Reuse and the original analyzed path are recorded explicitly.

`runtime-feedback.mjs` subscribes to `OBSERVE.diagnostics`, keeps semantic
execution/ownership codes, deduplicates by code and consumer location, and
retains dependency frames when present. It has no package catalog. Feedback
uses `basis: runtime-observation` and `certification: false`. Install before the
app's package setup executes, using its own Solid instance, and stop the
subscription on disposal. This prototype supports V8 stacks and has no bundle
source-map remapping or browser overlay. A returned accessor's read stack need
not retain its package factory; package origin is left unknown in that case.

The measurements, app-coverage limit and unexpected pagination setup diagnostic
are recorded in
[`2026-10-01-installed-models-and-live-feedback.md`](../../docs/package-contract-v2/phase22/2026-10-01-installed-models-and-live-feedback.md).

## Browser, holdout and editing experiments

These use retained installations and an existing browser executable. No package
installation is performed. Each output path must be fresh. Browser launch can
require permission to run outside the restricted process sandbox; requests to
nonlocal servers are blocked by the harness. The runner reports and preserves
unexpected observations rather than assigning them package certification.

```sh
node benchmarks/reviewed-package-models/prepare-holdout.mjs \
  rust/target/holdout-new-inputs

REVIEWED_MODEL_CATALOG=rust/target/holdout-new-inputs/catalog.json \
REVIEWED_MODEL_CASE_CATALOG=rust/target/holdout-new-inputs/selection.json \
REVIEWED_MODEL_EXTRA_CASES=benchmarks/reviewed-package-models/holdout-cases.mjs \
REVIEWED_MODEL_EXPLORATORY=1 REVIEWED_MODEL_ON_DEMAND=1 \
node benchmarks/reviewed-package-models/experiment.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/holdout-new-static \
  holdout-listener-owner,holdout-event-accessor,holdout-keyboard-owner,holdout-set-union,holdout-set-intersection,holdout-trigger-read,holdout-explicit-listener-disposal,holdout-global-event-bus,holdout-untracked-trigger,holdout-readonly-set

node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/browser-new '/absolute/path/to/cached/browser/executable'

node benchmarks/reviewed-package-models/editing-cost.mjs \
  rust/target/editing-new
node benchmarks/reviewed-package-models/model-scale-cost.mjs \
  rust/target/scale-new.json
```

An optional third browser argument selects comma-separated scenario IDs. The
full runner now includes app disposal and the two map behavior challenges.
The measured run predates their addition to the full runner; its supplemental
outputs are combined by the saved validator:

```sh
node benchmarks/reviewed-package-models/validate-browser-results.mjs \
  rust/target/browser-validation-new.json \
  rust/target/reviewed-models-all-browser-2/results.json \
  rust/target/reviewed-models-all-browser-app-disposal-final/results.json \
  rust/target/reviewed-models-all-browser-challenge/results.json

node benchmarks/reviewed-package-models/check-results.mjs \
  rust/target/reviewed-models-all-holdout-static/results.json
```

`analyze-browser-challenge.mjs` takes the challenge browser report and writes
fresh baseline/modeled native outputs beside it. It checks the frozen digests
in `rust/target/reviewed-models-all-holdout-inputs/study.json`; these exact paths
are intentionally bound to the recorded study. The extractor remains unchanged
through this holdout. Mutation tests change only copies of installed artifacts.

Results and limits are in the
[browser, context and cost report](../../docs/package-contract-v2/phase22/2026-10-01-browser-contexts-precision-and-cost.md).

## Broader misuse and generic guard tracing

`breadth-cases.mjs` adds 24 authored defect/control pairs and two intentional
controls across 18 packages, including copied-app navigation and modal mutations.
The browser runner checks every consumer against real published typings before
execution. Cases declare expected updates or disposal behavior; those expectations
are authored test assertions, not automatically inferred package contracts.

Use retained installations, cached app trees and an existing browser executable.
Every output must be fresh. Record the corpus and frozen static implementation
before starting the new executions:

```sh
node benchmarks/reviewed-package-models/prepare-breadth.mjs \
  rust/target/breadth-new-study.json

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/breadth-cases.mjs \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/breadth-new-browser '/absolute/path/to/cached/browser/executable'

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/breadth-cases.mjs \
REVIEWED_MODEL_GUARD_TRACE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/breadth-new-traced '/absolute/path/to/cached/browser/executable'

REVIEWED_MODEL_BREADTH_STUDY=rust/target/breadth-new-study.json \
node benchmarks/reviewed-package-models/breadth-static.mjs \
  rust/target/breadth-new-browser/results.json rust/target/breadth-new-static

REVIEWED_MODEL_BREADTH_STUDY=rust/target/breadth-new-study.json \
node benchmarks/reviewed-package-models/validate-breadth.mjs \
  rust/target/breadth-new-validated.json \
  rust/target/breadth-new-browser/results.json \
  rust/target/breadth-new-traced/results.json \
  rust/target/breadth-new-static/results.json
```

The static runner uses the existing release checker and producer, recording their
digests. It performs fresh baseline/model analyses; unchanged app findings do
not count as detections introduced by a mutation. Its app twins retain original
cross-file imports and do not constitute a rewritten application graph.

`guard-trace.mjs` instruments bounded owner/observer guards resolved through exact
core import symbols. It has no package-name rules and refuses unaudited runtime
versions. `guard-trace-runtime.mjs` records informational observations with source
digests and consumer frames. Intentional snapshots and background work can emit
the same notes; they must not become defect warnings without an applicable
behavioral expectation. The validator checks finite execution parity between
the traced and original consumers and retains every miss.

The [broader report](../../docs/package-contract-v2/phase22/2026-10-01-broad-misuse-and-guard-tracing.md)
records 15/24 targets detected by direct runtime feedback, eight additional guard
explanations, 24/24 exposed with explicit expectations, and only 4/23 static
detections. These are authored mutation observations, not universal coverage.

## Automatic channels, callback origins and intent limits

The next study adds an extended semantic/advisory collector, an optional live
attribution engine, callback origin tracing, and pairs with identical app code
but different externally declared desired behavior. Prepare the full manifest
with `REVIEWED_MODEL_AUTOMATIC_PROFILE` unset. All paths must be fresh:

```sh
node benchmarks/reviewed-package-models/prepare-automatic-study.mjs \
  rust/target/automatic-new-study.json
node benchmarks/reviewed-package-models/mechanism-inventory.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/automatic-new-inventory.json

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/automatic-feedback-cases.mjs \
REVIEWED_MODEL_AUTOMATIC_PROFILE=channels \
REVIEWED_MODEL_FEEDBACK_BRIDGE=benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
REVIEWED_MODEL_ATTRIBUTION=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/automatic-new-original '/absolute/path/to/cached/browser/executable'

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/automatic-feedback-cases.mjs \
REVIEWED_MODEL_AUTOMATIC_PROFILE=channels \
REVIEWED_MODEL_FEEDBACK_BRIDGE=benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
REVIEWED_MODEL_ATTRIBUTION=1 REVIEWED_MODEL_ORIGIN_TRACE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/automatic-new-origins '/absolute/path/to/cached/browser/executable'

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/automatic-feedback-cases.mjs \
REVIEWED_MODEL_AUTOMATIC_PROFILE=intent REVIEWED_MODEL_GUARD_TRACE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/automatic-new-intent '/absolute/path/to/cached/browser/executable'

node benchmarks/reviewed-package-models/validate-automatic-feedback.mjs \
  rust/target/automatic-new-study.json rust/target/automatic-new-inventory.json \
  rust/target/automatic-new-validated.json \
  rust/target/automatic-new-original/results.json \
  rust/target/automatic-new-origins/results.json \
  rust/target/automatic-new-intent/results.json
```

The attribution and origin profiles exclude core entries from prebundling so
they share the live core state and expose original source to the trace. The
validator checks actual engine installation. Merely requesting the engine is
insufficient. Runtime versions outside rc.9 do not enter this tracing capability.

The origin trace includes a narrow transform of the exact published settled
scheduler branch, because dropped-cleanup validation happens after a callback
returns. It changes served bytes and preserves retained installs. It supports
this synchronous branch, not arbitrary async provenance. Guard and origin
profiles are currently separate and cannot be enabled together.

Optional validator arguments after the intent report are a fresh breadth report,
its historical unchanged-code report, then focused error-delivery reports. The
validator records a changed transport count as a failed parity dimension; it
does not erase it by comparing unique messages. `REVIEWED_MODEL_DISABLE_CORE_PREBUNDLE=1`
allows focused isolation with neither attribution nor origin tracing enabled.

See [automatic feedback and intent limits](../../docs/package-contract-v2/phase22/2026-10-02-automatic-feedback-and-intent-limits.md)
for the additional findings, published typing rejection, identical-code witnesses
and measured limitations.

## Class source footprints and deferred reads

The class pass follows exact installed exports and stable private fields to
positive observer-call footprints. It reports informational setup-to-JSX flow
notes. It does not certify output reactivity or turn undeclared snapshot intent
into a defect. Generator and async bodies stay deferred; ambiguous dispatch and
escaping instances stay open. Use fresh output paths:

```sh
REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/class-footprint-cases.mjs \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/class-new-original '/absolute/path/to/cached/browser/executable'
REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/class-footprint-cases.mjs \
REVIEWED_MODEL_GUARD_TRACE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/class-new-traced '/absolute/path/to/cached/browser/executable'
node benchmarks/reviewed-package-models/class-footprint-inventory.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/class-new-inventory.json
node benchmarks/reviewed-package-models/class-footprint-study.mjs \
  rust/target/class-new-study.json rust/target/class-new-traced/results.json \
  rust/target/reviewed-models-breadth-browser-1/results.json
node benchmarks/reviewed-package-models/validate-class-footprints.mjs \
  rust/target/class-new-study.json rust/target/class-new-inventory.json \
  rust/target/class-new-original/results.json rust/target/class-new-traced/results.json \
  rust/target/class-new-validated.json
```

The [class source report](../../docs/package-contract-v2/phase22/2026-10-02-class-source-feedback-and-deferred-reads.md)
records six of nine new targets explained by source notes, the valid intentional
snapshot note, two additional broad-corpus notes, and the limited class reach.

## Finite getter paths and optimized attribution

The getter extension follows concrete field paths to exact signal/memo producer
calls. All outputs remain informational. It refuses unknown key sets and retains
source gaps. The new consumers also exercise two getter-only writes that pass
published typing and fail at runtime, plus two TS2540 exclusions and valid controls.

`REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE=1` with `REVIEWED_MODEL_ATTRIBUTION=1`
optimizes public runtime/attribution entries together. The live probe must show
installation; an unaudited runtime keeps that capability absent. This profile is
separate from original-file guard/origin tracing. The validated import graphs
include pnpm's transitive signals chunks.

Use fresh paths for each run:

```sh
REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/getter-path-cases.mjs \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/getter-new-browser '/absolute/path/to/cached/browser/executable'
node benchmarks/reviewed-package-models/getter-path-inventory.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/getter-new-inventory.json
node benchmarks/reviewed-package-models/getter-path-study.mjs \
  rust/target/getter-new-study.json rust/target/getter-new-browser/results.json \
  rust/target/reviewed-models-breadth-browser-1/results.json

REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/automatic-feedback-cases.mjs \
REVIEWED_MODEL_AUTOMATIC_PROFILE=channels \
REVIEWED_MODEL_FEEDBACK_BRIDGE=benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
REVIEWED_MODEL_ATTRIBUTION=1 REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/optimized-new-channels '/absolute/path/to/cached/browser/executable'
REVIEWED_MODEL_BROWSER_CASES=benchmarks/reviewed-package-models/breadth-cases.mjs \
REVIEWED_MODEL_FEEDBACK_BRIDGE=benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
REVIEWED_MODEL_ATTRIBUTION=1 REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE=1 \
node benchmarks/reviewed-package-models/browser-experiment.mjs \
  rust/target/optimized-new-breadth '/absolute/path/to/cached/browser/executable'

node benchmarks/reviewed-package-models/validate-getter-and-prebundle.mjs \
  rust/target/getter-new-study.json rust/target/getter-new-inventory.json \
  rust/target/getter-new-browser/results.json \
  rust/target/optimized-new-channels/results.json \
  rust/target/automatic-feedback-browser-shared-original/results.json \
  rust/target/optimized-new-breadth/results.json \
  rust/target/reviewed-models-breadth-browser-1/results.json \
  rust/target/getter-new-validated.json
```

The [getter and optimization report](../../docs/package-contract-v2/phase22/2026-10-02-getter-paths-and-optimized-feedback.md)
records the two additional errors, four source explanations, optimized runtime
identity and the retained error-delivery limitation.

## Explicit lifetimes and shared browser resources

The optional `lifetime-feedback.mjs` bridge installs a resource observer. A
consumer's explicit scope declares expected cancellation; it does not acquire
Solid ownership or repair cleanup. Timer callbacks propagate their registration,
while Promise continuations need an explicit `scope.run` after `await`. Once and
signal listeners remain gaps. All output is experimental and conditional.

Use fresh directories. First execute the four original comparison groups with
`REVIEWED_MODEL_BROWSER_CASES` set to each of `lifetime-cases.mjs`,
`lifetime-continuation-cases.mjs`, `lifetime-raf-cost-cases.mjs`, and
`lifetime-coercion-cases.mjs`. Use the
existing browser harness with the `extended-runtime-feedback.mjs` bridge and
both attribution environment flags set to `1`. Then run:

```sh
node benchmarks/reviewed-package-models/lifetime-final-run.mjs \
  rust/target/lifetime-new-final '/absolute/path/to/cached/browser/executable'
node benchmarks/reviewed-package-models/lifetime-platform-inventory.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/lifetime-new-inventory.json
node benchmarks/reviewed-package-models/validate-lifetime.mjs \
  rust/target/lifetime-new-final rust/target/lifetime-new-inventory.json \
  rust/target/lifetime-new-original/results.json \
  rust/target/lifetime-new-continuation-original/results.json \
  rust/target/lifetime-new-raf-cost-original/results.json \
  rust/target/lifetime-new-coercion-original/results.json \
  rust/target/lifetime-new-validated.json
```

The final runner captures its local inputs before and after the changed causal
propagation is exercised. The validator compares 33 consumers, their real
published typing observations, closure pins, behavior and core feedback. It
resolves inventory references against actual standard-library declarations.

The [lifetime report](../../docs/package-contract-v2/phase22/2026-10-02-explicit-lifetimes-and-shared-resource-feedback.md)
records eleven of twelve targets detected, twenty-one controls with no new lifetime
warning, the retained async miss, the shared RAF core warning, and measured cost.

## Automatic event lifetimes and native await

This follow-up declares a project policy that DOM handler work stops at its
creating owner's disposal. It supplies event context through exact renderer JSX
declarations, preserves native awaits, binds supported value callees with awaited
operands, and opts into native Promise callback delivery. An intentional
background handler uses `background(callback)`. No package model or cleanup is
supplied. Source instrumentation is limited to project files under `src/`.

Use fresh paths:

```sh
node benchmarks/reviewed-package-models/automatic-lifetime-run.mjs \
  rust/target/automatic-life-new '/absolute/path/to/cached/browser/executable'
node benchmarks/reviewed-package-models/validate-automatic-lifetime.mjs \
  rust/target/automatic-life-new rust/target/automatic-life-new-validated.json
```

The runner freezes local inputs and preserves their bytes, then compares four
profiles and the cached app flow. Its earlier recorded native profile detected
13 of 14 targets across six packages with no new lifetime warning on 13 controls.
Member calls with awaited operands were its measured miss; the follow-up below
closes that case. The generator profile is retained
as a rejected experiment because it breaks a valid control. The
[automatic lifetime report](../../docs/package-contract-v2/phase22/2026-10-02-automatic-event-lifetimes-and-native-await.md)
records behavior comparisons and remaining gaps. Outputs are observations with
no certification authority.

## Automatic ownership sampling and awaited member calls

The member bridge retains a receiver while arguments await. A focused original
and native replay is available through `automatic-lifetime-native-run.mjs`; the
existing lifetime validator accepts that two-profile study. It detects fourteen
of fourteen authored targets. The historical generator profile remains rejected.

The source sampler selects ownership-premised exports whose real published
signatures admit zero arguments. It supplies no per-export argument recipe.
Use fresh paths:

```sh
node benchmarks/reviewed-package-models/extract-catalog.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/zero-new-catalog.json
node benchmarks/reviewed-package-models/zero-argument-selection.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/zero-new-catalog.json \
  rust/target/zero-new-selected.json
node benchmarks/reviewed-package-models/zero-argument-run.mjs \
  rust/target/zero-new-selected.json rust/target/zero-new-browser \
  '/absolute/path/to/cached/browser/executable'
node benchmarks/reviewed-package-models/zero-argument-batch.mjs \
  rust/target/zero-new-selected.json rust/target/zero-new-batch
```

`zero-argument-static-cases.mjs` also supplies extra cases to `experiment.mjs`.
Set `REVIEWED_MODEL_ZERO_ARGUMENT_SELECTION`, `REVIEWED_MODEL_EXTRA_CASES`,
`REVIEWED_MODEL_CATALOG` and `REVIEWED_MODEL_CASE_CATALOG`, then filter to its
exported case IDs. `validate-zero-argument.mjs` compares the saved browser study
and static `results.json`. It validates preserved historical browser inputs and
records current local input differences explicitly; it still authenticates
current package closures and published declaration bytes.

The [ownership and member report](../../docs/package-contract-v2/phase22/2026-10-02-automatic-ownership-breadth-and-member-calls.md)
records seventeen detected ownership cases across twelve packages, seventeen
actively modeled static controls, the independent owner slice for opaque tuple
results, batch timings and the seventy-four unexercised argument-bearing exports.

## Argument witnesses and garbage-collection controls

`argument-selection.mjs` synthesizes bounded inputs from published declarations
for those argument-bearing exports. Shared DOM/scheduler seeds are type-checked;
it uses no per-package argument recipes or assertions. Runtime observations decide
which admitted inputs have a valid ownership control. Use fresh output paths:

```sh
node benchmarks/reviewed-package-models/argument-selection.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/zero-new-catalog.json \
  rust/target/arguments-new-selected.json
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/argument-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/arguments-new-browser '/absolute/path/to/cached/browser/executable' \
  rust/target/arguments-new-selected.json
node benchmarks/reviewed-package-models/argument-retry-selection.mjs \
  rust/target/arguments-new-selected.json rust/target/arguments-new-browser/browser/results.json \
  rust/target/arguments-new-retry.json
node benchmarks/reviewed-package-models/validate-arguments.mjs \
  rust/target/arguments-new-selected.json rust/target/arguments-new-validated.json \
  rust/target/arguments-new-browser
node benchmarks/reviewed-package-models/argument-batch.mjs \
  rust/target/arguments-new-validated-selection.json rust/target/arguments-new-batch demand
```

The retry selector accepts a fourth argument containing a previous consolidated
validation report, to avoid repeating its observed inputs. Do not run an empty
selection. `argument-batch.mjs` also accepts `base` as its profile. Declaration
redirects outside an authenticated physical artifact remain unmodeled.
`argument-static-cases.mjs` supplies focused cases to the existing static runner
through `REVIEWED_MODEL_INPUT_SELECTION` and `REVIEWED_MODEL_EXTRA_CASES`.

Use `gc-lifetime-cases.mjs` as the observation runner's case module, with
`lifetime-feedback.mjs` for monitoring or `extended-runtime-feedback.mjs` for the
original profile, and omit the selection argument. Native browser GC establishes
whether a weak listener target was collected before the lifetime ends.

The [argument and GC report](../../docs/package-contract-v2/phase22/2026-10-02-argument-witnesses-and-gc-precision.md)
records sixty new validated ownership pairs, seven runtime/loader/endpoint
limitations, seven synthesis refusals, static batch composition gaps and the
corrected GC false warning. Combined ownership evidence covers 77 primitives in
37 packages. These construction observations have no certification authority.

## Callback contexts and misuse breadth

`callback-selection.mjs` samples every retained package root, resolving actual
published callback types and synthesizing bounded arguments. `CallbackPaths`
records source context assumptions on exact resolved paths. The browser cases
preserve generated callback expressions while comparing native signal reads
with the same bodies containing writes. A validator maps diagnostics to exact
captured setters and requires clean read-only controls.

The validated sample has 116 callback write-error pairs across 47 packages.
Combined construction and callback evidence covers 166 exported names in 61
sampled packages. Six imperative caller controls and twelve explicit
`ownedWrite: true` controls permit the same writes. Source context assumptions
alone cannot establish forbidden writes. These finite observations have no
certification authority; quiet and unexecuted callbacks stay unverified.

Use fresh output names throughout. The chunk helper bounds browser runs to
eighty exported APIs each:

```sh
node benchmarks/reviewed-package-models/callback-selection.mjs \
  rust/target/primitives-checkpoint/run-browser.json rust/target/callback-new-selected.json
node benchmarks/reviewed-package-models/callback-chunks.mjs \
  rust/target/callback-new-selected.json rust/target/callback-new-chunks
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/callback-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/callback-new-browser-0 '/absolute/path/to/cached/browser/executable' \
  rust/target/callback-new-chunks/chunk-0.json
```

Run the other chunks in separate fresh directories, then validate all of them:

```sh
node benchmarks/reviewed-package-models/validate-callbacks.mjs \
  rust/target/callback-new-selected.json rust/target/callback-new-validated.json \
  rust/target/callback-new-browser-0 rust/target/callback-new-browser-1 \
  rust/target/callback-new-browser-2 rust/target/callback-new-browser-3
```

`callback-unavailable-selection.mjs` accepts the full selection, a fresh output
path and the browser result paths. It selects only observations unavailable
before attribution. Replay that subset and append its study to validation.
The validator retains the original history and refuses to overwrite successful
observations. Source and package bytes must remain unchanged during validation.

Caller and explicit-permission controls are selected from validated positives:

```sh
node benchmarks/reviewed-package-models/callback-imperative-selection.mjs \
  rust/target/callback-new-selected.json rust/target/callback-new-validated.json \
  rust/target/callback-new-imperative-selected.json
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/callback-imperative-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/callback-new-imperative '/absolute/path/to/cached/browser/executable' \
  rust/target/callback-new-imperative-selected.json
node benchmarks/reviewed-package-models/validate-callbacks.mjs \
  rust/target/callback-new-imperative-selected.json rust/target/callback-new-imperative-validated.json \
  rust/target/callback-new-imperative
node benchmarks/reviewed-package-models/validate-callback-callers.mjs \
  rust/target/callback-new-imperative-selected.json rust/target/callback-new-imperative-validated.json \
  rust/target/callback-new-callers-validated.json
```

Use `callback-permitted-selection.mjs`, `callback-permitted-cases.mjs` and
`validate-callback-permission.mjs` in the equivalent selection, browser and
comparison steps for the explicit signal option. Run `validate-callbacks.mjs`
on that study before its comparison validator.

The [callback report](../../docs/package-contract-v2/phase22/2026-10-02-callback-contexts-and-misuse-breadth.md)
records the sample, attribution requirements, control failures, source limits
and remaining package/runtime gaps.

## Cross-package population and returned validators

`cross-package-inventory.mjs` links exact cached installations from a declared
population into fresh consumer roots with rc.9. It preserves original package
files, records other-version refusals and links existing published type
namespaces. The unchanged callback selector and chunk helper then operate on its
`run.json` input. `cross-package-cases.mjs` uses those fresh installations and
distinct full-package output identities.

```sh
node benchmarks/reviewed-package-models/cross-package-inventory.mjs \
  rust/target/app-import-metric/apps rust/target/app-import-metric/apps/helge-dev \
  rust/target/cross-new-roots
node benchmarks/reviewed-package-models/callback-selection.mjs \
  rust/target/cross-new-roots/run.json rust/target/cross-new-selected.json
node benchmarks/reviewed-package-models/callback-chunks.mjs \
  rust/target/cross-new-selected.json rust/target/cross-new-chunks
REVIEWED_MODEL_SOURCE_PLUGIN=benchmarks/reviewed-package-models/cross-package-prebundle.mjs \
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/cross-package-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/cross-new-browser-0 '/absolute/path/to/cached/browser/executable' \
  rust/target/cross-new-chunks/chunk-0.json
```

Run the remaining chunks and use `validate-callbacks.mjs` as in the previous
section. Then `validate-callback-call-origins.mjs` takes the full selection, its
validation report and a fresh output name. It separates exact immediate
application, dependency and native Solid callers. A missing immediate frame is
never skipped; an exact served filesystem URL can supply module identity without
claiming an original source span.

To drive returned validators:

```sh
node benchmarks/reviewed-package-models/cross-protocol-selection.mjs \
  rust/target/cross-new-selected.json rust/target/cross-new-protocols.json
node benchmarks/reviewed-package-models/cross-protocol-clean-selection.mjs \
  rust/target/cross-new-protocols.json rust/target/cross-new-validated.json \
  rust/target/cross-new-protocols-clean.json
REVIEWED_MODEL_SOURCE_PLUGIN=benchmarks/reviewed-package-models/cross-package-prebundle.mjs \
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/cross-protocol-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/cross-new-protocol-browser '/absolute/path/to/cached/browser/executable' \
  rust/target/cross-new-protocols-clean.json
```

Validate the protocol study with the selected subset and `validate-callbacks.mjs`,
then audit origins. `cross-protocol-positive-selection.mjs` takes that subset and
validation report to select controls for `cross-protocol-permitted-cases.mjs`.
Validate those observations and compare them using
`validate-callback-permission.mjs`.

The [cross-package report](../../docs/package-contract-v2/phase22/2026-10-02-cross-package-callbacks-and-validation-flows.md)
records 22 package-invoked examples across six additional packages, 17 permitted
controls, direct application-call exclusions, provider/option-domain failures
and the remaining synthesis/flow limitations. Public protocol members select a
finite test flow, with no certification or unconditional static semantic claim.

## Imported value surfaces and binary callback flows

`surface-selection.mjs` resolves the exact imported value's properties, including
CommonJS `export =` members, and authenticates any separate published typing
package. `extended-witnesses.mjs` adds standard binary seeds and callbacks that
honor the requested byte count. `surface-cases.mjs` preserves member receivers.

```sh
node benchmarks/reviewed-package-models/surface-selection.mjs \
  rust/target/cross-new-roots/run.json rust/target/surface-new-selected.json \
  lodash,fflate,nanoid
node benchmarks/reviewed-package-models/callback-chunks.mjs \
  rust/target/surface-new-selected.json rust/target/surface-new-chunks
REVIEWED_MODEL_SOURCE_PLUGIN=benchmarks/reviewed-package-models/cross-package-prebundle.mjs \
node benchmarks/reviewed-package-models/observation-browser-run.mjs \
  benchmarks/reviewed-package-models/surface-cases.mjs \
  benchmarks/reviewed-package-models/extended-runtime-feedback.mjs \
  rust/target/surface-new-browser-0 '/absolute/path/to/cached/browser/executable' \
  rust/target/surface-new-chunks/chunk-0.json
```

Run every chunk, then validate with `validate-callbacks.mjs` and
`validate-callback-call-origins.mjs`. `surface-permitted-selection.mjs` accepts
the full selection, its validation, its caller-origin report and a fresh output.
Use that subset with `surface-permitted-cases.mjs`, validate it, and compare it
with `validate-callback-permission.mjs`. The subset records its parent selection
and baseline so all inputs remain authenticated by the comparison.

`surface-timing-selection.mjs` takes the surface selection and a fresh output;
run it with `surface-cases.mjs` for delayed-only debounce/throttle controls.

`binary-delivery-selection.mjs` takes the surface selection and a fresh output;
use `binary-delivery-cases.mjs` to await compression callback delivery before
invoking returned terminators. `binary-roundtrip-selection.mjs` accepts that
delivery selection and a fresh output, and supplies an explicitly reviewed
encoder/decoder relation table. Run its subset with `binary-payload-cases.mjs`
to assert actual decoded bytes. These format witnesses are package-specific
experimental inputs, without static or certification authority.

After the individual callback validators, `validate-surface-controls.mjs` takes
the surface validation, caller origins, permission comparison, timing validation,
payload validation, initial delivery validation and a fresh output. It checks
both successful and error-valued delivery so a quiet exception channel cannot
manufacture a valid operation.

The [surface and timing report](../../docs/package-contract-v2/phase22/2026-10-02-commonjs-surfaces-binary-inputs-and-timing.md)
records 44 additional package-invoked write-error examples, 44 successful write
permission controls, two safe timing counterexamples and five exact decoded
payload flows. All 420 browser records type-check. No production contract or
finding changed.

## Combined package feedback

`family-matrix-cases.mjs` supplies nineteen semantic target/control pairs and
three precision controls. `family-matrix-extension.mjs` adds a silent async
challenge and explicit snapshot intent. The transfer consumers exercise the
same returned-accessor check through an exact namespace import from the timer
package. Default-option warnings and mount failures remain separate evidence;
the published permitted-write option gives a successful paired control.

`family-matrix-feedback.mjs` shares native diagnostic collection and the
explicit resource lifetime collector. `family-matrix-static.mjs` captures
independent source/compiler observations of executed consumers.
`family-matrix-project.mjs` reprojects those saved observations without another
native analysis. `family-project-warning.mjs` joins exact operand symbols to
returned-accessor declarations, retaining source-assumption severity.
`family-transfer-static.mjs` uses `family-imports.mjs` for exact namespace calls.
Computed and shadowed dispatch remain unresolved.

`family-feedback-system.mjs` combines the channels without reading authored
expectations. `family-feedback-study.mjs` authenticates retained inputs, rechecks
published typing and compares the combined output with separate expectations.
The corpus, static copies and models do not gain certification authority.

For the retained run:

```sh
node benchmarks/reviewed-package-models/family-feedback-study.mjs \
  benchmarks/reviewed-package-models/family-study-inputs.json \
  rust/target/family-feedback-fresh-validation
```

Use a fresh output directory. The checked-in input configuration names the
retained research outputs. For a new browser run, prepare a fresh selection
with `prepare-family-matrix.mjs`, execute it through
`observation-browser-run.mjs`, analyze it with `family-matrix-static.mjs`, and
reproject with `family-matrix-project.mjs`. Build native analysis first through
`make build-checker-debug`. Extensions use their separate case modules and the
same collector. Write a separate input configuration pointing to the fresh
outputs; preserve the earlier evidence.

The [combined feedback report](../../docs/package-contract-v2/phase22/2026-10-02-combined-package-feedback-system.md)
records feedback on 20 of 21 patterns, 23 of 24 quiet controls, one silent async
miss and the four additional transfer precision observations. These counts
include advisory and candidate feedback; they are not counts of proven defects.

## Callback-phase refinement

`family-phase-specializer.mjs` combines the existing return extractor with
generic callback-phase tracing. `family-phase-projection.mjs` restores exact
source locations for callbacks copied into generated native memo operations.
`family-phase-static-v2.mjs` uses those independent premises with the unchanged
native rules. The first phase runner and its initially unprojected native async
finding remain preserved.

`family-refined-cases.mjs` preserves the previous population and flows, with
one explicit `untrack` edit for the intentional snapshot.
`family-phase-challenges.mjs` adds namespace/alias, conditional and nested-await
controls, explicit snapshots and a named-callback gap.

After their browser and static runs, reproduce the retained comparison with
fresh outputs:

```sh
node benchmarks/reviewed-package-models/family-feedback-study.mjs \
  benchmarks/reviewed-package-models/family-refined-inputs.json \
  rust/target/family-refined-fresh-validation

node benchmarks/reviewed-package-models/family-refined-validation.mjs \
  rust/target/family-feedback-study-v2/results.json \
  rust/target/family-refined-fresh-validation/results.json \
  rust/target/family-phase-challenge-browser/browser/results.json \
  rust/target/family-phase-challenge-static/results.json \
  rust/target/family-refined-fresh-comparison.json \
  rust/target/family-phase-original-intent-static/results.json
```

The [refinement report](../../docs/package-contract-v2/phase22/2026-10-02-package-feedback-refinement.md)
records matching feedback for 21 targets and silence for 24 controls: 45/45
with one explicit intent edit. The original unchanged snapshot remains noisy,
and the new named-callback challenge remains a demonstrated miss. Source
warnings retain their assumption basis and never gain certification authority.

## Packages held out of the combined matrix

`family-holdout-freeze.mjs` freezes the existing prototype modules, native
binaries, producer stamp and twelve installed package closures before new
consumers are authored. `family-holdout-cases.mjs` supplies eighteen pairs,
two extra precision controls and two published-typing exclusions.
`family-holdout-prepare.mjs` freezes all forty consumer sources, flows,
expectations and the actual resolved declarations before browser execution.
Packages are new to the combined forty-five-case matrix; some appeared in
earlier source and callback surveys.

The browser runner and `family-phase-static-v2.mjs` remain unchanged.
`family-holdout-validation.mjs` authenticates their observations and passes
only observed facts to the existing combined detector. Scoring requires a
matching rule or code. A quiet control with failed behavior, a harness failure
or an unrelated warning cannot improve the score.

The separate `family-holdout-package-checks.mjs` compares an unexpected
pagination result with a native memo. It preserves the original failed control
and never changes the main consumer population or detector.

Revalidate the retained experiment with a fresh result path:

```sh
node benchmarks/reviewed-package-models/family-holdout-freeze.mjs verify \
  rust/target/family-holdout-detector-freeze.json

node benchmarks/reviewed-package-models/family-holdout-validation.mjs \
  rust/target/family-holdout-preflight/population.json \
  rust/target/family-holdout-browser/browser/results.json \
  rust/target/family-holdout-static/results.json \
  rust/target/family-holdout-fresh-validation.json \
  rust/target/family-holdout-package-browser/browser/results.json
```

These commands require the retained installs and observations. To repeat
execution, use fresh preflight, browser and static output directories; pass
`family-holdout-cases.mjs` and the unchanged `family-matrix-feedback.mjs` to
`observation-browser-run.mjs`, with `cross-package-prebundle.mjs` as the source
plugin. Use the same browser profile for the three package checks.

The [heldout package report](../../docs/package-contract-v2/phase22/2026-10-02-heldout-package-feedback.md)
records the misses and package defect alongside the successful transfers.
No frozen detector module, production rule, contract or snapshot changes.

## Snapshot hints and further package transfers

`snapshot-feedback-v2.mjs` adds informational class snapshots and observed
getter snapshots. V3 authenticates recorded guard spans, v4 admits single-field
destructuring and refuses unknown callback contexts, and v5 admits observed
computed method calls and multi-field destructuring. Historical versions stay
unchanged. Every new candidate retains an intent limitation and no
certification authority.

The unchanged original population reaches 18/18 target feedback. Its failed
pagination control remains. The v4 detector frozen before 29 additional
consumers gives 8/10 target feedback and 16/17 quiet controls. V5 gives 10/10 on
that adapted replay. A further population authored after sealing v5 gives 2/3
targets and 6/6 quiet controls; a computed property getter remains a miss.
See the [snapshot report](../../docs/package-contract-v2/phase22/2026-10-02-snapshot-feedback-and-fresh-challenges.md).

Revalidate retained observations using fresh output paths:

```sh
node benchmarks/reviewed-package-models/snapshot-refinement-study-v5.mjs \
  rust/target/family-holdout-validation.json \
  rust/target/snapshot-original-guard-browser-v2/browser/results.json \
  rust/target/snapshot-fresh-replay-v5.json \
  rust/target/snapshot-challenge-browser-v1/browser/results.json

node benchmarks/reviewed-package-models/snapshot-challenge-validation-v1.mjs \
  rust/target/snapshot-challenge-preflight-v2/population.json \
  rust/target/snapshot-challenge-browser-v1/browser/results.json \
  rust/target/snapshot-fresh-replay-v5.json \
  rust/target/snapshot-fresh-validation-v5.json
```

For the further transfer, pass `snapshot-transfer-browser-v1/browser/results.json`
as the last study argument, then validate with
`snapshot-challenge-validation-v2.mjs` and
`snapshot-transfer-preflight-v1/population.json`. The original native results
are reused only after authenticating all frozen inputs. Fresh consumers reuse
no native results. Browser observations require the retained package installs;
`snapshot-guard-browser-run-v2.mjs` freezes its profile before and after a run.

## Feedback through expressions and local returns

V7 broadens observed setup-to-JSX flow to computed property reads, arithmetic,
conditionals, literal containers and templates. It recognizes exact native
`untrack` aliases and rejects discarded or unknown argument flow. V6's
source-map call-punctuation regression remains in its retained result.
V8 adds one exact local return/call edge; v9 keeps unknown calls and deferred
paths out of that return premise. All earlier modules remain unchanged.

`snapshot-expression-cases-v1.mjs` adds thirty consumers: the detector sealed
before authoring catches 9/10 targets with 18/18 quiet controls, and the adapted
local-return version reaches 10/10. `snapshot-helper-cases-v1.mjs` adds sixteen
further consumers after sealing v9: 4/6 targets and 10/10 quiet controls.
Multiple callers and escaped helpers remain misses. The broader adapter also
adds an unwanted informational hint to an earlier intentional control; the
older challenge population now has 15/17 quiet controls. See the
[expression report](../../docs/package-contract-v2/phase22/2026-10-02-expression-and-local-helper-feedback.md).

Reproduce the final helper replay with fresh output paths:

```sh
node benchmarks/reviewed-package-models/snapshot-refinement-study-v9.mjs \
  rust/target/family-holdout-validation.json \
  rust/target/snapshot-original-guard-browser-v2/browser/results.json \
  rust/target/snapshot-helper-fresh-study-v9.json \
  rust/target/snapshot-helper-browser-v1/browser/results.json

node benchmarks/reviewed-package-models/snapshot-challenge-validation-v4.mjs \
  rust/target/snapshot-helper-preflight-v1/population.json \
  rust/target/snapshot-helper-browser-v1/browser/results.json \
  rust/target/snapshot-helper-fresh-study-v9.json \
  rust/target/snapshot-helper-fresh-validation-v9.json
```

The validator recomputes scores and checks whether the used detector belongs
to the population's earlier freeze. For the expression replay, replace the
helper paths with `snapshot-expression-*`; that v9 result is adapted. These
commands require retained installs and observations and do not grant authority.

## Actual callers and short return chains

The versioned caller recorder preserves distinct helper call stacks and maps
every guard frame with the original consumer digest. V10 closes the two earlier
helper misses: 6/6 targets and 10/10 quiet controls on the adapted sixteen-row
replay. V11 follows exact immutable aliases, ordinary parameters, transparent
wrappers and at most four same-file helpers with single return expressions.

Fifty-two further consumers were frozen and executed in two stages. The first
thirty-row population gives 5/10 targets with the previously sealed v10 and
9/10 with adapted v11; all 18 controls remain quiet. After sealing v11, the
twenty-two-row transfer gives 5/6 targets and 14/14 quiet controls. Four
published typing errors are excluded. Member dispatch and the five-helper
depth target remain misses. The older two intentional-snapshot hints persist.

See the [caller and return-path report](../../docs/package-contract-v2/phase22/2026-10-02-observed-callers-and-local-return-paths.md)
for measured results, reproduction and limitations. New modules are
`guard-trace-runtime-v2.mjs`, `browser-experiment-v2.mjs`,
`snapshot-guard-browser-run-v3.mjs`, `snapshot-feedback-v10.mjs`,
`snapshot-feedback-v11.mjs` and their versioned studies. The two independent
caller auditors verify the retained source/stack witnesses using published
declarations. Earlier executed modules and frozen inputs remain unchanged.

## Stable members, deeper paths and observation source revisions

V12 adds exact own-member resolution with receiver mutation/escape checks and
iterative traversal of finite local helper graphs. It closes the previous two
misses: 10/10 earlier caller targets and 6/6 return-path targets, with 32/32
quiet controls. Consumer digests now gate all observed getter channels; older
nearest-frame observations require explicit authenticated replay.

Fifty-four new consumers are tested in two stages. Sealed v12 gets 10/12
targets and 20/20 quiet controls. V13 resolves nested inline object literals
and lifts that adapted population to 11/12. Sealed v13 gets 5/6 further targets
and 12/12 quiet controls. Four typing errors are excluded. Escaped receivers
and borrowed child objects remain misses; older intentional-snapshot noise is
preserved.

See the [stable-member and source-revision report](../../docs/package-contract-v2/phase22/2026-10-02-stable-members-deep-paths-and-source-revisions.md)
for reproducible studies and independent audits. `local-call-targets-v1.mjs`
and `local-call-targets-v2.mjs` contain the reusable resolver.
`snapshot-source-epoch-study-v2.mjs` tests real recorded observations against
unchanged files and a virtual changed source revision. It verifies the source
boundary rather than a complete hot-reload integration. All feedback additions
remain informational, and no production contract or finding snapshot changes.

## General package callbacks and silent async reads

Ninety additional records exercise Lodash, RxJS, Neverthrow, Zod, Valibot,
TanStack Query Core and Floating UI DOM. Shared runtime feedback survives
caught exceptions and observes writes and cleanup failures across package
callbacks. The first fifty consumers reach 21/21 targets and 26/26 quiet,
working controls after a CommonJS loader repair.

`extended-runtime-feedback-v2.mjs` retains distinct consumer frame paths and
counts repeated diagnostics on one path. Its unchanged-consumer replay has
identical behavior and native diagnostic deliveries. Fresh APIs preserve
both caller sites and coalesce four repeats into one message. Independent
audits resolve 44 runtime diagnostic locations through published typings.

The fresh thirty-eight consumers give 13/16 runtime detections. Original-source
native analysis adds one visible-await case, giving 14/16. RxJS and Neverthrow
async dependency losses remain silent. Two flawed untrack controls remain in
their original score; separately authored detached-owner controls work and
stay quiet. Five typing exclusions remain silent and are never executed.

Use `callback-context-browser-run-v1.mjs`, the three versioned context case
modules, `callback-context-study-v1.mjs`, `callback-context-native-v1.mjs`,
`callback-context-audit-v1.mjs`, `callback-context-parity-v2.mjs` and
`callback-context-seal-v1.mjs`. The seal audit authenticates two literal-loaded
support modules against earlier profiles without rewriting old freezes.

See [general callbacks and silent async reads](../../docs/package-contract-v2/phase22/2026-10-02-general-package-callbacks-and-silent-async-reads.md)
for scores, failed observations, reproduction and limits. Source/flow/labels,
published declarations and package bytes are retained. No whole-package
certification or universal rule coverage is claimed.

## Observed async reactive reads

`async-read-sites-v1.mjs` and V2 resolve local native accessors and memo-created
callbacks. Versioned transforms record exact read locations and native runtime
contexts. Versioned projectors authenticate current consumer bytes and emit
informational `intent-open` hints when the observed owner and observer are both
absent. Unknown external callback/value flow remains open. Package names,
authored expectations and displayed behavior do not enter hint selection.

The six original async consumers give 3/3 targets and 3/3 quiet controls. Among
32 additional consumers, V1 gives 6/11 and 17/18; adapted V2 gives 10/11 and
18/18. After sealing V2, 27 further consumers give 5/8 and 16/17. Five real
typing exclusions are silent. Unknown argument flow, mutable accessor aliases
and a deliberate referenced-inspection hint remain visible limitations.

Use `async-read-browser-run-v2.mjs`, `async-read-study-v2.mjs`,
`async-read-audit-v1.mjs` and `async-read-parity-v1.mjs` with the versioned case
modules. The independent auditor checks 20 read witnesses; original-six and
first-challenge parity comparisons preserve actual behavior and native
diagnostic deliveries. Early incomplete import-only seals and all original
results remain retained. The further population has a complete earlier seal.

See [observed async reads and limits of value flow](../../docs/package-contract-v2/phase22/2026-10-02-observed-async-reads-and-return-flow-limits.md)
for scores, reproduction and open facts. These additions create no production
finding or package certification authority. Real-app noise and development
overhead still require evaluation.

## Real project loading and feedback cost

The real-source inventory covers 986 configured files in eight retained rc.9
projects. All seven read candidates are synchronous array callbacks; two are
excluded by real project typing errors. `project-read-session-v1.mjs` preserves
project aliases and ambient declarations and reuses one program while observed
inputs agree. It corrects 11 false isolated-file exclusions among 28 sampled
files. Source, configuration, resolution and included-file changes invalidate
the program. Referenced projects and consumers outside the configured scope
remain unsupported.

`async-read-transform-v3.mjs` adds project loading. V4 adds a necessary syntax
gate through `async-read-prefilter-v1.mjs`; exact V2 symbol selection still decides
enrollment. `project-read-app-study-v2.mjs` skips 461/473 eligible TSX files,
preserves five valid candidates and measures 8.26 seconds for the complete
transform workload, mainly four initial project programs.

`project-read-browser-run-v2.mjs` runs the retained portfolio flow or a supplied
case module. `project-read-browser-audit-v1.mjs` compares the complete copied app
tree. The optimized portfolio replay skips all 13 loaded files and preserves
nine interaction steps plus an existing unlocated native performance advisory.
The original-six replay still gives 3/3 target hints and 3/3 quiet controls.
Neither source enrollment nor one app replay establishes package coverage.

See [real project loading and feedback cost](../../docs/package-contract-v2/phase22/2026-10-02-real-project-loading-and-feedback-cost.md)
for timings, reproduction, incomplete earlier comparisons and remaining limits.
No production rule, contract or finding snapshot changes.

## Native accessor identity across files and packages

The V5 read profile tags actual accessor function objects at the exact rc.9
native constructor, without replacing them. Runtime identity covers imports,
mutable aliases and getters returned by package primitives. Current consumer
facts and mapped native creation/read frames are still required; hints remain
informational and package callback timing, result flow and intent stay open.

Use `native-identity-freeze-v1.mjs`, `native-identity-prepare-v1.mjs`,
`native-identity-browser-run-v1.mjs`, `native-identity-study-v1.mjs` and
`native-identity-audit-v2.mjs` with `native-identity-cases-v2.mjs`. The 34-case
population gives 11/15 target hints, 14/15 quiet identity controls, four typing
exclusions and unchanged behavior/native deliveries. One inspection hint
remains noisy; an existing pagination initialization warning reduces the
combined quiet-control count to 13/15. Wrappers, bound copies, member calls and
unknown argument flow remain misses. No universal package coverage is claimed.

See [native accessor identity and package getters](../../docs/package-contract-v2/phase22/2026-10-02-native-accessor-identity-and-package-getters.md)
for the frozen evidence, independent audits, reproduction inputs and limits.

## Native reader observation through wrappers

`native-read-hook-v1.mjs` observes the exact rc.9 reader and synchronous native
`untrack` intent. `native-read-runtime-v1.mjs` associates nodes with their
constructor witness and records successful reads during admitted call scopes.
The V6 transform preserves receiver lookup/binding through a thunk. It adds
declared member candidates while computed and optional targets stay open.

Use `native-read-browser-run-v1.mjs`, `native-read-study-v1.mjs` and
`native-read-audit-v1.mjs` with the earlier identity cases for regression or
`native-read-cases-v1.mjs` for the fresh 31-case population. The replay reaches
14/15 targets and 14/15 quiet new-hint controls; the fresh population gives
8/12 and 15/17. Store-backed history, async wrappers, external callbacks and
computed members remain misses. Constant-returning debugging wrappers remain
intent noise. Plain comparisons preserve all 65 consumer behaviors and native
deliveries. No getter-specific package contract or universal coverage is claimed.

See [native read scopes and hidden wrapper reads](../../docs/package-contract-v2/phase22/2026-10-02-native-read-scopes-and-hidden-wrapper-reads.md)
for provenance, independent audits, compiler exclusions and remaining limits.

## Native store targets and property expressions

The V7 profile observes exact native store targets and their own-data serving
path, alongside the earlier native node reader. Declared property expressions
receive scopes without replacing Proxies or getters. Use
`native-read-browser-run-v2.mjs`, `native-read-study-v2.mjs` and
`native-read-audit-v2.mjs`; `native-store-cases-v2.mjs` supplies the fresh
38-case challenge. Detector modules were sealed before that population.

The unchanged 31-case replay improves to 9/12 target hints with 15/17 quiet
controls. The fresh challenge gives 8/15 targets and 20/21 quiet new-hint
controls. Intentional debugging wrappers remain noisy. Package observer
shortcuts, absent native keys and async continuations remain misses. All 69
plain comparisons preserve behavior and native deliveries. Feedback is
informational with open intent; no universal package coverage is claimed.

See [native store targets and package shortcuts](../../docs/package-contract-v2/phase22/2026-10-02-native-store-targets-and-package-shortcuts.md)
for independent audits, typing exclusions, reproduction inputs and limits.

## Package observer shortcuts

`package-shortcut-v1.mjs` resolves exact published core imports and bounded
no-observer return branches before later source-enrolled signal calls. Runtime
V3 records successful shortcuts during the existing consumer expression
scopes. It does not assert a native reactive read or returned-value flow.
Hints have their own informational `intent-open` channel.

Use `native-read-browser-run-v4.mjs` (V9 browser),
`native-read-study-v4.mjs`, `native-read-audit-v3.mjs` and
`package-shortcut-cases-v2.mjs`. The profile loads original ESM package source;
the earlier V8 profile hid those files through dependency prebundling and
loaded no shortcut models. Both observations are retained.

The earlier 38-case challenge improves to 10/15 targets without adding control
noise. The new 40-case challenge gives 13/16 targets and 19/22 quiet controls;
the six consumers added after the final profile was sealed give 3/3 and 3/3.
Debugging wrappers and a source-authored unused-signal package remain noisy.
Those counterexamples prevent treating source co-occurrence as a proven
reactivity defect. All 78 plain comparisons preserve behavior and native
deliveries. Original-source optimizer integration and real-app cost remain open.

See [package observer shortcuts and evidence strength](../../docs/package-contract-v2/phase22/2026-10-02-package-observer-shortcuts-and-evidence-strength.md)
for frozen evidence, independent audits, counterexamples and remaining gaps.

## Accessor use within package shortcuts

`signal-accessor-use-v1.mjs` and `package-shortcut-v2.mjs` require exact local
getter use, constant aliases, object escape or tuple index-0 calls. Unused,
setter-only and explicitly rebound paths are excluded. Object escape and
discarded calls still leave returned-value flow open; feedback stays
informational. Import injection preserves original dependency order.

Use `native-read-browser-run-v5.mjs`, `native-read-study-v5.mjs` and
`native-read-audit-v4.mjs`. The earlier 40-case replay keeps 13/16 detections
while quiet controls improve to 20/22. `accessor-use-cases-v2.mjs` exercises a
fresh source-authored package and gives 7/10 targets with 15/17 quiet controls.
This adds source-shape evidence rather than published-package coverage.
All 69 plain comparisons preserve behavior and native deliveries. Stored
but unread getters, discarded read results and deliberate debugging retain
intent noise; argument, computed and async paths keep their earlier gaps.

See [signal accessor use and remaining result flow](../../docs/package-contract-v2/phase22/2026-10-02-signal-accessor-use-and-remaining-result-flow.md)
for faithful typings, frozen evidence, independent audits and remaining limits.

## Calls with arguments

`native-read-sites-v4.mjs`, `async-read-prefilter-v4.mjs` and the V11 transform
admit exact declared calls with arguments and spreads. The complete original
call retains its receiver and evaluation order. Await/yield arguments, exact
built-in direct eval and computed/optional targets stay open. Runtime V3 and
shortcut V2 are unchanged. Native-only V7 and combined V8 projectors use the
current selector.

Use `native-read-browser-run-v7.mjs`, `native-read-study-v7.mjs` and
`native-read-audit-v6.mjs`. `argument-read-replay-v1.mjs` supplies the unchanged
69 consumers: 22/26 targets and 35/39 quiet controls. The frozen-profile
challenge `argument-read-cases-v1.mjs` gives 12/16 targets and 22/24 quiet
controls. `argument-native-cases-v1.mjs` adds 2/2 native-read targets and 2/3
quiet controls. All 117 plain comparisons preserve behavior and native
deliveries; 43 hints have independent audits.

Ignored scalar arguments and discarded callback reads remain intent noise,
including actual native reads. Deferred continuations, suspending arguments
and earlier native-store/profile gaps remain misses. Source observations stay
informational and do not prove returned-value flow. Initial argument V3/V10
modules and their executed evidence are retained unchanged; the final V4/V11
profile adds the conservative direct-eval boundary.

See [argument calls and result consumption limits](../../docs/package-contract-v2/phase22/2026-10-02-argument-calls-and-result-consumption-limits.md)
for typings, frozen evidence, the eval reproducer, independent audits and limits.

## Constant callback results

`constant-call-result-v1.mjs` supplies bounded source evidence for an exact
stable synchronous binding returning the same primitive. Combined projector
V9 suppresses a hint only when that call is the whole callback result, retaining
the original observation and its source model. Calls used for registration
inside larger results stay visible. Unknown bodies, ordinary members, writes,
direct eval and async/generator returns leave this evidence open.

Use `native-read-study-v8.mjs` with the current projector seal as its fourth
argument, and independent `native-read-audit-v7.mjs`. The V7 browser runner and
V11 transform are unchanged. Applying the filter to the earlier 117 consumers
retains 36/44 targets and improves quiet controls from 59/66 to 64/66; the
studies mark these as adapted replays. `constant-result-cases-v1.mjs` was
authored after the projector freeze and gives 11/11 targets with 19/20 quiet
controls. All fresh 34 plain comparisons preserve behavior/native deliveries.

Feedback remains informational with open intent. Source evidence applies to
the authenticated program; editor/HMR closure freshness, arbitrary result flow
and real-application precision remain open. An async constant helper and two
earlier member-dispatch controls retain noise. No universal coverage is claimed.

See [constant callback results and dependency registration](../../docs/package-contract-v2/phase22/2026-10-02-constant-callback-results-and-registration-precision.md)
for source boundaries, faithful typings, independent suppression audits,
retained-input recovery, verification and remaining gaps.

## Current-source feedback during live reloads

`project-read-session-v2.mjs` issues an in-memory revision over recorded program
inputs. The V12 transform embeds it; runtime V4 keeps revisions separate; the
combined V10 projector refuses unissued/retired observations before applying
V9 source models. A consumer alias can otherwise acquire a new constant-result
suppression for an old execution while its own source stays identical.

Transform V14 retires cached Vite consumer transforms when a recorded input or
included-file listing changes. V12's session invalidation alone loses current
feedback after helper reloads. V13 restores that cache boundary but also retires
valid inputs for unrelated startup events; both executed profiles are retained.

Use `feedback-revision-browser-v3.mjs` and independent
`feedback-revision-audit-v3.mjs`. The adapted V1 case population gives 15/15
targets and 21/21 quiet controls across 42 stages. V2 cases were authored after
the final detector freeze and give 10/15 targets with 15/15 quiet controls
across 35 stages. The misses are repeated revisions of one post-await read.
All 77 plain comparisons preserve behavior and native deliveries. Eleven real
typing exclusions stay silent; 90 old nonempty observation batches are refused.

This tests automatic full reloads with hot mode disabled. Stateful HMR, delayed
continuation attribution, concurrent serving, normal dependency prebundling,
real-app precision and larger-project cost remain open. All feedback stays
informational and carries no certification authority.

See [source revisions and live reload feedback](../../docs/package-contract-v2/phase22/2026-10-02-source-revisions-and-live-reload-feedback.md)
for input authority, cache failures, independent audits, timings and limits.

## Source async helper continuations

`async-continuation-sites-v2.mjs` and `async-continuation-transform-v2.mjs`
enroll exact source async helpers and wrap admitted operations with a lexical
per-invocation observation token. Runtime V5 carries provenance without Solid
context restoration, extra awaits or Promise handlers. It commits pending reads
only after explicit primitive normal completion and leaves unsupported returns,
throws and a 64-ticket budget open. Awaited child chains retain their actual
helper entry and read frames. The V12 projector checks current issued revisions
and exact source provenance before using the existing source/read models.

Use V16 of the transform plugin, `feedback-revision-browser-v6.mjs` and independent
`feedback-revision-audit-v5.mjs`. Selector V2 preserves assignment/update/delete/
tag receiver references after a valid assignment reproducer exposed an invalid
transform in V1. Executed V1 modules remain unchanged. Fourteen focused tests
and ten fresh browser variants check the reference refinement; all 113 earlier
stages produce identical emitted code/maps/sites/helper metadata under V2.

The adapted 77-stage replay catches all 30 targets with 35/36 quiet controls,
including all five earlier delayed-read misses. The 36-consumer challenge was
authored after the continuation detector freeze and gives 10/15 targets and
17/19 quiet controls. Ten later reference variants reuse paired consumer setup
with new helper bodies and give 5/5 targets and 5/5 quiet controls. Combined
123 plain comparisons preserve tested behavior/native diagnostics. These stages
and helper variants are not independent app defects or package-wide coverage.

Five valid misses and three async constant-result noisy controls remain.
External package async bodies, Promise adoption, nonprimitive completions,
nested callback timing, arbitrary value flow, intent and production integration
stay open. Feedback is informational and grants no certification authority.

See [async helper continuations and completion limits](../../docs/package-contract-v2/phase22/2026-10-02-async-helper-continuations-and-completion-limits.md)
for real typings, completion/scheduling checks, frozen evidence, independent
audits, the safety refinement and remaining counterexamples.

## Async constant fulfilled results

`async-constant-result-v1.mjs` models bounded structured completions for exact
stable source async bindings. Literal primitive returns, branch unions, catch
outcomes and finally overrides can establish one constant normal fulfilled
value. Unknown results/control flow and statement/completion budgets stay open.
The V13 projector applies the model only to an accepted actual helper witness
whose call is the whole returned expression, retaining original observations.
It checks the current issued revision again before returning feedback.

The independent `async-constant-audit-v1.mjs` resolves the source binding and
recomputes possible values/fallthrough/throws without importing the proof or
projector. Use browser V8 and revision audit V7; the browser transform V16 and
native runtime V5 are unchanged. The latest harness accepts explicit check-only
`noEmit` for JavaScript input. V2 cases preserve all V1 source/roles and correct
that config after the first runs stop on `TS5055`; unfinished runs are retained.

Replaying the earlier 123 stages/helper variants retains all 45/50 detections
and improves quiet controls from 57/60 to 60/60. The fresh 40-case source
challenge gives 8/9 targets and 25/29 quiet controls. All 163 plain comparisons
preserve tested behavior/native deliveries. These are repeated stages and
reused consumer setups, not independent defects or universal package coverage.

The new receiver-chain miss occurs before suppression: no candidate or event
is collected for `invoke(read).then(() => raw)`. A constant identifier, a loop,
an async method through a synchronous wrapper and a constant array remain noisy
controls. The five older continuation misses also remain. Source models cover
normal fulfilled values only; effects, rejection flow and intent stay open,
and informational feedback carries no certification authority.

See [async constant results and completion proofs](../../docs/package-contract-v2/phase22/2026-10-03-async-constant-results-and-completion-proofs.md)
for exact scopes, published typings, independent audits, frozen artifacts,
verification and remaining counterexamples.

## Installed async package source

`package-source-session-v2.mjs` keeps the original published typing program as
the application diagnostic gate and admits exact package ES module bodies in a
separate source program. A shared issued revision records both views. Plugin
V18 and browser V12 reuse the frozen continuation runtime/projector; no package
helper name grants behavior. Runtime imports in served `src` files seed source
discovery, type-only/configuration imports do not, and parsed module imports
replace metadata-wide dependency traversal. Skipped modules stay open when
64-package/512-file/4-MiB discovery budgets are reached.

Cross-file retry variants improve from 1/10 to 7/10 targets, with 12/14 quiet
controls. Inline callbacks remain 10/10 with 12/14 quiet controls. The earlier
76 result-flow/continuation variants retain their scores; all 128 comparisons
preserve tested plain behavior. Four new discarded-read controls are noisy,
and object/queued-task targets remain missed. Published typing exclusions
receive no hints. Enrollment spans 49 async bodies across 14 retained packages;
seven real configurations retain partial source after an earlier all-or-nothing
budget failure. Enrollment is not feedback coverage.

Browser V11/V12 retire analysis sessions after each server closes and release
the last program reference. With explicit collection, broad replays complete
below 352 MB collected heap after V9/V10 exhausted the default heap. Retained
evidence still grows. Current profiles are sealed before replay; the initial
V1 challenge is fresh and the refined V2 results are adapted replays.

See [installed async package source and typing boundaries](../../docs/package-contract-v2/phase22/2026-10-03-installed-async-package-source-and-typing-boundaries.md)
for the two-program boundary, real typings, independent audits, measured costs,
519 passing tests, immutable historical inputs and remaining open cases.

## Callback slots and deferred queue feedback

Plugin V19/browser V13 add source data-slot identity to the existing source
bridge. Runtime V6 activates attribution at an exact synchronous callback's own
entry after matching the registered object, field and function. The original
member call stays intact; normal registration and explicit callback return are
required. Promise fulfillment, result flow and reactive intent stay open.

The previous cross-file package population improves from 7/10 to 9/10 targets.
A fresh 23-case queue population gives 8/10 targets and 10/12 quiet controls;
the eight ordinary callback targets are detected, while async callbacks and
discarded-read constants expose the next limits. Two earlier populations retain
their scores. All 125 plain comparisons and seven typing exclusions pass their
independent audits. There are 553 passing prototype tests and 19 malformed
provenance refusal checks. No production rules or contracts change.

See [callback slot identity and queue feedback](../../docs/package-contract-v2/phase22/2026-10-03-callback-slot-identity-and-queue-feedback.md)
for the source/runtime boundary, immutable profiles, validation failures and
remaining storage, continuation, discovery and intent gaps.

## Async registered callbacks

Plugin V20/browser V14 link admitted source async callback entries to saved
object/function identities. Runtime V7 uses lexical operation tokens across
suspension and requires completed primitive helper chains. It preserves Solid
context, Promise scheduling and callback behavior. Failed registration calls,
unknown completion, explicit intent and exhausted budgets remain closed.

The earlier queue challenge now has 10/10 target hints and 10/12 quiet controls.
A fresh 46-case challenge gives 12/18 targets and 24/26 quiet controls; object
results, Promise adoption and budget exhaustion remain missed. Independent
audits cover 171 plain comparisons, nine typing exclusions and unchanged scores
in the other three replay populations. There are 606 passing prototype tests
and 27 malformed-provenance refusals. No production rules or contracts change.

See [async callback slots and suspension feedback](../../docs/package-contract-v2/phase22/2026-10-03-async-callback-slots-and-suspension-feedback.md)
for exact provenance, frozen profiles, checks and remaining completion,
budgeting, storage, discovery and scalability gaps.

## Loading coherence and repeated evidence

Plugins V21/V22 retain issued revisions when an update event leaves every
recorded input unchanged. Real input changes still invalidate consumer modules
and retire old evidence. The forced late-configuration replay improves from
0/3 to 3/3 target hints and preserves four quiet controls.

Runtime V8 coalesces matching pending read records before the 64-distinct-record
scope budget. Every original read still executes, and repeated child calls
retain a representative completed chain and exact occurrence count. The
corrected registered queue challenge improves from 6/14 to 12/14 targets;
65 distinct sources remain refused. Two previously refused constant controls
now emit noisy hints, so quiet controls decrease from 20/20 to 18/20.

The first new challenge followed directly returned reader candidates rather
than registered continuations. It is preserved separately at 14/14 targets
and 16/20 quiet controls, including constant/caught-error noises. The previous
async callback and continuation sets improve to 14/18 and 11/15 targets,
with quiet controls unchanged. Independent audits cover 157 plain comparisons;
674 prototype tests and 17 real published-typing CLI checks pass.

All changes remain research versions with informational feedback. Pending
budgets do not bound global events or metadata. General result flow, real-app
precision and long-session scalability remain open. See [loading coherence
and evidence budgets](../../docs/package-contract-v2/phase22/2026-10-03-semantic-update-coherence-and-distinct-evidence-budgets.md)
for adapted/fresh population distinctions, immutable evidence, checks and
remaining limits.

## Real application execution and admission

The retained `helge-dev` application now completes 39 audited UI comparisons,
including blog/article routes with clearly authored offline responses. Source,
typings and package bytes stay unchanged. Runtime V9 adds entry and buffer
counters; callback runtime V4, transform V6 and plugin V23 preserve V8's feedback
behavior. Browser V3 identifies document loads so resets and repeated snapshots
do not distort aggregate counts. Two candidates enter and remain quiet. No
positive real-app defect or precision rate is claimed.

The current inventory covers eight analysable configurations and one missing
runtime artifact. Five clean configurations have 78 candidate sites and 2,620
other-source call references outside recognizable tests and typings owners;
none directly shares a candidate's span. Declaration ownership is recorded
without assuming the runtime provider. Broader execution families and indirect
source/result flow remain open. Raw configured totals include tests and ambient
typings; the refined study reports those limitations separately.

The first observed screen takes about 4.05 seconds versus 0.90 plain in one
cold development comparison. Global buffers remain unbounded. Independent
audits validate UI/source/typing inputs and exact declaration references; the
prior async queue set retains 14/18 targets, 24/26 quiet controls and two typing
exclusions. All 682 prototype tests and five real typing CLI checks pass.

See [real application execution and feedback admission](../../docs/package-contract-v2/phase22/2026-10-03-real-application-execution-and-feedback-admission.md)
for adapted runtime profiles, evidence, cost and remaining coverage/precision
requirements. The result supports a combined feedback system rather than
making this memo-focused research path the complete product.

## Explicit async body returns

Research runtime V10/plugin V24 accept object and function returns with a
weaker body-return claim. Promise settlement and result flow stay unproved;
the runtime does not inspect thenables or add Promise reactions. Feedback
V16/V17/V18 validate primitive and body-only grades separately.

The earlier queue set reaches 18/18 targets with 24/26 quiet controls. Ordinary
helpers improve to 12/15 with 19/19 quiet controls. A fresh 32-case comparison
improves from 0/12 to 8/12 targets, while quiet controls fall from 20/20 to
12/20. Constant results, caught adoption failures and never-settling results
expose the need for result relevance before promoting observations to warnings.

There are 703 passing prototype tests, 114 distinct audited plain comparisons,
32 actual published-typing CLI checks and two sets of 31 projector refusals.
No production behavior changes. See [async body returns and warning precision](../../docs/package-contract-v2/phase22/2026-10-03-async-body-returns-and-warning-precision.md)
for frozen evidence, conditional claims and remaining coverage/precision limits.

## Capture proposals and feedback from failing tests

The source planner follows exact getter/parameter bindings from observed
registered callbacks and proposes a capture change only in an isolated copy.
Unconstrained replays measure initial/final results without expected answers.
Independent audits validate 20 source routes and edits. Ten proposals make
the authored value assertion pass; ten controls retain an already-correct
primary result. Task counts change in 18 proposals, so repair safety stays open.

A fresh visible-counter control changes the screen while its task result
remains correct. The assertion-feedback channel therefore requires a specific
failing value assertion that passes after replay. It emits ten conditional
notes and stays quiet on 22 controls in these authored populations. This
policy was designed after seeing the cases and is not a general precision rate.

All 734 tests, 28 distinct plain comparisons, 28 actual published-typing CLI
checks and independent browser/source/decision audits pass. Four getter shapes
remain refused and four earlier targets have no observed witness. Production
behavior is unchanged. See [capture replays and feedback from failing tests](../../docs/package-contract-v2/phase22/2026-10-03-capture-replays-and-test-assisted-feedback.md)
for immutable artifacts, phase/identity/side-effect risks and remaining work.

## Existing application assertions

`real-app-existing-tests-v1.mjs` runs selected original application tests and
the original Vite configuration in isolated source copies, with retained
dependencies. `real-app-existing-test-audit-v1.mjs` independently checks raw
assertions, exact test selection, source bytes and dependency closure digests.
The UI and Relay groups pass all 24 assertions without skips. These are plain
application baselines, with no feedback instrumentation or genuine failing
package-use case found. Original inputs remain unchanged; the configuration
regenerates only the copied route tree.

```sh
node benchmarks/reviewed-package-models/real-app-existing-tests-v1.mjs \
  rust/target/real-app-existing-ui-new ui
node benchmarks/reviewed-package-models/real-app-existing-tests-v1.mjs \
  rust/target/real-app-existing-relay-new relay
node benchmarks/reviewed-package-models/real-app-existing-test-audit-v1.mjs \
  rust/target/real-app-existing-ui-new/results.json \
  rust/target/real-app-existing-relay-new/results.json \
  rust/target/real-app-existing-audit-new.json
```

See [existing application assertions and confidence](../../docs/package-contract-v2/phase22/2026-10-03-existing-application-assertions-and-confidence.md)
for the tested runtime boundaries and the distinction between a useful debugging
experiment and general automatic warning coverage.

## Bounded runtime history

Native runtime V11/callback runtime V6 bound owned strong history by record
count and accounted UTF-8 bytes. Plugin V25/continuation transform V8 use those
successors; browser V25 uses projector V21 to validate retention and report
discarded observations as explicit open coverage. Metadata and guard cache
eviction preserve issued evidence. Refused guard identities lose observations.

The final two-mode trial completes 640 updates per mode with unchanged UI and
641 task calls. It retains 170/152 events below the 4 MiB accounted event budget,
reports 471/489 evictions and keeps one conditional read hint per mode. An earlier
completed comparison retains 73–76% less serialized evidence than the preserved
unbounded profile. This is an owned-history bound, not a total heap guarantee or
real application defect. The earlier two noisy raw controls remain noisy.

All 823 prototype tests, four new actual published-typing CLI checks and
independent source/behavior/retention/comparison audits pass. The final challenge
is `bounded-history-cases-v3.mjs`, executed with browser V25 and
`bounded-history-detector-freeze-v3.json`. The independent browser audit is V14;
`bounded-history-audit-v1.mjs` additionally reconstructs encoded event accounting.
Historical modules and production behavior remain unchanged.

See [bounded runtime evidence and session scalability](../../docs/package-contract-v2/phase22/2026-10-03-bounded-runtime-evidence-and-session-scalability.md)
for profile history, exact scope, artifacts and remaining cost/coverage gaps.

## Existing package hydration failure

`real-package-hydration-test-v1.mjs` reproduces the application's original
opt-in Kobalte hydration test in isolated copies. The candidate passes its real
configured TypeScript project, then fails hydration and its clean-console
assertion. The native control passes the same test and CSP. This is an existing
unused candidate compatibility failure, not a new active-application defect.

`real-package-hydration-observe-v1.mjs` binds independently generated hidden
maps to the exact original served client bytes and published source content.
`real-package-hydration-audit-v1.mjs` verifies paired test/browser/source inputs
and emits one informational failed-test note and none for the control. All 22
captured client diagnostic frames map to Solid runtime artifacts; causal package
responsibility, export dispatch and repair stay open. No per-package behavioral
contract or source instrumentation is added.

Run the test runner with a fresh output directory, `candidate` or `native`, and
the installed browser executable. Run the observer with its saved `results.json`,
another fresh output directory and the browser executable. The audit takes the
candidate/native test reports, then the candidate/native observation reports,
then a fresh JSON output path. Preserve the application's original fixed fixture
port by running browser trials sequentially.

See [existing package hydration failure and source feedback](../../docs/package-contract-v2/phase22/2026-10-03-existing-package-hydration-failure-and-source-feedback.md)
for evidence, reproduction scope and the distinction between useful test-assisted
debugging and general automatic warning coverage. Production behavior is unchanged.

## Compiler contexts for package source

`compiler-facts-probe-v1/` consumes the existing dialect adapter and normalized
fact seam. `compiler-source-feedback-v1.mjs` collects DOM/SSR facts from the
unchanged real browser-bundle sources and compares outputs with the retained
RC.9 compiler. All 96 fact runs preserve trace-on/off output; 53 outputs match
RC.9 and 43 differ. The driver expects the standalone probe built offline with
`--target-dir rust/target/debug`, whose Cargo profile puts its image under
`rust/target/debug/debug/`.

`compiler-package-bindings-v2.mjs` uses one configured TypeScript project per
bounded child and reuses the authenticated clean typing runs. It joins exact
compiler JSX spans to 84 component declarations, including 28 Kobalte bindings
across 18 source uses, and attaches 101 exact expression contexts. V1 terminates
before producing a result and remains preserved. Drifting outputs, source outside
the configured type programs and a non-unique declaration remain open.

`compiler-source-audit-v1.mjs` verifies artifact identity, contexts, source spans,
declaration bytes and the refusal partition. Runtime provider, callback invocation,
causal responsibility and repair stay open. No compiler lowering, production rule
or package behavioral contract changes. See
[compiler facts and package source contexts](../../docs/package-contract-v2/phase22/2026-10-03-compiler-facts-and-package-source-contexts.md)
for execution-context counts, exact reproduction boundaries and remaining work.

## Compiler binding fixes and installed output evidence

`compiler-package-bindings-v3.mjs` adds a bounded published-implementation program,
exact checked overload selection and unselected declaration families. The same old
facts now yield 103 bindings and 139 exact contexts, with no excluded matching
source or unresolved component declaration. `compiler-facts-probe-v2/` tries the
cached semantic-only revision 16f0988 without changing production pins; the final
comparison has 54 matching outputs, 104 bindings and 141 contexts. Forty-two
outputs still differ and remain refused. Build this standalone workspace offline
with `--target-dir rust/target/debug`; the image lives in the nested `debug/`.

`compiler-installed-getters-v1.mjs` finds zero full endpoint-map witnesses from
the actual RC.9 output. V2 instead records 380 unchanged expression copies in
emitted getters, requiring exact start and identifier/literal mappings and equal
parsed expression structure. This narrower evidence leaves lexical binding,
tracking, owner and runtime invocation open. It recovers 320 source links on
drifting-fork runs without authorizing their old compiler execution facts.

Resolution and copy tests cover actual published types, aliases, namespaces,
shadowing, members, UTF-8 spans, repeated expression text and malformed mappings.
The source audit V2 reconciles declarations and the 96 decisions; the installed
getter audit V1 repeats all 96 native compilations. These are research evidence,
with zero new findings. See
[compiler binding fixes and installed output evidence](../../docs/package-contract-v2/phase22/2026-10-03-compiler-binding-fixes-and-installed-output-evidence.md)
for exact results, artifacts, remaining cases and verification scope.

## Warning accuracy from returned fields

The returned-field experiment filters noisy field-value hints only when an
exact registered async callback returns the same primitive own fields on every
normal body return and the consumer uses only those fields. Raw read observations
remain available. Identity, effects, mutation and Promise settlement stay open.

The final projector is `native-read-feedback-v24.mjs`, browser V28 and independent
audit V17. Across 58 authored consumers, noisy controls drop from 22 to 10 while
all 20 detected targets retain feedback. Four targets remain missed. The first
policy was frozen before 18 fresh cases; the final refusal guard has a separate
adapted validation. Ninety-two focused checks pass and actual published typing
passes all valid consumers; two typing-error controls receive no extra feedback.

See [returned field relevance and warning accuracy](../../docs/package-contract-v2/phase22/2026-10-03-returned-field-relevance-and-warning-accuracy.md)
for scope, exact results, unchanged production behavior and reproduction.

## Late callback lineage and assertion-selected guidance

Runtime V12 and transform V10 preserve exact normal-body lineage for late
children and enroll lexical callbacks with mapped original entries. Final
projector V28 uses the named-constant data model V2. Browser V31 and independent
audit V18 recover all four previously missed queue targets and reduce original
automatic noise from 10/34 to 8/34. A fresh 12-case challenge detects 8/8 targets
and retains two noisy controls.

Capture planner V2 and independent capture audit V3 measure 58 isolated edits.
The existing primary-assertion selector admits 32/32 target notes and no notes
for 38 controls. Two source-identical target/control pairs require different
expected results, so this zero-noise result depends on explicit assertions.
Captures change task counts in 56/58 runs and break two passing control
assertions; repair safety stays open and no autofix is authorized.

All 118 focused tests, 130 plain comparisons, independent audits and three
modified-report refusals pass. Published CLI typing passes 256 valid source
files; two typing-error controls receive no extra feedback. See
[late callback lineage and assertion feedback](../../docs/package-contract-v2/phase22/2026-10-03-late-callback-lineage-and-zero-noise-assertion-feedback.md)
for the automatic/test-assisted distinction, seals, artifacts and reproduction.

## Focused development command

`experiment-dev-loop-v1.mjs --plan` lists the next focused run. With a fresh output
directory and installed browser path, it executes 40 tests, eight selected cases
and independent audit V19. It reuses the authenticated full plain baseline;
custom configurations can instead measure a new focused plain run. Browser V32
records phase times and preserves V31's instrumentation, waits and isolation.

The measured default cycle takes 21.27 seconds, compared with approximately
189 seconds for the preceding complete 58-case cycle. This gain comes from
focused coverage and baseline reuse. Selected runs are development-only and
do not add fresh challenge evidence or replace full validation. See
[experiment development speed](../../docs/package-contract-v2/phase22/2026-10-03-experiment-development-speed.md)
for timings, usage, remaining costs and validation boundaries.

## Lightpanda browser pilot

Browser V34 can start either Chromium or the checksum-verified local Lightpanda
1.0.0 binary through `experiment-browser-engine-v2.mjs`. The eight focused cases
match in values, counts, feedback, exact source facts and mapped frames, with
independent audit V19. The default development command remains on Chromium.

`lightpanda-benchmark-v2.mjs OUT` runs two sequential timing pairs in reversed
orders and then audits all runs. `lightpanda-comparison-v2.mjs BENCHMARK OUT`
authenticates and compares them. Mean full-process time is 15.53 seconds for
Lightpanda versus 16.51 for Chromium, a 5.9% reduction in this small population.
Preliminary V1 timing includes a corrected adapter timeout delay and is excluded
from the final speed claim. Broader compatibility and memory gains are unmeasured.
See [Lightpanda compatibility and speed](../../docs/package-contract-v2/phase22/2026-10-03-lightpanda-browser-compatibility-and-speed.md).
