# Package integrity: descriptor

ADR 0300: a selected package's sole provider and stable accessor-reading
getter prove four SC1001 untracked reads. Tracked JSX, memo, event, inert,
shadowed and project-created getter controls stay clean.

This is `dependency-source-context-getter` without its wildcard write. That
fixture retains ADR 0284 decision 8's wildcard veto. The spread, descriptor,
package-write and visible-provider variants withhold every getter violation.
The unseen-provider variant excludes the unseen provider by the premise; its
execution would violate the stated premise, not establish a runtime defect.

`solid-js.d.ts` is byte-identical to the original fixture. Its claim-bearing
signatures match published Solid 2.0.0-rc.13. Package JavaScript is not
type-checked. TypeScript `--noEmit` is clean.
