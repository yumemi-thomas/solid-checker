# ADR 0224: Open contract claims are one notice per package

- Status: accepted and implemented (2026-10-07).
- Owners: `projection::collapse_unaccepted_contract_defects`;
  `scripts/app-import-metric.mjs` reads the grouped sentence.
- Relation: extends the kobalte defect-7 collapse of open claims at call
  arguments, which grouped per `(package, export, open domains)`.

## Context

Accepting a package contract should never make a report longer while
improving its answers. ADR 0223 did that. A package with no contract was one
acceptance-gate notice listing its exports. A package whose accepted contract
left claims open was one notice per used export at its imports, plus one per
export at its call arguments. The rc.13 corpus went from 675 to 741 `SC9005`
rows, while three obligations were proven clean and four violations proven.

## Decision

1. **Open claims at imports, and at call arguments, group per
   `(package, open domains)`.** The export is no longer part of the key. The
   fix is the same for every export the contract leaves those domains open on:
   complete the package's contract.
2. **A group of several exports names them.** "the reactivity contract for M
   leaves D unknown for N imported (or called) exports: A, B, …; … (K import
   sites)", with at most six names listed. Its subject is `package`. A group
   of one export keeps its exact single-export wording and `package-export`
   subject.
3. **Other gates stay per export.** That covers a missing export summary, an
   unbound claim, an argument-shape refusal and a runtime-identity conflict,
   because each one's fix is that export or that call.
4. **The app-import metric reads the grouped sentence.** It attributes each
   site to the export whose name is the site's text, and to `?` when the text
   is not a listed name.

## Consequences

- rc.13 corpus: `SC9005` went from 741 to 648 rows, below the 675 before
  ADR 0223. No violation moved, and every change is a merge into a group.
- Snapshots of `package-argument-container-consumer`,
  `package-open-claims-call-sites` and `package-owned-computation-consumer`
  lose their merged rows. The unit test
  `open_claims_at_call_arguments_collapse_per_package_across_files` pins the
  grouping, the order and the wording.
- A reader who needs the per-export answer still has it: the message names
  each export, and the ESLint adapter reports the finding in every file
  holding one of its sites.
