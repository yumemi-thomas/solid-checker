# Defined-input origin: Corvu recovery

The corrected ADR 0080 implementation certifies `@corvu/popover@0.2.0` in its
retained Solid 1 project. The native transaction finished in 35.053 seconds
with actual exit 0, no cache misses, and ordinary-analysis
`receiptAuthenticated: true` / `exactCaseSelected: true`.

The baseline is the 418-row `2026-09-08-verified-retained-floor-full.json`,
finished at 17:36:28 JST. This scoped run follows the published case-set pointer,
verifies its document and catalog digests, filters exact package/version, and
records both root receipts and their binding envelopes in the
[measurement](2026-09-08-defined-input-corvu-measurement.json).

| Selection | Before | After |
| --- | --- | --- |
| `.` — `/exports/./solid`, `./dist/index.jsx` | refused | certified |
| `.` — `/exports/./default`, `./dist/index.js` | refused | certified |

There were no accepted cases in this row. The accepted name set is now `{.}`,
and the artifact-case set is:

- `artifact-case:c3cf608ee034655eb8b9c4acee90c42cd02a7d51f55a794754706235068578ac`
- `artifact-case:f745ece0751117da3009aa76f25b9d3bcc67c349518c04829c3f8cec7c5955b5`

Both use `./dist/index.d.ts`. The linked JSON includes full runtime,
declaration and closure hashes; the exact importer, resolved import root,
runtime/types resolution traces, exported claims, and receipt payloads.
Each receipt commits to dependency receipt and trust roots, and the catalog's
resolution closure names the immediate dependency artifact cases and accepted
contract digests. The case-set publishes root catalogs; unpublished dependency
catalogs are neither counted nor reconstructed from leftover files.

The unchanged metric sees one declared explicit entrypoint, no wildcard, and
certified root coverage. This is one **refused → complete** row transition,
two new artifact cases and one newly covered entrypoint name. It is new
certification, not a denominator correction. No prior accepted case in this
row could be lost. The completed full comparison below establishes preservation
across the corpus as well.

The positive premise is deliberately limited. After ADR 0079 resolves the
duplicate-installation compiler context, Floating UI's first `list.concat`
read can bind the strict `list === void 0` guard, its sole local-array store,
and the exact original parameter declaration. No earlier reference can retain
the caller's undefined state. Defined arguments retain caller origin; the
fallback branch is local and cannot prove a guaranteed caller read. A
zero-minimum, non-composed, exact-member read consumes that evidence. This
changes no external package trust or compiler diagnostic.

An earlier candidate also certified Corvu, but an overlay counterexample
showed that it admitted a read gated to fallback-only executions. That result
at `/private/tmp/retained-graph-native-Q6jPyF/result.json` is explicitly excluded.
The corrected result is
`/private/tmp/retained-graph-native-NwsJtf/result.json`. Producer and native
regressions now refuse that counterexample, along with aliases before the
candidate read, loose or mismatched guards, later stores, redeclarations,
whole-root substitution and guaranteed reads.

Validation: two armed native tests pass in 20.92 seconds. Full `make verify`
passes with actual exit 0, TOTAL 166.60 seconds and no `FAILED during step`
marker, including the producer race tests and client envelope regressions.
No existing snapshot changed. Nothing was committed or pushed.

The full 418-row rerun, `2026-09-08-defined-input-full.json`, finished at
19:10:24 JST in 1,430.426 seconds with actual exit 0. It used the fresh verified
native binary, matching producer, the same recovery probe set and graph
concurrency 1. The unchanged coverage metric moves from **324 complete / 62
partial / 23 refused / 9 not advanced** to **326 complete / 63 partial / 20
refused / 9 not advanced**. The runner's terminal headline counts generation
outcomes (349 success, 32 partial-success), not certified complete rows; it
must not replace the receipt-based metric.

Both `@corvu/popover@0.2.0|solid1|only` and
`@corvu-next/popover@0.1.5|solid2|only` move from refused to complete, each
adding `.` in two artifact cases. `corvu@0.7.2|solid1|only` moves from refused
to partial, adding two cases apiece for `./accordion`, `./calendar`, `./dialog`,
`./disclosure`, `./drawer`, `./otp-field`, `./popover`, `./resizable`, and
`./tooltip`. Its wildcard denominator still cannot establish completeness.

The [full measurement](2026-09-08-defined-input-full-measurement.json) records
exact before/after selections, publication pointers, catalog/document/receipt
digests and consumer-bound evidence. The
[all-claim audit](2026-09-08-defined-input-all-claim-preservation.json) finds
**1,510 → 1,532 accepted artifact cases**, preserving every old selection and
exported claim without changes. The
[closure audit](2026-09-08-defined-input-closure-transitions.json) finds zero
closure identity transitions and lists all 22 additions. Motion Solid 2 floor
and head retain `.`, `./m`, and `./v2`. These gains are new certification;
there was no coverage-metric correction.

Matching native and producer binaries and the producer stamp are archived at
`rust/target/ecosystem-investigations/2026-09-08-defined-input-binaries/` with
a SHA-256 manifest before any subsequent rebuild.
Other guards, later references, generic value paths, CJS exports and runtime
library policy blockers remain open. The coverage ceiling is unproven.
