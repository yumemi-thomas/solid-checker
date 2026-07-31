---
status: accepted
---

# V7 adds an opt-in runtime value domain

## Decision

Lifecycle schema v7 adds `EntityDemand.runtimeValueDomain`. Its entity fact is
a four-boolean `RuntimeValueDomain`: `mayBeCallable`, `mayBeUndefined`,
`mayBeOther`, and `unknown`.

The producer derives the fact only from TypeScript-Go checker types, flags,
constraints, assignability, union constituents, and call signatures. Rendered
type text is not an input. Unknown and recovery types remain explicitly
conservative; `never` is represented as a known empty domain.

Wire table schema v4 adds entity-row flag bit 5. When present, its payload is a
four-bit unsigned integer in the boolean order above. Compact demand bit 9
selects the fact.

## Compatibility

Lifecycle schemas v5 and v6 and their published digests are frozen. They keep
emitting Wire table schema v3 and reject both expanded and compact requests for
the new fact. The Rust session client opts into lifecycle v7 explicitly and
requires Wire table v4 for full, delta, and retained reuse responses. The
transition decoder still understands v3 so frozen fixtures remain verifiable.

## Consequences

Consumers can express policy over runtime value categories without depending
on Solid or another framework in the fact producer. Solid Checker can validate
cleanup returns with `!may_be_other && !unknown`, while other consumers can use
the same structured fact for different accepted domains.
