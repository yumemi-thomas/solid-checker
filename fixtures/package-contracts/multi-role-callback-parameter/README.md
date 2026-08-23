# Two executions for one parameter is not two claims, it is one false one

One `callbacks` row is pushed per *invocation site*, and `push_contract_callback`
dedups only exactly equal rows. A parameter invoked twice with two schedules
therefore published **both** rows, and both were schema-valid: the schema's
`uniqueItems` compares whole objects, so `{parameter: 0, execution: "inline"}`
and `{parameter: 0, execution: "tracked"}` coexist happily.

Schema v1 has one execution axis per parameter and one analyzed target has one
runtime behavior, so at least one of the pair is false, and a consumer that
picks either one is guessing. The qualifier matters: a *conditional* export can
legitimately behave differently per target, and there the exact per-branch
claims live in `variants`. That does not license the pair in the base either —
the base is environment-unaware and has the same one axis — and
`conditional-callback-conflict` is the fixture that pins it, because the
sentinel here runs per analyzed target and the cross-target union happens
afterwards, in `mergeSummaries`. Worse, the pair is *guaranteed* to produce a failing measurement: a
probe observes one behavior, so whichever row it does not match fails.
`@solid-primitives/range`'s `mapRange` carried `callbacks[2]` as `deferred` *and*
as `tracked`, and the corpus report lists both as failing with `observed
inline`.

The per-export unknown sentinel is the encoding schema v1 has for "this domain
cannot be stated", and it is what `escaped_parameters` already opens for a
retained callback (fixtures/package-contracts/retained-callback-parameter).

| Export | claim | why |
| --- | --- | --- |
| `inlineAndTracked` | `{"status":"unknown"}` | an inline body site and a tracked compute site — `createDerivedSpring` / `createDebouncedValue` |
| `inlineAndReturned` | `{"status":"unknown"}` | an inline body site and a site inside the returned accessor — `mapRange` |
| `twoTrackedSites` | one `tracked` row | negative: two sites, one schedule; identical rows dedup |
| `twoParameters` | `inline` at 0, `tracked` at 1 | negative: the axis is per parameter, so this is not a contradiction |
| `oneTrackedSite` | one `tracked` row | negative: the single-site control |
| `oneInlineSite` | one `inline` row | negative: the single-site control, other direction |
| `contradictOnZeroOnly` | `{"status":"unknown"}` | parameter 0 contradicts, parameter 1 is undisputed — and the whole domain still goes unknown |

Before this change `inlineAndTracked` published `[inline, tracked]` and
`inlineAndReturned` published `[inline, deferred]`; the four negatives are
unchanged. `twoTrackedSites` is the negative that rules out the cheaper rule
("more than one row for a parameter opens the sentinel"), which would discard a
proven claim here.

Rows that agree on `execution` and differ elsewhere — argument descriptors, an
owner — are deliberately *not* contradictory: those are additional facts about
one schedule.

## The sentinel is wider than the contradiction, on purpose

`contradictOnZeroOnly` is the case that states it: parameter 0 contradicts
itself, parameter 1 carries one proven `tracked` row that nothing disputes, and
parameter 1's row is discarded with the rest of the domain.

Narrowing it is not available in schema v1. The only granularity below
`{"status": "unknown"}` is whether a row is present, and an absent row is a
certified *negative* — "this export never invokes a caller-supplied callback at
that parameter", which is the claim the review plan's "no callback execution
row" section exists to make a reviewer sign off on (docs/package-contracts.md).
So dropping only parameter 0's rows would trade one contradiction for one
affirmative false negative, and "unknown at parameter 0, proven at parameter 1"
has no encoding at all. The per-export sentinel is the honest answer, and it is
the same width the pre-existing `escaped_parameters` sentinel has for a retained
callback.

## What this does not yet prove

The sentinel is emitted from the contract's own bytes, so the review plan lists
it as an `unknown-sentinel` item with no `because` attribution naming the
contradiction. Attaching a `contradictory-callback-execution` reason needs the
obligation label to be plumbed through the emitter in
rust/crates/solid-facts-backend/src/main.rs, which hardcodes
`UnknownCallbackExecution` / `contract-generation-obligation` for every callback
sentinel today. Recorded in docs/precision-backlog.md.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes solid-js@1.9.14's
`types/reactive/signal.d.ts` for the two primitives used, dropping only
inference machinery no claim here depends on.
