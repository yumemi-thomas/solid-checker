# ADR 0151: A certified claim may cite a dependency's compiled-in acceptance

- Status: accepted and implemented (2026-09-28); written with the implementation.
  The owner decided on 2026-09-28 that a dependent package's certification may
  cite a dependency's accepted, environment-matched claim as proof.
- Date: 2026-09-28
- Owners: the citation (`accepted_bundles.rs` `cite_compiled_in`,
  `diagnostics.rs` `compiled_in_citation`,
  `contract_certification/citations.rs`), the composition
  (`dependencies.rs` `VerifiedDependencyComposition::from_citations`,
  `finalization.rs` `cited_composition`), the receipt binding
  (`policy2_receipt.rs` `cited_acceptances`), admission (`accepted_bundles.rs`
  `withdrawn_compiled_in_citation`, `contract_interface.rs`), the generation
  adapter (`generate-package-contract.mjs` `citeCompiledInDependencies`, native
  `--cite-compiled-in`), the lane decision (`certify-contract.mjs`
  `citedFrontierRecords`) and the bundler (`bundle-accepted-contracts.mjs`
  `withdrawUncarriedCitations`)
- Relation: a second source of the authority `AcceptedDependencyComposition`
  demands, beside the graph transaction's own receipts. Admission is ADR 0123's
  rule as amended by ADRs 0126 and 0131, replayed from the dependent's tree;
  host partitioning is ADR 0140's. No producer change: no handshake protocol,
  no Type Facts field. No contract-format change.

## Context

### Where a dependency export stops a wrapper today

A package that imports another is certified in one of two lanes.

- **The published-graph lane** (`--dependency-graph-lane`, entrypoint
  recovery) acquires every dependency the root's closure reaches, generates it,
  and certifies it in the same transaction
  (`certify_graphs_with_recipe_gating`). The root is regenerated against a
  private catalog of those proposals (`mergeProposalDependencies`), so its
  closure states an `AcceptedDependencyEdge` per dependency, and
  `PublishedContractGraphPlan::authenticate_dependency_receipts` composes the
  receipts the transaction issued.
- **The plain lane** has no dependency identities to supply. The resolver and
  the certifier's replay (`record_external`) leave every external specifier as
  an `UnacceptedExternalDependency` hazard, which opens **every domain of every
  export** of the case. Even an edge supplied by hand could not certify:
  finalization requires a `VerifiedDependencyComposition` for any edge
  (`DependenciesRequired`), and only a graph transaction could construct one.

So the wall is **evidence admission**: there was no channel through which an
already-accepted claim could be authority for a dependent. Attribution follows
from it (the hazard is how the missing authority shows), and the proposal
(what the generator states given the authority) is downstream of both.

### Which host it bites

Measured 2026-09-28 (`make primitives-checkpoint`, release build of `cca097d3`):
under `node` the published graph refuses at the `solid-js@2.0.0-rc.9` node
(wall 1 of the checkpoint), 60 rows fall back to the plain lane, and every
`@solid-primitives` dependency becomes a frontier. `@solid-primitives/utils`
alone blocked 265 exports in 46 packages, and 353 exports were attributed to
the graph refusal. Host free and under `browser` the graph lane composes the
same dependencies in its own transaction, and utils blocked 9 exports (the
plain-lane rows `keyed` and `share`).

### What the motivating chain actually needs

`@solid-primitives/media`'s `createMediaQuery` returns the accessor
`createHydratableSignal` (utils) returns, and utils' returns what `createSignal`
returns. Citation makes utils' accepted claim available to the certifier; it
does not make that claim stronger than it is. In the compiled-in tier utils'
`createHydratableSignal` closes `reads` alone (host free and `browser`) or
nothing (`node`), because its body registers an `onSettled` computation and
calls the caller's `update`. No tier document closes a non-empty `creates`
(332 closed, all empty), and the `creates` census discharges a dependency
callee only when its `creates` is closed empty (`census_dependency_claim`;
anything else is the `create-publishing-callee` decline). The `returns` census
has no arm for the result of a dependency call. Those are the next walls for
this chain, and this ADR moves none of them.

## Decision

**A certification outside any graph transaction may discharge a dependency
edge of its verified closure by citing the receipt this build's compiled-in
tier carries for exactly that dependency artifact and contract, admitted in the
dependent's own installed tree. The dependent's receipt names every receipt it
cites, and is admitted nowhere once the running build's tier no longer carries
one of them.**

### The citation

For each edge `{specifier, packageName, artifactCase, acceptedContractDigest}`
of the replayed closure (`cite_closure_dependencies`), the certifier looks up
the compiled-in tier (`cite_compiled_in`) and cites an acceptance only when all
of these hold:

