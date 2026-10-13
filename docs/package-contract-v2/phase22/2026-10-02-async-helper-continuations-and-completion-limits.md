# Async helper continuations and completion limits

## Result

The experiment can now attribute a delayed read inside an admitted source async
helper to the memo callback that started it. It carries an observation token
through the helper's own source body. It preserves the actual Solid owner and
observer at the read and adds no await or Promise reaction.

This catches **all five previously missed delayed-read revisions**. A broader
36-consumer challenge authored after the detector freeze catches **10/15**
targets and leaves **17/19** working controls quiet. The five misses and two
noisy controls are retained counterexamples. An additional safety refinement
preserves property references; ten new browser helper variants give **5/5**
targets and **5/5** quiet controls.

| Population | Targets with hints | Quiet working controls | Typing exclusions | Plain comparisons |
| --- | --- | --- | --- | --- |
| Earlier 42 stages, adapted replay | 15/15 | 21/21 | 6 | 42 |
| Earlier 35 stages, adapted replay | 15/15 | 14/15 | 5 | 35 |
| Fresh async-helper challenge, 36 consumers | 10/15 | 17/19 | 2 | 36 |
| Fresh reference-preservation challenge, reused paired consumer setup | 5/5 | 5/5 | 0 | 10 |
| Combined stages and helper variants | 45/50 | 57/60 | 13 | 123 |

The first two populations repeat revisions of eleven earlier consumers. The
last population reuses two consumer setups with five new helper bodies. These
are **123 tested stages and helper variants**, not 123 independent application
defects or an estimate of coverage across all packages. The previous
constant-result study is separate and is not added to these totals.

All **123 plain/observed comparisons** preserve the tested displayed values,
source hashes, real typing diagnostics, caught errors and native diagnostic
deliveries. The 46 new comparisons also check recorded getter contexts and
helper counters. There are no browser page errors or harness failures. The
adapted runs retain **66 automatic full reloads** and refuse **108 nonempty
old-observation batches**.

These results support continued work on scalable package feedback. They also
show that an executed read plus a successful helper return is insufficient to
prove that the read's value affects the output. Feedback remains informational
`intent-open`, with `authority: false`, `certification: false` and open static
dispatch; it is not a proven violation.

## Carry attribution through source, preserve execution context

The earlier synchronous call scope ended before an async helper resumed. The
new source selector enrolls async function declarations, expressions, arrows
and methods with real bodies. It requires exact declarations for admitted
calls and named property reads. Functions and operations carry their complete
source hashes and original spans. Names alone grant no trust.

The transform captures a per-invocation token only when the helper actually
enters an active memo-call observation scope. The token records that consumer,
the issued project revision, the helper and its entry frames. Each admitted
operation briefly establishes an observation scope around the original
synchronous expression, then restores it before suspension. Nested synchronous
function bodies do not inherit the token. Nested async helpers can form a
chain when they actually enter from an observed operation.

The runtime observes actual native reads and package observer shortcuts under
the same retained native and package models as before. It never restores a
Solid owner or observer. In the new browser targets, the original getter
records `owner: false` and `observer: false`; those recordings agree exactly
with plain execution. Working controls capture their primitive during memo
computation and pass a getter over that captured value into the helper.

Read tickets remain pending until the helper finishes with an explicit normal
primitive return. The transform records original returns, escaping throws and
final completion. A later `finally` return replaces the recorded value; a
`finally` throw discards the tickets and rethrows the original error. Object,
function, Promise and implicit completions leave result flow open. The runtime
checks the returned value's primitive kind without accessing its `then` property.

Awaited child helpers can pass their tickets to a still-open parent, requiring
each parent to complete in the admitted way. Returning a child Promise without
awaiting it leaves the parent completion unresolved; the later child does not
reopen that parent. Each token admits at most 64 pending tickets. Exhaustion
discards feedback while preserving all original calls and records an open gap.
This bound does not bound the global research event and metadata buffers.

