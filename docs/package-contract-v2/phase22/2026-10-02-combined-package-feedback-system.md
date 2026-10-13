# A combined system for package feedback

The experiment supports building a useful feedback system without certifying
each whole package first. Combining native analysis, development runtime
observations, small source premises and declared lifetimes gives feedback for
**20 of 21 tested consumer patterns**. **23 of 24 correct-use or intent controls
are quiet**. One async pattern is missed, and one intentional snapshot still
gets a warning. These results establish a working approach, not coverage of all
packages or all rules.

The count includes source candidates, a rendering preference and a loading
advisory. It is **not twenty proven defects**. Two lifetime checks require an
explicit application policy. Source-derived feedback remains an assumption.

## What ran

The primary matrix has nineteen target/control pairs and three extra controls.
A follow-up adds an async pair and an explicit snapshot control. A transfer
pair exercises the same accessor check through a namespace import from another
package. Together these give 46 records: 45 executed consumers and one
TypeScript-owned exclusion. All 45 executed consumers pass the installed
published typings, as do their source analysis copies.

Four additional transfer observations retain the initial package setup
warnings and failures. There are **50 browser records in total**, with two
harness failures confined to those additional observations. They do not count
as successful controls or as detections of the intended accessor mistake.

| Installed package | Exact version | Use |
| --- | --- | --- |
| `@solid-primitives/memo` | `2.0.0-next.2` | Accessors, callbacks, async values, JSX |
| `@solid-primitives/static-store` | `1.0.0-next.2` | Getter snapshots and writes |
| `@solid-primitives/utils` | `7.0.0-next.4` | Ownership and leaf cleanup |
| RxJS | `7.8.2` | Operator callback ownership |
| Lodash | `4.18.1` | Action and flush through returned wrappers |
| `@solid-primitives/timer` | `1.4.5-next.1` | Resource lifetime and accessor transfer |
| `@solid-primitives/event-listener` | `3.0.0-next.5` | Resource lifetime |
| `@solid-primitives/bounds` | `1.0.0-next.2` | TypeScript exclusion only |

Seven packages have executed semantic consumers. The eighth supplies the
exclusion. Lodash uses the retained real `@types/lodash` 4.17.25 declarations.
Solid, signals and web resolve to 2.0.0-rc.9 throughout. No packages were
installed for this study.

## Feedback by pattern

Every row below has a corresponding correct-use control. The detectors receive
no target/control label, expected rule or expected outcome. Validation compares
the feedback with authored expectations afterwards.

| Pattern | Feedback from the combined system | Limit |
| --- | --- | --- |
| Reducer value frozen before JSX | Native strict-read warning and source warning | Intent can allow a snapshot |
| Reducer accessor interpolated as a value | Source-derived `uncalled-accessor` warning | Runtime stays quiet |
| Static-store getter frozen before JSX | Source getter snapshot information | Intent remains open |
| Direct write to static-store getter | Source candidate and actual getter-only exception | Descriptor changes remain open statically |
| `innerHTML` together with JSX children | Native `jsx-no-duplicate-props` violation on original code | Compiler evidence |
| Package array accessor rendered with `.map` | Source-derived `prefer-for` warning | Rendering preference |
| Write inside lazy memo | Native owned-write error | Static callback premise missing |
| `resolve` inside lazy memo | Actual runtime exception | No projected static rule proof |
| `until` inside lazy memo | Actual runtime exception | No projected static rule proof |
| Package cleanup in a leaf effect | Native forbidden-cleanup error and source warning | Unresolved callback facts remain visible |
| Package cleanup without owner | Native no-owner warning | Source placement remains unsupported |
| Write inside RxJS `map` during owned setup | Native owned-write error | RxJS also reports the caught error asynchronously |
| Lodash wrapper invokes an action during owned setup | Native action error | Requires the callback to execute |
| Lodash wrapper invokes `flush` in an action | Actual runtime exception | Not emitted on the diagnostic subscription |
| Eager read of pending package-derived value | Native pending-read error | Requires this pending execution |
| Pending package-derived JSX without `Loading` | Native loading advisory | Missing fallback may be intentional |
| Timer survives declared screen scope | Resource lifetime warning | Explicit scope policy |
| Listener survives declared screen scope | Resource lifetime warning | Explicit scope policy |
| Frozen accessor through computed namespace dispatch | Native strict-read warning | Static dispatch stays unresolved |
| Lazy memo reads its source after `await` | **No automatic feedback** | Declared update test proves stale output |
| Timer accessor concatenated as a value | Same source-derived accessor warning | Exact namespace symbol and permitted-write option |

