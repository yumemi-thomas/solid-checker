# Testing package feedback beyond the successful matrix

The frozen combined detector finds **14 of 18 target patterns across twelve
packages** that were absent from the combined forty-five-case matrix. All
**20 authored controls receive no feedback**. Nineteen controls behave as
intended; the remaining control exposes a reproducible pagination package
defect. Two additional inputs are rejected by real published typings and
receive no checker feedback.

This supports useful feedback across package families without authoring a
contract for each package. It also shows that the earlier **45/45 does not
generalize to every package**. Four stale collection/getter consumers remain
misses. The main population and detector were preserved throughout the study.

## Method and limits

Before authoring the consumers, `family-holdout-freeze.mjs` recorded digests
for 162 existing inputs, including the detector modules, native analyzer,
Type Facts producer and stamp, rule manifest, and twelve installed package
closures. The native analyzer was built through the pinned Makefile target;
the producer stamp matched the local source manifest.

`family-holdout-prepare.mjs` then froze forty consumers: eighteen target/control
pairs, two extra controls and two typing exclusions. It pinned each source,
flow and expectation, plus **846 actual resolved declaration files** and the
TypeScript/browser tooling. This improves on the previous study, where the
declaration digests were captured only during validation.

The unchanged browser runner executed the original consumers with published
Solid, signals and web **2.0.0-rc.9**. The unchanged phase runner analyzed both
the originals and separate analysis copies. Generated native analogs were
never executed. Source and flow hashes, expectations, package closures,
declarations and the complete frozen detector were checked afterward.

The existing combined feedback function receives observed facts only.
`family-holdout-score.mjs` evaluates expectations afterward. A target needs a
matching rule or code; an unrelated warning cannot count as detection. A
harness failure or incorrect control behavior cannot count as success.
There were **no harness failures or targets with only unrelated feedback**.

These packages are held out of the combined matrix. Some appeared in earlier
source and callback surveys. This is a deliberately authored test population,
not a random ecosystem sample, a blind third-party benchmark, or a statistical
estimate of coverage across npm. Every artifact keeps `authority: false` and
`certification: false`.

## Results by package

Control counts below require both silence and correct observed behavior.
The two typing exclusions are outside these counts.

| Package | Exact version | Matching targets | Quiet, correct controls |
| --- | --- | --- | --- |
| `@solid-primitives/map` | `1.0.0-next.2` | 0/2 | 3/3 |
| `@solid-primitives/set` | `1.0.0-next.2` | 1/2 | 2/2 |
| `@solid-primitives/scheduled` | `2.0.0-next.2` | 1/1 | 2/2 |
| `@solid-primitives/event-bus` | `3.0.0-next.3` | 2/2 | 2/2 |
| `@solid-primitives/date` | `3.0.0-next.3` | 2/2 | 2/2 |
| `@solid-primitives/mouse` | `4.0.0-next.3` | 0/1 | 1/1 |
| `@solid-primitives/controlled-signal` | `1.0.0-next.3` | 2/2 | 2/2 |
| `@solid-primitives/pagination` | `1.0.0-next.8` | 2/2 | 1/2 |
| `@solid-primitives/history` | `1.0.0-next.3` | 1/1 | 1/1 |
| `@solid-primitives/promise` | `2.0.0-next.2` | 1/1 | 1/1 |
| `neverthrow` | `8.2.0` | 1/1 | 1/1 |
| `zod` | `4.4.3` | 1/1 | 1/1 |

The preserved population scores **33/38 executed consumers**: fourteen
matching targets and nineteen quiet, working controls. **13/18 pairs** pass
both conditions. The failed pagination control remains in the denominator.

The channels overlap:

- **Runtime diagnostics match ten targets.** Seven are callback ownership
  failures through scheduling, event buses, controlled state, history,
  `neverthrow`, and Zod. Original source maps identify active frames inside
  the exact installed package files. The other three are untracked snapshots
  or a frozen reactive input.
- **Source assumptions match five targets.** These include three accessor
  coercions across set union, dates and pagination. The browser prints function
  text for those targets without issuing a runtime warning. Two snapshot
  warnings overlap the runtime channel.
- **Original-code native analysis matches two targets.** It catches the frozen
  pagination input and the read after awaiting inside `retry`, called from a
  native async memo. The retry target stays at `1` after the source becomes
  `2`; capturing the read before awaiting produces `2` in its control.
- Generic exceptions remain visible separately. Their presence alone does
  not count as matching a semantic rule.

These transfers required no package-specific detector changes. The scheduled
control also demonstrates that triggering a trailing debounce during component
setup can produce a valid later write; the synchronous leading callback is
diagnosed instead. The explicit `untrack` class snapshot remains quiet and
deliberately keeps the initial value.

## Four misses and what they establish

All four targets pass TypeScript, render stale output, and have working paired
controls. Their misses remain in the recorded score.

