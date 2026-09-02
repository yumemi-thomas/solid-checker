# Phase 20 plan — unblock every currently actionable ecosystem row

Date: 2026-08-30

Status: complete. The final corpus verifies 24 exact rows through fresh
ordinary discovery, gives all remaining complete candidates exact local
refusals, and records complete exact-leaf dependency plans for every currently
affected dependency row. Unsupported artifact and dependency shapes remain
fail-closed; completion does not mean that all 418 rows are certifiable.

Baseline commit: 870dcceb83d4aa72cb3671e4c41e503c5df018e4

Baseline report: benchmarks/ecosystem/report.json

Baseline report SHA-256:
45a9dd28f6360ba9438d69d6153b99a01bdd8801dd6041f4ba230bf1b4495c15

Slice 0 remeasurement report SHA-256:
fead590bff9878c945814140e1bf2fa4ca7d43a12eec8125a39f6d3b34559e47

Current full-corpus report SHA-256:
dfd72fa5d7e8108abdf840d0edfbb9d89cfd83df06ad03cbde2163c9e23894f2

Final Phase 20 ledger SHA-256:
1cf22736be8a71205f59cd5cd1ec02f6be0dd1c977552a89cc645b0ce8b72107

## Final implementation checkpoint — 2026-08-30

The authoritative 418-row run completed in 118,810 ms with 63 complete
proposals, 325 partial proposals, and 30 fully refused rows. All 63 complete
rows attempted policy-2 certification: 24 independently published, loaded,
authenticated, selected, and queried receipts; the other 39 retain exact local
refusals. No verified row depends on an omitted artifact case.

The final live failure ledger is 16 dependency-contract obligations, seven
no-exported-surface rows, six unclassified package/artifact failures, and one
geolocation export-kind conflict. Export-kind-unresolved is zero. All 24 rows
that still expose external dependency edges—16 fully refused and eight
partial—carry complete recursive plans: 570 exact package/artifact nodes,
4,294 edges, and 358 exact terminal leaves or source-bound module-loading
frontiers. These plans do not authenticate dependency receipts and therefore
cannot turn a parent row green; they name the exact remaining leaf instead.

Artifact applicability is now emitted by the proposal producer as structured
data rather than reconstructed from refusal prose. The final census is 1,793
runtime cases, 378 unavailable published targets, 194 unsupported artifact
shapes, and four unsupported condition environments. No case was classified
as verifier-proved type-only in this corpus.

Generation and certification share eight outer workers. Once complete rows
exist, at most two of those workers drain certification while the remaining
workers continue generation. Recursive dependency planning walks exact static
ESM closure without a checker program and stops CommonJS loads at exact
byte-offset-bound `require` frontiers. This keeps the full generation,
certification, and dependency-disposition transaction below the permanent
120,000 ms gate without increasing process concurrency.

Slice 0 retained the 44/314/60 proposal baseline while closing the reporting
evidence gaps: all 60 full-refusal rows now carry complete structured refusal
audits, all 358 generated rows carry accepted artifact-case identities, and
all 55 discovered external edges carry exact installed package versions. The
live classifier records geolocation as export-kind-conflict directly. The
generated authority is
benchmarks/package-contract-v2/phase20/row-ledger.json.

## Implementation checkpoint — 2026-08-30

Slice 1 now fails closed at the live Type Facts boundary for the reachable
cross-subject attacks: exact subpath, callable-path, export-value versus call
result, operation/ordinal/guard/recursive subject, unknown callability,
default-export alias, mutable external declaration, and caller-authored demand
assignment. The planner now always emits rest/spread demands for selected
calls, compiler reconciliation for transformed artifacts, and a base accepted
dependency-composition demand for every exact external dependency. This was
the pre-adapter checkpoint. The current transaction keeps the opaque plan
alive, acquires live export-value evidence, signs the complete policy-2
binding set, publishes atomically, and validates it in a separate analyzer
process. Pruning and normal-discovery tracers now pass for the supported
single-case value-only surface.

Slice 2 now constructs each inferred contract under the exact requested
entrypoint and validates that identity at the Rust seam. Proposal plans are
validated against their own normalized source document and then rebound into
the merged normalized contract; an elided subject is dropped explicitly rather
than attached to a neighboring operation.

Slices 6–9 now retain an opaque plan for every artifact case, materialize one
private package snapshot, acquire independently bound evidence in one pinned
Type Facts session, finalize and issue one receipt per case, and stage the
complete case census before anything becomes visible. The case-set document is
canonical and content-addressed. A separate fresh analyzer process reconstructs
each plan from the authenticated acquisition inputs, binds each artifact-case
ID to its exact resolved-import root, ordinary-loads and authenticates every
single-case catalog, and checks the complete census. Publication then
re-authenticates the committed content-addressed directory before atomically
moving the public pointer. Omission, duplication, case/root transplantation,
importer replay, POSIX and Windows traversal, catalog and pointer mutation,
reordered inputs, and failure before pointer visibility have focused negative
tests.

Two fresh, exact-probe measurements used release checker digest
`080d7bb5308e710a0b36953b3a14d11522343734a24a8141b12f8746777383b9`:

- All 57 rows assigned to the baseline `exact-proposal-identity` cohort lost
  the old `inference has no entrypoint` refusal. Seven now have complete
  proposals and 50 have partial proposals; none is fully refused. Four of the
  seven stop at the intentionally unavailable Type Facts witness adapter, and
  three expose a canonical closure-replay mismatch owned by artifact
  provenance.
- The four stale-subject probes all lost the stale-plan refusal.
  `@solid-primitives/debounce@1.3.0|solid1|only` now has a complete proposal and
  stops at the Type Facts adapter; the other three retain unrelated artifact
  refusals.

The compact evidence is
`benchmarks/package-contract-v2/phase20/slice2-remeasurement-summary.json`.
These affected-cohort measurements were subsequently confirmed by the full
418-row remeasurement below. That later run supersedes this zero-receipt
checkpoint.

## Decision

The next phase was not merely “turn on the witness adapters.” The opaque native
transaction and ordinary-discovery tracer are now live for the bounded
single-case value-only surface, while every other family still fails closed at
its exact unsupported premise.

Phase 20 will build one deep native certification transaction, repair every
bounded checker defect found in the current 418-row corpus, and remeasure each
row against an explicit disposition ledger. A row may become green only after
ordinary analyzer discovery loads an authenticated policy-2 receipt for every
supported artifact case and exact exported surface in that row.

The ledger uses three orthogonal axes rather than overlapping outcome labels:

1. proposal progress — complete, partial, or fully refused;
2. certification terminal state — not attempted, exact refusal, or verified
   through ordinary receipt loading; and
3. per-artifact-case applicability — runtime module, verifier-proved
   type-only, unavailable published target, unsupported
   condition/environment, or unsupported artifact shape; a row may contain
   several of these and derives its aggregate from the complete case set.

A structurally complete proposal with an open Type Facts premise is therefore
proposal-complete and certification-refused. “Verified” is reserved for the
receipt-loaded certification state; it is never inferred from proposal
progress or applicability accounting.

Changing a label, ignoring a package condition, treating unknown as a value, or
dropping an unsupported case from the denominator does not verify a row.

## What “verify a row” means

A benchmark row is verified only when all of these statements are true:

- the package name, version, registry origin, metadata, archive bytes, archive
  member census, requested entrypoint, condition set, runtime target, and
  declaration target are authenticated;
- every supported artifact case is represented, not merely the first case that
  generated successfully;
- every runtime export has exact runtime/declaration identity;
- every positive semantic claim has a complete, subject-matched witness;
- every retained closure claim has a complete negative census;
- every consulted source is bound as verifier-authored query input, root
  archive member, authenticated dependency member, or producer-bundled library;
- the candidate was finalized, normalized, inventoried, and replanned after any
  verifier-owned pruning;
- the issuer signed the final main document, proof-policy digest, complete
  coverage set, demand graph, source/provenance roots, and producer identities;
- the receipt was authenticated and loaded through the same catalog discovery
  path used by an ordinary analysis; and
- a fresh analysis query selected the accepted exact importer/specifier case.

A genuinely type-only export-map leaf may be classified as verifier-derived
runtime-inapplicable. JSON, CSS, workers, CJS modules, custom loaders, and
missing files are not silently inapplicable; they remain outside the supported
surface until their artifact model exists or the package is repaired.

