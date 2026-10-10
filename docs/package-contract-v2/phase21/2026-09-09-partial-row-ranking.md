# Audit of all 64 partial rows

The latest completed full report is `2026-09-08-original-helper-full.json`,
finished September 8 at 23:27 JST: 327 complete, 64 partial, 18 refused and
9 not advanced. This audit changes none of those totals. It overlays the
separately consumer-verified SolidStart and Solid Devtools publications, including
the newly certified Devtools `./setup`, when identifying work still outstanding.

| Audit classification | Rows | Missing executable entrypoint occurrences |
| --- | ---: | ---: |
| Explicit, non-wildcard executable gaps | 14 | 36 |
| Only assets/types among missing entrypoints | 32 | 0 |
| Wildcard exports | 15 | 70 candidate gaps; separate denominator limits |
| Deliberately scoped probes | 3 | 121 outside-scope candidates; not failed certifications |
| Total | 64 | These columns are not a replacement coverage metric |

Counts are probe occurrences, not unique packages or implementations. Floor and
head rows remain separate. An accepted entrypoint is not proof of every condition
or every behavior: the audit separately retains uncovered conditional selections
on already accepted entrypoints. In particular, Devtools has only package.json
missing at entrypoint level but its browser/development selections remain open.

## Ranking by actual certification opportunity

The first rank prioritizes complete-row potential. Subsequent ranks favor bounded
new executable coverage and concrete evidence. Gains below are target ceilings,
not measured certifications or promises that a single patch closes the row.

| Rank | Target | Missing executable entrypoints | Existing-metric complete-row ceiling | Evidence and next bounded action |
| --- | --- | --- | ---: | --- |
| 1 | `@solid-primitives/sse` 0.0.103; 1.0.0-next.2 floor/head | `./worker-handler` in 3 rows | 3 | Only missing entrypoint in each row. The files install `self` message/connect listeners, manage connections and call `makeSSE`. The current refusal is no runtime ESM exports. Requires an effectful module-initialization claim and exact Worker/SharedWorker host behavior, not an inert/empty-export shortcut. High effort and proof-design risk; investigate feasibility before committing to implementation. |
| 2 | `@solid-devtools/shared@0.20.0` | `.` and `./index` (same zero-byte runtime file) | 0 | Best bounded next probe: the new inert-module proof already supports the observed runtime shape. Recertify exact cases and preserve the existing three entrypoints. Declaration closure could still refuse. Wildcard denominator and two other executable gaps prevent a complete row. |
| 3 | `@tanstack/solid-pacer@0.22.0` | `./types` | 0 | Parent re-exports `@tanstack/pacer/types`; the child runtime is `export { };`. Existing graph refused the child's empty export surface. First remeasure with current inert support; parent still needs positive initialization composition through the authenticated child. Package.json remains excluded. |
| 4 | `@solid-devtools/ui@0.10.3` | `.` | 0 | `SignalContextProvider` is assigned `SignalContext.Provider` after `createContext`. Refusal: export root not compiler-proved callable/constructable. Investigate exact receipt-bound factory-member identity; do not infer it from `Provider` spelling. CSS remains excluded. |
| 5 | `@solid-devtools/shared ./utils` and `./chunk-DTKGRNV6`; `@kobalte/utils@0.9.2 ./src/scroll-into-view.ts`; diagnostics `./playwright` | 4 entrypoints across 3 rows | 0 | Native audits identify missing positive original-input identity in `formatTime`, `scrollIntoViewport`, and `captureBrowserArtifact`. These are three distinct semantic investigations, not one presumed shared fix. Shared `formatTime` defaults to `new Date()`, so unconditional caller-origin proof would be wrong. |
| 6 | `@solidjs/web@2.0.0-rc.3` | `./serialization`, `./serialization/decode` | 0 | Both refuse non-callable root proof for `DEFAULT_WEB_PLUGINS`, an `Object.freeze([...])` result. The new object-literal rule does not prove this call. Needs exact intrinsic/result identity and independent member premises; host-backed plugins may expose later blockers. |
| 7 | `@tanstack/solid-router` Solid 1 and Solid 2 floor/head | `.` and `./ssr/server` in 3 rows | 0 | Six entrypoint occurrences; exact router-core dependency contracts are missing. Root generation names `DEFAULT_PROTOCOL_ALLOWLIST`; SSR names an external export-all. Prepare one bounded graph case first and inspect its native blocker. Package.json still prevents existing-metric completeness. |
| 8 | `@tanstack/solid-table@9.1.2` | `./static-functions` | 0 | Native refusal is now deeper than the generation export-all message: `cell_getIsAggregated` lacks the exact signature-census path `column.table.atoms.grouping.get`. Requires a positive generic/member premise, not more routing. Package.json remains excluded. |
| 9 | `@solidjs/start@2.0.3` | `./client` | 0 | Missing exact `solid-js/web.hydrate` binding. Recover through that exact dependency/importer context. `./env` remains declaration-only. |
| 10 | `@tanstack/solid-start` Solid 1 and Solid 2 floor/head | `.`, `./client`, `./client-rpc`, `./hydration`, `./server`, `./server-rpc`, `./ssr-rpc` in each row | 0 | 21 remaining entrypoint occurrences after the six scoped inert additions. Multiple dependency paths name Hydrate, hydration condition and server RPC exports. High graph breadth; work one dependency family at a time. Package.json remains excluded. |
| 11 | Other wildcard runtime gaps | See census below | 0 | Solidbase client needs virtual-module/compiler authority; debugger bundled/chunk cases lack exact callable member paths; Solid 1 raw CJS paths lack declaration/host support; JSX-runtime aliases have executable modules despite no selected runtime export surface. These are not asset exclusions. |
| 12 | Published Kobalte test modules and diagnostics `./vitest` | 41 Kobalte test entrypoints; one Vitest adapter | 0 | Files execute test registration or `expect.extend`; empty exports do not make them inert. Supporting test-host initialization is broader and lower priority than library API coverage. The exported test modules must stay in the wildcard census even if intentionally deferred. |