| Consumer | Observed failure | Current limitation |
| --- | --- | --- |
| Direct `ReactiveMap.get` snapshot | `1` after value becomes `2` | Existing class extractor records `SOURCE_CLASS_SNAPSHOT_FLOW`, but the combined detector omits class candidates |
| Literal computed `ReactiveMap['get']` snapshot | `1` after value becomes `2` | Computed member flow lacks an admitted exact target; original analysis retains `reactive-dispatch-unresolved` |
| `ReactiveSet.size` snapshot | `1` after size becomes `2` | Existing class extractor records a candidate that the combined detector omits |
| `createMousePosition().x` snapshot | `0` after the mouse moves to `42` | Getter extraction encounters unresolved spreads, object paths and a source budget; the model remains open |

The direct class candidates are informational evidence with undeclared liveness
intent. Displaying them would provide useful feedback on two misses, but would
not turn them into proven violations. The candidate's presence also does not
resolve the computed-member case. The runtime alone misses these observer-gated
reads, so treating a quiet browser run as certification would be wrong.

The next improvements should be generic:

1. Connect the existing class candidates to the feedback display, preserving
   their information severity, source evidence and intent limitation.
2. Resolve computed members through exact declarations and stable receiver
   identity. Refuse ambiguous dispatch instead of trusting a member name.
3. Extend bounded getter extraction for supported object construction and
   spreads, retaining explicit gaps when keys or execution paths remain open.

Any revised detector should keep this frozen result as its baseline and be
tested on another fresh population. Fixing these four cases would measure
improvement on this corpus; it would not establish universal coverage.

## The pagination control exposed a package defect

The correct-form consumer passes the accessor to
`createSegment(items, 2, () => 1)`. Replacing `[1, 2]` with `[3, 4]` still renders
`1,2`. The target that passed `items()` also stays stale, but its untracked read
is correctly diagnosed. The two reasons for staleness must remain distinct.

The installed `createSegment` reads the current items, then returns its previous
slice when its start and end boundaries are unchanged. That branch does not
check whether the source contents changed. Three separate browser probes
confirm the result:

| Probe | Update | Actual output |
| --- | --- | --- |
| Package memo, same length | `[1,2]` to `[3,4]` | `1,2` |
| Package memo, different length but same slice boundaries | `[1,2]` to `[3,4,5]` | `1,2` |
| Native `createMemo(() => items().slice(0, 2))` | `[1,2]` to `[3,4]` | `3,4` |

All three pass the published typings, complete execution, and receive no
feedback. The package files were preserved. This is a package implementation
defect in the observed version, beyond the tested misuse diagnostics. A system
that reports use errors cannot promise to detect arbitrary incorrect package
algorithms without a separate behavioral specification or invariant.

## TypeScript boundary

The readonly set mutation is rejected with **TS2339**: `add` does not exist on
`ReadonlySet<number>`. The invalid date initialization is rejected with
**TS2345**: a boolean is not a `MaybeAccessor<DateInit>`. Both are excluded
before browser execution and refused by the static runner. The combined
detector returns no feedback or gaps for them.

## Verification, cost and retained artifacts

- **95/95 prototype tests pass**, including three new checks that prevent
  unrelated warnings, broken controls and typing exclusions from inflating
  the score. Syntax checks pass for all 160 prototype modules.
- Forty main records and three package probes were run against the retained
  published packages. Thirty-eight main consumers execute; two are excluded.
- Both native analyzer variants completed for every valid main consumer.
  Frozen input, declaration, source, flow and package checks pass.
- `make verify-fast` passes producer freshness, Rust formatting and pinned
  workspace Clippy. Schema validation, dialect manifest validation and
  `git diff --check` pass.
- Full `make verify`, coverage, ownership and contract certification gates
  were deferred because this adds isolated research consumers and evaluation
  code. No production Rust rule, bundled contract, finding snapshot or
  generated public manifest changed.

The browser report spans **35.6 seconds**, including launch, per-consumer
published-type checks and execution. The static runner took approximately
**9 minutes 16 seconds**, estimated from output-directory creation to its
reported finish. It repeatedly constructs independent programs and runs two
native analyses per valid consumer. These are prototype batch costs, not
editor latency measurements; interactive scalability remains unmeasured.

One original-consumer probe with `SOLID_CHECKER_TIMINGS=1` reports **6.65 seconds
overall**, including **2.14 milliseconds in reactive IR**. Its findings match
the initial run exactly. The reported phases do not explain most of the total,
so the remaining overhead needs separate measurement. Reducing repeated fresh
processes and measuring project session reuse are reasonable next experiments;
this observation does not establish their speedup.

The retained, ignored research artifacts are:

- `rust/target/family-holdout-detector-freeze.json`
- `rust/target/family-holdout-preflight/population.json`
- `rust/target/family-holdout-browser/browser/results.json`
- `rust/target/family-holdout-static/results.json`
- `rust/target/family-holdout-package-browser/browser/results.json`
- `rust/target/family-holdout-validation.json`
- `rust/target/family-holdout-cost.json` and the saved timing output

The experiment and revalidation commands are documented in the
[prototype README](../../../benchmarks/reviewed-package-models/README.md).
The four misses, package defect, source assumptions and unexercised environments
remain explicit. This result supports a combined feedback system, with broader
package and rule coverage still requiring evidence.
