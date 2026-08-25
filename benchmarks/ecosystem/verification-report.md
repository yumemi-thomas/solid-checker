# Ecosystem machine-verification report

How many real ecosystem packages machine-verify end to end: `contract generate` -> `contract probe --write` -> `contract verify`, run against a throwaway install of every probe row in the pinned corpus.

> **This measurement executes package code.** `contract probe` imports and runs each
> installed package, and its dependencies, in child processes. Every install and every
> execution happened inside temporary directories under the harness state directory, npm
> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran
> under both a per-mode timeout and a whole-phase wall budget.

- Started: 2026-08-25T21:22:54.444Z
- Finished: 2026-08-25T21:31:47.508Z
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Probe rows run: 416
- Checker native binary: `1ae57d08854302148fd7613a4c628c52e569cdd80c4067019b89856eea4c4a83` (73643504 bytes, mtime 2026-08-25T21:22:41.294Z)
- Type Facts binary: `31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889` (28390098 bytes, mtime 2026-08-25T12:41:36.432Z)
- Budgets: install 240000 ms, generate 120000 ms, probe 20000 ms per condition mode / 90000 ms + 500 ms per planned claim, capped at 900000 ms, whole phase, verify 90000 ms; concurrency 6
- Import-environment shim: enabled (client, development and production sessions only; server sessions never)

## Headline

| Figure | Count |
| --- | --- |
| Probe rows run | 416 |
| Reached a generated contract | 395/416 (94.95%) |
| **Reached `verified`** | **284/416 (68.27%)** of all rows |
| Reached `verified`, of rows that produced a contract | 284/395 (71.90%) |
| Refused by `contract verify` | 109/416 (26.20%) |

Outcome classes, raw:

| Outcome | Rows |
| --- | --- |
| `verified` | 284 |
| `refused` | 109 |
| `generate-failure` | 15 |
| `install-failure` | 6 |
| `no-runtime` | 2 |

## Per family

| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 23 | 11/23 (47.83%) | 10 | 1669/2519 (66.26%) | 1668/1669 (99.94%) | 121 | 525 | 188 |
| Kobalte | 6 | 3 | 1/6 (16.67%) | 2 | 1149/1567 (73.32%) | 1148/1149 (99.91%) | 2 | 7 | 3 |
| Solid Primitives | 289 | 281 | 220/289 (76.12%) | 61 | 1941/3118 (62.25%) | 1935/1941 (99.69%) | 498 | 627 | 743 |
| Corvu | 28 | 28 | 18/28 (64.29%) | 10 | 289/426 (67.84%) | 289/289 (100.00%) | 36 | 60 | 60 |
| TanStack | 52 | 45 | 26/52 (50.00%) | 19 | 2120/2735 (77.51%) | 2120/2120 (100.00%) | 127 | 567 | 379 |
| Solid Devtools | 12 | 9 | 6/12 (50.00%) | 3 | 127/274 (46.35%) | 127/127 (100.00%) | 6 | 29 | 18 |
| Solid Recharts | 3 | 3 | 1/3 (33.33%) | 2 | 124/364 (34.07%) | 124/124 (100.00%) | 29 | 6 | 103 |
| Motion for Solid | 3 | 3 | 1/3 (33.33%) | 2 | 908/966 (94.00%) | 908/908 (100.00%) | 0 | 24 | 333 |

| Solid target | Rows | Contracts | Verified | Refused |
| --- | --- | --- | --- | --- |
| solid1 | 168 | 151 | 111/168 (66.07%) | 40 |
| solid2 | 248 | 244 | 173/248 (69.76%) | 69 |

## Why verification refuses

109 rows were refused. `contract verify` raises every blocker it finds rather than stopping at the first, so the row counts below sum to more than the number of refused rows.

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |
| --- | --- | --- |
| `kind-observed` | 60 | 130 |
| `probe-report-includes-evidence-write` | 45 | 45 |
| `incompleteness` | 38 | 589 |
| `probe-failed` | 8 | 8 |
| `attested-closure-note` | 3 | 9 |
| `closure-note` | 2 | 5 |

Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a *consequence*: `contract probe --write` declines to write evidence once a probe failed or an incompleteness was reported, so verification then sees passing claims that never reached the contract. It is counted as a root cause only on a row where it stands alone.

| Root cause | Refused rows |
| --- | --- |
| `kind-observed` | 60 |
| `incompleteness` | 37 |
| `probe-failed` | 8 |
| `closure-note` | 2 |
| `attested-closure-note` | 2 |

## Drivability

| Figure | Count |
| --- | --- |
| Claims planned across every probed contract | 11969 |
| Driven | 8327/11969 (69.57%) |
| Passed | 8319/11969 (69.50%) |
| Failed | 8 |
| Undriven | 3642/11969 (30.43%) |
| Incompleteness findings | 589 |

Undriven claims by reason:

| Reason | Claims |
| --- | --- |
| no probe form: reactiveReads | 1113 |
| entrypoint import threw | 601 |
| no probe form: ownerRequirements | 465 |
| synthesized call threw | 380 |
| no probe form: nested return leaf | 270 |
| synthesized call did not invoke the callback | 237 |
| no plantable reactive source | 235 |
| no probe form: asyncBehavior | 96 |
| no unambiguous summary for the mode | 56 |
| probe session aborted by package code | 38 |
| runtime re-runs nothing in this mode | 37 |
| probe session wrote no report | 35 |
| callback ran more often than the call site | 25 |
| no probe form: store path | 23 |
| no probe form: callback arguments | 13 |
| planted write was never re-read | 7 |
| callback re-ran with nothing written | 6 |
| probe session hit the per-mode timeout | 3 |
| callback ownership ambiguous in the driver's read scope | 2 |

