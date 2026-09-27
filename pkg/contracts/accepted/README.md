# The compiled-in accepted-contract tier

Contracts this repository certified, reviewed, and compiles into the checker, so
that a project which never runs `contract certify` still analyses against them.

This is **not** `pkg/contracts/bundled/`. That directory is the retired policy-1
first-party seam for the Solid runtime foundation, and ADR 0027 keeps that
foundation out of package contracts entirely — ordinary analysis takes
`solid-js`, `@solidjs/signals` and `@solidjs/web` from the dialect. A contract
about one of those is refused here, by the generator and by the loader.

## What is here

- `index.json` — one entry per bundle: the five fields that make up the
  artifact's identity (package name, version, tarball integrity, requested
  entrypoint, sorted export conditions), the package-relative runtime and
  declaration targets, the object paths and their digests, the receipt
  bindings, and the `dependencyEnvironment` the receipt's
  `dependencyEnvironmentRoot` commits to (index version 2, ADR 0123).
- `objects/<sha256>.main.json` — the canonical contract document, byte-identical
  to what certification published.
- `objects/<sha256>.receipt.json` — a **built-in** receipt re-issued over that
  same document and those same bindings.

`rust/crates/solid-facts-backend/src/accepted_bundles/embedded.rs` is generated
beside them and is the `include_bytes!` list that puts these bytes in the binary.

## What a bundle asserts, and on whose authority

Nothing in a bundle is re-proven. The document, its semantic digest, its
closed-claims root and every witness root are the certification's own; re-issuing
the receipt changes only *who vouches for them* — from a configured Ed25519
issuer a consumer would have to be told to trust, to this repository, because
these bytes are reviewed here and compiled in.

The built-in receipt authenticates against a compiled-in entry digest over its
own bytes. That is a consistency check on the pair, not a second independent
witness: anyone who can change what this build compiles in can change both. The
authority is review, and it should be described that way.

## What makes a bundle apply to a project

The artifact, **and the environment it was certified in** (ADR 0123).
`policy2_artifact_acceptance_root` commits to the five identity fields and to
no importer and no path, so a project whose installed tree reproduces that root
resolved the same published artifact the contract was proven about, whatever
file imported it. Admission recomputes the root from the consumer's own lockfile
integrity and refuses when it does not reproduce.

That alone is not enough: a verdict also depends on which dialect archive
answered its negative rows (for example `@solidjs/signals` rc.6, which has
audited rows, against rc.0, which has none) and on each semantic dependency's
certified contract. So every bundle carries its receipt's signed
`dependencyEnvironmentRoot` and the `{name, version, integrity}` entries behind
it. The bundle is admitted only when Node resolution from the imported
package's location reaches exactly those entries in the consumer's tree:
anything missing, different, ambiguous or unreadable refuses, and the import
behaves as if no bundle existed. See `accepted_bundles::admitted_bundle_artifacts`,
which shares its case-selection rule with the project-catalog tier.

A lockfile integrity is the record of what was fetched, and a patched package
keeps it (ADR 0131). So the imported package's installed files must also
reproduce the receipt's signed `snapshotRoot`, and no environment package may
be patched by any record the tree keeps: pnpm and Bun `patchedDependencies`
(lockfile, `package.json`, `pnpm-workspace.yaml`), pnpm's `patch_hash`, a
Yarn `patch:` resolution, or a `patch-package` patch file.

One artifact therefore usually has several bundles, one per environment the
corpus certified it in: the floor row (signals rc.0) and the head row (rc.6),
plus dependency-node certifications, whose environment is graph-wide and so
rarely applies. A version-1 index (no environment) loads but admits nothing.

A project's own catalogs always win: the tier is folded in with
`with_fallback`, and admission keeps the first answer for a specifier.

## Regenerating

```sh
make contract-coverage-census   # certifies, and pins what the contracts say
make accepted-bundles           # re-issues the same run's catalogs as bundles
make build-checker-debug
```

Delivery and measurement come from one certification run on purpose: a bundle
set built from a different run than the pinned census would ship contracts
nobody counted.

Read the diff before committing. These are documents this checker asserts to
users about packages they installed; they are review material, not build output,
even though a script writes them.

## Refresh policy

A bundle is pinned to an exact `(package, version, integrity, entrypoint,
conditions)` and dependency environment. It helps a user whose tree reproduces
both and nobody else, silently — an
unmatched project simply keeps the behaviour it has today.

Two things can invalidate a bundle, and only one of them is a rebuild:

- **The proof policy.** `authenticate_policy2_receipt` compares the receipt's
  `policyDigest` against the running `proof_policy_2()`; a policy change
  therefore refuses every bundle in the set, and they must be re-certified and
  re-issued. `every_bundle_this_build_carries_authenticates` fails first, so
  this cannot ship silently.
- **A new package version, or a new version of a dependency in its
  environment.** Nothing breaks; the bundle stops matching. Adding it is a
  fresh certification run.

The verifier build digest is *not* a refresh trigger, despite what an earlier
note in `phase21/2026-09-15-what-blocks-the-open-claims-gate.md` § 23 said:
authentication compares the compiled-in entry's `verifierBuildDigest` to the
receipt payload's, and both come from the bundle. Rebuilding the checker does
not invalidate a bundle.
