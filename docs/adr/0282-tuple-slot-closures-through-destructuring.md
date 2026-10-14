# ADR 0282: Tuple slot closures through destructuring

- Status: accepted and implemented (2026-10-13).
- Owner: `accessor_provenance::Resolver::tuple_slot`, with
  `returned_tuple_elements`, `simple_array_binding` and `slot_closure`. The
  call branch of `Resolver::value` is the only place that builds a
  `Value::Tuple`.
- Found while analyzing `@solid-primitives/raf`'s `createRAF` as app source
  (dependency-source experiments, 2026-10-13); research report
  `rust/target/research/dep-mode/tuple-returns.md`, probes
  `solid-checker-rc13-corpus/experiments/sol-tuple/p1..p10`. This is the
  report's slice 1. Slice 2, dead return sites under a stated host, is not
  part of it.

## Context

ADR 0273's transport follows `const { start } = make(); start()` to the arrow
that `make`'s returned object literal holds. ADRs 0275–0280 then decide
whether that arrow's read is a violation at the call. The tuple form,
`const [running, start, stop] = make(); start()`, went nowhere. An
array-shaped binding fell through to the signal-tuple witness, then to
`None`. The call was no graph edge, emitted no read and raised no obligation:
silence, not even `SC9012`. Only the computed `t[1]()` was an obligation.

## Decision

A destructured tuple slot names a closure when all of these hold:
- The pattern is a `const` array pattern of plain identifier slots: no
  default, no rest, no nesting. Holes are allowed (`[, start]`). Its
  initializer is one call, written directly.
- The call resolves to a current, synchronous project function (as for the
  object form, through `straight_line_calls`). The function has exactly one
  own return, which is unconditional and has no control tests.
- That return's argument is itself an array literal with no hole and no
  spread, and its elements are not all closed objects (those stay ADR 0273's
  `map`/`forEach` receivers).
- The slot's element is an inline arrow, or an identifier for an unwritten
  `const` arrow binding that the factory declares in its own body. Arrows
  only, as in `member_value`.