The TypeScript boundary remains absolute: if the real published typings make
the program invalid, the checker must not add a duplicate diagnostic. The
ecosystem ledger may record the package artifact as unavailable, but the
reactivity checker does not become a package-resolution linter.

## Superseded implementation checkpoint — 85-second pre-final run

This section is retained as optimization history. Its 52/324/42 and 16-row
counts were superseded by the final checkpoint above.

The hardened release report finished at 2026-08-30T06:36:53.855Z in 85,413 ms
and contains:

| Outcome | Rows | Meaning today |
| --- | ---: | --- |
| success | 52 | structurally complete proposal; certification state is separate |
| partial-success | 324 | proposal with one or more omitted cases |
| failure | 42 | no certifiable artifact case |
| verified receipt | 16 | authenticated and selected through fresh ordinary discovery |

All 52 structurally complete rows attempted certification. Sixteen verified:
11 Corvu/Corvu Next rows, `@solid-devtools/transform`, `@solidjs/html`, and
three TanStack rows. Every one is represented in the generated row ledger.
The remaining 36 rows have exact local refusals: 12 unresolved recursive value
leaves, 10 non-export operation demands outside the export-value transcript,
seven canonical closure-replay mismatches, six producer CBOR callability
decoding defects, and one unsafe duplicate archive member. All 36 rows that
previously stopped at the single-case finalizer were multi-case rows—not 33 as
the earlier checkpoint stated. Multi-case support certified 14 of them and
exposed the deeper exact premises above. The 324 partial rows retain 1,781
explicit refusals: 1,779 artifact-case refusals and two entrypoint-census
refusals.

The current 42 full failures are:

| Current class | Rows |
| --- | ---: |
| dependency-contract-obligation | 16 |
| export-kind-unresolved | 9 |
| no-exported-surface | 7 |
| export-kind-conflict | 1 |
| unclassified | 9 |

The live Phase 20 ledger is
`benchmarks/package-contract-v2/phase20/row-ledger.json`, with SHA-256
`d5392af1f287046c50f172af2fa429326b3b7e66427d3e1e4ab2f1b079ae9691`.

### Generation performance checkpoint

Proposal generation owns caches scoped to one exact package transaction. The
first reuses export bindings and canonical closure results only when the
logical package root, manifest digest, runtime and declaration root paths and
digests, and accepted dependency-contract bindings are identical. The second
reuses module descriptions across genuinely different root pairs while
checking the current file digest before an acquisition reuses a description.

The remaining bottleneck was one complete TypeScript `Program` per module.
The transaction now syntax-parses the exact package source census once and
builds one checker program for every checker-relevant external module in that
package. A non-module script that needs checker queries retains an isolated
program, so unrelated global scripts cannot satisfy each other's bindings.
A changed requested file digest rebuilds the package parser. Runtime and
declaration resolution traces are still constructed independently for every
condition case. No cache is global or persisted across package transactions,
and a changed transitive closure member invalidates the semantic acquisition.

The ecosystem runner reads the immediately preceding authoritative report as
advisory cost data, schedules the longest rows first, and restores canonical
manifest order before report construction. Missing or malformed historical
data falls back to manifest order and never changes a row result. The default
worker count is now 12 on the measured 14-CPU host, reserving two CPUs; hosts
with eight or fewer available CPUs retain their previous full parallelism.
Generation for all rows completes before certification begins, so Type Facts
processes cannot contend with parser/program construction. Certification has a
separate bounded pool. Phase one retains no project directories; phase two
reinstalls only the 52 complete rows from the warm package cache, rechecks exact
version and integrity, certifies, reads the audit, and cleans the project.

`SOLID_CHECKER_TIMINGS=1` now emits one structured timing record for a
generation transaction. The exact `solid-recharts@1.0.1` harness is pinned to
the package integrity and proposal semantic digest; its contract, proposal,
and refusal sidecar remained byte-identical while wall time fell from 62,538
ms to 2,948 ms. Its preparation phase fell from 59,488 ms to 696 ms.

The optimization reruns preserved every proposal classification,
accepted/refused case count, and contract-content measurement after timing
fields were removed from the comparison. The latest authoritative run executes
the real certification transaction for all 52 complete proposals, independently
reconstructs every multi-case plan in fresh processes, re-authenticates the
committed content-addressed case sets, and still completes in 85,413 ms. A
permanent replay test requires the checked full 418-row report to remain
strictly below 120,000 ms.

That run also exposed a teardown race: if a producer failed and its response
reader observed EOF immediately before session drop registered a close
request, close could wait forever. Graceful close is now bounded to 250 ms
before forced process cleanup. A process regression test kills the producer
between evidence and close and requires teardown in under one second. The
exact previously hanging `@tanstack/ai-solid` row now returns its typed local
CBOR refusal in 5,108 ms including reinstall and certification.

Relative to the immediately preceding full report:

| Cost | Before | After | Change |
| --- | ---: | ---: | ---: |
| corpus wall time | 231,032 ms | 85,413 ms | -63.0% |
| accumulated generation | 1,142,006 ms | 701,395 ms | -38.6% |
| accumulated worker time | 1,365,399 ms | 909,921 ms | -33.4% |
| generation p95 | 6,991 ms | 2,976 ms | -57.4% |
| slowest generation | 75,105 ms | 39,646 ms | -47.2% |

Across both optimization rounds, corpus wall time fell from 532,503 ms to
85,413 ms (-84.0%), accumulated generation from 3,011,708 ms to 701,395 ms
(-76.7%), and accumulated worker time from 3,293,252 ms to 909,921 ms (-72.4%).
The final generation median is 1,234 ms. The gain remains concentrated in
repeated cases, compiler setup, long-tail scheduling, higher safe parallelism,
and bounded failure teardown.

## Slice 0 baseline (historical)

The report finished at 2026-08-29T19:09:57.009Z and contains:

| Outcome | Rows | Meaning today |
| --- | ---: | --- |
| success | 44 | structurally complete, unaccepted proposal |
| partial-success | 314 | proposal with one or more omitted cases |
| failure | 60 | no certifiable artifact case |
| verified receipt | 0 | no row is currently verified |

The 314 partial rows contain 1,954 recorded refusal instances:

| Refusal stage | Instances |
| --- | ---: |
| artifact-case | 1,944 |
| entrypoint-census | 2 |
| proposal-merge | 8 |

All 44 structurally complete baseline rows attempted certification:

| First refusing owner | Rows | Recorded missing demands |
| --- | ---: | ---: |
| Type Facts | 39 | 1,456 |
| artifact provenance | 5 | not represented as demand counts |

The 1,456 Type Facts refusals are:

| Family | Demands |
| --- | ---: |
| recursive-value-shape | 703 |
| callable-path | 686 |
| operation-cardinality | 21 |
| operation-reachability | 21 |
| argument-binding | 13 |
| selected-signature | 12 |

All 44 baseline attempts have zero domain-exhaustiveness demands. The first
tracer therefore needs an authority-bound empty probe root, not a fake probe
harness. A nonempty probe schedule must continue to refuse until the pinned
harness and runtime-image binding exist.

The 60 Slice 0 full failures were exactly:

| Current class | Rows |
| --- | ---: |
| dependency-contract-obligation | 21 |
| export-kind-unresolved | 15 |
| no-exported-surface | 9 |
| package-contract-export-missing | 3 |
| unresolved-parameter-behavior | 1 |
| unclassified | 11 |

These counts replace the stale 20/14/8 Phase 16 queue still embedded in
scripts/package-contract-v2-phase19-report.mjs.

### Delivery forecast

Memberships overlap; this table ranks work and does not sum to 418.

| Track | Current membership | What the implementation may claim |
| --- | --- | --- |
| bounded proposal/artifact defects | 43 subpath rows; 7 local-closure rows; 4 closure-replay attempts; 2 wildcard census rows; 4 stale-subject rows; 2 export-local identity rows | the named current blocker was removed |
| archive topology policy | 1 canonical-alias attempt | retain unless ordered-topology invariance is proved |
| value-only certification | 32 structurally complete rows | receipt candidate after security and round-trip prerequisites |
| optional-cardinality finalization | 7 structurally complete rows | honest reduced-semantics candidate after full replan |
| dependency composition | 21 full rows plus 3 partial rows | current external export-all blocker removed |
| export kind | 15 full rows plus 2 partial rows and geolocation | exact callable/noncallable/open terminal result |
| bounded artifact-model extensions | solidbase “?raw” and context dependency layout | verify only with transform/layout authority |
| unavailable published targets | 271 partial rows and 5 malformed full rows | upstream repair or explicit artifact-unavailable result |
| dependency-layout provenance | @solid-primitives/context, 1 full row | authenticate the exact layout or retain |
| unsupported no-ESM surface | 9 full rows plus 136 partial cases | retained until the owning CJS/type-only/worker/side-effect model exists |

