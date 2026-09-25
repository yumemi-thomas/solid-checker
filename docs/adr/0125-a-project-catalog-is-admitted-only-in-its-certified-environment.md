# ADR 0125: A project catalog is admitted only in its certified environment

- Status: accepted and implemented (2026-09-26); written with the implementation
- Date: 2026-09-26
- Owners: project-catalog admission (`contract_interface.rs`,
  `diagnostics.rs`), the shared admission rule (`accepted_bundles.rs`), and the
  policy-2 receipt's signed payload (`policy2_receipt.rs`)
- Relation: extends ADR 0123 from the compiled-in tier to the project tier
  (`contract certify` into `<project>/.solid-checker/`), so there is one
  admission rule rather than two. Prerequisite for project-side certification
  as a delivery path, which a 2026-09-26 sweep of 151 real consumers found to be
  the one that can reach them (the compiled-in tier matched 0 of their
  environments).

## Context

Two gaps remained after ADR 0123:

1. Project-catalog admission matched an accepted artifact by its acceptance
   root alone (`let _ = (project_directory, trust);`). A catalog certified in
   one tree was admitted in any tree with the same package bytes, whatever
   dependencies were installed beside it.
2. `artifactAcceptanceRoot` was not part of the Ed25519 signed payload, so a
   project-signed receipt's root could be rewritten to another artifact
   without breaking its signature. This was confirmed by a failing test before
   the fix.

## Decision

**Project catalogs and compiled-in bundles are admitted by one function,
`admit_by_artifact`.** It checks, in order:

1. the receipt states a dependency environment;
2. the consumer's installed identity reproduces the signed acceptance root;
3. the signed environment is installed exactly (ADR 0123's
   `environment_is_installed`);
4. the resolved file is reached and case selection admits it.

The catalog reader verifies the published `dependencyEnvironment` entries
against the signed root and refuses the whole catalog on a mismatch.

**A receipt issued before ADR 0123, which states no environment, is refused
project-wide.** It still authenticates, and it still applies to the exact file
it was certified from. No checked-in fixture relied on the old behaviour.

**A persistent-local or portable receipt signs its acceptance root.** The
signed payload gains a tagged frame, `artifact-acceptance-root:v1`, whenever a
root is stated, following the `dependency-environment-root:v1` model. There is
no `receiptVersion` or policy-digest change, and a receipt without a root is
byte-identical to before.

A receipt signed the old way now fails with a named error:
`UnsignedArtifactAcceptanceRoot` ("issued before artifactAcceptanceRoot was
signed … re-run `solid-checker contract certify`"). That error only names the
refusal; it never accepts the receipt.

Built-in receipts are not framed: their authority is the compiled-in digest of
the whole file, which covers the root already, and framing them would
invalidate all 164.

## Consequences

Measured (2026-09-26):

- coverage: 87 projects, 452 findings, nothing moved; no fixture or receipt
  re-issued;
- a process test (`project_catalog_environment_process.rs`):
  - certified in its own tree: `certified`;
  - the same catalog in a tree with a different dependency version: `missing`;
  - a re-pointed root: refused, "receipt signature is invalid";
  - no environment: `missing`;
- the real CLI on `@solid-primitives/context@2.0.0-next.2`:
  - certified under bun: `certified`, with a two-entry environment;
  - copied into a signals rc.0 tree: `missing`;
  - tampered root: exit 2.

Still open:

- **An npm lockfile yields an empty environment.** Dependency-source
  acquisition reads only `bun.lock` and `pnpm-lock.yaml`, so `certify` under
  `package-lock.json` records `dependencyEnvironment: []`, and an empty
  environment is admitted in any tree with the same artifact. That is the
  ADR 0123 gap again, reached through the lockfile format. It must fail closed:
  the next change refuses to state an environment it could not acquire.
- **A project catalog holds one package.** Each `certify` replaces it.
- **The `missing` status does not say why.** It never names an environment
  mismatch, and its remedy text still describes the retired generate/verify
  flow.
