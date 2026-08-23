# Ecosystem machine-verification report

How many real ecosystem packages machine-verify end to end: `contract generate` -> `contract probe --write` -> `contract verify`, run against a throwaway install of every probe row in the pinned corpus.

> **This measurement executes package code.** `contract probe` imports and runs each
> installed package, and its dependencies, in child processes. Every install and every
> execution happened inside temporary directories under the harness state directory, npm
> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran
> under both a per-mode timeout and a whole-phase wall budget.

- Started: 2026-08-23T03:42:05.948Z
- Finished: 2026-08-23T03:49:16.858Z
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Probe rows run: 416
- Checker native binary: `8dde96e824c41d3274453f446aa0ed876f65e5bd028cc51a4182a65dbf99c673` (14630032 bytes, mtime 2026-08-23T02:42:11.680Z)
- Type Facts binary: `2bbdef833749ed8c9fdda60ed9245b54baeaa9ceb98b1a880853a2c90ac56f2d` (28389218 bytes, mtime 2026-08-23T02:42:11.693Z)
- Budgets: install 240000 ms, generate 120000 ms, probe 20000 ms per condition mode / 90000 ms + 500 ms per planned claim, capped at 900000 ms, whole phase, verify 90000 ms; concurrency 6
- Import-environment shim: enabled (client, development and production sessions only; server sessions never)

## Headline

| Figure | Count |
| --- | --- |
| Probe rows run | 416 |
| Reached a generated contract | 409/416 (98.32%) |
| **Reached `verified`** | **222/416 (53.37%)** of all rows |
| Reached `verified`, of rows that produced a contract | 222/409 (54.28%) |
| Refused by `contract verify` | 185/416 (44.47%) |

Outcome classes, raw:

| Outcome | Rows |
| --- | --- |
| `verified` | 222 |
| `refused` | 185 |
| `generate-failure` | 4 |
| `install-failure` | 3 |
| `no-runtime` | 2 |

## Per family

| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 23 | 7/23 (30.43%) | 14 | 1595/3130 (50.96%) | 1565/1595 (98.12%) | 29 | 18 | 52 |
| Kobalte | 6 | 4 | 1/6 (16.67%) | 3 | 900/1717 (52.42%) | 891/900 (99.00%) | 3 | 6 | 4 |
| Solid Primitives | 289 | 288 | 193/289 (66.78%) | 95 | 2078/3411 (60.92%) | 1979/2078 (95.24%) | 388 | 540 | 474 |
| Corvu | 28 | 28 | 7/28 (25.00%) | 21 | 294/426 (69.01%) | 287/294 (97.62%) | 0 | 21 | 0 |
| TanStack | 52 | 50 | 11/52 (21.15%) | 39 | 1748/2827 (61.83%) | 1711/1748 (97.88%) | 175 | 141 | 269 |
| Solid Devtools | 12 | 10 | 2/12 (16.67%) | 8 | 146/363 (40.22%) | 138/146 (94.52%) | 0 | 2 | 1 |
| Solid Recharts | 3 | 3 | 0/3 (0.00%) | 3 | 136/366 (37.16%) | 110/136 (80.88%) | 0 | 0 | 0 |
| Motion for Solid | 3 | 3 | 1/3 (33.33%) | 2 | 912/966 (94.41%) | 910/912 (99.78%) | 0 | 24 | 333 |

| Solid target | Rows | Contracts | Verified | Refused |
| --- | --- | --- | --- | --- |
| solid1 | 168 | 163 | 83/168 (49.40%) | 80 |
| solid2 | 248 | 246 | 139/248 (56.05%) | 105 |

## Why verification refuses

185 rows were refused. `contract verify` raises every blocker it finds rather than stopping at the first, so the row counts below sum to more than the number of refused rows.

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |
| --- | --- | --- |
| `probe-report-includes-evidence-write` | 108 | 108 |
| `kind-observed` | 107 | 358 |
| `probe-failed` | 75 | 218 |
| `incompleteness` | 59 | 1080 |
| `closure-note` | 7 | 32 |

Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a *consequence*: `contract probe --write` declines to write evidence once a probe failed or an incompleteness was reported, so verification then sees passing claims that never reached the contract. It is counted as a root cause only on a row where it stands alone.

| Root cause | Refused rows |
| --- | --- |
| `probe-failed` | 75 |
| `kind-observed` | 71 |
| `incompleteness` | 37 |
| `closure-note` | 2 |

## Drivability

| Figure | Count |
| --- | --- |
| Claims planned across every probed contract | 13206 |
| Driven | 7809/13206 (59.13%) |
| Passed | 7591/13206 (57.48%) |
| Failed | 218 |
| Undriven | 5397/13206 (40.87%) |
| Incompleteness findings | 1080 |

Undriven claims by reason:

