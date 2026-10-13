# ADR 0188: Two versions of one dependency nest as Node resolves them

- Status: accepted and implemented (2026-10-05). Twelfth lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners:
  - the private Type Facts project's placement (`nest_under_importers`, used
    by `materialize_with_source_refs` for dependency plans and declaration
    sources);
  - the CLI's declaration-source lookup and lock locators
    (`certify-contract.mjs`, `published-contract-graph.mjs`).
- Relation: amends ADR 0184's lookup rule. No wire change and no new trust.

## Context

`@tanstack/solid-router`, the most-imported package in the 38-app corpus,
failed in-place certification in `sefer` and `spotify-desk-thing` with:

> distinct authenticated snapshots collide at …/node_modules/seroval/…

Two versions of `seroval` are installed there: 1.6.4 for `solid-js` and 1.6.7
for `@tanstack/router-core`. pnpm keeps each package's real root in its own
`.pnpm/<name>@<version>/` directory, and each importer finds its own version
beside it. The private project places every dependency relative to the
certified package's installation directory, falling back to a flat
`node_modules/<name>`. Every pnpm store path falls back, so both versions land
on one path.

Fixing that exposed a second defect. ADR 0184 looked a source's dependencies up
from the importer's *written* path first. A copy hoisted above a pnpm link
(`app/node_modules/seroval`) then won over the sibling Node resolves from the
real path. Admission replays lookups from real paths, so the certification
recorded an environment the install does not reproduce:

> seroval resolved from @tanstack/router-core installed 1.6.2, certified 1.5.6

## Decision

1. **Nest a second version under its importer.**
   - When a different snapshot already holds a placement's target, it moves
     to `<importer>/node_modules/<name>`, where that importer's resolution
     finds it first. The first holder keeps the slot.
   - The importer is the plan's recorded `resolved_import.importer`, or a
     source's single recorded resolution edge. It is located among the
     placements by installed root, written or real.
   - A source with several importers is not moved: nesting it under one would
     let the others resolve another version's files.
   - No single importer, an importer that is not a placement, or a nested slot
     that is also taken: the target stays, and the byte-identity check refuses
     as before.
2. **Dependencies are looked up from the importer's real path**, as Node and
   TypeScript (without `preserveSymlinks`) resolve them. This replaces ADR
   0184's written-path-first fallback.
3. **Path-keyed lock locators compare canonical paths.** npm and Bun locators
   are paths relative to the lockfile's directory. Both sides are now
   canonicalized, so a real-path root and a written lockfile path cannot
   disagree (macOS temporary directories move `/var` to `/private/var`).

## Consequences

- Every materialized copy is still its plan's authenticated snapshot; only
  its path changes. Nothing is placed by name.
- A declaration source's installed root is reported as its real path.

## Evidence

- Unit test `a_second_version_nests_under_its_importer_and_nothing_else_moves`:
  - the first holder keeps the slot, and the second version nests under its
    importer;
  - the same snapshot twice is one materialization;
  - no importer, or an importer no placement installs, keeps the colliding
    target.
- CLI suite passes (378 tests). A source's installed root is now asserted as
  its real path.
- 38-app local certification rerun (`rust/target/local-certify/local-0188`):
  `@tanstack/solid-router` goes from refused to certified in `sefer` and
  `spotify-desk-thing`. No other certification changed status.
- The new catalogs were bundled into the tier (ADR 0186 procedure, 46 bundles,
  1,876 in total). On the plain release binary the 38-app browser sweep keeps
  273 violations. Uncertifiable goes from 10,377 to 10,396 (+21, −2): those
  router imports now carry its partial contract instead of none.
- Misuse ledger unchanged at 79 of 123, with no correct twin flagged. Coverage,
  the contract corpus and the ownership gate are unchanged. `make test-rust`
  passes except the known pre-existing
  `sessions_process::incremental_contract_exports_refresh_changed_summaries`.
