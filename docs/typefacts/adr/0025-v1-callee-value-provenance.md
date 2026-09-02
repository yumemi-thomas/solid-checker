---
status: accepted
---

# V1 traces callee value provenance and the read families read it

## Decision

The implementation call census carries one new field. Handshake protocol moves
12 → 13 and the schema digest moves with it.

**`calleeSources`.** The traced value provenance of the *callee expression*:
the same `[]ImplementationValueSource` that `ReturnSite.Sources` carries for a
returned expression and `ArgumentSources` carries for an argument, produced by
the same `returnValueSourcesLocked` walk applied to `node.Expression()`, under
the same gates — exactly one declaration, no assignment to the symbol anywhere
but its declaration, no rest element, no default initializer, a reference
positioned at or after the end of its binding's whole `VariableDeclaration`,
and an initializer that is the call itself. It is recorded for a construction
as well, because it is a value trace and not the callee-parameter resolution
the construct branch withholds; a consumer whose claim is about a *call* still
checks `Kind` first.

**It answers a question no other field on the struct answers.** `Target`,
`TargetName`, `TargetModule`, `Declaration` and `CalleeParameter` state the
callee's *resolution* — which symbol, which declaration, which parameter — and
are unchanged. This states what created the value being called. The two coexist
and disagree usefully: for `const [read] = createSignal(0); … read()` the
resolution is a `BindingElement` named `read` and the provenance is
`createSignal`'s tuple slot 0. A consumer that wants "the callee is parameter
N" keeps reading `CalleeParameter`, and a parameter callee traces nothing here,
because a parameter's binding element is not an array binding element of a
variable declaration.

**An empty list is "traced nothing".** Never "the callee is not an accessor",
never "the callee is plain", never a claim about the callee at all. An ordinary
`arr.push(x)` traces nothing, and so do a computed callee
(`(options.storage || createSignal)(…)`), a reassigned `let`, a redeclared
`var`, an imported identifier, and a property read. A nil Go slice omits the
key and the Rust client's `#[serde(default)]` reads the omission as an empty
list; both structs say what the emptiness means and both consumers fail closed
on it.

