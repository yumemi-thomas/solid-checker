# Browser contexts, precision and editing cost

All four follow-up experiments ran against retained published artifacts. They
support building development feedback without making package certification a
prerequisite. They also show why one feedback mechanism cannot cover every
package behavior.

The concrete result is **32 browser scenarios**, **13 semantic observations
mapped to original source**, a frozen static consumer holdout with **6/6 targets
and 10 quiet controls**, and an editing study. The browser holdout independently
executes the published code and agrees with all 16 static holdout observations.
Two additional type-invalid consumers are excluded before checker analysis.
This is an execution oracle independent of model extraction, not a blinded
study by an independent investigator.

## 1. Real browser execution

The harness uses cached Chromium 151.0.7922.34, Playwright 1.63.0, Vite 8.3.0
and `@solidjs/vite-plugin` 3.0.0-next.44. It serves isolated copies or generated
consumers and blocks requests outside each local server. It installs the
observer before app modules execute. Hot replacement wrappers are disabled for
the final context study so those wrappers do not add another execution scope.

| Integration | Executed behavior | Result |
| --- | --- | --- |
| Cached `helge-dev` app, Solid rc.9 | Home → Projects → About → Home, modal open/close, mobile navigation, history back, app disposal | All steps passed; no selected semantic observations or page exceptions |
| `@corvu-next/dialog` 0.1.5, Solid rc.9 | Open, close, Escape, focus restoration, disposal | Passed; no selected semantic observations or page exceptions |
| `@tanstack/solid-query` 6.0.0-rc.0, Solid rc.4 | Local async fetch, refetch, disposal | Rendered 1 then 2; query observer count became zero after disposal |

The portfolio cache already contains adapted source; this is not an untouched
upstream checkout. Its mount return is exposed in the copied entry solely to
invoke disposal. Both the original entry digest and instrumented digest are
recorded. The original cached app and its installation remain untouched.
The dialog and query are isolated integrations using actual app installations,
not complete executions of their authenticated parent apps.

The query runtime has no `OBSERVE.diagnostics` subscription API. Capability
detection lets the query execute without that channel. Its successful behavior
does not count as a clean semantic-diagnostic control. It is still refused by
the static rc.9 model audit; no runtime version is silently substituted.
The compiler plugin comes from the cached rc.9 app and its version is recorded
separately from the query's actual rc.4 runtime. This finite successful render
does not establish general compiler/runtime equivalence.

### Source locations and origin

`runtime-feedback.mjs` now accepts an app-frame predicate, captures up to 100 V8
frames synchronously, and restores the previous stack limit. The browser
harness remaps frames through Vite's actual source maps. All 13 selected
semantic observations have verified original file, line and column locations.
Captured router exceptions also have original app locations.

Dependency frames are retained as evidence, without asserting that every
dependency getter on the stack is an internal package defect. For pagination,
the tracked caller still receives a package setup observation at
`opts().initialPage`. The additional eager caller read has its own location
and loses the factory frame. That distinction survives the browser run.

The first browser trial retained three harness failures: a backdrop click at
the modal's covered center, a focus assertion before asynchronous restoration,
and importing an unavailable rc.4 observer export. The corrected run clicks an
uncovered backdrop point, waits for focus restoration, and detects capabilities.
None required a package change. A separate app-disposal trial caught an exact
mount-template mismatch; the final instrumented run passed. Failed outputs are
kept, and the final validator accepts only successful reports.

## 2. Generate execution-context tests

The router test resolves the exact `useLocation` import symbol in the cached
app's `NavBar.tsx`, records its source digest and position, and copies the actual
zero-argument call. It generates setup, memo, effect-apply and event contexts
inside a minimal router. Other primitives use explicitly synthetic consumers.
Closed-call transplantation has no package-specific model or LLM audit.
Captured application callbacks and backend options are not automatically
transplanted; query uses an explicitly local query function.