| Reason | Claims |
| --- | --- |
| no probe form: reactiveReads | 1354 |
| other | 835 |
| entrypoint import threw | 651 |
| no probe form: ownerRequirements | 565 |
| synthesized call threw | 444 |
| no probe form: parameter identity | 421 |
| synthesized call did not invoke the callback | 278 |
| no probe form: nested return leaf | 257 |
| no plantable reactive source | 180 |
| no probe form: asyncBehavior | 100 |
| probe session wrote no report | 91 |
| no unambiguous summary for the mode | 85 |
| probe session hit the per-mode timeout | 55 |
| no probe form: callback arguments | 25 |
| no probe form: store path | 23 |
| callback ownership ambiguous in the driver's read scope | 23 |
| planted write was never re-read | 10 |

## The probe environment

An entrypoint whose module cannot be imported yields no observation at all. 34 of the corpus's rows had at least one entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or TypeScript loader, and only the packages the corpus manifest installs beside the probed one. Some of these throws are facts about the package; others are facts about that environment, and the two are not separated here.

| Import failure | Claims left undriven |
| --- | --- |
| Error [ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING]: Stripping types is currently unsupported for files under node_modules, | 227 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@solid-primitives/utils' imported from /private/t | 94 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: Package subpath './web' is not defined by "exports" in <path> | 81 |
| Error: [solid-devtools]: Debugger hasn't found the exposed Solid Devtools API | 66 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'server-only' imported from <path> | 60 |
| TypeError: Cannot read properties of null (reading '_depth') | 54 |
| TypeError [ERR_UNKNOWN_FILE_EXTENSION]: Unknown file extension ".jsx" for <path> | 27 |
| SyntaxError: The requested module 'solid-js' does not provide an export named 'onSe | 10 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'react' imported from <path> | 6 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vite' imported from <path> | 5 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: No "exports" main defined in <path> | 4 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@angular/core' imported from <path> | 4 |
| Error [ERR_UNSUPPORTED_ESM_URL_SCHEME]: Only URLs with a scheme in: file, data, and node are supported by the  | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find module '<path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'preact' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'svelte' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vue' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@rsbuild/core' imported from <path> | 2 |
| SyntaxError: The requested module '@tanstack/router-generator' does not provide an  | 1 |

### The globals the probe worker faked

A module that reads `window` while it is being evaluated throws in a bare Node process, the worker stops, and every claim of that entrypoint goes undriven — so nothing at all is observed about the package. The worker therefore defines a small inert browser surface before it imports anything, in the `client`, `development` and `production` sessions only.

**A claim observed under the shim is a weaker observation than one made in a browser.** The fake `document` renders nothing, the fake `matchMedia` never matches, the fake `navigator` says it is this checker. A package that branches on any of that was observed on the branch the fake sent it down. Every `<contract>.probe.json` and `<contract>.verify.json` records the per-mode list of faked names, so where the distinction matters the record says so rather than the number implying a browser.

`server` sessions are never shimmed: an import that throws on `window` under `--conditions node` is a truthful observation of that entrypoint in that mode, and faking it there would manufacture a pass the package never earned.

- Rows where at least one session faked at least one global: 404

| Faked global | Rows |
| --- | --- |
| `IntersectionObserver` | 404 |
| `MutationObserver` | 404 |
| `ResizeObserver` | 404 |
| `cancelAnimationFrame` | 404 |
| `document` | 404 |
| `getComputedStyle` | 404 |
| `history` | 404 |
| `localStorage` | 404 |
| `location` | 404 |
| `matchMedia` | 404 |
| `requestAnimationFrame` | 404 |
| `screen` | 404 |
| `self` | 404 |
| `sessionStorage` | 404 |
| `window` | 404 |

### Worker processes

A worker stops at its first throw and the mode is restarted for what is left — the only way to un-halt a Solid 2.0 development runtime. A restart is not a failure; a row that needed many is the shape behind a slow or timed-out probe.

| Figure | Count |
| --- | --- |
| Worker processes started | 20367 |
| Of those, restarts after a throw | 18784 |
| Sessions that died (crash, timeout, unreadable output) | 78 |

## The install environment

Each row installs the pinned package, the Solid runtime the manifest row pins, and the non-optional peers the installed artifact's own `package.json` declares. Peers are installed in a second npm invocation so that no peer range can take part in resolving the pinned versions; if it moves a pin anyway, the pinned-only tree is restored and the row is recorded as such.

| Figure | Rows |
| --- | --- |
| Solid 2 rows given the `@solidjs/web` half of the runtime the row pinned only half of | 53 |
| Rows with a completed peer install | 27 |
| Peer packages installed | 37 |
| Rows whose peer install failed or moved a pin | 4 |

A package that **imports something it declares nowhere** — not a dependency, not a peer — is outside what any install policy can supply, and is reported above as an import throw rather than fixed here. Completing an undeclared import would mean this harness choosing a version the package never named.

## Probe failures: claims the package answered differently

