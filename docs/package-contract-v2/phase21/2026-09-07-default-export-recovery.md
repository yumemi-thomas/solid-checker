# Default export recovery: scoped measurement

The default-declaration census fix in [ADR 0060](../../adr/0060-default-export-census.md)
adds 68 certified artifact selections across three scoped probes. Their previous
accepted sets were empty. All three move from refused to partial; none becomes
complete. The coverage metric is unchanged.

| Probe | Before accepted cases | After accepted cases | Accepted entrypoints |
| --- | ---: | ---: | --- |
| @kobalte/solidbase@0.6.13, Solid 1 | 0 | 66 | 33, enumerated in the evidence |
| @solidjs/vite-plugin@3.0.0-next.34, Solid 2 floor | 0 | 1 | `.` |
| @solidjs/vite-plugin@3.0.0-next.34, Solid 2 head | 0 | 1 | `.` |

The [exact machine-readable before/after evidence](2026-09-07-default-export-scoped-measurement.json)
follows each retained report row's published case-set pointer and named catalogs.
It records runtime/declaration hashes, resolution branches, importer coordinates,
catalog/document/receipt digests and receipt payloads. Ordinary consumer analysis
authenticated the receipts and selected the exact cases. Native archive census
equality and all behavior proofs remain required; removing an invented export
from the producer's census does not grant any semantic premise.

This is new certification, not a denominator correction. Solidbase has wildcard
exports and no accepted root; its client still needs the real
`virtual:solidbase/components` module and its route-config target is unavailable.
Vite Plugin's two remaining explicit exports select no runtime condition in
these probes. They do not count as certified cases or complete rows.

Focused artifact-resolution tests passed (76 tests). Full `make verify` passed
with actual exit 0, TOTAL 67.94s, and no `FAILED during step` marker in
`/private/tmp/default-export-verify.log`. No fixture snapshots changed in this
slice. The scoped run does not reverify preservation of the full baseline's 837
accepted selections; that requires the subsequent full-corpus comparison.