`no probe form: reactiveReads` and `no probe form: ownerRequirements` are family-A compiler proofs that verification retains; *undriven* means no independent generic runtime probe exists for them, not that the verified contract discarded those static claims. The other rows must be read by their named reason: some become unknown, while a failed claim or incompleteness remains a blocker.

### Why a `kind` observation is missing

`kind` is the one claim schema v1 has no unknown sentinel for, so an unobserved one blocks rather than converting — which makes *why* it was unobserved the number the rule's next revision turns on. An **observation of absence** (`export-missing`: the namespace loaded and the binding was not in it) says the export does not exist in that artifact, so there is no consumer claim about that mode to certify. Every other non-observation is a **gap** — an import that threw, a session that died, a mode never attempted, a mode where no unambiguous summary resolves — and a gap must keep blocking. Every number in this section counts gaps only: a mode that was observed and *disagreed* is a failing claim, and it has its own section below rather than a row here, because amendment A9 forbids the two sharing a number.

- Rows with at least one gap in a stated `kind` mode: 83
- `kind` obligations with at least one gapped stated mode: 2261

| Why the mode produced no passing `kind` observation | (claim, mode) pairs |
| --- | --- |
| entrypoint import threw | 3819 |
| probe session wrote no report | 116 |
| probe session aborted by package code | 93 |
| no unambiguous summary resolves in the mode (no kind claim exists) | 56 |
| export-missing in this mode | 45 |
| probe session hit the per-mode timeout | 8 |

| Mode | Gapped `kind` obligations |
| --- | --- |
| `server` | 2170 |
| `production` | 684 |
| `development` | 644 |
| `client` | 639 |

### `kind` claims the probe contradicted

A mode whose observation **exists and disagreed** with the contract. Nothing above counts these, and nothing in any relaxation of the `kind` rule may absorb them: the package answered the claim differently, which is a generator bug or a package change, and neither is fixed by narrowing a mode away or converting a claim to unknown. They refuse the whole document today and must keep doing so.

- Rows with at least one contradicted `kind` claim: 1
- `kind` claims contradicted in at least one mode: 1

| Mode | Contradicted `kind` claims |
| --- | --- |
| `client` | 1 |
| `development` | 1 |
| `production` | 1 |

## The probe environment

An entrypoint whose module cannot be imported yields no observation at all. 33 of the corpus's rows had at least one entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or TypeScript loader, and only the packages the corpus manifest installs beside the probed one. Some of these throws are facts about the package; others are facts about that environment, and the two are not separated here.

| Import failure | Claims left undriven |
| --- | --- |
| Error [ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING]: Stripping types is currently unsupported for files under node_modules, | 227 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@solid-primitives/utils' imported from /private/t | 84 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: Package subpath './web' is not defined by "exports" in <path> | 81 |
| Error: [solid-devtools]: Debugger hasn't found the exposed Solid Devtools API | 66 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'server-only' imported from <path> | 60 |
| TypeError [ERR_UNKNOWN_FILE_EXTENSION]: Unknown file extension ".jsx" for <path> | 28 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'react' imported from <path> | 12 |
| SyntaxError: The requested module 'solid-js' does not provide an export named 'onSe | 10 |
| Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: No "exports" main defined in <path> | 4 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@angular/core' imported from <path> | 4 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'preact' imported from <path> | 3 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vue' imported from <path> | 3 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package '@rsbuild/core' imported from <path> | 3 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'vite' imported from <path> | 3 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'octane' imported from <path> | 3 |
| Error [ERR_UNSUPPORTED_ESM_URL_SCHEME]: Only URLs with a scheme in: file, data, and node are supported by the  | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find module '<path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'svelte' imported from <path> | 2 |
| Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'lit' imported from <path> | 2 |
| SyntaxError: The requested module '@tanstack/router-generator' does not provide an  | 1 |

### The globals the probe worker faked

A module that reads `window` while it is being evaluated throws in a bare Node process, the worker stops, and every claim of that entrypoint goes undriven — so nothing at all is observed about the package. The worker therefore defines a small inert browser surface before it imports anything, in the `client`, `development` and `production` sessions only.

**A claim observed under the shim is a weaker observation than one made in a browser.** The fake `document` renders nothing, the fake `matchMedia` never matches, the fake `navigator` says it is this checker. A package that branches on any of that was observed on the branch the fake sent it down. Every `<contract>.probe.json` and `<contract>.verify.json` records the per-mode list of faked names, so where the distinction matters the record says so rather than the number implying a browser.

`server` sessions are never shimmed: an import that throws on `window` under `--conditions node` is a truthful observation of that entrypoint in that mode, and faking it there would manufacture a pass the package never earned.

- Rows where at least one session faked at least one global: 390

| Faked global | Rows |
| --- | --- |
| `IntersectionObserver` | 390 |
| `MutationObserver` | 390 |
| `ResizeObserver` | 390 |
| `cancelAnimationFrame` | 390 |
| `document` | 390 |
| `getComputedStyle` | 390 |
| `history` | 390 |
| `localStorage` | 390 |
| `location` | 390 |
| `matchMedia` | 390 |
| `requestAnimationFrame` | 390 |
| `screen` | 390 |
| `self` | 390 |
| `sessionStorage` | 390 |
| `window` | 390 |