The evidence channels overlap. Their counts are not additive:

| Channel | Targets with feedback | Controls with feedback |
| --- | ---: | ---: |
| Native analysis of original consumer | 1 | 0 |
| Source assumptions projected through native analysis | 5 | 1 |
| Source getter candidates | 2 | 0 |
| Native runtime diagnostics | 9 | 1 |
| Observed exceptions | 9 | 0 |
| Declared lifetime checks | 2 | 0 |

Seven target consumers also fail their declared rendered-output expectation.
Six receive another feedback signal. The after-await consumer is found only
by that application expectation; it is retained as the automatic miss.

## Changes that made the combined system possible

`family-feedback-system.mjs` combines the independent evidence channels and
keeps native uncertifiable findings in a separate gap list. A missing package
contract remains unresolved, while runtime observations and independent
compiler findings can still reach the user. The prototype issues no receipt
and admits no experimental premise as an accepted contract.

`family-project-warning.mjs` reuses a returned-accessor premise for two more
existing native rules: `uncalled-accessor` and `prefer-for`. Their native
findings lack declaration-related locations. The adapter therefore resolves
the exact original operand through TypeScript and joins its symbol to the
exact modeled declaration. A shadowed name, an unrelated declaration, ambiguous
premises or absent source facts cannot supply the join. Unicode source offsets
are mapped from native UTF-8 bytes back to the original file.

This is an analysis copy, never executed package code. Native violations in the
copy become **source-assumption warnings**, not proven violations in the
original consumer. The adapter also removes a co-located surrogate
`missing-owner` warning when a leaf-owner warning establishes that an owner
exists but forbids cleanup. Historical native outputs remain unchanged.

`family-imports.mjs` enumerates exact namespace member calls as well as named
imports. It compares the member's TypeScript symbol with the module export.
Computed dispatch and shadowed namespace parameters remain unresolved. The
same source extractor and accessor projection transfer from `createReducer`
to `createIntervalCounter`; no export-specific detector or contract was added.

The source getter checks are retained as information because source alone does
not establish that the user wanted future updates or that a descriptor can
never change. The resource collector uses platform registrations and an
explicit application's screen scope. It does not infer that every resource
should die at every Solid root disposal.

## Precision challenges retained

### Snapshot intent

An intentionally frozen reducer value remains `1` after dispatch and passes
its authored expectation. It still receives native and source strict-read
warnings. Reading it with explicit `untrack(value)` preserves the same intended
output and makes both channels quiet. A production system needs a clear way to
express snapshot intent; a warning cannot infer it from syntax alone.

### Silent async failure

The lazy memo callback awaits and then reads a signal. After that signal changes
from `1` to `2`, the rendered result remains `1`. The control captures the read
before awaiting and updates to `2`. Both pass published typing, and both
runtime diagnostic streams are quiet.

The source extractor establishes an accessor return but not the callback's
tracked phase. The synthetic analysis reports a strict-read finding in both
copies; projecting it would also warn on the correct control. The adapter
rejects this unsupported inference. Closing this gap needs an exact callback
phase/await fact, or an application behavior test. It cannot be closed by
declaring every callback tracked.

### Package-internal failures

The first timer transfer pair reads the actual accessor correctly in its
control, but both consumers get a strict-read warning during package
construction. Moving setup to a root exposes a write in the package's polling
effect, at `timer/dist/index.js:126`. The mount throws before assigning the
disposer; the later flow records a harness failure. Those two observations
cannot serve as successful controls.

The published `ownedWrite: true` option permits the polling signal's write.
With that option, both consumers mount and dispose cleanly. The source accessor
warning remains only on the consumer that renders function text. This separates
an actual package-internal runtime failure from the intended consumer mistake.
The feedback display still needs to distinguish their locations and merge
repeated reports of the same exception. Raw channels remain available here.

