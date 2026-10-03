# Package observer shortcuts and evidence strength

## Result

Source-derived branch observations extend feedback to packages that avoid
native reactive reads when no observer exists. This supplies a useful weaker
channel alongside actual native-read observations. The branch evidence still
does not prove a defect or that a reactive value drives the returned result.

The unchanged 38-case store challenge improves from **8/15 to 10/15** target
hints: `ReactiveMap.size` and a media breakpoint property now receive a
shortcut hint. New-hint controls remain **20/21** quiet. The original debugging
hint and auxiliary memo initialization warning remain, giving **19/21** quiet
controls across all feedback.

A new 40-case package challenge gives **13/16** target hints and **19/22** quiet
working controls. All its hints are shortcut observations. Two deliberate
debugging wrappers and an unused-signal counterexample remain noisy. All 43
working controls across both populations behave as intended; all 31 targets
retain their demonstrated stale result. Instrumentation does not repair code.

## Source model and runtime witness

`package-shortcut-v1.mjs` resolves actual published Solid declarations. It
admits a negated, zero-argument call to the exact imported `getObserver`, an
immediate return branch, and a later exact `createSignal` call in the same
synchronous function body. Nested functions, async/generator bodies, local
lookalikes, shadowed imports and arbitrary observer checks are refused.

This is bounded enrollment. The later signal call is a syntax and symbol fact,
not proof that the alternate path executes, reads its accessor, or contributes
to the consumer's result. The model explicitly records `valueFlow: open`.
It does not infer member dispatch from names or claim a package contract.

The hook rewrites only the original guarded return. It captures a ticket
before evaluating the original return expression, evaluates that expression
once, and commits only after it succeeds. Receiver, arguments, getter counts
and thrown return expressions have focused tests. An enclosing consumer
expression that throws discards its successful inner tickets.

The V8 consumer transform uses the earlier exact memo/callback/property
selector, native hooks and synchronous expression scopes. Runtime V3 adds a
separate package-guard identity. A guard ticket requires an active original
consumer expression, absent native owner and no explicit native `untrack`
intent. The actual original guard supplied the no-observer branch condition.
Scopes are not propagated across `await`.

The feedback projector authenticates current consumer declarations and spans,
the exact package model and published declaration hashes, the pinned native
observer provider, and mapped package-return and consumer frames. If both
native and shortcut evidence occur at one site, the native channel takes
precedence. The shortcut code is `OBSERVED_MEMO_CALLBACK_PACKAGE_SHORTCUT`,
with severity `info`, category `intent-open`, open static dispatch,
`authority:false` and `certification:false`.

Its wording states the observed package branch and suggests capture during
memo compute if updates are expected. It does not state that a native reactive
read occurred or report a proven violation.

## Browser source visibility

The initial V8 browser replay remained at 8/15 because Vite prebundled package
dependencies. Neither target loaded an original shortcut model, and both
reported zero guard entries. That is a verified instrumentation gap, not
evidence that the detector ran and found no shortcut.

The immutable V9 browser profile disables dependency discovery for both
instrumented and plain execution. `native-read-browser-run-v4.mjs` uses that
profile. `native-read-study-v4.mjs` requires its source-profile metadata and
authenticates every executed shortcut against a loaded model. The corrected
replay loads the original trigger-cache and static-store functions and reaches
10/15. Both the initial and corrected observations remain available.

This original-source ESM profile is a research prerequisite. It does not
establish production optimizer integration, CommonJS support or development
performance. A production adapter must retain package source provenance
through optimization rather than treating optimizer bundles as original files.

## New cases

The retained versions are static-store, map and set `1.0.0-next.2`, trigger
`3.0.0-next.2`, and media `4.0.0-next.2`, using Solid and signals rc.9.

The 13 detected targets span those five packages:

- Static-store own properties, wrappers and nested properties.
- Map value iteration, membership and weak-map membership through wrappers.
- Set size and membership through wrappers.
- A trigger-cache wrapper around a plain mutable counter.
- A media wrapper around a breakpoint property.
- Three additional cases: map key iteration, a namespace-created static
  store, and media's derived `key` getter.

The first 34 consumers were authored after the shortcut instrumenter was
sealed, but before the browser prebundling repair. They are adapted evidence
for the final loading profile. The final six consumers were authored after
the entire V9 profile was sealed: **3/3** stale-result targets receive hints,
and **3/3** working controls stay quiet. This small held-out subset supports
transfer; it does not establish population precision.

