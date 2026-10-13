# Async constant results and completion proofs

## Result

The research projector now suppresses an observed async-helper hint when bounded
source evidence proves a constant primitive fulfilled value and the helper call
is the whole returned callback expression. The actual executed helper must match
that source function. It retains the original observation and the source model.
This extends the earlier synchronous constant-result filter without changing
the browser transform or restoring reactive context.

Replaying the earlier **123 stages/helper variants** retains all **45/50**
target detections and improves quiet working controls from **57/60 to 60/60**.
The three earlier async constant-result noisy hints disappear. The same five
continuation misses remain. These are adapted replays, not fresh validation.

A **40-case source challenge authored after the proof froze** gives **8/9**
target detections and **25/29** quiet controls. It exposes a further receiver-call
enrollment miss and four working shapes whose result flow remains unresolved.
The proof was not changed after seeing those cases. One check-only configuration
was corrected for the JavaScript input, as described below.

| Population | Targets with hints | Quiet working controls | Typing exclusions | Plain comparisons |
| --- | --- | --- | --- | --- |
| Earlier 42 revision stages | 15/15 | 21/21 | 6 | 42 |
| Earlier 35 revision stages | 15/15 | 15/15 | 5 | 35 |
| Earlier 36 continuation consumers | 10/15 | 19/19 | 2 | 36 |
| Earlier 10 reference variants | 5/5 | 5/5 | 0 | 10 |
| Fresh 40 helper/result-flow challenges | 8/9 | 25/29 | 2 | 40 |
| Combined stages and helper variants | 53/59 | 85/89 | 15 | 163 |

The earlier cohorts repeat consumer revisions and reuse paired setup. The new
cohort also reuses the controlled-signal consumer setup with different source
helpers and result expressions. These counts are **tested stages and helper
variants**, not independent application defects or package-wide coverage rates.
The earlier 151-case constant-result study is separate and is not added here.

All **163 plain/observed comparisons** preserve tested displayed values, source
hashes, actual published typing diagnostics, caught errors and native diagnostic
deliveries. The 86 comparisons in the continuation/reference/new cohorts also
check recorded getter contexts and helper counters. The older 77 plain reports
did not capture those fields; their absence is not treated as a context proof.
Completed runs have no page errors or harness failures. The adapted runs retain
**66 automatic full reloads** and refuse **108 nonempty old-observation batches**.

Feedback remains informational `intent-open`, with `authority: false`,
`certification: false` and open static dispatch. The source model establishes
only the helper's constant value on normal fulfillment. Side effects, rejection
behavior, dependency registration and developer intent remain open.

## Bounded completion evidence

`async-constant-result-v1.mjs` resolves the exact function through constant
bindings, transparent TypeScript wrappers, named imports and direct ES namespace
imports. Declaration-only functions, mutable aliases and ordinary object-member
dispatch do not receive this proof. Source function declarations require an
external module, no exact-symbol writes across the current program, and no
exact built-in direct eval in their module. The absence check records the
complete source-input hashes it inspected. No function name grants trust.

Only actual async functions with source bodies are admitted. Generators are
excluded. Primitive evidence covers literal numbers, signed numeric literals
including negative zero, strings, booleans, null, bigint and explicit undefined.
A literal under await can retain that value evidence. Identifier values,
objects, arbitrary calls and Promise adoption remain unknown.

The structured completion analysis handles blocks, returns, throws, branches,
try/catch and finally. Opaque expressions and variable/class statements admit
possible throws. Conditions admit both branches. Unknown statement forms carry
an open alternative; they do not become proof of a constant result.

A catch replaces possible throw outcomes with its own outcomes. A finally
outcome replaces an earlier exit unless the finally completes normally. Thus
`try { return read(); } finally { return 9; }` has constant normal fulfilled
value 9, while a conditional finally returning 9 leaves the original unknown
return possible. A normal finally preserves the earlier result but can still
throw. Nested returns inside other functions do not return from the helper.