The first implementation tracer is @solidjs/html@2.0.0-rc.3, but its status is
a forecast, not a forced success criterion. An exact new refusal is a valid
result.

## Adversarial code audit

### Generation and artifact-resolution defects

1. Subpath identity is discarded during proposal generation.

   rust/crates/solid-facts-backend/src/main.rs constructs the inferred
   PackageContract with a single “.” entrypoint before it reads the
   ResolvedImport. The normalizer later asks for the exact requested subpath.
   This produces “inference has no entrypoint” for 613 directly blocked partial
   artifact cases across 39 rows and is also the root cause of 174 cases in
   four full failures: 787 current artifact cases in total. The repair is to
   build the proposal under resolution.requested_entrypoint and test that two
   subpaths with the same export names cannot alias.

2. Node and Rust local-module resolution share two parity defects.

   A declaration-axis walk rooted in source TypeScript does not try source
   extensions for an extensionless child, and extname-style handling treats a
   suffix such as “.dev” as a terminal extension instead of trying
   “.dev.jsx”, “.dev.tsx”, and declaration counterparts. Seven rows currently
   fail on these bounded cases.

3. Proposal closure replay has two independently reproduced parity defects.

   packages/cli/scripts/artifact-resolution.mjs passes no role override for a
   static child imported by a literal dynamic chunk, which resets that child to
   runtime. Rust correctly preserves LiteralDynamicChunk. This exactly explains
   the closure replay mismatch for @tanstack/devtools and is expected to explain
   the two @solidjs/start-devtools rows.

   The @solidjs/router mismatch is different. Node asks whether assignment
   targets have any TypeScript symbol, so ambient lib.d.ts symbols make Promise
   appear locally bound and two mutable-unbound-global hazards disappear.
   Rust correctly treats those writes as unbound within the package source
   file. Node must test source-file-local binding for assignment, destructuring,
   for-loop, and update targets.

4. The artifact-case budget is counted globally instead of per entrypoint.

   @kobalte/core@0.13.13 has 560 finite entrypoints, of which 491 source
   subpaths are unconditional. Multiplying every entrypoint by the global
   condition partition creates 1,120 candidates and exceeds the 1,024 cap.
   Enumerating and deduplicating branches per entrypoint yields 629
   actual cases. The cap remains; the census calculation changes.

   @solid-devtools/debugger has a related aggregate-census defect: one absent
   source branch rejects the whole finite wildcard even though other branches
   have materialized dist targets. Successful finite branches must be unioned,
   while each absent branch remains an exact local refusal. An all-empty
   wildcard still fails closed.

5. Normalization leaves stale proposal-closure subjects.

   inferred_contract collects closure candidates before normalization.
   Normalization can elide the claimed domain, after which proposal
   canonicalization tries to bind a subject that no longer exists. This causes
   eight proposal-merge refusals across @solid-primitives/debounce,
   @solidjs/signals, @solidjs/web, and solid-js@1.9.14. Candidate subjects must
   be recomputed or filtered against the normalized contract; no
   analyzer-visible positive or closure may be silently dropped.

6. Non-code applicability is not explicit.

   Some declaration-only export-map leaves are sent into the runtime generator,
   while package.json, CSS, worker, and CJS surfaces are mixed into similar
   refusal text. The verifier must derive a typed applicability decision from
   the authenticated snapshot. It must not manufacture a declaration or call a
   missing source branch verified.

7. Export binding is all-or-nothing at artifact-case granularity.

   @solidjs/web’s CLAIMS_DOCUMENT and solid-js@2’s createDeepProxy are runtime
   exports without exact declaration bindings in 12 current cases. bind_exports
   rejects the entire artifact case when one export disagrees. The bounded
   improvement is to retain exact bound exports and attach an export-local
   refusal to the unmatched name. The artifact case and row remain incomplete;
   this is useful proof granularity, not permission to ignore the export.

8. One archive presents a security-sensitive canonical alias policy decision.

   @solid-primitives/start contains package/./dist/index.cjs and
   package/dist/index.cjs as distinct raw member names with identical regular
   file bytes. The snapshot currently rejects the canonical duplicate before
   comparing them. Identical bytes alone are not sufficient because archive
   consumers may disagree about path canonicalization and first/last-member
   order. Default to refusal. Acceptance is permitted only after the phase
   proves content invariance under every relevant extraction rule and binds the
   ordered raw member topology, names, kinds, offsets/order, and digests into
   provenance. Repeated raw names, differing bytes or kinds, case-fold
   collisions, links, and unsafe topology remain hard refusals.

### Certification and authority defects

1. The CLI is a refusal-only scaffold.

   packages/cli/scripts/certify-contract.mjs always refuses live witnesses,
   certification, receipt issuance, and publication. Zero receipts therefore
   demonstrates missing orchestration, not semantic impossibility.

2. Native planning destroys the capability needed to certify.

   write_contract_certification_plan in
   rust/crates/solid-facts-backend/src/main.rs serializes demand IDs and summary
   fields, then drops the opaque CertificationPlan and its subjects. The
   TypeFactsCertificationSchedule constructor later requires caller-supplied
   demand-to-invocation assignments. A caller-authored JSON schedule is not an
   authority boundary.

3. Producer identity is defined but not configured in production.

   TypeFactsProducerPin::configured requires compile-time executable and source
   manifest digests, but the normal build does not provide them and no
   production transaction invokes it. A replaced producer must fail before any
   transcript is accepted.

4. Current Type Facts family reconciliation is not sufficient for issuance.

   The verifier accepts any complete callable path rather than the demanded
   exact path, maps ValueRoot::Export to a selected call result, requires a
   selected signature even for a plain exported value, and does not reconcile
   every operation/ordinal subject. Default-export declaration aliases can also
   fail the exact declaration-name comparison. These are security
   prerequisites, not post-launch cleanup.

5. Source provenance stops at the root archive.

   A Type Facts project can consult external declaration files. Authenticating
   only paths under the root package leaves those dependency inputs mutable.
   Certification must materialize the package in a private snapshot-owned
   project and classify every consulted source as verifier-authored query or
   harness bytes, an authenticated root-archive member, an authenticated
   dependency-archive member, or a producer-bundled TypeScript library. Each
   class enters the source/provenance root under a separate domain; unknown or
   mutable sources refuse.

6. Cardinality is explicitly unsupported.

   The OperationCardinality branch always refuses arbitrary runtime loop or
   reentry bounds. Policy 2 permits a verifier to remove an unproved optional
   positive, but only by cascading dependent operations/edges as necessary,
   normalizing a new main document, and acquiring a wholly new demand graph.
   Old witnesses may not be reused across that semantic-digest change.

### Receipt, catalog, and dependency defects

1. Policy-2 publication and ordinary discovery are incompatible.

   publish_policy2_catalog writes a content-addressed policy-2 publication and
   singular pointer, while ordinary discovery expects
   .solid-checker/accepted-contracts.json. read_accepted_contract_catalog only
   understands obsolete-policy1 entries, and load_accepted_contract always
   returns ReceiptAuthenticationRequired. A receipt that cannot survive
   publish-to-normal-load round-trip does not verify a row.

2. External export-all refuses even when accepted contracts are supplied.

   exported_names_for_file unconditionally refuses a bare external export-all.
   emit_package_contract receives no AcceptedContractIndex, and the real Node
   prepareArtifact path does not pass the lower-level acceptedDependencies
   input. The current message claiming that --accepted-contracts unblocks the
   case is false.

3. Dependency planning reports only the first edge.

   The 21 dependency rows include recursive, multi-subpath graphs. Examples
   include 13 @tanstack/pacer subpaths, three @tanstack/table-core subpaths,
   four direct/transitive Solid Start packages, and nine @corvu packages.
   Certification must derive the complete exact graph from the archive module
   closure, detect cycles, and certify leaves bottom-up.

