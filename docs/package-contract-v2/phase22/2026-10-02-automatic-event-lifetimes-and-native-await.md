# Automatic event lifetimes and native await

The new prototype detects **13 of 14 authored lifetime mistakes across six
published packages**, with **no new lifetime warning on 13 controls**. It removes
per-handler scope annotations while keeping an explicit project policy: work
started by a DOM handler should stop when its creating owner is disposed.
Intentional background handlers have an explicit escape.

This supports a scalable way to detect a class of mistakes without certifying
each package. It does not establish coverage of most packages or every misuse.
All findings remain conditional warnings with `certification: false`; no
accepted package contract or production analyzer rule changed.

## What was tested

The final study executes 27 consumers under four profiles, plus the cached Helge
app under the original and native profiles: **110 fresh browser executions**.
All 27 consumers pass strict TypeScript against real published declarations;
the app also has no published typing error in this comparison. Inputs are
Solid/signals/web 2.0.0-rc.9, TypeScript 5.9.3 and Chromium 151.0.7922.34.

| Profile | Targets detected | Controls with new lifetime warnings | Execution failures |
| --- | ---: | ---: | --- |
| Original source; monitor with no declared event context | 0 / 14 | 0 / 13 | 0 |
| Automatic DOM handler binding; native async unchanged | 1 / 14 | 0 / 13 | 0 |
| Generator lowering with captured resumption context | 14 / 14 | 0 / 13 | 1 valid control |
| Native await; captured operations and Promise delivery | **13 / 14** | **0 / 13** | **0** |

The six packages are scheduled, timer, event-listener, resize-observer,
intersection-observer and mutation-observer, all from the retained published
Solid Primitives installations. This is a designed experiment, not a blinded
package sample. Aliased and namespace imports work without package-specific
resource models.

Detected cases include synchronous and async events, rejection with catch/finally,
nested async helpers, a separate TypeScript helper module, a continuation after
child-component removal, calls with awaited arguments, ordinary Promise
callbacks, and uncancelled timers, listeners and observers. The warning points
to the original operation in the app or local helper, rather than generated
instrumentation.

Controls include owner-created cleanup, manual cleanup, intentional background
work, unrelated work while a handler awaits, and scheduling order for native
Promises, thenables and a Promise subclass. Resize, intersection and mutation
cleanup controls retain their original core `NO_OWNER_CLEANUP` warning. The
experiment adds no lifetime warning there and does not reinterpret those core
messages as new detections.

## How it scales

1. Resolve intrinsic DOM tags and event properties through the installed
   renderer's actual JSX declarations. A custom component's similarly named
   prop supplies no automatic event premise.
2. At handler construction, capture its actual Solid owner and register a
   deferred disposal observation. Binding the callback supplies monitor context;
   it does not restore Solid ownership or supply resource cleanup.
3. Capture that context at native async-function entry. Execute supported calls
   and constructions inside it after `await`, keeping the original await syntax.
4. For supported calls with awaited arguments, bind the value callee before
   evaluating the arguments. Member receivers remain open in this branch.
5. An opt-in bridge carries context through native Promise callback delivery.
   The original `then` creates and returns the Promise; the bridge adds no
   Promise reaction. Timers already propagate their registration context.
6. Observe actual browser resources and cancellation. A resource still active
   after the declared lifetime ends produces the existing experimental
   `RESOURCE_OUTLIVES_DECLARED_SCOPE` warning.

The shared mechanism applies wherever a package reaches a monitored browser
API. It needs no per-export contract. The earlier source inventory found such
references in 37 of 97 retained package roots. That number measures potential
reach, not demonstrated misuse coverage. Core diagnostics and positive source
premises remain separate ways to explain other mistakes, including stale
reactive reads; resource monitoring does not detect those by itself.

## Rejected approach and corrected assumptions

The first owner check rejected plain owners because it required integer
`_flags`. The published rc.9 `createOwner` object initially omits that field;
teardown writes it. The corrected check admits that exact plain-owner shape and
retains the children-forbidden and disposed checks. This still uses a private
runtime ABI, so an unaudited runtime version is refused.

