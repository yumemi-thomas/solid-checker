# Async body returns and warning precision

## Conclusion

There is reason for cautious optimism about useful package feedback. The
earlier queue population now has 18/18 target hints. A fresh challenge shows
the limit of that result: broader observations also produce more unwanted
hints. This experiment establishes a useful observation boundary; it does not
establish a general warning policy or coverage of every package.

All feedback remains informational, conditional on reactive intent, with
`authority:false` and `certification:false`. No production behavior changes.

## Change and evidence strength

Frozen runtime V9 required an explicit primitive return from every async helper
in a chain. Successor V10 also admits explicit object and function returns.
These have a weaker claim: the function body returned normally after the read.
Promise settlement and result flow remain unproved. A returned Promise may
reject, and a returned thenable may fail while the language adopts it.

Primitive chains retain `explicit-primitive-normal-return`. Any nonprimitive
link produces `explicit-normal-async-body-return`, with
`promiseSettlement:'unobserved'` and `resultFlow:'unproved'`. A nonprimitive
registered callback has a corresponding body-return label. A primitive parent
does not erase an object child's weaker evidence. Projectors validate the
grade against every observed return kind and reject inconsistent claims.

The runtime does not inspect `then`, add a Promise reaction, add an `await`,
restore Solid context, or replace an original return value. Throws, implicit
completion, failed registrations, exhausted observation budgets and children
finishing after their parent remain closed. Admission of a body observation
does not establish that the read affected the consumer's result.

New research modules live under `benchmarks/reviewed-package-models/`:

- Native runtime V10, callback runtime V5, continuation transform V7 and
  plugin V24 transport the weaker evidence.
- Feedback V16/V17/V18 preserve ordinary helper, constant-result and registered
  callback projection, respectively.
- Browser V20 and independent audits V12/callback V4 distinguish the grades.
- `async-body-return-v1.test.mjs` adds 21 focused runtime checks.
- `async-body-return-cases-v1.mjs` supplies fresh consumer challenges after
  the detector freeze; projection check V3 tests two evidence grades.

Earlier modules and their seals remain unchanged. Published package bytes
remain unchanged: the queue is `@solid-primitives/queue@1.0.0-next.3`, using
the retained Solid/signals/web RC.9 development runtime and real declarations.

## Browser results

| Population | Previous target hints | Current target hints | Previous quiet controls | Current quiet controls | Typing exclusions |
| --- | ---: | ---: | ---: | ---: | ---: |
| Earlier async queue, 46 variants | 14/18 | 18/18 | 24/26 | 24/26 | 2 |
| Earlier ordinary helpers, 36 variants | 11/15 | 12/15 | 19/19 | 19/19 | 2 |
| Fresh body-return queue, 32 variants | 0/12 | 8/12 | 20/20 | 12/20 | 0 |

The previous scores for the first two populations come from the preceding
runtime V9 study. Both are adapted replays. The fresh population was authored
after the current detector freeze and also executed with the preserved V9
profile and without instrumentation. It is an authored challenge, not a
representative package or application sample; these ratios are not population
accuracy estimates.

The four remaining older queue targets now emit hints: serial/concurrent
object results and direct Promise adoption. Ordinary helpers gain their
object-result target. The new queue challenge gains hints for Promise-wrapped
objects, a thenable whose read occurs before return, function objects and
awaited object children. It still misses serial/concurrent adopted children
that finish after their parent, and reads inside Promise reactions.

All eight noisy fresh controls are retained in the result: constant objects,
caught rejected Promises, caught throwing `then` getters and never-settling
Promises, each in serial and concurrent queues. They have real untracked
reads and normal body returns. The observation alone does not prove a stale
fulfilled value or a need to update that value. Relabeling the evidence avoids
a false claim of settlement but does not make these hints desirable warnings.
All 12 captured-value controls remain quiet. The two older queue constant
controls remain noisy. General queue-result suppression remains open.

The current profile records 51 observations across the three populations:
30 primitive-return and 21 body-only. There are two independently reconstructed
constant-result suppressions in ordinary helpers. No fresh constant/error
control is silently removed to improve the score.

## Verification and artifacts

Independent audits reconstruct the original public typing programs, the
separate runtime-source programs, exact declarations and spans, issued input
revisions, source hashes, registered identities, mapped entry/read frames,
completion grades and constant-result proofs. They authenticate package and
detector inputs and compare visible results, native diagnostics, contexts and
consumer counters with the plain runs. Native runtime models and captured
traces remain research premises; the audits do not certify complete discovery
or developer intent.

- 114 distinct variants have matching plain behavior: 46 prior queue, 36 prior
  helpers and 32 fresh cases. The 32-case previous-profile comparison also
  passes its independent audit against the same plain population.
- All 703 prototype tests pass, with no skips, including 21 new runtime checks.
- All 32 fresh strict consumer projects pass the actual TypeScript CLI with
  `--noEmit --incremental false`, against retained published declarations.
- Two projector checks each refuse 31 malformed records and a retired
  revision. Their valid records are deliberately rebound to a newly issued
  test revision; these checks validate projection, not trace authenticity.
- All 494 prototype modules pass syntax checks. All 54 existing/current
  pre-handoff seals authenticate 503 distinct pins.

Artifacts under ignored `rust/target/`:

- `async-body-return-detector-freeze-v1.json` freezes 494 input files before
  the fresh challenge.
- `async-body-return-{prior-queue,prior-helper}-reads-v1/results.json` and
  matching `*-audit-v1.json` hold the replays.
- `async-body-return-fresh-{reads,plain,previous}-v1/results.json` and
  `async-body-return-fresh{,-previous}-audit-v1.json` hold the fresh comparison.
- `async-body-return-{primitive,body}-projection-check-v1.json`,
  `async-body-return-published-tsc-v1.json`, the unit-test log, syntax and seal
  reports hold focused validation.
- The handoff seal, combined summary and `verify-fast` log record final checks.

The universal handoff checks run through `make verify-fast`, plus schema JSON
validation, dialect manifest validation and `git diff --check`. Full
`make verify`, production process/coverage/ownership/contract gates and CLI
release checks are deferred because this slice changes research modules and
documentation. No snapshots, accepted contracts, schemas, dialect manifests,
Rust implementation or compiler lowering change.

## Remaining boundaries and next direction

The old helper misses are an adopted child, a nested reaction and an async
generator. The fresh misses are adopted late children and reaction reads.
Implicit returns, unresolved or ambiguous source operations, unsupported
storage shapes, failed registrations and budget overflow remain closed.
External source discovery, CJS/prebundling, mixed package hook profiles,
framework/server integration and installed-dependency updates remain partial.

The previous real-app trial remains a narrow unchanged application control:
39 matching UI states and two quiet candidate entries. This experiment adds
no genuine application defects, application-level precision estimate, startup
improvement or long-session memory bound. Global event and metadata retention
remain unbounded.

The practical direction is to retain these body observations as explainable
research evidence, then require result relevance and settled error/value flow
before promoting them into warnings. Source facts and exact runtime witnesses
can be shared across packages; unsupported behavior must remain explicit.
The next success criterion should include unseen defects and quiet controls,
rather than another perfect target count in an already familiar population.