4. Positive facts can cross unmodeled authority seams.

   Dependency demands are currently derived from dependencies multiplied by
   closure candidates; a dependency-derived positive with no proposed closure
   can receive no composition demand. Compiler, rest/spread, and consulted
   source provenance also need explicit subjects. No positive may cross one of
   these seams without a witness.

### Reporting defects

The Phase 19 report script starts from frozen Phase 16 owner buckets and records
the first terminal class. It therefore says 20 dependency rows and 14
export-kind rows where the current report has 21 and 15, hides the
@solid-primitives/geolocation terminal export-kind conflict behind an earlier
UnknownCallbackExecution marker, and cannot show deeper dependency edges.

Phase 20 reporting must consume the current report schema directly and retain
all refusal instances and external edges. Every forecast below is a cohort
membership, not an additive promise: cohorts overlap and a fixed first blocker
may reveal the next exact refusal.

## Target architecture

The implementation should expose one small native interface:

    certify_published_contract(request, authorities)
        -> CertifiedPublication | ExactRefusalSet

The solid-facts-backend certification module owns the whole transaction:

1. validate registry metadata and immutable archive bytes;
2. construct and retain the opaque artifact snapshot and CertificationPlan;
3. replay resolution and one canonical module-closure implementation;
4. materialize a private, snapshot-owned TypeScript project;
5. derive evidence schedules from demand subjects inside Rust;
6. launch pinned Type Facts, compiler, dependency, and probe adapters;
7. authenticate complete consulted-source provenance;
8. reconcile every witness with its exact demand subject;
9. finalize conservatively, pruning only policy-permitted positives;
10. normalize, re-inventory, and replan after any semantic change;
11. verify complete demand coverage;
12. construct receipt bindings internally;
13. ask the configured issuer to sign those exact bindings;
14. authenticate the returned receipt;
15. atomically publish the loader-compatible catalog.

Steps 1–15 are one native-process authority transaction and one opaque
capability. After it has published and released that capability, the
orchestrator starts a separate fresh analyzer process as a post-publication
validation step. That process must discover the catalog normally, authenticate
the receipt independently, select the exact importer/specifier, and query the
accepted semantics.

Node remains a shallow acquisition and presentation adapter. It may fetch exact
registry/archive bytes and select explicit configuration, but it may not author
witnesses, demand assignments, accepted dependency digests, receipt fields, or
semantic pruning.

This seam is intentionally deep: artifact identity, proof scheduling,
finalization, issuance, and publication are local to one module and one
same-process capability. Exposing those intermediate objects would make every
caller reproduce security-sensitive ordering rules.

## Implementation slices

### Slice 0 — Make the row ledger executable

Replace the frozen Phase 16 owner queue with a report-derived Phase 20 ledger.
For every probe ID record:

- all artifact-case and proposal-merge refusals, not only the first;
- all external module edges with exact subpaths and resolved package versions;
- whether the row is proposal-complete, proposal-partial, or fully refused;
- certification owner and every proof family/count;
- supported, type-only-inapplicable, and unsupported artifact cases;
- current terminal disposition and next owning slice; and
- forecast versus observed result after every remeasurement.

Fix classifier precedence so geolocation terminates as export-kind-conflict.
Add fixtures with shuffled diagnostic marker order and multi-edge examples for
solid-form, solid-start, solid-pacer, solid-table, and corvu.

Exit gate: the generated ledger assigns all 418 row IDs exactly one proposal
state and one certification state; assigns every enumerated artifact case
exactly one applicability classification; derives each row’s aggregate
applicability from its complete mixed case set; preserves overlapping blocker
memberships separately; and reproduces the 44/314/60 baseline and 21/15/1
failure ledgers.

### Slice 1 — Add adversarial red tests before enabling authority

Pin tests for every currently unsafe acceptance:

- wrong subpath with the same export name;
- unrelated complete callable path offered for a demanded path;
- export-value demand answered by a call-result fact;
- wrong operation, ordinal, guard, or recursive subject;
- unknown root callability presented as “value”;
- default-export alias mismatch;
- mutable external declaration substituted after planning;
- caller-supplied demand assignment;
- transcript from a different plan, row, generation, or producer;
- missing compiler/rest/dependency source-provenance demand;
- reused evidence after pruning; and
- policy-2 publication that normal discovery cannot load.

Exit gate: the tests fail against the current implementation for the intended
reason, and no live adapter is enabled yet.

### Slice 2 — Repair exact proposal identity and normalized subjects

Build the inferred PackageContract under
ResolvedImport.requested_entrypoint. Make requested entrypoint an explicit
argument of the proposal-generation module instead of reading it after
inference. Reject missing or mismatched entrypoint identity.

Add process fixtures for root versus subpath, two subpaths exporting the same
name, wildcard-captured subpaths, condition variants, namespace/default
exports, and a deliberately absent requested subpath.

Exit gate: all 43 current row memberships in the subpath cohort lose the
specific “inference has no entrypoint” or full export-missing cause. This is a
proposal-stage claim only; rows with another refusal remain unverified.

In the same owning proposal module, derive closure/proof candidates from the
normalized contract rather than the pre-normalized summary. Add fixtures where
normalization retains a domain, elides a domain, and would otherwise leave a
stale subject.

Second exit gate: all eight current proposal-merge refusals disappear;
@solid-primitives/debounce advances to a complete proposal; and no positive or
closure candidate is lost without an explicit finalizer decision.

### Slice 3 — Repair local resolution and finite branch census

Unify Node and Rust local-module candidate rules:

- declaration-axis extensionless imports from source files try source and
  declaration extensions in canonical order;
- multi-dot basenames such as HeadContent.dev try supported suffixes;
- source substitutions are byte-for-byte parity-tested; and
- assets remain assets rather than guessed modules.

Change artifact-case counting to enumerate and deduplicate active branches per
entrypoint before enforcing the existing 1,024 cap. Keep ambiguous wildcard
censuses and genuine over-budget cases refused.

Exit gates:

- the seven current local-closure rows advance past their missing local module;
- @kobalte/core@0.13.13 advances past the false resource refusal;
- @solid-devtools/debugger advances past the aggregate wildcard refusal but
  retains its exact absent-source and subpath refusals; an all-empty wildcard
  remains refused; and
- a boundary fixture with 1,024 versus 1,025 real cases preserves the cap.

### Slice 4 — Make artifact applicability explicit

Introduce a verifier-owned ArtifactApplicability result:

- RuntimeModule — must be certified;
- TypeOnlyExport — proven from the selected export-map leaf and declarations,
  with no runtime target;
- UnsupportedAsset — JSON, CSS, worker, native, Wasm, or other unmodeled
  runtime shape;
- MissingPublishedTarget — selected target absent from the authenticated
  archive;
- UnsupportedConditionSet — the export tree has no runtime branch for the
  supported condition environment; and
- UnsupportedModuleSystem — CJS or another unsupported execution model.

Only TypeOnlyExport is locally inapplicable under this plan. The other four
non-runtime outcomes keep the row unverified unless a later artifact-model
slice owns them. A future product metric may show JSON/CSS as explicit
not-applicable cases, but that accounting result is not semantic verification.

Change export binding to preserve exact runtime/declaration pairs while
recording unmatched runtime exports as export-local refusals. First investigate
the proposal and receipt completeness representation: no artifact case may be
called complete while one runtime export is refused.

Exit gate: declaration-only cases no longer enter runtime inference, while
package.json/CSS/CJS/missing-target cases remain visible and fail closed; the
12 current CLAIMS_DOCUMENT/createDeepProxy cases retain their proven sibling
exports but remain locally incomplete.

### Slice 5 — Make canonical closure replay single-owner

Move proposal closure construction behind the Rust artifact snapshot module, or
make Node consume the exact Rust result without independently reimplementing
role and candidate rules. Preserve LiteralDynamicChunk for its static local
children. Make unbound-global detection depend on source-file-local bindings,
not ambient TypeScript symbols. Emit a structured
entry/dependency/hazard diff on mismatch.

Pin the already reproduced @solidjs/router@1.0.0 hazard difference and
@tanstack/devtools role difference as exact tests. Remeasure both
@solidjs/start-devtools rows after the role fix rather than promising they share
the same cause.

Treat canonical-alias archive handling as a separate provenance-policy gate for
@solid-primitives/start. Before considering acceptance, prove that the selected
member bytes are invariant under the extraction/canonicalization rules used by
the checker and relevant package consumers, and bind the ordered raw topology
into the provenance root. Only then may identical ordinary-file aliases be
accepted. Add mutation tests for order reversal, first-wins/last-wins behavior,
repeated raw names, different bytes, different kinds, case-fold collisions,
links, and traversal. If the invariance proof is incomplete, retain the current
refusal and require an upstream republish.

