---
status: accepted
---

# V1 adds primitive domains and exact tuple lengths

## Decision

The sole active lifecycle schema, V1, adds demand-shaped
`primitiveValueDomain` facts and an optional `tupleShape.exactLength`. Wire
table schema v13 carries both additions; earlier table schemas remain frozen
and decodable.

`primitiveValueDomain` is the compiler-derived set of possible JavaScript
value categories at one exact expression span: string, number, boolean,
bigint, symbol, null, undefined, and object, plus unknown. Aliases are transparent,
unions combine their categories, and constrained type parameters follow the
compiler's resolved constraint. `any`, `unknown`, recovery, and cycles include
the unknown bit. The fact deliberately does not encode a consumer policy such
as “JSON safe”.

`tupleShape.exactLength` is present only when every tuple constituent has the
same required-only length. Optional, rest, variadic, and unequal-length union
tuples leave it absent. This is separate from `fixedLength`, which counts
optional slots and participates in the tuple-structure meet; using that value
as runtime spread arity would be unsound.

Compact demand bit 15 selects primitive domains. Entity flag bit 12 carries
their nine-bit payload. A v13 tuple payload appends `exactLength + 1`, with
zero reserved for absence. Full, delta, retained-reuse, equality, and schema
hash paths all carry the new values.

## Performance and representation

Runtime and primitive value domains use compact integer bitsets in the Rust
client. The primitive domain is also a compact bitset in the Go producer and is
stored inline rather than allocated per entity. The Rust retained `EntityFact`
row remains guarded by its existing 144-byte size test. Semantic classification
runs only when demanded; no project-wide type walk was added.

The implementation must be accepted only with the scale benchmark and memory
gate passing. Benchmark comparisons use the same command, corpus, warm edit,
and repeated median before and after the change.

On the Apple M4 Pro development host, an immediate five-run baseline/current
comparison measured warm leaf-edit latency at 609,099/603,222 ns (-0.96%), the
analyze portion at 242,727/241,915 ns (-0.33%), and cold full-table analysis at
3,319,568/3,313,941 ns (-0.17%). Response sizes were unchanged and median
allocation counts did not increase. Cold allocated bytes rose from about 1.855
MB to 1.879 MB (+1.31%), attributable to the additional demand/fact vocabulary;
the 144-byte Rust retained-row gate remained unchanged.

## Consequences

Consumers can certify primitive-only policies without parsing rendered type
text and can determine spread-call arity for exact tuples. They must remain
fail-closed when a domain has the unknown bit, when the fact is absent, or when
`exactLength` is absent. Runtime serialization behavior, compiler lowering,
and bundler entry selection remain outside these structural facts.
