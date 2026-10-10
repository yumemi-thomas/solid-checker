---
status: accepted
---

# V1 traces argument value provenance and narrows the value tracer

## Decision

The implementation call census carries one new field, and the tracer that fills
it is narrowed. Handshake protocol moves 11 → 12 and the schema digest moves
with it.

**`argumentSources`.** Per written argument slot, the traced value provenance of
the expression written in that slot: the same `[]ImplementationValueSource`
`ReturnSite.Sources` already carries for a returned expression, produced by the
same `returnValueSourcesLocked` walk. One entry per written argument slot,
parallel to `argumentParameters` and subject to the same `exactArgumentSlots`
gate, so a slot a spread has displaced carries an empty list rather than a trace
of the expression written at that position. The empty list is written as an
empty list, never omitted for that slot: the invariant a consumer indexes by is
one entry per written argument.

**An empty list is "traced nothing".** It is never "this argument is not an
accessor", never "this argument is plain", and never a claim about the slot at
all. The tracer follows parenthesized expressions, array literals, callable
expressions, call results, and one hop through an array binding element that
meets all five premises below; every other expression — a conditional, a
property read, a computed callee, a reassigned or redeclared binding, a rest or
defaulted element, a reference before its declaration — leaves the slot empty.
A short list is the same absence. Both structs say so, and both consumers fail
closed on it.

**The identifier arm is narrowed, on five premises.** It followed
`symbol.Declarations` and took the first array-binding element it found, so four
shapes stated a provenance that was not the value: `let [a] = f(); if (c) { [a]
= g(); }` traced to `f()`, and a redeclared `var` did the same; `const [...rest]
= createSignal(1)` traced the *tail array* to slot 0; `const [a = fallback] =
createSignal(2)` stated slot 0 for a value the default may have replaced; and
`cb(hoisted); var [hoisted] = createSignal(1);` traced slot 0 for a reference
that reads `undefined` — which `tsc` does not report for a `var`. The arm now
requires:

1. exactly one declaration for the symbol;
2. no assignment to the symbol anywhere other than its declaration
   (`symbolIsAssignedLocked`, i.e. the checker's own assignment-target symbols
   rather than a source-text scan);
3. no rest element;
4. no default initializer on the element;
5. the reference positioned at or after the end of its binding's whole
   `VariableDeclaration`, in the same file — which also refuses a
   self-reference inside the initializer.

The slot is computed by position among the pattern's elements, so an omitted
element (`const [, set] = …`) still holds its place. The one-hop, no-cycle bound
is unchanged. Premise 5 over-refuses a reference written earlier inside a
closure that runs later; that is recorded rather than special-cased.

**The single-assignment premise is the assignment census, not `const`.** An
`ast.IsVarConst` requirement was implemented first and reverted: bundler output
across the measured corpus — `solid-js@1.9.14`'s own `dist/solid.js` among it,
along with `@tanstack/query-devtools` and `@corvu-next/dismissible` — destructures
`createSignal` with `let` or `var` and never reassigns, so the `const` gate
refused real rows for no soundness the assignment census does not already
provide. A `let` that is never an assignment target is traced; a `let`
reassigned in any branch, and a `var` declared twice, are not.

**The narrowing applies to `ReturnSite.Sources` as well, and can only remove
authority.** That surface has had this hole since it existed. Narrowing the arm
withdraws sources; it never adds one, so every consumer of return sources can
only refuse more than before. `require_return_callable_source` is the one
consumer, and the measured effect on the corpus was none: every certified
control row that reads it, `@solid-primitives/jsx-parser@0.2.0|solid1|only`
included, stayed certified.

**Producer: the use census records an object-literal shorthand.**
`parameterUseCensusLocked` skipped every identifier for which
`ast.IsDeclarationNameOrImportPropertyName` holds, and a shorthand property
assignment's name is one — so `const holder = { cb }` recorded **no use of
`cb` at all**, while `{ cb: cb }`, `[cb]`, `(0, cb)` and
`new Map([["k", cb]])` each recorded `unknownEscape`. A shorthand whose name
resolves (through `GetShorthandAssignmentValueSymbol`, since the symbol at that
location is the *property*) to a censused parameter is now recorded as
`unknownEscape` at the identifier's location; `{ ...{ cb } }` is the same node
kind and is covered with it. This strengthens every consumer of the use census,
not only the new arm: measured, no fixture finding and no control row moved.

**Consumer: one new evidence arm.** `require_operation_recursive_subject` gains
a reactive-input arm, answered before the input's parameter root is required.
It claims a demand only when the root is `OperationInput{operation, index}`, the
operation's kind is `Invoke`, the demanded path is the root, the demand asserts
no callability, and the input shape is `Reactive { role }`. It discharges only
when *every* call of the exact callback parameter that the implementation may
reach traces slot `index` to a call result whose `(target_name, target_path)`
resolves — through the new dialect table — to exactly the demanded role, from a
module a dialect exports that name from in value position.

