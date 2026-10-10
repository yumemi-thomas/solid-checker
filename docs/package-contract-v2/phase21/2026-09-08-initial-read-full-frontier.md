# Full corpus after original-input read correction and Motion recovery

`2026-09-08-initial-reads-full.json` finished at 2026-09-08 00:57:05 JST,
actual exit 0, 1,522.584 seconds. It contains the same 418 probe identities and
installed versions as `implementation-owner-full`. The published-catalog
[measurement](2026-09-08-initial-reads-full-measurement.json) records exact
before/after artifact selections, hashes, declaration/resolution identities,
consumer authentication, receipts, and explicit losses.

| Metric | Earlier full baseline | Current full run |
| --- | ---: | ---: |
| Complete rows | 324 | 318 |
| Partial rows | 63 | 61 |
| Refused rows | 22 | 30 |
| Not advanced | 9 | 9 |
| Accepted artifact selections | 1,489 | 1,459 |

There are 30 lost selections across ten rows and no new selections against this
earlier baseline. Existing coverage is **not preserved overall**. Eight rows
become refused; two remain partial with fewer accepted cases. This is not a
denominator correction. The ordinary metric retains wildcard, asset/type and
scoped-probe distinctions.

| Probe(s) | Lost artifact selections | Transition |
| --- | ---: | --- |
| `@kobalte/utils@0.9.2`, Solid 1 | 20 | partial → refused |
| `@solid-devtools/shared@0.20.0`, Solid 1 | 2 (`./utils`, source and distribution) | partial → partial |
| `@solid-primitives/i18n@2.2.1`, Solid 1 | 1 (root) | complete → refused |
| `@solid-primitives/i18n@3.0.0-next.4`, Solid 2 floor/head | 2 (one root each) | two complete → refused |
| `@solid-primitives/marker@0.2.2`, Solid 1 | 1 (root) | complete → refused |
| `@solid-primitives/marker@2.0.0-next.2`, Solid 2 floor/head | 2 (one root each) | two complete → refused |
| `@solidjs/diagnostics@2.0.0-rc.3`, Solid 2 | 1 (`./playwright`) | partial → partial |
| `@tanstack/solid-db@0.2.40`, Solid 1 | 1 | partial → refused |

Motion recovery survives the full run. Both Solid 2 probes retain `.`, `./m`,
and `./v2`; Solid 1 retains `.` and `./v1`; Utils 6.4.1 retains root and
`./immutable`. The [full-run claim comparison](2026-09-08-initial-read-full-motion-claim-preservation.json)
confirms all 29 selections and exported claim sets from the nine-probe control
group remain identical to the pre-regression baseline. Thus the scoped recovery
is real, but it does not establish corpus-wide preservation.

## Next bounded proof work

`makeSearchRegex` first evaluates `search.trim()` on the caller's string in
`search = search.trim().replace(...)`. Later calls operate on the reassigned
string. A positive premise for the exact first receiver evaluated before the
assignment could recover three complete rows. It must bind that call's source
location and parameter path; an earlier whole-root read cannot justify a later
member call. The ordinary replacement and early-root/later-member negative
controls must remain refused. This is an implementation opportunity, not a
measured gain.

`resolveTemplate` assigns `string = string.replace(...)` inside a conditional
loop. Its origin proof needs the loop/branch execution distinction, not the
current opening-declaration prefix. `scrollIntoViewport` reads `targetElement`
in one branch and reassigns it while traversing parents in another. Neither may
be cleared from declared member shape alone. The remaining losses retain their
exact refusal records; inspect each before generalizing these proof rules.

## Graph-context investigation

Corvu's `contains` and `sortByDocumentPosition` produce both unwritten bindings
when their exact installed JavaScript files are queried directly, including
when queried under their published declaration signatures. The generated
`./dom` case IDs exactly match the failing graph cases (`f34410e...` and
`84d6a8c...`), ruling out a JSX-condition substitution.

A separate [producer-only reproducer](2026-09-08-duplicate-package-origin-evidence.json)
shows a relevant context failure: two installations of the same package name
and version share a compiler declaration in one program. A query for B names
A's declaration and therefore loses B's unwritten-input premise. Putting both
source roots first does not repair it. Separate programs for the two resolution
contexts preserve both exact identities. This does **not yet prove** the cause
of the actual Corvu graph refusal; capture that exact graph transcript before
changing preparation. Do not admit a foreign declaration, reuse its receipt, or
weaken the identity check. The investigation used temporary Go test overlays;
the measured source and binaries were unchanged.

Full `make verify` remains green for the measured implementation: actual exit 0,
`TOTAL 242.12s`, no failed-step marker. The full corpus run is measurement, not a
claim that every row certifies. No snapshots, bundled contracts, commits or
pushes changed in this follow-up. The coverage ceiling remains unproven.