Exit gate: @tanstack/devtools and @solidjs/router replay identically; each
@solidjs/start-devtools row either replays identically or records a new exact
artifact refusal; @solid-primitives/start advances only through the identical
ordinary-file alias rule after the provenance-policy gate; otherwise it remains
refused. Every unsafe duplicate shape still fails.

### Slice 6 — Build the opaque native certification transaction

Keep CertificationPlan alive for the duration of certification. Derive
TypeFactsCertificationSchedule and every location assignment from
snapshot-verified export bindings and demand subjects inside Rust. Never
serialize the opaque plan as authority.

Materialize package and exact dependency archives in a private snapshot root.
Record all source files consulted by Type Facts and classify every byte under
one of four authorities:

- verifier-authored query/harness sources — constructed deterministically
  inside the opaque plan and bound by path, bytes, purpose, and plan root;
- root-package sources — exact members of the authenticated root archive;
- dependency sources — exact members of an authenticated dependency
  archive/lock/accepted-contract edge; and
- producer-bundled TypeScript library sources — bound to the pinned producer
  source manifest and compiler identity.

Unknown, ambient, or mutable sources refuse. A source cannot move between these
classes without changing the source/provenance root and reacquiring evidence.

Wire the release/build process to provide
SOLID_TYPEFACTS_CERTIFICATION_SHA256 and
SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256, invoke
TypeFactsProducerPin::configured, and reject byte, manifest, protocol, build-id,
or restart/generation substitution.

Exit gate: @solidjs/html@2.0.0-rc.3 can acquire live evidence for its two
non-artifact demands or return a new exact semantic refusal. “Automatic adapter
is unavailable” is no longer an allowed terminal result.

### Slice 7 — Add exact export-value Type Facts evidence

Extend the producer/client protocol with an authority-bearing exact export
value transcript. It must identify the snapshot-selected declaration or alias,
root callability/constructability, recursive value alternatives, exact paths,
and completeness/open reasons without inventing a dummy call.

Repair family reconciliation so:

- callable-path matches the demanded root, alternative, and complete path;
- RecursiveValueShape distinguishes export value, operation input, and
  operation output;
- selected signatures are required only for actual invocation subjects;
- argument binding, reachability, guard partitions, and ordinals match their
  exact normalized operation;
- default exports bind by canonical export alias plus declaration identity; and
- Unknown never certifies either callable or noncallable behavior.

Amend the proof-policy digest and authority audit for the new family/fields.

Exit gate: all Slice 1 cross-subject and unknown-value attacks remain refused,
and the html tracer reaches complete Type Facts evidence.

### Slice 8 — Finalize, issue, publish, and load one real receipt

Implement a Rust finalizer:

- retain every witnessed positive;
- remove or cascade-open an unwitnessed optional positive only where policy 2
  permits;
- refuse any unwitnessed mandatory closure or unsupported invariant;
- normalize the finalized document;
- derive a new semantic digest and demand graph; and
- reacquire all evidence after any change.

Define canonical empty authority roots for every mandatory receipt root whose
verified schedule/set is empty, including the current empty probe schedule.
Each empty root is domain-separated by policy digest, demand-graph root, proof
family/adapter, schedule version, and zero item count. It is not a shared zero
hash or caller-supplied placeholder. ProbeGateSchedule authentication must
accept this verifier-derived empty schedule without launching a fake harness;
nonempty schedules still require the pinned harness/runtime image.

Add explicit persistent-local issuer and trust-store configuration outside the
analyzed project. Secrets never enter logs or audit documents. Construct
bindings in Rust, sign only the final binding set, authenticate the returned
receipt, and atomically publish one catalog format understood by ordinary
discovery.

The single-process authority transaction ends only after atomic publication.
Row verification then has a separate mandatory postcondition: a fresh analyzer
process reads .solid-checker/accepted-contracts.json through ordinary
discovery, independently authenticates the policy-2 entry, and selects it for
the exact importer/specifier.

Exit gate: @solidjs/html@2.0.0-rc.3 becomes the first verified row, or the slice
reports a newly discovered exact premise that prevents that forecast.

Required negative tests include untrusted/self-signed/revoked keys, wrong
scope, wrong importer/specifier, mutated main/receipt/pointer, stale trust
configuration, path traversal, crash before pointer rename, and cache reuse
across a trust-root change. Also reject a missing, arbitrary, differently
domain-separated, or transplanted empty root.

### Slice 9 — Scale the value-only Type Facts cohort

After the html tracer, run the nine two-export/four-demand rows:

- @corvu-next/calendar@0.1.5|solid2|only;
- @corvu-next/drawer@0.1.5|solid2|only;
- @corvu-next/focus-trap@0.1.5|solid2|only;
- @corvu-next/persistent@0.1.5|solid2|only;
- @corvu-next/presence@0.1.5|solid2|only;
- @corvu-next/prevent-scroll@0.1.5|solid2|only;
- @corvu-next/transition-size@0.1.5|solid2|only;
- @solid-devtools/transform@0.10.4|solid1|only; and
- @tanstack/solid-charts@0.15.0|solid1|only.

Then attempt the rest of the 32 value-only rows in increasing demand count.
Run solid-recharts@1.0.1 last with explicit time and memory budgets because it
contains 327 exports and 636 of this cohort’s demands.

Exit gate: every row either has a receipt loaded through normal discovery or a
row-local exact refusal. No row shares evidence by name or package family.

### Slice 10 — Reconcile optional cardinality claims

For the seven operation rows, first try verifier-owned finalization:

- remove unsupported optional scope/min/max cardinality fields;
- cascade removal/opening through dependent operations and edges when needed;
- normalize a new main document; and
- replan and reacquire every witness from zero.

Do not weaken or waive OperationCardinality and do not reuse the old graph.
Retaining the original cardinality claims requires a separately reviewed finite
cardinality producer extension.

After cardinality-only removal, the forecast remaining demand counts are:

| Row | Remaining demands |
| --- | ---: |
| @solid-primitives/local-store@1.1.4 | 8 |
| @solid-primitives/visibility-observer@2.0.1 | 11 |
| @solid-primitives/throttle@1.2.0 | 12 |
| @solidjs/element@2.0.0-rc.3 | 14 |
| @solid-primitives/gestures@1.2.1 | 32 |
| @solidjs/meta@0.29.4 | 38 |
| @solid-primitives/jsx-parser@0.2.0 | 42 |

Exit gate: a row may receive an honest partial-semantics receipt only if its
final main document contains no unsupported cardinality assertion and all
remaining claims are witnessed. Otherwise it stays refused.

### Slice 11 — Add exact runtime binding/write census

Extend Type Facts and retained fact data with an exact runtime binding census:

- immutable initializer and every binding write;
- callable, constructable, noncallable, mixed, or open result;
- finite enum/IIFE initialization;
- exact alias and reexport target identity;
- conditional initializer branches;
- property mutation distinguished from binding reassignment; and
- dynamic/eval/external-call-result escape reasons.

Wire it into export_kind_proof_from_entity without any name heuristic.

First target the 11 high-confidence rows: five enum/IIFE rows, three isBrave
rows, and three AnimatePresence rows. The four TanStack devtools rows remain
forecast-only until exact accepted dependency semantics can prove their helper
call results.

Apply the same census to 41 export-kind refusals in two partial rows:
@solid-devtools/locator@0.16.7 (addClickInterceptor) and solid-js@1.9.14
(Aliases across condition cases). These are candidates for exact answers, not
assumed callable or noncallable values.

Exit gate: every exact closed binding gets the right kind; dynamic/mixed writes
and opaque external call results remain refused.

### Slice 12 — Repair geolocation reconciliation

Add a focused process fixture for the immutable arrow exports in
@solid-primitives/geolocation. Assert:

- the identifier-span Type Facts answer;
- the exact entry export entity;
- ExportKindProof;
- the inferred summary before and after promotion; and
- the terminal classifier result.

Fix the entity join or callability proof revealed by the fixture. Independently
make promote_entry_callable refuse an inconsistent NonCallable proof paired
with a value summary that still carries function effects, rather than allowing
contract validation to fail later.