**And only when the use census accounts for the callback as exactly those
calls.** The call census states `calleeParameter` only for a callee that
resolves to the parameter exactly, so quantifying over it alone is not universal
quantification: measured against the real producer, `const f = cb; f(plain)`
states no callee at all, `cb.call(null, plain)` and `cb.apply(…)` state the
parameter at path length 1 (which the exact match refuses), `Reflect.apply(cb,
…)` and `holder.cb(plain)` state none, and `new cb(plain)` is a construction for
which the producer states no `calleeParameter` by design. Each of those, paired
with one honest `cb(accessor)`, satisfied "every matching call proves the role"
while the export handed the callback a plain value.

The arm therefore also requires that every use rooted at the callback parameter
that the floor admits **is the callee of** one of the calls it just proved.
*Not* "lies inside" one: byte containment was tried and is wrong, because
`cb(text, cb)` and `cb(text, (held = cb))` put an `argumentKnown` and an
`unknownEscape` use inside the proved call's own span. Identity is positional —
a call expression begins at its callee, so an identifier callee starts at the
call's start byte and ends inside it, `cb?.(x)` included. A parenthesized
`(cb)(x)` starts the call one byte earlier and is refused; that over-refusal is
pinned as a decision.

This subsumes a use-kind gate: `storage`, `aliasCall`, `propertyAccess`,
`argumentKnown`, `return` and `unknownEscape` uses are the callee of no proved
call, and neither is the `directCall` use inside `new cb(plain)`. It keeps the
shape that must stay accepted: marker's `cb(text)` inside `createRoot(dispose =>
…)` is a `capture` use that *is* its proved call's callee.

**The premise depends on the use census being exhaustive, so that is asserted
rather than assumed.** Two conditions, both refusing rather than approximating:
the transcript must be `complete` with **no** open reason at all — not even
`controlFlowUnsupported`, which `require_export_implementation` otherwise admits,
because the use census withholds rows inside an unsafe-jump region and a
withheld escape is the one thing this premise cannot afford. The blast radius
is wider than the jump regions that motivate it: the producer also reports
`controlFlowUnsupported` for an implementation body containing a plain `while`,
`for`, `try`, or `switch` with no jump at all, so this arm can never prove an
export whose body has a loop, a `try`, or a `switch` — the `mapArray` /
`indexArray` shapes among them — independently of mechanism B; and the callback
must be bound to a *whole* parameter, because the census is rooted at parameters
and for a callback at `Parameter{0, ["cb"]}` a use of `props` is
indistinguishable from a use of `props.cb`. Both are pinned by
`reactive_operation_input_requires_an_answerable_use_census`.

**Dialect seam.** `solid_dialect::unambiguous_reactive_result_slot(name, slot)
-> Option<ReactiveRole>` over `ResultSlot::{Whole, TupleItem(n)}`, backed by a
per-dialect `Dialect::reactive_result_slot` table, plus
`solid_dialect::exports_value_from(origin_module, name)` wrapping the export
index. Nothing about accessor-ness lives in the IR or the certifier. Silence is
disagreement: if one dialect that canonically exports the name answers a role
and the other answers nothing, the aggregate answers `None`, so one dialect's
audited row never speaks for the other's unreviewed name.

**Refusal message.** `parameter_source` became
`operation_input_parameter_root`, which receives the demand and the operation
input's identity and emits

```
Type Facts demand <id> is unsupported: operation input <operation>[<index>] is
<shape>, and the implementation census binds only parameter-rooted operation
inputs (family=<family>)
```

with `<shape>` a stable per-constructor spelling. The operation identity an
emitted document carries is already qualified by artifact case and export, so
the input is named once. `packages/cli/scripts/certify-contract.mjs` reads the
demand and family back out of the native refusal, so the audit sidecar records
them instead of `demandId: null, family: null`.

## Why

`@solid-primitives/marker`'s `createMarker` creates a signal and hands the
accessor to the caller's callback: `const [text, set] = createSignal(matchText);
… mapMatch(text)`. The emitted contract states that `callback-0`'s input 0 is
`reactive/accessor`, which is true and is the fact two live consumers in the
user's project depend on — `source_discovery.rs` registers the user's callback
parameter as a reactive source from it. The certifier could not prove it: the
only implementation evidence it had for an operation input was "this input is
parameter *p* of the export", and this input is not a parameter of anything. It
therefore refused with `demand: "operation-input"` — a literal, not a demand id
— and three marker rows, two timer rows and solid-js all produced the same
unattributable sentence.

The provenance the proof needs was already computed for return expressions and
was one call away for arguments. What was missing was a *slot-level* dialect
answer: `creates_reactive_source` says a call produces a source but carries no
slot, and `unambiguous_callable_result_tuple_item` says both `createSignal`
slots are callable — a proof built on either would have certified the setter as
an accessor.

Three decisions inside the arm are worth their own justification.

**Universal quantification over call sites.** `cb(accessor); cb(plainValue);`
makes "there is a call that hands an accessor" true and the operation's input
shape false. So every admitted call must prove the role, and a call whose slot
traces to nothing refuses the whole demand rather than being skipped.

**The floor is `MayExecute`, and this is the one operation-input branch where
that is not a relaxation.** The other branches assert what a position *is* and
keep the strict floor, because a call in a loop body proves a parameter callable
no more than it proves the loop runs. This demand's claim is conditional in the
same shape a conditional call is: *whenever* this invoke happens, input `index`
has this shape. Whether it happens at all is `operation-reachability`'s
question, answered separately through `callback.from`. `Unreachable` still
clears nothing — a call after a `return` neither witnesses the shape nor
contradicts it, and a census whose only matching call is unreachable leaves the
witness set empty and refuses. marker forces the issue: `mapMatch(text)` is
`reach=Unknown, captured=true`, inside `createRoot(dispose => …)`.

**`captured` is not a veto, but it is recorded.** A claim about what a call
passes does not depend on proving the closure holding it runs. The enclosing
callable is written into the witness site so the audit says which body each
site sits in, and a producer that reports `captured` without naming one is
refused rather than recorded as if the call sat in the export's own body.

## Consequences

`@solid-primitives/marker@0.2.2|solid1|only`,
`@solid-primitives/marker@2.0.0-next.2|solid2|floor` and `|head` certify.
Control rows were re-measured and none moved. Everything else in this
refusal class stays refused, now attributably; the exact remaining cases are
listed in `docs/precision-backlog.md` under this change's entry.

Named fail-closed cases this change does not address:

* **Read families (mechanism B).** A `read` operation's `Reactive` input carries
  no span — `ContractReactiveRead` drops the read's origin location and the
  shape carries nothing — so the operation cannot be matched to a census call
  and this arm does not claim it. `@solid-primitives/timer@1.4.5-next.1|solid2`
  and solid-js's 123 read demands stay refused there. No `calleeSources` field
  was added; the design that would consume it is not implemented.
* **Interprocedural composition.** `createIntervalCounter`'s census is the
  single call `createPolled(timeout, options)`; nothing in its own transcript
  witnesses the read its row claims. This needs provenance of a *composed*
  operation in the contract model, not a producer fact.
* **The self-artifact premise.** solid-js certifying itself would need "the
  traced callee's resolved declaration lies inside the artifact case under
  certification and is that artifact's own export of `name`", because
  `createSignal` is declared locally there and `target_module` is empty. Not
  implemented; the module premise refuses it.
* **`Plain`, `Object`, `Callable`, `Tuple`, `Choice` and `Store` inputs** stay
  unsupported. `Plain` carries a negative callability claim the census cannot
  make about an argument value; `Object`/`Tuple` need a property or slot path
  the tracer does not produce for object literals; `Store` needs its own dialect
  row and its own review.
* **`createMemo`'s row is reachable, but only inline.** Both dialects answer
  `Whole = Accessor` for it, from the published declarations, and
  `cb(createMemo(fn))` reaches that row: the call-expression arm emits a source
  with an empty target path, which is `ResultSlot::Whole`. What does *not*
  reach it is a binding — `const c = createMemo(fn); cb(c)` traces to nothing,
  because the identifier arm hops only through an array binding element and a
  plain `VariableDeclaration` is not one. That is the gap, not the row.
  `createResource`, `useTransition`, `createDeferred`, `createSelector`,
  `createOptimistic` and the store family carry no row at all.
* **`target_module` is the written import specifier**, not a resolved package
  identity. A project that aliases `"solid-js"` to another package would satisfy
  the module premise. This is the same approximation
  `require_return_callable_source` already makes; the existing `== "solid-js"`
  literal sites were left alone in this change.
* **"Silence is disagreement" is enforced in code but unpinned by a row.** No
  name is currently exported by both dialects with a reactive result slot only
  one of them answers, so the aggregate's disagreement branch has no corpus
  witness; `one_dialect_row_does_not_answer_for_the_other_dialects_silence`
  asserts the per-dialect answers directly instead, because the aggregate alone
  cannot tell "neither has a row" from "one does".
* **A callback bound to a property path is refused, not proved.**
  `Parameter{0, ["cb"]}` is a legitimate binding the arm declines, because the
  use census cannot separate uses of the object from uses of the property. A
  premise that skipped those rows would be vacuous exactly where the value can
  escape.
* **The position premise over-refuses a later-running closure.** A reference
  written before its binding's declaration but executed after it — a closure
  stored and called later — traces nothing.
* **The callback binding is resolved with `.find()`**, which takes the first
  callback whose `operation` is the demanded one. That is exact only because
  operation ids are per-callback in every emitted document seen so far; a
  document that bound two callbacks to one operation id would have its first
  one proved and its second ignored.
