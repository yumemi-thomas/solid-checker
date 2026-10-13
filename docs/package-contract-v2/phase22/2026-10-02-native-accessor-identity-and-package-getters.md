# Native accessor identity across files and packages

## Conclusion

There is reason for cautious optimism. A development runtime can recognize
reactive getters returned by packages without a behavioral contract for every
wrapper. The new experiment catches 11 of 15 authored stale-result targets,
including imports, mutable aliases, shared factories and three actual package
primitives. All paired corrections work. Four targets still escape detection,
and a deliberate inspection still receives an informational hint.

This establishes a useful mechanism for one family of mistakes. It does not
establish coverage of every package, rule, execution path or user intention.
The earlier real-app inventory found no new verified async defect. Product
precision, broad app coverage and development cost still need measurement.

## Mechanism

`native-accessor-hook-v1.mjs` inspects the exact retained rc.9
`@solidjs/signals/dist/dev.js` artifact. Its accessor constructor binds the
native reader to a node and returns that function from signal and memo
factories. The hook records the returned function in a WeakMap and returns
the same object. It imports the observation runtime before native factory
initialization, so getters created during application-module initialization
are included.

`native-identity-sites-v1.mjs` selects exact declared, zero-argument identifier
calls in admitted returned expressions of callbacks created inside exact
native memos. Unlike the earlier selector, it does not need a local native
getter-creation declaration. Imports and mutable aliases can be candidates.
Unknown functions are candidates only; runtime identity is required for a
read observation.

`async-read-transform-v5.mjs` uses the existing project session and necessary
syntax gate. Its helper calls the original function once, returns the original
value and propagates exceptions. It neither restores an owner nor introduces
an observer. Each tagged getter carries context probes from its own native
runtime instance. Getter identity, native properties and return identity are
preserved in focused tests. The added call frame and stack collection are
observable costs; universal instrumentation neutrality is not claimed.

`native-identity-feedback-v1.mjs` authenticates current source bytes, exact
declarations, the selected memo/callback spans, native constructor bytes and
mapped creation/read frames. An observed read with both owner and observer
absent produces `OBSERVED_MEMO_CALLBACK_UNTRACKED_READ`, with severity `info`,
category `intent-open`, and `staticDispatch: open`. It does not certify package
timing, returned value flow or reactive intent. Explicit native `untrack` and
discarded-result cases retain the earlier suppression rules.

No package name, authored expectation or displayed result decides whether a
hint is emitted. WeakMap identity supplies observed provenance instead of
static trust in a package wrapper. A user can mutate the observation runtime;
these traces are research evidence, not an adversarial root of trust.

## Population and results

The mechanism uses Solid and signals 2.0.0-rc.9. Actual package getters come
from `@solid-primitives/pagination@1.0.0-next.8` and
`@solid-primitives/media@4.0.0-next.2`; their retained published declarations
and package closures remain unchanged.

| Observation | Result |
| --- | --- |
| Authored stale-result targets | 11/15 receive the new hint |
| Working controls quiet in the identity channel | 14/15 |
| Working controls quiet across identity and existing native feedback | 13/15 |
| Real typing exclusions | 4/4 silent, excluded before execution |
| Harness failures | 0 |
| Independent creation/read witnesses | 12, including the noisy inspection |
| Plain/instrumented comparison | 34 cases, no behavior or native-delivery changes |
| Additional source comparison | 16 file instances, unchanged |

Eleven caught targets cover shared module signals, barrel re-exports,
namespace-derived aliases, returned signal factories, a returned memo factory,
mutable and conditionally assigned aliases, a valid generic factory,
pagination's page accessor, segment accessors and media-query accessors.
For every caught target, the displayed result stays at its initial value after
the underlying source changes. The paired control captures the source during
memo compute and updates to the desired value.

The four misses also retain stale results:

- A wrapper function calls a native getter but has a different function identity.
- A getter copied with `bind` has a different identity.
- A member call is outside the current candidate grammar.
- A read passed through `Promise.resolve` has unknown argument/result flow.

The new informational noise is a returned inner inspection whose value is
saved for debugging while the memo resolves a deliberate constant. Actual
native identity and absent tracking context are insufficient to establish
reactive intent. This limitation from the earlier experiment remains.

The additional existing warning on the pagination control is
`STRICT_READ_UNTRACKED`. The mapped package frame is its initialization read of
`opts().initialPage` in `pagination/dist/index.js:72`. The warning also appears
without instrumentation. It is retained as native feedback and reduces the
combined quiet-control count; it is not counted as a new identity hint or as
proof of consumer misuse.

The first preflight refused a generic helper because unrestricted `T` does not
meet the published signal initializer overload. Its two invalid cases remain
as typing exclusions with TS2769. Valid counterparts use
`Exclude<T, Function>` and an explicit generic signal invocation. TS2554 and
TS2322 exclude the other two cases. No fixture stub replaces published types.

## Evidence and freeze discipline

All modules that existed when the identity detector was frozen are unchanged.
The initial 282-file seal and supplemental seals are retained. The final
284-file seal includes the independent audit correction and the original
consumer module before the valid generic counterparts were authored.
There was no detector adaptation after browser execution. The original
invalid preflight directory and unexecuted faulty audit version remain visible.
The completed study reports `detectorFrozenBeforePopulation: true`.

The new browser profile authenticates all 285 source/support pins and package
closures before and after each run. It records hashes for every additional
consumer file. Browser V5 maps both the native constructor stack and consumer
read stack through source maps. `native-identity-audit-v2.mjs` imports neither
the site selector nor the transform/projector and independently inspects the
native bound-reader constructor, published typing boundary and original spans.
It audits all 12 hints, including the deliberate-inspection noise.

Primary retained observations:

- `rust/target/native-identity-preflight-v2/population.json`
- `rust/target/native-identity-browser-v1/browser/results.json`
- `rust/target/native-identity-browser-plain-v1/browser/results.json`
- `rust/target/native-identity-study-v1.json`
- `rust/target/native-identity-audit-v2.json`
- `rust/target/native-identity-parity-v1.json`
- `rust/target/native-identity-additional-source-parity-v1.json`
- `rust/target/native-identity-historical-seals-v1.json`

The plain comparison preserves all original consumer hashes, package pins,
published typing errors, displayed values, callback counts, native diagnostic
deliveries and caught exceptions. Additional-file hashes also agree.

## Verification and remaining scope

The prototype suite passes 229/229 tests, including 13 new identity tests.
All 284 prototype modules pass syntax checks. Six historical/current seals
authenticate 284 distinct pins. `make verify-fast`, schema parsing, dialect
manifest validation and whitespace checks pass. The producer freshness check
reuses the existing matching producer; workspace formatting and pinned Clippy
pass. Full `make verify`, production coverage/ownership, contract corpus and
certification gates are deferred because this slice changes research modules
and documentation only. No production Rust rule, public contract, manifest or
finding snapshot changes. Generated research observations live under
`rust/target/`.

The hook supports one exact native artifact and refuses changed bytes. Consumer
selection is still limited to the existing lexical memo/callback and expression
grammar. Object/store property reads, wrapper functions, bound copies, unknown
argument flow, callbacks declared elsewhere, other native versions, HMR and
server execution remain open. Project references remain unsupported. Only
executed paths supply runtime feedback, and informational intent noise remains.
Capturing a stack at every accessor creation costs time and memory; no reliable
overhead bound was measured in this experiment.

The practical direction is a development feedback system combining native
diagnostics, observed ownership/read facts and small source models where
needed. The evidence supports investing in that approach. Whole-package
certification and universal rule coverage remain separate, unestablished claims.