A **failure** is the strongest thing this measurement produces. The contract states a claim, the probe drove it, and the package did something else — a generator bug or a package change, never an environment gap and never an unreachable claim. Verification refuses the whole contract on one of these, deliberately: converting a contradicted claim to the unknown sentinel would hide it.

218 failing claim(s) across the corpus, by shape:

| Claim, claimed, observed | Claims |
| --- | --- |
| callbacks[n]: claimed tracked, observed inline | 99 |
| kind: claimed value, observed function | 53 |
| callbacks[n]: claimed deferred, observed inline | 34 |
| callbacks[n]: claimed deferred, observed tracked | 17 |
| returns: claimed accessor, observed array | 6 |
| callbacks[n]: claimed inline, observed tracked | 4 |
| callbacks[n]: claimed tracked, observed deferred | 3 |
| callbacks[n]: claimed inline, observed deferred | 2 |

The first 60, in full (the JSON report carries all 218):

| Probe | Export | Claim | Observed | Modes |
| --- | --- | --- | --- | --- |
| `@solid-primitives/keyed@1.5.3|solid1|only` | `.:keyArray` | `callbacks[0]=deferred` | inline | server |
| `@solid-primitives/pagination@0.5.2|solid1|only` | `.:createInfiniteScroll` | `callbacks[0]=deferred` | inline | client, development, production |
| `@solid-primitives/range@0.2.5|solid1|only` | `.:mapRange` | `callbacks[2]=deferred` | inline | client, development, production, server |
| `@solid-primitives/range@1.0.0-next.3|solid2|floor` | `.:mapRange` | `callbacks[2]=deferred` | inline | client, development, production, server |
| `@solid-primitives/range@1.0.0-next.3|solid2|head` | `.:mapRange` | `callbacks[2]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@1.5.4|solid1|only` | `.:createBranch` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@1.5.4|solid1|only` | `.:createDisposable` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@1.5.4|solid1|only` | `.:createSubRoot` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | `.:createBranch` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | `.:createDisposable` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | `.:createSubRoot` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | `.:createBranch` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | `.:createDisposable` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | `.:createSubRoot` | `callbacks[0]=deferred` | inline | client, development, production, server |
| `@solid-primitives/static-store@0.1.4|solid1|only` | `.:createHydratableStaticStore` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|floor` | `.:createHydratableStaticStore` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|head` | `.:createHydratableStaticStore` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@6.4.1|solid1|only` | `.:createHydratableSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@6.4.1|solid1|only` | `.:createHydrateSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | `.:createHydratableSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | `.:createHydrateSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | `.:createHydratableSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | `.:createHydrateSignal` | `callbacks[1]=deferred` | inline | client, development, production |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | `.:effect` | `callbacks[1]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | `.:renderToString` | `callbacks[0]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | `./jsx-dev-runtime:effect` | `callbacks[1]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | `./jsx-runtime:effect` | `callbacks[1]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | `.:effect` | `callbacks[1]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | `.:renderToString` | `callbacks[0]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | `./jsx-dev-runtime:effect` | `callbacks[1]=deferred` | inline | server |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | `./jsx-runtime:effect` | `callbacks[1]=deferred` | inline | server |
| `solid-js@1.9.14|solid1|only` | `./web:use` | `callbacks[0]=deferred` | inline | client, development, production |
| `solid-js@2.0.0-rc.1|solid2|floor` | `.:createComponent` | `callbacks[0]=deferred` | inline | client, production |
| `solid-js@2.0.0-rc.1|solid2|head` | `.:createComponent` | `callbacks[0]=deferred` | inline | client, production |
| `@corvu-next/utils@0.1.5|solid2|only` | `./dom:afterPaint` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@corvu/utils@0.4.2|solid1|only` | `./dom:afterPaint` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | `.:createWritableMemo` | `callbacks[0]=deferred` | tracked | client, development, production, server |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | `.:createWritableMemo` | `callbacks[0]=deferred` | tracked | client, development, production, server |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | `.:createTimeoutLoop` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | `.:createTimeoutLoop` | `callbacks[1]=deferred` | tracked | client, development, production |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | `.:createTimeoutLoop` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | `.:createTimeoutLoop` | `callbacks[1]=deferred` | tracked | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | `.:afterPaint` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | `.:afterPaint` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/video@1.0.0-next.3|solid2|floor` | `.:createVideoFrameCallback` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/video@1.0.0-next.3|solid2|head` | `.:createVideoFrameCallback` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@tanstack/table-devtools@9.2.0|solid1|only` | `.:subscribeTableDevtoolsTargets` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@tanstack/table-devtools@9.2.0|solid1|only` | `./production:subscribeTableDevtoolsTargets` | `callbacks[0]=deferred` | tracked | client, development, production |
| `motion-solidjs@0.7.0-beta.4|solid2|head` | `.:createAnimationFrame` | `callbacks[0]=deferred` | tracked | client, development, production |
| `motion-solidjs@0.7.0-beta.4|solid2|head` | `./v2:createAnimationFrame` | `callbacks[0]=deferred` | tracked | client, development, production |
| `solid-js@1.9.14|solid1|only` | `./jsx-dev-runtime:createSelector` | `callbacks[0]=deferred` | tracked | server |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | `.:createTimer` | `callbacks[2]=inline` | deferred | client, development, production |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | `.:createTimer` | `callbacks[2]=inline` | deferred | client, development, production |
| `@solid-primitives/timer@1.4.4|solid1|only` | `.:createTimer` | `callbacks[2]=inline` | tracked | client, development, production |
| `solid-js@1.9.14|solid1|only` | `./jsx-dev-runtime:createComputed` | `callbacks[0]=inline` | tracked | server |
| `solid-js@1.9.14|solid1|only` | `./jsx-dev-runtime:createMemo` | `callbacks[0]=inline` | tracked | server |
| `solid-js@1.9.14|solid1|only` | `./jsx-dev-runtime:createRenderEffect` | `callbacks[0]=inline` | tracked | server |
| `solid-js@1.9.14|solid1|only` | `.:onMount` | `callbacks[0]=tracked` | deferred | client, development, production |
| `solid-js@1.9.14|solid1|only` | `./jsx-dev-runtime:onMount` | `callbacks[0]=tracked` | deferred | client, development, production |
| `solid-js@1.9.14|solid1|only` | `./jsx-runtime:onMount` | `callbacks[0]=tracked` | deferred | client, development, production |

