# ADR 0095: A class is a callee

- Status: accepted and implemented (2026-09-12); written with the
  implementation
- Date: 2026-09-12
- Owners: Type Facts producer (`class_constructions.go`,
  `export_value_transcripts.go`) and the `creates` implementation census
  (`contract_certification/type_facts.rs`)
- Relation: ADR 0047's `own-class` line, followed rather than redrawn.

## Context

§ 79 ranked the frontier left after ADR 0094 by withheld closure entries and
found that the two largest families were **seven premises and four premises**
rather than one each. The only coherent gap in them spanned both, under two
different sentences:

```js
export class TriggerCache { … }              // "finds no function-like declaration node"
var LiteQueuer = class { … };                // "not a function or arrow literal"
```

`new X(…)` where `X` is a class this artifact declares refused in either
spelling. `census_callee_declaration_node` matches a resolved declaration
against `source.function_nodes()`, which is `facts.functions` — and a class is
not a function there. The two sentences are one absence seen from two
directions.

Measured by reading every site: **21 exports, 115 withheld closure entries**,
with nothing partially blocked by it.

## Decision

A class is a callee. `new C(…)` resolves to the constructor whose body it runs,
in both spellings, and the census walks that body exactly as it walks a
function's.

The resolution is split across the seam the way the seam is meant to be used.
The **consumer** finds the class from its own Oxc facts — by the class node's
span, by its name, or through the declarator that binds an anonymous class
expression — and demands a transcript at it. The **producer** decides what the
construction runs, because that is a judgement about code rather than about
spans.

`class C {…}` and `const C = class {…}` are the same construction; the second is
what every bundler emits for the first, and answering for one and not the other
would make certification depend on which bundler produced the artifact.

The transcript's **declaration is the class** while the body it censuses is the
constructor. A consumer demanded `C` and binds the answer to `C`; reporting the
constructor as the declaration answers `"constructor"` to a query for `"C"`,
which the session refuses outright — correctly, since that check is what stops a
producer describing a different node than the one demanded.

The class binding is then held to the same two questions
`census_local_binding_is_stable` asks of a function binding — is it written
anywhere in the file, is it declared again — because a class binding can be
reassigned exactly as a function binding can, and the census would then be
reading a body that is not what runs.

## What refuses, and why each is a refusal rather than a skip

A construction evaluates, in order, the heritage clause's constructor, every
field initializer, then the constructor body. A census of the third that ignored
the first two would be silent about code that runs, which is the failure this
census exists to prevent. So:

- **A heritage clause.** `extends WeakMap` runs the engine's constructor,
  `extends WithPromise` runs this artifact's, `extends someExpression()` runs
  whatever that returned — three different claims, none made here. ADR 0047 drew
  its `own-class` line at exactly this clause for the neighbouring reason (what
  the prototype chain can carry), and this follows that line rather than
  inventing a second one.
- **A field initializer**, static or instance. `#keyTriggers = new
  TriggerCache(WeakMap)` runs at construction and is a *different node* than the
  one demanded. A bare `#timeoutId;` initializes to `undefined` and runs
  nothing, so it is admitted.
- **A static block**: it runs at class-definition time, which is module
  evaluation — a different question.
- **A computed member name**: the key expression runs when the class is defined.
  ADR 0047 refuses it already.
- **A decorator**, on the class or any member: a call of user code at definition
  time.
- **A parameter property** (`constructor(readonly value)`): it assigns a field
  with no node of its own in the body, so a body census would not see the
  assignment.
- **An implicit constructor**, and this one is a deliberate over-refusal. With
  no heritage clause an implicit constructor runs *nothing at all*, which would
  be the strongest possible answer — but "nothing runs" is a positive claim and
  there is no node here to census into a transcript that states it. Stating it
  is a separate decision; refusing is the direction that cannot be wrong.

Every one of these is an `openReasons` entry, and a consumer refuses on any
nonempty list whether or not it has seen the word — so the vocabulary can grow
without the consumer learning it, and an unknown reason fails closed.

## Consequences

Handshake protocol 54 → 55. The refusals need no number — silence on an unknown
reason already fails closed — but the *admitting* side does: a consumer that
reviewed only protocol 54 would receive a constructor census under a demand it
believed could only answer for a function.

`constructOwnClass` and `constructCompiledClass` certify in
`fixtures/package-contracts/implementation-census-creates`, with
`constructDerivedClass` and `constructInitializedClass` refusing beside them,
and `constructImplicitClass` — a class whose explicit constructor has an empty
body — certifying as the control that proves the other two were *reached and
censused* rather than never looked at.

What this does not reach: every refusal above, and in particular the two shapes
the corpus actually contains beside the admitted ones —
`class ReactiveWeakMap extends WeakMap` and `class JSAnimation extends
WithPromise`. A heritage premise is its own decision, and the engine-owned base
is the easier half of it.

The corpus figure is in the commit message. § 79.2 priced it at 21 exports and
115 withheld closure entries **as an upper bound**, and §§ 76 and 78 are why
that word is there: the census refuses a domain on the *first* premise it cannot
establish, so clearing one blocker routinely reveals the next.
