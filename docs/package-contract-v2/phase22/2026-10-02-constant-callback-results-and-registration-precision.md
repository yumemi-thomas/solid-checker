# Constant callback results and dependency registration

## Result

The prototype removes **five earlier noisy hints without losing any of the
36 earlier detections**. Quiet working controls improve from **59/66 to
64/66** when the new source filter is applied to the unchanged 117-consumer
observations. These are adapted replays, not fresh tests of an unseen
population.

A separate 34-consumer challenge was authored after the new projector was
frozen. It catches **11/11** demonstrated stale results and leaves **19/20**
working controls quiet. Eight observed reads in deliberately constant-result
controls are suppressed. The remaining noisy control returns a Promise from
an async helper; that behavior is outside the bounded constant-value model.

| Population | Stale results with hints | Quiet working controls | Typing exclusions | Suppressed observations |
| --- | --- | --- | --- | --- |
| Earlier 69 consumers, adapted replay | 22/26 | 37/39 | 4 | 2 |
| Earlier 43 argument consumers, adapted replay | 12/16 | 24/24 | 3 | 2 |
| Earlier 5 native-reader consumers, adapted replay | 2/2 | 3/3 | 0 | 1 |
| Fresh 34 consumers, frozen projector | 11/11 | 19/20 | 3 | 8 |
| Combined 151 consumers | 47/55 | 83/86 | 10 | 13 |

All 86 working controls behave correctly; all 55 targets retain their
demonstrated stale result. The fresh 34 plain/observed browser comparisons
preserve behavior and native diagnostic deliveries, with no harness failures.
The earlier 117 comparisons remain authenticated and unchanged. No new
execution or instrumentation was needed to adapt those earlier observations.

These are controlled, authored consumers of retained published artifacts and
earlier source-authored research packages. The combined score is not an
estimate of precision or coverage in real applications.

## What changed

`constant-call-result-v1.mjs` adds bounded source evidence for a synchronous
call whose normal completion always returns the same primitive. The combined
projector `native-read-feedback-v9.mjs` first validates the original observation
through the unchanged V8 projector. It suppresses a hint only when this exact
call is the whole returned expression of the admitted callback and the new
constant-result evidence is available.

Suppression retains the original observation, its witness and the source
model. It changes the visible informational feedback; it does not change
execution, native diagnostics or the recorded read. No package-specific
contract is needed for the consumer helper.

The source model requires:

- An exact identifier binding or direct ES namespace-import member.
- A real source body, through constant function bindings and constant aliases,
  or a function declaration in an external module.
- For function declarations, no exact-symbol write in any non-declaration
  program source and no built-in direct eval in the declaration's source file.
  The full scanned source set is retained as negative evidence.
- A synchronous, nongenerator function. An expression body must be an admitted
  primitive literal; a block must end in a return and every return belonging to
  that function must have the same admitted primitive value.
- The exact whole callback result, allowing transparent TypeScript wrappers.

The admitted values are numeric literals with an optional unary sign, strings,
booleans, null, bigint literals and narrowly expressed undefined. Negative
zero remains distinct. The model does not fold arbitrary expressions or infer
values from parameter types, identifiers, calls, object/array construction or
declaration-only signatures.

Source and binding spans carry hashes. JavaScript shorthand destructuring
writes require TypeScript's shorthand assignment value symbol: the property
name's symbol alone does not identify the written binding. The focused tests
exposed this distinction before the detector was frozen.

## Dependency registration counterexamples

A helper can return a constant while its reactive read registers the dependency
for a larger result. Suppressing every constant-returning call would remove
useful feedback in that case. The filter therefore requires the call to be the
whole returned expression.

The fresh challenge retains all three dependency-registration targets:

- A constant helper contributes to a binary result that also reads mutable data.
- A wrapper calls the registration helper and then returns mutable data.
- A registration helper chooses a conditional expression whose result reads
  mutable data.

It also retains targets with mixed literal returns, returned objects and arrays,
mutable ordinary members, mutable local functions, JavaScript exported-function
reassignment and shorthand destructuring reassignment, and a shadowed inner
function. Each paired control captures the needed value during memo computation
and updates correctly.

Quiet constant controls include constant aliases, ignored scalar arguments,
same-valued return branches, named and namespace imports of a synchronous
callback helper, null and boolean results, and a registration helper whose
constant value is the whole callback output. An executed reactive read alone
does not establish that this output should vary.

The retained package artifacts include `@solid-primitives/map@1.0.0-next.2`
and `@solid-primitives/trigger@3.0.0-next.2` with the previously audited Solid
rc.9 runtime. The fresh helper source is authored consumer code and is labeled
`retained-published-package-with-authored-consumer-helpers`.

## Evidence and typing boundary

