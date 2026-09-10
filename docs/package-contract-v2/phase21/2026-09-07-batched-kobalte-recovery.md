# Bounded large-case recovery: Kobalte scoped result

`@kobalte/core@0.13.13|solid1|only` moves from refused to partial. Its accepted
set changes from empty to 503 exact artifact cases across 441 entrypoints.
The independent proposal census contains 577 candidates: 503 publish and 74
retain explicit native proof refusals. The root remains unproved. Wildcard
exports still cannot establish completeness under the existing metric.

The [exact before/after evidence](2026-09-07-batched-kobalte-scoped-measurement.json)
enumerates every accepted entrypoint and runtime/declaration artifact selection,
resolution branches, importer coordinates, published pointer/catalog digests,
receipt digests and payloads, and all 74 refusals. Inventories follow only the
report row's published case-set pointer and its named catalogs. Ordinary
consumer verification authenticated receipts and selected exact cases.
All 503 selections have nonempty export claims. Runtime artifact extensions
are 62 `.js`, 62 `.jsx`, 296 `.tsx` and 83 `.ts`; none is a declaration-only
artifact. Published source-condition entrypoints are kept distinct from their
distribution artifacts and do not establish wildcard completeness.

For unaccepted value-only proposals larger than 32 cases, the orchestrator now
subdivides refused batches within the document limit of 1,024 cases. Successful
private batches supply selection hints. A fresh native transaction verifies
and publishes their combined proposal, retaining complete claims within each
selected case. No trial receipt is reused. Existing publications cannot be
replaced with smaller subsets, and non-proof failures or final conflicts stop
publication. The bound is at most twice the case count in native transactions.
The graph recovery lane retains its separate 32-case bound and mandatory
retained cases. [ADR 0059](../../adr/0059-independent-artifact-case-recovery.md).

The scoped probe exited 0 after 1,213.353 seconds; certification took 1,187.412
seconds within its 1,200-second limit. The full comparison uses a declared
1,800-second per-probe limit to accommodate concurrency. That resource-window
change is separate from proof and coverage-metric semantics.

Focused contract workflow tests passed (82 tests), including a mixed 64-case
selection, a conflicting final union, an all-refused search, and the oversized
1,025-case guard. Final full verification passed with exit 0, TOTAL 133.04s,
and no `FAILED during step` marker in `/private/tmp/batched-cases-final-verify.log`.
Full-corpus preservation is recorded separately when complete.

The root's next blocker is `ColorModeContext`: its recursive-value-shape demand
lacks compiler proof that the export is non-callable and non-constructable.
The published declaration is `solid_js.Context<ColorModeContextType | undefined>`
and the runtime initializes it through `createContext()`. Neither the generic
type's spelling nor that function's name grants a result-shape premise. Other
remaining cases retain their exact demands in the evidence. This result does
not establish the overall coverage ceiling or any new complete row.

Follow-up investigation found that repeated compiler-source acquisitions shared
scratch names and could lose `solid-js` after the first attempt. The installed
declaration answers correctly in both TypeScript 5.9.3 (no diagnostics) and a
temporary Type Facts diagnostic test. [ADR 0061](../../adr/0061-compiler-source-retry-isolation.md)
fixes the directory collision; this completed scoped result remains evidence
for its original implementation, not the final recovery ceiling. The subsequent
`batched-cases-full` run was interrupted with exit 130 after discovering this
defect; it supplies no completed full-corpus counts.
