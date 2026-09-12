# ADR 0092: A coercion whose every operand is the caller's

- Status: accepted and implemented (2026-09-12); written with the
  implementation
- Date: 2026-09-12
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the `creates`
  implementation census (`contract_certification/type_facts.rs`)
- Relation: ADR 0042's argument, applied through a different operator. Protocol
  51's diagnostic becomes a premise; handshake protocol 51 → 52.

## Context

`census_form_shape_reads_the_subject` already admits two shapes on one
argument. ADR 0047's `instanceof` reaches whatever `Symbol.hasInstance` the
right operand carries; ADR 0042's iteration protocol reaches whatever
`Symbol.iterator`, `next` and `return` the iterated value carries. Both are
admitted when the value is the caller's, and both on the same sentence: *the
code that runs is the caller's exactly as a getter's is.*

A coercing operator is the same shape. `+`, `-`, `<`, a template substitution
and a coercing unary all perform ToPrimitive on their operands, which calls
`Symbol.toPrimitive`, `valueOf` or `toString`. That is user code, and whoever
built the object installed it.

With one difference that decides the premise, and it is why this could not be
the accessor rule with another node kind in its list: an accessor has one
receiver, and a coercion has **operands**. One operand this program built is one
object whose `valueOf` this program owns, which makes the form this program's
act however the others rooted.

Protocol 51 stated the resulting question as a diagnostic —
`coercionSubjectRoot`, the derivation every operand agreed on — and measured it
before this ADR was written: **13 claims across 5 packages** root every operand
at `parameter`. Not the 44 the form class suggested. § 68 of
`phase21/2026-09-10-reads-veto-observation-design.md` records the measurement
and recommends writing it while saying 13 in the sentence a plan would quote.

## Decision

A `coercion` form is dispositioned `parameter-rooted-coercion` when

- every operand ToPrimitive reaches is rooted at the same caller-provenance
  derivation — `parameter`, `parameter-default` or `parameter-result`, the
  three ADR 0048 joined for accessors — with a provably primitive operand
  skipped rather than counted, because it has nothing for ToPrimitive to reach;
- the producer states the **slots** those operands rooted at, strictly
  increasing and deduplicated (`coercionSubjectParameters`, protocol 52); and
- the form's node kind is one the producer's own operand walk enumerates, so
  that "every operand" quantifies over the operands the operator really has; and
- the form sits in the **censused export's own declaration**, not in a
  local-recursion frame.

The slots are the companion fact ADR 0047 requires of every caller-provenance
derivation: a claim that the value is the caller's must say *which* slot,
because that is what the receipt asserts. A coercion names every slot its
operands rooted at rather than one receiver, and the receipt spells it
`parameter/0,2`. One slot coerced against itself is one claim about one
parameter, so `a + a` names slot 0 once.

## What refuses, and why each is a refusal rather than a skip

- **Operands that root at different derivations.** The producer answers
  `mixed-operand-roots` and states no slots. This is stricter than the argument
  strictly requires — an operand rooted at `parameter` beside one rooted at
  `parameter-default` is caller-supplied on every branch — and it is kept
  because the producer states one agreed derivation, not a set. Widening it is
  a separate question with its own measurement.
- **An agreement that is not the caller's.** `own-literal`, `own-class` and
  `default-library` can each be what every operand agreed on, and each refuses.
  An agreement is not a grant: a value this program built is the case the
  premise is *about*.
- **A caller-rooted derivation with no slots, or slots that are not strictly
  increasing.** Both are the producer's two walks disagreeing, and the consumer
  refuses rather than picking a reading.
- **A form that also carries an accessor subject.** `subjectRoot`,
  `subjectParameter` and `subjectDeclaration` describe a receiver; a coercion
  carrying one alongside its own fields is the same disagreement.
- **A node kind outside the operand walk.** The quantifier would range over an
  empty set, and "every operand rooted" would be vacuously true — the repository's
  standing trap in its purest form.
- **A local-recursion frame's parameters**, and this is the one restriction
  that is not obvious from ADR 0034. That ADR admits "a parameter binding of the
  censused declaration — the export itself, **or a `local-recursion` target
  frame**". This ADR does not follow it into the frame, because the difference
  between the two operators bites exactly there: at depth 0 a parameter holds a
  value the external caller passed, by construction, while inside a frame the
  value at that slot was supplied by a call site *in this very artifact*, which
  may have handed it an object this program built — whose `valueOf` is then this
  program's code. Whether ADR 0034's frame case survives that argument is its
  own question and is measured nowhere; refusing it here is the direction that
  cannot be wrong.

  It was the fixture that found this. Writing the premise without a depth gate
  flipped `helperSpreadCoercion` and `helperUntypedArgument` — two cases written
  to pin ADR 0038's boundary, both of which refuse *at the helper* — and closing
  them on provenance would have retired that boundary silently while proving
  something this ADR has not argued.

## Consequences

`untypedCoercion` in `fixtures/package-contracts/implementation-census-creates`
flips from refusing to certifying, which is the premise working on the case the
fixture was written to hold open: `value + 1` where `value` is declared
`unknown` records a form precisely because `unknown` may be an object, and that
object is the caller's. Four cases were added beside it — two slots, one slot
twice, a mixed pair, and two operands that agree on `own-literal` — and the last
two are the vacuity guard: a literal operand would record no form at all and
certify while asserting nothing.

§ 68.2 measured 13 claims across 5 packages whose every coerced operand roots
at `parameter`. That measurement did not separate a form in an export's own
declaration from one in a local frame, so the depth gate above puts the yield at
13 or below; the corpus figure in the commit is the measured one.

What this does **not** reach: a local-recursion frame (above), the
`nested-parameter` (8) and `written-parameter` (7) legs beside it — the same two
legs that top the accessor class — and a coercion whose operands are
caller-supplied under *different* derivations.