The projector first checks the current issued source revision. For continuation
events it then requires the exact current helper and operation declarations,
the same helper/consumer revision, mapped helper-entry and read frames, mapped
original consumer entry, and a completed primitive-return chain. It passes the
accepted evidence through the existing source/read projector. The entry frame
supplies provenance that the native stack no longer retains after await.

## Safety issue found during handoff

An additional valid TypeScript reproducer exposed a transform defect:
`box.value = 2` was treated as a property read and replaced with a value-producing
call. Plain execution returns 2; the instrumented version throws
`ReferenceError: Invalid left-hand side in assignment`. This is a prototype
instrumentation defect, not a package misuse or a TypeScript finding.

The source selector V2 preserves properties used as assignment, update, delete
or tagged-template receiver references, including transparent TypeScript
wrappers and destructuring targets. These operations produce explicit open
records rather than being rewritten as reads. Right-hand calls remain eligible
for observation. The refinement uses TypeScript's assignment-target facts and
the original AST reference position.

Fourteen focused tests compare actual values and getter/setter traces for
assignment, compound/logical assignment, prefix/postfix increment, array/object
destructuring, loop assignment, type wrappers, delete and receiver use. Five
new helper bodies exercise assignment, compound assignment, postfix increment,
delete and a tagged receiver in real Solid browser consumers, each with a
working captured-value control. All ten comparisons pass, with five hints and
five quiet controls. The deliberately type-invalid delete assertion from an
initial test attempt was corrected before scoring; TypeScript owns that error.

The historical V1 detector and execution evidence remain unchanged. A separate
same-program comparison checks **242 source-module transforms across all 113
earlier stages**. V2 emits byte-identical code and maps, and identical sites and
helper metadata for those inputs. This is transformation equivalence, not an
additional browser replay. The ten new browser variants execute the revised
V16 plugin and V12 projector.

## Counterexamples kept in the score

The fresh challenge's ten detected shapes include a delayed read, two awaits,
an async arrow, an awaited helper chain, an async object method, a branch,
normal `finally`, a cross-file child helper, a default parameter and concurrent
invocations. The concurrent case preserves both calls; repeated reads from the
same origin, operation and native node can coalesce with two occurrences.

Five valid, demonstrably stale consumers still receive no hint:

- A parent returns the child's Promise without awaiting it.
- The read runs inside a nested synchronous Promise reaction.
- The helper returns an object containing the read value.
- An async generator yields the read value.
- Seventy reads exhaust the 64-ticket observation budget.

Working paired controls stay quiet for all five shapes. They remain targets
and controls in the denominator; the open boundaries are not relabeled as
typing exclusions.

Three working stages receive noisy hints: the adapted async constant helper,
a fresh helper that reads and returns 9, and a fresh helper whose `finally`
returns 9. The earlier synchronous constant-result proof deliberately rejects
async bodies. Primitive completion establishes completion provenance, not
read-to-result flow. These cases motivate a separate bounded result-flow proof.
Explicit native `untrack` and a throwing `finally` remain quiet in the fresh
challenge.

## Actual packages and TypeScript boundary

The retained published packages are `@solid-primitives/map@1.0.0-next.2`,
`@solid-primitives/set@1.0.0-next.2` and
`@solid-primitives/controlled-signal@1.0.0-next.3`, with the audited Solid rc.9
runtime. Map/set exercise the package observer-shortcut channel; controlled
signal exercises an actual native reader. All consumer and helper bodies are
explicitly authored research source. This does not audit arbitrary external
package async implementations.

Complete TypeScript programs use the retained published declarations. Eleven
adapted stages have the earlier `TS2322` helper error; the two new exclusions
produce `TS2769` for the invalid Solid signal argument and `TS2345` for the
invalid map key. All thirteen receive zero hints and zero suppressions. Live
Vite can still execute their erased JavaScript; they are excluded from checker
feedback and target/control scoring, not claimed to be prevented from running.

Eight explicit `tsc --noEmit` checks give six clean programs, including missed
and noisy cases, and the two expected typing exclusions. The reference tests
and browser variants also use complete real typing programs. No declaration
stub is broadened, no package installation is performed and published inputs
remain unchanged.

## Independent audit and verification

