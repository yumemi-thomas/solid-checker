# Full measurement: export census and bounded case recovery

Baseline: `2026-09-07-independent-cases-full.json`, 418 probes,
322 complete / 60 partial / 27 refused / 9 not advanced, 837 accepted cases.

After: `2026-09-07-retry-sources-full.json`, finished 2026-09-07
22:22:55 JST, actual exit 0, 1,216.599 seconds. No probe timeout or memory-limit
failure was recorded. The unchanged entrypoint metric reports **324 complete /
63 partial / 22 refused / 9 not advanced**.

There are **651 new artifact cases across 546 new per-row entrypoints**,
bringing the accepted set to **1,488 cases across 387 certified rows**.
Every one of the previous 837 cases across 382 rows is preserved, including
runtime/declaration hashes, resolution branches and multiplicity. Existing
Motion floor/head selections are included in that comparison.

## Exact accepted-set changes

The [machine-readable before/after sets](2026-09-07-retry-sources-measurement.json)
enumerate all artifact selections, hashes, importer/resolution coordinates,
published case-set pointers, catalogs, receipt digests and receipt payloads.
Large entrypoint sets are enumerated there rather than abbreviated into a
different denominator. Rows not listed below gained no artifact cases.

| Probe | Cases before → after | Entrypoints before → after | Row transition |
| --- | ---: | --- | --- |
| Kobalte Core 0.13.13, Solid 1 | 0 → 576 | empty → 507, including `.` | refused → partial |
| Solidbase 0.6.13, Solid 1 | 0 → 66 | empty → 33 | refused → partial |
| Locator 0.16.7, Solid 1 | 0 → 2 | empty → `.` | refused → complete |
| Devtools Shared 0.20.0, Solid 1 | 5 → 8 | `./detect`, `./primitives`, `./utils` → those plus `./theme` | partial → partial |
| Devtools UI 0.10.3, Solid 1 | 2 → 3 | `./animation`, `./theme` → those plus `./icons` | partial → partial |
| Solid Router 2.0.0-next.18 | 2 → 3 | `./fs`, `./server` → those plus `.` | partial → complete |
| Vite Plugin 3.0.0-next.34, Solid 2 floor | 0 → 1 | empty → `.` | refused → partial |
| Vite Plugin 3.0.0-next.34, Solid 2 head | 0 → 1 | empty → `.` | refused → partial |

Shared adds its source-condition `./primitives` case and both distribution and
source cases for `./theme`. UI adds `./dist/icons/index.js`, paired with its
exact declaration file. Router adds the `solid`-condition root at
`./dist/index.jsx`, paired with `./dist/index.d.ts`. Locator adds two root
selections of the published `./dist/server.js`: `/exports/./browser/import`
and `/exports/./import`. The evidence contains their exact hashes and receipts.

**The two complete-row transitions are under the existing entrypoint-name
metric, not complete coverage of every conditional artifact.** Locator's
browser/development case still refuses because it has no locally closed
semantic claim. Router's default-import root still refuses at `useLocation`'s
open index value path. Neither refusal was removed or counted as certification.
Wildcard, asset/type and intentionally scoped probes keep their old treatment.
No new coverage metric or denominator correction was introduced.

## Implementation and consumer-bound premises

- [ADR 0060](../../adr/0060-default-export-census.md): a named default declaration
  exposes `default`; its local name is public only when explicitly exported.
  Native archive replay already enforced this. Correcting the producer's census
  permits Solidbase and Vite Plugin's exact exports to reach existing proofs.
- [ADR 0059](../../adr/0059-independent-artifact-case-recovery.md): large unaccepted
  proposals use binary subdivision, bounded to 1,024 cases and at most twice
  the case count in native transactions. Complete selected cases retain their
  claims; the union is freshly verified and published. Empty selections,
  incompatible unions, non-proof failures and shrinking an existing publication
  remain errors. The separate graph recovery bound remains 32 cases.
- [ADR 0061](../../adr/0061-compiler-source-retry-isolation.md): repeated source
  acquisitions get fresh scratch directories. This prevents directory collisions
  from falsely withholding exact lock-selected dependency types, including
  `solid-js`. Actual acquisition/identity failures remain fail-closed.

Every new selection underwent native snapshot replay, exact importer and
condition binding, source/semantic proof, applicable dependency-receipt and trust
checks, and ordinary consumer receipt authentication and exact case selection.
No receipt crossed importer contexts or moved from a private trial to publication.
The measurement follows published pointers and named catalogs, checking their
digests; it does not inventory arbitrary retained catalog leftovers.

## Validation and remaining work

Artifact-resolution tests: 76 passed. Final contract-workflow tests: 83 passed.
The retry regression failed before the patch and passes afterward, while a real
later acquisition failure still withholds the package. Final `make verify`:
exit 0, `TOTAL 81.04s`, no `FAILED during step` marker, log
`/private/tmp/retry-sources-verify.log`. The full corpus and exact preservation
comparison both exited 0. These CLI slices changed no fixture snapshots,
protocol, receipt format or bundled contracts. Nothing was committed or pushed.

The first Kobalte subdivision run published 503 cases; source retry isolation
raised that scoped result to 576 while preserving all 503. Its remaining case
is `./src/index.tsx`, where a namespace export can be confused with a same-named
callable member during proposal generation. The aborted intermediate
`batched-cases-full` attempt exited 130 and contributes no aggregate counts.
The final full run uses a declared 1,800-second probe limit, versus the earlier
baseline's 300 seconds, to accommodate the measured large-case transaction.
That resource-window change is separate from certification and metric semantics.

Other measured blockers remain: Shared's `mutate_filter`/`splice` path, UI's
`SignalContextProvider`, Solidbase's application virtual module and missing
published route target, Vite Plugin's non-runtime condition selections, plus
the callback/dependency, indexed value, CommonJS and runtime-library frontiers
on the [active work queue](2026-09-07-coverage-ceiling-plan.md). The overall
coverage ceiling is not established; no unmeasured further gain is claimed.
