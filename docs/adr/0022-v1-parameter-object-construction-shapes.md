---
status: accepted
---

# V1 adds demand-shaped parameter object construction shapes

## Decision

The sole active lifecycle schema, V1, adds `parameterObjectShape`, a demand
that augments resolved argument mappings with the selected declaration
parameter type's required object properties. Wire table schema v17 carries the
nested shape; V16 remains frozen and rejects its parameter flag. Compact demand
bit 18 selects the work. The handshake protocol stays 2 because no lifecycle
operation changed; the schema digest and build identity move.

Each required property has its exact compiler symbol name and one bounded
construction candidate: `emptyArray`, `emptyObject`, or `unknown`. Optional
properties are absent. The producer uses compiler types and symbols only. It
does not parse `TypeToString`, trust a package or export name, or claim that a
constructed value demonstrates runtime behavior.

`emptyArray` requires every constituent to be the compiler's global array type
or a tuple with no required element. `emptyObject` requires every constituent
to be an object with no required property and no call or construct signature;
a type parameter is examined through its compiler-resolved base constraint;
the candidate becomes an inhabitance proof only after the completed synthetic
call resolves validly with ordinary contextual inference.
Open, error, cyclic, primitive, callable, constructable, and otherwise
unproven shapes remain `unknown`.

## Why

Wide packages often expose factories whose useful runtime objects are
reachable only through a required options object. TanStack Table is the
motivating case: the exact declaration-selected parameter requires `features`,
`data`, and `columns`; the compiler proves `{}`, `[]`, and `[]` inhabit those
properties. Blind primitive probing throws before it creates a Table and can
force thousands of isolated process restarts.

The producer already validates a completed synthetic call through
`resolvedCall`. The missing fact was the finite required-property vocabulary
needed to propose that call without reconstructing TypeScript types in a
consumer. A consumer can select an unambiguous signature with a bottom-typed
placeholder, read the shape, synthesize a candidate, and then require a second
ordinary resolved-call proof for the completed expression.

## Consequences

The shape is reachability input only. Runtime observations remain the source of
behavioral evidence, and a completed synthetic call must still be validated by
TypeScript before package invocation. Recovery, unresolved, and composite
calls expose no resolved parameter and therefore no shape. Unknown witnesses
stay fail-closed or require exact candidate enumeration.

The fact is demand-shaped so ordinary resolved calls pay no property walk or
wire cost. Retained demand identity includes the new bit, parameter caches are
partitioned by it, and type descriptors for the selected parameter and its
properties record the declaration dependencies needed for incremental
invalidation.
