# Ecosystem machine-verification report

How many real ecosystem packages machine-verify end to end: `contract generate` -> `contract probe --write` -> `contract verify`, run against a throwaway install of every probe row in the pinned corpus.

> **This measurement executes package code.** `contract probe` imports and runs each
> installed package, and its dependencies, in child processes. Every install and every
> execution happened inside temporary directories under the harness state directory, npm
> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran
> under both a per-mode timeout and a whole-phase wall budget.

- Started: 2026-08-22T23:40:15.832Z
- Finished: 2026-08-22T23:46:45.650Z
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Probe rows run: 416
- Checker native binary: `27edf9e078e65d78c5442d61b72485079a8f307636d8d496672de2740e0d5426` (14493248 bytes, mtime 2026-08-22T23:32:43.475Z)
- Type Facts binary: `0fe187a2884a0326d07dd36520b856b0e5c272c41e63a8fd65282dfb256d31a7` (28369538 bytes, mtime 2026-08-22T23:32:43.507Z)
- Budgets: install 240000 ms, generate 120000 ms, probe 20000 ms per condition mode / 120000 ms whole phase, verify 90000 ms; concurrency 6

## Headline

| Figure | Count |
| --- | --- |
| Probe rows run | 416 |
| Reached a generated contract | 409/416 (98.32%) |
| **Reached `verified`** | **194/416 (46.63%)** of all rows |
| Reached `verified`, of rows that produced a contract | 194/409 (47.43%) |
| Refused by `contract verify` | 210/416 (50.48%) |

Outcome classes, raw:

| Outcome | Rows |
| --- | --- |
| `refused` | 210 |
| `verified` | 194 |
| `generate-failure` | 4 |
| `install-failure` | 3 |
| `probe-timeout` | 3 |
| `probe-error` | 2 |

## Per family

| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 23 | 7/23 (30.43%) | 14 | 1388/3151 (44.05%) | 1357/1388 (97.77%) | 36 | 23 | 47 |
| Kobalte | 6 | 4 | 1/6 (16.67%) | 2 | 398/922 (43.17%) | 398/398 (100.00%) | 3 | 7 | 3 |
| Solid Primitives | 289 | 288 | 175/289 (60.55%) | 113 | 2014/3412 (59.03%) | 1910/2014 (94.84%) | 340 | 390 | 381 |
| Corvu | 28 | 28 | 7/28 (25.00%) | 21 | 290/426 (68.08%) | 285/290 (98.28%) | 0 | 21 | 0 |
| TanStack | 52 | 50 | 3/52 (5.77%) | 46 | 1140/2243 (50.82%) | 960/1140 (84.21%) | 0 | 6 | 0 |
| Solid Devtools | 12 | 10 | 1/12 (8.33%) | 9 | 124/364 (34.07%) | 124/124 (100.00%) | 0 | 2 | 0 |
| Solid Recharts | 3 | 3 | 0/3 (0.00%) | 3 | 136/365 (37.26%) | 107/136 (78.68%) | 0 | 0 | 0 |
| Motion for Solid | 3 | 3 | 0/3 (0.00%) | 2 | 549/561 (97.86%) | 545/549 (99.27%) | 0 | 0 | 0 |

| Solid target | Rows | Contracts | Verified | Refused |
| --- | --- | --- | --- | --- |
| solid1 | 168 | 163 | 70/168 (41.67%) | 91 |
| solid2 | 248 | 246 | 124/248 (50.00%) | 119 |

## Why verification refuses

210 rows were refused. `contract verify` raises every blocker it finds rather than stopping at the first, so the row counts below sum to more than the number of refused rows.

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |
| --- | --- | --- |
| `kind-observed` | 136 | 351 |
| `probe-report-includes-evidence-write` | 122 | 122 |
| `probe-failed` | 84 | 353 |
| `incompleteness` | 60 | 1091 |
| `closure-note` | 7 | 32 |

Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a *consequence*: `contract probe --write` declines to write evidence once a probe failed or an incompleteness was reported, so verification then sees passing claims that never reached the contract. It is counted as a root cause only on a row where it stands alone.

| Root cause | Refused rows |
| --- | --- |
| `probe-failed` | 84 |
| `kind-observed` | 82 |
| `incompleteness` | 42 |
| `closure-note` | 2 |

