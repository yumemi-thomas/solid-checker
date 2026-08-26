---
status: accepted
---

# V1 adds span-exact library type sets

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.libraryTypes` and
optional `EntityFact.libraryTypes`: the sorted, deduplicated set of
standard-library type names the type at the demanded trivia-normalized start and
exact end bytes is built from **at its top level**.

Top level means the type itself, its union and intersection constituents, and one
array-element unwrap. It does not mean an object type's properties, a function's
return type, or a generic's other type arguments. A name is recorded only when
the resolved symbol has a declaration in a file the compiler reports as a default
library, so a user-declared `Map` is not the global `Map`.

Wire table schema v12 appends entity flag bit 11, carrying a count followed by
that many dictionary string indices. Compact demand bit 14 selects the fact.
Retained contributions, demand hashes, and row equality carry the field so full,
delta, and reuse responses are equivalent.

## Consequences

The question this answers — "is this value one of these well-known runtime
types" — was previously answered by splitting `TypeDescriptor.text` on top-level
`|` and `&`, stripping a `[]` suffix, and matching the head against a name list.
Text cannot answer it. `type Stamps = Date[]` renders as `Stamps` and matched
nothing, whether declared locally or imported; `Array<Date>` and `Date[]` are the
same runtime value but only the second matched; and a user-defined type could
match a global's name.

Absence means nothing at the top level came from the standard library. It never
means the type was unresolved — a consumer that must distinguish those should
read the type descriptor or symbol alongside it.

The names are the compiler's own symbol names, so a consumer holds the list of
names it cares about. That keeps the vocabulary — which types are interesting,
and why — where it belongs, and keeps this fact free of any consumer's policy.

`EntityFact` grew 136 → 144 bytes. The set is held behind a thin `Arc`, as
`resolvedCall` and `typeDescriptor` already are: as a slice it would have cost
every retained row 16 bytes to carry an absence, because a slice pointer is fat.
