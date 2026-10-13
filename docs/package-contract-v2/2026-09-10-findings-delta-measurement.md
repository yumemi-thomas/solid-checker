# Findings delta: what package contracts change in a consumer's verdict

- **Status:** measurement. No production code, fixture, snapshot, contract or
  receipt was changed by this pass.
- **Date:** 2026-09-10.
- **Question:** for a project that consumes a certified package, how many
  findings does the accepted catalog add, remove, or change, and which claim
  domain is responsible for each?
- **Basis:** working tree at HEAD `6b687543`, checker
  `rust/target/debug/solid-checker-rust` (built 2026-09-09 10:33), producer
  `bin/solid-typefacts` of the same build. Retained inputs:
  [delta JSON](phase21/2026-09-10-findings-delta-measurement.json),
  [harness](phase21/2026-09-10-findings-delta-harness.mjs).

## Answer

> **Superseded in part by §10 (2026-09-10).** The "strictly worse" claim below
> compares against a baseline that only occurs for packages which do not declare
> Solid. Read §10 before acting on this section.

**Package contracts currently make consumer verdicts strictly worse.** Across
every fixture project carrying an accepted catalog, plus one real policy-2
certified contract over real published bytes, enabling contracts added 17
findings, removed 0, enabled 0 violations, and moved 14 of 16 projects from
`certified` to `uncertifiable`. Every added finding was
`package-contract-incomplete` (SC9005), kind `uncertifiable`.

This is not a property of the sample. It is a corollary of the SC9005
predicate and the corpus closure table, and it holds for all 8950 exports —
see §3.

## 1. Method

`analyze` runs the checker twice per project: once as the tree stands, once
with `.solid-checker/accepted-contracts.json` removed from a copy. Findings
are keyed on `(id, rule, kind, file:line:column)` and diffed. Nothing in the
repository is mutated; both runs read copies under the session scratchpad.
Contract discovery is `<package root>/.solid-checker/accepted-contracts.json`
(`main.rs:2884`), so deleting that file is exactly "contracts off".

Two corpora:

- **Fixture corpus.** The 20 fixture trees carrying an accepted catalog; 4
  have no `tsconfig.json` and were skipped, leaving 16 analyzable projects.
- **Real certification.** `/private/tmp/solid-checker-rc7-reproduction`, a
  retained run holding an installed consumer project and a **policy-2**
  accepted catalog with a valid receipt over the published `seroval` artifact.
  Run in place, contracts supplied by `--accepted-contracts=` and
  `--receipt-trust-configuration=` rather than by deletion.

## 2. Result

> **Baseline corrected in §10.** The `certified` column is a fixture artifact:
> these packages declare no Solid dependency, so no contract obligation is raised
> for them at all. A realistic package's no-contract baseline is already SC9005.

| | contracts off | contracts on |
| --- | --- | --- |
| projects `certified` | 14 | 0 |
| projects `uncertifiable` | 1 | 15 |
| projects `violation` | 1 | 1 |

Delta, all 16 fixture projects:

| | count |
| --- | --- |
| findings added | 17 |
| findings removed | **0** |
| violations added | **0** |
| violations removed | **0** |
| status transitions `certified` → `uncertifiable` | 14 |

Added findings by rule: `package-contract-incomplete` 15,
`v1/package-contract-incomplete` 2. Every one is kind `uncertifiable`.

**The fixture result is weak evidence on its own.** All 20 fixture catalogs
carry policy-1 receipts, so 15 of the 17 added findings are the obsolete-policy
rejection path (`obsolete-policy1-receipt: policy 1 cannot authorize analyzer
semantics`), not open claims. That says the fixture corpus is stale; it says
little about contract value.

**The real certification is the evidence that counts.** The `seroval` case is
policy 2, receipt-valid, over real published bytes:

~~~
off: certified,     0 findings
on:  uncertifiable, 2 findings
  SC9005 unknown-contract-claims:reactiveReads — "leaves reactiveReads unknown
    for imported export createPlugin"
  SC9005 unknown-contract-claims:callbacks     — "leaves callbacks unknown
    for imported export createPlugin"