## Drivability

| Figure | Count |
| --- | --- |
| Claims planned across every probed contract | 11444 |
| Driven | 6039/11444 (52.77%) |
| Passed | 5686/11444 (49.69%) |
| Failed | 353 |
| Undriven | 5405/11444 (47.23%) |
| Incompleteness findings | 1091 |

Undriven claims by reason:

| Reason | Claims |
| --- | --- |
| entrypoint import threw | 1281 |
| no probe form: reactiveReads | 1122 |
| probe session failed (process died) | 700 |
| no probe form: ownerRequirements | 527 |
| no probe form: parameter identity | 386 |
| synthesized call threw | 354 |
| synthesized call did not invoke the callback | 255 |
| no probe form: nested return leaf | 227 |
| no plantable reactive source | 151 |
| no probe form: asyncBehavior | 100 |
| probe session wrote no report | 90 |
| no unambiguous summary for the mode | 83 |
| probe session hit the per-mode timeout | 53 |
| no probe form: callback arguments | 25 |
| no probe form: store path | 23 |
| callback ownership ambiguous in the driver's read scope | 20 |
| planted write was never re-read | 8 |

## The probe environment

An entrypoint whose module cannot be imported yields no observation at all. 56 of the corpus's rows had at least one entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or TypeScript loader, and only the packages the corpus manifest installs beside the probed one. Some of these throws are facts about the package; others are facts about that environment, and the two are not separated here.

| Import failure | Claims left undriven |
| --- | --- |
| ReferenceError: window is not defined | 432 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@solidjs/web' imported from <path> | 248 |
| Error [ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING]: Stripping types is currently unsupported for files under node_modules, | 227 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@solid-primitives/utils' imported from /private/t | 94 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: Package subpath './web' is not defined by "exports" in <path> | 81 |
| Error: [solid-devtools]: Debugger hasn't found the exposed Solid Devtools API | 67 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'server-only' imported from <path> | 60 |
| TypeError [ERR_UNKNOWN_FILE_EXTENSION]: Unknown file extension ".jsx" for <path> | 27 |
| SyntaxError: The requested module 'solid-js' does not provide an export named 'onSe | 10 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'react' imported from <path> | 6 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vite' imported from <path> | 6 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: No "exports" main defined in <path> | 4 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@angular/core' imported from <path> | 4 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@rsbuild/core' imported from <path> | 3 |
| Error [ERR_UNSUPPORTED_ESM_URL_SCHEME]: Only URLs with a scheme in: file, data, and node are supported by the  | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find module '<path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'preact' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'svelte' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vue' imported from <path> | 2 |
| ReferenceError: document is not defined | 1 |

## Conversion volume

A conversion replaces one export's whole claim domain with the `{"status":"unknown"}` sentinel because the probe neither observed nor statically proved it.

| Figure | Count |
| --- | --- |
| Claim domains converted to unknown | 379 |
| Exports carrying an unknown in the verified rows, at generation | 150/880 (17.05%) |
| Exports carrying an unknown in the verified rows, after verification | 431/880 (48.98%) |

How much a verified contract actually certifies from observation:

| Figure | Count |
| --- | --- |
| Verified rows carrying at least one probed behavioral row | 6/194 (3.09%) |
| Probed behavioral row markers kept across the whole corpus | 12 |
| Inferred row markers dropped by verification | 1118 |
| Probed markers discarded as unwitnessed by this run's report | 11 |

Converted domains by field:

| Field | Conversions |
| --- | --- |
| `returns` | 217 |
| `callbacks` | 152 |
| `asyncBehavior` | 10 |

## The composite a consumer feels

Of every export the corpus's generated contracts describe:

| State | Exports |
| --- | --- |
| (a) certified by a verified contract | 449/9015 (4.98%) |
| (b) honest unknown inside a verified contract | 431/9015 (4.78%) |
| (c) inside a contract that never reached `verified` | 8135/9015 (90.24%) |

(c) is every export of a contract that was generated and then refused, timed out, or errored before a probe report existed. Rows whose `npm install` or `contract generate` failed describe no exports at all and are in none of the three states.

## Wall time

