# ADR 0123: A compiled-in contract is admitted only in the environment it was certified in

- Status: superseded in part by [ADR 0228](0228-the-certified-contract-tier-is-retired.md) (2026-10-08): the compiled-in tier is retired. Artifact and environment admission for project catalogs and the authored tier remains in force. Originally: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the policy-2 receipt (`policy2_receipt.rs`), certification finalization
  (`finalization.rs`, `dependencies.rs`, `type_facts.rs`), the catalog and bundle
  tool (`contract_interface.rs`, `contract_bundling.rs`), the compiled-in tier
  loader and admission (`accepted_bundles.rs`, `diagnostics.rs`), and the bundler
  (`scripts/bundle-accepted-contracts.mjs`)
- Relation: closes a delivery gap that ADR 0007's 2026-09-25 amendment
  (archive-scoped negative rows) made reachable. Restores for policy 2 what the
  retired policy-1 path's `checked_closure_matches` did. It is the delivery
  precondition for step 7 part 2 (condition-aware rows, ADR 0124).

## Context

A compiled-in bundle was admitted for a consumer's import after comparing only
the imported package's identity: name, version, lockfile integrity, requested
entrypoint and conditions (the acceptance root). A certification's verdict
also depends on its dependency environment:

- which dialect archive answered its negative rows (since 4878e163,
  `@solidjs/signals@2.0.0-rc.6` has audited rows and rc.0 has none);
- which certified contract each semantic dependency contributed.

Measured on the 2026-09-25 census run, the same bytes certify differently on
the corpus's floor row (signals rc.0) and head row (rc.6): `createMicrotask`,
rootless `createCallback` and scheduled `throttle` close `creates` only on
head. The bundler keyed bundles without the environment, and its
"keep the closing certification" rule assumed an open-versus-closed difference
came from demand scope. So a regenerated tier would have applied rc.6-proven
negatives to rc.0 projects, and it dropped `@solid-primitives/utils` `.` as a
conflict. The regeneration was held back.

## Decision

**A receipt binds the dependency environment it was certified in, and a
compiled-in bundle is admitted only where the consumer's installed tree
reproduces that environment exactly.**

### The binding

A new receipt binding, `dependencyEnvironmentRoot`, is a hash over the sorted
`{name, version, integrity}` entries of:

- every dependency snapshot the Type Facts census admitted as a root, which
  includes the dialect archives the axioms answer from;
- statically from the graph, each transitive semantic dependency and every
  source of each of them;
- never the package itself.

It is part of the signed payload whenever stated. The entries travel beside
the root, as `dependencyEnvironment` in the catalog and index entries, and
every reader recomputes the root from them and refuses a mismatch.

The field is optional, as `artifactAcceptanceRoot` was: older receipts re-encode
and verify byte for byte, and there is no `receiptVersion` or policy digest
change. On the graph lanes the environment is graph-wide, because the graph
census uses graph-wide roots. That is sound but strict: a dependency node's
certification is rarely admissible, and a root row's is.

### Admission

A bundle is admitted only when all of these hold:

1. its acceptance root matches, as before;
2. it states an environment;
3. every entry is found by Node resolution from a located package, starting
   at the imported package's real path and continuing from each package
   found;
4. every lookup of that name from every located package reaches exactly that
   entry's name, manifest version and lockfile integrity.

Anything missing, different, ambiguous or unreadable refuses the bundle, and
the import then behaves as if no bundle existed. An environment naming two
copies of one package never applies. The analysis dialect is never a
substitute for the installed archive. WASM, which has no filesystem, admits
only bundles with an empty environment.

### The bundler and the index

The bundle key includes the environment, and `relate()` refines only within
one key, so floor and head certifications become separate bundles.
`bundleIndexVersion` is 2. A version-1 index still loads and authenticates, but
none of its bundles is ever admitted: it supplies nothing, which is sound.

## Consequences

- The checked-in tier (47566ff8, index version 1) is inert until it is
  regenerated. Measured: coverage 87 projects, 452 findings, nothing moved; no
  fixture states a bundled package's integrity.
- Regenerating the tier needs a re-certification by this build. Measured
  results are recorded in the backlog entry of the same date.
- Still open:
  - the project-catalog tier (`admitted_project_artifacts`) still matches by
    artifact alone;
  - `artifactAcceptanceRoot` is not in the Ed25519 signed payload, so a
    project-signed receipt's acceptance root can be edited without breaking
    its signature (built-in receipts are covered by their entry digest). That
    was spun off separately;
  - there is no process-level fixture pair yet (stub signals rc.0 and rc.6
    lockfiles) with a matching control, because the binary admits only
    compiled-in bundles.

## Amendment (2026-09-26): ADR 0126

An environment may now record, per entry, the lookup that reached it
(`resolvedFrom`). For such an environment, admission replays each lookup from
its importer instead of requiring every lookup from every located package to
match, and two copies of one package under different importers apply. The
all-lookups rule above still governs environments without edges, including
every bundle in the tier this ADR regenerated.

## Amendment (2026-09-27): ADR 0131

A lockfile keeps the published integrity for a patched package, so name,
version and integrity do not show that the installed bytes are the certified
ones. Admission now also requires the imported package's installed files to
reproduce the receipt's signed `snapshotRoot` (step 2b). It also refuses an
environment entry that the tree records as patched: pnpm or Bun
`patchedDependencies`, pnpm `patch_hash`, a Yarn `patch:` resolution, or a
`patch-package` file. Both refusals are reported the way a mismatch is.

## Amendment (2026-09-28): index version 3

The per-host tier (ADR 0140) certifies one artifact in one environment for
every host, so version 2 repeated most environments and restated every
receipt's bindings. Measured on the regenerated 729-bundle tier: 7.7 MB, with
`bindings` 48% and `dependencyEnvironment` 45% of it, and only 120 distinct
environments. That is over the repository's 5 MB pre-commit limit.

`bundleIndexVersion` 3 stores each fact once:

- an `environments` table holds each distinct environment, keyed by its
  `dependencyEnvironmentRoot`, the root the receipts already sign. Each bundle
  names its environment by that root (`dependencyEnvironmentRoot`);
- `bindings` are not restated. Authentication already refused any copy that
  was not equal to the receipt payload, and the index pins the receipt by
  digest, so the copy added nothing. The loader reads the bindings from the
  pinned receipt.

The index is refused whole, before any bundle is authenticated, when a table
key is not the root of its own canonical entries, or a bundle names a root the
table does not carry or its receipt does not sign. Admission is unchanged: the
loader resolves each version-3 bundle into the entry version 2 would have
stated, and the tests pin that both spellings load the same bundles. On the
729-bundle tier the index is 1.35 MB, and converting it resolves every bundle
to its version-2 entry exactly. Version 2 still loads with its old meaning,
and the bundler writes only version 3.
