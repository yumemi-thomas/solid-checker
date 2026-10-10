# ADR 0114: A domain for the computations a call registers on its caller's owner

- Status: accepted and implemented (2026-09-23); written with the implementation
- Date: 2026-09-23
- Owners: the semantic model (`contract_semantics.rs`, `validate.rs`,
  `canonical.rs`), the wire (`contract_document.rs`,
  `schema/solid-reactivity.schema.json`), the generator's owner-requirement
  publication (`inferred_contract.rs`), the certifier's owner witness
  (`type_facts.rs`), and the consumer's owner-requirement projection
  (`contracts.rs`)
- Relation: the repair `semantic-model.md` § creates' 2026-09-03 decision names
  for a free-standing owner requirement, option (c) of
  `phase21/2026-09-03-implementation-census-plan.md` § 2.1, reopened there for
  exactly this case. **No producer change**: no handshake protocol, no Type
  Facts field.

## Context

An export that must be called under an owner *because it registers a
computation on that owner* — the analyzer's `OwnerRequirementOperation::Effect`,
the fact behind `SC4001` — had no home in version 1. It is not a `create`: that
domain is registrations into a runtime *outside* the invocation, and
registering on the caller's owner is exactly what the audits publish beside
`creates: []` **closed**. The `requires`/`requiresChildren`/`requiresCleanup`
triple is a field of whichever operation needs the owner, so a requirement with
no operation of its own had nowhere to go. The generator therefore **withheld**
it by name, and a consumer could never get `SC4001` from a generated dependency
contract. The corpus withheld 14, all of this role.

The withholding had a second cost, found the same day: a consumer reads a closed
`creates` as "no owner requirement beyond the published items"
(`project_owner_requirements`), and only the `creates` walk kept the domain open
beside a withheld requirement — by declining a call it had no row for. A
`createTrackedEffect` call has one, so a caller could close `creates` and read to
a consumer as needing no owner. The generator now closes `creates` only where
every requirement it found is stated (`docs/precision-backlog.md`, 2026-09-23);
this ADR is what lets it state the `Effect` one.

## Decision

**A tenth call domain, `computations`, whose one operation kind is `compute`: the
registration of a reactive computation on an owner the operation does not
create. Version 1 states it by item only.**

### What a `compute` states

Its owner relation is the claim: `source: ambient-at-call`, `requires:
required`, `requiresChildren: required`. What it *produces* stays unknown rather
than empty: the registered computation is itself an owner node, and no witness
exists for a resource axis, so neither "produces nothing" nor a named child
owner is something this generation could prove. `validate_call_claims` refuses a
`compute` that does not require an owner it does not create
(`Operation::imposes_owner_requirement`) and child owners of it, and holds the
kind rule both ways: a `compute` must be listed in `computations`, and only a
`compute` may be.

### Item only

The domain's negative would be "registers no computation on an owner it does
not create". No census decides it, and no consumer reads it: owner-requirement
completeness stays `creates`' closure. So:

- `closed` and `proposedClosures` may not name it — `WireCallDomain` has no
  variant for it, the schema's `callDomain` does not list it, and validation and
  `close_verified_claim` refuse a closed one;
- `ClaimDomain::CLOSABLE` is every domain but this one, and an open domain is an
  *unresolved claim* only there, so an unknown `computations` adds nothing to any
  document's unresolved claims: nothing could resolve it;
- a document that does not mention it says nothing.

### Canonical identity

A contract in which some export states a `computations` item writes the
length-delimited marker `solid-checker:semantic-computations:v1` first, and the
domain after `disposals` in each export's claims. Every other contract hashes
exactly the stream it hashed before; `semantic_model_v1_digest_algorithm_and_golden_vector_are_frozen`
still holds and `computations_digest_family_is_separate_and_frozen` pins the new
family. A `compute` is the operation kind tag 8 and the domain tag 9; neither can
occur outside the family.

### The generator, the certifier, the consumer

- `owner_requirement_operation` publishes an `Effect` requirement as a `compute`
  in `computations`. Only `Boundary` is still withheld — it is a compiler-lowering
  fact the implementation census does not record — so `WithheldOwnerRequirement`
  keeps that one variant. With the requirement stated, a clean walk may close
  `creates` again beside it.
- The certifier witnesses a `compute` exactly as it witnessed the resourceless
  `create` this shape replaces: `require_owner_operation_call`, keyed on
  `requiresChildren`, finds the archive's own reachable, uncaptured call of an
  `Effect`-role primitive (`createEffect`, `createRenderEffect`,
  `createTrackedEffect`, per the dialect's `owner_requirement_role`), under the
  operation-reachability floor. A cleanup registrar's call does not witness it.
- `project_owner_requirements` reads `computations` for its items only, as it
  reads `cleanups`, and projects a `compute` as `Effect`. Owner requirements are
  known where `creates` is closed; with `creates` open, the items it states still
  project and `creates` is what reports open.
- `withhold_operations` opens `creates` whenever it withdraws an operation that
  imposes an owner requirement on the caller, since the closure is the
  consumer's completeness signal. A refused `compute` or `cleanup` then leaves
  the import uncertifiable rather than reading as needing no owner. Before this
  the same hole existed for a refused `cleanup` requirement.

## What remains

- `Boundary` requirements are withheld, and `creates` stays open beside them.
- A requirement the owner census does not find is not stated, and owner-requirement
  completeness is still borrowed from `creates`' closure, which counts every
  registration into an *outside* runtime and says nothing by itself about the
  caller's owner. It is sound only because the generator closes `creates` where
  every requirement it found is stated, and the census walked every call.
- The two frozen Solid 1.x authority documents keep their `ambient-at-call`
  `create`; the consumer still reads it.
- The ecosystem benchmark's row-kind table (`contract-content.mjs`) does not
  count `compute`: adding a key moves the pinned sentinel report's shape, which is
  a separate step.

## Consequences

- Contract corpus: 6 of 99 fixtures move, each only by its withheld `Effect`
  requirement becoming a `compute`: withheld claims 14 → 0, possible operations
  441 → 455, local open claims 6,192 → 6,247 (each `compute` carries its open
  operation axes). `signals-reexport-creates`' `trackEach` also proposes `creates`
  now (proof candidates 1,552 → 1,553), its walk having been clean all along, and
  its refusal sidecar is empty and is removed.
- `fixtures/reactive-ir/package-computation-consumer` is the consumer's half, on
  an authorized contract: an unowned call of an export stating a `compute` is
  `SC4001`, the same call inside a component is clean, and the same export
  stating none reports nothing (coverage: 83 projects, 442 findings; the
  stable-main ledger moves to 140). Unauthorized, the fixture reports nothing.
- No existing coverage finding moves, and no document that states no
  `computations` item changes its digest, its unresolved claims, or its receipt.
