---
status: accepted
---

# V1 adds span-exact constant values

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.constantValue`
and optional `EntityFact.constantValue`. The fact is a tagged string or number
and is present only when a bounded evaluator proves the complete expression at
the demanded trivia-normalized start and exact end bytes.

The evaluator folds primitive literals, substitution-free templates,
transparent expression wrappers, unary numeric signs, same-kind binary `+`,
and compiler-resolved immutable declarations (`const`, `readonly`, and enum
members). A depth limit and declaration cycle guard make recursion bounded.
Unsupported syntax, mutable or parameter references, mixed coercion, checker
error/dynamic types, and failed resolution produce absence.

Wire table schema v8 appends entity flag bit 8. The payload is a kind tag plus
either a dictionary string or IEEE-754 bits. Compact demand bit 11 selects the
fact. Retained contributions, demand hashes, and row equality carry the field
so full, delta, and reuse responses are equivalent.

## Compatibility

Producer and Rust client ship together. The schema digest and required Wire
table schema advance together, so stale pairs fail during startup or response
validation. Existing demands and complete-expression selection are unchanged.

## Consequences

Consumers can recover static JSX attribute strings without treating rendered
literal-type text as value evidence. Missing `constantValue` is a fail-closed
"not proven constant" result, including when the field was not demanded.
