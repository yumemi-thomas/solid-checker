# Geometry premise diagnostic, 2026-09-07

Investigation at HEAD `6b687543`, Type Facts protocol 34. No production patch
was retained. The existing dirty worktree was preserved.

The exact chain already in `declared_signature_premise_test.go` was demanded
one hop further than `TestAnOmittedSlotSurvivesTheHelperPremiseChain`:

| Transcript | Stated parameter types | Remaining coercion forms |
| --- | --- | --- |
| `applyBoxDelta` | `Box`, `BoxDelta` | 0 |
| `applyAxisDelta` | `Axis`, `number`, `number`, `number`, `undefined` | 0 |
| `applyPointDelta` | `number`, `number`, `number`, `number`, `undefined` | 1, with a callee-completion premise |
| `scalePoint`, first call | `number`, `never`, `number` | 1: `scale * distanceFromOrigin` |
| `scalePoint`, second call | `number`, `number`, `number` | 0 |

Every demanded helper echoed its parameter premises without a premise refusal.
Both `scalePoint` transcripts stated `primitiveCompletion: true`. The first
call's second slot stated `type: never`, `identity: flags:262144`. The second
call's first slot stated `type: number`, `identity: flags:64`.

Thus the ADR 0049 reconstruction that assignment from the first call poisons
`point` at the second call is **refuted for this fixture**. The conditional
`boxScale !== undefined`, under an incoming `undefined` premise, narrows
`boxScale` to `never`. The form census still records the call as reachable;
this experiment did not change reachability.

The cause is in `mayBeObjectTypedLocked`, in
`apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms.go`.
`provablyNonObjectFlags` already includes `TypeFlagsNever`, but the function
first calls `Distributed()` and returns `true` for an empty result. The pinned
TypeScript-Go implementation in `internal/checker/types.go:726` explicitly
returns `nil` for `TypeFlagsNever`. Therefore the later flag check cannot
handle this type.

The temporary prototype inserted the following immediately after the nil-type
guard, before `Distributed()`:

```go
if value.Flags()&checker.TypeFlagsNever != 0 {
    return false
}
```

This tests an **explicit type flag**. It does not admit empty constituent
lists, missing types, `any`, or `unknown`.

Observed sequence:

| Source state | `scalePoint` coercions across both calls | Test result |
| --- | --- | --- |
| Original | 1 | diagnostic passed; output inspected |
| Explicit `never` prototype | 0 | 12 selected tests passed |
| Original restored | 1 | diagnostic passed; output inspected |

The 12-test selection included the declared-signature premise tests, the
existing unknown/helper/protocol controls, the object/written/library
completion controls, omitted-slot tests, and the diagnostic. This was a
producer mechanism experiment, **not a certification run or measured corpus
gain**. The parent coercion remains recorded and must still be dispositioned
by the consumer using the leaf's primitive completion.

Commands (all offline):

```sh
GOPROXY=off GOSUMDB=off go test ./apps/solid-typefacts/internal/typefacts/tsgo \
  -run '^TestOpportunityChainDiagnostic$' -count=1 -v
GOPROXY=off GOSUMDB=off go test ./apps/solid-typefacts/internal/typefacts/tsgo \
  -run 'TestOpportunityChainDiagnostic|TestCoercion|Test.*Primitive|TestDeclaredSignaturePremise|TestOmittedArgument|TestAnOmitted' \
  -count=1 -v
```

The diagnostic below was temporarily placed in
`apps/solid-typefacts/internal/typefacts/tsgo/opportunity_diagnostic_test.go`
and removed afterward. It reuses the existing fixture and demand helpers; it
does not add a looser package declaration or invent a transcript.

```go
package tsgo

import (
    "testing"
    "github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestOpportunityChainDiagnostic(t *testing.T) {
    analyzer, dir := premiseProject(t)
    root := premiseTranscript(t, analyzer, dir, "applyBoxDelta")
    axis := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "applyAxisDelta", root.CallArgumentPremises[0].Arguments)
    point := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "applyPointDelta", axis.CallArgumentPremises[0].Arguments)
    dump := func(name string, tr typefacts.ExportImplementationTranscript) {
        t.Logf("%s premises=%+v refusal=%q primitive=%v", name, tr.ParameterPremises, tr.ParameterPremiseRefusal, tr.PrimitiveCompletion)
        for _, f := range tr.UncensusedInvokingForms {
            t.Logf("%s form=%+v", name, f)
        }
        for _, c := range tr.CallArgumentPremises {
            t.Logf("%s call=%+v arguments=%+v", name, c.Call, c.Arguments)
        }
    }
    dump("root", root)
    dump("axis", axis)
    dump("point", point)
    for _, call := range point.CallArgumentPremises {
        leaf := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "scalePoint", call.Arguments)
        dump("scalePoint", leaf)
    }
}
```

Raw local logs: `/private/tmp/creates-chain-baseline.log`,
`/private/tmp/creates-chain-prototype.log`, and
`/private/tmp/creates-chain-restored.log`. They are temporary evidence, not
acceptance receipts. This document preserves the reproduction and observation
if those logs are removed.
