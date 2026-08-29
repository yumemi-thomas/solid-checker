# Next package-contract design

Status: **implemented on temporary schema version 2; stable public version 1
cut pending**.

This documentation set specifies the machine-first replacement for the current
package-contract schema. The current implementation, bundled contracts, and
public documentation continue to describe the legacy schema until the migration
plan reaches its atomic cut. Do not use this directory as evidence that an
existing contract has already been accepted under the new proof policy.

Phase 14 made the replacement the only producer and analyzer path, Phase 15
bounded and adversarially hardened it, and Phase 16 measured its corpus,
compactness, and performance gates. The stable public version-1 cut remains a
later atomic phase. Incomplete behavior remains locally open; no coverage
target may weaken proof requirements.

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
- [Phase 8 proposal-generator completion](phase8/2026-08-27-proposal-generator-refactor.md)
  — Rust-owned open proposal construction, local proof/probe planning,
  deterministic fixed points, and Node-only acquisition/orchestration.
- [Phase 9 claim-ID and evidence-sidecar completion](phase9/2026-08-27-claim-ids-evidence-sidecars.md)
  — position-independent semantic claim IDs, separate proof/probe evidence
  families, bidirectional hash binding, and raw-sidecar-free analysis.
- [Phase 10 runtime-probe completion](phase10/2026-08-27-authoritative-runtime-probes.md)
  — exact artifact-mode matrices, semantic event transcripts, isolated bounded
  repeat runs, positive-witness/closure-falsification authority, and local
  refusal without negative proof.
- [Phase 11 proof/receipt completion](phase11/2026-08-27-proof-checker-receipts.md)
  — replayable claim-local proof families, authority separation, verified-only
  closure, canonical proof roots, receipts, and content-addressed storage.
- [Phase 12 analyzer-integration completion](phase12/2026-08-28-analyzer-integration.md)
  — receipt-gated loading, exact import/export identity, demand-shaped guard
  queries, local open-domain diagnostics, native precedence, and cache identity.
- [Phase 13 Solid 2 RC.3 conformance completion](phase13/2026-08-28-solid2-rc3-conformance.md)
  — sixteen exact-artifact normalized cases, finite dependency-closure
  censuses, proof/probe expectations, six-way fixture coverage, and replayed
  published declarations/runtime bytes without the Phase 14 public cutover.
- [Phase 14 atomic migration completion](phase14/2026-08-28-producer-consumer-migration.md)
  — temporary-v2 producer/consumer cutover, receipt-only analyzer loading,
  regenerated bundles and fixtures, and removal of legacy normalization.
- [Phase 15 adversarial-hardening completion](phase15/2026-08-28-adversarial-hardening.md)
  — bounded document families, false-closure mutation gates, graph and path
  attacks, and deterministic semantic fuzzing.
- [Phase 16 corpus, compactness, and performance completion](phase16/2026-08-28-corpus-compactness-performance.md)
  — exact RC.3 and ecosystem coverage, local refusal records, wire/evidence
  size distributions, accepted load/query costs, and offline-analysis gates.
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
