---
status: accepted
---

# V1 adds span-exact tuple shapes

> Historical record. Superseded as active protocol guidance by
> [ADR 0018](0018-v1-primitive-domains-and-exact-tuple-lengths.md); Wire table
> schema v11 remains frozen and decodable.

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.tupleShape` and
optional `EntityFact.tupleShape`. The fact carries the fixed slot count, whether
a rest or variadic tail follows, and the callability of the first slot's type,
for the type at the demanded trivia-normalized start and exact end bytes.

It is emitted when that type resolves to a tuple — itself a tuple, or a union
whose every value-carrying constituent is one — and never for the global
`Array`/`ReadonlyArray` types, which carry a number index signature rather than
fixed slots. A union reports the constituents' **meet**: the slots they all have,
a rest tail only if all carry one, callable only if all are, and the largest
argument requirement among them, so what it reports holds whichever constituent
the value turns out to be. A single non-tuple constituent voids the answer, since
then no shape is common to every value. Nullish constituents carry no structure
and are skipped, so an optional tuple still describes the tuple it is when
present; a consumer that also needs presence should read `runtimeValueDomain`.

The meet is a widening of the original rule ("itself a tuple"), not a format
change: the payload and Wire table schema are unchanged, and producer and client
ship in build-id lockstep regardless. The
compiler's own `isTupleType` is the predicate and its `fixedLength` and element
flags are the source of the counts, so a spread-only tuple that TypeScript has
already reduced to an array type is reported as the array it became.

Wire table schema v10 appends entity flag bit 10, carrying one packed word
(`fixedLength << 1 | hasRest`) and one callability tag. Compact demand bit 13
selects the fact. Retained contributions, demand hashes, and row equality carry
the field so full, delta, and reuse responses are equivalent.

## Amendment: `elementZeroMinimumParameters` (Wire table schema v11)

`elementZero` says whether the first slot is callable, which is not enough to
decide whether it can be *invoked*: a function requiring more arguments than a
caller supplies is not assignable to that caller's signature, even though it is
callable. `TupleShape` therefore also carries `elementZeroMinimumParameters`, the
fewest arguments any of the slot's call signatures requires — the minimum across
overloads, matching assignability, since the checker needs only one compatible
signature. Optional and rest parameters lower it; it is zero when the slot is
absent or not callable, so it must be read together with `elementZero`.

Wire table schema v11 extends the tuple payload with one trailing count.
**v10 is retired rather than frozen.** Every prior bump added a flag bit, leaving
older frames unambiguously decodable; this one changed an existing field's
payload, so a v10 row cannot be read without knowing which of the two layouts
produced it. v10 shipped for a single commit, and the handshake's schema digest
and build-id lockstep make a v10 producer unpairable with any current client, so
retiring it is honest where keeping it half-decodable would not be.

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
