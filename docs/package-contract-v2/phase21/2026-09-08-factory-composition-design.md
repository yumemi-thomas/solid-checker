# Factory export composition: required evidence

Status: initializer transport and receipt-bound root proof implemented; retained Solid 1 measurement shows a later blocker and no published gain. ADRs 0074–0075
establish an accepted exhaustive Seroval return identity in a diagnostic
context. The graph consumer still needs the following independent premises.

## Export initializer identity

Seroval Plugins 1.5.6 does not export the initializer binding directly. Its
development web module declares `AbortSignalPlugin = createPlugin({...})`,
then `abort_signal_default = AbortSignalPlugin`, and exports the latter alias.
The root declaration type is `any`; it supplies no shape premise.

The producer must bind the demanded runtime export to an exact, bounded chain
of same-module variable symbols, each declared once and positively unwritten,
ending in one ordinary initializer call. Alias references must follow their
initializations; cycles, redeclarations, assignments, direct eval, defaults,
destructuring, optional calls and spreads need explicit refusal in the first
rule. Name equality and absence of a record are not identities. Existing
`symbolIsAssignedLocked` handles syntactic assignment targets but does not by
itself account for eval, so it is only one premise of this derivation.

The call must identify its exact callee declaration/export and the actual slot
bound to the dependency's returned parameter. For the measured target that
slot is an object literal. The engine creates a non-callable, non-constructable
object for an object literal; this says nothing about its properties, getters,
callbacks or side effects. The initial consumer should prove only the empty
export-root path and no nested value behavior.

## Accepted dependency identity

Bind the callee to the existing graph's authenticated dependency artifact,
runtime/declaration export binding and accepted semantic digest. Require a
closed returns enumeration with the one supported whole-parameter identity,
and the child receipt's exact closed semantic claim. An open candidate, a
missing receipt, or the diagnostic Seroval receipt from another importer
context cannot satisfy this requirement.

The existing creates-census composition code illustrates the required binding,
but its record and finalizer are specifically for creates-domain obligations.
The new requirement belongs to a positive export-root proof, not to a parent
creates closure. Reusing that channel without extending its ownership would
leave the condition undischargeable or falsely labelled. The finalizer must
commit all such positive-proof dependency requirements, refuse missing or
conflicting discharges, and bind their authenticated receipt/trust roots.

## Existing transport and remaining choice

Before protocol 44, `ExportValueTranscript` provided the declared value and callable
implementation transcripts, not a variable's initializer derivation. The
separate invocation protocol can describe a call but does not itself connect
the call's result to the exported binding. A consumer needs both subjects;
substituting an invocation result for the declared export transcript is not a
proof. Either an explicit initializer derivation or an equally exact joined
source-and-invocation acquisition is required before implementation.

Required negative controls include reassigned and forward aliases, cycles,
another parameter index, callable argument values, a different dependency
artifact or export, open returns, missing/wrong receipts, missing producer
derivations, and a finalizer presented with an undischarged conditional proof.
Positive controls must go through ordinary graph publication and consumer
verification. No coverage gain is inferred from this design.

## Measured post-closure graph attempt

The retained Solid 1 recovery with ADR 0075 and the Seroval recipes completed
with exit 0 in 252.260 seconds, with no archive/metadata cache misses. Its
combined graph refusal still names `seroval-plugins@1.5.6 ./web`'s
`AbortSignalPlugin` export root as lacking positive non-callable and
non-constructable evidence. The mandatory retained baseline separately refuses
the raw Solid `createResource` tuple path, and the proposal fallback publishes.

The [exact comparison](2026-09-08-closed-identity-solid1-measurement.json)
records 29 → 29 artifact cases and 16 → 16 entrypoint names, with no lost
selections or changed export claims. The row remains partial. The result is
`/private/tmp/retained-graph-native-8Aqizu/result.json`, and its graph refusal
is at `audit.graphPreparation.entrypointRecovery.combinedRefusal`.
This confirms the consumer gap persists after the return-closure prerequisite;
it is not a confirmed complete-row opportunity yet.

## Internal producer implementation

`tsgo/export_initializer.go` now implements a private, non-serialized source
derivation for the same-module alias chain. It requires one declaration per
symbol, module-level identifier bindings, positive assignment-target checks,
no eval/arguments mention, initialized aliases in source order, a direct
imported callee and ordinary non-spread call arguments. It records exact object
literal argument nodes and bounds the chain to 16 bindings. It deliberately
does not classify properties or infer the factory's behavior.

The focused `TestExportInitializerRequiresExactUnwrittenAliasChain` passed,
covering direct and aliased calls, transparent wrappers, reassignment of either
binding, forward aliases, cycles, eval (including nested eval), redeclaration,
callable arguments, spread, optional calls and a local non-imported factory.
Protocol 44 now serializes the helper's derivation and validates its joined
subjects at the Rust session boundary (ADR 0076). The helper also resolves an
imported harness query to its exact runtime binding before walking the
same-module chain. ADR 0077 now connects the source derivation to a conditional
export-root proof and independently authenticates the exact importer-bound
dependency return receipt. The full native graph fixture has positive and
negative controls. The [new measurement](2026-09-08-factory-return-solid1-measurement.json)
advances the graph refusal to the raw Solid `createResource` tuple, while
preserving all 29 accepted cases and 16 entrypoint names. The
[verified-floor investigation](2026-09-08-verified-retained-floor-frontier.md)
records why two already-unaccepted generated cases still block the web graph
frontier. No full-corpus rerun or row gain is claimed.

Full verification after this internal producer slice passed with actual exit 0,
TOTAL 135.45 seconds, and no failed-step marker in
`/private/tmp/export-initializer-verify.log`. The Make workflow rebuilt the
producer and matching verifier pins. No protocol field, coverage metric,
snapshot or certification outcome changed in this slice; no new case is claimed.
