# ADR 0197: The rc.13 archives carry the rc.9 rows

- Status: accepted and implemented (2026-10-05). Owner request of 2026-10-05
  to re-read the negative rows on rc.13. Supersedes ADR 0194's § 5 ("no
  negative row is carried to rc.13").
- Owners: `AUDITED_ARCHIVES`, `NEGATIVE_ROWS` and their audit tables in
  `rust/crates/solid-dialect/src/solid_2.rs`; `audited-archives.json`; the
  rc.13 slices under `rust/crates/solid-dialect/audited-slices/solid-v2/rc13/`;
  the archive provisioning and the `SOLID_CHECKER_RC13_ARCHIVE_ROOT` arm.
- Relation: the audit is
  `docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-negative-rows.md`,
  with its tools and probe outputs beside it. No wire change.

## Context

Rows are archive-scoped: a row read on one prerelease's bytes answers for no
other. ADR 0194 made rc.13 the audited release and carried no row to it, so
certification on rc.13 closed no claim domain from a row. The rows also back
two consumer-side answers, the returned memo and signal accessors (ADRs 0162
and 0175), which were bound to rc.9.

## Decision

1. **The 48 rc.9 rows were re-read on rc.13's own bytes**, by the method of the
   audit that granted each rc.9 row, in every build each archive's `exports`
   map can select. **47 are granted**:
   - 41 flat rows;
   - the two `browser` host-target rows (`solid-js` `createSignal` and
     `createMemo` `creates`), whose flat rows stay withheld for the rc.9
     reason: the server builds reach `ctx.serialize`, measured again on rc.13
     (§ W1, § W2);
   - the four argument-scoped `reads` rows.

   **One is withheld:** `@solidjs/signals` `reconcile` `reads` (§ 24). rc.13's
   optimistic-store adoption reaches an engine read path
   (`applyAdopt → optHooks.optimisticView → readerOverride → nodeValue →
   serve`), and the reading could not establish that every application runs
   under `authoritativeServe()`.
2. **Each row cites its rc.13 bytes.** There are 189 citations, each with the
   file digest from `phase0/rc13/*/files.json` and the slice digest. The 111
   distinct slices are checked in. The rc.13 archives are listed (tuples
   re-derived from the registry tarballs), provisioned by
   `make audited-archives-provision`, and armed in `make test-rust` and
   `scripts/verify.sh`.
3. **The returned-accessor answers admit rc.13.** The reading found them to
   hold (audit, "Other rc.9-scoped answers", A1).
4. **One new judgement, accepted by the owner (O1):** rc.13's dev and observe
   builds register roots in a module-level `FinalizationRegistry` and
   `WeakRef` (`registerRoot`, `dev-shared.js:775`). The reading treats that as
   engine bookkeeping, not a `create`, as for every other dev-only registry
   the archive keeps. It is recommended for acceptance because the registry:
   - is absent from the production build;
   - holds roots weakly, so it never keeps one alive;
   - is released on disposal (`unregisterRoot`, `:784`);
   - serves only the engine's `graphSize()` leak count.

   Nothing the application can observe depends on it, and nothing in it needs
   disposal by the caller. **Accepted by the owner, 2026-10-05.**

## Consequences

- Certification on rc.13 closes the same domains it closes on rc.9, except
  `reconcile` `reads`.
- The `DEFINITION_ALIASES` for `createOptimisticStore` and `createProjection`
  hold on rc.13 (the same `export { … as … }` forms).
- Residual approximations are the audit's: call graphs are name-based
  over-approximations, with node-field dispatch and hook slots handled by
  hand; the hook-slot follow-up ran on the dev build only.

## Evidence

- `solid-dialect` tests pass (89), among them:
  - `the_negative_table_is_derived_from_the_audited_documents`: shipped rows
    equal derivable minus withheld, with 41 rc.13 flat rows, 5 host-target
    rows and 8 argument rows in total;
  - `negative_rows_are_sorted_unique_and_canonical`;
  - `every_negative_row_citation_resolves_to_the_bytes_it_claims`: 484
    citations. Run with all four archive roots armed, so every rc.13 slice
    was also read out of the provisioned archive.
- The audit's self-check: every slice file and cited range hashes to its
  digest, and every file digest equals `files.json`.
- `make verify` passes (705 s), after the benchmark's pin of the audited
  archives (`scripts/ecosystem-benchmark/dialect-authority.test.mjs`) moved
  with them.