**Protocol 12 → 13.** `ImplementationCall` denies unknown fields, so the added
field alone is a break in one direction; the other direction matters more. A
protocol-12 producer's *silence* on the field is indistinguishable from "traced
nothing" for every call, and a protocol-13 consumer reading it would conclude
that no callee anywhere has provenance — which is a wrong answer rather than a
missing one, since the whole rule below is existential. `TYPE_FACTS_HANDSHAKE_PROTOCOL`
and `TypeFactsHandshakeProtocol` move to 13 together, `protocolv3_test.go` pins
the number, and `TYPE_FACTS_SCHEMA_SHA256` is regenerated from the file
(`sha256:1e85e91a…`, from protocol 12's `sha256:3d97fa9a…`). No table-schema
change: invocation transcripts do not travel through the compact encoding.

**Consumer: mechanism B, the `read` arm of the operation-input class.** Three
families died in `operation_input_parameter_root` for a `read` operation whose
`inputs[0]` is `ValueShape::Reactive` — `operation-reachability` and
`operation-cardinality` through the `Read` arm of `require_operation_evidence`,
and `recursive-value-shape` through `require_operation_recursive_subject` at
the empty path. All three now reach
`require_reactive_read_operation_input`, which discharges when:

> over the calls with `is_call_expression`, `floor.admits(reach)` and
> `!captured`: **at least one** has a `calleeSources` entry with an empty
> `path` that `traced_source_proves_role` accepts for the demanded role, and
> **none** has one that proves an unambiguous dialect result of a *different*
> role.

`traced_source_proves_role` is reused verbatim from mechanism A: a call result
with a resolved callee, a `(target_name, slot)` pair every dialect that exports
the name agrees is exactly this role, and a module a dialect exports that name
from in value position. The floor is the operation's own
`operation_reachability_floor`, which is `MayExecute` for every inferred read
because the shared constructor stamps `min: Some(0)`; the recursive family, which
carries no bound of its own, derives it from the same operation.

**This rule is weaker than mechanism A, and the weakness is the decision.** A
`read` operation carries **no span** — `ContractReactiveRead` drops the read's
origin and `ValueShape::Reactive` carries nothing — so it cannot be matched to
*a* census call the way an `invoke` operation's input is matched to its
callback's own call sites. Universal quantification is not available either: it
would be vacuously false for every real body, because every ordinary member
call has an empty `calleeSources`. Existential-plus-no-counterexample is
therefore the strongest sound claim the demand as inventoried supports, and what
it cannot distinguish is *which* read. Two reads of the same `(kind, label)`
collapse into one row in `contract_export_function`'s dedup, so the row is
witnessed by a set: every matching call is written into the witness list, so
the receipt records the set it was proved against rather than implying a site.

**`captured` stays a veto here, and that is the load-bearing difference from
mechanism A.** These operations are stamped `at: call / schedule: same-stack`
by `inferred_contract.rs`'s shared constructor, and `ContractReactiveRead` has
no execution or schedule column in which a different schedule could ever be
stated — so the row means unconditionally "this export reads that accessor when
you call it", and the uncaptured gate is the only enforcement of it anywhere.
Mechanism A relaxes `captured` because its claim is about what a call *passes*,
which does not depend on the closure running; this claim is about what the
export *does*, which does. `@solid-primitives/timer`'s `createPolled` forces
the issue in both directions: its uncaptured `depSignal()` at 4105‥4116
witnesses the row, and the `depSignal()` at 4148‥4159 inside
`createEffect(() => depSignal(), …)` must not — and does not, because
`read_escapes_synchronous_extent` already dropped it upstream and the veto would
refuse it here anyway.

**A no-counterexample clause was implemented and removed, and the reasoning
that motivated it was wrong.** The clause refused the demand when any admitted
call proved the *other* role, on the argument that a rule ignoring
`setPolled(v)` would certify a setter write as an accessor read. It would not:
`traced_source_proves_role` is asked about the *demanded* role, so a setter
trace can never be a witness in the first place and the clause added no
soundness at all. What it did add was a false refusal, because writing a signal
and reading it in one body is ordinary —
`const [g, setG] = createSignal(3); setG(4); return g();` publishes an accessor
read row honestly witnessed by `g()`, and the clause refused it because
`setG(4)` sat beside it. A trace to a non-dialect helper, to a locally declared
`createSignal`, or to a slot the dialect table does not answer likewise
witnesses nothing and contradicts nothing; the role comparison is the whole
gate.

## Why

`@solid-primitives/timer@1.4.5-next.1|solid2` refused on
`createIntervalCounter:read-0[0] is reactive/accessor`, and behind it on
`createPolled:read-0`. The IR knows exactly what those rows are: a call whose
callee's type descriptor resolves, through the dialect's type-export index, to a
Solid accessor declaration. The projection to `ContractReactiveRead` keeps only
`(kind, label)`, so by the time the certifier sees the operation the only thing
left is "input 0 is `reactive/accessor`" — a shape with no parameter root, and
the only implementation evidence the certifier had for an operation input was
"this input is parameter *p* of the export".

The provenance the proof needs was already computed for return expressions and
for argument slots, and was one call away for the callee. What was missing was
not a new kind of fact but the same fact at a third position. `solid-js@1.9.14`
alone plans 123 read demands in this class.

Two decisions inside the arm are worth their own justification beyond the
weakness above.

**The kind gate stays even though today's producer states no `calleeParameter`
for a construction.** `new Accessor()` is a different claim about the value than
`accessor()`, and this side does not certify against the producer's habits. A
census row with no stated kind deserializes to `CallKind::Unknown` and is
refused: absence is never read as "call".

**The module premise is what keeps a package's own `createSignal` out.** A
locally declared factory traces with an empty `target_module`, and
`solid_dialect::exports_value_from("", "createSignal")` is false. That is also
why `solid-js` certifying itself stays refused: `createSignal` is declared
locally in its own `dist/solid.js`, so the trace is honest and the module
premise correctly declines it. The self-artifact premise that would answer it is
not implemented.

## Consequences

`@solid-primitives/timer@1.4.5-next.1|solid2|floor` and `|head` certify — but
only together with intra-package operation composition, which is a separate
change in the contract model. Mechanism B alone discharges
`createPolled:read-0` and leaves `createIntervalCounter:read-0` refused,
because `createIntervalCounter`'s census is the single call `createPolled(…)`
whose callee traces nothing.

`solid-js@1.9.14|solid1|only` stays refused on the same demand, family and
subject as before — `recursive-value-shape` `sha256:5463f0ed…`,
`ErrorBoundary:read-0` — with only the *reason* moving, from "the
implementation census binds only parameter-rooted operation inputs" to this
arm's attributable "has no reachable uncaptured call whose callee traces to an
unambiguous dialect reactive/accessor". Every control row re-measured kept its
status, and every refused row kept its demand digest.

Named fail-closed cases this change does not address:

* **Which read.** The row is existential over the export's census, so a
  document with two reads of the same `(kind, label)` publishes one row and one
  set of witnesses. A rule that named a site would need the read's origin in the
  contract model, which `ContractReactiveRead` does not carry.
* **A read reached only through a closure.** The `captured` veto refuses it,
  and correctly, until `inferred_contract.rs` derives `at`/`schedule` instead of
  stamping them. Until then a genuinely queued read cannot be published as one,
  so it cannot be proved as one either.
* **Every read whose callee traces to nothing**: a computed callee, a
  reassigned or redeclared binding, `const c = createMemo(fn); c()` (the
  identifier arm hops only through an array binding element), a property read,
  and a callee that is a parameter — which keeps going through
  `calleeParameter` and `require_parameter_read_evidence`, a different witness
  against a different fact.
* **`Plain`, `Object`, `Callable`, `Tuple`, `Choice` and `Store` inputs** stay
  unsupported, for the reasons ADR 0024 records.
* **`createResource`, `useTransition`, `createDeferred`, `createSelector`,
  `createOptimistic` and the store family** carry no `reactive_result_slot` row
  at all, in either dialect.
* **The self-artifact premise** is still unimplemented, so `solid-js`
  certifying itself stays refused regardless of this arm.
* **`target_module` remains the written import specifier**, not a resolved
  package identity — the same approximation `require_return_callable_source` and
  `traced_source_proves_role` already make.
* **Cross-package composition** is not this change's subject and is not
  implemented; see the composition entry in `docs/precision-backlog.md`.

## Receipt compatibility of the composition half

Mechanism B alone touches no digest. The composition half adds
`Operation.composed_from`, and folding it into `canonical::operation` through
`option` — which stamps a discriminator whether or not the field is set — would
have moved the semantic digest of **every** contract carrying any operation,
and with it every policy-2 receipt already issued for one, while
`schemaVersion` and `semanticModelVersion` both stayed 1. That is a
receipt-compatibility break, which version 1 does not get to make.

Omitting the `None` encoding inside a single stream is not the alternative: a
streaming hash carries no descriptor, so a field written only when present is
self-delimiting merely by argument about how the neighbouring fields happen to
encode. The change therefore uses **domain separation**. A contract in which no
operation carries provenance emits the legacy stream byte for byte under
`SEMANTIC_DIGEST_DOMAIN` and keeps its digest and its receipts; a contract with
at least one emits the provenance stream under
`SEMANTIC_DIGEST_DOMAIN_COMPOSED`. Each family is injective on its own, the two
cannot collide because the domain is the length-prefixed first thing written,
and the family is a function of the contract rather than a mode a caller
chooses. Measured: the frozen golden vector is unchanged at
`sha256:23c3aef3…`, the contract corpus moves only the two new
composition fixtures, and every ecosystem demand digest returns to its
pre-change value. A provenance-carrying contract is a new document making a new
claim, and it gets a digest in its own family.