Exit gate: the row advances with exact callable facts or a correctly classified
export-kind refusal; it never reports parameter behavior for this terminal
state.

### Slice 13 — Authenticate recursive dependency composition

Derive the complete dependency DAG from the snapshot module graph. Resolve
exact versions and integrities, certify leaves bottom-up, authenticate every
leaf receipt in Rust, expand exact external export-all names from the selected
accepted artifact, and bind parent closure/positive facts to those dependency
receipts.

The generator must receive accepted dependencies only as authenticated native
objects, never caller-supplied digest records.

Land in tiers:

1. one single-edge wrapper;
2. corvu with nine already proposal-complete @corvu leaves;
3. @tanstack/solid-pacer with 13 subpaths;
4. @tanstack/solid-table with three subpaths;
5. solid-form and Solid Start multi-hop graphs; and
6. the remaining single-edge/version-paired rows.

Tests cover nested installations, the same specifier at different versions,
exact subpaths, missing exports, cycles, depth/node budgets, stale/revoked
leaves, transitive trust-root changes, and a positive fact whose only authority
is a dependency.

Exit gate: each of the 21 full-failure parent rows and the eight affected cases
in three partial Solid Router rows advances past the current external
export-all refusal. This is not a promise of verified rows; newly exposed leaf
Type Facts, artifact, compiler, probe, or cardinality refusals stay local.

### Slice 14 — Decide bounded artifact-model extensions

Two checker-addressable shapes remain outside the current artifact model:

- @kobalte/solidbase imports a local file with a “?raw” loader query; and
- @solid-primitives/context reaches dependency declarations through an
  archive-relative node_modules path.

Support “?raw” only through a pinned, deterministic transform identity whose
input bytes and output digest enter the closure and receipt. Never strip the
query and pretend it is ordinary JavaScript.

Support the context case only if the exact dependency layout is authenticated
by the recursive dependency graph and matches the package’s declaration
resolution. Otherwise request an upstream republish and retain the refusal.

Exit gate: each shape is either covered by an authority-bound model with
mutation tests or remains explicitly unsupported.

### Slice 15 — Remeasure every row and close the phase

Run the 418-row corpus uncached with the exact fresh checker and Type Facts
binaries. Compare the generated row ledger to every forecast in this document.
For each row record:

- proposal outcome;
- authenticated receipt outcome;
- verified artifact cases and exports;
- type-only-inapplicable cases;
- remaining refusal owner/family/subject;
- upstream artifact defect, when applicable; and
- whether TypeScript already rejects the published surface.

Run focused checks after each slice, then the proportional handoff set,
contract conformance, ownership/coverage gates, the policy-2 adversarial suite,
and make verify once at final handoff.

Exit gate: all 418 row IDs have an evidence-backed final disposition, no
verified count depends on omitted/refused cases, and every checker-fix forecast
is either observed or replaced by a more exact refusal with a named owner.

## Exact current row cohorts

Counts in this section are memberships and may overlap. “Candidate” means the
named current blocker is bounded; it is not a guarantee that no later blocker
exists.

### Structurally complete Type Facts rows without cardinality — 32

These are the first receipt candidates after Slices 6–9:

- @corvu-next/accordion@0.1.5|solid2|only;
- @corvu-next/calendar@0.1.5|solid2|only;
- @corvu-next/dialog@0.1.5|solid2|only;
- @corvu-next/disclosure@0.1.5|solid2|only;
- @corvu-next/dismissible@0.1.5|solid2|only;
- @corvu-next/drawer@0.1.5|solid2|only;
- @corvu-next/focus-trap@0.1.5|solid2|only;
- @corvu-next/list@0.1.5|solid2|only;
- @corvu-next/otp-field@0.1.5|solid2|only;
- @corvu-next/persistent@0.1.5|solid2|only;
- @corvu-next/popover@0.1.5|solid2|only;
- @corvu-next/presence@0.1.5|solid2|only;
- @corvu-next/prevent-scroll@0.1.5|solid2|only;
- @corvu-next/resizable@0.1.5|solid2|only;
- @corvu-next/tooltip@0.1.5|solid2|only;
- @corvu-next/transition-size@0.1.5|solid2|only;
- @corvu/accordion@0.2.5|solid1|only;
- @corvu/calendar@0.1.2|solid1|only;
- @corvu/dialog@0.2.4|solid1|only;
- @corvu/disclosure@0.2.2|solid1|only;
- @corvu/drawer@0.2.4|solid1|only;
- @corvu/otp-field@0.1.4|solid1|only;
- @corvu/popover@0.2.0|solid1|only;
- @corvu/resizable@0.2.5|solid1|only;
- @corvu/tooltip@0.2.2|solid1|only;
- @solid-devtools/transform@0.10.4|solid1|only;
- @solidjs/html@2.0.0-rc.3|solid2|only;
- @solidjs/meta@1.0.0-next.2|solid2|floor and |head;
- @tanstack/ai-solid@0.19.1|solid1|only;
- @tanstack/solid-charts@0.15.0|solid1|only; and
- solid-recharts@1.0.1|solid1|only.

### Structurally complete rows with cardinality — 7

- @solid-primitives/gestures@1.2.1|solid1|only;
- @solid-primitives/jsx-parser@0.2.0|solid1|only;
- @solid-primitives/local-store@1.1.4|solid1|only;
- @solid-primitives/throttle@1.2.0|solid1|only;
- @solid-primitives/visibility-observer@2.0.1|solid1|only;
- @solidjs/element@2.0.0-rc.3|solid2|only; and
- @solidjs/meta@0.29.4|solid1|only.

### Structurally complete artifact-provenance rows — 5

- @solid-primitives/start@0.0.4|solid1|only — identical ordinary-file canonical
  aliases; provenance-policy candidate that remains refused without the
  ordered-topology invariance proof;
- @solidjs/router@1.0.0|solid1|only — structured closure diff and checker
  parity repair candidate;
- @solidjs/start-devtools@1.0.0-next.4|solid2|floor and |head — dynamic-chunk
  role propagation repair candidates; and
- @tanstack/devtools@0.14.2|solid1|only — dynamic-chunk role propagation repair
  candidate.

### Exact subpath-identity cohort — 43 memberships, 787 current cases

Thirty-nine partial rows:

- @corvu-next/utils@0.1.5|solid2|only;
- @corvu/utils@0.4.2|solid1|only;
- @kobalte/core@0.13.13|solid1|only;
- @kobalte/utils@2.0.0-alpha.0|solid2|only;
- @solid-devtools/debugger@0.28.1|solid1|only;
- @solid-devtools/ui@0.10.3|solid1|only;
- @solid-primitives/analytics@2.0.0-next.2|solid2|floor and |head;
- @solid-primitives/sse@0.0.103|solid1|only;
- @solid-primitives/sse@1.0.0-next.2|solid2|floor and |head;
- @solid-primitives/storage@4.4.0|solid1|only;
- @solid-primitives/storage@5.0.0-next.4|solid2|floor and |head;
- @solid-primitives/utils@6.4.1|solid1|only;
- @solid-primitives/utils@7.0.0-next.4|solid2|floor and |head;
- @solid-primitives/workers@2.0.1-next.1|solid2|floor and |head;
- @solidjs/diagnostics@2.0.0-rc.3|solid2|only;
- @solidjs/h@2.0.0-rc.3|solid2|only;
- @solidjs/image@0.1.0|solid1|only;
- @solidjs/router@2.0.0-next.18|solid2|only;
- @solidjs/start@2.0.3|solid1|only;
- @solidjs/web@2.0.0-rc.3|solid2|only;
- @tanstack/ai-devtools-core@0.5.8|solid1|only;
- @tanstack/devtools-ui@0.7.1|solid1|only;
- @tanstack/form-devtools@1.0.0-alpha.2|solid1|only;
- @tanstack/pacer-devtools@1.4.0|solid1|only;
- @tanstack/solid-ai-devtools@0.2.71|solid1|only;
- @tanstack/solid-pacer-devtools@0.14.0|solid1|only;
- @tanstack/solid-router@1.170.30|solid1|only;
- @tanstack/solid-router@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-start-client@1.168.29|solid1|only;
- @tanstack/solid-start-client@2.0.0-rc.2|solid2|floor and |head;
- solid-js@1.9.14|solid1|only; and
- solid-js@2.0.0-rc.3|solid2|only.

Four full failures:

