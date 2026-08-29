# Phase 16 corpus, compactness, and performance report

- Solid authority: `2.0.0-rc.3`
- Ecosystem: 40/418 complete, 318 partial (85.65% generatable)
- Solid Primitives: 6/291 complete, 268 partial (94.16% generatable)
- Receipt-issued cases preserved: 24
- Synthetic generator fixtures: 39

## Compactness

| Measure | p50 | p95 | max |
| --- | ---: | ---: | ---: |
| Canonical main bytes | 3342 | 16202 | 21538 |
| Pretty main bytes | 5943 | 27772 | 44703 |
| Normalized semantic debug bytes | 12514 | 77601 | 77687 |
| Proof evidence bytes | 348386 | 3122474 | 3209546 |
| Acceptance receipt bytes | 669 | 669 | 669 |
| Ecosystem proposal main bytes | 1599 | 4813 | 43055 |
| Ecosystem proposal-plan bytes | 49172 | 303349 | 3676782 |

Raw proof evidence retained by ordinary analysis: **0 bytes**.

## Performance

| Phase | p50 | p95 | max | Unit |
| --- | ---: | ---: | ---: | --- |
| Ecosystem generation | 1568 | 25439 | 434856 | ms / row |
| Current isolated runtime probe | 20.13 | 21.16 | 21.73 | ms / session |
| Proof-input generation | 1923458 | 14881292 | 15047792 | ns / accepted case |
| Verification and receipt | 22744667 | 206064250 | 207305125 | ns / accepted case |
| Accepted corpus load | 15886458 | 16152208 | 16240875 | ns / 24 cases |
| Normalized export query | 31 | 32 | 58 | ns / lookup |

The current probe row measures the temporary-v2 driver with a deterministic witness in a fresh process, realm, and module instance. The Phase 0 418-row distribution remains a historical ecosystem execution-envelope reference. Neither measurement is acceptance authority, and temporary-v2 proposals are never promoted by coverage or probe non-observation.

## Offline ordinary analysis

Ordinary queries receive only `AcceptedContractIndex`: no raw sidecars, package code execution, network access, or query-time file reads. Artifact acquisition and receipt validation terminate before the analyzer-facing index is constructed.
