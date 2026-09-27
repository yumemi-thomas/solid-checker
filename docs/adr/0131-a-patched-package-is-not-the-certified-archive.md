# ADR 0131: A patched package is not the certified archive

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the one admission rule (`accepted_bundles.rs`: `admit_by_artifact`,
  `environment_difference`, `admission_refusals`), the installed-tree readers
  (`diagnostics.rs`: `installed_artifact_bytes`, `admission_input_paths`), the
  patch-record reader (`installed_patches.rs`), and the installed snapshot root
  (`contract_certification.rs`: `installed_package_snapshot_root`)
- Relation: amends ADR 0123, ADR 0125 and ADR 0126. Their admission steps stay
  in force, and this ADR adds a byte check to step 2 and a patch check to
  step 3. It is the admission-side twin of the consumer-environment derivation,
  which already refuses to deliver a patched package
  (`scripts/ecosystem-benchmark/lib/consumer-environments.mjs`).

## Context

Admission compares an installed package with a receipt by name, manifest
version and lockfile integrity. That covers the certified package (through the
acceptance root) and every entry of its dependency environment. But a lockfile
integrity records what was fetched, not what is on disk. Every package manager
that patches a dependency keeps the published integrity and changes the files:

- pnpm applies `patchedDependencies` and keeps `resolution.integrity`;
- Bun does the same with its own `patchedDependencies`;
- `patch-package` rewrites `node_modules` from a postinstall script, and no
  lockfile hears about it.

So a contract certified for the published bytes could be admitted for bytes
the consumer patched. The live case is viviana-ui (`viviana-ui-main-b005c00a`).
Its lock patches `@tanstack/solid-start@2.0.0-rc.8` and still states the
registry integrity for it.

## Decision

**A package whose installed bytes are not the certified archive's is not
admitted, and the refusal names why.**

### The certified package: its bytes (step 2b)

Every policy-2 receipt signs `snapshotRoot`. That is the digest of the
published archive the proof read: every member's package-relative path and
bytes, plus the directories derived from them (`snapshot_root`). Admission now
recomputes that root from the files installed for the imported package and
requires it to equal the signed one. The package's own `node_modules`
directory is left out, because npm and Yarn nest its dependencies there. Any
other member that is not a regular file refuses.

This check covers every way the files can change: pnpm and Bun patches,
`patch-package`, the package's own install script, a hand edit. Before hashing,
the patch reader below is consulted, so that a recorded patch is named as the
reason.

Refusal: `AdmissionRefusal::InstalledBytesDiffer`. `contract check` reports it
as, for example:

> the installed package's files are not the certified archive's:
> plain-package@1.0.0 is patched (pnpm-lock.yaml patchedDependencies)

With no recorded patch it reports:

> the installed package's files are not the certified archive's: they do not
> reproduce the signed snapshot root, so something changed them after install

A receipt that signs no snapshot root is never admitted.

### The dependency environment: the patch records (step 3)

An environment entry states only `{name, version, integrity}` and carries no
byte digest, so the check above has nothing to compare it with. The
environment replay (both the all-lookups rule and the edge rule) therefore
refuses an entry that matches by identity and that the tree records as
patched. The records are read from every ancestor of the install directory,
the same walk the lockfile integrity takes:

| manager | where the record is read |
| --- | --- |
| pnpm | `pnpm-lock.yaml` `patchedDependencies`; `(patch_hash=…)` in snapshot keys and dependency versions; `pnpm-workspace.yaml` and `package.json` `pnpm.patchedDependencies`; a `.pnpm` store directory named `…patch_hash=…` |
| Bun | `bun.lock` and `package.json` `patchedDependencies` |
| Yarn | `yarn.lock` descriptors resolved through `patch:` (Berry is already refused because it states no integrity, and this keeps it refused when another lockfile sits beside it) |
| npm, Yarn classic, any manager | `patch-package` files (`<parent>++<scope>+<name>+<version>….patch`) under `patches/` next to any `package.json`, and under every `--patch-dir` that a `package.json` script passes |

