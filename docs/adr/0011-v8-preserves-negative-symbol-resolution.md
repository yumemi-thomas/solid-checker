---
status: accepted
---

# V8 preserves explicit negative symbol resolution

> Historical record. Superseded as active protocol guidance by
> [ADR 0013](0013-v1-call-result-runtime-value-domains.md); the repository now
> ships one lifecycle schema, V1.

## Decision

Lifecycle schema v8 adds `EntityFact.symbolUnresolved`. The producer sets it
only when a demand selected symbol evidence, the demanded source node exists,
and TypeScript-Go's `Checker.GetSymbolAtLocation` explicitly returns no symbol.
A missing source file or source node leaves both `symbol` and
`symbolUnresolved` empty, preserving unavailable evidence as a separate state.

Wire table schema v5 adds entity-row flag bit 6. The bit has no payload:
`symbolUnresolved` is true when it is present. An entity row containing both a
symbol and the unresolved bit is invalid and the Rust decoder rejects it.

JSX member-name normalization now applies only when a demand spans the complete
tag name. A narrower root demand for `Runtime` inside `Runtime.Component`
therefore resolves the namespace root rather than being widened to the selected
member.

## Compatibility

Lifecycle schemas v5-v7 and their published digests remain frozen. V5 and v6
emit Wire table schema v3; v7 emits Wire table schema v4. Only lifecycle v8
emits Wire table schema v5 and preserves explicit negative symbol evidence.

## Consequences

Consumers can distinguish a compiler-proven missing binding from an omitted
demand, a stale source, or another unavailable semantic fact. This permits
undefined-name diagnostics without turning producer gaps into false positives.
