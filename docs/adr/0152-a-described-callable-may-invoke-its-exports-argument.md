# ADR 0152: A described callable may invoke its export's argument

- Status: accepted and implemented (2026-09-28); written with the
  implementation. The owner approved nested `callbacks` claims for ADR 0145's
  described callables on 2026-09-28.
- Date: 2026-09-28
- Owners: the semantic model (`DescribedCallback`, `DescribedCall::callbacks`,
  tag 23), the wire and schema, the Type Facts producer
  (`ImplementationCall::callee_unwritten_parameter`,
  `ImplementationCall::unconditional`; handshake protocol 71), the generator's
  described-callable walk (`returns_walk.rs`) and its outer half
  (`main.rs` `propose_returned_literal_captures`), the described-callable
  census and the `result-access` census of a function export
  (`type_facts.rs`), the synthesized veto (`synthesized_vetoes.rs`), and the
  consumer (`contracts.rs` `project_returned_invocations`,
  `execution_role.rs`)
- Relation: fills the `callbacks` domain ADR 0145 left fixed at `[]`, in the
  vocabulary ADR 0100 and ADR 0139 already use. ADR 0145 named this as its
  next vocabulary and ADR 0139's `result-access` as its outer half.

## Context

ADR 0145 states what one invocation of a returned function literal does, and
fixes its `callbacks` at `[]`: a literal that calls anything it did not define
is withdrawn. `@solid-primitives/utils`' `pipe` is the plainest such export:

```js
function pipe(a, b) {
  return (raw) => b(a(raw));
}
```

Its caller hands it two callables. The export's own call runs neither. The
returned function runs both, once each, whenever it is called, on its caller's
stack. Before this ADR nothing could say so: the export's `callbacks` census
counts the captured calls as its own invocations of caller-supplied code and
refuses `[]`, the IR proposes a deferred row it cannot prove, and the `return`
is withdrawn. A consumer then treats a callback handed to `pipe` as one of
unproven timing everywhere.

## Decision

### The nested items

`DescribedCall` gains `callbacks: Vec<DescribedCallback>`. An item is spelled
as a top-level `callbacks` item is, `{from, operation}`, with the operation
written inline -- nothing references it, so it has no id -- in the fields and
spellings an `invoke` operation uses. Inside a described callable every `call`
is the invocation of the described callable. `from` is `{"arg": i, "path":
[]}`, the **export's** argument `i`.

Validation (`validate::normalize_described_callable`) admits exactly one
invocation, the one the census proves:

```json
{"from": {"arg": 0, "path": []},
 "operation": {"kind": "invoke", "trigger": {"event": "call"},
   "at": {"event": "call", "schedule": "same-stack"},
   "tracking": "ambient-at-execution", "owner": {"source": "ambient-at-execution"},
   "count": {"scope": "call", "min": 1, "max": 1}}}
```

That is: argument `i` runs exactly once on every normal completion of one
invocation of the returned value, before it returns, on the invoker's stack, in
the invoker's tracking context and under its owner. At most one item names an
argument; items sort canonically. `returns` may then name
`{"kind": "invocation-result", "parameter": i}` -- ADR 0116's shape, reused --
exactly what that invocation returned, and only for an argument an item names.

`callbacks` is optional on the wire and written only when it names an item:
absent is `[]`, which is what every document before the field meant. A
described callable with items encodes under an appended tag 23; one without
keeps tag 21 and ADR 0145's stream byte for byte, so no existing digest,
receipt or recipe address moves (pinned by the frozen ADR 0145 vector).

A deferred, conditional, repeated or member invocation is **not stated**: the
`return` is withdrawn by name instead. The owner's requirement is that a
callable which invokes its callback conditionally or unknowably stays
uncertifiable in that domain, never a guessed claim.

### The outer half: `result-access` for a function export

The export's own `callbacks` keeps each such argument at ADR 0139's
`result-access` event, whose validated shape is unchanged: the export's call
runs none of them, keeps them only in the value it returns, and the value's
invoker runs them. ADR 0139 confirmed the event only for a construction; this
ADR adds the function arm (`returned_literal_capture_evidence`): every use of
the parameter the producer's use census records is the callee of a call the
producer states is **of that parameter by binding identity** and whose
innermost enclosing callable is a function or arrow literal one of the
export's returns hands back (`ReturnSite::callable`). A use inside a nested
callable is recorded as a capture whatever it does, so the use is identified
with a call that *starts* at it, as ADR 0024 identifies uses with calls. The
same evidence answers the item's positive facts (argument binding, callable
path, reachability, cardinality), and the callbacks census skips exactly those
captured sites of a kept slot, which it refused before (ADR 0100 rule 6).

### The producer states two facts (protocol 71)

- `ImplementationCall::callee_unwritten_parameter`: the censused
  implementation's parameter the callee *is*, by the predicate a returned
  parameter's identity already uses (`unwrittenParameterIdentityLocked`): a
  plain identifier parameter with no initializer, not rest or destructured,
  declared once, written nowhere in the implementation -- nested callables
  included -- in a body mentioning neither `arguments` nor `eval`. Stated at
  any depth of nesting, which `callee_parameter` is not asked to be.
