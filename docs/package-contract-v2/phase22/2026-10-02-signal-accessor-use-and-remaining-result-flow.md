# Signal accessor use and remaining result flow

## Result

Following a factory's accessor use removes the unused-signal shortcut hint
without losing the earlier detections. The unchanged 40-case package replay
still catches **13/16** stale results and improves quiet working controls from
**19/22 to 20/22**. The two deliberate debugging wrappers remain noisy.

A new 29-case challenge exercises a source-authored package created after the
detector was sealed. It catches **7/10** stale results and leaves **15/17**
working controls quiet. All 39 working controls across both populations behave
as intended; all 26 targets retain their demonstrated stale result. Four
type-invalid inputs remain silent. All **69** plain/instrumented comparisons
preserve behavior and native diagnostic deliveries.

The new source-authored package tests transfer across accessor-use shapes. It
does not add to the published-package coverage claim: the replay still spans
the same five retained published packages and earlier local counterexample.

## Stronger source enrollment

`signal-accessor-use-v1.mjs` uses exact local declarations and references. The
V2 shortcut model now requires a later admitted source-factory path with at
least one of these facts:

- A direct call to the first getter binding of a constant tuple destructure.
- A call through a constant alias of that binding.
- Transfer of that exact getter into a known object property or shorthand.
- A zero-argument call to index `0` of a factory tuple or its constant alias.
- An inline call to `createSignal(...)[0]()`.

Factory, binding, use and alias-declaration spans are recorded. Core imports
still resolve to the actual published declarations. The extractor rejects
unused getters, setter-only use, unused tuples, mutable getter aliases,
explicit tuple rebinding/member writes, nested-only uses, dynamic indices,
optional calls, defaulted getter bindings and shadowed names. Transparent
wrappers preserve exact bindings.

The retained trigger-cache path transfers its exact first getter into an
object; its later member dispatch remains open. The static-store path assigns
the source tuple to the exact local variable later called at index `0`.
Neither descriptor proves counterfactual execution or returned-value flow.
Object escape is explicitly weaker than an observed accessor call.

`package-shortcut-v2.mjs` retains the same observed no-observer return model
and successful-return tickets. It also adds the owner probe to an existing
Solid import rather than prepending a new dependency. A focused check verifies
that original module import order is preserved.

The V9 transform, V10 browser, V5 runner/study and V4 projector use these
source facts. Native hooks, runtime V3, consumer candidate selection and
synchronous scopes retain their earlier semantics. Original ESM package source
must remain visible. Native observation takes precedence if both channels
have evidence at one consumer site. Hints remain informational `intent-open`,
with open static dispatch, `authority:false` and `certification:false`.

## New transfer challenge

`study-accessor-use@0.0.0-research` has source-faithful declarations and an
explicit `source-authored-package` label. Its valid read helpers combine a
backing value with a real upstream Solid signal. Tracked controls follow that
signal; the no-observer return serves the backing value without tracking.
Neither its source nor its declarations is represented as a published package.

The seven detections cover direct getters, getter aliases, tuple calls, tuple
aliases, inline getter calls, an object lookup and a zero-argument wrapper
around an argument-bearing package call. Each target retains its initial
value after update; its captured-value control updates correctly.

The direct argument-bearing call, computed member call and async helper
continuation remain misses. The latter reads only after the caller's
synchronous observation scope has ended. Their working controls still update.

Unused getters, setter-only use and explicitly rebound tuples receive no
hint. Wrapped native `untrack` and an ordinary function remain quiet.

Two deliberately constant helpers still receive hints:

1. A getter is stored in metadata but never read.
2. A getter is called, its value is discarded, and the helper returns `9`.

Both results remain intentionally constant. These examples prove that accessor
use or escape is insufficient to establish that the returned value should
update. Requiring a returned getter value everywhere would also exclude valid
dependency-registration helpers such as trigger-cache methods. Further work
must distinguish registration, returned-value flow and developer intent.

Alternate read paths whose accessor origin falls outside the admitted local
factory remain open. Unknown helper mutation, arbitrary member dispatch,
argument/value flow and async attribution are not certified. The earlier
absent/computed/optional/then store paths, symbols and ownerful untracked reads
retain their limits.

## Typings and evidence

The initial preflight expected `TS2345` for `createSignal<boolean>(1)`. The real
published overloads instead produce `TS2769`; V2 cases record that exact
diagnostic. The original failed preflight and V1 case module remain available.
The V2 async setup also removes an unnecessary helper call. Detector code is
unchanged between the two seals.

Seven explicit `tsc --noEmit` runs confirm five clean cases, the published
Solid overload error and a missing member (`TS2339`) in the source-authored
package's faithful declarations. Both inputs are excluded before execution.
The two earlier published-package typing exclusions stay silent. No public
typing stub is changed or broadened.

The independent `native-read-audit-v4.mjs` imports neither extractor, selector,
transform nor projector. In addition to the existing branch/consumer audit,
it rebinds accessor declarations, constant aliases, object escapes and tuple
origins, checks explicit intervening writes, and authenticates all source
spans. It audits **24** hints, including the four noisy hints across both
populations. Runtime traces remain research evidence rather than an
adversarial root of trust.

Both studies authenticate that detector modules were sealed before their
population and unchanged across execution. The fresh source-authored package
was created after the first seal and is authenticated through package closure
pins. The second seal adds the original case module before the corrected V2
population. Source labels enter evaluation after selection and observation.

Plain comparisons preserve source/package pins, displayed values, callback
counts, native deliveries and exceptions for all 69 cases. Eight additional
source-file instances retain identical bytes. There are no harness failures.

Primary artifacts:

- `rust/target/accessor-use-detector-freeze-v1.json` and `-v2.json`
- `rust/target/accessor-use-regression-preflight-v1/population.json`
- `rust/target/accessor-use-regression-browser-v1/browser/results.json`
- `rust/target/accessor-use-regression-study-v1.json`
- `rust/target/accessor-use-regression-audit-v1.json`
- `rust/target/accessor-use-regression-parity-v1.json`
- `rust/target/accessor-use-fresh-preflight-v2/population.json`
- `rust/target/accessor-use-fresh-browser-v1/browser/results.json`
- `rust/target/accessor-use-fresh-browser-plain-v1/browser/results.json`
- `rust/target/accessor-use-fresh-study-v1.json`
- `rust/target/accessor-use-fresh-audit-v1.json`
- `rust/target/accessor-use-fresh-parity-v1.json`
- `rust/target/accessor-use-tsc-v1/results.json`

The prototype passes **314/314** tests, including 29 accessor-use checks. All
337 prototype modules pass syntax checks. Fourteen historical/current seals
authenticate 337 distinct pins without changes. `make verify-fast`, schema
parsing, dialect manifest validation and whitespace checks pass. The matching
producer is reused and pinned workspace Clippy passes. Full `make verify`,
production coverage/ownership, contract corpus and certification gates are
deferred for this research-only slice.

No production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated observations and the isolated source-authored
package remain under `rust/target/`. Real-application precision, transform and
stack cost, optimizer integration, other Solid artifacts, HMR and server
execution remain unverified here.