| Operation/context | Observed feedback | Behavior after update/disposal |
| --- | --- | --- |
| Router call and pathname read in setup | `STRICT_READ_UNTRACKED` | New route remount records `/next`; this does not prove a retained setup snapshot updates |
| Router call in memo | Quiet | Tracked pathname reaches `/next` |
| Router call in effect apply | Context-access exception, no diagnostic-channel event | Call fails |
| Router call in event | Context-access exception, no diagnostic-channel event | Call fails |
| Numeric timer at module level | `NO_OWNER_CLEANUP` | Continues after root disposal |
| Numeric timer in component setup | Quiet | Stops after disposal |
| Numeric timer in memo | Quiet | Stops after disposal |
| Numeric timer in effect apply | `NO_OWNER_CLEANUP` | Continues after root disposal |
| Numeric timer in event | `NO_OWNER_CLEANUP` | Continues after root disposal |
| Tracked pagination read | Package setup `STRICT_READ_UNTRACKED` | Page updates to 2 |
| Eager pagination read | Package setup plus caller `STRICT_READ_UNTRACKED` | Captured page stays 1 |

The exception results extend the observer design: collecting only
`OBSERVE.diagnostics` misses package context errors. Keep runtime exceptions and
structured semantic observations distinct, with original locations for both.
Each source is checked against its installed published declarations; the
successful browser study has zero consumer TypeScript errors.

## 3. Frozen consumer holdout and a silent failure

Before the new consumer study, the extractor, adapter and specializer digests
were recorded. They remain unchanged through the final result. Five packages
already present in the generic census get new consumer tests: event-listener,
keyboard, set, trigger and event-bus. No holdout failure was used to tune the
extractor.

Six targets cover mandatory listener ownership, event accessor reads, keyboard
ownership, union/intersection accessor reads, and trigger tuple reads. Ten
controls include tracked reads, owned setup, explicit listener disposal, a
global bus, explicit `untrack`, and a readonly set snapshot. Static analysis
detects all six targets with no warnings on controls; the baseline has no
matching finding, including uncertifiable findings. Actual browser execution
emits the matching semantic code on all six targets and stays quiet on the ten
controls. All 16 original and generated analysis consumers pass published
strict typings. TypeScript rejects a numeric event handler and numeric key;
their actual diagnostics are saved and those consumers never reach the checker.

This sample does not estimate precision across arbitrary packages. The same
investigator authored the consumers, and source patterns already supported by
the bounded extractor are overrepresented. An independent blinded corpus and
large application executions remain open.

The separate `ReactiveMap` challenge is a useful counterexample. A declared
test expectation says the displayed size should follow inserted entries.
Capturing `map.size` in setup leaves the DOM at **0** after an insertion; reading
it in JSX displays **1**. Both runtimes are quiet. Both sources pass TypeScript.
The frozen extractor produces no premise for the class, and native analysis
keeps `package-contract-incomplete` as uncertifiable on both consumers. No
proven violation or projected warning identifies the stale display.

This establishes a missed observed behavior, not a universal rule against
snapshots: a snapshot can be intentional. A generated behavior test can report
the failed expectation without inventing a package-wide contract.

## 4. Editing, cache reuse and artifact changes

The editing harness measures one small timer consumer with reusable TypeScript
source objects and the existing in-process specialization cache. Dependencies
are still authenticated from their current bytes during lowering.

| Measurement | Observed cost |
| --- | ---: |
| Initial installed catalog, no requested generic exports | 50.2 ms |
| Specializer creation, including authentication | 13.6 ms |
| First consumer preparation | 314.7 ms |
| Ten warm body-only edits: preparation median / observed p95 | 33.5 / 96.2 ms |
| Same edits: model authentication and lowering median / observed p95 | 14.4 / 16.0 ms |
| Three separate native launches | 1,431–1,467 ms |
| Preparation plus native analysis | 1,462–1,768 ms |

Ten body-only edits perform zero new package extractions. Changing a literal
delay to an inline function performs exactly one new extraction. The total is
two extractions and ten cache hits. This measures preparation reuse; the native
checker is still launched separately and its consumer analysis is repeated.
It is not an implemented language server or a keystroke-latency promise.

