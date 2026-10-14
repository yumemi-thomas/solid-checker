# ADR 0302: Deferred apply needs a compute dependency

- Status: accepted (2026-10-14).
- Owner: `effect_execution`, through `Dialect::effect_apply_defer_options`.

## Context

The rc.13 runtime skips initial apply when `defer` is true. A dependency-free
compute cannot schedule it later. SC1001 nevertheless reported a violation
inside that apply. The recall runtime probe observed zero apply calls and
zero warnings, including the reduced compass-ui Bridge mutations.

The installed `@solidjs/signals@2.0.0-rc.13` implementation's `effect` calls
`recompute` and gates initial `runEffect`/enqueue on `!options?.defer`.
`createEffect` and `createRenderEffect` both use it. The published
`EffectOptions.defer` says initial execution waits for the next change.
The [Solid 2 RFC](https://github.com/solidjs/solid/blob/next/documentation/solid-2.0/01-reactivity-batching-effects.md)
describes compute tracking separately from apply execution.

## Decision

An apply's phase is not proof of its invocation. The dialect identifies the
option slot. Literal true needs a proven changing compute dependency;
proven absence suppresses the claim, and unknown dependencies remain
uncertifiable. Unknown options may disable or enable defer, so they supply
no unconditional execution claim. Omitted options, an absent key and
literal false retain initial execution.

The bounded positive proof follows exact immutable identities to a writable
primitive accessor, with a retained setter and no unknown/custom equality
option, read on compute's every-completion path. It also follows ordinary
zero-argument project helpers. Unknown calls, Gets, conditional reads,
derived/readonly sources, async computations and opaque options stay open.
Explicit dialect-proven clearing callbacks supply no compute dependency.
Write/action and directive-creation rows have no invocation-uncertainty
projection; their execution claims are withheld when apply is unresolved.

Reads, owner/leaf diagnostics and callback delivery share this premise.
Render apply no longer inherits an initial owner or contributes initial
reads to its creating caller when defer can skip that run. A named callback's
other invocations remain independent. The error bundle arm has a separate
compute-error trigger and is not inferred absent from dependency absence.
`createTrackedEffect` has no defer option; `createReaction` already forces
defer for the internal effect when its returned tracking function is armed.

## Evidence

`reactive-ir/deferred-effect-apply` pins clean, violation and uncertifiable
cases, named/namespace/wrapper controls and render-effect ownership. Its
signatures are faithful rc.13 excerpts; all source cases also pass
`tsc --noEmit` against installed rc.13 declarations. The original runtime
probe is in the rc.13 corpus's `experiments/recall/runtime-probe.json`.

The original reproduction now certifies with no findings in both source and
contract modes. IR tests (383), dialect tests (96) and a retained-session
process regression pass. The process case edits only a cross-file compute
helper through the session API: changing dependency, unknown dependency,
then no dependency. Reads and cleanup ownership match fresh analysis at
every step; the call-site answers are bound into the cross-file cache digest.

The 49-project rc.13 sweep against a matching `bcaa80d32` parent moves only
Bridge.tsx:379:19 SC1001 from violation to uncertifiable (562 to 561
violations; 3,238 to 3,239 uncertifiable). Its compute reads `stops`, a memo;
this bounded proof does not establish that memo's changing dependencies.
There are no new violations. The requested older `sweep-sol-integrity2.json`
has 3,211 uncertifiable results: 28 additional SC9005 package obligations
and one removed Jar Hell obligation are already present in the required
parent, independent of this change.
