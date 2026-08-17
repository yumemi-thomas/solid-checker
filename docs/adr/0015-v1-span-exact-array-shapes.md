---
status: accepted
---

# V1 adds span-exact array shapes

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.arrayShape` and
optional `EntityFact.arrayShape`. The fact is a closed classification —
`array`, `notArray`, `mixed`, `unknown` — of the type at the demanded
trivia-normalized start and exact end bytes.

The classification comes from the checker's own `isArrayOrTupleType` predicate
applied to the real union constituents: `array` when every constituent is a
reference to the global `Array`/`ReadonlyArray` type or a tuple, `notArray` when
no constituent is, `mixed` when a union genuinely holds both. `unknown` covers
`any`, `unknown`, `never`, error types, and instantiable types, whose inhabitants
the predicate cannot settle; proving the predicate over every substitution of a
constrained type parameter is a separate proof this fact does not attempt.

`array` is deliberately narrower than "array-like". A type merely assignable to
`ReadonlyArray<any>` — an interface extending `Array`, or another purpose-built
wrapper — is `notArray`, because its author chose that type over an array.

Wire table schema v9 appends entity flag bit 9, carrying one tag. Compact demand
bit 12 selects the fact. Retained contributions, demand hashes, and row equality
carry the field so full, delta, and reuse responses are equivalent. The
classification records the type's alias declaration locations as dependencies,
so an edit to a tuple alias in another file re-derives the shape instead of
reusing a stale row.

## Compatibility

Producer and Rust client ship together. The schema digest and required Wire
table schema advance together, so stale pairs fail during startup or response
validation. Existing demands and complete-expression selection are unchanged.

## Consequences

Consumers can settle an array/tuple question without matching rendered type
text. That closes two false negatives text could not reach: an aliased tuple
renders as its alias and fails every prefix test, and an array of functions is
indistinguishable by text from a function returning an array — the trailing
`[]` is identical — which previously required a second `callability` fact to
disambiguate and still could not see through an alias.

Missing `arrayShape` is a fail-closed "not proven either way", including when
the demanded span is not exactly one expression and when the field was not
demanded. `mixed` and `unknown` are proven states that still prove neither side;
only `notArray` licenses a consumer to rely on the negative.
