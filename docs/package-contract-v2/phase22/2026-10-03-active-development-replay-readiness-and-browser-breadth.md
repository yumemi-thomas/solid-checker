# Active development, offline replay and broader browser validation

## Outcome

The active experiment now has an application readiness protocol, bounded
projection transactions, authenticated offline replay, a small program pool,
memory sampling and one working profile. Historical scripts and all 636 prior
code/input pins and 19 artifact pins remain unchanged.

The final eight-case browser process averages **10.18 s on Chromium**, compared
with **16.51 s** before this slice: a **38.4% reduction**. Lightpanda averages
**9.18 s**, 9.8% below current Chromium. These measurements include shutdown
and memory sampling, with sequential Chromium → Lightpanda → Lightpanda →
Chromium runs. Audits run afterwards. They measure the browser process, not a
complete development cycle or general browser performance.

| Focused browser process | First execution | Second execution | Mean |
| --- | ---: | ---: | ---: |
| Chromium | 10.12 s | 10.23 s | 10.18 s |
| Lightpanda | 9.13 s | 9.22 s | 9.18 s |

The source protocol changed deliberately. Independent plain/source audits
validate the new apps; this is not a claim that instrumentation or raw internal
read counts stayed identical to the old wait-based apps. The four former
targets, two named-data controls and two identity controls retain their previous
feedback outcomes. All 24 focused comparisons against the first current
Chromium execution match, including exact mapped frames and runtime counts.

## Broader Lightpanda result

The new derived population has **63 cases and 93 stages**, using retained
published queue@1.0.0-next.3, map@1.0.0-next.2 and
controlled-signal@1.0.0-next.3 packages with Solid 2.0.0-rc.9 and TypeScript
5.9.3. It includes object/function returns, adoption/reactions,
rejection/getter failures, intentionally pending work, namespace/re-export and
delayed helpers, typing errors, source edits and configuration reloads.

Both engines pass independent audits against a fresh Chromium plain run:

- **39/39 target stages receive hints**, with no missed targets.
- **41/49 control stages are quiet**; the same eight noisy controls remain.
- Five real published-typing error stages receive no additional hints.
- 54 nonempty retired observation batches are refused in each engine.
- Both perform 30 automatic reloads and need no explicit stage reloads.

These are adaptations of existing authored cases, not fresh challenges or
evidence about all packages. They extend browser compatibility coverage; they
do not improve the automatic selector's precision. Rejected adoption/getter
failures, pending Promises and accepted identity behavior remain noisy here.
The earlier test-assisted zero-noise result remains a separate result with an
explicit intent premise.

Values, hints, suppression models, source facts, mapped frames and retired
batches match in **93/93 stages**. Exact counter parity is **90/93**. Three
capture controls have 39 Chromium versus 37 Lightpanda native-read entries;
their other compared counters match. Different pending-probe polling could
explain this, but the reports do not record probe counts, so that explanation
remains an inference. The comparison preserves these differences.

| Broad observed process, one execution per engine | Chromium | Lightpanda |
| --- | ---: | ---: |
| Wall time | 109.07 s | 99.82 s |
| Sampled maximum browser-tree summed RSS | 870.1 MB | 98.3 MB |
| Sampled maximum complete runner-tree summed RSS | 2,355.0 MB | 1,592.8 MB |

Memory samples are taken every 100 ms from the owned process tree. Summed RSS
can double-count shared pages; these are sampled maxima, not unique physical
memory or continuous peaks. Node/TypeScript remains a substantial cost even
with Lightpanda. Layout, other browser APIs, long sessions and other package
families remain unmeasured. Chromium stays the default and final validator;
Lightpanda is an optional lower-memory research runner.

## What changed

### Readiness and page loads

Navigation waits for document loading. An authored pending probe reads the
same memo/field used by the view and asks the installed `Solid.isPending` for
its status. Intentionally pending cases use their existing body counter. All
waits have a timeout. Unknown protocols fail closed. No fixed 80 ms update
wait, quiet-network window or global Promise patch is used.

The first implementation used a dynamically registered owned `onSettled`
callback. A full plain run timed out on a tracked map update, although its
isolated reproduction passed. That protocol was replaced; its timings are
excluded from the final result. A later broad plain run was interrupted by a
startup navigation between update and snapshot. The runner now retries only
an interrupted page context or mismatched load identity, at most three
attempts, from a fresh app state. Semantic failures stop immediately, and
snapshots from different loads are never combined. Unit tests cover retries,
bounds and refusal. The successful recovered plain run needed no retries.