The three new target misses are a direct argument-bearing `map.has(2)` call,
a callback body declared only in another module, and a read after an imported
async helper's own `await`. Zero-argument wrappers around membership calls
are covered because their actual synchronous execution falls inside a
consumer expression scope. No helper callback schedule is asserted.

The unchanged replay still misses absent store keys, computed and optional
properties, explicit `then`, and an async store helper continuation. Symbols,
ownerful untracked reads, unknown argument/value flow and unsupported guard
forms remain open.

## What the counterexample proves

The source-authored `study-shortcut-control` returns `9` from both paths. Its
tracked path creates a signal whose getter is unused. Its declaration
`read(): number` is faithful to the actual source. This local research package
is explicitly marked `source-authored-counterexample`; it is not counted as a
retained published package or supported primitive.

The detector enrolls its observer shortcut and emits an informational hint in
the async callback despite the deliberately constant result. Independent
audit confirms that all branch and consumer witnesses are correct. Therefore
accurate branch provenance alone does not distinguish useful dependency
tracking from an unrelated signal call.

The two real-package debugging wrappers demonstrate a separate limit: their
package lookups are reactive, but the wrappers intentionally return a constant.
Suppressing those requires returned-value flow or explicit developer intent.
Suppressing the unused-signal case requires a stronger relationship between
the factory's accessor and the alternate return path. Neither issue is solved
by changing the hint to a warning.

## Typings, parity and audits

All published-package consumers use retained real declarations. Two fresh
inputs are excluded before execution and stay silent: a wrong static-store
setter value (`TS2345`) and a missing map property (`TS2339`). Seven explicit
`tsc --noEmit` runs confirm five clean examples and those two diagnostics.
The two earlier store typing exclusions also remain silent. No published
typing stub or public contract is modified.

The V9 plain/instrumented comparisons cover all **78** cases and preserve
source and package pins, displayed values, callback counts, native diagnostic
deliveries and exceptions. Twelve additional-source file instances retain
identical bytes. The original V7 plain profile also agrees on the unchanged
38-case replay. There are no harness failures or failing working controls.

`native-read-audit-v3.mjs` imports neither selector, transform, model extractor
nor projector. It independently rebinds guard and source-factory imports,
checks original function/return spans and declarations, checks the native
provider hash, and authenticates mapped execution frames. It audits **27**
hints across the corrected replay and new challenge, including all noisy
hints. Runtime traces remain research evidence, not an adversarial root of trust.

Primary retained artifacts:

- `rust/target/package-shortcut-detector-freeze-v1.json` through `-v3.json`
- `rust/target/package-shortcut-regression-browser-v1/browser/results.json`
- `rust/target/package-shortcut-regression-study-v1.json`
- `rust/target/package-shortcut-regression-preflight-v2/population.json`
- `rust/target/package-shortcut-regression-browser-v2/browser/results.json`
- `rust/target/package-shortcut-regression-browser-plain-v1/browser/results.json`
- `rust/target/package-shortcut-regression-study-v2.json`
- `rust/target/package-shortcut-regression-audit-v2.json`
- `rust/target/package-shortcut-regression-parity-v3.json`
- `rust/target/package-shortcut-fresh-preflight-v2/population.json`
- `rust/target/package-shortcut-fresh-browser-v1/browser/results.json`
- `rust/target/package-shortcut-fresh-browser-plain-v1/browser/results.json`
- `rust/target/package-shortcut-fresh-study-v1.json`
- `rust/target/package-shortcut-fresh-audit-v1.json`
- `rust/target/package-shortcut-fresh-parity-v1.json`
- `rust/target/package-shortcut-heldout-summary-v1.json`
- `rust/target/package-shortcut-tsc-v1/results.json`

The prototype passes **285/285** tests, including 22 shortcut tests and the
additional intent counterexample test. All 326 prototype modules pass syntax
checks. Twelve seals authenticate 326 distinct historical/current pins without
changes. `make verify-fast`, schema parsing, dialect manifest validation and
whitespace checks pass. The matching producer is reused and pinned workspace
Clippy passes. Full `make verify`, production coverage/ownership, contract
corpus and certification gates are deferred for this research-only slice.

No production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated observations and the isolated local counterexample
stay under `rust/target/`. Real-application noise, stack/transform cost, optimizer
integration, other Solid artifacts, HMR and server execution remain unverified.
