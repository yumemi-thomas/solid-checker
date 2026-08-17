---
status: accepted
---

# V1 adds exact-span call-result runtime value domains

## Decision

The sole active lifecycle schema, V1, includes `EntityDemand.callResultDomain`
and the matching `EntityFact.callResultDomain`. The producer selects a
call-like expression by the demand's trivia-normalized start and exact end
bytes, then classifies the checker type at that call expression with the
existing runtime value-domain classifier. A missing exact call-like node
produces no field. Checker error and recovery types remain present but unknown.

Wire table schema v7 appends entity flag bit 7 and a four-bit domain payload.
Compact demand bit 10 selects the fact. Retained contributions and row
equality carry the field so full, delta, and reuse responses remain equivalent.

V1 also makes the Rust-owned sparse transition path the only producer
materialization path. Go transfers per-file entity runs and compact retention
evidence; it does not build a duplicate symbol closure. Symbol/reference
evidence is resolved before `ReleaseAnalysisState`, so retained analysis cannot
prune compiler state before evidence extraction.

## Compatibility

There is no production consumer of the lifecycle protocol yet, so the latest
protocol vocabulary is collapsed into V1 rather than adding another lifecycle
version. V1 emits Wire table schema v7. Existing `runtimeValueDomain`,
`callability`, and `resolvedCall` semantics are unchanged.

## Consequences

Consumers can classify returned calls without confusing a call's result with
the callable callee. Absence and unknown remain fail-closed outcomes.
