# Package integrity: getter

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