~~~

A certified contract, correctly acquired and authenticated, converts a clean
verdict into two uncertifiables — because the domains the consumer's proof
needs are the ones certification does not close.

## 3. Why this generalizes to the whole corpus

> **One claim corrected in §10.** The `asyncBehavior` conjunct is unreachable on
> the *consumer* path, not dead in general.

`push_unknown_contract_claims`
([contracts.rs:859](../../rust/crates/solid-reactive-ir/src/contracts.rs)) emits
SC9005 unless every one of four conjuncts is satisfied: `reads`, `returns`,
`creates` (as `ownerRequirements`) and `throws` (as `asyncBehavior`).

**One of the four is inert.** `project_async_behavior`
([contracts.rs:656](../../rust/crates/solid-reactive-ir/src/contracts.rs))
inserts `ClaimDomain::Returns` when returns is not closed and never inserts
`Throws`; nothing else inserts it either, and the function always returns
`ContractClaim::Known`, so `async_behavior.is_open()` is never true. The
`asyncBehavior` conjunct therefore never fires. This is the vestigial plumbing
the [demand proposal](2026-09-09-rule-demand-proposal.md) § 1.1 named — it
checks `Throws` while `project_async_behavior` reads returns — confirmed here
as dead rather than merely misleading.

The effective predicate is three conjuncts: **`reads` ∧ `returns` ∧ `creates`**.
Corpus closure, from [accuracy-roadmap.md](accuracy-roadmap.md) § "What accuracy
is here":

| conjunct | exports closed / 8950 |
| --- | --- |
| `reads` | **0** |
| `returns` | 44 |
| `creates` | 244 |

`reads` alone settles it: closed for zero exports corpus-wide, so the
conjunction is false for **every export of every certified package** and no
import of a certified package can avoid SC9005. The two measured corpora are
consistent with that, and the corollary does not depend on them.

`exportsProven` being 0 of 8950 and this result are the same fact seen from
the two ends of the pipeline.

## 4. What this does and does not license

**It does not say contract facts are worthless.** The demand proposal's 664
positive operation occurrences (616 projectable) are real inputs, and a closed
`creates` still terminates a dependency composition. What is measured here is
that no *consumer verdict* in the available corpora improves, and that the
uncertifiable SC9005 accompanies every certified import regardless.

**It does not measure code shapes absent from the corpora.** 16 synthetic
fixtures and one real consumer are a thin basis for the positive direction; a
contract could enable a violation in a shape nothing here exercises. The
negative direction — SC9005 on every certified import — is settled by §3 and
needs no further corpus.

**One documented suppression path is untested.** The demand matrix records
that B-membership alone suppresses the `reactive-source-uncaptured`
obligation. The only catalog-bearing fixture carrying that rule is
`reactive-ir/v1-reactivity`, whose catalog is policy-1 rejected, so membership
is never established and the path did not run. Measured removals there: 0.

## 5. Consequences for the roadmap

> **Consequence 1 withdrawn in §10.** Partial closure already retains what its
> closed domains prove; there is now a test pinning it.

The claim-domain effort ordering is inverted with respect to this predicate.
Thirteen ADRs (0038–0050) closed `creates` candidates. `creates` is one of the
four conjuncts, so closing it cannot silence SC9005 on its own while `reads`
and `throws` are at zero — and it is the conjunct with the fewest rule
consumers in the demand matrix.

Two consequences follow directly, both stated as questions for an ADR rather
than as decisions taken here:

1. **SC9005's conjunction is probably too strong.** It demands four domains
   for every import, whatever the importing code does. A per-call-site demand
   — raise the obligation only for the domains the consumer's own proof reads
   — would let a partially closed contract be worth acquiring. This is the
   demand proposal's argument arriving at a specific, testable predicate.
2. **`reads` and `throws` at zero gate everything else.** Under the current
   predicate, no work on any other domain changes a single consumer verdict.