On copied artifacts, an unrelated consumer edit preserves model admission.
Changed runtime bytes at the same version, changed public declaration bytes,
and a changed package version all refuse the old model. These are simulated
artifact changes, including comment changes that deliberately invalidate the
whole-package pin. A real published release upgrade was not provisioned.
Dependency bytes are never modified in the retained installations.

A second cost study authenticates 1, 10, 30 and 56 models across their retained
installs three times. Median sequential costs are **15.9, 192.3, 531.6 and
982.1 ms**. This excludes extraction, typing and native analysis and is not a
combined app benchmark. It exposes repeated whole-closure hashing, including
shared dependency work. A production editor needs validation shared by
dependency identity and an invalidation mechanism that detects byte changes.
Source-model cache hits alone do not solve that cost.

## Recommended implementation

Build a development integration with three independent inputs:

1. Collect runtime semantic diagnostics and exceptions, with source maps and
   preserved dependency frames. This needs no per-package certification.
2. Derive bounded static premises from requested installed exports and concrete
   call arguments. Label their feedback as assumption-based warnings, and keep
   unresolved package behavior explicit.
3. Generate context, update and disposal tests from isolatable call sites. Use
   explicit behavior expectations to catch stale values and resources that keep
   running, including cases where the diagnostic channel is quiet.

Cache model extraction by exact artifact, dependency, host, extractor and
argument identity. Share dependency validation across active models. Retain an
incremental native session for consumer edits; the measured process launch
path is too slow for continuous full analysis. Record which calls and flows
were exercised, so quiet results cannot be mistaken for coverage.

Certification can remain a separate source of stronger proof. It need not gate
development feedback. The prototypes remain outside the production analyzer;
no accepted contract, schema, fixture snapshot or production diagnostic changes.

## Evidence and verification

Implementation and offline commands are in
`benchmarks/reviewed-package-models/README.md`. Ignored generated evidence:

- `rust/target/reviewed-models-all-browser-2/results.json`: 30 successful scenarios.
- `rust/target/reviewed-models-all-browser-app-disposal-final/results.json`: final
  full app flow, including explicit disposal.
- `rust/target/reviewed-models-all-browser-challenge/results.json` and
  `static-challenge.json`: two silent map consumers and fresh native analysis.
- `rust/target/reviewed-models-all-browser-validated.json`: 33 saved observations,
  32 unique scenarios and all 13 source locations replay-validated.
- `rust/target/reviewed-models-all-holdout-inputs/`: frozen digests, selection,
  installed catalog and actual excluded TypeScript diagnostics.
- `rust/target/reviewed-models-all-holdout-static/results.json`: 16 fresh baseline
  and 16 fresh modeled analyses, also replay-validated.
- `rust/target/reviewed-models-all-editing-cost/results.json` and
  `rust/target/reviewed-models-all-model-scale.json`: preparation, native launch,
  byte-change and scale measurements.

The handoff passed **23 experiment tests**, the browser saved-observation
validator, and replay of all 16 static holdout observations. Syntax checks cover
27 JavaScript modules; tracked and untracked whitespace checks, schema and
dialect validation also passed. `make verify-fast` reused the stamped producer
and passed Rust formatting and workspace Clippy with the repository's
certification environment. The Bun launcher attempted to provision its runtime
and failed on restricted DNS; the same dialect validation ran successfully
through the already-installed Node runtime. No installation was performed. Full
`make verify`, production coverage, ownership and contract-corpus gates are
deferred because this slice changes isolated prototypes and documentation.

Remaining gaps: older runtimes without an observer channel; unexecuted routes
and callback branches; async attribution after the initiating stack is gone;
non-V8 browsers and production bundles; captured arguments that cannot be
transplanted; unsupported classes, subpaths, loops and callback-phase models;
independent blinded precision and real published upgrades; persistent model
caches, shared byte validation and incremental native editor analysis.
