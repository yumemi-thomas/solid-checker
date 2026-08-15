---
status: accepted
---

# V9 carries exhaustive resolved-call target candidate sets

## Decision

Lifecycle schema v9 adds `Call.Targets`, a `CallTargetSet` holding an explicit
`Exhaustive` proof bit and deterministically ordered, deduplicated
`ResolvedDeclaration` candidates. The tsgo adapter emits the set only for a
valid call whose callee type is a union in which every constituent is one
closed concrete callable and every one of its call (or construct) signatures
names one exact implementation declaration — `FunctionDeclaration`,
`MethodDeclaration`, `ArrowFunction`, `FunctionExpression`, or `Constructor` —
with a canonical symbol. Any `any`, `unknown`, `never`, nullable, generic,
intersection, error, type-level (interface call signature, function type,
method signature, index signature), recovery, or declaration-less constituent
voids the whole set; an incomplete candidate set is never emitted, because it
would read as the complete dispatch surface of the callee type.

The proof is type-level: candidates cover every call signature of the callee's
*apparent type*, the same evidence the existing single `Declaration` field
carries for a non-composite callee. The compiler's union subtype reduction
means structurally identical implementations collapse to a single selected
declaration before this fact is derived; the candidate set therefore never
widens the trust model, it only extends the existing selected-declaration
evidence from one signature to a proven finite set. Argument mappings stay
`compositeSignature`-unresolved for composite callees: per-candidate mappings
may differ, and a shared mapping without that proof would be a guess.

Wire table schema v6 appends a presence bit, the exhaustiveness bit, a
candidate count, and packed resolved-declaration rows after the selected
declaration inside each resolved-call row. Candidate symbols participate in
retained symbol closure and evidence dependencies like the selected
declaration's symbol.

## Compatibility

Lifecycle schemas v5-v8 and their published digests remain frozen. V5 and v6
emit Wire table schema v3; v7 emits v4; v8 emits v5. Only lifecycle v9 emits
Wire table schema v6 and carries target candidate sets.

## Consequences

A consumer can resolve a composite or union-typed dispatch to a finite set of
exact implementations and certify the call only when every candidate's
analyzed behavior is equivalent, instead of failing closed on every composite
call. Consumers must check the exhaustiveness bit and must keep candidate sets
with unresolved or divergent behavior fail-closed.