Audit V4/V5 imports no selector, transform, session or projector. It reconstructs
each stage's TypeScript program from the authenticated original source and real
declarations, verifies exact async function/operation spans and resolved
declarations, validates entry/read frames and completed-chain provenance, checks
issued revisions and retired batches, and compares plain behavior. V5 also
compares the getter context and helper-counter recordings.

Across the four populations it audits **54 observations**, including **24 async
observations and 26 invocation links**, plus **five existing suppressions**.
Native semantic models and the collected execution traces remain research
premises; the audit does not establish a new certification root for them.

Verification passes:

- **432/432** prototype tests with no skips, including 21 continuation tests
  and 14 reference-preservation tests.
- **400** benchmark module syntax checks.
- **24** historical/current seals authenticating **402** distinct pins unchanged.
- `make verify-fast`: matching producer reuse, Rust formatting and the
  certification/probe-pinned workspace Clippy run.
- Schema parsing, dialect manifest validation, document links and whitespace.

The fast handoff run precedes the JavaScript-only reference refinement; the
unchanged Rust/producer inputs do not justify repeating it. Full `make verify`,
production coverage/ownership, contract corpus and certification gates are
deferred for this research slice. No production Rust rule, compiler lowering,
public contract, manifest, fixture stub or finding snapshot changes.

## Remaining scope

External package async bodies outside the current program and project-source
transform boundary remain open. Computed/optional/`this` dispatch, immediate
suspension in call arguments, generators, direct eval, nonprimitive completion,
arbitrary Promise adoption and nested callback timing remain outside the
admitted continuation profile. Preserved reference operations themselves do
not acquire a read witness.

Read-to-result flow and developer intent remain open even where provenance is
accepted. Function-source inspection, state-preserving HMR, late completions
across retained runtime instances, concurrent module serving, default dependency
prebundling and mixed revisions require further work. Source revisions refuse
unsupported evidence; this does not establish coverage for every such lifecycle.

Large-project overhead, global buffer bounds, real-application precision and
production/editor integration are unmeasured here. The mechanism is promising
because it reuses source and native execution facts across package values
without a new helper-specific trust contract. Universal package or rule coverage
is not established.

## Evidence and implementation

Current source modules are under [reviewed-package-models](../../../benchmarks/reviewed-package-models/README.md):
`async-continuation-sites-v2`, `async-continuation-transform-v2`,
`async-read-transform-v16`, `native-read-runtime-v5`,
`native-read-feedback-v12`, `feedback-revision-browser-v6` and independent
`feedback-revision-audit-v5`. Earlier executed versions remain available.

Generated research artifacts are retained under `rust/target/`:

- [Combined summary](../../../rust/target/async-continuation-combined-summary-v1.json).
- [Adapted 42-stage audit](../../../rust/target/async-continuation-replay-v1-audit-v1.json)
  and [adapted 35-stage audit](../../../rust/target/async-continuation-replay-v2-audit-v1.json).
- [Fresh 36-consumer audit](../../../rust/target/async-continuation-fresh-audit-v1.json)
  and [fresh reference audit](../../../rust/target/async-continuation-reference-audit-v1.json).
- [Reference transformation equivalence](../../../rust/target/async-continuation-reference-equivalence-v1/results.json)
  and [initial assignment defect](../../../rust/target/async-continuation-write-probe-ZVQKj2/result.json).
- [Explicit TypeScript checks](../../../rust/target/async-continuation-tsc-v1/results.json).
- [All tests](../../../rust/target/async-continuation-all-tests-v2.log),
  [fast handoff](../../../rust/target/async-continuation-verify-fast-v1.log),
  [syntax checks](../../../rust/target/async-continuation-syntax-v2.json) and
  [seal authentication](../../../rust/target/async-continuation-historical-seals-v2.json).

Each audit pins its observed/plain browser reports and case module. Three new
detector seals, before the initial delayed-read challenges and the later
reference challenges, retain the exact module bytes. Initial unit harness
failures are preserved in their versioned logs; they do not count as passing
tests. Package installations, production snapshots and earlier reports are
unchanged.