Rules:

- A key that names no version (`name`), or a range, patches every version, as
  pnpm and `patch-package` read it.
- A `patch-package` file patches its package whatever the installed version,
  because `patch-package` applies a mismatched patch with a warning.
- A declaring file that exists and cannot be read or parsed refuses every
  package in the tree. Whether anything is patched is then not a fact the tree
  states.

Refusal: `EnvironmentDifference::Patched`, reported through the existing
`EnvironmentDiffers`, which is what an environment mismatch reports. For
example:

> its dependency environment differs: leaf-dep@1.0.0 resolved from
> @solidjs/signals@2.0.0-rc.6 is patched (pnpm-lock.yaml patchedDependencies),
> so its installed bytes are not the published archive the certification read

At the import site, a refused contract behaves exactly as an environment
mismatch does: nothing is admitted, and the import reaches the acceptance gate
(`SC9005`, uncertifiable), with the refusal as evidence. `contract certify`'s
self-admission check (ADR 0126) now also says "certified but not admitted" in
a patched tree.

### Cache inputs

`admission_input_paths` adds each ancestor's `package.json` and
`pnpm-workspace.yaml`, and every patch file the reader would consult. Adding or
removing a patch file therefore changes the retained daemon's key.

### WASM

The WASM adapter has no filesystem. Its host already asserts each installed
package's identity (`installedPackages`), and it now also asserts
`snapshotRoot` for each one. An entry without it admits nothing. The WASM
adapter admits only bundles with an empty environment, and the tier has none,
so what WASM admits today does not change.

## Consequences

Measured (2026-09-27):

- The signed `snapshotRoot` reproduces from installed files for all 33
  distinct packages in the compiled-in tier. They were found in leftover
  ecosystem certification trees installed by npm and Bun.
- A pnpm install and the extracted `npm pack` tarball give the same root for
  `solid-js@2.0.0-rc.9` (33 files) and `@solidjs/signals@2.0.0-rc.9`
  (119 files).
- Cost, in the unoptimized debug build: `@kobalte/core@2.0.0-alpha.2` (401
  files, 2.3 MB) takes 94 ms cold and about 30 ms warm, and
  `@tanstack/solid-router@2.0.0-rc.8` (527 files) takes 84 ms cold and about
  25 ms warm. The cost is paid at most once per installed package per
  admission, memoized by canonical directory, and only for a package whose
  acceptance root already matched. Reading the patch records takes 1.5 to
  5 ms per install directory, once per admission.
- Coverage: 122 projects, 687 findings, nothing moved. No fixture admits by
  artifact.

Tests: `accepted_bundles::tests` exercises real trees. pnpm with the patch on
the environment package, on an unrelated package, on a transitive environment
package, and on the certified package. A hand edit with no record. A nested
`node_modules`. Bun `patchedDependencies`. `patch-package` under npm.
`installed_patches::tests` covers each record form, and
`project_catalog_environment_process.rs` runs the same cases through the real
binary, for flat npm with `patch-package` and for pnpm with
`patchedDependencies`.

Still open (not detected, or refused more than strictly needed):

- **Changes to an environment package that leave no record** are not seen: a
  hand edit, an install script that rewrites another package, or a
  `patch-package` invocation from a script file whose `--patch-dir` no
  `package.json` names. Environment entries carry no byte digest. Closing this
  needs each entry to carry its archive's snapshot root, which is a receipt
  change.
- **A package whose archive bundles dependencies** under its own
  `node_modules` never reproduces its root, and is refused.
- **An install that rewrites `package.json`** (npm 6 added `_resolved` and
  similar fields) is refused.
- **The retained daemon** does not treat the installed files themselves as
  inputs. A hand edit made while the daemon lives keeps the cached verdict
  until another input moves. This was already true of every finding.