### Worker processes

A worker stops at its first throw and the mode is restarted for what is left — the only way to un-halt a Solid 2.0 development runtime. A restart is not a failure; a row that needed many is the shape behind a slow or timed-out probe.

| Figure | Count |
| --- | --- |
| Worker processes started | 19555 |
| Of those, restarts after a throw | 16308 |
| Sessions that died (crash, timeout, unreadable output) | 65 |

## The install environment

Each row installs the pinned package, the Solid runtime the manifest row pins, and the non-optional peers the installed artifact's own `package.json` declares. Peers are installed in a second npm invocation so that no peer range can take part in resolving the pinned versions; if it moves a pin anyway, the pinned-only tree is restored and the row is recorded as such.

| Figure | Rows |
| --- | --- |
| Solid 2 rows given the `@solidjs/web` half of the runtime the row pinned only half of | 53 |
| Rows with a completed peer install | 23 |
| Peer packages installed | 31 |
| Rows whose peer install failed or moved a pin | 5 |

A package that **imports something it declares nowhere** — not a dependency, not a peer — is outside what any install policy can supply, and is reported above as an import throw rather than fixed here. Completing an undeclared import would mean this harness choosing a version the package never named.

## Probe failures: claims the package answered differently

A **failure** is the strongest thing this measurement produces. The contract states a claim, the probe drove it, and the package did something else — a generator bug or a package change, never an environment gap and never an unreachable claim. Verification refuses the whole contract on one of these, deliberately: converting a contradicted claim to the unknown sentinel would hide it.

8 failing claim(s) across the corpus, by shape:

| Claim, claimed, observed | Claims |
| --- | --- |
| callbacks[n]: claimed tracked, observed inline | 3 |
| callbacks[n]: claimed deferred, observed inline | 2 |
| callbacks[n]: claimed deferred, observed tracked | 2 |
| kind: claimed function, observed value | 1 |

Each one, in full:

| Probe | Export | Claim | Observed | Modes |
| --- | --- | --- | --- | --- |
| `@solid-primitives/pagination@0.5.2|solid1|only` | `.:createInfiniteScroll` | `callbacks[0]=deferred` | inline | client, development, production |
| `@solidjs/testing-library@0.8.10|solid1|only` | `.:testEffect` | `callbacks[0]=deferred` | inline | client, development, production |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | `.:createWritableMemo` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | `.:createWritableMemo` | `callbacks[0]=deferred` | tracked | client, development, production |
| `@solid-primitives/date-difference@1.0.2|solid1|only` | `.:createDateNow` | `callbacks[0]=tracked` | inline | client, development, production |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|floor` | `.:createInfiniteScroll` | `callbacks[0]=tracked` | inline | client, development, production |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|head` | `.:createInfiniteScroll` | `callbacks[0]=tracked` | inline | client, development, production |
| `@kobalte/core@2.0.0-alpha.0|solid2|only` | `./menubar:t` | `kind=function` | value | client, development, production |

## Conversion volume

A conversion replaces one export's whole claim domain with the `{"status":"unknown"}` sentinel because the probe neither observed nor statically proved it.

| Figure | Count |
| --- | --- |
| Claim domains converted to unknown | 819 |
| Exports carrying an unknown in the verified rows, at generation | 1511/4643 (32.54%) |
| Exports carrying an unknown in the verified rows, after verification | 1827/3672 (49.75%) |

How much a verified contract actually certifies from observation:

| Figure | Count |
| --- | --- |
| Verified rows carrying at least one probed behavioral row | 20/284 (7.04%) |
| Probed behavioral row markers kept across the whole corpus | 83 |
| Inferred row markers dropped by verification | 4351 |
| Probed markers discarded as unwitnessed by this run's report | 162 |
| Entrypoints verification refused inside a promoted document | 50 |
| Verified rows carrying at least one such refusal | 14 |

The last two rows are a **cost made visible, not a regression**. An entrypoint whose `kind` claims this run did not observe is refused and omitted, exactly as `contract generate` already refuses an entrypoint it cannot certify, so the package's other entrypoints are not sunk by one unimportable subpath. A refused entrypoint is absent from the contract, which is an explicit uncertifiable result at the consumer rather than a wrong claim; a document where *no* entrypoint would certify anything is still refused whole. The exports it dropped are their own state in the composite below, still inside its denominator: a certified *share* that rose because unobservable exports left the population would be measuring nothing.

Converted domains by field:

| Field | Conversions |
| --- | --- |
| `callbacks` | 424 |
| `returns` | 365 |
| `asyncBehavior` | 30 |

## The composite a consumer feels

Of every export the corpus's generated contracts describe:

| State | Exports |
| --- | --- |
| (a) certified by a verified contract | 1845/8691 (21.23%) |
| (b) honest unknown inside a verified contract | 1827/8691 (21.02%) |
| (c) dropped from a verified contract with its refused entrypoint | 971/8691 (11.17%) |
| (d) inside a contract that never reached `verified` | 4048/8691 (46.58%) |

(c) is the cost of amendment A9 stage 1 stated as a consumer-facing number: the row verified, and these exports are absent from the document it promoted, so importing one is an explicit uncertifiable result. They stay in the denominator — a certified *share* that rose because unobservable exports left the population would be measuring nothing. (d) is every export of a contract that was generated and then refused, timed out, or errored before a probe report existed. Rows whose `npm install` or `contract generate` failed describe no exports at all and are in none of the four states.

## Wall time

