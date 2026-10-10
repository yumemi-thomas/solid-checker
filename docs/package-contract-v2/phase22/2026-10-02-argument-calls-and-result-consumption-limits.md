# Calls with arguments and result consumption limits

## Result

The prototype can observe direct calls with arguments without a new contract
for each package method. It catches two earlier misses: a direct map membership
lookup and a direct argument-bearing call in the source-authored package.
The unchanged 69-consumer replay improves from **20/26 to 22/26** stale-result
detections, with the same **35/39** quiet working controls and four silent
typing exclusions.

Two populations authored after the argument profile was frozen provide more
evidence:

| Population | Stale results with hints | Quiet working controls | Typing exclusions |
| --- | --- | --- | --- |
| 43 argument consumers | 12/16 | 22/24 | 3 |
| 5 native-reader consumers | 2/2 | 2/3 | 0 |

All **66** working controls across the replay and new populations behave as
intended. All **44** targets retain their demonstrated stale result. All
**117** plain/observed comparisons preserve behavior and native diagnostic
deliveries. There are no harness failures. Seven type-invalid inputs remain
silent.

These are controlled, authored consumers of retained package artifacts and
earlier source-authored research packages. They are not a representative
sample of real application defects or evidence of universal package coverage.

## Consumer scope

`native-read-sites-v4.mjs` permits nonoptional calls to exact declared
identifiers and named member paths, including arguments and spreads. It
records each argument's original span and spread status. Exact native memo
declarations, the nested callback and admitted returned expression remain
required. Static dispatch and argument/result flow stay open.

`async-read-transform-v11.mjs` wraps the complete original call in a
synchronous observation thunk. It does not replace the callee, receiver or
individual arguments. The actual receiver lookup, argument evaluation and
spread iteration happen once, in their original order. Successful tickets
commit only when the entire call returns successfully. Argument or iterator
throws discard partial observations; the original exception and catch path
are preserved.

The necessary syntax prefilter V4 now admits argument-bearing identifier calls
as well as properties. It provides no semantic trust. Exact native `untrack`
calls and their callbacks retain deliberate snapshot intent. Calls with
`await` or `yield` in their immediately evaluated arguments remain open because
the synchronous thunk cannot contain suspension from the caller. Creating an
async callback argument does not itself suspend; its later execution is
outside the scope unless another admitted expression observes it.

The native-only V7 projector and combined V8 projector both use the current
selector. Browser V12, runner V7, study V7 and independent audit V6 reproduce
the same profile. Native hooks/runtime V3 and package-shortcut V2 are unchanged.
Original ESM package source remains visible through the existing raw-source
loader; dependency discovery stays disabled in both plain and observed runs.

No source enrollment or package-specific contract was added to make these
argument calls detectable. An exact declaration admits the consumer candidate;
an observed native reader or admitted package shortcut supplies the runtime
witness. An ordinary declared method with no such witness produces no hint.

## Breadth and counterexamples

The argument challenge catches direct `ReactiveMap.get/has`,
`ReactiveWeakMap.get/has`, `ReactiveSet.has`, `ReactiveWeakSet.has`, a spread key,
`Function.call`, `Reflect.apply`, a generic local helper, a synchronous imported
callback helper, and the earlier source-authored package's argument method.
These executions use the real published declarations of the retained map,
set and static-store versions. The local research package remains explicitly
labeled `source-authored-package`.

The native-reader addendum uses
`@solid-primitives/controlled-signal@1.0.0-next.3`. Argument-bearing helpers
around uncontrolled boolean and externally controlled numeric accessors both
receive the native-read channel. Their paired controls capture the value
during memo computation and update correctly. This verifies the current
native projector with argument sites, independently of package shortcut
observations.

The fresh argument population still misses:

- An imported helper that invokes its callback in a later promise continuation.
- A lookup whose argument itself awaits a value.
- A computed method lookup.
- An optional method lookup.

The replay retains four misses: the earlier deferred external callback,
two async helper cases, and a computed member in the source-authored package.
Absent/inherited/symbol/`then` store paths, unknown dispatch, origins outside
the bounded shortcut enrollment and ownerful untracked reads retain their
earlier limits. Other Solid artifacts, server execution and HMR are unverified.

Three new working controls receive informational hints:

1. A map read is evaluated as an argument, but the callee returns `9`.
2. An imported callback reads a static store, discards the value and returns `9`.
3. A native accessor is evaluated as an argument, but the callee returns `9`.

The four earlier noisy debugging/escape/discard controls also remain noisy.
The native case proves this limitation is not confined to shortcut enrollment:
an actual reactive read still does not establish that the returned value should
depend on it. These controls must remain working counterexamples, rather than
being relabeled as defects to improve the score.

Hints remain `info`, `intent-open`, `staticDispatch:open`, `authority:false`
and `certification:false`. Native observations establish an executed successful
read; shortcut observations establish an executed no-observer return before
an admitted source accessor use or object escape. Neither establishes developer
intent, arbitrary callback timing, counterfactual execution or returned-value
flow. They are not production violations or certified contracts.

## Direct eval boundary

