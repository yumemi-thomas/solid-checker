# ADR 0021: V1 carries bounded primitive literal candidates

## Status

Accepted.

## Context

`PrimitiveValueDomain` proves JavaScript value categories but cannot provide a
sound concrete inhabitant for a broad category. In particular, the empty string
does not inhabit a parameter declared as `"open" | "closed"`. Consumers that
need type-correct runtime construction must not parse rendered type text or
guess a representative.

## Decision

Wire table schema v16 adds `primitiveLiteralCandidates` as an explicit entity
demand and response. The producer reads exact compiler
literal types and returns at most 32 deterministic, deduplicated string, finite
number, and boolean values.

The list is a set of proven inhabitants, not an exhaustive domain. Broad
primitive types, branded intersections, enum literals, recovery types, and
unconstrained generics contribute nothing. A constrained generic may contribute
literal members from its compiler-resolved base constraint. Consumers may use a
candidate to construct a type-correct call, but the candidate is not evidence
about what that call will do.

V15 remains frozen and rejects the new row flag. The Rust row stores the list
behind one thin `Arc`; the retained-row ceiling rises from 144 to 152 bytes, the
minimum inline cost of the optional fact.

## Consequences

Consumers can explore literal-directed runtime branches without a textual type
parser. Candidate enumeration is bounded in both the producer and decoder, and
absence continues to fail closed. BigInt and unique-symbol construction remain
outside this fact.