1. **The artifact.** The bundle's specifier is the edge's, and its export
   conditions are the dependent's resolution conditions, exactly (with
   `import`). Another set resolved another closure: a `node` dependent cites
   only a `node` bundle, a host-free one only a host-free bundle (ADR 0140).
2. **The claim.** The bundle's receipt certifies the document the edge names:
   its signed semantic digest is the edge's `acceptedContractDigest`, and its
   one artifact case is the edge's. Another document about the same bytes, from
   another environment or another build, is another claim and is never cited.
3. **The environment, per host.** Steps 1-3 of the one admission rule
   (`admission_refusals`), replayed with the dependent's installed location as
   the project: the dependency Node finds from there reproduces the signed
   acceptance root (name, manifest version, lockfile integrity, entrypoint,
   conditions); its files reproduce the signed `snapshotRoot` and no patch
   record names it (ADR 0131); its environment is reproduced from its own
   location, edge by edge where the receipt states edges (ADR 0126), by the
   all-lookups rule where it does not (ADR 0123). A patched dependency, a
   different version or integrity, a differing environment entry, all refuse.
4. **Authentication.** The receipt is re-authenticated as a built-in receipt
   against the bundle's pinned digest.

Two admitted acceptances of the one document are both true in the tree; the
one resting on fewer installed packages is cited, the receipt digest breaking
a tie. One edge with no citation leaves the plan with no dependency authority,
and finalization refuses naming the edge (`CitationRefused`).

### What a citation proves, and nothing more

`VerifiedDependencyComposition::from_citations` answers every
`AcceptedDependencyComposition` demand of the plan exactly as a graph receipt
does, with the cited document where a node's certified candidate stands:

- the `DependencyArtifact` demand by the cited receipt itself (checks 1-4);
- a `creates` census obligation on the dependency by an **empty** `creates`
  the cited document closes and the receipt lists among its closed claims, and
  an inherited-closure obligation by the domain the cited document closes,
  both as in the graph;
- a factory-return obligation names a graph node's importer, which a citation
  has none of, so it refuses.

On the plain lane the Type Facts census has no dependency plans, so it records
no dependency obligation: a call into a cited dependency is refused by the
`creates` census (`census_call_disposition`), and every domain whose census
follows a callee into the dependency stays open. What the citation removes is
the opaque frontier; the censuses of the dependent's own body then decide the
rest, exactly as they do when the graph lane supplies the same edge. No
transitive trust is added: the dependent trusts exactly the receipts it cites,
each receipt's own dependencies are premises of its signed environment, and the
dependent's environment gains the cited dependency and every entry that
environment states, with the lookups that reach them replayed from the
dependent's tree.

The authority is membership in this build's tier, so there is no issuer,
trust-store or verifier-build comparison with the certifying transaction, and
the composition states no verifier build (`DependencyVerifierBuildMismatch`
cannot arise from a citation). The witness names the cited receipt
(`compiled-in-citation:<receipt digest>`), the cited artifact, and binds the
acceptance root, environment root and installed integrity into its evidence
root; the dependency receipts and trust roots commit to the same rows.

### The receipt names what it cites

`Policy2ReceiptBindings.citedAcceptances` is a sorted, unrepeated list of
`{packageName, packageVersion, receiptDigest}`. It is framed into the signed
payload under `cited-acceptances:v1` only when non-empty, so every receipt that
cites nothing keeps its exact bytes and no receipt, catalog or tier moves.
Validation refuses an unsorted or repeating list, so one statement has one
encoding.

### Admission withdraws with the tier

A receipt citing a receipt the running build's tier does not carry (the same
digest, package and version) is admitted nowhere
(`withdrawn_compiled_in_citation`):

- by artifact, in both tiers, as step 1b of `admit_by_artifact`, reported as
  `AdmissionRefusal::CitationWithdrawn`;
- by importer, a project catalog entry is read as absent;
- in the tier itself, an index carrying a bundle whose citation it does not
  carry refuses whole, and the bundler drops such a bundle by name first,
  transitively (`withdrawUncarriedCitations`).

A tier regeneration that re-certifies a dependency into a different receipt
therefore withdraws every contract built on the old one until those are
certified again.

### The adapter and the lanes

`contract generate` asks the native checker (`--cite-compiled-in`, answered by
the function certification uses) which acceptance it could cite for each bare
specifier the case's closure left as a frontier, and re-prepares the case with
those as accepted dependencies. It never cites a specifier the case re-exports
(a citation carries no per-export bindings, so an `export *` through it would
forward nothing), the runtime foundation, a Node builtin or a non-package
specifier, and never inside a graph transaction. A re-preparation that fails
keeps the original. The answer is a hint; certification replays every citation
from the tree. `SOLID_CHECKER_COMPILED_IN_CITATIONS=0` turns it off.

