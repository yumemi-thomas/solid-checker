# ADR 0145: A returned callable carries its own call claims

- Status: accepted and implemented (2026-09-28); written with the implementation.
  The owner decided on 2026-09-28 that a returned callable's behaviour is
  stated as nested call claims.
- Date: 2026-09-28
- Owners: the semantic model (`ValueShape::DescribedCallable`,
  `DescribedCall`), the wire and schema, the Type Facts producer's return
  facts (`ReturnSite::callable`, `ReturnArm::callable`,
  `ReturnSite::primitive_syntax`; handshake protocol 66), the generator's
  described-callable walk (`returns_walk.rs`, `main.rs`,
  `inferred_contract.rs`), the policy-2 `returns` census, positive fact and
  synthesized veto (`type_facts.rs`, `synthesized_vetoes.rs`), and the
  consumer's `returns` projection (`contracts.rs`)
- Relation: one more exact output beside ADR 0113's `plain` and ADR
  0115/0116/0121's argument containers, decided by the same census pattern.
  ADR 0146 adds the one nested read this ADR leaves empty.

## Context

The certification metric's largest missing claim form after ADRs 0142/0143 is
"`returns` never proposed" (181 exports), and its largest group is an export
that hands back a fresh function or arrow literal (34): `createIdGenerator`,
`chain`, `pipe`, `createMicrotask`, `createMediaQuery`'s server branch. Nothing
could describe what such a value does when the caller later calls it. The
model had `callable`, which says only that the value can be invoked, and the
consumer's `accessor` leaf, which the generator could state from the IR's
local summary but no census decides.

The owner's decision (2026-09-28): the returned callable carries its own call
claims as a nested summary, proven by a census over the returned closure's own
body, projected by the consumer onto the call site of the returned value, and
unknown -- fail closed -- wherever identity or capture is not exact.

## Decision

### The shape

`ValueShape::DescribedCallable(DescribedCall { reads, returns })`, canonical tag
21 (appended), wire `{"kind": "described-callable", "reads": [], "returns":
["plain"]}` with both lists required. Stated as a `return`'s whole output, it
claims, for **one invocation of the returned value**, by whoever holds it:

- it invokes no callable it did not itself define: neither its own arguments
  nor anything the export was handed (`callbacks: []`);
- it creates nothing and registers nothing on an owner (`creates: []`, no owner
  requirement);
- it performs exactly `reads` -- each on the invoking caller's stack, in that
  caller's tracking context (empty in this ADR; ADR 0146 adds one);
- it hands back exactly one of `returns`: `plain`, or nothing (`[]`).

It says nothing about the other domains, and nothing about the export's own:
calling the returned value is a separate invocation. That is the reading the
audited `createMemo` already fixes -- it closes `reads: []` while handing back
an accessor whose invocation reads.

**Exact, not a nested `CallSemantics`.** Every nested domain is stated closed,
so the shape carries no knowledge set and no closure of its own, exactly as
ADR 0115's `argument-array` does. A nested claim the census cannot establish
whole is not stated partially: the `return` is withdrawn by name, and the
export's `returns` opens with it. This is the owner's "unknown where not
exact", spelled the way every other exact output is. A nested *partial*
summary would need claim paths, candidates and censuses for nested domains
that nothing in the certifier has, for no consumer that reads them.

Validation admits it only as the whole output of a `return`, sorts both lists,
refuses duplicates, and admits `returns` items from `plain` alone (ADR 0146
adds `read-value`, beside a read).

### The producer states the literal (protocol 66)

`ReturnSite::callable` / `ReturnArm::callable`: the exact location of the
function or arrow expression a returned value is, after identity-preserving
wrappers. Every evaluation hands back a fresh closure of exactly that node's
code, so the node's own transcript -- demanded through the existing
local-declaration demand, unpremised -- is what the returned value runs. An
identifier naming a function states nothing: a binding's value is not the
literal's identity (`throughMutableBinding`).

`ReturnSite::primitive_syntax`: the returned expression is a primitive by its
grammar alone -- a non-object literal, an untagged template, a unary,
arithmetic, bitwise, relational or equality operator, a compound assignment,
and conditionals, `&&`/`||`/`??`, `=` and comma sequences of them. It exists
because a checker type is not that proof in a JavaScript file: `let n = 0;
… n = {}; return n;` is typed `number` (measured with `tsc` 5.9.3, `checkJs`
off: `export function straight(): number;`), and a closure's read of a captured
binding is typed by the declaration whatever was written to it.

### The census

