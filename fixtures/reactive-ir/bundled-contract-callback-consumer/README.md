# Exact first-party bundle refusal for ambient imports

This fixture now pins the Phase 14 artifact-identity boundary. Its `solid-js`
and `@solidjs/web` declarations are ambient stubs; there is no installed
runtime artifact whose manifest, runtime bytes, declarations, closure, and
receipt match a first-party bundle case. Package spelling alone therefore
cannot authorize either bundle.

The nested `doubled()` read in `Untracked` remains a locally proven violation.
The old additional findings on the `flatten` and `applyRef` call sites were
name-only contract effects and intentionally disappear. The compiler-tracked
controls and native `createEffect` control remain clean.

Published RC.3 callback operations are checked separately by the Phase 13
conformance corpus and the deterministic receipt-issued first-party bundle
gate. This fixture proves that those claims do not leak to unrelated bytes.
The declaration signatures remain compatible with the published APIs and the
project is clean under `tsc --noEmit`.