## Conversion volume

A conversion replaces one export's whole claim domain with the `{"status":"unknown"}` sentinel because the probe neither observed nor statically proved it.

| Figure | Count |
| --- | --- |
| Claim domains converted to unknown | 595 |
| Exports carrying an unknown in the verified rows, at generation | 797/1885 (42.28%) |
| Exports carrying an unknown in the verified rows, after verification | 1133/1885 (60.11%) |

How much a verified contract actually certifies from observation:

| Figure | Count |
| --- | --- |
| Verified rows carrying at least one probed behavioral row | 15/222 (6.76%) |
| Probed behavioral row markers kept across the whole corpus | 25 |
| Inferred row markers dropped by verification | 2292 |
| Probed markers discarded as unwitnessed by this run's report | 29 |

Converted domains by field:

| Field | Conversions |
| --- | --- |
| `returns` | 316 |
| `callbacks` | 267 |
| `asyncBehavior` | 12 |

## The composite a consumer feels

Of every export the corpus's generated contracts describe:

| State | Exports |
| --- | --- |
| (a) certified by a verified contract | 752/9015 (8.34%) |
| (b) honest unknown inside a verified contract | 1133/9015 (12.57%) |
| (c) inside a contract that never reached `verified` | 7130/9015 (79.09%) |

(c) is every export of a contract that was generated and then refused, timed out, or errored before a probe report existed. Rows whose `npm install` or `contract generate` failed describe no exports at all and are in none of the three states.

## Wall time

| Phase | Rows | Median | p90 | Max | Mean |
| --- | --- | --- | --- | --- | --- |
| install | 416 | 734 ms | 1575 ms | 19857 ms | 938 ms |
| generate | 413 | 113 ms | 643 ms | 16399 ms | 459 ms |
| probe | 407 | 661 ms | 3593 ms | 198058 ms | 3559 ms |
| verify | 407 | 47 ms | 57 ms | 100 ms | 48 ms |
| pipelineWithoutInstall | 413 | 892 ms | 4204 ms | 214508 ms | 4013 ms |
| total | 416 | 1650 ms | 5650 ms | 216665 ms | 4960 ms |

`install` may run against a warm npm cache, so it is a lower bound; `pipelineWithoutInstall` is the number that describes the checker's own cost.

## Rows that never reached verification

| Stage | Rows |
| --- | --- |
| `npm install` failed | 3 |
| `contract generate` failed | 4 |
| `contract probe` errored before writing a report | 0 |
| no Solid runtime the row could honestly be probed against | 2 |
| timed out under the harness budget | 0 |

The manifest pins the runtime each row runs against, and for these it pins no `solid-js` — `@solidjs/signals` *is* the reactive core, so there is no second package to settle a probe with. Pairing one in would be this harness auditing a combination the corpus deliberately did not. They are their own class rather than an error:

- `@solidjs/signals@2.0.0-rc.1|solid2|floor`
- `@solidjs/signals@2.0.0-rc.1|solid2|head`

Generation failures by class:

| Class | Rows |
| --- | --- |
| `no-esm-runtime-target` | 2 |
| `cjs-only-entrypoint` | 1 |
| `no-exported-surface` | 1 |

## Caveats, stated because these numbers are easy to over-read

