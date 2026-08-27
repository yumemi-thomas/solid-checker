# Phase 8 completion report: proposal generator refactor

Date: 2026-08-27

Branch: `codex/phase8-proposal-generator-refactor`

Authority: the frozen package-contract-v2 semantic model, the Phase 6
temporary decoder, the Phase 7 exact artifact-resolution boundary, published
`solid-js@2.0.0-rc.3` and related `@solidjs/*` artifacts where behavior is
relevant, and the rule that only later proof replay may accept closure

## Result

Phase 8 items 92-100 are implemented for the replacement generator path.
`solid-facts-backend::proposal_generation` now owns semantic proposal
construction, proof planning, probe planning, fixed-point plan composition, and
deterministic proposal emission. `packages/cli/scripts/contract-proposal-pipeline.mjs`
owns the matching acquisition and process-orchestration path.

Generation without verification always ends as an open, unaccepted proposal.
The Rust emission says `acceptance: "unaccepted"` and has no main package-
contract bytes, accepted `closed` field, receipt, or evidence sidecar. The
existing public schema-v1 generator remains unchanged because Phase 14 owns the
atomic producer/consumer migration and legacy deletion.

## Explicit stages and module boundary

The replacement path has seven named stages:

1. package discovery in Node;
2. exact standalone artifact resolution in Node through the Phase 7 adapter;
3. semantic analysis in Rust;
4. proposal construction in Rust;
5. proof planning in Rust;
6. probe planning in Rust;
7. emission of Rust-produced bytes by Node.

The Node pipeline passes acquisition and Rust products through without reading
or rewriting semantic summaries. Every analysis, proposal, proof-plan, and
probe-plan result must carry the Rust process-product discriminator at the
orchestration boundary. The replacement JavaScript path has no condition-summary merge,
variant collapse, summary aliasing, or mutable shared semantic object.

This does not delete the legacy generator's schema-v1 JavaScript normalization;
that code remains live until every producer and consumer changes together in
Phase 14. Introducing a second compatibility decoder or partially switching
the public command here would violate the migration order.

## Exact artifact binding

`ProposalAnalysis` carries normalized semantic artifact cases plus independent
Phase 7 `ResolvedImport` records. Construction refuses:

- no exact resolution;
- zero or multiple matching artifact cases;
- stale package, manifest, runtime, declaration, transform, closure, entrypoint,
  or branch identity;
- incomplete resolution coverage across analyzed artifact cases;
- missing or out-of-closure exact export bindings.

The selected cases receive independently resolved runtime and declaration
export targets before proposal knowledge is opened. Opaque closure frontiers
retain Phase 7 locality and cannot be converted into negative behavior.

## Open knowledge and false-closure prevention

`ExportSemantics::open_proposed_closure` recursively visits all immediate
set-valued knowledge domains:

- call claims;
- tuple items, object properties, choice alternatives, and reactive/store
  capabilities at every recursive value leaf;
- operation owner productions and operation input/output shapes;
- resource states and capabilities;
- guard partitions and guarded operation selections.

For each locally complete domain it records the exact `ClaimPath`, then weakens
the semantic knowledge monotonically:

| Candidate before construction | Open proposal after construction |
| --- | --- |
| complete positive | partial positive with every known item retained |
| complete negative | unknown |
| partial positive | unchanged |
| unknown | unchanged |

No parent, child, sibling, export, artifact case, or unrelated claim domain is
opened by this transformation. A naturally unknown recursive leaf adds a local
unresolved edge while independently complete siblings remain closure candidates
for later proof replay.

## Operations, proof obligations, and probes

Every known operation survives proposal construction. Its operation kind and
cardinality-derived strength (`Possible` or `Guaranteed`) are emitted as a
positive candidate even when the containing call domain is open. This prevents
an unresolved edge from erasing useful positive behavior.

Proof planning creates three disjoint obligation families:

- `ProveClosure` for every withdrawn local completeness candidate;
- `ResolveOpenClaim` for naturally unresolved leaves that were not merely
  opened for later closure proof;
- `ProvePositiveOperation` for every positive operation candidate.

Probe planning selects only possible-positive operations. A probe plan can
witness that behavior occurred; it cannot accept closure, prove absence,
promote a finite maximum, or turn a possible operation into guaranteed
behavior.

Phase 9 will define stable semantic claim IDs. Phase 8 records structured
artifact-case/export/`ClaimPath` subjects only and intentionally does not assign
an ID derived from JSON position, summary name, or debug spelling.

