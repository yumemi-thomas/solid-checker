# Marker first-receiver recovery

The seven-probe run finished on 2026-09-08 at 01:30:55 JST, exit 0, in
281.307 seconds. Its [catalog measurement](2026-09-08-initial-receiver-scoped-measurement.json)
compares the same probe identities and installed versions with the latest full
protocol-40 baseline. It follows published pointers and named catalogs, checks
their hashes, and requires ordinary consumer receipt authentication and exact
case selection. The runner's console summary counts generated contracts; the
coverage results below use the published certification catalogs.

| Probe | Accepted entrypoints before → after | Artifact selections | Row transition |
| --- | --- | ---: | --- |
| Marker 0.2.2, Solid 1 | empty → `.` | 0 → 1 | refused → complete |
| Marker 2.0.0-next.2, Solid 2 floor | empty → `.` | 0 → 1 | refused → complete |
| Marker 2.0.0-next.2, Solid 2 head | empty → `.` | 0 → 1 | refused → complete |
| Utils 6.4.1, Solid 1 | `.`, `./immutable` → same | 2 → 2 | complete → complete |
| Motion 0.6.0, Solid 1 | `.`, `./v1` → same | 2 → 2 | complete → complete |
| Motion 0.7.0-beta.4, Solid 2 floor | `.`, `./m`, `./v2` → same | 3 → 3 | complete → complete |
| Motion 0.7.0-beta.4, Solid 2 head | `.`, `./m`, `./v2` → same | 3 → 3 | complete → complete |

This is **three newly certified selections and three refused-to-complete
transitions** against the immediately preceding full baseline. The scoped set
moves from 4 complete / 0 partial / 3 refused to 7 complete / 0 partial / 0
refused, with 10 → 13 artifact selections. All ten existing selections and
their exported claim sets are [identical](2026-09-08-initial-receiver-claim-preservation.json).
No coverage denominator or metric changed. These recover support lost when the
original-input precision guard was introduced; they do not exceed the earlier
pre-correction full baseline.

All three new cases select `.` → `./dist/index.js`, with declarations at
`./dist/index.d.ts`, runtime branch `/exports/./import/default` and type branch
`/exports/./import/types`. Exact source hashes are:

| Version | Runtime SHA-256 | Declaration SHA-256 |
| --- | --- | --- |
| 0.2.2 | `c0655c82e93df388db6bb788c644f977b471395b3c29068585c049dbb14c048e` | `cca3a5710f94c6a37751458368bafb7d7fc47d5e0435ee94b8c0eb2f3b9d6081` |
| 2.0.0-next.2, both probes | `d201b2eee90a3ffded5b8c7543f258831aeecd1376f75a4ac6ca1e4b6e0a553d` | `e8560d5047a0042921888fce3752fdb8830089d05716be6228c6284ea4752693` |

The measurement retains the complete before/after selection objects, closure
digests, importer/specifier/resolution identities, receipt payloads, dependency
receipt and trust roots, witness roots and binary identities. Floor/head use
separate certification transactions and receipts even where package bytes
match. No receipt moves between contexts.

[ADR 0066](../../adr/0066-original-receiver-before-assignment.md) supplies the
missing positive premise: the first `search.trim()` receiver is the caller's
parameter, evaluated before assignment. The verifier binds its exact property
use to the same call and member path. This does not justify later reads of the
reassigned string or bypass the independent behavior proofs.

Focused Go producer tests pass; the two native/verifier tests pass in 16.64
seconds. Full `make verify` passes with actual exit 0, `TOTAL 243.12s`, and no
`FAILED during step` marker in `/private/tmp/initial-receiver-verify.log`.
Both ordinary-CLI [negative controls](2026-09-08-initial-receiver-negative-controls.json)
still refuse for missing positive original-input identity and publish no
catalog. No snapshots or bundled contracts changed; nothing was committed or
pushed.

The 418-probe corpus was not rerun for this narrow slice. Its latest measured
aggregate remains 318 complete / 61 partial / 30 refused / 9 not advanced and
1,459 accepted selections. Do not present the arithmetic projection of this
scoped gain as a new full result. Of that run's 30 losses against the earlier
baseline, these three are now recovered; the other 27 were not remeasured here.
I18n's conditional loop reads, Kobalte's branch/loop origins, and the duplicate
package compiler-context issue remain next targets. Their gains are unmeasured,
and the coverage ceiling is not established.