| Phase | Rows | Median | p90 | Max | Mean |
| --- | --- | --- | --- | --- | --- |
| install | 416 | 749 ms | 1700 ms | 24497 ms | 1039 ms |
| generate | 410 | 177 ms | 1826 ms | 120005 ms | 1498 ms |
| probe | 393 | 709 ms | 3491 ms | 203748 ms | 3535 ms |
| verify | 393 | 51 ms | 65 ms | 178 ms | 55 ms |
| pipelineWithoutInstall | 410 | 997 ms | 5617 ms | 245911 ms | 4938 ms |
| total | 416 | 1837 ms | 7859 ms | 248580 ms | 5948 ms |

`install` may run against a warm npm cache, so it is a lower bound; `pipelineWithoutInstall` is the number that describes the checker's own cost.

## Rows that never reached verification

| Stage | Rows |
| --- | --- |
| `npm install` failed | 6 |
| `contract generate` failed | 15 |
| `contract probe` errored before writing a report | 0 |
| no Solid runtime the row could honestly be probed against | 2 |
| timed out under the harness budget | 0 |

The manifest pins the runtime each row runs against, and for these it pins no `solid-js` — `@solidjs/signals` *is* the reactive core, so there is no second package to settle a probe with. Pairing one in would be this harness auditing a combination the corpus deliberately did not. They are their own class rather than an error:

- `@solidjs/signals@2.0.0-rc.1|solid2|floor`
- `@solidjs/signals@2.0.0-rc.1|solid2|head`

Generation failures by class:

| Class | Rows |
| --- | --- |
| `unclassified` | 10 |
| `no-esm-runtime-target` | 2 |
| `timeout` | 1 |
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