## Fixed point and deterministic emission

`ProposalPlan::fixed_point` monotonically unions closure candidates, local
unresolved edges, positive operations, proof obligations, and probe candidates.
Duplicate or reordered analysis rounds therefore converge to the same ordered
plan. Mixing rounds with different semantic digests is refused.

Emission uses ordered Rust collections and the normalized semantic digest. The
same analysis emits byte-identical pretty JSON with one trailing newline. Its
document family is the internal proposal-plan envelope
`solid-checker-contract-proposal` version 1, independent of the main contract
wire version. It is not a proof/evidence sidecar format and is not consumed by
ordinary analysis.

## Tests

Focused Rust tests cover:

- false complete-positive and complete-negative closure withdrawal;
- retention of partial positive operations;
- local recursive uncertainty and known sibling preservation;
- unrelated closure candidates surviving an incomplete domain;
- separate closure, naturally unresolved, and positive-operation obligations;
- witness-only probe selection for possible operations;
- deterministic output with no `closed` key and explicit unaccepted status;
- idempotent, order-independent proposal fixed points and mixed-digest refusal;
- exact Phase 7 artifact selection and binding through the proposal boundary.

Focused Node tests cover:

- the exact seven-stage order;
- identity-preserving acquisition and Rust-product handoff;
- refusal of a semantic-stage result without the Rust process-product
  discriminator;
- live wiring to Phase 7 independent runtime/declaration standalone resolution.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, schema, protocol, build identity, or
fixture changed. No Solid compiler fork, compiler pin, execution-facts
protocol, semantic trace, identity document, or notice changed.

No bundled contract, package-contract fixture, analyzer snapshot, dialect
manifest, runtime lock, public schema, proof/evidence sidecar, receipt, or other
generated artifact changed. The internal deterministic proposal emission is
covered in memory by unit tests; no emitted proposal is checked in.

## Verification

Focused iteration:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib`
  — 168 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib`
  — 52 passed, including six proposal-generation tests;
- `bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs packages/cli/test/contract-proposal-pipeline.test.mjs`
  — one file and four tests passed.
- `bun run --cwd packages/cli test` — seven files and 60 tests passed, including
  TypeScript compilation;
- focused Clippy for `solid-reactive-ir` and `solid-facts-backend` with
  `-D warnings` — passed.

Final authority:

- `make verify` — passed in 36.28 seconds. The run included all workspace,
  Go race, Clippy, backend/WASM feature, compiler-identity, Type Facts stamp,
  coverage (94 projects and 557 findings), ownership (289 cases), performance,
  CLI, TypeScript oracle (333 tests and 161 two-sided cases), obligation,
  dialect-manifest, bundled-contract, registry-pin, and conformance gates.

## Exact remaining open or uncertifiable cases

- Proposal closure is never accepted. Phase 10 proof engines and Phase 11
  finalization/receipts must independently replay each obligation.
- Stable claim IDs and evidence/probe sidecar document families remain Phase 9.
- Runtime probes, falsification records, environment identity, timeouts, and
  sandbox policy remain Phase 9; this phase plans only possible-positive
  operation subjects.
- The proposal plan is an internal Rust-owned staging artifact, not the public
  temporary main schema-v2 document and not an accepted analyzer input.
- Ordinary analyzer consumers and bundled contracts remain on the legacy path
  until Phases 12-14, so current SC9005/SC9012 behavior and corpus findings do
  not change here.
- The public package generator, missing-contract generator, bundled generator,
  probe workers, review/verifier tools, WASM adapter, fixtures, and bundles are
  not cut over until Phase 14.
- The legacy JavaScript normalizer and schema-v1 condition/variant mechanics
  remain only on that scheduled-to-be-deleted public path; the replacement
  pipeline contains none of them.
- All runtime and semantic evidence gaps listed in the Phase 5 Solid 2
  conformance matrix remain open unless earlier Type Facts, compiler facts, or
  Phase 7 artifact identity already established the exact positive premise.
- Nonliteral dynamic loading, `eval`, native addons, opaque WASM, mutable
  unbound globals, unmaterialized transforms, unaccepted dependencies, invalid
  export maps, ambiguous export identity, and incomplete artifact coverage
  remain exact fail-closed/refusal domains.

## Handoff

Branch: `codex/phase8-proposal-generator-refactor`

The exact commit and solid-checker pull-request URL are recorded after
publication. No upstream Solid pull request is created.
