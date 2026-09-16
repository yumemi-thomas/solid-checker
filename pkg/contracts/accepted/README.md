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
  declaration targets, the object paths and their digests, and the receipt
  bindings.
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

Only the artifact. `policy2_artifact_acceptance_root` commits to the five
identity fields and to **no importer and no path**, so a project whose installed
tree reproduces that root demonstrably resolved the same published artifact the
contract was proven about, whatever file imported it. Admission recomputes the
root from the consumer's own lockfile integrity and refuses when it does not
reproduce — see `accepted_bundles::admitted_bundle_artifacts`, which shares its
case-selection rule with the project-catalog tier so "does this acceptance
apply here" has one answer rather than two.

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
conditions)`. It helps a user on that version and nobody else, silently — an
unmatched project simply keeps the behaviour it has today.

Two things can invalidate a bundle, and only one of them is a rebuild:

- **The proof policy.** `authenticate_policy2_receipt` compares the receipt's
  `policyDigest` against the running `proof_policy_2()`; a policy change
  therefore refuses every bundle in the set, and they must be re-certified and
  re-issued. `every_bundle_this_build_carries_authenticates` fails first, so
  this cannot ship silently.
- **A new package version.** Nothing breaks; the bundle stops matching. Adding
  the new version is a fresh certification run.

The verifier build digest is *not* a refresh trigger, despite what an earlier
note in `phase21/2026-09-15-what-blocks-the-open-claims-gate.md` § 23 said:
authentication compares the compiled-in entry's `verifierBuildDigest` to the
receipt payload's, and both come from the bundle. Rebuilding the checker does
not invalidate a bundle.
