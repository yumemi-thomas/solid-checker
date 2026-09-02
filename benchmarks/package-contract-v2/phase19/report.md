# Phase 19 authenticated policy and refusal-leverage report

- Ecosystem rows: 418 (52 complete proposals, 324 partial, 42 full refusals, 0 not applicable)
- Policy-2 receipts: 0/73 baseline receipt documents
- Policy-2 verified exports: 0/8984 proposal exports
- Policy-1 migration: 0 reissued, 73 retired/demoted, 0 pending
- Structurally complete rows attempted: 52/52

No acceptance target weakens proof. The current zero issuance count is a result: the mandatory live producer and probe authorities are incomplete, so policy 2 cannot authenticate any ecosystem row yet.

## Refusal owner queue

| Order | Owner | Rows | Disposition |
| ---: | --- | ---: | --- |
| 1 | accepted-dependency-composition | 16 | open-no-policy2-dependency-receipts |
| 2 | type-facts/export-kind-census | 9 | open-producer-evidence-required |
| 3 | type-facts/export-kind-reconciliation | 1 | open-producer-evidence-required |
| 4 | type-facts/parameter-behavior | 0 | open-producer-evidence-required |
| 5 | artifact-resolver/export-identity | 0 | open-resolution-repair-required |
| 6 | artifact-resolver/finite-wildcard-census | 9 | finite-wildcard-subset-remeasured-with-deeper-exact-refusals; other artifact shapes open |
| 7 | artifact-model/no-esm-surface | 7 | retained-refusal |

Finite wildcard census support was remeasured across the 418-row corpus. Its 5 historical rows now expose deeper exact refusals and unlock zero verified exports; 7 no-exported-surface rows remain refusals.

## Measurement availability

Open proposal main bytes are measured (376 samples), and proof-input cost is measured for 52 snapshot-bound certification attempts. Verification, receipt, accepted-load, accepted-query, accepted main, proof, sidecar, and receipt distributions retain zero samples where the exact missing live authority stopped the transaction; null percentiles are reported instead of fabricated zero costs.

## Trust boundary

Ordinary analysis consumes no audit transcript, open proposal, raw evidence, registry response, or package execution. Every active policy-2 count remains zero until an authenticated receipt closes the exact demand graph.