- The call site and the pattern are in the same file, and no wildcard
  member-write veto exists in the project (ADR 0273's `all`).

The slot then resolves to `Callable::Function` of that arrow. The existing
transported-call consumer adds the edge and the ADR 0276 standing, so no
interprocedural code changes.

Why each refusal exists:
- A held tuple is refused: `const t = make()`, a parameter handoff, a context
  provider's value, or `return make()` forwarding another factory's tuple.
  Whoever holds the array can replace a slot before it is read, and the
  provider's array is shared by every consumer. A fresh literal consumed by
  the pattern in the same statement is the only entry with no other holder.
  For the same reason, sibling names escaping (`onClick={stop}`) refuse
  nothing: a `const` name still calls the closure it was bound to.
- A hole or a spread in the returned literal shifts later positions. A
  pattern default, rest or nesting binds a name with no exact slot.
- A `function` expression's `this` is outside the summary, and a function
  declaration's binding is hoisted and writable. A module binding, an import,
  a parameter, a call result and a `let` are not the binding the factory
  declares, so their identity would need a census this slice does not run.
- An accessor slot (`running`) belongs to source discovery. It is not claimed
  a second time.
- More than one return site, `if (isServer) return [...]` included, is refused.
  The arms disagree (`noop` against `start`), and nothing decides the guard
  without the host constants of slice 2.

Premise: array destructuring runs `Array.prototype[Symbol.iterator]`. Its
stability is assumed, in the class of ADR 0273's visible-veto-only premise. A
write whose key no literal names is the one veto this slice can see.

## Consequences

- `const [running, start, stop] = make(); start()` in a component body is
  `SC1001` when `start` reads on every completion. This covers the cross-file,
  same-file and default-import cases.
- An inline arrow slot is outside the producer's function universe, so its
  call now gets the `returned-callable-outside-source-universe` `SC9012`
  obligation, as an inline object member already did. This includes a call in
  JSX: `fp-summary-returned-closure`'s `positive()` gained two such rows.
- `createRAF` itself stays silent. Its body has the `isServer` guard return,
  which is slice 2. (ADR 0288: under `--dependency-source` the ADR 0284 fold
  already removes that return; what silences raf there is the wildcard veto
  below.)
- Only function declarations resolve across files, as for the object form.
  Reads through a captured factory parameter stay unattributed.
- No cache change. The slot rides the existing transport map, which
  `transport_identity_matches` compares whole, and the target arrow's summary
  is an `InlineFunction` dependency. `single_return` and every shared answer
  are untouched, so `cross_file_proof_digest` needs nothing new.
- No fact, schema, wire or contract change.

## Amendment: iteration mutation veto (2026-10-14)

The original prototype-spelled reflective-target test missed shipped false
positives: `Array.prototype[Symbol.iterator] = replacement` changes which
closures destructuring binds. The review's `nsb` runtime yields harmless
closures, so following the original slot's signal read was unsound.

Returned-tuple identity now has its own visible iteration-mutation census.
Normalized assignment/update/delete targets and exact library reflective
mutations cover `@@iterator`, iterator `next` and early-close `return`, plus
`__proto__`/`Object.setPrototypeOf`/`Reflect.setPrototypeOf`. Aliased
`Array.prototype`, `%ArrayIteratorPrototype%` obtained by `getPrototypeOf`,
unknown keys, unknown receivers, spread arguments and opaque descriptor/source
maps veto. A library-signature mutation alias retains a possible mutation;
it grants no runtime callee authority. A missing callee signature/implementation
also vetoes when its supplied first argument may be an intrinsic receiver,
including erased signatures. No target is classified by a `prototype` text search.

Known other string keys are unrelated to the engine's dense literal iteration:
the literal owns indices and length, and iterator results own value/done.
An exact unwritten local class prototype or a separate literal allocation
(including immutable local aliases) is disjoint and does not veto. Missing,
imported, parameter-rooted, reassigned or cyclic identities never discharge a
relevant mutation. This conservative project-wide veto is also global across
analyzed dependency sources; this amendment does not change source selection.

An otherwise supported tuple closure under this veto retains SC9012
**uncertifiable**, replacing its SC1001 violation. No substitute implementation
is followed. The companion typed-source array-slot proof and contracted tuple
source binding also refuse: a declared Accessor type cannot restore the
original slot. A direct accessor call retains its existing SC9012 typed-source
uncertifiable finding. The cross-file proof digest binds the veto so a mutation in a
different file invalidates the consuming proof. This remains a visible-veto
premise, not an exhaustive mutation census of unseen runtime code.

Fixture `tuple-iteration-veto` seeds the project assignment from the review.
Its process test runs the positive and negative `cases.json` variants in
isolated projects, since one global positive would veto every control tuple
in a shared project. All variants are tsc-clean against published rc.13.

## Evidence

- Fixture `reactive-ir/tuple-slot-provenance`: four proven `SC1001`s, one new
  `SC9012` (an inline slot), five clean cases, and 15 silent refusals. The pre-existing
  accessor-slot `SC1001`, `t[1]()` `SC9012` and factory `SC4001` are
  unchanged. Fixture `reactive-ir/tuple-slot-context`: the context-tuple
  refusal beside an object control. `tsc --noEmit` is clean for both.
- Unit tests in `accessor_provenance.rs`:
  `returned_tuple_census_requires_every_slot_literal`,
  `simple_array_binding_refuses_defaults_rest_nested_and_mutable` and
  `tuple_slot_is_the_exact_arrow_the_factory_declares`.
- rc.13 corpus sweep (`--browser`, release binary) against ADR 0281's: 562 →
  562 violations and 3,731 → 3,731 uncertifiable, nothing added or removed.
  The corpus's identifier tuple returns are accessor/setter pairs or values,
  not slot closures read in a body. The measured demand is slice 2's.