## 6. Reproducing

~~~sh
export REPO="$PWD" SCRATCH=/tmp/findings-delta
export PROJECTS="$(find fixtures -name accepted-contracts.json -path '*.solid-checker*' | sort)"
mkdir -p "$SCRATCH" && bun docs/package-contract-v2/phase21/2026-09-10-findings-delta-harness.mjs
~~~

The harness needs `rust/target/debug/solid-checker-rust` and
`bin/solid-typefacts`; a stale checker invalidates the result the usual way.

## 7. Addendum (2026-09-10): the fixture packages cannot be re-certified

The natural follow-up to §2 — re-certify the fixture catalogs under policy 2
and see how many of the 23 pre-cut violations return — **is not possible**, and
the reason is structural rather than a matter of effort.

**What the pre-cut state was.** Commit `662dd7ba` ("Cut package contracts to
proof policy 2", 2026-08-30) did not migrate the fixture receipts. It replaced
`"receipt": "node_modules/reactive-package/solid-reactivity.receipt.json"` with
`"status": "obsolete-policy1"` in every fixture catalog and deleted the receipt
files. Before that commit the same fixtures produced 23 violations across 9
rules, all of them third-party reactive misuse:

| rule | violations |
| --- | --- |
| `strict-read-untracked` (+ `v1/`) | 14 |
| `v1/no-direct-mutation` | 3 |
| `missing-owner`, `no-destructure`, `prefer-for`, `v1/uncalled-accessor`, `v1/reactive-read-after-await`, `v1/reactive-handler-frozen` | 1 each |

**Why they cannot come back.** Policy-2 certification is defined over an
acquired *published* archive. `contract certify` requires `--integrity <SRI>`
and resolves the package from a registry origin; `SOLID_CHECKER_REGISTRY_CACHE`
caches bytes for an exact `(origin, package, version, integrity)` but is not an
offline mode. The fixture packages are fabricated — `reactive-package@1.0.0`
exists only inside each fixture's `node_modules`. Measured, on
`fixtures/reactive-ir/package-return-consumer`:

~~~
$ solid-checker contract certify --package-root=<fixture>/node_modules/reactive-package
solid-checker: --integrity is required; certification cannot choose package
bytes from registry metadata alone

$ solid-checker contract certify --package-root=… --integrity=sha512-<locally computed>
solid-checker: artifact-or-demand-planning refused: registry metadata
acquisition returned HTTP 404
~~~

A second, independent blocker sits behind the first: these packages carry no
runtime bytes. `reactive-package` is `package.json` + `index.d.ts` +
the hand-authored `solid-reactivity.json`, with no `main`, no `exports`, and no
module. A policy-2 receipt binds 17 witness roots plus a mandatory
`probeGateRoot` over executed runtime bytes; there is nothing here to execute.

**Nor would a real package restore them.** Substituting a published package
does not help, because §3 applies: `reads` is closed for 0 of 8950 exports, so
any real certified contract raises SC9005 regardless of how the consumer uses
it.

**Status of the fixtures.** The obsolete state is *pinned as expected* rather
than tracked as debt: the backend's own
`tests/fixtures/contract-async/.solid-checker/accepted-contracts.json` is also
`obsolete-policy1`, and `contracts_process.rs:322` asserts the
`obsolete-policy1-receipt:` analysis context. So the 23 violations are not a
regression the gates will ever report — the checker's ability to diagnose
third-party reactive misuse currently has **no live test anywhere in the
repository**, and no supported path to acquire one.

**What this implies.** The consumer machinery is intact and current —
`project_callbacks`, `project_reactive_reads`, `project_return`,
`project_owner_requirements`, `project_async_behavior` all feed
`push_contracted_return_source` and `contracted_accessor_symbols` in
`source_discovery.rs`. Nothing needs building in the rules.

## 8. Addendum (2026-09-10): the issuer is not the blocker; provenance is

An earlier revision of §7 recommended "a test-scoped issuer". That was the
wrong diagnosis. Signing was never the obstacle:
`ConfiguredReceiptIssuer::persistent_local(scope, seed)` already exists
([policy2_receipt.rs:181](../../rust/crates/solid-facts-backend/src/contract_certification/policy2_receipt.rs))
and the retained rc7 run used it to issue a valid policy-2 receipt under scope
`rc7-demand-audit`. What refuses a fixture package is **artifact acquisition
provenance**.

### The refusal is deliberate and test-pinned

`SnapshotProvenance` has exactly two accepted variants, `Published` and
`LockPinned`. A third input type exists for local trees and is refused
outright:

~~~rust
// Workspace/link acquisition has a separate input type so it cannot acquire
// registry provenance by filling in name/version/integrity fields. Semantic
// model 1 has no accepted local provenance identity, so policy 2 currently
// refuses this input explicitly.
pub struct LocalArtifact { root: String }

pub fn from_local(artifact: &LocalArtifact, _limits: SnapshotLimits)
    -> Result<Self, ArtifactSnapshotError> {
    Err(ArtifactSnapshotError::UnsupportedProvenance(format!(
        "policy 2 semantic model 1 cannot certify local artifact {}", artifact.root)))
}
~~~

`contract_certification.rs:4137` asserts that refusal. Making `from_local`
succeed would reverse a recorded decision and break its test; it is not the
path.

### `LockPinned` is the sanctioned seam, and it already works

`LockPinnedArchive` takes the archive **bytes** plus package manager, lockfile
digest, locator, name, version and integrity — no registry contact — and
`ArtifactSnapshot::from_lock_pinned` is implemented and exercised
(`contract_certification.rs:4119`). Its own comment states the property that
makes it correct here: *"a lock selection proves pinned bytes, not that a
registry currently publishes those bytes."* The existing test asserts that
published and lock-pinned snapshots of the same archive share a `snapshot_root`
but differ in `provenance_root`, so a lock-pinned receipt is honestly
distinguishable from a published one.

A fixture package packed into a tarball with a locally computed SHA-512 is
therefore certifiable under genuine policy 2 — real bytes, real closure, real
probe gate, real signature — with no weakening of the trust model.

**It is not reachable from either boundary today.** `certify-contract.mjs`
only ever builds registry acquisition, and
`certification_plan_from_request_in` (`main.rs:737`) hardcodes
`PublishedArchive::new(...)` wrapped as
`UntrustedArtifactEnvelope::Published`. The planning request carries
`registry_origin`, `registry_metadata` and `archive`, so reaching `LockPinned`
is a change at the certify process boundary, not only in the CLI.

### A receipt cannot be committed

`verify_configured`
([policy2_receipt.rs:809](../../rust/crates/solid-facts-backend/src/contract_certification/policy2_receipt.rs))
requires `entry.allowed_verifier_builds.contains(payload.verifierBuildDigest)`.
A receipt is bound to the verifier build that issued it, so a checked-in
fixture receipt is rejected by the next checker rebuild. Fixture receipts must
be **minted at gate time by the current build**, alongside a trust
configuration the gate supplies out of band — never referenced by the fixture
catalog, since "doing so would let an analyzed project nominate its own
issuer" (`contract_interface.rs:461`).

### The two shapes this can take

1. **Gate-time fixture certification, test-scoped.** A test or gate step packs
   each fixture package, drives real planning and certification through
   `LockPinnedArchive`, signs with a `persistent_local` issuer, writes catalog
   plus trust configuration into a temporary copy, and runs the checker against
   it with `--receipt-trust-configuration`. Nothing is committed but fixture
   sources and expectations. No protocol change, no production surface.
2. **Lock-pinned acquisition as a product feature.** Expose the same seam
   through `contract certify`, so installed-but-unpublished packages
   (workspaces, vendored trees, prereleases pulled from a lockfile) can be
   certified. Requires a certify-boundary protocol change, and expands what
   the shipped CLI will certify.

These are not the same deliverable. (1) restores the regression test this
document says is missing; (2) is a capability decision about the product.

## 9. Landed (2026-09-10): a test-scoped issuer, and the regression test it restores

`contracts_process::contract_closure_process::a_closed_contract_lets_the_rules_diagnose_third_party_reactive_misuse`
now covers the gap §7 identified. It is inside the `contracts_process` target,
so the existing `contract-process` row of the check table already runs it; no
new test binary and no mapping change.

**What it does.** Copies `fixtures/reactive-ir/package-return-consumer` to a
temporary tree, reuses the catalog's existing `import` block verbatim as the
resolver answer, mints a policy-2 receipt over the fixture's contract document
with `ConfiguredReceiptIssuer::persistent_local("solid-checker-fixture", …)`,
publishes the catalog through `publish_policy2_catalog`, hands the checker the
trust configuration out of band via `--receipt-trust-configuration`, and
asserts the outcome: **no `package-contract-incomplete`, and exactly one
`strict-read-untracked` violation** — which is precisely what that fixture
produced before the 2026-08-30 cut.

**Why it is not vacuous.** The two assertions bite from opposite sides, and §2
measured both failure modes on this same fixture: with no catalog the checker
returns `certified` with zero findings (the accessor is not known to be
reactive, so the second assertion fails), and with the obsolete policy-1
catalog it returns SC9005 (the first assertion fails). Only an authenticated,
closed contract satisfies both.

**Why it does not weaken the trust model.**

- The issuer kind is the existing `PersistentLocal`, not a new one. Nothing in
  the acquisition path changed: `ArtifactSnapshot::from_local` still refuses,
  and its test still pins that refusal.
- The trust configuration is written by the test and passed on the command
  line. A project still cannot nominate its own issuer.
- The key exists for the duration of one test. No receipt, key or trust
  document is committed.
- The receipt cannot overstate what it carries: the consumer rebinds
  `closedClaimsRoot` from the document, so an issuer cannot assert a closure
  the contract does not have. This is what the first iteration got wrong —
  an arbitrary root was refused with `does not bind the selected contract:
  closedClaimsRoot`.

**Two small public additions**, both shape-only and both needed by any
out-of-crate issuer:

- `RECEIPT_WITNESS_FAMILIES` — so a complete `Policy2ReceiptBindings` can be
  built without duplicating the 17 family names.
- `policy2_main_closed_claims_root(canonical_main)` — the sibling of
  `policy2_main_semantic_digest`, recomputing the root the consumer will
  rebind, without reaching into the semantic model.

**What it still does not prove.** Only the consumer half. Certification cannot
currently produce a contract like this one for a real package: §3 stands, and
`reads` is closed for 0 of 8950 corpus exports. The test's value is that the
next closure lever now has a live, failing-if-broken target to aim at — and
that the product's central use case is no longer untested.

**Not attempted, deliberately.** The other 19 fixture catalogs remain
`obsolete-policy1`; restoring all 23 pre-cut violations would couple coverage
to certification and the probe harness, and was deferred as its own decision.

## 10. Correction (2026-09-10): the predicate was largely right

Acting on §5's first consequence — "SC9005's conjunction is probably too
strong" — meant first establishing that it *was* too strong. It is not. Three
claims in this document were wrong, and the measurements that correct them are
below.

### 10.1 The `certified` baseline was a fixture artifact

`external_package_contract_requirements`
([package_requirements.rs:47](../../rust/crates/solid-facts-backend/src/package_requirements.rs))
raises a contract obligation only for an import whose package manifest
`manifest_uses_solid`. Every fixture package here is a bare stub
(`{name, version, types}`), so with its catalog removed it is not merely
uncertified — it is **out of scope**, and the checker answers `certified`
because it was never asked. Adding the contract is what brought it into scope.

Measured on `package-return-consumer`, the same misuse, the same checker:

| package manifest | catalog | verdict |
| --- | --- | --- |
| no Solid dependency | none | `certified`, 0 findings |
| no Solid dependency | obsolete policy-1 | `uncertifiable`, 1 SC9005 |
| **declares `solid-js` peer** | **none** | **`uncertifiable`, 1 SC9005** (`no receipt-accepted contract matches this exact import`) |
| declares `solid-js` peer | authenticated, fully closed | `certified`-scope, **1 `strict-read-untracked` violation, 0 SC9005** |

So for a package the checker actually demands a contract for, acquiring one
does **not** make the verdict worse. The honest statement is that today it
usually does not make it *better* either: one SC9005 is traded for another
until every demanded domain closes. That is a much weaker indictment than §2's,
and it points at closure rather than at the predicate.

### 10.2 Partial closure already pays

§5 assumed an open sibling domain discards the closed ones. It does not.
`push_unknown_contract_claims` *adds* an obligation; it never suppresses the
binding, and the projection below it keeps every verified positive item.

Pinned by
`contracts_process::contract_closure_process::a_partially_closed_contract_still_proves_what_its_closed_domains_prove`:
reopening `returns` on the fixture's own document — leaving the accessor item
in place while making the collection non-exhaustive — still yields the
`strict-read-untracked` violation, and the obligation names exactly
`unknown-contract-claims:returns`.

Worth recording for anyone writing such a case: a domain the document states as
an **empty** collection cannot be reopened at all. The validator refuses with
"open domain call operation claim has an empty collection", so partial
knowledge always means "these items, and maybe more" — never "nothing known,
maybe something".

### 10.3 The `asyncBehavior` conjunct is not dead in general

§3 said it never fires. It cannot fire on the *consumer* path, where every
summary comes from `project_accepted_export` (always `Known`) or the local
builder at `contracts.rs:1516` (also always `Known`). But
`ContractExport::unknown_runtime_kind()`
([lib.rs:1060](../../rust/crates/solid-reactive-ir/src/lib.rs)) constructs the
field genuinely open, and `main.rs:7276` reaches it for
`ExportKindProof::Unresolvable`. Deleting the guard would have removed a
fail-closed answer.

### What actually changed

One disjunct, in `push_unknown_contract_claims`:

~~~diff
-    if summary.async_behavior.is_open()
-        || summary.open_claims.contains(&ClaimDomain::Throws)
-    {
+    if summary.async_behavior.is_open() {
~~~

`project_async_behavior` derives this field from **returns** and inserts
`ClaimDomain::Returns`; nothing inserts `Throws`. The disjunct was both
unreachable and a claim about the wrong domain, and returns is already the
conjunct above it. The `is_open()` guard stays, for §10.3's reason. No finding
moves: coverage compares 94 projects and 547 findings unchanged.

### What is left, and it is not the predicate

`reads` closed for 0 of 8950 exports is still the whole problem, and §5's
second consequence stands unchanged: under any predicate, a domain that never
closes never stops being reported. The lever is a `reads` census (roadmap lever
F), not the shape of this check.

## 11. Addendum (2026-09-10): one domain measurably left a real consumer's message

§2's `seroval` row is the only real certification here, and it left
`createPlugin` open on `reactiveReads` and `callbacks`. The same retained tree
also carried a *withheld* candidate — `createReference`'s `returns`, held back
only because no probe recipe addressed its claim. Authoring that recipe closes
the domain, and a consumer that binds and reads the result goes from

~~~
SC9005 … leaves reactiveReads,returns unknown for imported export createReference
~~~

to

~~~
SC9005 … leaves reactiveReads unknown for imported export createReference
~~~

with the corpus as the only variable. Full method, falsifying control and
caveats: [the returns-recipe measurement](phase21/2026-09-10-real-package-returns-recipe.md).

This does not overturn §2 — the verdict is still `uncertifiable` and no
violation was enabled. It sharpens it. On that export every other domain is
now proved (`creates` by census, `returns` by census plus an executed veto, and
it takes no callback), so the single remaining open domain is `reads`, exactly
as §3 and §10's closing paragraph predicted. The corollary is worth stating in
the positive direction for once: **the pipeline does what it claims; `reads` is
the only thing standing between it and a clean verdict on a real package.**
