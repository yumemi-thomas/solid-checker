# Next package-contract design

Status: **approved design; not yet the behavior of the current analyzer**.

This documentation set specifies the machine-first replacement for the current
package-contract schema. The current implementation, bundled contracts, and
public documentation continue to describe the legacy schema until the migration
plan reaches its atomic cut. Do not use this directory as evidence that an
existing contract has already been accepted under the new proof policy.

The replacement has one overriding objective: certify package behavior
accurately while allowing the checker to verify a large package corpus without
human review. Incomplete behavior remains locally open; no coverage target may
weaken proof requirements.

## Authority

Published `solid-js@2.0.0-rc.3` and related `@solidjs/*@2.0.0-rc.3` runtime
artifacts and declarations are authoritative where available. Older Solid 1
behavior, early Solid 2 betas, the RC.0 skill baseline, prose RFCs, and current
RC.0 bundled examples cannot override those bytes.

## Documents

- [Implementation plan](implementation-plan.md) — exhaustive sequencing,
  deliverables, gates, and exit conditions.
- [Architecture](architecture.md) — module ownership, interfaces, dependency
  direction, and failure locality.
- [Resolved design decisions](decisions.md) — concrete answers to the review's
  previously open implementation questions.
- [Semantic model](semantic-model.md) — knowledge lattice, operations,
  recursive values, guards, ownership, and invariants.
- [Wire format](wire-format.md) — frozen compact version-2 document shape and
  normalization rules.
- [Phase 1 semantic and policy freeze](phase1/2026-08-27-freeze.md) — closure
  record for the knowledge, operation, guard, ownership, evidence, digest, and
  stability decisions that define semantic-model version 1.
- [Proof, evidence, and receipts](proof-and-evidence.md) — proof authority,
  sidecars, acceptance, and adversarial obligations.
- [Fact-producer plan](fact-producer-plan.md) — required Type Facts and Solid
  compiler execution-fact improvements.
- [Compiler and Type Facts bootstrap](compiler-and-typefacts-bootstrap.md) —
  early migration from DOM Expressions to Solid's compiler, followed by Type
  Facts repatriation into this repository.
- [Type Facts repatriation conformance](typefacts-repatriation/2026-08-27-conformance.md)
  — import identity, cross-pair protocol parity, finding parity, performance,
  and completed external-repository retirement gate.
- [Phase 3 Type Facts completion](phase3/2026-08-27-typefacts.md) — exact
  invocation transcripts, local completeness, and remaining flow refusals.
- [Phase 4 compiler-facts completion](phase4/2026-08-27-compiler-facts.md) —
  semantic trace 3, compiler-facts protocol 2, identity chain, scope proof, and
  remaining compiler/runtime boundary.
- [Phase 5 normalized-model completion](phase5/2026-08-27-normalized-semantic-model.md)
  — wire-independent knowledge, operation, resource, guard, ownership, value,
  artifact, validation, and canonical-digest semantics.
- [Phase 6 temporary-wire completion](phase6/2026-08-27-temporary-wire-schema-v2.md)
  — strict schema-v2 decoding, private normalization, goldens, cross-field
  validation, and bounded expansion without producer or consumer cutover.
- [Phase 7 artifact-resolution completion](phase7/2026-08-27-artifact-resolution-closure.md)
  — exact host/Type Facts/standalone resolution, independent runtime/types
  bindings, canonical dependency closure, local opaque frontiers, and strict
  artifact-case selection.
- [Compiler execution-facts protocol](phase4/compiler-execution-facts.md) —
  operation identity, independent execution axes, completeness, normalization,
  and acceptance invariants.
- [Solid 2 conformance matrix](solid2-conformance-matrix.md) — required RC.3
  behaviors and evidence expectations.
- [Migration and verification](migration-and-verification.md) — corpus
  migration, temporary version 2, stable version 1, gates, and measurements.
- [Baseline](baseline.md) — reproducible quantitative and architectural
  starting point, backed by the checked Phase 0 benchmark artifacts.
- [Phase 0 benchmark artifacts](../../benchmarks/package-contract-v2/phase0/README.md)
  — raw uncached measurements, exact RC.3 audit, row classifications, fixture
  freeze, and replay commands.
- [Sub-agent reports](subagent-reports/README.md) — preserved independent review
  evidence used to arrive at the design.

## Governing decision

The accepted architectural decisions are recorded in
[ADR 0001](../adr/0001-machine-verified-package-contracts.md),
[ADR 0002](../adr/0002-colocate-type-facts-with-the-checker.md), and
[ADR 0003](../adr/0003-follow-the-solid-next-compiler.md), and
[ADR 0004](../adr/0004-freeze-package-contract-semantic-model-v1.md).

## Relationship to existing RFCs

[RFC 0002](../rfcs/0002-machine-verified-contracts.md) remains a historical
description of the current generator/probe promotion path. This design replaces
its trust model for the new format: probes never prove negative behavior, and a
generator cannot close a claim domain. Existing package-contract documentation
remains current until the migration completes.
