# Snapshot feedback and fresh package challenges

The unchanged heldout population now receives matching feedback on **18/18
target patterns**, up from 14/18. Four informational snapshot hints close the
misses. This is a feedback score; the new hints do not establish four proven
violations or certify their packages.

All twenty original controls remain quiet. Nineteen behave correctly; the
existing pagination cache defect still fails its control. The resulting score
is **37/38 executed consumers and 17/18 passing pairs**. The TS2339 and TS2345
exclusions remain silent. Original consumers, labels, declarations, detector
modules and historical observations retain their bytes.

## How the four misses close

| Original miss | New evidence | Feedback |
| --- | --- | --- |
| Direct `ReactiveMap.get` snapshot | Existing positive class source footprint and setup-to-JSX value flow | Informational source candidate |
| Literal computed `ReactiveMap['get']` snapshot | Exact published member declaration, stable construction and key; analysis-only normalization preserves offsets | Informational source candidate |
| `ReactiveSet.size` snapshot | Existing positive class getter footprint and setup-to-JSX value flow | Informational source candidate |
| `createMousePosition().x` snapshot | Executed package guard skips tracking; original consumer location joins to a setup property read used in JSX | Informational observed candidate |

Class evidence refuses instance escape, member replacement and constructor or
prototype inspection. Computed normalization changes an analysis copy only.
It requires a concrete string key and exact declaration; it does not guess
dynamic dispatch. Inline tracked or unknown callback bodies remain open.

The mouse path uses a generic observer-guard instrumenter. It records the
actual `getObserver` branch in the installed static-store implementation,
without editing installed package files. The adapter hashes the source and
replays the instrumenter to authenticate the exact guard span before using an
observation. Missing, changed or fabricated guard premises acquire no hint.
An explicit `untrack` snapshot remains quiet.

The message identifies the displayed value that was read once, explains that
it stays frozen, and suggests a read inside JSX when it should stay live.
Liveness intent remains undeclared, so severity is `info`, category is
`intent-open`, and `certification` is false.

## Fresh challenges and further improvements

The v4 detector was sealed before authoring another **29 consumers** across
six installed packages. Scroll, resize-observer and media were outside the
original twelve-package heldout population. The other consumers broaden map,
set and mouse forms. All sources, flows, labels and 443 resolved published
declarations were frozen before execution.

| Stage | Matching targets | Quiet, correct controls | Typing exclusions |
| --- | --- | --- | --- |
| v4, frozen before the 29 new consumers | 8/10 | 16/17 | 2 |
| v5, adapted after seeing those observations | 10/10 | 16/17 | 2 |
| v5, sealed before nine further consumers | 2/3 | 6/6 | 0 |

There were no harness failures, failed fresh controls or targets counted using
unrelated feedback. The two readonly assignments produce TS2540 against the
real published types; all checker channels stay silent for them.

The first frozen transfer succeeds for weak collections, constant method
keys, a namespace/wrapper getter, single-field destructuring, scroll position,
window width and breakpoint state. The two misses were a dynamically computed
method key and multi-field destructuring.

V5 uses authenticated executed guards to add hints for those two forms. It
does not invent a static call target: the candidate explicitly retains
`staticDispatch: "open"`. A multi-field hint describes the whole declaration;
the recorded guard does not identify which individual field skipped tracking.
Defaults, rest bindings, deferred JSX functions and callback bodies stay open.
The 10/10 replay is an adapted result, not a fresh benchmark.

After sealing v5, nine further consumers test a weak-map key produced by a
function call, renamed resize imports with aliased destructuring, and
additional default/rest/explicit-untrack controls. The two newly supported
forms transfer successfully and all six controls remain quiet. A third target,
`const frozen = position['x']`, stays stale without feedback. The observed
adapter supports direct properties and computed method calls but has no
computed property-read branch. This miss remains in the score.

## Intent and practical limits

One of the first seventeen fresh controls deliberately keeps an ordinary
class read frozen without an intent marker. Its desired behavior passes, but
it receives the same informational hint as a stale-value mistake. The control
is retained as noise. Expected output and target/control labels never enter
the detector. Explicit `untrack` controls are quiet.

This supports a scalable feedback system built from original-code analysis,
reusable package source facts and development-time observations. It still
requires a distinction between proven findings, source assumptions and
informational intent questions. Snapshot hints should be individually
dismissible or resolved through explicit snapshot intent. Automatically
promoting them to errors would recreate the precision problem.

Remaining limits include the computed property miss, undeclared snapshot
intent, unexecuted browser paths, ambiguous or mutated static dispatch,
unsupported callback/binding shapes, server paths, CommonJS guard loading and
other untested rule families. Package-internal behavior such as the pagination
cache defect needs a separate behavioral specification. These studies do not
establish coverage of every rule or package.

## Cost and retained evidence

Authenticated native analysis is reused only for byte-identical original
consumers and unchanged frozen inputs. The new source channels are recomputed.
The original forty-row replay takes **5.00 seconds**, compared with the earlier
roughly 556-second native static pass. This is an incremental research replay,
not measured editor latency or a replacement for cold native analysis. The
two original mouse consumers require a separate 2.58-second guard browser run.

The 29-consumer browser pass takes 29.15 seconds; its combined original-plus-
challenge replay takes 8.68 seconds. The nine-consumer transfer browser pass
takes 10.97 seconds, followed by a 6.09-second combined replay. Fresh consumers
reuse no native analysis; their reported feedback comes from the new source
channels and actual runtime diagnostics.

Retained artifacts under `rust/target/`:

- `snapshot-v4-detector-freeze.json` and `snapshot-v5-detector-freeze.json`:
  source-module freezes before each new population.
- `snapshot-original-guard-browser-v2/`: unchanged mouse guard observations.
- `snapshot-refinement-original-v4.json`: original 18/18 replay.
- `snapshot-challenge-preflight-v2/` and `snapshot-challenge-browser-v1/`:
  first fresh population and observations.
- `snapshot-refinement-fresh-v4.json` and `snapshot-refinement-fresh-v5.json`:
  frozen and adapted results, retained separately.
- `snapshot-challenge-validation-v4.json` and
  `snapshot-challenge-validation-v5.json`: source, flow, label, declaration,
  package-byte, profile and feedback-span checks.
- `snapshot-transfer-preflight-v1/`, `snapshot-transfer-browser-v1/`,
  `snapshot-refinement-transfer-v5.json` and
  `snapshot-transfer-validation-v5.json`: final transfer including its miss.

## Verification and scope

All **105 prototype tests** pass, including ten new regression tests for exact
computed declarations, escapes, explicit snapshots, authenticated guards,
callback context, binding shapes and TypeScript exclusions. All **179 prototype
modules** pass syntax checks. `make verify-fast` passes producer freshness,
Rust formatting and pinned workspace Clippy. Diff whitespace, schema JSON and
dialect manifest checks pass.

Changes are versioned research modules, this report, README instructions and a
precision-backlog entry. No production Rust analyzer, package contract,
finding snapshot or public manifest changed. Full `make verify`, fixture
coverage, ownership and certification gates were deferred because this slice
changes the isolated prototype only. The new browser and result artifacts are
generated research outputs, not published contract authority.
