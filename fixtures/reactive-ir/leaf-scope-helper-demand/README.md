# Local helper resolved-call demand (ZA)

Candidate fixture; not executed. Expectations below concern the leaf-owner
rule, not a full snapshot. The patch only widens demand through local lexical
references; it does not certify a function because it was demanded.

| Case | Role | Expected leaf finding |
| --- | --- | --- |
| LocalHelperForbidden | positive | SC3001 violation: local helper registers cleanup |
| LocalHelperClean | negative | none: two local helper hops reach String.trim |
| SetterCallbackClean | negative | none: demanded Date member inside an updater literal |
| WrappedHelperClean | wrapper | none: transparent satisfies wrapper keeps the local reference |
| ShadowedHelper | shadowing | SC9012 uncertifiable: supplied callback is not the same-named helper |
| OpaqueMember | unresolved | SC9012: a demanded structural run signature has no exact implementation |
| ImportedHelperStillOpen | cross-file boundary | SC9012: imported helper's non-parameter trim remains outside this first slice |

The stub is a byte-for-byte copy of
fixtures/reactive-ir/leaf-scope-host-callbacks/solid-js.d.ts. Its cleanup and
leaf-callback signatures must not be loosened. The adjacent package manifest
only selects Solid 2; it is not an audit of runtime bytes. Real published-type
tsc validation is explicitly deferred under the no-build/no-test instruction.
When promoted, add the normal per-fixture .gitignore exceptions and compare
coverage before creating a snapshot. No snapshot is invented here.
