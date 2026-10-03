# Capture replays and feedback from failing tests

## Outcome

An isolated replay can help explain a failing test without authoring a package
contract. Exact source bindings produced 20 capture proposals across two
authored populations. Ten changed the asserted task result to the expected
value. Ten controls retained their already-correct task result.

A fresh counterexample prevents a broader claim: two legitimate execution
counters change from one task execution to two after capture, although the
task result remains correct. A changed screen alone is insufficient evidence
of a defect. The proposal changes execution timing, function identity and
possibly side effects; it is not a safe automatic fix.

The promising role is feedback attached to a specific failing test:
“Capturing this read during the memo made this value assertion pass in the
isolated replay.” This is an observed debugging fact. It remains informational,
intent-open, `authority:false` and `certification:false`. It does not establish
a general package rule, complete test coverage or safe repair.

## Source proposal and isolated execution

`capture-replay-plan-v1.mjs` reconstructs an exact route from an observed
registered async callback's native read to a caller's getter alias. It checks:

- current source hashes, exact declaration spans and matching witness revisions;
- the callback returned directly by one exact source factory;
- an unmodified factory parameter and exact arguments through witnessed helper
  calls, including an awaited child;
- one const getter alias initialized directly in the native memo compute;
- an unmodified, non-generic getter with one zero-argument signature and a
  definitely primitive declared result;
- absence of binding writes and exact built-in direct eval in the public
  source program, and a fresh private capture identifier.

Names do not confer permission. Renamed factory exports, consumer aliases and
helper parameters work through exact symbols. Shadowed, ambiguous, written,
nested, generic, object-returning and optional-parameter getter shapes remain
explicitly open. These checks admit a proposal, not a behavioral certificate.

The proposal replaces only the getter alias's initializer in a generated copy:

```ts
const read = get;
// Proposed isolated experiment:
const read = (() => { const captured = get(); return () => captured; })();
```

This calls the getter during the memo compute and passes a new closure to the
original package. Package bytes and the original consumer remain unchanged.
The original interaction is replayed against real published declarations and
the retained queue/Solid runtime. Remote requests remain blocked. No proposal
is applied to a user's application or accepted as an autofix.

Preparer V1 and a first 16-case replay supplied expected values in the browser
plan. Their artifacts remain preserved as a preliminary trial. Preparer V2
and browser V22 remove that oracle: initial and final values are measured,
`desired` is null, and the cases have a separate `replay-only` role. The
original authored assertion is consulted afterward when explaining the result.
This prevents an expected-answer check from selecting the measured outcome.

Browser V21/V22 also record visible page text. Independent browser audits
V13/V14 compare that text, values, contexts, native diagnostics, consumer
counters, public/source typing programs, input manifests and source revisions
with plain runs. Capture audit V2 independently reconstructs source bindings
and the proposed edit; it imports no planner, selector or feedback projector.

## Results and counterexample

| Population | Original consumers | Observed reads | Source proposals | Asserted results changed to expected | Already-correct primary results unchanged | Source refusals |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Earlier body-return challenge | 32 | 16 | 16 | 8 | 8 | 0 |
| Fresh binding/counter challenge | 8 | 8 | 4 | 2 | 2 | 4 |

The first population is an adapted replay of the preceding experiment. Its
four late-child/reaction targets have no observation and receive no proposal.
The fresh challenge was authored after the source planner/visible browser
profile was frozen. Two renamed-binding targets work. Object-returning and
optional-parameter getter targets remain refused in both queue variants.
All six fresh targets retain the earlier raw informational hint, but only two
have an admitted replay proposal.

The fresh visible-counter controls return `{value:9}` after reading and
recording a task execution. Capture leaves the primary result at 9 while the
screen changes from “Executed 1” to “Executed 2”. All four admitted fresh
proposals change visible text; only two correspond to a failing primary-value
assertion. General UI change is therefore rejected as a diagnostic criterion.

Task execution counts change in 18/20 proposals. The two never-settling
controls retain one execution and a pending display. Throwing `then` getters
execute twice after capture in two controls. Native diagnostics remain
unchanged. This is direct evidence that equivalent primary results do not
establish equivalent effects or a safe repair.