- @kobalte/core@2.0.0-alpha.0|solid2|only;
- @tanstack/charts@0.15.0|solid1|only;
- @tanstack/devtools-a11y@0.2.2|solid1|only; and
- @tanstack/devtools-utils@0.7.0|solid1|only.

The partial rows account for 613 directly blocked cases. The four full rows
account for 174. These figures count the named current blocker, not cases
guaranteed to certify after it is removed.

### Exact local-closure candidate cohort — 7

- @tanstack/solid-query-devtools@5.102.5|solid1|only;
- @tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor and |head;
- @tanstack/solid-router@1.170.30|solid1|only;
- @tanstack/ai-solid-ui@0.7.20|solid1|only; and
- solid-recharts@2.0.0-beta.1|solid2|floor and |head.

These represent 16 currently confirmed local-module blocker cases. Full-row
refusal totals may include additional artifact cases with a different blocker.

### Stale normalized proposal-subject cohort — 4 rows, 8 cases

- @solid-primitives/debounce@1.3.0|solid1|only — one case and the row’s only
  current refusal;
- @solidjs/signals@2.0.0-rc.3|solid2|only — one case;
- @solidjs/web@2.0.0-rc.3|solid2|only — two cases; and
- solid-js@1.9.14|solid1|only — four cases.

### Export-local runtime/declaration identity cohort — 2 rows, 12 cases

- @solidjs/web@2.0.0-rc.3|solid2|only — CLAIMS_DOCUMENT; and
- solid-js@2.0.0-rc.3|solid2|only — createDeepProxy.

The forecast is finer local refusal granularity, not proof of the unmatched
exports. These rows remain incomplete until every runtime export has exact
identity or an explicitly supported type/runtime applicability disposition.

### Non-code and type-only applicability cohorts

Twenty-six rows contain 74 cases with no declaration target: 70 package.json
cases and four CSS cases. A further CSS case in
@solid-devtools/ui@0.10.3|solid1|only currently reaches TypeScript inference.
These assets stay unsupported under the executable ESM contract model.

Eight cases in @solidjs/vite-plugin@3.0.0-next.34|solid2|floor and |head
currently select no active package-export condition for ./boundary-modules and
./virtual-solid-manifest. They become TypeOnlyExport only if the authenticated
export tree proves that the leaf has no executable runtime branch in the
supported environment. Otherwise they remain UnsupportedConditionSet; the
resolver must not invent a default branch.

Seven rows contain 728 cases whose selected runtime target is a declaration
file outside the runtime TypeScript project:

- @solidjs/h@2.0.0-rc.3|solid2|only;
- @solidjs/image@0.1.0|solid1|only;
- @solidjs/start@2.0.3|solid1|only;
- @solidjs/universal@2.0.0-rc.3|solid2|only;
- @solidjs/web@2.0.0-rc.3|solid2|only;
- solid-js@1.9.14|solid1|only; and
- solid-js@2.0.0-rc.3|solid2|only.

The verifier must prove that a selected export-map leaf is genuinely type-only
before classifying it runtime-inapplicable. It must not analyze declaration
bytes as executable code.

### Current 21 dependency rows

- @solidjs/testing-library@0.8.10|solid1|only;
- @tanstack/solid-db@0.2.40|solid1|only;
- @tanstack/solid-form@2.0.0-alpha.2|solid1|only;
- @tanstack/solid-hotkeys@0.10.0|solid1|only;
- @tanstack/solid-pacer@0.22.0|solid1|only;
- @tanstack/solid-query@5.102.5|solid1|only;
- @tanstack/solid-query@6.0.0-rc.0|solid2|floor and |head;
- @tanstack/solid-query-persist-client@5.102.5|solid1|only;
- @tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor and |head;
- @tanstack/solid-start@1.168.47|solid1|only;
- @tanstack/solid-start@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-start-server@1.167.36|solid1|only;
- @tanstack/solid-start-server@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-store@0.11.1|solid1|only;
- @tanstack/solid-table@9.1.2|solid1|only;
- @tanstack/solid-virtual@3.13.37|solid1|only; and
- corvu@0.7.2|solid1|only.

Their known first/deeper edges are:

- testing-library -> @testing-library/dom;
- solid-db -> @tanstack/db;
- solid-form -> @tanstack/form-core -> @tanstack/solid-store ->
  @tanstack/store;
- solid-hotkeys -> @tanstack/hotkeys;
- solid-pacer -> 13 exact @tanstack/pacer root/subpath imports;
- solid-query -> @tanstack/query-core;
- solid-query-persist-client -> @tanstack/query-persist-client-core;
- solid-start -> @tanstack/start-client-core, /client-rpc,
  @tanstack/solid-start-client, and @tanstack/solid-start-server ->
  @tanstack/start-server-core;
- solid-table -> @tanstack/table-core root, /static-functions, and
  /experimental-worker-plugin;
- solid-virtual -> @tanstack/virtual-core; and
- corvu -> nine @corvu component packages.

The same composition seam blocks eight cases in three partial rows:

- @tanstack/solid-router@1.170.30|solid1|only — four cases; and
- @tanstack/solid-router@2.0.0-rc.2|solid2|floor and |head — two cases each.

Those rows also belong to subpath/resolver cohorts, so fixing dependency
composition alone does not complete them.

### Current 15 export-kind rows

High-confidence bounded census candidates — 11:

- @kobalte/utils@0.9.2|solid1|only — EventKey;
- @solid-primitives/analytics@0.2.1|solid1|only — EventType;
- @solid-primitives/audio@1.4.5|solid1|only — AudioState;
- @solid-primitives/cookies-store@1.1.11|solid1|only — CookieSitePolicy;
- @solid-primitives/intersection-observer@2.2.5|solid1|only — DirectionX;
- @solid-primitives/platform@0.2.1|solid1|only — isBrave;
- @solid-primitives/platform@1.0.0-next.2|solid2|floor and |head — isBrave;
- motion-solidjs@0.6.0|solid1|only — AnimatePresence; and
- motion-solidjs@0.7.0-beta.4|solid2|floor and |head — AnimatePresence.

Dependency-aware forecast only — 4:

- @tanstack/hotkeys-devtools@0.9.0|solid1|only — HotkeysDevtoolsCore;
- @tanstack/solid-hotkeys-devtools@0.7.0|solid1|only —
  HotkeysDevtoolsPanel;
- @tanstack/solid-table-devtools@9.2.0|solid1|only —
  TableDevtoolsPanel; and
- @tanstack/table-devtools@9.2.0|solid1|only — TableDevtoolsCore.

The latter four use conditional branches and/or arrays returned by external
helpers. Local syntax is not enough to certify them.

Two partial rows contain another 41 export-kind cases:

- @solid-devtools/locator@0.16.7|solid1|only — addClickInterceptor, one case;
  and
- solid-js@1.9.14|solid1|only — Aliases, 40 cases.

They are owned by the same exact census but also have other cohort memberships.

### Current misclassified geolocation row — 1

- @solid-primitives/geolocation@1.5.5|solid1|only.

This is owned by export kind/summary reconciliation, not parameter behavior.

### Proposal-stage progress candidates after bounded checker fixes — 31

These rows have at least one current blocker owned by the bounded subpath,
resolver, branch-census, normalized-subject, applicability, export-kind, or
export-local identity work. Removing that blocker should advance proposal
coverage. It does not imply a structurally complete proposal: in particular,
@solidjs/web and solid-js@2 retain 12 unmatched runtime/declaration cases, and
@solid-devtools/locator retains an export-kind premise. Every row will still
require certification:

- @corvu-next/utils@0.1.5|solid2|only;
- @corvu/utils@0.4.2|solid1|only;
- @kobalte/utils@2.0.0-alpha.0|solid2|only;
- @solid-devtools/locator@0.16.7|solid1|only;
- @solid-primitives/debounce@1.3.0|solid1|only;
- @solidjs/h@2.0.0-rc.3|solid2|only;
- @solidjs/router@2.0.0-next.18|solid2|only;
- @solidjs/signals@2.0.0-rc.3|solid2|only;
- @solidjs/universal@2.0.0-rc.3|solid2|only;
- @solidjs/vite-plugin@3.0.0-next.34|solid2|floor and |head;
- @solidjs/web@2.0.0-rc.3|solid2|only;
- @tanstack/ai-devtools-core@0.5.8|solid1|only;
- @tanstack/devtools-ui@0.7.1|solid1|only;
- @tanstack/form-devtools@1.0.0-alpha.2|solid1|only;
- @tanstack/pacer-devtools@1.4.0|solid1|only;
- @tanstack/solid-ai-devtools@0.2.71|solid1|only;
- @tanstack/solid-devtools@0.8.12|solid1|only;
- @tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only;
- @tanstack/solid-pacer-devtools@0.14.0|solid1|only;
- @tanstack/solid-router-devtools@1.167.1|solid1|only;
- @tanstack/solid-router-devtools@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-router-ssr-query@1.167.2-pre.0|solid1|only;
- @tanstack/solid-router-ssr-query@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-start-client@1.168.29|solid1|only;
- @tanstack/solid-start-client@2.0.0-rc.2|solid2|floor and |head;
- @tanstack/solid-start-config@1.120.20|solid1|only; and
- solid-js@2.0.0-rc.3|solid2|only.

