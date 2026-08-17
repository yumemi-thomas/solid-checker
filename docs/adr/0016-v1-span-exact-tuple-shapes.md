---
status: accepted
---

# V1 adds span-exact tuple shapes

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.tupleShape` and
optional `EntityFact.tupleShape`. The fact carries the fixed slot count, whether
a rest or variadic tail follows, and the callability of the first slot's type,
for the type at the demanded trivia-normalized start and exact end bytes.

It is emitted only when that type is *itself* a tuple: never for a union, whose
constituents may disagree, and never for the global `Array`/`ReadonlyArray`
types, which carry a number index signature rather than fixed slots. The
compiler's own `isTupleType` is the predicate and its `fixedLength` and element
flags are the source of the counts, so a spread-only tuple that TypeScript has
already reduced to an array type is reported as the array it became.

Wire table schema v10 appends entity flag bit 10, carrying one packed word
(`fixedLength << 1 | hasRest`) and one callability tag. Compact demand bit 13
selects the fact. Retained contributions, demand hashes, and row equality carry
the field so full, delta, and reuse responses are equivalent.

## Relationship to `arrayShape`

[ADR 0015](0015-v1-span-exact-array-shapes.md) answers "is this iterable as an
array", collapsing arrays and tuples into one verdict because both of its
consumers wanted the union of them. That collapse is exactly what this fact
undoes, for consumers that must decide whether a value satisfies an interface
with *numbered* members — where an array, having no `0` or `1` property, is not
interchangeable with a two-slot tuple.

The two are independent demands. `arrayShape` stays cheaper and answers for
unions; `tupleShape` answers only for a real tuple but describes its structure.

## Consequences

Contextual typing decides tupleness for an array literal, and consumers depend
on that: a literal written where the expected type has numbered members acquires
fixed slots, while the same literal in an unconstrained position stays a plain
array. A consumer can therefore distinguish "the checker examined this and it is
not a valid pair" from "nothing here constrains it" — which is the difference
between a defect the type system already reports and one only a rule can.

Missing `tupleShape` is a fail-closed "not proven a tuple", including for a
union, for a non-tuple type, when the demanded span is not exactly one
expression, and when the field was not demanded. `fixedLength` counts optional
slots, matching the compiler, so a consumer needing "is there a value at index
n" must consider `hasRest` as well.
