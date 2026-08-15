# package-contract-export-missing

`SC9001` · **error** · uncertifiable

An imported package ships a reactivity contract, but the contract has no entry for
this export.

## What it does

Flags import bindings whose package has a `solid-reactivity.json` contract that
does not describe the imported export. (Exports of `solid-js` itself are exempt —
the bundled contract is authoritative there.)

## Why is this analysis-limiting?

solid-checker certifies whole programs by following reactive values across calls.
For code inside the project it reads the source; for packages it relies on the
contract's per-export summaries — which reactive values an export reads, which
callbacks it tracks, and whether it returns accessors. An export with no summary is
a hole in that map: anything flowing through it cannot be certified, so every use
site becomes uncertifiable rather than certified or proven wrong.

## Example

```tsx
// solid-reactivity.json for "solid-widgets" describes createWidget but not createGizmo.
import { createGizmo } from "solid-widgets";

const gizmo = createGizmo(count); // Uncertifiable: is `count` read reactively? Returned?
```

## Removed 1.x APIs

Under the Solid 2.0 dialect, when the flagged export of `solid-js` (or
`@solidjs/web`) is a known API that 2.0 removed or renamed — `batch`, `on`,
`onMount`, `onError`, `createResource`, `createComputed`, `createMutable`,
`mergeProps`, `splitProps`, `Suspense`, and the rest of the migration guide's
removal table — the finding's hint points at the 2.0 replacement instead of
asking for a contract entry: there is no export left to describe, and the fix
is migrating the call site.

## How to fix

Add an export summary for the flagged export to the package's
`solid-reactivity.json` — its reactive reads, tracked callbacks, and return kind.
If the export is not reactive at all, an empty summary certifies that explicitly.

If you consume the package but do not maintain it, place a local contract at
`.solid-checker/contracts/<package>/solid-reactivity.json` in your project.

See [package-contracts.md](../package-contracts.md) for the contract format.

## Related

- [package-contract-missing](package-contract-missing.md) — no contract at all