Successful measured observations were preserved after the plain-run failure.
Recovery creates a new report linking the failed parent, fresh plain execution
and new audits. It does not rewrite failed reports or repeat unchanged timed
observations. The later reversed focused runs exercise the final runner.

### Reuse source work safely

A projection transaction acquires the exact current source program and full
input manifest once. Nested selectors reuse that state synchronously. Full
inputs and issued revision are validated before and after projection. The
facade refuses unissued revisions, async work, invalidation and access after
closure. This removes repeated complete filesystem validation inside the
frozen selector layers without changing their semantic models.

The pool keeps two in-memory TypeScript programs by default. Keys include the
full content/resolution manifest and source/TypeScript producer identity.
Changed bytes, missing-file facts, directory membership, real paths and
configuration refuse reuse. There is no timestamp-only key, persisted TS
program, cross-project AST sharing or blanket package trust. Closing a resident
workspace releases its cache and refuses further projections.

### Offline selector work

Replay authenticates the saved report, original archived profile, package
closures and complete final-stage inputs. It rebuilds the same program and
reproduces the original feedback first, then reuses the program for the working
selector. The output separates observed execution from derived projection and
archives the working tool bytes. Browser observations are never relabeled as
execution of edited code.

Ten final stages reproduce their recorded projections exactly, with ten cold
builds and ten cache hits. Their cold projections total 5.86 s and warm
projections 1.87 s; the complete command takes 8.13 s, including authentication
and both projection passes. These replay timings were not isolated from the
plain validation and the resident-workspace test. They are observations of this
run, not controlled performance estimates. A separate real-source test uses a
distinct wording selector, verifies a cache hit, preserves witnesses and leaves
the recorded report unchanged.

Only final stages whose full inputs still match disk are admitted. Changed
source, stale revisions or absent facts refuse replay. Historical filesystem
views, changed runtime instrumentation and unexecuted paths require new work or
new browser execution. Selector edits still require focused tests and an
independent semantic audit before being accepted.

### One active interface

`benchmarks/reviewed-package-models/development/` is the mutable working area.
`selector.mjs` is the feedback seam; `profile.mjs` defines the focused selection.
`run.mjs` snapshots working tools, runs tests, browser execution and the audit,
and saves phase times. Configurations record a hypothesis and success criterion.
Passing checks does not automatically prove the stated hypothesis.

Historical semantic modules remain imported and sealed; this slice does not
rewrite them into a new analyzer. Future changes can use the working seam and
archive a milestone rather than copy another full runner chain. See the
[active experiment guide](../../../benchmarks/reviewed-package-models/development/README.md)
for browser, baseline reuse and replay commands.

## Validation and artifacts

All **61 focused tests** and the **one resident-workspace test** pass, with no
skips. The safety tests cover same-size/same-mtime edits, negative resolution
facts, directory changes, real paths, unknown inputs, cache limits and closure,
stale event revisions, synchronous transaction lifetime and page-load retries.

Two broad audits validate 186 observed stage executions and 186 comparisons
against the complete plain population. Four final focused audits validate 32
additional observed stages and plain comparisons. These are repeated/adapted
executions, not 218 independent semantic challenges. The ten-stage final replay
and altered-selector resident test also pass.

The universal fast checks pass: `make verify-fast` supplies the producer/probe
pins for formatting and workspace Clippy; schema parsing, dialect manifests,
source syntax checks and whitespace checks pass. Full production `make verify`,
coverage, ownership, oracle and contract gates are deferred because this slice
changes research tooling and documents. No production source, diagnostic,
fixture snapshot, bundled contract, compiler pin or dependency changes.

Principal generated evidence, under ignored `rust/target/`:

- `development-broad-recovered-v1/results.json` and its two independent audits;
- `development-broad-comparison-v1.json` (93 stages, three counter differences);
- `development-focus-benchmark-v1/results.json` and four independent audits;
- `development-focus-comparison-v1.json` (24/24 exact comparisons);
- `development-replay-focus-v3/results.json` (ten rebuilt/warm final stages);
- `development-tooling-handoff-freeze-v1.json` (final source/artifact references).

Earlier failed/provisional runs and their source snapshots remain preserved and
are excluded from the final timing claim. All evidence is user-writable
research data with `authority: false` and `certification: false`. Package
runtime semantics, effects, Promise settlement/result flow, intent, general
compatibility and universal coverage remain open.