The model requires every possible normal fulfilled value to be the same known
primitive. It treats function fallthrough as undefined and refuses a model with
no normal fulfilled outcome. Unsupported loops/switches remain open unless an
unconditional finally exit overrides their possible completions. Analysis closes
when it exceeds 256 flow visits or 64 distinct completion alternatives. Bounds
do not prove general language coverage or performance at application scale.

The source model records exact function/binding spans, hashes, return evidence,
completion alternatives and a source-node certificate. Projector V13 first uses
the unchanged V12 provenance checks. It then requires the model's function to
match an actual entered helper in the accepted async chain and the helper call
to be the whole returned expression, optionally under await. A larger callback
result remains open. The issued input revision is checked again before returning
notes or suppressions. The browser plugin, native runtime, scheduling and
execution-context instrumentation are unchanged.

## Fresh counterexamples

Fresh constant controls cover discarded reads, finally replacement, nested
finally, catch agreement, normal finally, matching branches, matching conditional
finally, await of a literal, an overridden loop, primitive kinds, an async arrow,
namespace dispatch, cross-file re-export and an awaited whole call. Their actual
reads remain available as observations even when their hints are suppressed.

Value-dependent targets retain hints for direct results, catch-only constant
fallback, conditional finally, a finally returning the read, branching results,
loop returns, an async method and a written JavaScript function binding. Paired
controls capture their primitive during memo computation and update correctly.

One fresh target receives no hint. The helper reads the reactive source and
returns 9; the consumer uses `invoke(read).then(() => raw)` as the returned
expression. Its displayed value stays 1 instead of updating to 2. The working
capture control updates to 2. The original call is a receiver in another member
call, outside the current admitted returned-expression profile. There are zero
candidate sites, zero continuation captures, zero events and zero suppressions.
Both V12 and V13 projectors yield no note. This miss precedes the new filter;
it is not a constant-result suppression hiding an existing detection.

Four working controls still receive noisy hints:

- A local constant identifier supplies the returned 9.
- A loop discards the read before returning 9.
- A synchronous wrapper returns an async method's constant result.
- `Promise.all` collects a constant helper result into a constant array.

These shapes remain in the control denominator. Literal-only value evidence,
unsupported loop flow, wrapper/member result flow and larger result expressions
are the respective missing proofs. The source model does not certify those
paths merely because their observed outputs happen to agree.

The earlier five misses remain: unawaited child-Promise adoption, a nested
synchronous reaction, an object result, an async generator and the 64 pending
read-ticket budget. External package async bodies and general intent/value flow
remain open beyond these specific source shapes.

## Published types and the harness correction

The adapted populations use retained published `@solid-primitives/map@1.0.0-next.2`,
`@solid-primitives/set@1.0.0-next.2` and
`@solid-primitives/controlled-signal@1.0.0-next.3`, with audited Solid rc.9.
The new helper/result-flow population primarily reuses controlled-signal and
includes the real map typing exclusion. Helpers and consumer variations are
authored research source; this is not an audit of arbitrary package internals.

Every stage checks a complete TypeScript program against real published
declarations. Fifteen invalid stages receive no hint or suppression: eleven
earlier `TS2322` stages and two pairs of `TS2769`/`TS2345` Solid/map exclusions.
Live Vite may still execute erased invalid JavaScript; exclusion is from feedback
and target/control scoring, not a claim that execution is prevented.

The first new browser attempts stop on the JavaScript source-write case with
`TS5055`: the test config enables JavaScript input but omits `noEmit`, so
TypeScript treats that input as an output destination. The checker correctly
refuses instrumentation under that diagnostic. No result is scored from those
unfinished runs.

Browser V8/audit V7 support an explicit check-only `noEmit` option. Case module
V2 preserves all V1 source, roles and expected behavior and sets this option only
for the JavaScript input. The completed 40-case runs use it. Both failed reports,
V1 cases and the first detector seal remain unchanged. The result proof itself
was frozen before the V1 challenge and receives no semantic change after it.

Eight explicit `tsc --noEmit` checks give six clean programs, including noisy,
value-dependent and written-JavaScript cases, and the two expected typing
exclusions. No declaration stub is substituted or broadened. No network or
package installation is used; publication inputs remain unchanged.

