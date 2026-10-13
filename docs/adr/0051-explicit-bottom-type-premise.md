# ADR 0051: An explicit bottom type is a non-object fact

- Status: implemented (2026-09-07)
- Owners: Type Facts producer and creates census consumer
- Relation: ADRs 0045 and 0049; handshake protocol 34 → 35

## Evidence

The geometry-chain diagnosis recorded in
[the investigation](../package-contract-v2/phase21/2026-09-07-creates-chain-diagnostic.md)
refutes the proposed loss of `point: number`. `applyPointDelta`'s first
`scalePoint` call carries `[number, never, number]`; its second carries
`[number, number, number]`. The omitted fifth argument is explicitly
`undefined`, and the first call is inside `boxScale !== undefined`.

The checker reports `TypeFlagsNever`, but `Distributed()` deliberately returns
no constituents for this type. `mayBeObjectTypedLocked` returned its conservative
answer for an empty constituent list before examining the already-listed
`Never` flag. The first leaf therefore retained one coercion; the second none.
The focused regression fails on that original implementation and passes with
the explicit flag check.

## Decision and consumer binding

After rejecting a missing type, inspect its explicit `TypeFlagsNever` flag
before distributing constituents. That affirmative type fact cannot denote an
object. An empty constituent list without that flag still refuses. This is not
a new inference that a branch is unreachable, nor permission to skip its calls.

The existing caller transcript records the argument's printed `never` type and
identity. The verifier demands the helper under exactly that slot premise;
the helper echoes it byte for byte. The receipt names the helper's
`census-premise:` site, including the slot and printed `never` type; identity
is bound by the verifier's byte-for-byte premise/echo comparison. The parent's
coercion still requires the callee's own stated primitive completion and names
its `primitive-coercion` disposition. Other calls and forms remain censused.

The shared predicate also classifies primitive completion. An explicit `never`
return means no normal completion can hand the caller an object. It does not
prove that evaluating the implementation creates nothing. This meaning change
is versioned with handshake 35 and the schema digest, and the consumer's census
protocol floor is raised accordingly. No wire field is added.

## Boundaries and validation

Producer tests cover both geometry calls, removal of the `never` slot (which
records a real coercion), a forged identity (which refuses the premise), and
explicit `unknown`, `any`, generic, object and unavailable types. They also
separate explicit bottom completion from unknown and object completion.
These tests exercise compiler facts; they introduce no checker diagnostic for
a claim TypeScript already reports.

The complete producer test packages pass with networking disabled. The
implementation fixture and consumer tests exercise receipt binding and the
unknown/any fifth-argument controls. Full `make verify` passed in 193.09
seconds: exit 0, `TOTAL` present, no failure marker. The precision backlog
records the verification scope and the subsequent full-corpus measurement.

## Scope

The baseline contains six `applyBoxDelta` candidates whose first refusal is the
leaf's `scale * distanceFromOrigin`. A subsequent full 418-probe run measured
**506 → 500** withheld candidates and **449 → 443** creates census refusals;
all six accepted `motion-dom` mains explicitly close `applyBoxDelta`'s creates
domain, with digest-bound receipts. Row statuses stay 368 / 30. See the
[complete measurement](../package-contract-v2/phase21/2026-09-07-protocol35-census-measurement.md).
Remaining arithmetic, local result
shapes, construction, returned callables, and external contracts are separate
premises. This decision does not authorize the dependency-terminator change.
