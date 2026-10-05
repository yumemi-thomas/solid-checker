# ADR 0191: The tier is admitted on its Solid runtime

- Status: accepted and implemented (2026-10-05). The first slice of
  [ADR 0189](0189-package-contracts-are-authored-and-probe-checked.md),
  decision 3, applied to the contracts the checker already ships.
- Owners: step 3 of `admit_by_artifact` (`accepted_bundles.rs`) and its
  `EnvironmentRule`; `AuthenticCase` in `contract_interface.rs`.
- Relation: relaxes ADR 0123's environment step for the compiled-in tier
  only. Project catalogs keep it unchanged. No wire change, no bundle change.

## Context

The compiled-in tier holds 1,876 bundles, trusted on repository review
(`ReceiptIssuerKind::BuiltIn`). Each bundle applied only to a tree that
reproduces its whole dependency environment. The census and checkpoint
environments matched no app install, so the tier reached a project only when
it installed exactly the tree a certification ran in (ADR 0186).

ADR 0189 decided that a shipped contract applies by package version and bytes,
not by the dependency tree, except for the Solid runtime the proof ran on.

## Decision

1. **Admission by package.** Steps 1, 2 and 2b of `admit_by_artifact` are
   unchanged:
   - the receipt states an environment;
   - every compiled-in citation is still carried;
   - the installed name, version and integrity reproduce the acceptance root;
   - the installed files reproduce the signed snapshot root.
2. **The Solid runtime still matches.** For the compiled-in tier
   (`EnvironmentRule::SolidRuntime`), step 3 needs only the environment's
   entries that name the Solid runtime foundation
   (`solid_dialect::primitive_defining_package`, so the dialect owns the names)
   to be installed as stated.
   - The recorded lookup edges cannot be replayed without the other entries.
     So these entries are checked by the edge-free rule: each must resolve,
     from the package or from a runtime package resolved before it, to
     exactly the stated identity.
   - A patched runtime package still refuses.
3. **An exact environment is preferred.** When any candidate for a specifier
   reproduces its whole environment, only those candidates go on to case
   selection. Otherwise every candidate admitted on its runtime does, and
   ADR 0187's same-set rule and `agreed_admissions` decide among them as
   before. Several identities with different claims are still refused.
4. **Project catalogs** (`EnvironmentRule::Exact`) keep the full rule.

## Consequences

- A tier bundle now reaches any project that installs the same package bytes
  on the same Solid runtime, whatever else it installs.
- **Trust weakens as ADR 0189 states.** A claim proven with one version of a
  non-runtime dependency is applied with another. A contract composed from a
  dependency's contract is still bound by its citations (ADR 0151), which are
  checked unchanged.
- `admission_refusals`, the diagnostic replay, still reports the first
  difference of the whole environment. It runs only for an acceptance that was
  not admitted, so it never contradicts an admission.
- **The runtime closure.** The runtime's own dependencies are kept with it: an
  entry looked up by a runtime package, transitively, stays required. A
  patched or different `leaf-dep` under `@solidjs/signals` still refuses. An
  environment stated without edges cannot say which package read an entry, so
  it keeps the exact rule; 715 of the 1,876 bundles state edges.

## Evidence

- **Unit test** `the_tier_needs_only_the_solid_runtime_of_its_environment`:
  - a different non-runtime dependency is refused by `Exact` and admitted by
    `SolidRuntime`;
  - another Solid runtime, or another version of a runtime dependency, is
    refused by both;
  - an environment without edges keeps the exact rule;
  - with two candidates, the one whose whole environment is installed wins.

  The patch tests (`a_pnpm_patch_of_an_environment_package_refuses_…`,
  `bun_and_patch_package_patches_refuse_…`) failed against a first version that
  kept only the runtime entries themselves, which is why the closure is kept.
  All 41 `accepted_bundles` tests pass.
- **38-app browser sweep** (release binary, against ADR 0190's
  `a1c-browser.json`): no change. Violations stay at 273 and uncertifiable at
  8,297; one result moved by seven columns.
- **Misuse ledger:** unchanged at 79 of 123, with no twin moved.

### Why the corpus does not move

Of the 186 versions of tier packages installed in the corpus, 115 are a
version the tier carries. But the apps run three Solid runtimes (rc.3, rc.4
and rc.9). The pairs of package version and runtime they install are, in
practice, the ones they were certified in place on, which already matched
exactly. The Solid runtime, not the rest of the tree, is what limits the
tier's reach. A contract proven on one Solid release says nothing about
another without a check, so the authored contracts of ADR 0189 will need a
probe run per audited Solid release (`rust/target/audited-archives` holds
rc.3, rc.6 and rc.9).

The change matters for projects outside the corpus, which install a version
the tier carries, on a runtime it was proven on, with other dependencies.
