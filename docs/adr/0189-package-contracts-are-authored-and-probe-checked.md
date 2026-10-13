# ADR 0189: Package contracts are authored and probe-checked

- Status: accepted (owner, 2026-10-05); not yet implemented. Implementation
  is tracked in [the package direction of 2026-10-05](../2026-10-05-package-direction.md).
- Owners:
  - a new authored tier, read beside the compiled-in accepted tier
    (`accepted_bundles.rs`) and folded into the same `AcceptedContractIndex`
    (`solid-reactive-ir/src/contract_semantics/consumer.rs`);
  - a gate that runs every authored claim's probe test against the published
    package;
  - the `SC9005` wording (`dialect.rs`, `contracts.rs`).
- Relation: replaces certification as the source of package knowledge. It
  supersedes the environment-keyed admission of ADRs 0123 and 0151 and the
  in-place certification of ADRs 0184 to 0188 once Track C of the direction
  lands. The contract vocabulary (`ExportSemantics`) and the runtime probe
  harness are kept.

## Context

A package contract today counts only when it is certified:

1. the generator analyzes the package code and proposes claims;
2. a census and a runtime veto check them;
3. the result is signed into a receipt for one exact dependency environment;
4. a consumer admits it only if its own installed tree reproduces that
   environment, the installed bytes and the acceptance root.

Measured on 2026-10-04 and 2026-10-05, over the 38-app corpus:

- **Analysis is the limit, not the ceremony.** Signing the generator's raw
  proposals instead of certified contracts moved no violation. Of 2,052
  exported dependency callables, 76% propose nothing closed, and 6% close
  every domain.
- **Exact-environment keying reaches almost no one.** The census tier matched
  no app install until ADR 0186 bundled the apps' own in-place certifications.
  Those bundles apply only to the same dependency tree.
- **The yield is small.** ADRs 0184 to 0188 took in-place certification from
  90 to 111 of 180 packages. They produced 10 package findings, all true
  positives, in two apps.

The compiled-in tier already rests on repository review: its receipts are
`ReceiptIssuerKind::BuiltIn`, authenticated against a digest compiled into the
same binary (`accepted_bundles.rs`, module comment). What certification adds
on top of that review is applicability to one environment, and inferred claims
that are mostly empty.

The owner will author contracts and fix them when users report issues. Package
authors must carry no burden.

## Decision

1. **Authored contracts are trusted because they ship in the checker.** The
   authority is repository review, as for the compiled-in tier today. No
   receipt, signature or trust configuration is involved.
2. **One contract per package and an explicit list of versions.** No semver
   ranges: each listed version has been probe-checked. Each version entry
   records:
   - the published integrity;
   - the snapshot root of the published files.

   A version that is not listed has no contract, and the import gets the
   ordinary uncertifiable result.
3. **Applicability is checked against the install, without the dependency
   tree.**
   - The installed name and version select the entry.
   - The installed files must reproduce its snapshot root, and nothing may be
     patched (`installed_package_snapshot_root`, `installed_patches.rs`).
   - Where a lockfile records an integrity, it must equal the entry's.

   The rest of the installed dependency environment is not compared. The Solid
   runtime is the exception: the probe runs on the audited Solid release, and a
   project on another release keeps today's `unaudited-solid-release` notice.
4. **A contract states claims, not domains.**
   - It uses the existing `ExportSemantics` vocabulary.
   - A claim the file does not state stays open, so code whose proof depends on
     it is uncertifiable, exactly as with today's partial contracts.
   - Nothing is ever inferred as closed because the file is silent about it.
5. **Every claim has a probe test.** Each stated claim names a probe case, run
   against the real published package at every listed version. The gate fails
   if:
   - a claim has no case;
   - a case fails;
   - a listed version is not installable with its recorded integrity.

   The gate is part of `make verify`.
6. **A finding names its contract.** A violation whose proof uses an authored
   claim names the package, the version and the contract file, so a report
   points at the claim to fix.
7. **Order of tiers.** The authored tier is consulted before the certified
   tier while both exist. A package with both, at the same version, uses the
   authored entry.

## Consequences

- Contracts reach every project that installs a listed version. They no longer
  depend on the rest of the tree being identical.
- The trust weakens in one way: a claim is proven for the audited Solid
  release and the package's published bytes, not for each dependency version
  it may resolve. A claim that depends on another package's behavior must be
  probed with that package at the versions the listed release allows, or
  stay open.
- A wrong authored claim can produce a false positive. The probe test is the
  defense: a claim is only as strong as its case, so cases must exercise the
  behavior the claim asserts (for a "returns a reactive store" claim, a read
  of the returned value outside a tracking scope must warn under the audited
  runtime).
- `AcceptedContract` can today be built only by `proof::verify_and_accept`. The
  authored tier needs its own constructor.
- New package releases have no contract until they are probe-checked and
  listed. A scheduled job that probes new releases against the existing
  contract and proposes the list update is a later step.
- Certification, the generator as an authority, project catalogs, receipts,
  trust configuration and environment admission are retired after the pilot
  meets its exit criterion (Track C of the direction). The existing certified
  contracts are converted into authored entries first, so no finding is lost.

## Evidence required before implementation is called done

- **Pilot:** authored contracts for TanStack router and query, `@solidjs/router`
  and `@solidjs/meta`, with the certified tier off (`--no-bundled-contracts`).
  The 38-app sweep must still report the 10 package findings of ADR 0186, with
  no violation that is not a reviewed true positive.
- **Fixtures** for each applicability rule:
  - a listed version is admitted;
  - an unlisted version is uncertifiable;
  - patched files, or another integrity, are refused;
  - a silent claim stays open.
- **Gate fixtures:** a claim with no probe case fails the gate, and so does a
  failing case.
