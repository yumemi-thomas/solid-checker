# ADR 0127: The audited Solid 2 release is rc.9

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the Solid 2 release review (`rust/crates/solid-dialect/src/solid_2/releases.rs`),
  the `SC9014` notice (`rust/crates/solid-facts-backend/src/dialect.rs`,
  `release_scope.rs`), the negative rows (`rust/crates/solid-dialect/src/solid_2.rs`)
- Relation: moves the audited release named by ADR 0110 § 1 and the rc.1-rc.8
  and rc.9 reviews from `2.0.0-rc.3` to `2.0.0-rc.9`. Nothing about which
  releases are analyzed or refused changes.

## Context

Until now the audited triple was `solid-js`, `@solidjs/signals` and
`@solidjs/web` at `2.0.0-rc.3`. Users install rc.9: every rc.9 project got the
`SC9014` notice, the accepted tier carried no rc.9 environment, and precision
work was measured on a release nobody starts a project on.

In the dialect "audited" had two meanings that happened to coincide: the triple
`SC9014`'s hint tells a user to pin, and "the review lists no gap for it". rc.9
had two release-wide gaps: `@solidjs/signals` rc.9 carried 5 negative rows
where rc.3 carries 25, and `solid-js` rc.9's typings re-export five names they
no longer declare (`createErrorBoundary`, `createLoadingBoundary`,
`createRevealOrder`, `sharedConfig`, `$DEVCOMP`; an upstream defect).

## Decision

1. **rc.9 is the audited triple.** `AUDITED_INSTALLATION` and `Solid2::AUDITED`
   name it, and a project with no `solid-js` resolved is analyzed under it
   (`Solid2::DEFAULTED`). The conservative answers for an unresolved owner are a
   separate vocabulary (`Solid2::CONSERVATIVE`) and do not follow the audited
   release. Dialect ids are keyed by features against the conservative
   vocabulary, so no install's id changed.
2. **rc.9's gaps are closed or scoped.**
   - `@solidjs/signals` rc.9 carries rc.6's 24 rows, read on rc.9's bytes
     (`docs/package-contract-v2/audits/2026-09-27-solid-2-rc9-signals-negative-rows-parity.md`);
     `createOptimisticStore` `reads` is withheld because rc.9's bytes read the
     store it creates, which is an answer, not a gap.
   - `solid-js` and `@solidjs/web` rc.9 are audited archives with their own rows
     (`docs/package-contract-v2/audits/2026-09-27-solid-2-rc9-core-and-web-negative-rows.md`).
   - The re-export gap is due only where a project reaches one of the five
     names from `solid-js`, and fails closed where the facts cannot tell
     (owner decision, 2026-09-27).
3. **Older releases keep their answers and get the notice.** rc.0-rc.8 are
   analyzed with the answers their reviews gave them, and `SC9014` names them
   as older than the audited release: new rules and precision work are
   measured on rc.9 only (owner decision, 2026-09-27). No per-release variant
   is removed.

## Consequences

- An rc.9 project that imports none of the five names certifies; an rc.3
  project no longer does. Fixture snapshots moved accordingly: 31 gained the
  notice, 9 lost it, and `write-scope` (no `solid-js` stub) gained two `SC2001`
  findings, the store setters in a component body that rc.9's guard rejects.
- Re-reading rc.3's rows for the rc.9 audit found five rc.3 rows contradicted
  by rc.3's own bytes (`Show` `reads` and `creates`, `Loading` `creates`,
  `render` and `hydrate` `reads`); they are withdrawn.
- The tsc oracle, the ecosystem benchmark's ceiling and the accepted tier's
  consumer environments move to rc.9 with it (their own commits).