Study V8 authenticates both the original population detector seal and the
current projector seal. It explicitly records earlier populations as
`adapted-replay-of-prior-population`; they have
`detectorFrozenBeforePopulation:false` for this new filter. The fresh population
records `authored-after-current-projector-freeze` and a true freeze check.
Earlier frozen modules and executed observations remain unchanged.

Independent audit V7 imports neither the selector, transform, projector nor
constant-result utility. It rechecks the original observation evidence and
independently resolves the source bindings, writes, return paths and primitive
values used for each suppression. Across all four studies it audits **63**
observations, including **13** independent suppression audits. There are 50
visible hints. Runtime traces remain research evidence rather than an
adversarial root of trust.

Every consumer is checked as a complete TypeScript program against the actual
retained published typings before execution. Three fresh exclusions produce
`TS2769` for an invalid Solid signal argument, `TS2630` for assignment to a
TypeScript function declaration, and `TS2345` for a wrong map-key type. All ten
combined typing exclusions remain silent. The function-write counterexamples
use actual JavaScript source; they do not manufacture a TypeScript-valid
assignment by weakening library typings.

Twelve explicit `tsc --noEmit` runs use TypeScript 5.9.3: nine clean consumers
and the three expected errors above. No published typing stub is changed.

## Verification and retained inputs

The prototype passes **375/375** tests, including **31** focused constant-result
checks. All **365** benchmark modules pass syntax checks. Eighteen
historical/current detector seals authenticate 365 distinct pins unchanged.
Eight additional source-file instances in the fresh browser comparison retain
identical bytes.

Some retained temporary package inputs were missing during the first checks.
They were recovered from exact cached publication archives, with archive
integrity, earlier source pins or original lockfile identities, and surviving
file bytes checked before recovery. The recovery restores 214 missing files in
73 isolated package copies: three primary copies and 70 dependency copies.
It overwrites no existing file and uses no network or package installation.
Final package digests match the authenticated artifact bytes. Recovery receipts
are retained with the observations; the failed intermediate checks are also
preserved.

`make verify-fast`, schema parsing, dialect manifest validation and whitespace
checks pass. The matching Type Facts producer is reused and certification/probe
pins arm workspace Clippy. Full `make verify`, production coverage/ownership,
contract corpus and certification gates are deferred for this research slice.
No production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated populations, browser observations, comparisons and
audits remain under `rust/target/`.

Primary artifacts:

- `rust/target/constant-result-detector-freeze-v1.json`
- `rust/target/constant-result-fresh-preflight-v2/population.json`
- `rust/target/constant-result-fresh-browser-reads-v1/browser/results.json`
- `rust/target/constant-result-fresh-browser-plain-v1/browser/results.json`
- `rust/target/constant-result-fresh-study-v1.json` and `constant-result-fresh-audit-v1.json`
- `rust/target/constant-result-fresh-parity-v1.json`
- `rust/target/constant-result-additional-source-parity-v1.json`
- `rust/target/constant-result-replay-study-v1.json` and `constant-result-replay-audit-v1.json`
- `rust/target/constant-result-argument-study-v1.json` and `constant-result-argument-audit-v1.json`
- `rust/target/constant-result-native-study-v1.json` and `constant-result-native-audit-v1.json`
- `rust/target/constant-result-replay-comparison-v1.json`
- `rust/target/constant-result-combined-summary-v1.json`
- `rust/target/constant-result-tsc-v1/results.json`
- `rust/target/constant-result-all-tests-v3.log`
- `rust/target/constant-result-verify-fast-v1.log`
- `rust/target/constant-result-recovered-artifacts-v1/receipt.json`
- `rust/target/constant-result-recovered-artifacts-v1/dependent-receipt-v1.json`

## What optimism is justified

There is positive evidence for useful development feedback across package
surfaces: common runtime observations supply hints, and bounded source evidence
removes avoidable noise without a separate contract for each helper. The fresh
dependency-registration counterexamples show that this precision improvement
can preserve the defects it was designed to catch.

Feedback remains `info`, `intent-open`, `staticDispatch:open`,
`authority:false` and `certification:false`. These observations are not
certified violations. Eight earlier targets remain missed: deferred external
callbacks, async helper continuations, suspending arguments and computed or
optional calls. The three remaining noisy controls are a stored-but-unread
getter, a read-but-discarded getter behind ordinary member dispatch, and the
async constant helper.

Unknown declaration-only bodies, ordinary mutable member dispatch, arbitrary
callback timing and broader result/side-effect flow remain open. Earlier
absent/inherited/symbol/`then` store paths and ownerful untracked reads retain
their limits. The constant-value evidence applies to the authenticated source
program used by these studies; automatic closure freshness across editor
updates or HMR is unverified. Other runtime artifacts, server execution,
optimizer integration, real-application precision and observation cost also
remain open.

The next useful evidence is feedback from ordinary applications under their
normal toolchain, with explicit accounting for missed paths, noisy hints and
source changes. The current results justify that investment; they do not
establish coverage of every package or every rule.
