# Package integrity: getter

`Conditional.tsx` extends exact getter provenance through conditional values.
Both alternatives must independently close before a Get is a proven read.
`BothKnown` reports SC1001 violation; `UnknownBranch` and `InertBranch` retain
SC9012 uncertifiable obligations; `TrackedConditional` stays clean. Known
alternatives also participate in may-provenance when a sibling is unresolved,
so a conditional cannot silently hide a possible package getter. All existing
provider, mutation, integrity and recursion gates remain in force. The added
source passes strict and loose TypeScript checks against published Solid
2.0.0-rc.13 typings without this fixture's Solid declaration stub.

The same file covers extracting the stable `location` data member with a
single plain object-binding slot, including renaming. `DestructuredObject`
and `RenamedObject` must still report SC1001 violation at the later reactive
`search` read. `MutableObject` stays SC9012 uncertifiable; `TrackedObject`
is clean. This does not certify destructuring reactive values: the existing
`App.tsx` `SearchDestructured` getter read remains a violation. Defaulted,
computed, rest and multi-binding patterns receive no new exact proof, and
project-created receiver escape checks are not waived.

ADR 0300: a selected package's sole provider and stable accessor-reading
getter prove four SC1001 untracked reads. Tracked JSX, memo, event, inert,
shadowed and project-created getter controls stay clean.

This replaces `dependency-source-context-getter`'s unknown-target wildcard
with two unrelated, exactly rooted literal targets, including a local result
forwarded by `Object.assign`. Both visible package providers supply the getter.
The original fixture retains ADR 0284 decision 8's unknown-target veto. The spread, descriptor,
package-write and visible-provider variants withhold every getter violation.
The unseen-provider variant excludes the unseen provider by the premise; its
execution would violate the stated premise, not establish a runtime defect.

`solid-js.d.ts` is byte-identical to the original fixture. Its claim-bearing
signatures match published Solid 2.0.0-rc.13. Package JavaScript is not
type-checked. TypeScript `--noEmit` is clean.
