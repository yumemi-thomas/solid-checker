# ADR 0194: The audited Solid 2 release is rc.13

- Status: accepted and implemented (2026-10-05). Owner request of 2026-10-05:
  focus on the latest Solid release.
- Owners:
  - the Solid 2 release review (`rust/crates/solid-dialect/src/solid_2/releases.rs`:
    `AUDITED_INSTALLATION`, `READ_RELEASES`);
  - the export tables and module ownership (`exports/solid_v2_solid_js.rs`,
    `Solid2::modules`, `namespace_import_primitives`);
  - the tsc oracle pin (`fixtures/tsc-oracle/packages.json`).
- Relation: moves the audited release named by ADR 0127 from `2.0.0-rc.9` to
  `2.0.0-rc.13`. Nothing about which releases are analyzed or refused
  changes. The review is
  `docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-vocabulary-review.md`.

## Context

`solid-js`, `@solidjs/signals` and `@solidjs/web` `2.0.0-rc.13` are `next` on
npm, published 2026-09-30. The audited triple was rc.9 (2026-09-18). The
release table read only up to rc.9, so an rc.13 project was analyzed with the
conservative vocabulary and lost all seven release-dependent answers.

The review found rc.13 vocabulary-compatible with rc.9:

- every timing, tracking, ownership and write-guard premise holds (134 probes,
  dev and prod);
- the seven release-dependent answers hold;
- rc.9's five broken root re-exports are fixed.

## Decision

1. **rc.13 is the audited triple.** `AUDITED_INSTALLATION` names it. Its
   vocabulary is rc.9's (`Solid2::AUDITED` stays `Solid2::RC9`), with no gap.
2. **The read releases are an explicit list**, `READ_RELEASES` = rc.0–rc.9 and
   rc.13. This replaces `NEWEST_READ = 9`.
   - The three answers rc.9 introduced (B2 `dynamic` static form, B3 `omit`
     predicate, N3 store-setter root guard) hold "from rc.9" among read
     releases, so for rc.9 and rc.13.
   - rc.10 and rc.11 were published and never read, rc.12 was never
     published, and rc.14+ do not exist yet. All of them stay unread: the
     conservative answers, under `SC9014`.
3. **rc.9 becomes an older release.** It keeps its answers and its re-export
   gap, and gains the notice that it is older than the audited release, as
   rc.0–rc.8 already had (owner decision of 2026-09-27, ADR 0127 § 3).
4. **`solid-js/internal` is owned.** rc.13 declares `createErrorBoundary`,
   `createLoadingBoundary` and `createRevealOrder` only there. The export
   table lists both the root and `solid-js/internal` for the three, the
   dialect owns the subpath, and a namespace import from it exposes them. On
   rc.0–rc.9 that subpath does not export them, so a named import there is a
   TypeScript error and nothing changes. On rc.13 a root import of them is
   TS2305, and the checker stays silent.
5. **No negative row is carried to rc.13.** The archive-scoped rows serve
   package certification and contract inference only, not the analysis of
   an application. ADR 0189 retires certification as the source of package
   knowledge, so re-reading the rows (about 39, ADR 0127 § 2) would buy
   nothing going forward.
   - rc.13's archives are not listed in `AUDITED_ARCHIVES`, which lists an
     archive to say which rows answer for it.
   - Certification on rc.13 therefore fails closed at the identity gate, and
     closes no claim domain from a row.
   - The tarball integrities, re-derived from the registry, are recorded in
     `benchmarks/package-contract-v2/phase0/rc13/`.
6. **The tsc oracle moves with it.** It installs the rc.13 triple at the
   re-derived integrities, as the gate requires.

## Consequences

- An rc.13 project is analyzed with the audited vocabulary and certifies when
  nothing else is open. An rc.9 project now gets the notice.
- Fixture snapshots: 21 changed, all by gaining the `SC9014` notice on an
  rc.9-pinned fixture (17 notices), and no other finding moved. A new control,
  `store-root-write-rc13`, is the rc.13 twin of `store-root-write-rc9` and
  carries no notice.
- Not moved here, as in ADR 0127, each in its own change: the ecosystem
  benchmark's audited constant (`AUDITED_SOLID_2`) and signals head, the
  accepted tier's environments, and the compiler fork. The fork's runtime
  helper interface is compatible with rc.13 (review § 4); adopting the
  prepared rc.13 rebase needs the fork branch pushed, which is the owner's
  call.
- Open from the review: D4, whether SC1002 agrees with every case in which
  rc.13's new `UNTRACKED_READ_AFTER_AWAIT` warning stays silent.

## Evidence

- `solid-dialect` tests pass (89), including `every_reviewed_triple_answers_as_the_reviews_measured`,
  which now has an rc.13 row (rc.9's answers, no gap) and asserts that rc.10,
  rc.11, rc.12 and rc.14 get the unread answers.
- tsc oracle gate on rc.13: 103 cases hold on both sides, and 27 rules carry
  a keystone, with no `SC9014` on the oracle install.
- Coverage: 166 fixture projects, 878 findings (snapshot changes above).
- `tsc --noEmit` over `store-root-write-rc13/store.ts` against the real rc.13
  install reports nothing. The stub's declarations are byte-faithful to
  rc.13's.