- **`verified` is not `reviewed`.** A verified contract certifies what a machine observed or statically proved and converts everything else to the unknown sentinel. It is a weaker claim than the human `reviewed` tier, and a stronger one than the `inferred` draft the generation benchmark measures.
- **Some observations were made against a fake DOM.** The probe worker defines a minimal inert browser surface in the client, development and production sessions so that an import-time `window` read does not cost the whole entrypoint. What is then observed is the package's behavior *given that fake*, which is not the same fact as its behavior in a browser. Every probe report and verify sidecar names the globals it faked; server sessions fake nothing.
- **The install is peer-complete, not project-complete.** It installs the probed package, the Solid runtime the manifest row pins, and the peers the artifact declares. A package that imports something it declares nowhere still fails to import, and that is a fact about the package rather than about this harness.
- **A timeout is never a verification result.** Rows that exceeded the probe wall budget are their own outcome class and are counted as neither verified nor refused. The budget now scales with each row's planned claim count, so fewer rows hit one — which changes how many rows the measurement can speak about, never what a timeout means.
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
| `@corvu-next/utils@0.1.5|solid2|only` | corvu | `probe-failed` | 12 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@corvu/accordion@0.2.5|solid1|only` | corvu | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/calendar@0.1.2|solid1|only` | corvu | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@corvu/dialog@0.2.4|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/disclosure@0.2.2|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/drawer@0.2.4|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@corvu/otp-field@0.1.4|solid1|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu/popover@0.2.0|solid1|only` | corvu | `incompleteness` | 5 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@corvu/resizable@0.2.5|solid1|only` | corvu | `kind-observed` | 1 | kind-observed |
| `@corvu/tooltip@0.2.2|solid1|only` | corvu | `incompleteness` | 5 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@corvu/utils@0.4.2|solid1|only` | corvu | `probe-failed` | 8 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@kobalte/core@0.13.13|solid1|only` | kobalte | `probe-failed` | 113 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@kobalte/core@2.0.0-alpha.0|solid2|only` | kobalte | `incompleteness` | 88 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@kobalte/utils@0.9.2|solid1|only` | kobalte | `incompleteness` | 17 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@solid-devtools/debugger@0.28.1|solid1|only` | solid-devtools | `kind-observed` | 3 | kind-observed |
| `@solid-devtools/extension-adapter@0.12.1|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/frontend@0.15.4|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/locator@0.16.7|solid1|only` | solid-devtools | `probe-failed` | 13 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-devtools/logger@0.9.11|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `@solid-devtools/shared@0.20.0|solid1|only` | solid-devtools | `kind-observed` | 5 | kind-observed |
| `@solid-devtools/ui@0.10.3|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed |
| `@solid-primitives/controlled-props@0.1.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/countdown@1.0.9|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date-difference@1.0.2|solid1|only` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/date@2.1.8|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 25 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/date@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 24 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 14 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 14 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/fetch@2.5.2|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/focus@1.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/focus@1.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/form@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/form@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/graphql@3.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/history@0.2.5|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/history@1.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/history@1.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/immutable@2.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/keyed@1.5.3|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/map@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/map@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/memo@1.5.1|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/pagination@0.5.2|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|floor` | solid-primitives | `probe-failed` | 6 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|head` | solid-primitives | `probe-failed` | 6 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/promise@1.1.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/range@0.2.5|solid1|only` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/range@1.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/range@1.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@1.1.4|solid1|only` | solid-primitives | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@3.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 8 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/refs@3.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 9 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@1.5.4|solid1|only` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/set@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/set@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 4 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/share@4.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/share@4.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/signal-builders@0.2.4|solid1|only` | solid-primitives | `incompleteness` | 33 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 196 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 200 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|floor` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|head` | solid-primitives | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@solid-primitives/spring@0.1.2|solid1|only` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/spring@1.0.0-next.3|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/spring@1.0.0-next.3|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/start@0.0.4|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/static-store@0.1.4|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.4|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | solid-primitives | `probe-failed` | 4 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/until@0.1.1|solid1|only` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/upload@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/upload@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 1 | kind-observed |
| `@solid-primitives/utils@6.4.1|solid1|only` | solid-primitives | `probe-failed` | 5 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | solid-primitives | `probe-failed` | 8 | probe-failed, probe-report-includes-evidence-write |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | solid-primitives | `probe-failed` | 8 | probe-failed, probe-report-includes-evidence-write |
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
| `@solidjs/start@2.0.3|solid1|only` | official-solid | `kind-observed` | 28 | closure-note, kind-observed |
| `@solidjs/testing-library@0.8.10|solid1|only` | official-solid | `kind-observed` | 1 | kind-observed |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|floor` | official-solid | `closure-note` | 1 | closure-note |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|head` | official-solid | `closure-note` | 1 | closure-note |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | official-solid | `probe-failed` | 42 | closure-note, incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | official-solid | `probe-failed` | 42 | closure-note, incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/ai-devtools-core@0.5.6|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/ai-solid-ui@0.7.18|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 91 | closure-note, kind-observed |
| `@tanstack/devtools-a11y@0.2.2|solid1|only` | tanstack | `probe-failed` | 8 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/devtools-ui@0.7.1|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/devtools-utils@0.7.0|solid1|only` | tanstack | `kind-observed` | 4 | kind-observed |
| `@tanstack/devtools@0.14.2|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/form-devtools@1.0.0-alpha.2|solid1|only` | tanstack | `probe-failed` | 6 | closure-note, kind-observed, probe-failed |
| `@tanstack/hotkeys-devtools@0.9.0|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/pacer-devtools@1.4.0|solid1|only` | tanstack | `probe-failed` | 4 | kind-observed, probe-failed |
| `@tanstack/solid-charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-form@2.0.0-alpha.2|solid1|only` | tanstack | `incompleteness` | 8 | incompleteness, probe-report-includes-evidence-write |
| `@tanstack/solid-hotkeys-devtools@0.7.0|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-hotkeys@0.10.0|solid1|only` | tanstack | `probe-failed` | 3 | probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-pacer-devtools@0.14.0|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/solid-pacer@0.22.0|solid1|only` | tanstack | `probe-failed` | 27 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-query-devtools@5.101.4|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@5.101.4|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router-devtools@1.167.1|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router-devtools@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-router@1.170.29|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed |
| `@tanstack/solid-router@2.0.0-rc.1|solid2|only` | tanstack | `incompleteness` | 7 | incompleteness, kind-observed, probe-report-includes-evidence-write |
| `@tanstack/solid-start-client@1.168.28|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 3 | kind-observed |
| `@tanstack/solid-start-config@1.120.20|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@1.167.35|solid1|only` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 1 | kind-observed |
| `@tanstack/solid-start@1.168.46|solid1|only` | tanstack | `kind-observed` | 10 | kind-observed |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 7 | kind-observed |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 7 | kind-observed |
| `@tanstack/solid-store@0.11.1|solid1|only` | tanstack | `incompleteness` | 5 | incompleteness, probe-report-includes-evidence-write |
| `@tanstack/solid-table-devtools@9.2.0|solid1|only` | tanstack | `probe-failed` | 3 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-table@9.1.2|solid1|only` | tanstack | `probe-failed` | 12 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `@tanstack/solid-virtual@3.13.37|solid1|only` | tanstack | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write |
| `@tanstack/table-devtools@9.2.0|solid1|only` | tanstack | `probe-failed` | 7 | kind-observed, probe-failed, probe-report-includes-evidence-write |
| `corvu@0.7.2|solid1|only` | corvu | `probe-failed` | 31 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `motion-solidjs@0.6.0|solid1|only` | motion-solidjs | `incompleteness` | 59 | incompleteness, probe-report-includes-evidence-write |
| `motion-solidjs@0.7.0-beta.4|solid2|head` | motion-solidjs | `probe-failed` | 67 | incompleteness, probe-failed, probe-report-includes-evidence-write |
| `solid-devtools@0.34.5|solid1|only` | solid-devtools | `kind-observed` | 1 | kind-observed |
| `solid-js@1.9.14|solid1|only` | official-solid | `probe-failed` | 39 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-js@2.0.0-rc.1|solid2|floor` | official-solid | `probe-failed` | 51 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-js@2.0.0-rc.1|solid2|head` | official-solid | `probe-failed` | 51 | incompleteness, kind-observed, probe-failed, probe-report-includes-evidence-write |
| `solid-recharts@1.0.1|solid1|only` | solid-recharts | `probe-failed` | 28 | kind-observed, probe-failed, probe-report-includes-evidence-write |
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
| `@kobalte/utils@2.0.0-alpha.0|solid2|only` | 10 | 4 | 3 | 1 |
| `@solid-devtools/overlay@0.33.5|solid1|only` | 1 | 1 | 0 | 0 |
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
| `@solid-primitives/autofocus@0.1.5|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/bounds@0.1.7|solid1|only` | 2 | 2 | 0 | 0 |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/broadcast-channel@0.1.1|solid1|only` | 2 | 2 | 1 | 0 |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|floor` | 2 | 2 | 1 | 0 |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|head` | 2 | 2 | 1 | 0 |
| `@solid-primitives/clipboard@1.6.6|solid1|only` | 9 | 3 | 3 | 0 |
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
| `@solid-primitives/deep@1.0.0-next.3|solid2|floor` | 4 | 4 | 3 | 0 |
| `@solid-primitives/deep@1.0.0-next.3|solid2|head` | 4 | 4 | 3 | 0 |
| `@solid-primitives/destructure@0.2.4|solid1|only` | 1 | 1 | 0 | 0 |
| `@solid-primitives/devices@1.3.1|solid1|only` | 6 | 6 | 6 | 0 |
| `@solid-primitives/devices@3.0.0-next.2|solid2|floor` | 4 | 4 | 4 | 0 |
| `@solid-primitives/devices@3.0.0-next.2|solid2|head` | 4 | 4 | 4 | 0 |
| `@solid-primitives/event-bus@1.1.4|solid1|only` | 11 | 5 | 2 | 2 |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|floor` | 11 | 6 | 4 | 2 |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|head` | 11 | 6 | 4 | 2 |
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
| `@solid-primitives/intersection-observer@2.2.5|solid1|only` | 11 | 7 | 3 | 0 |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|floor` | 12 | 5 | 5 | 0 |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|head` | 12 | 5 | 5 | 0 |
| `@solid-primitives/jsx-parser@0.2.0|solid1|only` | 4 | 2 | 3 | 0 |
| `@solid-primitives/jsx-tokenizer@1.1.4|solid1|only` | 4 | 2 | 1 | 0 |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|floor` | 4 | 2 | 2 | 0 |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|head` | 4 | 2 | 2 | 0 |
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
| `@solid-primitives/map@0.7.4|solid1|only` | 4 | 4 | 0 | 0 |
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
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|floor` | 2 | 2 | 0 | 0 |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|head` | 2 | 2 | 0 | 0 |
| `@solid-primitives/notification@1.0.0-next.3|solid2|floor` | 4 | 2 | 2 | 0 |
| `@solid-primitives/notification@1.0.0-next.3|solid2|head` | 4 | 2 | 2 | 0 |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|floor` | 4 | 1 | 1 | 0 |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|head` | 4 | 1 | 1 | 0 |
| `@solid-primitives/page-visibility@2.1.6|solid1|only` | 2 | 0 | 0 | 0 |
| `@solid-primitives/permission@1.3.2|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/permission@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/permission@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/platform@0.2.1|solid1|only` | 23 | 0 | 0 | 0 |
| `@solid-primitives/platform@1.0.0-next.2|solid2|floor` | 23 | 0 | 0 | 0 |
| `@solid-primitives/platform@1.0.0-next.2|solid2|head` | 23 | 0 | 0 | 0 |
| `@solid-primitives/pointer@0.3.6|solid1|only` | 7 | 7 | 0 | 0 |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|floor` | 7 | 4 | 1 | 0 |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|head` | 7 | 4 | 1 | 0 |
| `@solid-primitives/presence@0.1.4|solid1|only` | 1 | 1 | 2 | 0 |
| `@solid-primitives/presence@1.0.0-next.2|solid2|floor` | 1 | 1 | 2 | 0 |
| `@solid-primitives/presence@1.0.0-next.2|solid2|head` | 1 | 1 | 2 | 0 |
| `@solid-primitives/promise@2.0.0-next.2|solid2|floor` | 7 | 3 | 4 | 0 |
| `@solid-primitives/promise@2.0.0-next.2|solid2|head` | 7 | 3 | 4 | 0 |
| `@solid-primitives/props@3.2.4|solid1|only` | 6 | 3 | 2 | 0 |
| `@solid-primitives/props@4.0.0-next.3|solid2|floor` | 8 | 4 | 4 | 0 |
| `@solid-primitives/props@4.0.0-next.3|solid2|head` | 8 | 4 | 4 | 0 |
| `@solid-primitives/queue@1.0.0-next.3|solid2|floor` | 6 | 5 | 5 | 0 |
| `@solid-primitives/queue@1.0.0-next.3|solid2|head` | 6 | 5 | 5 | 0 |
| `@solid-primitives/raf@2.3.5|solid1|only` | 4 | 4 | 5 | 0 |
| `@solid-primitives/raf@4.0.0-next.2|solid2|floor` | 4 | 4 | 5 | 0 |
| `@solid-primitives/raf@4.0.0-next.2|solid2|head` | 4 | 4 | 5 | 0 |
| `@solid-primitives/reducer@0.0.101|solid1|only` | 1 | 1 | 2 | 0 |
| `@solid-primitives/resize-observer@2.2.0|solid1|only` | 7 | 4 | 2 | 0 |
| `@solid-primitives/resource@0.4.3|solid1|only` | 8 | 7 | 1 | 0 |
| `@solid-primitives/scheduled@1.5.3|solid1|only` | 6 | 5 | 5 | 0 |
| `@solid-primitives/script-loader@2.3.2|solid1|only` | 1 | 0 | 0 | 0 |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/scroll@2.1.6|solid1|only` | 5 | 2 | 1 | 0 |
| `@solid-primitives/selection@0.1.3|solid1|only` | 2 | 1 | 1 | 0 |
| `@solid-primitives/selection@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 |
| `@solid-primitives/selection@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|floor` | 10 | 6 | 7 | 0 |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|head` | 10 | 6 | 7 | 0 |
| `@solid-primitives/set@0.7.4|solid1|only` | 4 | 4 | 0 | 0 |
| `@solid-primitives/share@2.2.5|solid1|only` | 35 | 2 | 2 | 0 |
| `@solid-primitives/sse@0.0.103|solid1|only` | 10 | 1 | 2 | 0 |
| `@solid-primitives/sse@1.0.0-next.2|solid2|floor` | 15 | 3 | 6 | 1 |
| `@solid-primitives/sse@1.0.0-next.2|solid2|head` | 15 | 3 | 6 | 1 |
| `@solid-primitives/state-machine@0.1.1|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 |
| `@solid-primitives/storage@4.4.0|solid1|only` | 11 | 8 | 0 | 0 |
| `@solid-primitives/storage@5.0.0-next.4|solid2|floor` | 11 | 4 | 2 | 0 |
| `@solid-primitives/storage@5.0.0-next.4|solid2|head` | 11 | 4 | 2 | 0 |
| `@solid-primitives/stream@0.7.4|solid1|only` | 5 | 4 | 5 | 0 |
| `@solid-primitives/styles@0.1.4|solid1|only` | 4 | 0 | 0 | 0 |
| `@solid-primitives/styles@1.0.0-next.2|solid2|floor` | 4 | 0 | 0 | 0 |
| `@solid-primitives/styles@1.0.0-next.2|solid2|head` | 4 | 0 | 0 | 0 |
| `@solid-primitives/throttle@1.2.0|solid1|only` | 1 | 1 | 1 | 0 |
| `@solid-primitives/transition-group@1.1.2|solid1|only` | 2 | 2 | 3 | 0 |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|floor` | 2 | 2 | 4 | 0 |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|head` | 2 | 2 | 4 | 0 |
| `@solid-primitives/trigger@1.2.4|solid1|only` | 3 | 2 | 1 | 0 |
| `@solid-primitives/tween@1.4.1|solid1|only` | 2 | 2 | 2 | 0 |
| `@solid-primitives/tween@2.0.0-next.2|solid2|floor` | 1 | 1 | 2 | 0 |
| `@solid-primitives/tween@2.0.0-next.2|solid2|head` | 1 | 1 | 2 | 0 |
| `@solid-primitives/upload@0.1.5|solid1|only` | 3 | 3 | 3 | 0 |
| `@solid-primitives/url@0.2.0-next.2|solid2|floor` | 12 | 4 | 1 | 0 |
| `@solid-primitives/url@0.2.0-next.2|solid2|head` | 12 | 4 | 1 | 0 |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|floor` | 6 | 2 | 4 | 0 |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|head` | 6 | 2 | 4 | 0 |
| `@solid-primitives/visibility-observer@2.0.1|solid1|only` | 2 | 1 | 1 | 0 |
| `@solid-primitives/websocket@1.4.0|solid1|only` | 6 | 2 | 2 | 0 |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|floor` | 10 | 5 | 5 | 0 |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|head` | 10 | 5 | 5 | 0 |
| `@solid-primitives/workers@0.4.3|solid1|only` | 3 | 3 | 0 | 0 |
| `@solid-primitives/workers@2.0.1-next.1|solid2|floor` | 5 | 3 | 4 | 0 |
| `@solid-primitives/workers@2.0.1-next.1|solid2|head` | 5 | 3 | 4 | 0 |
| `@solidjs/element@2.0.0-rc.1|solid2|only` | 5 | 2 | 1 | 0 |
| `@solidjs/h@2.0.0-rc.1|solid2|only` | 9 | 0 | 0 | 0 |
| `@solidjs/meta@0.29.4|solid1|only` | 9 | 7 | 2 | 0 |
| `@solidjs/meta@1.0.0-next.2|solid2|floor` | 8 | 7 | 0 | 0 |
| `@solidjs/meta@1.0.0-next.2|solid2|head` | 8 | 7 | 0 | 0 |
| `@solidjs/router@2.0.0-next.17|solid2|only` | 30 | 29 | 26 | 0 |
| `@solidjs/universal@2.0.0-rc.1|solid2|only` | 1 | 0 | 0 | 0 |
| `@tanstack/ai-solid@0.18.3|solid1|only` | 21 | 5 | 4 | 0 |
| `@tanstack/solid-ai-devtools@0.2.70|solid1|only` | 4 | 0 | 0 | 0 |
| `@tanstack/solid-db@0.2.37|solid1|only` | 207 | 127 | 18 | 2 |
| `@tanstack/solid-devtools@0.8.12|solid1|only` | 1 | 0 | 0 | 0 |
| `@tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only` | 1 | 0 | 0 | 0 |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor` | 2 | 0 | 0 | 0 |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|head` | 2 | 0 | 0 | 0 |
| `@tanstack/solid-query@5.101.4|solid1|only` | 57 | 43 | 43 | 1 |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|floor` | 57 | 47 | 55 | 1 |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|head` | 57 | 47 | 55 | 1 |
| `@tanstack/solid-router-ssr-query@1.167.2-pre.0|solid1|only` | 1 | 0 | 0 | 0 |
| `motion-solidjs@0.7.0-beta.4|solid2|floor` | 357 | 333 | 0 | 0 |
