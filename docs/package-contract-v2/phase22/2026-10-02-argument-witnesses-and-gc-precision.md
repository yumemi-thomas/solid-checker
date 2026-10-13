# Argument witnesses and garbage-collection precision

The ownership sample now has **77 exported primitives across 37 packages** with
an observed unowned diagnostic and a valid owned control. This combines the
previous seventeen zero-argument witnesses with sixty argument-bearing witnesses.
The new inputs come from published types and shared generation rules, without
per-package argument recipes, casts, or fixture stubs.

This establishes broader construction ownership feedback. Returned values,
deferred operations, all option branches and other classes of misuse remain
outside this count. These are development observations and source assumptions;
no production rule, accepted package contract or certification authority changed.

The precision tests also found and fixed a real lifetime-monitor false warning:
a collected event target was still reported as having an active listener.

## Type-generated consumers

The source catalog's 91 browser ownership premises span 42 packages among the
97 retained roots. Seventeen signatures allow zero arguments. A new bounded
generator addresses the remaining 74 exports:

| Measure | Result |
| --- | ---: |
| Argument-bearing exports considered | 74 |
| Exports with an admitted strict published-type witness | 67 |
| Packages with admitted witnesses | 32 |
| Exports with a runtime ownership diagnostic observed | 65 |
| Exports with a valid unowned/owned pair | 60 |
| Packages with those valid pairs | 29 |
| Distinct argument inputs executed in validated studies | 77 |
| Browser executions in those studies | 154 |
| Published typing errors in executed consumers | 0 |

The generator resolves exact exported declarations and required argument counts.
It generates bounded literals, unions, arrays, tuples, functions and structural
objects. DOM targets and native scheduling functions come from shared seeds
checked against real declaration types. Optional property and nonempty-array
profiles give alternate witnesses. Generic function inputs prefer generated
callbacks; native scheduler seeds require the exact type identity.

Every generated call is checked against actual installed declarations in strict
mode. Type-invalid candidates are refused, and never become checker findings.
Candidate rejection diagnostics, package closure pins, declaration hashes,
source premises and selected inputs are retained. Types establish whether a call
is allowed; they cannot establish its runtime domain or execute its branches.

Adaptive retries use only previously admitted inputs. They skip inputs already
executed and separate module-load failures from failed invocations. This is a
discovery sample selected partly by its observations, not an independent holdout.
Construction is exercised; returned accessors and most callbacks are not invoked.

### Results that remain unverified

Seven admitted exports do not have a valid paired observation:

- `date.createCountdown` and `createCountdownFromNow` emit
  `REACTIVE_WRITE_IN_OWNED_SCOPE` in owned controls with string, numeric and Date
  inputs. A further callable overload of `createCountdown` also raises it.
- `date.createTimeAgo` and `createTimeDifferenceFromNow` log
  `Cannot access 'diff' before initialization` with string and numeric inputs.
  Those console exceptions are retained even though the attempt itself returns.
- `keyed.Key` and `keyArray` cannot load their cached `@solid-primitives/utils`
  import. Their consumer modules never execute. An older harness then attempted
  to call an absent disposer, producing `fn is not a function`; that was a
  harness consequence, not evidence about the primitives.
- `sse.createSSE` receives HTTP 404 from the unsupplied local event-stream
  endpoint. This observation cannot establish a valid network control.

The date errors are execution observations inside dependencies. They are not
classified as incorrect application ownership or as conclusively diagnosed
package defects. One otherwise valid timer control emits `EFFECT_RELAY_TEAR`;
the advisory is preserved and does not become an ownership violation.

The first MutationObserver witness, `{}`, passes TypeScript but violates a
runtime option requirement. A shared optional-property profile produces a valid
observation with an unowned warning and an owned control. This is a concrete
reason to keep type admission separate from behavioral validation.

Seven other exports remain outside bounded synthesis: analytics' guard,
pagination's infinite scroll, `Range` and `IndexRange`, the file uploader, and
the websocket message/store constructors. Required shapes, intersections,
nominal resources and callback types can exceed the generator's bounds. Refusal
does not mean the API is safe or impossible to check.

## Whole-project static feedback

The first synthetic project links the exact retained package roots for type
resolution and contains 67 unowned calls plus 67 owned controls. It executes no
package code and does not establish runtime graph deduplication.

| Profile | Calls | Ownership warnings | Owned ownership warnings | Preparation | Modeled analysis |
| --- | ---: | ---: | ---: | ---: | ---: |
| Base source premises | 134 | 63 | 0 | 1.830 s | 0.944 s |
| Demand-specialized source premises | 134 | 63 | 0 | 3.112 s | 0.965 s |
| Demand profile on the 60 validated runtime witnesses | 120 | 58 | 0 | 2.592 s | 0.942 s |