Only SSE offers a three-row transition under the unchanged metric from an
executable fix alone. It is also a substantial proof extension. The recommended
short first experiment is the two empty Shared entrypoints (rank 2), followed by
Pacer's exact inert dependency chain. These can establish useful certification
progress without pretending to increase complete-row totals.

## Wildcard rows

Eight wildcard rows have no missing executable entrypoint in the installed-file
census: Corvu-next utils, Corvu utils, Kobalte core 2, Kobalte utils 2, Solid h,
Solid universal, Corvu, and Solid 2 core. This does not certify their denominator
or all conditions. Solid 2 core still has unmatched server selections.

Seven wildcard rows retain executable gaps: Kobalte core 1 (41 test modules),
Solidbase (1 plus an unresolved selection), Kobalte utils 1 (1), Devtools
debugger (2 plus 27 unresolved selections), Devtools shared (4), Solid web (4),
and Solid 1 core (17). The detailed JSON preserves every name, selected target,
digest and recorded refusal. Debugger also has an unexpanded `./src/*.ts` branch;
an incomplete branch cannot be declared covered merely because other branches
expand successfully.

## Scope and denominator exclusions

The three scoped probes remain charts (`./solid` only), devtools-a11y (`./core`,
`./core/production`, `./solid`, `./solid/production`) and devtools-utils (`./solid`,
`./solid/class`). Their 121 outside-scope executable candidates are not failed
attempts and are excluded from the implementation ranking.

Two Vite-plugin rows previously looked like unresolved runtime selections. Their
manifest explicitly exposes `./boundary-modules` and `./virtual-solid-manifest`
only through `types` conditions selecting retained `.d.ts` files. They belong to
the 32 asset/type rows for this audit, not to the executable backlog. Other
declaration targets are identified by the actual selected `.d.ts/.d.mts/.d.cts`
file, not by names such as `./types`; Pacer's `./types` selects JavaScript and
therefore remains an executable gap.

## Evidence and limits

