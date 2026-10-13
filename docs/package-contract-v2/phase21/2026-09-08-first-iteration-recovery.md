# I18n first-iteration recovery

The ten-probe run finished on 2026-09-08 at 01:52:40 JST, actual exit 0,
294.447 seconds. The [measurement](2026-09-08-first-iteration-scoped-measurement.json)
uses the latest full report for I18n and the later Marker recovery report for
the seven controls, with both input report hashes recorded. It follows the
published pointers/named catalogs and verifies exact artifact selections and
ordinary consumer receipt authentication. No arbitrary leftover catalog is
counted.

| Probe | Accepted entrypoints before → after | Artifact selections | Transition |
| --- | --- | ---: | --- |
| I18n 2.2.1, Solid 1 | empty → `.` | 0 → 1 | refused → complete |
| I18n 3.0.0-next.4, Solid 2 floor | empty → `.` | 0 → 1 | refused → complete |
| I18n 3.0.0-next.4, Solid 2 head | empty → `.` | 0 → 1 | refused → complete |
| Marker 0.2.2, Solid 1 | `.` → same | 1 → 1 | complete → complete |
| Marker 2.0.0-next.2, each Solid 2 probe | `.` → same | 1 → 1 each | complete → complete |
| Utils 6.4.1, Solid 1 | `.`, `./immutable` → same | 2 → 2 | complete → complete |
| Motion 0.6.0, Solid 1 | `.`, `./v1` → same | 2 → 2 | complete → complete |
| Motion 0.7.0-beta.4, each Solid 2 probe | `.`, `./m`, `./v2` → same | 3 → 3 each | complete → complete |

The scoped comparison moves from **7 complete / 0 partial / 3 refused** to
**10 complete / 0 partial / 0 refused**, and 13 → 16 accepted artifact
selections. All 13 prior selections and exported claim sets are
[identical](2026-09-08-first-iteration-claim-preservation.json). The three I18n
selections are new certifications against their immediately preceding measured
state, recovering earlier precision-guard losses. No denominator changed.

The new cases all select `.` → `./dist/index.js`, paired with
`./dist/index.d.ts`, runtime branch `/exports/./import/default` and types branch
`/exports/./import/types`:

| Version | Runtime SHA-256 | Declaration SHA-256 |
| --- | --- | --- |
| 2.2.1 | `2c19ca3658dc8b46bd7cc85d7ecae000dedc26f4f8c7358b9e94956ff63e36d2` | `53cd8dd5b1f1887902628504abf93111a7b5f15359991f48e67ec317628dfa03` |
| 3.0.0-next.4, both probes | `1304777a3c87a88b776ca592310466214ad369bc94dd4a220f140a0d806e0d14` | `6f8f93a9cb9af6425f924e08a3715bd0e14ebfeb6f2eeb5a754e5ebd1b800ba7` |

The complete selection objects, closures, importer/resolution identities,
receipt payloads, dependency receipt/trust roots, witness roots and binary
identities are retained in the measurement. The floor/head transactions have
distinct receipts. [ADR 0067](../../adr/0067-first-iteration-input-origin.md)
binds the exact first-loop receiver and call path to the original input, with
an explicit first-iteration limitation. Only a zero-lower-bound operation can
consume that premise; later iterations and guaranteed calls gain no authority.
Other behavior proofs remain independent.

Focused Go producer, packed native/verifier (two tests, 22.54 seconds), and
client-validation tests pass. Full `make verify` passes with actual exit 0,
`TOTAL 240.40s`, and no `FAILED during step` marker in
`/private/tmp/first-iteration-verify.log`. Both ordinary-CLI
[replacement controls](2026-09-08-first-iteration-negative-controls.json)
still refuse for missing original-input identity and publish no catalog.
No snapshots or bundled contracts changed; no commit or push was made.

The 418-probe aggregate has not been rerun: its latest measured state remains
318 complete / 61 partial / 30 refused / 9 not advanced, 1,459 selections.
Marker and I18n together have now recovered six of its 30 historical losses in
scoped runs. The other 24 are not remeasured by this comparison; do not report
an arithmetic projection as a new full result.

A separate [Kobalte Utils recovery attempt](2026-09-08-kobalte-utils-independent-measurement.json)
finished at 01:49:46 JST, exit 0, 21.639 seconds. It remains refused with zero
accepted cases. Enabling recovery alone is insufficient: graph recovery fails
on `./src/scroll-into-view.ts:scrollIntoViewport`, then its fallback retries the
whole unaccepted generated proposal. That same case blocks the other generated
cases again. `executeNativeOrGraphCertification` currently applies independent
case recovery only when no graph was prepared, not in its graph fallback.

The next bounded change is to apply fresh independent certification to that
unaccepted fallback proposal, preserve any pre-existing publication, and record
the exact remaining graph and case refusals. Publication-state protection must
be captured before the attempt: the failed native transaction itself creates
the catalog directory. A directory created by this attempt is not a previously
accepted case set. No gain from this change is measured yet. Batch the next
full-corpus rerun after this workflow correction; the coverage ceiling remains
open.