After the initial argument profile was executed, an isolated non-strict
JavaScript reproducer showed that moving direct `eval` into an arrow thunk
changes where a `var` declaration lives: `0number` becomes `0undefined`.
The same reproducer in strict mode returns `0undefined` in both forms.
Solid ES modules are strict, so this is **not** counted as a confirmed Solid
application behavior regression.

The final selector conservatively excludes the exact built-in direct `eval`
symbol resolved to the program's default library declaration. It preserves
ordinary declared `.eval` methods and indirect aliases. Transparent wrappers
around the built-in direct call are excluded too. This is an explicit dynamic
scope boundary, not a name-only package rule. The fresh direct-eval control
remains uninstrumented and behaves identically in the browser.

Executed V3-selector/V10-transform modules, tests, first seal and first replay
are retained unchanged. The final V4-selector/V11-transform profile is a new
version. Replaying all 69 consumers with the final profile preserves the
initial argument profile's displayed behavior, source, typings, native
deliveries and findings.

## Typings, audits and verification

Every preflight and browser run checks a complete TypeScript program against
the actual retained published types. Three new invalid argument cases produce
`TS2554` for a missing map key and `TS2345` for wrong map/weak-set key types.
They are excluded before execution and receive no hints or native feedback.
The four earlier typing exclusions remain silent. No public typing stub is
changed or broadened.

Sixteen explicit `tsc --noEmit` runs use TypeScript 5.9.3: thirteen clean cases
and the three published argument errors above. Full-program errors suppress
the prototype before it can duplicate TypeScript's claim.

The independent audit V6 imports neither selector, transform nor projector.
It authenticates the original consumer span and argument spans, exact memo
and callee declarations, no suspension/direct eval, the observed context,
native constructor/reader frames or the package shortcut branch and accessor
use facts. It audits **43** hints, including all seven noisy controls. Runtime
traces remain research evidence rather than an adversarial root of trust.

All three final studies authenticate that their detector modules were frozen
before their population and unchanged across execution. The final detector was
sealed before the 43-case challenge was authored; the later native-reader
addendum uses the same frozen detector. Case labels enter scoring after
selection and observation.
Seals V2/V3 differ only by adding the argument case module; the first seal and
executed initial argument profile remain available.

Plain comparisons preserve original source/package pins, displayed values,
callback counts, native diagnostic deliveries and caught exceptions for all
117 consumers. Eighteen additional source-file instances also retain identical
bytes. The throwing-argument and throwing-spread controls keep their catch
results and produce no partial-read hint.

The prototype passes **344/344** tests, including 14 checks of the initial
argument profile and 16 of the final profile. They include actual instrumented
rc.9 native reads, original member lookup/`this`/argument/spread order,
suspension exclusions, exact untrack intent, throws, deferred callbacks and
the direct-eval boundary. All **359** prototype modules pass syntax checks.
Seventeen historical/current seals authenticate **359** distinct pins without
changes; the native addendum is authenticated as a later consumer by its
population and browser pins.

`make verify-fast`, schema parsing, dialect manifest validation and whitespace
checks pass. The matching Type Facts producer is reused and pinned workspace
Clippy passes. Full `make verify`, production coverage/ownership, contract
corpus and certification gates are deferred for this research slice. No
production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated observations remain under `rust/target/`.

Primary artifacts:

- `rust/target/argument-read-detector-freeze-v1.json`, `-v2.json`, `-v3.json`
- `rust/target/argument-read-replay-preflight-v2/population.json`
- `rust/target/argument-read-replay-browser-v2/browser/results.json`
- `rust/target/argument-read-replay-study-v2.json` and `-audit-v2.json`
- `rust/target/argument-read-replay-parity-v2.json`
- `rust/target/argument-read-replay-initial-final-parity-v1.json`
- `rust/target/argument-read-replay-projection-parity-v1.json`
- `rust/target/argument-read-fresh-preflight-v1/population.json`
- `rust/target/argument-read-fresh-browser-v1/browser/results.json`
- `rust/target/argument-read-fresh-study-v2.json` and `-audit-v2.json`
- `rust/target/argument-read-fresh-parity-v1.json`
- `rust/target/argument-native-fresh-preflight-v1/population.json`
- `rust/target/argument-native-fresh-browser-reads-v1/browser/results.json`
- `rust/target/argument-native-fresh-study-v1.json` and `-audit-v1.json`
- `rust/target/argument-native-fresh-parity-v1.json`
- `rust/target/argument-read-tsc-v1/results.json`
- `rust/target/argument-native-tsc-v1/results.json`
- `rust/target/argument-read-additional-source-parity-v1.json`
- `rust/target/argument-read-combined-summary-v1.json`

## What optimism is justified

The common observation mechanism transfers to more package surfaces without
authoring a separate method contract. This is positive evidence for useful
development feedback on executed paths. It is not evidence that every package
rule can be detected or that every hint is a defect.

The next precision task is to distinguish source reads used to register a
dependency from values that actually reach the returned result. Ignored
arguments and discarded callback results give concrete counterexamples for
that work. Dynamic continuation attribution, raw-source optimizer integration,
real-application precision and instrumentation/stack cost remain open.