[Machine-readable audit](2026-09-09-partial-row-audit.json) contains all 64 rows,
baseline and scoped artifact cases, manifest digests, publication pointers and
catalog digests, finite condition selections, missing entrypoints, and native
refusals from each retained certification audit. The audit follows the published
pointer exclusively; it never collects arbitrary leftover catalogs.

[Reproduction script](prototypes/audit-partial-rows.mjs) verifies pointer/document,
catalog, main and receipt digests; checks installed manifests against accepted
main manifest hashes; reconciles every baseline accepted-entrypoint count; checks
scoped consumer-audit flags and preservation; and asserts exactly 64 rows.

This is an audit, not a new certification run or a new complete-executable metric.
Wildcard expansion uses retained installed files and records their hashes. No
fresh native archive-membership/inapplicability replay or receipt-signature
verification runs here. Existing scoped ordinary-consumer results are referenced,
not recreated. A future denominator metric must independently bind a complete
expected-case census to the authenticated archive and verify every exclusion.
The existing complete/partial/refused totals are unchanged; no performance or
certification-gain estimate is added to them.

Validation is the bounded audit with its digest/count/preservation assertions,
JavaScript syntax checking and `git diff --check`. Full `make verify` and the
ecosystem benchmark are intentionally not rerun for this documentary audit.
No production code, receipts, snapshots or existing publications were changed.

## Every partial row

Counts below are entrypoint occurrences in this audit, not exact condition-case completeness.
Scoped missing counts are outside-scope candidates. Wildcard counts require archive census replay.