All original and analysis sources pass strict published typing. Warnings retain
the `source-extracted-assumption` basis and have no certification authority.
Initial package authentication before the preparation timer is excluded.
These are individual measurements on small synthetic projects, not editor
latency or large-application benchmarks.

The first two profiles leave four exports unmodeled: `createRootPool`,
`createIntervalCounter`, `createPolled` and `createMicrotask`. TypeScript redirects
their declarations to different retained physical installations with the same
package identity. The adapter preserves its exact artifact boundary and records
these refusals. It does not trust a matching package name/version.

Focused runs in each export's own retained installation detect all four, and
their four actively modeled owned controls stay quiet. These eight observations
use sixteen fresh baseline/model analyses. In the final 60-witness batch, only
`createRootPool` and `createMicrotask` retain the declaration redirect; their
focused observations supply the remaining two static cases with the same inputs.
This establishes static ownership feedback for the sampled witnesses while
leaving declaration composition across separate installations open.

An early debug build's slow per-consumer run was interrupted and its partial
outputs retained. The whole-project measurements use the same cached release
binary as the preceding ownership study. These timings are not compared as
equivalent builds. An initial broad browser run also stopped on the
case-insensitive `Repeat`/`repeat` output-directory collision. Export hashes now
make those IDs distinct. Neither incomplete run enters the validated table.

An empty retry selection once ran the browser harness's default cases. Those
unrelated results are excluded. The runner now refuses an empty selection before
launching a browser, with a regression test for that boundary.

## A garbage-collected listener must stay quiet

The browser test registers a listener through the real package wrapper and keeps
only a `WeakRef` to its target. Two native GC requests run in separate jobs.
The test verifies the target is gone before ending the explicit lifetime scope.
It also tests a held target and an explicitly cancelled listener.

| Control | Original behavior | Previous monitor | Corrected monitor |
| --- | --- | --- | --- |
| Target confirmed collected | Collected | Spurious lifetime warning | Quiet; stale resource retired |
| Target held | Event delivered once | Lifetime warning | Lifetime warning; event delivered once |
| Listener cancelled | Collected | Quiet | Quiet |

All nine observations pass actual published typing. The previous registry kept
resource metadata after its weak EventTarget key had disappeared. Resources now
carry weak endpoint references and are retired when collection is observed.
Observer target indexes also use weak keys so the monitor does not add a strong
reference to those targets. Missing `WeakRef` produces an explicit gap instead
of a lifetime warning. Native cancellation and callback identity stay unchanged.

The original/native automatic lifetime comparison was rerun after this change.
Its controlled profile still detects **14 of 14 targets** across six packages;
thirteen controls receive no new lifetime warning. The cached app flow and native
Promise scheduling controls retain their measured behavior. That is another
56 browser executions. An earlier replay had a differing initial Intersection-
Observer delivery count; the flow now waits for initial resize/intersection
delivery before disposal. Both replays are retained, and only the controlled
comparison passes the full behavior validator.

Lifetime warnings still depend on an explicit project policy about work ending
with its creating owner. Background work, unobserved native cancellation,
unsupported resource kinds and async boundaries remain separate open cases.
The owner binding still depends on the audited rc.9 ABI. This is a correction
to the prototype, not a claim of universal lifetime precision.

## What this supports

Shared Solid diagnostics can report actual ownership/phase failures inside many
packages without per-package contracts. Source premises can provide earlier
conditional warnings, while shared resource observation adds cleanup feedback.
Published types can generate a useful test population at low authoring cost.
The experiments also show why a quiet trace, an admitted call or a source premise
alone cannot establish complete package coverage.

The total of 77 primitives spans 37 of the 97 retained roots, and one narrow
misuse family. Most-package coverage and package-specific semantic mistakes
remain unproven. Dynamic options, generic callbacks, returned methods, SSR,
hydration, multiple runtime copies, HMR, other bundlers and arbitrary async
continuations still need broader tests or explicit evidence.

## Verification and evidence

- Seventy-two experimental unit tests pass, including the type witness,
  missing weak reachability and empty-selection regressions.
- Frozen browser inputs, package/declaration identities and published typing
  are validated. Historical source bytes are preserved, and later source changes
  are recorded explicitly rather than described as unchanged.
- `make verify-fast` passes pinned producer freshness, Rust formatting and
  workspace Clippy. Schema JSON, dialect manifests, source syntax and whitespace
  checks pass. Full verification, coverage, ownership gates and certification
  probes are deferred because production analyzer and fixture behavior did not
  change.
- No accepted contracts, schemas, manifests or finding snapshots changed.

Local evidence includes `rust/target/argument-witness-validated-final.json`, its
successful selection, the four validated argument browser studies,
`argument-witness-batch-closed`, `argument-witness-batch-demand`,
`argument-witness-confirmed-batch`, `argument-witness-static-identity-controls`,
the three `gc-lifetime-*` studies, and
`automatic-life-GC-safe-controlled-validated.json`. The successful selection
records construction-only observations and has `authority: false`.
