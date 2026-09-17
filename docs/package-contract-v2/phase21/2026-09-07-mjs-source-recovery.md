# ESM source-subject recovery

The baseline is `2026-09-07-exact-subject-full.json`: 322 complete, 52 partial,
24 refused, 20 not advanced; 768 accepted artifact selections. The goal remains
new executable certifications, not changes to the denominator.

## Implemented selection rule

[ADR 0058](../../adr/0058-authenticated-mjs-source-subject.md) allows an exact
published `.mjs` file to be its own source subject when declaration resolution
fails. Both the CLI resolver and native archive replay first exhaust the
existing declaration branches, then permit the source fallback. Matching
`.d.mts` and late `types` branches retain precedence. Cross-format declaration
borrowing and null/invalid target bypasses remain forbidden.

This uses the existing source-subject proof lane. It supplies no synthetic type
facts and clears no semantic obligation by itself. Exact source hashes,
export/resolution identities, importer and dependency receipts still have to
pass their existing consumer checks. Protocol 39 and receipt formats are unchanged.

## Scoped measurement and shortfall

The fresh pinned debug build ran the `@solidjs/testing-library@0.8.10` probe.
`2026-09-07-mjs-source-testing.json` records the result. Before, generation
stopped because `dom-accessibility-api@0.5.16/dist/index.mjs` had no declaration
target. After, graph preparation proceeds to `aria-query@5.3.0`, whose
`lib/index.js` has no runtime ESM exports accepted by the current model.

Testing Library remains refused. Its before and after published accepted
entrypoint and artifact-case sets are both empty. No new ordinary-consumer
acceptance exists for this row. Implementing a CommonJS export model would be
a separate semantic change; do not declare this package recovered.

## Focused validation

- CLI artifact resolver: 75 tests pass, including matching-sibling priority,
  source fallback and cross-format refusal.
- Native declaration candidate tests: five pass.
- Native condition ordering tests: two pass, including late declarations,
  source fallback after exhaustion, missing source and null-target refusal.
- `git diff --check` passes.

The first CLI run caught premature fallback before a late `types` branch. The
two-pass selection fixes that regression; the final tests preserve declaration
precedence. No unrelated fixture snapshots changed. No commits or pushes.

## Full validation and measurement

Full `make verify` exited 0 with `TOTAL 115.84s` and no `FAILED during step`
marker (`/private/tmp/mjs-source-verify.log`).

The full corpus, `2026-09-07-mjs-source-full.json`, exited 0 in 647.689 seconds,
finishing at 2026-09-07 20:15:41 JST. Catalog-bound counts remain
**322 complete / 52 partial / 24 refused / 20 not advanced**. There are no row
transitions and no newly certified artifact cases.

The [exact comparison](2026-09-07-mjs-source-measurement.json) follows each
published case-set pointer and verifies catalog, main and receipt file digests.
It checks package/version, runtime and declaration paths/hashes, resolution
branches and selection multiplicity. All **768 prior selections across 374
certified rows** are preserved; ordinary consumers authenticate receipts and
select exact cases. No coverage-metric correction was made. The evidence JSON
is diagnostic, not a replacement for native signature verification.

Measured shortfall: zero new certifications from the one targeted package.
Testing Library still requires CommonJS export semantics for `aria-query`;
Floating UI separately needs assignment/control-flow evidence for its member
input. Neither is implied by the source fallback implemented here.