| Probe | Family | Root cause | Blocker lines | Classes | Kind gaps |
| --- | --- | --- | --- | --- | --- |
| `@corvu-next/otp-field@0.1.5|solid2|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@corvu-next/popover@0.1.5|solid2|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x13 |
| `@corvu-next/resizable@0.1.5|solid2|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@corvu-next/tooltip@0.1.5|solid2|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@corvu-next/utils@0.1.5|solid2|only` | corvu | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | entrypoint import threw x1 |
| `@corvu/otp-field@0.1.4|solid1|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@corvu/popover@0.2.0|solid1|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x13 |
| `@corvu/resizable@0.2.5|solid1|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@corvu/tooltip@0.2.2|solid1|only` | corvu | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@corvu/utils@0.4.2|solid1|only` | corvu | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | entrypoint import threw x1 |
| `@kobalte/core@0.13.13|solid1|only` | kobalte | `incompleteness` | 22 | incompleteness, probe-report-includes-evidence-write | entrypoint import threw x409 |
| `@kobalte/core@2.0.0-alpha.0|solid2|only` | kobalte | `probe-failed` | 25 | incompleteness, probe-failed, probe-report-includes-evidence-write | entrypoint import threw x333, **contradicted** x1 |
| `@solid-devtools/extension-adapter@0.12.1|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-devtools/frontend@0.15.4|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x3 |
| `@solid-devtools/logger@0.9.11|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@solid-primitives/controlled-props@0.1.4|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x28 |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x28 |
| `@solid-primitives/countdown@1.0.9|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date-difference@1.0.2|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@2.1.8|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x52 |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x52 |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 13 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 13 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x44 |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x44 |
| `@solid-primitives/fetch@2.5.2|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/focus@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x6 |
| `@solid-primitives/focus@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x6 |
| `@solid-primitives/graphql@3.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/history@0.2.5|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/history@1.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/history@1.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/immutable@2.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x9 |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x9 |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/pagination@0.5.2|solid1|only` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|floor` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|head` | solid-primitives | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solid-primitives/promise@1.1.4|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x8 |
| `@solid-primitives/refs@1.1.4|solid1|only` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/refs@3.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/refs@3.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x6 |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | probe session aborted by package code x6 |
| `@solid-primitives/share@4.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x140 |
| `@solid-primitives/share@4.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x140 |
| `@solid-primitives/signal-builders@0.2.4|solid1|only` | solid-primitives | `incompleteness` | 25 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|floor` | solid-primitives | `incompleteness` | 145 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/signal-builders@1.0.0-next.4|solid2|head` | solid-primitives | `incompleteness` | 148 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/sortable@1.0.0-next.0|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/start@0.0.4|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@solid-primitives/until@0.1.1|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-primitives/upload@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/upload@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/virtual@0.2.5|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x2 |
| `@solid-primitives/virtual@1.0.0-next.4|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@solid-primitives/virtual@1.0.0-next.4|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@solidjs/html@2.0.0-rc.1|solid2|only` | official-solid | `kind-observed` | 2 | kind-observed | entrypoint import threw x1 |
| `@solidjs/router@1.0.0|solid1|only` | official-solid | `kind-observed` | 2 | kind-observed | entrypoint import threw x38 |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|head` | official-solid | `kind-observed` | 2 | kind-observed | probe session aborted by package code x1 |
| `@solidjs/start@2.0.3|solid1|only` | official-solid | `closure-note` | 11 | attested-closure-note, closure-note | entrypoint import threw x332 |
| `@solidjs/testing-library@0.8.10|solid1|only` | official-solid | `probe-failed` | 2 | probe-failed, probe-report-includes-evidence-write | — |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|floor` | official-solid | `attested-closure-note` | 1 | attested-closure-note | — |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|head` | official-solid | `attested-closure-note` | 1 | attested-closure-note | — |
| `@tanstack/ai-solid-ui@0.7.18|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x36 |
| `@tanstack/charts@0.14.0|solid1|only` | tanstack | `closure-note` | 1 | closure-note | entrypoint import threw x62, probe session aborted by package code x4, no unambiguous summary resolves in the mode (no kind claim exists) x3 |
| `@tanstack/solid-charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x1 |
| `@tanstack/solid-form@2.0.0-alpha.2|solid1|only` | tanstack | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@tanstack/solid-pacer-devtools@0.14.0|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed | entrypoint import threw x2 |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor` | tanstack | `kind-observed` | 2 | kind-observed | probe session aborted by package code x3 |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head` | tanstack | `kind-observed` | 2 | kind-observed | probe session aborted by package code x3 |
| `@tanstack/solid-router-devtools@1.167.1|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-router-devtools@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4, probe session aborted by package code x3 |
| `@tanstack/solid-router@1.170.29|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed | entrypoint import threw x23 |
| `@tanstack/solid-router@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x120 |
| `@tanstack/solid-start-client@1.168.28|solid1|only` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x10, probe session aborted by package code x3 |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x10 |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x10 |
| `@tanstack/solid-start-config@1.120.20|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-start-server@1.167.35|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x39 |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x39 |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x39 |
| `@tanstack/solid-store@0.11.1|solid1|only` | tanstack | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `motion-solidjs@0.6.0|solid1|only` | motion-solidjs | `incompleteness` | 19 | incompleteness, probe-report-includes-evidence-write | — |
| `motion-solidjs@0.7.0-beta.4|solid2|head` | motion-solidjs | `incompleteness` | 31 | incompleteness, probe-report-includes-evidence-write | — |
| `solid-js@1.9.14|solid1|only` | official-solid | `incompleteness` | 12 | incompleteness, probe-report-includes-evidence-write | no unambiguous summary resolves in the mode (no kind claim exists) x3, export-missing in this mode x1 |
| `solid-js@2.0.0-rc.1|solid2|floor` | official-solid | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | no unambiguous summary resolves in the mode (no kind claim exists) x1 |
| `solid-js@2.0.0-rc.1|solid2|head` | official-solid | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | no unambiguous summary resolves in the mode (no kind claim exists) x1 |
| `solid-recharts@2.0.0-beta.1|solid2|floor` | solid-recharts | `kind-observed` | 2 | kind-observed | entrypoint import threw x436 |
| `solid-recharts@2.0.0-beta.1|solid2|head` | solid-recharts | `kind-observed` | 2 | kind-observed | entrypoint import threw x436 |

## Every verified contract

| Probe | Exports | Exports unknown | Conversions | Probed rows kept | Entrypoints refused |
| --- | --- | --- | --- | --- | --- |
| `@corvu-next/accordion@0.1.5|solid2|only` | 8 | 2 | 0 | 0 | — |
| `@corvu-next/calendar@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/dialog@0.1.5|solid2|only` | 10 | 1 | 0 | 0 | — |
| `@corvu-next/disclosure@0.1.5|solid2|only` | 5 | 1 | 0 | 0 | — |
| `@corvu-next/dismissible@0.1.5|solid2|only` | 2 | 2 | 0 | 0 | — |
| `@corvu-next/drawer@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/focus-trap@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/list@0.1.5|solid2|only` | 2 | 0 | 0 | 0 | — |
| `@corvu-next/persistent@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/presence@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/prevent-scroll@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu-next/transition-size@0.1.5|solid2|only` | 1 | 1 | 0 | 0 | — |
| `@corvu/accordion@0.2.5|solid1|only` | 8 | 5 | 6 | 0 | — |
| `@corvu/calendar@0.1.2|solid1|only` | 9 | 9 | 5 | 0 | — |
| `@corvu/dialog@0.2.4|solid1|only` | 10 | 2 | 2 | 0 | — |
| `@corvu/disclosure@0.2.2|solid1|only` | 5 | 2 | 2 | 0 | — |
| `@corvu/drawer@0.2.4|solid1|only` | 11 | 8 | 10 | 0 | — |
| `@kobalte/utils@2.0.0-alpha.0|solid2|only` | 10 | 3 | 2 | 0 | — |
| `@solid-devtools/debugger@0.28.1|solid1|only` | 5 | 5 | 0 | 0 | `.`, `./bundled`, `./index` |
| `@solid-devtools/overlay@0.33.5|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@solid-devtools/shared@0.20.0|solid1|only` | 21 | 8 | 6 | 0 | `./chunk-DTKGRNV6`, `./detect`, `./utils` |
| `@solid-devtools/transform@0.10.4|solid1|only` | 2 | 0 | 0 | 0 | — |
| `@solid-devtools/ui@0.10.3|solid1|only` | 13 | 1 | 0 | 0 | `.`, `./icons` |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|floor` | 7 | 3 | 2 | 0 | — |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|head` | 7 | 3 | 2 | 0 | — |
| `@solid-primitives/active-element@2.1.6|solid1|only` | 5 | 4 | 1 | 0 | — |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|floor` | 3 | 1 | 1 | 0 | — |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|head` | 3 | 1 | 1 | 0 | — |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|floor` | 10 | 2 | 2 | 0 | — |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|head` | 10 | 2 | 2 | 0 | — |
| `@solid-primitives/async@0.0.101-next.3|solid2|floor` | 6 | 4 | 5 | 0 | — |
| `@solid-primitives/async@0.0.101-next.3|solid2|head` | 6 | 4 | 5 | 0 | — |
| `@solid-primitives/audio@3.0.0-next.2|solid2|floor` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/audio@3.0.0-next.2|solid2|head` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/autofocus@0.1.5|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/bounds@0.1.7|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/broadcast-channel@0.1.1|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|floor` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|head` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/clipboard@1.6.6|solid1|only` | 9 | 3 | 3 | 0 | — |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|floor` | 9 | 2 | 3 | 0 | — |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|head` | 9 | 2 | 3 | 0 | — |
| `@solid-primitives/connectivity@0.4.6|solid1|only` | 3 | 3 | 0 | 0 | — |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|floor` | 6 | 3 | 1 | 0 | — |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|head` | 6 | 3 | 1 | 0 | — |
| `@solid-primitives/context@0.3.2|solid1|only` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/context@2.0.0-next.2|solid2|floor` | 4 | 0 | 0 | 0 | — |
| `@solid-primitives/context@2.0.0-next.2|solid2|head` | 4 | 0 | 0 | 0 | — |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|floor` | 5 | 5 | 5 | 0 | — |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|head` | 5 | 5 | 5 | 0 | — |
| `@solid-primitives/cookies@0.0.3|solid1|only` | 4 | 3 | 0 | 0 | — |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/cursor@0.1.4|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/db-store@1.1.4|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/debounce@1.3.0|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/deep@0.3.7|solid1|only` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/deep@1.0.0-next.3|solid2|floor` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/deep@1.0.0-next.3|solid2|head` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/destructure@0.2.4|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@solid-primitives/devices@1.3.1|solid1|only` | 6 | 6 | 6 | 0 | — |
| `@solid-primitives/devices@3.0.0-next.2|solid2|floor` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/devices@3.0.0-next.2|solid2|head` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/event-bus@1.1.4|solid1|only` | 11 | 7 | 4 | 0 | — |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|floor` | 11 | 7 | 5 | 1 | — |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|head` | 11 | 7 | 5 | 1 | — |
| `@solid-primitives/event-dispatcher@0.1.1|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-listener@2.4.6|solid1|only` | 11 | 11 | 3 | 0 | — |
| `@solid-primitives/event-props@0.3.1|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/filesystem@1.3.4|solid1|only` | 15 | 10 | 6 | 0 | — |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|floor` | 15 | 7 | 6 | 0 | — |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|head` | 15 | 7 | 6 | 0 | — |
| `@solid-primitives/flux-store@0.1.1|solid1|only` | 4 | 3 | 2 | 0 | — |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|floor` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|head` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/form@1.0.0-next.2|solid2|floor` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/form@1.0.0-next.2|solid2|head` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/fullscreen@1.3.5|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|floor` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|head` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/geolocation@1.5.5|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|floor` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|head` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/gestures@1.2.1|solid1|only` | 9 | 7 | 1 | 0 | — |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|floor` | 11 | 1 | 1 | 0 | — |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|head` | 11 | 1 | 1 | 0 | — |
| `@solid-primitives/i18n@2.2.1|solid1|only` | 9 | 4 | 2 | 3 | — |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|floor` | 12 | 2 | 2 | 4 | — |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|head` | 12 | 2 | 2 | 4 | — |
| `@solid-primitives/idle@0.2.3|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/idle@1.0.0-next.3|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/idle@1.0.0-next.3|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/input-mask@0.3.1|solid1|only` | 7 | 2 | 1 | 0 | — |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|floor` | 7 | 2 | 2 | 0 | — |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|head` | 7 | 2 | 2 | 0 | — |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|floor` | 12 | 4 | 4 | 0 | — |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|head` | 12 | 4 | 4 | 0 | — |
| `@solid-primitives/jsx-parser@0.2.0|solid1|only` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/jsx-tokenizer@1.1.4|solid1|only` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/keyboard@1.3.7|solid1|only` | 6 | 6 | 1 | 0 | — |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|floor` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|head` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/keyed@1.5.3|solid1|only` | 6 | 6 | 4 | 0 | — |
| `@solid-primitives/lifecycle@0.1.2|solid1|only` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|floor` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|head` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list@0.1.2|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list@1.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list@1.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/local-store@1.1.4|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/map@0.7.4|solid1|only` | 4 | 4 | 0 | 0 | — |
| `@solid-primitives/map@1.0.0-next.2|solid2|floor` | 4 | 2 | 0 | 0 | — |
| `@solid-primitives/map@1.0.0-next.2|solid2|head` | 4 | 2 | 0 | 0 | — |
| `@solid-primitives/marker@0.2.2|solid1|only` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/marker@2.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/marker@2.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/masonry@0.1.4|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/masonry@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/masonry@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/match@0.0.100|solid1|only` | 3 | 3 | 0 | 0 | — |
| `@solid-primitives/match@1.0.0-next.2|solid2|floor` | 3 | 3 | 3 | 0 | — |
| `@solid-primitives/match@1.0.0-next.2|solid2|head` | 3 | 0 | 0 | 0 | — |
| `@solid-primitives/media@2.3.6|solid1|only` | 6 | 4 | 0 | 0 | — |
| `@solid-primitives/media@4.0.0-next.2|solid2|floor` | 6 | 1 | 0 | 0 | — |
| `@solid-primitives/media@4.0.0-next.2|solid2|head` | 6 | 1 | 0 | 0 | — |
| `@solid-primitives/memo@1.5.1|solid1|only` | 12 | 12 | 11 | 0 | — |
| `@solid-primitives/mouse@2.1.7|solid1|only` | 8 | 8 | 1 | 0 | — |
| `@solid-primitives/mutable@1.1.1|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/mutation-observer@1.2.4|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|floor` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|head` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/notification@1.0.0-next.3|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/notification@1.0.0-next.3|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|floor` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|head` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/page-visibility@2.1.6|solid1|only` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/permission@1.3.2|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/permission@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/permission@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/pointer@0.3.6|solid1|only` | 7 | 7 | 0 | 0 | — |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|floor` | 7 | 4 | 1 | 0 | — |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|head` | 7 | 4 | 1 | 0 | — |
| `@solid-primitives/presence@0.1.4|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/presence@1.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/presence@1.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/promise@2.0.0-next.2|solid2|floor` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/promise@2.0.0-next.2|solid2|head` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/props@3.2.4|solid1|only` | 6 | 3 | 2 | 0 | — |
| `@solid-primitives/props@4.0.0-next.3|solid2|floor` | 8 | 4 | 4 | 0 | — |
| `@solid-primitives/props@4.0.0-next.3|solid2|head` | 8 | 4 | 4 | 0 | — |
| `@solid-primitives/queue@1.0.0-next.3|solid2|floor` | 6 | 5 | 6 | 0 | — |
| `@solid-primitives/queue@1.0.0-next.3|solid2|head` | 6 | 5 | 6 | 0 | — |
| `@solid-primitives/raf@2.3.5|solid1|only` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/raf@4.0.0-next.2|solid2|floor` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/raf@4.0.0-next.2|solid2|head` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/range@0.2.5|solid1|only` | 6 | 6 | 4 | 0 | — |
| `@solid-primitives/range@1.0.0-next.3|solid2|floor` | 7 | 6 | 6 | 0 | — |
| `@solid-primitives/range@1.0.0-next.3|solid2|head` | 7 | 6 | 6 | 0 | — |
| `@solid-primitives/reducer@0.0.101|solid1|only` | 1 | 1 | 2 | 0 | — |
| `@solid-primitives/resize-observer@2.2.0|solid1|only` | 7 | 5 | 2 | 0 | — |
| `@solid-primitives/resource@0.4.3|solid1|only` | 8 | 7 | 2 | 0 | — |
| `@solid-primitives/rootless@1.5.4|solid1|only` | 8 | 8 | 6 | 0 | — |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | 8 | 8 | 7 | 0 | — |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | 8 | 8 | 7 | 0 | — |
| `@solid-primitives/scheduled@1.5.3|solid1|only` | 6 | 6 | 5 | 0 | — |
| `@solid-primitives/script-loader@2.3.2|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/scroll@2.1.6|solid1|only` | 5 | 2 | 0 | 0 | — |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|floor` | 6 | 2 | 1 | 0 | — |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|head` | 6 | 2 | 1 | 0 | — |
| `@solid-primitives/selection@0.1.3|solid1|only` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/selection@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/selection@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|floor` | 10 | 6 | 7 | 0 | — |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|head` | 10 | 6 | 7 | 0 | — |
| `@solid-primitives/set@0.7.4|solid1|only` | 4 | 4 | 0 | 0 | — |
| `@solid-primitives/set@1.0.0-next.2|solid2|floor` | 9 | 6 | 4 | 1 | — |
| `@solid-primitives/set@1.0.0-next.2|solid2|head` | 9 | 6 | 4 | 1 | — |
| `@solid-primitives/share@2.2.5|solid1|only` | 35 | 2 | 2 | 0 | — |
| `@solid-primitives/spring@0.1.2|solid1|only` | 2 | 2 | 3 | 0 | — |
| `@solid-primitives/spring@1.0.0-next.3|solid2|floor` | 3 | 3 | 5 | 0 | — |
| `@solid-primitives/spring@1.0.0-next.3|solid2|head` | 3 | 3 | 5 | 0 | — |
| `@solid-primitives/sse@0.0.103|solid1|only` | 10 | 7 | 1 | 0 | — |
| `@solid-primitives/sse@1.0.0-next.2|solid2|floor` | 15 | 10 | 5 | 0 | — |
| `@solid-primitives/sse@1.0.0-next.2|solid2|head` | 15 | 10 | 5 | 0 | — |
| `@solid-primitives/state-machine@0.1.1|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/state-machine@1.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/static-store@0.1.4|solid1|only` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|floor` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/static-store@1.0.0-next.2|solid2|head` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/storage@4.4.0|solid1|only` | 11 | 8 | 0 | 0 | — |
| `@solid-primitives/storage@5.0.0-next.4|solid2|floor` | 11 | 3 | 1 | 0 | — |
| `@solid-primitives/storage@5.0.0-next.4|solid2|head` | 11 | 3 | 1 | 0 | — |
| `@solid-primitives/stream@0.7.4|solid1|only` | 5 | 4 | 5 | 0 | — |
| `@solid-primitives/styles@0.1.4|solid1|only` | 4 | 2 | 0 | 0 | — |
| `@solid-primitives/styles@1.0.0-next.2|solid2|floor` | 4 | 2 | 0 | 0 | — |
| `@solid-primitives/styles@1.0.0-next.2|solid2|head` | 4 | 2 | 0 | 0 | — |
| `@solid-primitives/throttle@1.2.0|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/timer@1.4.4|solid1|only` | 5 | 5 | 4 | 0 | — |
| `@solid-primitives/timer@1.4.5-next.1|solid2|floor` | 5 | 5 | 4 | 0 | — |
| `@solid-primitives/timer@1.4.5-next.1|solid2|head` | 5 | 5 | 4 | 0 | — |
| `@solid-primitives/transition-group@1.1.2|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/transition-group@2.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/trigger@1.2.4|solid1|only` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|floor` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/trigger@3.0.0-next.2|solid2|head` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/tween@1.4.1|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/tween@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/tween@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/upload@0.1.5|solid1|only` | 3 | 3 | 3 | 0 | — |
| `@solid-primitives/url@0.2.0-next.2|solid2|floor` | 12 | 6 | 1 | 0 | — |
| `@solid-primitives/url@0.2.0-next.2|solid2|head` | 12 | 6 | 1 | 0 | — |
| `@solid-primitives/utils@6.4.1|solid1|only` | 75 | 49 | 13 | 0 | — |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | 99 | 27 | 16 | 5 | — |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | 99 | 27 | 16 | 5 | — |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|floor` | 6 | 2 | 4 | 0 | — |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|head` | 6 | 2 | 4 | 0 | — |
| `@solid-primitives/video@1.0.0-next.3|solid2|floor` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/video@1.0.0-next.3|solid2|head` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/visibility-observer@2.0.1|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/websocket@1.4.0|solid1|only` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|floor` | 10 | 5 | 5 | 0 | — |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|head` | 10 | 5 | 5 | 0 | — |
| `@solid-primitives/workers@0.4.3|solid1|only` | 3 | 3 | 0 | 0 | — |
| `@solid-primitives/workers@2.0.1-next.1|solid2|floor` | 5 | 3 | 4 | 0 | — |
| `@solid-primitives/workers@2.0.1-next.1|solid2|head` | 5 | 3 | 4 | 0 | — |
| `@solidjs/element@2.0.0-rc.1|solid2|only` | 5 | 5 | 1 | 0 | — |
| `@solidjs/h@2.0.0-rc.1|solid2|only` | 9 | 1 | 0 | 0 | — |
| `@solidjs/image@0.1.0|solid1|only` | 1 | 0 | 0 | 0 | `.` |
| `@solidjs/meta@0.29.4|solid1|only` | 9 | 7 | 2 | 0 | — |
| `@solidjs/meta@1.0.0-next.2|solid2|floor` | 8 | 7 | 0 | 0 | — |
| `@solidjs/meta@1.0.0-next.2|solid2|head` | 8 | 7 | 0 | 0 | — |
| `@solidjs/router@2.0.0-next.17|solid2|only` | 30 | 28 | 22 | 3 | — |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|floor` | 3 | 3 | 0 | 0 | — |
| `@solidjs/universal@2.0.0-rc.1|solid2|only` | 1 | 0 | 0 | 0 | — |
| `@solidjs/web@2.0.0-rc.1|solid2|floor` | 318 | 64 | 47 | 7 | `.`, `./frames`, `./server-functions` |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | 321 | 66 | 49 | 7 | `.`, `./frames`, `./server-functions` |
| `@tanstack/ai-solid@0.18.3|solid1|only` | 21 | 21 | 9 | 0 | — |
| `@tanstack/devtools-a11y@0.2.2|solid1|only` | 8 | 8 | 0 | 0 | `./angular`, `./react`, `./react/production` |
| `@tanstack/devtools-ui@0.7.1|solid1|only` | 8 | 0 | 0 | 0 | `.`, `./icons` |
| `@tanstack/devtools-utils@0.7.0|solid1|only` | 7 | 1 | 0 | 0 | `./preact`, `./react`, `./svelte`, `./vue` |
| `@tanstack/devtools@0.14.2|solid1|only` | 3 | 1 | 0 | 0 | — |
| `@tanstack/form-devtools@1.0.0-alpha.2|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/hotkeys-devtools@0.9.0|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@tanstack/pacer-devtools@1.4.0|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-ai-devtools@0.2.70|solid1|only` | 4 | 4 | 0 | 0 | — |
| `@tanstack/solid-db@0.2.37|solid1|only` | 207 | 120 | 10 | 0 | — |
| `@tanstack/solid-devtools@0.8.12|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@tanstack/solid-hotkeys@0.10.0|solid1|only` | 64 | 13 | 5 | 0 | — |
| `@tanstack/solid-pacer@0.22.0|solid1|only` | 108 | 36 | 21 | 19 | — |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|head` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-query@5.101.4|solid1|only` | 57 | 40 | 35 | 4 | — |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|floor` | 57 | 47 | 21 | 4 | — |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|head` | 57 | 47 | 21 | 4 | — |
| `@tanstack/solid-start@1.168.46|solid1|only` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|floor` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|head` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-table-devtools@9.2.0|solid1|only` | 3 | 1 | 0 | 0 | — |
| `@tanstack/solid-table@9.1.2|solid1|only` | 295 | 7 | 1 | 0 | `.` |
| `@tanstack/solid-virtual@3.13.37|solid1|only` | 17 | 4 | 3 | 1 | — |
| `@tanstack/table-devtools@9.2.0|solid1|only` | 10 | 10 | 1 | 0 | — |
| `corvu@0.7.2|solid1|only` | 43 | 21 | 11 | 0 | `./otp-field`, `./popover`, `./resizable`, `./tooltip` |
| `motion-solidjs@0.7.0-beta.4|solid2|floor` | 357 | 333 | 0 | 0 | — |
| `solid-devtools@0.34.5|solid1|only` | 5 | 3 | 0 | 0 | — |
| `solid-recharts@1.0.1|solid1|only` | 109 | 103 | 29 | 0 | — |
