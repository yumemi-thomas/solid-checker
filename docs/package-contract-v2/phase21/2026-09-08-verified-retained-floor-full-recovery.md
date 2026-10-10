# Full retained corpus after the verified retained floor

`2026-09-08-verified-retained-floor-full.json` finished at 17:36:28 JST on
September 8, with actual exit 0 after 1,377.768 seconds. All 418 probe identities
match the previous retained full report. The run kept artifacts, used generation
concurrency 4, certification concurrency 2, graph concurrency 1 and a 900-second
probe timeout. Source and binaries were held fixed throughout the run.

The [published-catalog measurement](2026-09-08-verified-retained-floor-full-measurement.json)
records 324 complete / 62 partial / 23 refused / 9 not-advanced rows both before
and after. There are no complete-row transitions. Accepted artifact cases
increase from 1,496 to 1,510:

- Solid 1 `solid-js@1.9.14`: 29 → 39 cases, 16 → 20 names. Adds seven `./web`
  conditions and the three raw web JavaScript entrypoints measured earlier.
- Solid 2 `@solidjs/web@2.0.0-rc.3`: four new root cases, under import,
  development+import, browser+import and browser+development+import. Covered
  names increase from 8 to 9 and root coverage becomes true.

Both rows remain partial under the unchanged wildcard-aware metric. The four
Solid 2 cases are a newly measured effect of the accumulated implementation
since the last full report; this comparison does not attribute them exclusively
to ADR 0078. This is new certification, not a denominator correction.

The [all-claim audit](2026-09-08-verified-retained-floor-all-claim-preservation.json)
finds no missing physical entrypoint selection. Exactly one old claim set changes:
`@solidjs/start@2.0.3 ./fns/client:createClientReference` adds a same-stack
parameter-0 return operation. Its prior callable shape and the other export's
claims remain present. The other 1,495 prior exported claim sets are identical.

Strict artifact-case identity preservation is **not** claimed: 66 prior closure
identities change. The [closure comparison](2026-09-08-verified-retained-floor-closure-transitions.json)
verifies identical local closure entries, hazards and package identities, plus
the same dependency specifier/package census. Only accepted dependency artifact
case and contract digests change. Those are real bindings, not ignorable fields.
The final native transactions freshly authenticate their receipts and ordinary
exact case selection; no old receipt is copied into a new context.

Motion Solid 2 floor and head retain `.`, `./m` and `./v2`, with identical
exported claim sets and runtime/declaration bytes. Their closure bindings are
among the rebindings above. Solid 1 Motion likewise retains `.` and `./v1`.
Thus executable coverage and exported claims survive; exact old closure IDs do
not. The linked evidence retains both sets and their receipt-bound identities.

Full `make verify` for this source passed with actual exit 0, TOTAL 76.30 seconds
and no `FAILED during step` marker; all 214 CLI tests passed. This corpus run
changed only retained measurement artifacts. No snapshots, bundled contracts,
commits or pushes changed. Remaining raw Solid tuple-path refusals and the
Corvu compiler-context collision remain open. The coverage ceiling is unproven.