| Probe | Class | Accepted entrypoints | Missing executable | Other missing |
| --- | --- | ---: | ---: | ---: |
| @corvu-next/utils@0.1.5 / solid2 / only | Wildcard | 17 | 0 | 0 |
| @corvu/utils@0.4.2 / solid1 / only | Wildcard | 17 | 0 | 0 |
| @kobalte/core@0.13.13 / solid1 / only | Wildcard | 508 | 41 | 11 |
| @kobalte/core@2.0.0-alpha.0 / solid2 / only | Wildcard | 61 | 0 | 0 |
| @kobalte/solidbase@0.6.13 / solid1 / only | Wildcard | 33 | 1 | 76 |
| @kobalte/utils@0.9.2 / solid1 / only | Wildcard | 22 | 1 | 1 |
| @kobalte/utils@2.0.0-alpha.0 / solid2 / only | Wildcard | 7 | 0 | 1 |
| @solid-devtools/debugger@0.28.1 / solid1 / only | Wildcard | 4 | 2 | 27 |
| @solid-devtools/frontend@0.15.4 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @solid-devtools/shared@0.20.0 / solid1 / only | Wildcard | 3 | 4 | 0 |
| @solid-devtools/ui@0.10.3 / solid1 / only | Executable gap | 3 | 1 | 1 |
| @solid-primitives/sse@0.0.103 / solid1 / only | Executable gap | 2 | 1 | 0 |
| @solid-primitives/sse@1.0.0-next.2 / solid2 / floor | Executable gap | 2 | 1 | 0 |
| @solid-primitives/sse@1.0.0-next.2 / solid2 / head | Executable gap | 2 | 1 | 0 |
| @solidjs/diagnostics@2.0.0-rc.3 / solid2 / only | Executable gap | 3 | 2 | 1 |
| @solidjs/h@2.0.0-rc.3 / solid2 / only | Wildcard | 3 | 0 | 2 |
| @solidjs/image@0.1.0 / solid1 / only | Assets/types | 2 | 0 | 3 |
| @solidjs/signals@2.0.0-rc.3 / solid2 / only | Assets/types | 1 | 0 | 1 |
| @solidjs/start@2.0.3 / solid1 / only | Executable gap | 11 | 1 | 1 |
| @solidjs/universal@2.0.0-rc.3 / solid2 / only | Wildcard | 1 | 0 | 2 |
| @solidjs/vite-plugin@3.0.0-next.34 / solid2 / floor | Assets/types | 1 | 0 | 2 |
| @solidjs/vite-plugin@3.0.0-next.34 / solid2 / head | Assets/types | 1 | 0 | 2 |
| @solidjs/web@2.0.0-rc.3 / solid2 / only | Wildcard | 9 | 4 | 29 |
| @tanstack/ai-devtools-core@0.5.8 / solid1 / only | Assets/types | 2 | 0 | 1 |
| @tanstack/charts@0.15.0 / solid1 / only | Scoped | 1 | 112 | 0 |
| @tanstack/devtools-a11y@0.2.2 / solid1 / only | Scoped | 4 | 4 | 1 |
| @tanstack/devtools-ui@0.7.1 / solid1 / only | Assets/types | 3 | 0 | 1 |
| @tanstack/devtools-utils@0.7.0 / solid1 / only | Scoped | 2 | 5 | 1 |
| @tanstack/form-devtools@1.0.0-alpha.2 / solid1 / only | Assets/types | 2 | 0 | 2 |
| @tanstack/hotkeys-devtools@0.9.0 / solid1 / only | Assets/types | 2 | 0 | 1 |
| @tanstack/pacer-devtools@1.4.0 / solid1 / only | Assets/types | 2 | 0 | 1 |
| @tanstack/solid-db@0.2.40 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-devtools@0.8.12 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-form@2.0.0-alpha.2 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-form-devtools@1.0.0-alpha.2 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-hotkeys@0.10.0 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-hotkeys-devtools@0.7.0 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-pacer@0.22.0 / solid1 / only | Executable gap | 13 | 1 | 1 |
| @tanstack/solid-pacer-devtools@0.14.0 / solid1 / only | Assets/types | 2 | 0 | 1 |
| @tanstack/solid-router@1.170.30 / solid1 / only | Executable gap | 1 | 2 | 1 |
| @tanstack/solid-router@2.0.0-rc.2 / solid2 / floor | Executable gap | 1 | 2 | 1 |
| @tanstack/solid-router@2.0.0-rc.2 / solid2 / head | Executable gap | 1 | 2 | 1 |
| @tanstack/solid-router-devtools@1.167.1 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-router-devtools@2.0.0-rc.2 / solid2 / floor | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-router-devtools@2.0.0-rc.2 / solid2 / head | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-router-ssr-query@1.167.2-pre.0 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-router-ssr-query@2.0.0-rc.2 / solid2 / floor | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-router-ssr-query@2.0.0-rc.2 / solid2 / head | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-start@1.168.47 / solid1 / only | Executable gap | 5 | 7 | 1 |
| @tanstack/solid-start@2.0.0-rc.2 / solid2 / floor | Executable gap | 5 | 7 | 1 |
| @tanstack/solid-start@2.0.0-rc.2 / solid2 / head | Executable gap | 5 | 7 | 1 |
| @tanstack/solid-start-client@1.168.29 / solid1 / only | Assets/types | 3 | 0 | 1 |
| @tanstack/solid-start-client@2.0.0-rc.2 / solid2 / floor | Assets/types | 3 | 0 | 1 |
| @tanstack/solid-start-client@2.0.0-rc.2 / solid2 / head | Assets/types | 3 | 0 | 1 |
| @tanstack/solid-start-config@1.120.20 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-store@0.11.1 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/solid-table@9.1.2 / solid1 / only | Executable gap | 3 | 1 | 1 |
| @tanstack/solid-table-devtools@9.2.0 / solid1 / only | Assets/types | 2 | 0 | 1 |
| @tanstack/solid-virtual@3.13.37 / solid1 / only | Assets/types | 1 | 0 | 1 |
| @tanstack/table-devtools@9.2.0 / solid1 / only | Assets/types | 2 | 0 | 1 |
| corvu@0.7.2 / solid1 / only | Wildcard | 9 | 0 | 0 |
| solid-devtools@0.34.5 / solid1 / only | Assets/types | 4 | 0 | 1 |
| solid-js@1.9.14 / solid1 / only | Wildcard | 20 | 17 | 30 |
| solid-js@2.0.0-rc.3 / solid2 / only | Wildcard | 2 | 0 | 15 |