TypeScript's async-to-generator lowering reached all targets but broke a valid
Promise-subclass control after the subsequent Solid compilation:
`ReferenceError: _self$ is not defined`. It is rejected. A higher detection count
cannot compensate for a broken valid program. These observations do not prove
that every generator lowering changes scheduling; this particular pipeline
already fails before that control can complete.

The first native profile found 11 of 13 targets. Promise callback delivery and
value-callee binding closed its two measured misses. A new member-call target
was added to retain an actual remaining miss rather than report complete
coverage from the earlier specimen set. A type-invalid resize cleanup that used
an unpublished `disconnect` property was excluded by TS2339 in the exploratory
run; the final case uses the published `unobserve(box)` operation. No checker
finding is created for that TypeScript error.

## Behavior checks and remaining limits

The native profile preserves all recorded consumer values and all core feedback
in this run. All three scheduling arrays equal their original-source arrays.
Unit tests also cover callback receivers, argument order, results, rejection
identity, empty helpers, type assertions and a consumer binding named
`globalThis`. These are finite behavior checks, not proof of every observable
JavaScript property. Instrumentation changes served functions and API identity.

The cached app completes navigation, modal open/close, mobile navigation,
history and disposal with unchanged core feedback and no new lifetime warning.
Nine DOM event expressions are instrumented. The app flow did not capture the
monitor's global binding/gap state, so nine transformed expressions do not prove
nine runtime admissions or complete app coverage.

Remaining open cases are concrete:

- `Timer.makeTimer(callback, await delay, setTimeout)` is type-valid and its
  callback runs after disposal without a lifetime warning. The member receiver
  path with awaited operands remains an explicit source gap.
- Optional call chains, direct eval, async generators, for-await and yielded
  operands remain outside admitted continuation transformations.
- Ordinary callbacks need a delivery mechanism that propagates context. Promise
  and timer delivery are covered here; observer callbacks and arbitrary package
  schedulers are not.
- Native async bodies inside dependencies are not transformed. Project `.ts`,
  `.js`, `.tsx` and `.jsx` helpers under `src/` are the admitted source boundary.
- Bound event tuples, custom component event forwarding and other JSX event
  declaration families are not established by this intrinsic-event transform.
- Automatic setup/effect lifetime context, resource transfer between owners,
  shared resources, multiple realms, HMR, SSR, hydration and other bundlers need
  separate tests. Once/AbortSignal listeners and coerced cancellation retain
  the existing monitor's explicit gaps.
- The project policy supplies intent. Code that intentionally outlives a UI
  owner requires an escape; runtime evidence cannot infer that intention.
- Instrumentation cost and app latency were not measured for this added async
  profile. The previous resource-registration cost does not measure it.

## Artifacts and validation

Implementation and reproduction live in
`benchmarks/reviewed-package-models/automatic-lifetime-{runtime,transform,feedback,cases,run}.mjs`,
with focused tests and `validate-automatic-lifetime.mjs`. The original browser
harness gained an optional source-plugin hook. The resource audit gained
context capture/binding and opt-in Promise delivery; its default Promise behavior
is unchanged.

Final observations are under `rust/target/automatic-life-final-promises/`; the
summary is `rust/target/automatic-life-validated-promises.json`. Sixteen local
input files were hashed before and after all four profiles, stayed unchanged,
and have preserved byte copies in `source-inputs/`. The validator also compares
package closure pins, real typing observations, source hashes, caller locations,
control feedback and scheduling. Earlier trial outputs remain separate.

Validation passed: **66 prototype tests**, the saved-study validator, Rust
formatting, pinned workspace Clippy, diff whitespace, schema JSON parsing and
dialect manifest validation. Full `make verify`, certification, fixture coverage
and ownership gates were deferred because this change is isolated experimental
code and documentation. No public contracts, accepted receipts, fixture snapshots,
schema or production analyzer semantics changed.