- `ImplementationCall::unconditional`: the call runs exactly once on every
  normal completion of its flow owner. `reach` never said that: it keeps both
  arms of an `if` reachable and ignores `&&`, `?:` and optional chains
  (recorded as an approximation in the precision backlog). The new fact is a
  whitelist walk from the call to its flow owner's body -- each step a
  position its parent evaluates exactly once, unconditionally; every block on
  the way checked for an earlier `return`, `break` or `continue`; the owner
  neither async nor a generator; no optional call. A `throw` before the call is
  admitted, because a completion that throws is not a normal one. False claims
  nothing.

Both are stated on the export's own transcript, so the described-callable
census reads, for each call in the literal's own frame, the call at the same
location in the export's transcript whose enclosing callable is the literal.

### The census

In a described-callable walk (and nowhere else) a call at depth 0 in the
literal's own frame whose export-side twin states `callee_unwritten_parameter`
is the disposition `captured-parameter-call`, recorded apart from the
parameter-rooted family. The literal then states one item per such parameter
when each such call is `unconditional` with `reach: reachable`, and at most
one per parameter; anything else refuses by name ("not stated to run exactly
once", "more than once"). A call of a captured parameter from a callable the
literal nests, or of a defaulted one, is never this disposition and refuses as
before. A completion whose `ReturnSite::call` is such a call is
`invocation-result` of its parameter. The positive fact of each `return`
shares the evidence, as in ADR 0145.

### The veto

A claim whose described callables name items selects a counting module
(`invoking_described_callable_module_source`): each callable slot of the
export's samples is a counting callable of its own, and during each nested call
a slot no claimed item names running, a slot every claimed item names not
running exactly once, or a slot some claimed item names running more than once,
contradicts. An `invocation-result` leaves the nested completion unchecked. A
claim without items keeps ADR 0145's module byte for byte. The `callbacks`
closure's veto is ADR 0100's module, in which a `result-access` slot is outside
every mask: the module hands the returned value to nothing, so a kept slot
running at any time up to the end of the drain contradicts.

### The generator

`returns_walk::described_callable_returns` proposes, per returned literal, one
item for each identifier parameter of the export (no default) that the literal
calls in its own frame by that spelling and does not redeclare, and
`invocation-result` for a completion that is such a call. A proposal input
only; the census reads identity from the producer. `propose_returned_literal_captures`
replaces the IR's deferred rows for those parameters with `result-access` items
(and leaves a slot the IR also saw invoked inline or tracked alone, which the
census refuses).

### The consumer

`project_returned_invocations` answers, for an accepted export, the slots the
returned value invokes on its invoker's stack: `returns` closed with every item
a described callable naming the slot (a union where one alternative does not
say only "may run"), and `callbacks` closed with every item from the slot at
`result-access`. A callback literal written directly at such a slot takes the
role of the returned value's proven invocations
(`contract_returned_invoker_callback_role`, sharing
`role_at_returned_invocations` with the dialect's returned-callback
composition); disagreeing invocations are a deferred callback, as there; with
no proven invocation it answers nothing, and `callee_callback_timing` keeps the
read of unproven timing.

## Consequences

- `fixtures/package-contracts/implementation-census-described-callbacks` is the
  tracer (corpus, and end to end in
  `the_described_callback_census_certifies_exactly_the_unconditional_captured_calls`):
  `pipe` and `changed` byte for byte from `@solid-primitives/utils` and
  `@solid-primitives/promise`, `required`, `registered` certify their nested
  items; `guarded`, `early`, `twice`, `deferred`, `defaulted` withdraw the
  `return` by name; `guarded`, `deferred`, `defaulted`, `registered` refuse the
  `result-access` item by name.
- `fixtures/reactive-ir/package-described-callback-consumer` is the consumer's
  half: a read inside a callback handed to `pipe` is `SC1001` where the
  returned function runs in the component body, clean inside JSX, silent when
  it is never called, and of unproven timing (`SC9005`) with `callbacks` open.
  `tsc --noEmit` is clean on it against the fixture stub and against the
  published `@solid-primitives/utils@7.0.0-next.4`, `solid-js`,
  `@solidjs/web` and `@solidjs/signals` `2.0.0-rc.9`.
- Three corpus exports that returned a literal calling a captured parameter now
  propose the items (`invokesCaptured`, `returned-callback-descendant`'s
  `Direct`, `multi-role-callback-parameter`'s `inlineAndReturned`, the last
  without `result-access` because the export also calls it inline). No findings
  snapshot moved.
- Handshake protocol 71 (after ADR 0149's 68 and ADR 0153's 70): a rebuilt `bin/solid-typefacts` and new certification
  pins.

## What still refuses

- A literal that calls a captured argument conditionally (`callback &&
  callback()`, `chain`'s and `reverseChain`'s loops), more than once
  (`sortable`'s `by` calls `accessor` twice), from a callable it nests
  (`createMicrotask`'s `queueMicrotask`, `createAction`'s `untrack`), inside
  `try` (`safe`), or a defaulted argument (`by`'s `comparator`).
- A literal that calls its **own** argument, or a member of it
  (`preventDefault`'s `e.preventDefault()`): the nested `from` names the
  export's arguments only. That needs a root of its own and the accessor
  question ADR 0120 answers at top level.
- The counting veto observes only runs during a nested call; a named slot
  queued to run after the nested call returned is the census's to refuse.
- The consumer composes only a callback written directly at the slot and only
  through invocation sites `returned_callback_invocation_sites` proves; a
  returned value that escapes elsewhere contributes no role there.
