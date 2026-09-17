# Executable gap reconciliation after the machine full run

The completed `2026-09-09-machine-full.json` remains the measurement authority:
327 complete, 64 partial, 18 refused and 9 not advanced. The refreshed audit
follows its retained publication pointers and verifies document, receipt and
manifest digests. It does not issue certificates or change the denominator.

| Partial-row class | Rows | Missing executable entrypoint occurrences |
| --- | ---: | ---: |
| Explicit executable gaps | 13 | 35 |
| Wildcard exports | 15 | 68 installed-file candidates |
| Assets/types only | 33 | 0 |
| Deliberately scoped | 3 | 121 outside-scope candidates |

There are 103 in-scope executable occurrences in this census, not 144.
Wildcard rows also have 28 unresolved selections, kept separate rather than
classified as executable or inapplicable without evidence. The previous audit
had 36 explicit and 70 wildcard executable occurrences. Pacer `./types` and
Shared `.`/`./index` account for the three recovered occurrences; these are
already included in the machine full run and are not new gains from this audit.
Conditional selections on accepted entrypoints remain separately recorded.

## Devtools UI root: declaration-resolution blocker confirmed

A bounded offline certification of `@solid-devtools/ui@0.10.3` root, using
the full run's retained package and the pinned release checker, exited 1.
Audit: `/private/tmp/ui-root-qs8eP7/audit.json`.
Log: `/private/tmp/probe-ui-root-cached.log`.
The first refusal remains `callable-path` for `SignalContextProvider`:
the export root is not compiler-proved callable or constructable.

TypeScript 5.9.3 was run against the actual installed `dist/index.d.ts`, with
strict checking, `module: esnext`, `moduleResolution: bundler`, and no skipped
library checking. `solid-js/types/reactive/signal` resolves to no module.
The exported provider has zero resolved call signatures. The declaration file
produces TS2307 at lines 3 and 4 for that import. Solid's published wildcard
target is `./types/*`, while this extensionless import does not select the
existing `types/reactive/signal.d.ts` under Bundler resolution.

This establishes a declaration defect under the configured resolution mode,
not permission to infer a callable value from the runtime member spelling.
Do not add a checker diagnostic duplicating TS2307 or change resolution modes
solely to recover this case. A runtime factory-member proof, if pursued later,
must independently bind the exact dependency and member semantics.

## SolidStart client: virtual-module authority is also required

The refreshed retained audit for `@solidjs/start@2.0.3 ./client` already records
a deeper graph-preparation blocker than the generation message about hydrate:
`solid-start:app` is not installed above `dist/client/StartClient.jsx`.
This is a framework virtual module. Fetching an npm archive or merely providing
the hydrate binding cannot discharge that closure. No redundant probe was run.

The highest existing-metric complete-row opportunity remains SSE's three
`./worker-handler` cases. Those execute Worker/SharedWorker listener setup and
need positive effectful initialization and host premises. They are not inert
modules. Other candidate families remain ranked in the earlier partial-row
ranking, with Shared and Pacer removed from the outstanding work.

## Validation and limits

The refreshed auditor completed with all 64 rows and its pointer/digest/count
assertions passing. The bounded UI certification refused as described; no
new accepted case or row transition is claimed. Existing publications, source,
interfaces, receipts and snapshots were not changed. Full `make verify` was
not rerun for this audit-only work. The installed-file wildcard census is not
an authenticated archive census or a replacement complete-executable metric.

Evidence: [refreshed audit](2026-09-09-machine-partial-row-audit.json) and
[reproduction](prototypes/audit-machine-partial-rows.mjs).