`census_returns_domain`'s new arm, for a claim whose every item is a `return`
of a described callable, no two alike (`described_callable_return_sites`,
shared with each operation's positive fact):

1. a call, a plain completion form, and a classified control-flow census;
2. every value-carrying return the producer did not prove unreachable is a
   literal by `callable` -- the site's own, or every arm's;
3. each literal's own transcript is acquired and walked by the shared call
   walk, at depth 0, following nothing, and every site it dispositions is
   unreachable, a reviewed standard-library member (whose callable arguments,
   if any, are literals inside the same body), a coercion of primitives, a
   data property of a literal the program built, or `instanceof` against a
   default-library constructor. A parameter-rooted site (the literal's own
   argument), a call of a captured parameter, a dialect primitive, a
   dependency's export and a local helper all refuse;
4. each of its live value-carrying completions is a primitive by type (ADR
   0113's value fact) **and** by grammar (`primitive_syntax`), `plain`; one
   with none states `returns: []`;
5. what the literal shows is a claimed described callable, and every claimed
   one is shown by some literal.

The positive fact reads the same evidence over the whole claim, and refuses a
declared result the producer states non-callable.

### The veto

`Observation::DescribedCallable { nested }`: every sample the declared
signatures admit calls the export; a completion that is not a function
contradicts. Each result is called with four nested samples -- nothing, a
recording callable, `{}`, `1` -- and a nested completion outside the claimed
returns (not `undefined` when every claim is valueless; an object or function
when every claimed return is `plain`; unchecked beside ADR 0146's
`read-value`), or the recording callable running at any time up to the end of the
drain, contradicts. A run in which no nested call completes normally is
incomplete. Callables handed to the export itself are not observed.

### The generator

`returns_walk::described_callable_returns`: a non-`async`, non-generator
function whose every value-carrying completion is a function literal the facts
list, or a conditional of them (the producer's bounds), each itself neither
`async` nor a generator, whose completions ADR 0035's walk clears (`[]`) or
ADR 0113's walk proposes (`[plain]`). One `return` per distinct shape, reads
empty, proposed wherever the reactive analysis left the return undescribed.

### The consumer

A described callable that reads nothing names no leaf: `Known(None)`, alone or
in a union -- calling it observes nothing reactive, invokes nothing the caller
handed over, and creates nothing. One that reads (ADR 0146) is an `accessor`.

## Consequences

- `fixtures/package-contracts/implementation-census-described-callables` is the
  tracer (corpus, and end to end in
  `the_described_callable_census_certifies_exactly_the_literals_own_claims`):
  four exports certify, six are withdrawn by name. Twelve exports in eight
  other corpus fixtures now propose a described callable where `returns` was
  open, and nothing else moves (110 fixtures; possible operations 647 → 671,
  proof candidates 1,829 → 1,885).
- `fixtures/reactive-ir/package-described-callable-consumer` is the consumer's
  half, with ADR 0146's: an inert described callable leaves the import clean
  and its call silent, and the open control reports `SC9005`.
- Measured with `make certification-metric` at this ADR alone, against the
  ADR 0142/0143 run before it (the base also carries ADR 0140): the one clean
  addition is `@solid-primitives/utils`' `createIdGenerator`, the first
  callable certified clean whose return is a function; 27 exports leave
  "`returns` never proposed" (181 → 154), nearly all for a named withdrawal --
  `callable-path` where a literal invokes a captured callback (`chain`, `pipe`,
  `safe`, `reverseChain`, `createMicrotask`, `preventDefault` and its two
  siblings, `toEffect`, `makeTimer`), a local helper (`composeEventHandlers`) or
  a non-primitive completion (`keyArray`).
- Handshake protocol 66: a rebuilt `bin/solid-typefacts` and new certification
  pins. No existing document's digest moves.

## What still refuses

- A literal that invokes anything it did not define: its own arguments,
  captured parameters (`pipe`, `chain`, `composeEventHandlers`), a local
  helper, a dependency or dialect primitive. A nested `callbacks` item from a
  captured argument is the next vocabulary, and ADR 0139's `result-access` is
  its outer half: ADR 0152 adds both, for a captured argument the literal calls
  exactly once on every completion.
- A literal that reads anything: a signal it captured is ADR 0146's; a store,
  props or a member of a captured value refuses.
- A return of a binding naming a function, an object or tuple of functions, a
  class instance, and a value built by a call: none is a literal the producer
  names.
- A returned literal whose value is a primitive by type but not by grammar
  (`makeCounter`'s `return count`).

## Found while doing this: ADR 0113 trusts the same type

ADR 0113's `plain` return reads the checker's type of each return expression,
and in a JavaScript file that type is a declaration's, whatever an unchecked
write stored since (`let n = 0; n = {}; return n` is typed `number`). An export
of that shape can be proposed and certified `returns: [plain]` while returning
an object whenever the veto's finite samples miss the write. `primitive_syntax`
is the fact that would close it. Closed on 2026-09-28 by ADR 0113's amendment,
which holds every plain return -- this ADR's nested ones included -- to the same
evidence (`plain_return_evidence`).