### TypeScript boundary

Assigning to `createElementBounds(...).width` produces TS2540 against the real
published types. It is excluded before browser execution and before native
static analysis. The combined system returns no checker feedback for it.
By comparison, the static-store assignment passes its real types and actually
throws; the runtime exception provides information TypeScript did not report.

## What this says about scalability

The reusable unit can be a **small behavior fact**: returns an accessor,
registers cleanup, invokes a callback in a known phase, or exposes a getter.
Whole-package completeness is not required for feedback that uses an established
fact. Exact installed bytes, host, argument profile and native runtime still
belong in the model and cache identity. Unknown branches remain gaps.

Runtime feedback has a separate scaling advantage: Solid's own execution checks
observe callbacks reached through RxJS and Lodash without a behavior contract
for those helpers. Ordinary application tests can exercise those paths. Runtime
silence does not establish correct behavior, and unexecuted paths remain open.

The 40-consumer primary browser run takes about 37.4 seconds on this machine.
Its source catalog construction totals about 851 ms, with a median of 19.7 ms
and a maximum of 94.9 ms per consumer. These catalog numbers exclude TypeScript
program creation, getter/class analysis, native checker processes and browser
work. They are observations of this small corpus, not production performance
guarantees. The research driver repeatedly starts native analysis; an editor
integration should reuse project facts and cache premises by their inputs.

The resulting product design has four evidence sources:

1. **Native/compiler facts** for original-code violations and lowering behavior.
2. **Partial source models** for labeled static warnings and candidates.
3. **Development runtime checks** for actual executed violations and exceptions.
4. **Application expectations** for lifetimes and behavior the library cannot infer.

Certification remains useful when a caller requests proof of complete behavior.
Useful feedback can have a separate mode that keeps gaps visible without
requiring every dependency to be fully certified first.

## All-rule limit

The generated result includes an inventory of all 31 current diagnostic
identities. Fifteen rule concepts have authored consumers in this study. This
does not establish full coverage of those fifteen rules: some feedback is an
exception or a source candidate, and after-await is a miss.

Ten other semantic/preference rules are untested here: frozen handlers, props
destructuring, conditional component returns, static dynamic async sources,
directive application, effect callback requirements, synchronous computations
receiving async results, server module directives, server argument transport
and `prefer-show`. Six identities describe uncertifiable/environment states,
including the server HTTP-flush gap. Server/SSR and directive contexts need
their own compiler and execution experiments. No claim of all-rule or
all-package coverage follows from this matrix.

## Evidence and verification

The combined result is
`rust/target/family-feedback-study-v2/results.json`. Its input configuration is
`benchmarks/reviewed-package-models/family-study-inputs.json`. The validator checks
frozen browser source closures before and after execution, executed consumer
hashes, package closure pins, native static input hashes and the projection's
identity. It rechecks real published typing for all fifty records and captures
706 declaration-file digests during validation. Those typing digests do not
retroactively establish a before-execution declaration snapshot.

Failed startup/output directories and the initial empty accessor projection
remain retained. Historical shared modules and their successful observations
were preserved. The evidence remains experimental; the combined report sets
`authority: false` and `certification: false`.

Verification:

- Fresh pinned debug checker built through `make build-checker-debug`; Type Facts
  source stamp matched and required no rebuild.
- Browser consumers, original/twin native analyses, exact-symbol projection and
  combined evidence validation completed.
- 87 prototype tests passed, including eight new tests for symbol identity,
  ambiguity, Unicode mapping, leaf ownership, expectation independence and
  TypeScript exclusion.
- `make verify-fast` passed: producer freshness, Rust formatting and pinned
  workspace Clippy. Syntax, schema, dialect manifest and diff checks passed.
- Full `make verify`, fixture coverage, ownership and certification gates were
  intentionally deferred for this isolated research prototype. No production
  rule, fixture snapshot, accepted contract, schema or Rust source changed.

The next production slice should provide one feedback adapter over these
channels, exact package/application attribution and exception grouping, then
close the demonstrated callback-phase/await gap. Source assumptions and
application expectations must remain distinguishable from proven violations.