## Independent audit and verification

The new independent auditor imports no result-proof, selector, transform, session
or projector. It resolves the actual source binding itself and computes a
different outcome representation: possible return values, fallthrough and
throw outcomes. It independently checks branch unions, catch outcomes and
finally overrides, normalizes fallthrough to undefined, and requires every
possible fulfilled value to match the claimed primitive. It also authenticates
binding absence inputs, executed-helper correspondence and certificate spans.

The existing independent revision/frame audits reconstruct each stage's real
TypeScript program, check input manifests and exact declarations, validate
mapped entry/read frames and completion provenance, reject retired batches and
compare plain behavior. Across the five completed populations they audit
**88 observations**, including **58 async observations and 60 invocation links**,
and **26 suppressions**, of which **21 use the new async proof**. Native semantic
models and collected traces remain research premises, not a certification root.

Verification passes:

- **491/491** prototype tests with no skips, including 45 result-proof tests
  and 14 independent outcome/audit checks.
- **411** benchmark module syntax checks.
- **26** historical/current seals authenticate **413** distinct pins unchanged.
- Eight explicit TypeScript checks: six clean and two expected exclusions.
- `make verify-fast`: matching producer reuse, Rust formatting and
  certification/probe-pinned workspace Clippy.
- Schema parsing, dialect manifest validation, document links and whitespace.

Full `make verify`, production coverage/ownership, contract corpus and
certification gates are deferred for this research slice. No production Rust
rule, compiler lowering, public contract, manifest, fixture stub or finding
snapshot changes. New source modules and generated research reports are the
changed artifacts; historical sealed modules and earlier evidence are preserved.

## Remaining work

The proof improves precision across package values without a helper-specific
trust contract. The fresh challenges show where more general source/value flow
and receiver-call attribution are still required. These results support a useful
feedback system; they do not establish coverage for all packages or rules.

Stateful HMR, late completions across retained runtime instances, concurrent
serving, default dependency prebundling, external package async bodies, large
project cost, buffer bounds, real-app precision and production/editor integration
remain open. Side effects and rejection-dependent behavior remain outside the
constant fulfilled-value claim even for a modeled helper.

## Evidence

Implementation is under [reviewed-package-models](../../../benchmarks/reviewed-package-models/README.md):
`async-constant-result-v1`, `native-read-feedback-v13`, independent
`async-constant-audit-v1`, `feedback-revision-browser-v8` and
`feedback-revision-audit-v7`. Transform V16 and native runtime V5 are unchanged.

- [Combined summary](../../../rust/target/async-constant-combined-summary-v1.json).
- [42-stage audit](../../../rust/target/async-constant-replay-v1-audit-v1.json)
  and [35-stage audit](../../../rust/target/async-constant-replay-v2-audit-v1.json).
- [Continuation replay audit](../../../rust/target/async-constant-continuation-audit-v1.json)
  and [reference replay audit](../../../rust/target/async-constant-reference-audit-v1.json).
- [Fresh 40-case audit](../../../rust/target/async-constant-fresh-audit-v1.json).
- [Receiver-call enrollment gap](../../../rust/target/async-constant-registration-gap-v1.json).
- [Explicit TypeScript checks](../../../rust/target/async-constant-tsc-v1/results.json).
- [All tests](../../../rust/target/async-constant-all-tests-v1.log),
  [fast handoff](../../../rust/target/async-constant-verify-fast-v1.log),
  [syntax checks](../../../rust/target/async-constant-syntax-v1.json) and
  [seal authentication](../../../rust/target/async-constant-historical-seals-v1.json).
- [First unfinished observed run](../../../rust/target/async-constant-fresh-browser-reads-v1/results.json)
  and [first unfinished plain run](../../../rust/target/async-constant-fresh-browser-plain-v1/results.json).

Each successful audit pins its exact case and observed/plain reports. The two
new detector seals bind the proof modules before challenge authoring and the
later check-only harness correction. Package bytes, production snapshots and
earlier reports are unchanged.