| Phase | Rows | Median | p90 | Max | Mean |
| --- | --- | --- | --- | --- | --- |
| install | 416 | 457 ms | 1484 ms | 22218 ms | 825 ms |
| generate | 413 | 102 ms | 535 ms | 15324 ms | 403 ms |
| probe | 409 | 753 ms | 3224 ms | 120005 ms | 3308 ms |
| verify | 404 | 42 ms | 52 ms | 72 ms | 43 ms |
| pipelineWithoutInstall | 413 | 937 ms | 3964 ms | 135329 ms | 3721 ms |
| total | 416 | 1574 ms | 5109 ms | 135821 ms | 4519 ms |

`install` may run against a warm npm cache, so it is a lower bound; `pipelineWithoutInstall` is the number that describes the checker's own cost.

## Rows that never reached verification

| Stage | Rows |
| --- | --- |
| `npm install` failed | 3 |
| `contract generate` failed | 4 |
| `contract probe` errored before writing a report | 2 |
| timed out under the harness budget | 3 |

Probe errors by cause:

| Cause | Rows |
| --- | --- |
| no installed solid-js beside the package | 2 |

Generation failures by class:

| Class | Rows |
| --- | --- |
| `no-esm-runtime-target` | 2 |
| `cjs-only-entrypoint` | 1 |
| `no-exported-surface` | 1 |

Timeouts, named individually because a timeout is never a verification result:

- `@kobalte/core@0.13.13|solid1|only` — probe-timeout after 135821 ms
- `@tanstack/solid-table@9.1.2|solid1|only` — probe-timeout after 126670 ms
- `motion-solidjs@0.7.0-beta.4|solid2|head` — probe-timeout after 122332 ms

## Caveats, stated because these numbers are easy to over-read

- **`verified` is not `reviewed`.** A verified contract certifies what a machine observed or statically proved and converts everything else to the unknown sentinel. It is a weaker claim than the human `reviewed` tier, and a stronger one than the `inferred` draft the generation benchmark measures.
- **The install environment is the corpus manifest's, and it was built for static generation.** It installs the probed package and the Solid runtime versions the manifest selected — not the package's full peer set. Several `ERR_MODULE_NOT_FOUND` import failures above are that gap, not the package's.
- **A timeout is never a verification result.** Rows that exceeded the probe wall budget are their own outcome class and are counted as neither verified nor refused.
- **Per probe row, not per package.** A package with a Solid 1.x row and two Solid 2.x rows contributes three rows to every figure here.
- **This measurement executed package code.** Nothing here is a safety claim about any package; it is a record of what happened when each one was imported and driven in a sandboxed child process.

## Every refusal

