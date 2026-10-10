# ADR 0205: Coverage gaps group by family and subject

- Status: accepted and implemented (2026-10-06). Follows ADR 0202 (feedback
  reports what the user can act on).
- Owners:
  - `coverage_group` in `solid-reactive-ir/src/projection.rs`, which sets the
    fields;
  - `Finding::coverage_family`/`coverage_subject` and their
    `SnapshotFinding` twins;
  - `render_coverage` in `solid-facts-backend/src/snapshot_emission.rs`.

## Context

ADR 0202's default output lists analysis-coverage gaps one line per rule and
message. Messages name their subject (`DataGrid invokes .countLabel …`, `the
reactivity contract for @tanstack/solid-query leaves …`), so a line is a
single helper or a single export. On the rc.13 corpus, 3,252 coverage sites
made 1,962 lines. That is a list to scroll, not a summary.

Grouping by message text would mean parsing the analyzer's own prose.
Grouping instead needs what the gap *is* and what it is *about*, as data.

## Decision

1. **Each coverage finding states a family and a subject**, set where the
   finding is projected, from the defect's own fields:

   | Family | Defects | Subject |
   | --- | --- | --- |
   | `package-contract` | an import whose package contract is missing or incomplete | the package name (`@scope/name`, without a subpath) |
   | `own-export-contract` | a callback this project's export hands to code with unknown timing (open programs only, ADR 0202) | the export |
   | `caller-supplied-member` | a helper invoking a member of a caller-supplied value (`parameter-member-*`, `exported-parameter-member-dispatch`) | the helper |
   | `call-target` | a call whose runtime target cannot be selected | the call |
   | `undescribed-callee` | a reactive value passed to a function with no described behaviour | the function |
   | `leaf-callback` | a call in a leaf owner's callback whose body cannot be followed | the owner primitive |

   The fields are `coverageFamily` and `coverageSubject` in JSON, and are
   absent on every other finding.
2. **The default output prints one group per family.** Each group gives:
   - its site count and how many distinct subjects it covers;
   - its five most frequent subjects, with counts;
   - its first site.

   A coverage finding with no family (`unaudited-solid-release`) keeps its
   per-message line. `--format full` and `json` are unchanged apart from the
   two fields.

## Consequences

- A user sees what kind of gap dominates, and which package or helper to
  look at first, without reading every site.
- A new kind of coverage defect needs a family, or it falls back to
  per-message lines. Nothing is hidden, only less compressed.

## Evidence

- **Process test** `default_output_groups_coverage_gaps_after_findings_to_review`
  pins the family lines on `feedback-tiers-open`. Its `forward` export is an
  `own-export-contract` gap and its `ageOf` helper a `caller-supplied-member`
  gap, each with its subject and first site.
- **Unit test** `a_coverage_subject_names_the_package_not_the_subpath`.
- **Coverage:** 175 fixture projects, 924 findings, unchanged. Snapshots do not
  record the new fields.
- **CLI tests** (378) and backend process tests (18) pass.
- **`app-game`** (rc.13 corpus, browser host): the coverage section lists 815
  sites in 5 lines. Before, it was one line per message.
  - 408 leaf-callback sites (`createTrackedEffect` 318, `onSettled` 90);
  - 299 caller-supplied-member sites in 185 helpers, led by `drawPoints` (18);
  - 68 package-contract sites in 60 packages, led by `@solidjs/router` (9);
  - 23 undescribed-callee sites, led by `createElementSize` (12);
  - 17 call-target sites.