The 16-proposal instrumented batch takes 26.6 seconds and its plain batch
16.4 seconds. The four fresh proposals take 7.1 and 6.4 seconds. These are
single concurrent development observations, including browser/UI waits and
source analysis; they are not isolated performance measurements or an editor
latency guarantee. The workflow is currently a candidate for test debugging,
not an automatic replay on each editor keystroke.

## Conditional feedback policy

`replay-assertion-feedback-v1.mjs` emits an informational suggestion only when
an explicit provided primary-value assertion fails before the proposal and
passes afterward. It reports the extra task executions and leaves repair
safety and intent open. A missing assertion, an assertion still failing,
an already-passing assertion or a broken assertion cannot produce that note.
Changed other visible output is retained as an explicit open result.

The report command uses the authored benchmark's original `desired` values;
it does not derive an answer from the repair or consult target/control roles
when selecting feedback. Independent assertion audits reconstruct the
decision from the paired measurements and the explicit assertions.

| Authored assertion population | Targets receiving conditional feedback | Controls receiving no conditional feedback |
| --- | ---: | ---: |
| Earlier 32 consumers | 8/12 | 20/20 |
| Fresh 8 consumers | 2/6 | 2/2 |

The 10 suggestions identify changes that satisfy those provided assertions.
The 22 controls stay quiet in this channel. Raw body-read observations and
their earlier noisy hints remain preserved. This policy was designed after
examining the populations and counterexample; it is not a held-out warning
precision estimate. The primary assertion does not cover all visible state,
effects, inputs or executions. Real applications still need their own tests
and review of the proposal's changed behavior.

## Verification and immutable artifacts

- All 734 prototype tests pass without skips: 703 previous tests, 23 exact
  source proposal checks and eight assertion-policy checks.
- All 28 distinct new source variants pass plain comparison and independent
  browser audits: 16 unconstrained older proposals, eight fresh consumers and
  four fresh proposals. The preliminary 16 comparisons are preserved
  separately; they are not additional distinct source variants.
- Independent pair audits validate all 20 admitted source routes, edits,
  original/replayed values, changed task counts and four visible comparisons.
  Independent assertion audits validate the 10 conditional notes and explicit
  open cases against authenticated inputs.
- The actual TypeScript CLI passes all 28 strict projects using real published
  typings, with `--noEmit --incremental false`.
- All 512 prototype modules pass syntax checks. The pre-handoff audit
  authenticates 58 historical/current seals and 516 distinct pins.

All earlier modules remain immutable. New modules under
`benchmarks/reviewed-package-models/` include the planner/tests, preparers
V1/V2, preliminary/unconstrained/fresh case populations, browsers V21/V22,
browser audits V13/V14, independent capture audits V1/V2 and assertion
feedback/report/audit modules. Native runtime V10, plugin V24, callback models
and the earlier feedback projector remain unchanged.

Ignored artifacts under `rust/target/` include:

- `capture-replay-{prepared,unconstrained-prepared,challenge-prepared}-v1.json`;
- `capture-replay-browser-{reads,plain}-v1/results.json` for the preliminary
  trial, and its browser/pair audits;
- `capture-replay-unconstrained-{reads,plain}-v1/results.json`, browser and
  pair audits;
- `capture-replay-challenge-{reads,plain}-v1/results.json`, and
  `capture-replay-challenge-repaired-{reads,plain}-v1/results.json`, with audits;
- both assertion-feedback reports and their independent decision audits;
- three detector seals, the handoff seal, typing report, final test/syntax logs,
  historical seal report, combined summary and fast handoff log.

Fast handoff uses `make verify-fast` for the producer stamp, Rust formatting
and pinned workspace Clippy, plus schema validation, dialect manifest
validation and `git diff --check`. Full `make verify` and production
process/coverage/ownership/contract/release gates are deferred for this
research/documentation slice. No production Rust, CLI behavior, compiler
lowering, snapshots, contracts, schemas or manifests change.

## Remaining work

This does not replace package contracts for static proof. It offers a separate
route to useful feedback where tests supply an observable assertion. General
source discovery, package storage, CJS/prebundling, framework/server profiles,
native callback coverage, result/error flow, arbitrary getters and long-session
memory bounds remain open. The previous real-app study still contains no
positive application defect.

The next useful trial should use genuine failing application tests and check
additional assertions and side effects after each proposal. A replay should
explain a measured change, and the existing static/native evidence should
explain why that change is relevant. Neither should silently turn a partial
observation into a certified violation or a safe automatic repair.