| Probe | Family | Root cause | Blocker lines | Classes |
| --- | --- | --- | --- | --- |
| `@corvu-next/accordion@0.1.5|solid2|only` | corvu | `incompleteness` | 3 | incompleteness, probe-report-includes-evidence-write |
| `@corvu-next/calendar@0.1.5|solid2|only` | corvu | `incompleteness` | 2 | incompleteness, probe-report-includes-evidence-write |
| `@corvu-next/drawer@0.1.5|solid2|only` | corvu | `incompleteness` | 2 | incompleteness, probe-report-includes-evidence-write |
| `@corvu-next/focus-trap@0.1.5|solid2|only` | corvu | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@corvu-next/list@0.1.5|solid2|only` | corvu | `incompleteness` | 3 | incompleteness, probe-report-includes-evidence-write |
| `@corvu-next/otp-field@0.1.5|solid2|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu-next/popover@0.1.5|solid2|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu-next/resizable@0.1.5|solid2|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu-next/tooltip@0.1.5|solid2|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu-next/utils@0.1.5|solid2|only` | corvu | `incompleteness` | 11 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@corvu/accordion@0.2.5|solid1|only` | corvu | `incompleteness` | 2 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/calendar@0.1.2|solid1|only` | corvu | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@corvu/dialog@0.2.4|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/disclosure@0.2.2|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/drawer@0.2.4|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/otp-field@0.1.4|solid1|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu/popover@0.2.0|solid1|only` | corvu | `incompleteness` | 5 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@corvu/resizable@0.2.5|solid1|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu/tooltip@0.2.2|solid1|only` | corvu | `incompleteness` | 5 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@corvu/utils@0.4.2|solid1|only` | corvu | `probe-failed` | 7 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@kobalte/core@2.0.0-alpha.0|solid2|only` | kobalte | `incompleteness` | 101 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@kobalte/utils@0.9.2|solid1|only` | kobalte | `incompleteness` | 17 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@solid-devtools/debugger@0.28.1|solid1|only` | solid-devtools | `kind-observed` | 3 | kind-observed |
| `@solid-devtools/extension-adapter@0.12.1|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/frontend@0.15.4|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/locator@0.16.7|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/logger@0.9.11|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/overlay@0.33.5|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/shared@0.20.0|solid1|only` | solid-devtools | `kind-observed` | 5 | kind-observed |
| `@solid-devtools/ui@0.10.3|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed |
| `@solid-primitives/autofocus@0.1.5|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/clipboard@1.6.6|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/controlled-props@0.1.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/countdown@1.0.9|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date-difference@1.0.2|solid1|only` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/date@2.1.8|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 32 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 30 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/devices@3.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/devices@3.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/event-bus@1.1.4|solid1|only` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 14 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 14 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/fetch@2.5.2|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/focus@1.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/focus@1.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/form@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/form@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/graphql@3.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/history@0.2.5|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/history@1.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/history@1.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/immutable@2.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/keyed@1.5.3|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/map@0.7.4|solid1|only` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/map@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/map@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/memo@1.5.1|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/platform@0.2.1|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/platform@1.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/platform@1.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 3 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 3 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/promise@1.1.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/props@3.2.4|solid1|only` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/props@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 2 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/props@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 2 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/range@0.2.5|solid1|only` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/range@1.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/range@1.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@1.1.4|solid1|only` | solid-primitives | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@3.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@3.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 9 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@1.5.4|solid1|only` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/set@0.7.4|solid1|only` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/set@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/set@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/share@2.2.5|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/share@4.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/share@4.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/signal-builders@0.2.4|solid1|only` | solid-primitives | `incompleteness` | 33 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 204 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 208 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/spring@0.1.2|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/spring@1.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 5 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/spring@1.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 5 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/sse@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/sse@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/start@0.0.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/static-store@0.1.4|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.4|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/trigger@1.2.4|solid1|only` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/until@0.1.1|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/upload@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/upload@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/url@0.2.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/url@0.2.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/utils@6.4.1|solid1|only` | solid-primitives | `probe-failed` | 5 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 5 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 5 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/video@1.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/video@1.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/virtual@0.2.5|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/virtual@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/virtual@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solidjs/html@2.0.0-rc.1|solid2|only` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/image@0.1.0|solid1|only` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/router@1.0.0|solid1|only` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|floor` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|head` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/start@2.0.3|solid1|only` | official-solid | `probe-failed` | 31 | closure-note, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solidjs/testing-library@0.8.10|solid1|only` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|floor` | official-solid | `closure-note` | 1 | closure-note |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|head` | official-solid | `closure-note` | 1 | closure-note |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | official-solid | `probe-failed` | 77 | closure-note, incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | official-solid | `probe-failed` | 77 | closure-note, incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/ai-devtools-core@0.5.6|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/ai-solid-ui@0.7.18|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/ai-solid@0.18.3|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 91 | closure-note, kind-observed |
| `@tanstack/devtools-a11y@0.2.2|solid1|only` | tanstack | `probe-failed` | 8 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/devtools-ui@0.7.1|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/devtools-utils@0.7.0|solid1|only` | tanstack | `kind-observed` | 4 | kind-observed |
| `@tanstack/devtools@0.14.2|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/form-devtools@1.0.0-alpha.2|solid1|only` | tanstack | `probe-failed` | 6 | closure-note, kind-observed, probe-failed |
| `@tanstack/hotkeys-devtools@0.9.0|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/pacer-devtools@1.4.0|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/solid-charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-db@0.2.37|solid1|only` | tanstack | `probe-failed` | 104 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-devtools@0.8.12|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-form@2.0.0-alpha.2|solid1|only` | tanstack | `probe-failed` | 11 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-hotkeys-devtools@0.7.0|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-hotkeys@0.10.0|solid1|only` | tanstack | `probe-failed` | 9 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-pacer-devtools@0.14.0|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/solid-pacer@0.22.0|solid1|only` | tanstack | `probe-failed` | 39 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-query-devtools@5.101.4|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@5.101.4|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query@5.101.4|solid1|only` | tanstack | `probe-failed` | 12 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router-devtools@1.167.1|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router-devtools@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router@1.170.29|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/solid-router@2.0.0-rc.1|solid2|only` | tanstack | `probe-failed` | 15 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-start-client@1.168.28|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-config@1.120.20|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@1.167.35|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start@1.168.46|solid1|only` | tanstack | `kind-observed` | 7 | kind-observed |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|floor` | tanstack | `probe-failed` | 9 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|head` | tanstack | `probe-failed` | 9 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-store@0.11.1|solid1|only` | tanstack | `probe-failed` | 8 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-table-devtools@9.2.0|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-virtual@3.13.37|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/table-devtools@9.2.0|solid1|only` | tanstack | `probe-failed` | 7 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `corvu@0.7.2|solid1|only` | corvu | `probe-failed` | 28 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `motion-solidjs@0.6.0|solid1|only` | motion-solidjs | `probe-failed` | 65 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `motion-solidjs@0.7.0-beta.4|solid2|floor` | motion-solidjs | `probe-failed` | 5 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-devtools@0.34.5|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `solid-js@1.9.14|solid1|only` | official-solid | `incompleteness` | 47 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `solid-js@2.0.0-rc.1|solid2|floor` | official-solid | `probe-failed` | 63 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-js@2.0.0-rc.1|solid2|head` | official-solid | `probe-failed` | 63 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-recharts@1.0.1|solid1|only` | solid-recharts | `probe-failed` | 31 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-recharts@2.0.0-beta.1|solid2|floor` | solid-recharts | `kind-observed` | 1 | kind-observed |
| `solid-recharts@2.0.0-beta.1|solid2|head` | solid-recharts | `kind-observed` | 1 | kind-observed |

## Every verified contract

| Probe | Exports | Exports unknown | Conversions | Probed rows kept |
| --- | --- | --- | --- | --- |
| `@corvu-next/dialog@0.1.5|solid2|only` | 10 | 0 | 0 | 0 |
| `@corvu-next/disclosure@0.1.5|solid2|only` | 5 | 0 | 0 | 0 |
| `@corvu-next/dismissible@0.1.5|solid2|only` | 2 | 0 | 0 | 0 |
| `@corvu-next/persistent@0.1.5|solid2|only` | 1 | 0 | 0 | 0 |
| `@corvu-next/presence@0.1.5|solid2|only` | 1 | 0 | 0 | 0 |
| `@corvu-next/prevent-scroll@0.1.5|solid2|only` | 1 | 0 | 0 | 0 |
| `@corvu-next/transition-size@0.1.5|solid2|only` | 1 | 0 | 0 | 0 |
| `@kobalte/utils@2.0.0-alpha.0|solid2|only` | 10 | 3 | 3 | 1 |
| `@solid-devtools/transform@0.10.4|solid1|only` | 2 | 0 | 0 | 0 |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|floor` | 7 | 2 | 2 | 0 |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|head` | 7 | 2 | 2 | 0 |
| `@solid-primitives/active-element@2.1.6|solid1|only` | 5 | 4 | 1 | 0 |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|floor` | 3 | 1 | 1 | 0 |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|head` | 3 | 1 | 1 | 0 |
| `@solid-primitives/analytics@0.2.1|solid1|only` | 2 | 0 | 0 | 0 |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|floor` | 10 | 1 | 1 | 0 |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|head` | 10 | 1 | 1 | 0 |
| `@solid-primitives/async@0.0.101-next.3|solid2|floor` | 6 | 4 | 5 | 0 |
| `@solid-primitives/async@0.0.101-next.3|solid2|head` | 6 | 4 | 5 | 0 |
| `@solid-primitives/audio@1.4.5|solid1|only` | 4 | 1 | 0 | 0 |
| `@solid-primitives/audio@3.0.0-next.2|solid2|floor` | 3 | 1 | 2 | 0 |
| `@solid-primitives/audio@3.0.0-next.2|solid2|head` | 3 | 1 | 2 | 0 |
| `@solid-primitives/bounds@0.1.7|solid1|only` | 2 | 2 | 0 | 0 |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/broadcast-channel@0.1.1|solid1|only` | 2 | 1 | 1 | 0 |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|floor` | 9 | 2 | 3 | 0 |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|head` | 9 | 2 | 3 | 0 |
| `@solid-primitives/connectivity@0.4.6|solid1|only` | 3 | 3 | 0 | 0 |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|floor` | 6 | 1 | 1 | 0 |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|head` | 6 | 1 | 1 | 0 |
| `@solid-primitives/context@0.3.2|solid1|only` | 2 | 1 | 0 | 0 |
| `@solid-primitives/context@2.0.0-next.2|solid2|floor` | 4 | 0 | 0 | 0 |
| `@solid-primitives/context@2.0.0-next.2|solid2|head` | 4 | 0 | 0 | 0 |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|floor` | 5 | 5 | 5 | 0 |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|head` | 5 | 5 | 5 | 0 |
| `@solid-primitives/cookies-store@1.1.11|solid1|only` | 3 | 2 | 0 | 0 |
| `@solid-primitives/cookies@0.0.3|solid1|only` | 4 | 3 | 0 | 0 |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|floor` | 4 | 3 | 3 | 0 |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|head` | 4 | 3 | 3 | 0 |
| `@solid-primitives/cursor@0.1.4|solid1|only` | 2 | 2 | 1 | 0 |
| `@solid-primitives/db-store@1.1.4|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/debounce@1.3.0|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/deep@0.3.7|solid1|only` | 4 | 4 | 3 | 0 |
| `@solid-primitives/deep@1.0.0-next.3|solid2|floor` | 4 | 3 | 3 | 0 |
| `@solid-primitives/deep@1.0.0-next.3|solid2|head` | 4 | 3 | 3 | 0 |
| `@solid-primitives/destructure@0.2.4|solid1|only` | 1 | 1 | 0 | 0 |
| `@solid-primitives/devices@1.3.1|solid1|only` | 6 | 6 | 6 | 0 |
| `@solid-primitives/event-dispatcher@0.1.1|solid1|only` | 1 | 0 | 0 | 0 |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 |
| `@solid-primitives/event-listener@2.4.6|solid1|only` | 11 | 10 | 3 | 0 |
| `@solid-primitives/event-props@0.3.1|solid1|only` | 1 | 0 | 0 | 0 |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 |
| `@solid-primitives/filesystem@1.3.4|solid1|only` | 15 | 5 | 5 | 3 |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|floor` | 15 | 5 | 5 | 3 |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|head` | 15 | 5 | 5 | 3 |
| `@solid-primitives/flux-store@0.1.1|solid1|only` | 4 | 3 | 2 | 0 |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|floor` | 4 | 2 | 3 | 0 |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|head` | 4 | 2 | 3 | 0 |
| `@solid-primitives/fullscreen@1.3.5|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|floor` | 3 | 1 | 2 | 0 |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|head` | 3 | 1 | 2 | 0 |
| `@solid-primitives/geolocation@1.5.5|solid1|only` | 2 | 2 | 1 | 0 |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|floor` | 6 | 2 | 2 | 0 |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|head` | 6 | 2 | 2 | 0 |
| `@solid-primitives/gestures@1.2.1|solid1|only` | 9 | 6 | 1 | 0 |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|floor` | 11 | 1 | 1 | 0 |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|head` | 11 | 1 | 1 | 0 |
| `@solid-primitives/i18n@2.2.1|solid1|only` | 9 | 7 | 5 | 0 |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|floor` | 12 | 7 | 7 | 0 |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|head` | 12 | 7 | 7 | 0 |
| `@solid-primitives/idle@0.2.3|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/idle@1.0.0-next.3|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/idle@1.0.0-next.3|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/input-mask@0.3.1|solid1|only` | 7 | 2 | 1 | 0 |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|floor` | 7 | 2 | 2 | 0 |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|head` | 7 | 2 | 2 | 0 |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|floor` | 5 | 1 | 1 | 0 |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|head` | 5 | 1 | 1 | 0 |
| `@solid-primitives/intersection-observer@2.2.5|solid1|only` | 11 | 6 | 3 | 0 |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|floor` | 12 | 4 | 6 | 0 |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|head` | 12 | 4 | 6 | 0 |
| `@solid-primitives/jsx-parser@0.2.0|solid1|only` | 4 | 2 | 3 | 0 |
| `@solid-primitives/jsx-tokenizer@1.1.4|solid1|only` | 4 | 1 | 1 | 0 |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|floor` | 4 | 1 | 2 | 0 |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|head` | 4 | 1 | 2 | 0 |
| `@solid-primitives/keyboard@1.3.7|solid1|only` | 6 | 2 | 1 | 0 |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|floor` | 7 | 2 | 2 | 0 |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|head` | 7 | 2 | 2 | 0 |
| `@solid-primitives/lifecycle@0.1.2|solid1|only` | 3 | 2 | 2 | 0 |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|floor` | 3 | 2 | 2 | 0 |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|head` | 3 | 2 | 2 | 0 |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 |
| `@solid-primitives/list@0.1.2|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/list@1.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 |
| `@solid-primitives/list@1.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 |
| `@solid-primitives/local-store@1.1.4|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/marker@0.2.2|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/marker@2.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 |
| `@solid-primitives/marker@2.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 |
| `@solid-primitives/masonry@0.1.4|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/masonry@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/masonry@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/match@0.0.100|solid1|only` | 3 | 3 | 0 | 0 |
| `@solid-primitives/match@1.0.0-next.2|solid2|floor` | 3 | 3 | 3 | 0 |
| `@solid-primitives/match@1.0.0-next.2|solid2|head` | 3 | 0 | 0 | 0 |
| `@solid-primitives/media@2.3.6|solid1|only` | 6 | 3 | 0 | 0 |
| `@solid-primitives/media@4.0.0-next.2|solid2|floor` | 6 | 0 | 0 | 0 |
| `@solid-primitives/media@4.0.0-next.2|solid2|head` | 6 | 0 | 0 | 0 |
| `@solid-primitives/mouse@2.1.7|solid1|only` | 8 | 8 | 1 | 0 |
| `@solid-primitives/mutable@1.1.1|solid1|only` | 2 | 2 | 0 | 0 |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 1 |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|head` | 2 | 1 | 1 | 1 |
| `@solid-primitives/mutation-observer@1.2.4|solid1|only` | 2 | 2 | 0 | 0 |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|floor` | 2 | 0 | 0 | 0 |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|head` | 2 | 0 | 0 | 0 |
| `@solid-primitives/notification@1.0.0-next.3|solid2|floor` | 4 | 2 | 2 | 0 |
| `@solid-primitives/notification@1.0.0-next.3|solid2|head` | 4 | 2 | 2 | 0 |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|floor` | 4 | 1 | 1 | 0 |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|head` | 4 | 1 | 1 | 0 |
| `@solid-primitives/page-visibility@2.1.6|solid1|only` | 2 | 0 | 0 | 0 |
| `@solid-primitives/pagination@0.5.2|solid1|only` | 4 | 3 | 4 | 0 |
| `@solid-primitives/permission@1.3.2|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/permission@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/permission@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/pointer@0.3.6|solid1|only` | 7 | 7 | 0 | 0 |
| `@solid-primitives/presence@0.1.4|solid1|only` | 1 | 1 | 2 | 0 |
| `@solid-primitives/presence@1.0.0-next.2|solid2|floor` | 1 | 1 | 2 | 0 |
| `@solid-primitives/presence@1.0.0-next.2|solid2|head` | 1 | 1 | 2 | 0 |
| `@solid-primitives/promise@2.0.0-next.2|solid2|floor` | 7 | 3 | 4 | 0 |
| `@solid-primitives/promise@2.0.0-next.2|solid2|head` | 7 | 3 | 4 | 0 |
| `@solid-primitives/queue@1.0.0-next.3|solid2|floor` | 6 | 5 | 5 | 0 |
| `@solid-primitives/queue@1.0.0-next.3|solid2|head` | 6 | 5 | 5 | 0 |
| `@solid-primitives/raf@2.3.5|solid1|only` | 4 | 4 | 5 | 0 |
| `@solid-primitives/raf@4.0.0-next.2|solid2|floor` | 4 | 4 | 5 | 0 |
| `@solid-primitives/raf@4.0.0-next.2|solid2|head` | 4 | 4 | 5 | 0 |
| `@solid-primitives/reducer@0.0.101|solid1|only` | 1 | 1 | 2 | 0 |
| `@solid-primitives/resize-observer@2.2.0|solid1|only` | 7 | 4 | 2 | 0 |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|floor` | 7 | 3 | 3 | 0 |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|head` | 7 | 3 | 3 | 0 |
| `@solid-primitives/resource@0.4.3|solid1|only` | 8 | 7 | 1 | 0 |
| `@solid-primitives/scheduled@1.5.3|solid1|only` | 6 | 5 | 5 | 0 |
| `@solid-primitives/script-loader@2.3.2|solid1|only` | 1 | 0 | 0 | 0 |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/scroll@2.1.6|solid1|only` | 5 | 2 | 1 | 0 |
| `@solid-primitives/selection@0.1.3|solid1|only` | 2 | 1 | 1 | 0 |
| `@solid-primitives/selection@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/selection@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/sse@0.0.103|solid1|only` | 10 | 1 | 2 | 0 |
| `@solid-primitives/state-machine@0.1.1|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/storage@4.4.0|solid1|only` | 11 | 8 | 0 | 0 |
| `@solid-primitives/storage@5.0.0-next.4|solid2|floor` | 11 | 2 | 2 | 0 |
| `@solid-primitives/storage@5.0.0-next.4|solid2|head` | 11 | 2 | 2 | 0 |
| `@solid-primitives/stream@0.7.4|solid1|only` | 5 | 4 | 5 | 0 |
| `@solid-primitives/styles@0.1.4|solid1|only` | 4 | 0 | 0 | 0 |
| `@solid-primitives/styles@1.0.0-next.2|solid2|floor` | 4 | 0 | 0 | 0 |
| `@solid-primitives/styles@1.0.0-next.2|solid2|head` | 4 | 0 | 0 | 0 |
| `@solid-primitives/throttle@1.2.0|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/transition-group@1.1.2|solid1|only` | 2 | 2 | 3 | 0 |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|floor` | 2 | 2 | 4 | 0 |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|head` | 2 | 2 | 4 | 0 |
| `@solid-primitives/tween@1.4.1|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/tween@2.0.0-next.2|solid2|floor` | 1 | 1 | 2 | 0 |
| `@solid-primitives/tween@2.0.0-next.2|solid2|head` | 1 | 1 | 2 | 0 |
| `@solid-primitives/upload@0.1.5|solid1|only` | 3 | 3 | 3 | 0 |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|floor` | 6 | 2 | 4 | 0 |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|head` | 6 | 2 | 4 | 0 |
| `@solid-primitives/visibility-observer@2.0.1|solid1|only` | 2 | 1 | 1 | 0 |
| `@solid-primitives/websocket@1.4.0|solid1|only` | 6 | 2 | 2 | 0 |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|floor` | 10 | 5 | 5 | 0 |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|head` | 10 | 5 | 5 | 0 |
| `@solid-primitives/workers@0.4.3|solid1|only` | 3 | 3 | 0 | 0 |
| `@solid-primitives/workers@2.0.1-next.1|solid2|floor` | 5 | 3 | 4 | 0 |
| `@solid-primitives/workers@2.0.1-next.1|solid2|head` | 5 | 3 | 4 | 0 |
| `@solidjs/element@2.0.0-rc.1|solid2|only` | 5 | 2 | 2 | 0 |
| `@solidjs/h@2.0.0-rc.1|solid2|only` | 9 | 0 | 0 | 0 |
| `@solidjs/meta@0.29.4|solid1|only` | 9 | 2 | 2 | 0 |
| `@solidjs/meta@1.0.0-next.2|solid2|floor` | 8 | 7 | 0 | 0 |
| `@solidjs/meta@1.0.0-next.2|solid2|head` | 8 | 7 | 0 | 0 |
| `@solidjs/router@2.0.0-next.17|solid2|only` | 30 | 29 | 32 | 0 |
| `@solidjs/universal@2.0.0-rc.1|solid2|only` | 1 | 0 | 0 | 0 |
| `@tanstack/solid-ai-devtools@0.2.70|solid1|only` | 4 | 0 | 0 | 0 |
| `@tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only` | 1 | 0 | 0 | 0 |
| `@tanstack/solid-router-ssr-query@1.167.2-pre.0|solid1|only` | 1 | 0 | 0 | 0 |