A citation must not hide a frontier from the published-graph lane, which
composes the same dependency in its own transaction and may prove more than the
tier does. The refusal audit records each citation (`citedDependencies`), and
`certify` treats a cited edge as a graph frontier coordinate
(`citedFrontierRecords`). The graph regenerates the root against its own
catalog; when the graph cannot be prepared, the reused proposal, the one that
cites, is what certifies.

## Consequences

Measured 2026-09-28 with `make primitives-checkpoint` (release build, the
checked-in tier `3d0197b4`, not regenerated), against the same run at
`cca097d3`:

| | before | after |
| --- | ---: | ---: |
| packages at criteria 1-3 | 1 | 1 |
| exports clean in all three hosts | 92 | 92 |
| clean host free / `browser` / `node` | 95 / 95 / 92 | 95 / 95 / 92 |
| per-export buckets changed, any host | | 0 |
| rows on the published-graph lane, host free / `node` | 63 / 1 | 63 / 1 |
| wall "unaccepted dependency: `@solid-primitives/utils`" | 265 exports, 46 packages | **0** |
| wall "graph lane refused at `solid-js` rc.9 (`node`)" | 353 exports, 61 packages | 105 exports, 16 packages |
| wall "unaccepted dependency: `@solid-primitives/event-listener`" (`node`) | 65 | 80 |
| `node`: returns / callbacks / reads / creates never proposed | 28 / 44 / 1 / 6 | 276 / 292 / 209 / 89 |

- **No export and no package moved.** Every export the citation reaches is
  held by another wall once the frontier is gone, the same walls it hits host
  free: under `node` the utils wall was hiding the missing claim forms, the
  recipe and census walls, exactly as the checkpoint's `node` column warned.
  `node` now measures the lane the other two hosts measure, less the rows that
  still reach an uncited dependency.
- **`event-listener` grew by 15** because it is not in the tier for
  `solid-js@2.0.0-rc.9` under `node` (its bundles were certified in rc.3
  environments), so exports that cited utils now stop at it instead. One
  uncited dependency still opens every domain of every export of its case,
  because the frontier hazard is case-wide.
- `@solid-primitives/media` under `node` cites utils and rootless (receipts
  `sha256:5079faf6…` and `sha256:cf6a19b8…`), certifies with an edged
  environment, and passes self-admission; its exports stay behind
  `event-listener` and `static-store`.

Tests: `accepted_bundles` (`a_citation_names_the_admitted_acceptance_of_the_exact_contract`,
`a_citation_is_refused_where_the_environment_differs`,
`a_patched_dependency_is_never_cited`, `an_unaccepted_claim_is_never_cited`,
`an_index_carrying_a_bundle_whose_citation_it_dropped_refuses`,
`an_acceptance_citing_a_receipt_the_tier_no_longer_carries_is_withdrawn`),
the composition seam
(`a_dependency_edge_composes_only_from_a_citation_of_its_exact_contract`), the
receipt binding
(`cited_acceptances_are_signed_and_citing_nothing_keeps_the_older_bytes`), the
adapter (`contract-workflow.test.mjs`, "ADR 0151") and the bundler
(`bundle-accepted-contracts.test.mjs`, "ADR 0151").

## Still open

- **The tier is the ceiling.** A citation reaches only what the tier carries
  for the exact environment: 68 of the 97 packages have no bundle, and
  `event-listener`, `static-store` and others have none for rc.9 under `node`.
  Regenerating the tier from the checkpoint corpus is the lead's step
  (checkpoint step 7); after it, citations reach further with no code change.
- **Non-empty composition.** A dependency callee whose `creates` closes with
  items, and a return of a dependency call's result, have no census arm. A
  cited claim would be the evidence for both (for `returns`, an ADR 0113 kind
  exactly as strong as the cited return), and neither is implemented here.
- **Per-export attribution of the frontier.** An uncited dependency opens
  every export of its case, including exports that never reach it
  (`sortBreakpoints` stays behind `event-listener`).
- **Re-exports** through a cited dependency are left to the graph lane,
  because a citation carries no per-export bindings.
- **The graph lane does not cite**: it composes in its own transaction, and a
  dependency node whose tier bundle is admitted is still re-certified there.

## Amendment (2026-09-28): ADR 0155

A citation now names the claim by content, not by receipt digest:
`citedAcceptances` entries carry the cited receipt's `artifactAcceptanceRoot`,
`dependencyEnvironmentRoot` and `semanticDigest` beside its digest (frame
`cited-acceptances:v2`), and a tier carries the citation while some bundle
states those three for the same package and version. Measured on two local
regenerations of the tier from the checkpoint corpus: every one of the 466
content triples reappeared and none of the utils receipt digests did, because a
receipt binds the certifying run's own importer path, so the digest identity
withdrew all 44 citing bundles at every regeneration. With the content identity
a third regeneration kept all 44 and dropped none.
