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
| Ecosystem proposal main bytes | 1626 | 4967 | 43055 |
| Ecosystem proposal-plan bytes | 49252 | 303349 | 3676782 |

Raw proof evidence retained by ordinary analysis: **0 bytes**.

## Performance

| Phase | p50 | p95 | max | Unit |
| --- | ---: | ---: | ---: | --- |
| Ecosystem generation | 1500 | 27026 | 470672 | ms / row |
| Current isolated runtime probe | 20.1 | 20.37 | 21.33 | ms / session |
| Proof-input generation | 1882541 | 14492958 | 14599958 | ns / accepted case |
| Verification and receipt | 22637459 | 203746333 | 205342583 | ns / accepted case |
| Accepted corpus load | 15683417 | 15814042 | 15912958 | ns / 24 cases |
| Normalized export query | 31 | 31 | 58 | ns / lookup |

The current probe row measures the stable-v1 driver with a deterministic witness in a fresh process, realm, and module instance. The Phase 0 418-row distribution remains a historical ecosystem execution-envelope reference. Neither measurement is acceptance authority, and stable-v1 proposals are never promoted by coverage or probe non-observation.

## Offline ordinary analysis

Ordinary queries receive only `AcceptedContractIndex`: no raw sidecars, package code execution, network access, or query-time file reads. Artifact acquisition and receipt validation terminate before the analyzer-facing index is constructed.