This 31-row progress cohort must be regenerated after each proposal slice. A
row becomes proposal-complete only when the live report has no remaining case
or export-local refusal; new proof demands may then make it semantically
unsupported.

## Rows that checker code must not falsely unblock

### Published target defects

The partial corpus contains 310 missing-target instances across 271 rows: 267
Solid Primitives rows and four Solid Devtools rows. The common absent paths are
267 src/index.ts targets and 17 src/index.tsx targets, followed by worker,
relay, Tauri, nested source, and selected Solid dist/index.jsx paths.

Two hundred fifty-six rows have no other current refusal. They are not 256
easy checker wins. Under the current artifact-case policy, the selected target
is absent from the exact published archive. Those cases require a package
republish or an explicitly reviewed change to which package conditions the
product promises to certify. The checker may improve the ledger and avoid a
duplicate diagnostic, but it cannot issue semantics for missing bytes.

Full-failure published defects:

- @kobalte/themes@0.0.1-next.0|solid1|only — manifest points to absent
  dist/index.jsx;
- @solid-primitives/animation@1.0.0-next.1|solid2|floor and |head — archive has
  no dist payload;
- @solid-primitives/composites@1.1.1|solid1|only — archive has no dist payload;
- @solid-primitives/workers@0.4.3|solid1|only — declarations import absent
  ./types.js.

These five rows need upstream corrected artifacts.

### Unsupported execution shapes

Nine full rows have no supported ESM runtime surface:

- @solid-devtools/babel-plugin@0.3.1|solid1|only;
- @solid-devtools/ext-adapter@0.17.0|solid1|only;
- @solid-devtools/extension-adapter@0.12.1|solid1|only;
- @solid-devtools/shared@0.20.0|solid1|only;
- @solid-primitives/countdown@1.0.9|solid1|only;
- @solid-primitives/date-difference@1.0.2|solid1|only;
- @solid-primitives/reducer@0.0.101|solid1|only;
- @solid-primitives/until@0.1.1|solid1|only; and
- solid-devtools@0.34.5|solid1|only.

They remain refused until a separately designed CJS/module-side-effect proof
model exists. Adding CJS support solely to improve the row count is outside
this phase.

Another 136 no-ESM artifact cases remain in six partial rows:

- @kobalte/utils@2.0.0-alpha.0|solid2|only — two type-source cases;
- @solid-primitives/sse@0.0.103|solid1|only and
  @solid-primitives/sse@1.0.0-next.2|solid2|floor and |head — three worker
  handler cases;
- @solidjs/diagnostics@2.0.0-rc.3|solid2|only — one vitest/side-effect case; and
- solid-js@1.9.14|solid1|only — 130 CJS cases.

The 130 CJS cases need a CJS/interop model. The remaining six need an explicit
type-only, worker, test-module, or side-effect applicability/execution model.
All remain unverified under the current executable ESM contract model.

Also retained unless Slice 14 establishes an exact model:

- @kobalte/solidbase@0.6.13|solid1|only — custom “?raw” loader semantics; and
- @solid-primitives/context@0.3.2|solid1|only — unauthenticated dependency
  layout in declaration resolution.

### Semantic refusals retained by construction

- arbitrary runtime loop/reentry cardinality that remains in the final main;
- dynamic or mixed export binding writes;
- an opaque external helper’s call result;
- ambiguous wildcard or package-condition selection;
- unresolved dependency cycles or non-exact versions;
- nonempty probe schedules without the pinned harness/runtime image;
- compiler-owned claims without exact source/output/options/tool evidence; and
- any positive or closure premise whose witness is incomplete or subject-mismatched.

## Verification matrix

| Changed seam | Focused checks | Corpus/gate before merge |
| --- | --- | --- |
| report/classifier | ecosystem report and classifier tests | regenerate Phase 20 ledger |
| proposal/resolution | backend contract process tests and CLI workflow tests | uncached affected-row probes |
| artifact snapshot/closure | certification and artifact-resolution tests | contract conformance and mutation suite |
| Type Facts protocol | typefacts and backend certification library tests | producer/client pin and protocol gates |
| finalizer/receipt/catalog | policy2 receipt, contract interface, process tests | issue-publish-load adversarial tracer |
| export kind | solid-facts, IR, backend process fixtures | tsc oracle plus affected-row probes |
| dependency composition | certification, export-all, nested install tests | bottom-up dependency corpus |
| fixtures/snapshots | focused coverage comparison | ownership gate and universal set |

For every semantic slice:

1. run the smallest test that proves the changed fact;
2. rebuild the fresh native checker only when source or bundled contracts
   changed;
3. point process/corpus checks at the fresh checker and stamped Type Facts
   producer;
4. run the affected probe IDs with gate cache disabled;
5. compare every refusal removed and every new demand introduced; and
6. update only the corresponding row ledger and focused snapshots.

The final full run uses SOLID_CHECKER_GATE_CACHE=0. A warm cache is useful
during iteration but is not evidence for the phase outcome.

## Commit strategy

Keep each commit independently green and avoid mixing row-count movement with
receipt authority changes:

1. live row ledger and classifier truth;
2. adversarial red tests;
3. subpath identity and normalized candidate subjects;
4. resolver parity and per-entrypoint census;
5. artifact applicability;
6. single-owner closure replay and archive-topology policy gate;
7. producer pins and opaque native transaction;
8. exact export-value protocol and reconciliation;
9. finalizer plus issuer/publish/load tracer;
10. value-only receipt cohorts;
11. cardinality pruning/replan;
12. runtime binding census;
13. geolocation reconciliation;
14. recursive dependency composition;
15. optional bounded artifact models; and
16. final uncached report and documentation.

Snapshot updates travel with the semantic commit that caused them. No commit
may update an expected report merely to bless unexplained movement.

## Definition of done

Phase 20 is complete when:

- all 418 current rows appear in a generated, reproducible disposition ledger;
- at least one real policy-2 receipt has completed the same-process
  plan/evidence/finalize/issue/authenticate/publish authority transaction, then
  been independently discovered, authenticated, selected, and queried by a
  separate fresh analyzer process;
- every row marked verified satisfies the full row definition above;
- every consulted Type Facts source is classified under verifier-authored,
  root-archive, authenticated-dependency, or producer-library provenance, with
  unknown/mutable sources refused;
- every mandatory receipt root, including an empty schedule/set, is
  verifier-derived, domain-separated, bound to the final graph, and covered by
  transplant/mutation tests;
- every one of the 32 value-only candidates and seven cardinality candidates
  has either a loaded receipt or an exact row-local semantic refusal;
- all currently affected dependency rows (the final corpus has 16 fully
  refused and eight partial rows) have traversed complete recursive dependency
  planning and either compose authenticated leaves or name the exact remaining
  leaf (including a source-bound module-loading frontier);
- all 15 full-failure export-kind rows, both partial export-kind rows, and
  geolocation have exact producer-backed terminal dispositions;
- every bounded subpath, normalized-subject, resolver, closure-role,
  unbound-global, branch-census, export-local identity, canonical-alias, and
  catalog defect identified here has a positive and negative regression test;
- missing published files, CJS-only surfaces, custom loaders, unsafe duplicate
  archive members, arbitrary cardinality, and other retained shapes are not
  counted as verified;
- no checker diagnostic duplicates a real published-typing TypeScript error;
- no receipt can be forged, transplanted between rows or demands, replayed
  after pruning, or loaded under the wrong importer/specifier; and
- make verify passes at the final handoff with exact generated artifacts
  reviewed.

The success metric is authenticated row coverage with named exact refusals, not
a target refusal percentage.
