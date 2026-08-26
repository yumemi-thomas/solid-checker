# Ecosystem machine-verification report

How many real ecosystem packages machine-verify end to end: `contract generate` -> `contract probe --write` -> `contract verify`, run against a throwaway install of every probe row in the pinned corpus.

> **This measurement executes package code.** `contract probe` imports and runs each
> installed package, and its dependencies, in child processes. Every install and every
> execution happened inside temporary directories under the harness state directory, Bun
> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran
> under both a per-mode timeout and a whole-phase wall budget.

- Started: 2026-08-26T10:05:01.608Z
- Finished: 2026-08-26T10:07:09.786Z
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Probe rows run: 416
- Checker native binary: `a4fd84a499b8f3ce97a3010cfe62da7e3d928c281d2293a63a30c6a6d6e8bbad` (15062544 bytes, mtime 2026-08-26T10:04:47.012Z)
- Type Facts binary: `983d0b702ace1476ecd7f5633e9e25b33003287b5319404851cdc5141d0d1844` (28446386 bytes, mtime 2026-08-26T08:23:47.354Z)
- Budgets: install 240000 ms, generate 120000 ms, probe 20000 ms per condition mode / 90000 ms + 500 ms per planned claim, capped at 900000 ms, whole phase, verify 90000 ms; concurrency 3
- Import-environment shim: enabled (client, development and production sessions only; server sessions never)

## Headline

| Figure | Count |
| --- | --- |
| Probe rows run | 416 |
| Reached a generated contract | 397/416 (95.43%) |
| **Reached `verified`** | **308/416 (74.04%)** of all rows |
| Reached `verified`, of rows that produced a contract | 308/397 (77.58%) |
| Refused by `contract verify` | 88/416 (21.15%) |

Outcome classes, raw:

| Outcome | Rows |
| --- | --- |
| `verified` | 308 |
| `refused` | 88 |
| `generate-failure` | 16 |
| `install-failure` | 3 |
| `no-runtime` | 1 |

## Per family

| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 20 | 13/23 (56.52%) | 6 | 1063/1616 (65.78%) | 1063/1063 (100.00%) | 118 | 338 | 263 |
| Kobalte | 6 | 4 | 1/6 (16.67%) | 3 | 1153/1756 (65.66%) | 1153/1153 (100.00%) | 2 | 7 | 3 |
| Solid Primitives | 289 | 280 | 236/289 (81.66%) | 44 | 1969/3310 (59.49%) | 1969/1969 (100.00%) | 501 | 619 | 844 |
| Corvu | 28 | 28 | 18/28 (64.29%) | 10 | 291/460 (63.26%) | 291/291 (100.00%) | 50 | 55 | 65 |
| TanStack | 52 | 51 | 32/52 (61.54%) | 19 | 1810/2353 (76.92%) | 1810/1810 (100.00%) | 142 | 633 | 451 |
| Solid Devtools | 12 | 9 | 6/12 (50.00%) | 3 | 157/271 (57.93%) | 157/157 (100.00%) | 20 | 96 | 36 |
| Solid Recharts | 3 | 3 | 0/3 (0.00%) | 3 | 124/366 (33.88%) | 124/124 (100.00%) | 0 | 0 | 0 |
| Motion for Solid | 3 | 2 | 2/3 (66.67%) | 0 | 330/330 (100.00%) | 330/330 (100.00%) | 0 | 1 | 329 |

| Solid target | Rows | Contracts | Verified | Refused |
| --- | --- | --- | --- | --- |
| solid1 | 168 | 154 | 118/168 (70.24%) | 36 |
| solid2 | 248 | 243 | 190/248 (76.61%) | 52 |

## Why verification refuses

88 rows were refused. `contract verify` raises every blocker it finds rather than stopping at the first, so the row counts below sum to more than the number of refused rows.

| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |
| --- | --- | --- |
| `kind-observed` | 54 | 118 |
| `probe-report-includes-evidence-write` | 30 | 30 |
| `incompleteness` | 30 | 503 |
| `attested-closure-note` | 4 | 10 |
| `closure-note` | 1 | 4 |

Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a *consequence*: `contract probe --write` declines to write evidence once a probe failed or an incompleteness was reported, so verification then sees passing claims that never reached the contract. It is counted as a root cause only on a row where it stands alone.

| Root cause | Refused rows |
| --- | --- |
| `kind-observed` | 54 |
| `incompleteness` | 30 |
| `attested-closure-note` | 3 |
| `closure-note` | 1 |

## Drivability

| Figure | Count |
| --- | --- |
| Claims planned across every probed contract | 10462 |
| Driven | 6897/10462 (65.92%) |
| Passed | 6897/10462 (65.92%) |
| Failed | 0 |
| Undriven | 3565/10462 (34.08%) |
| Incompleteness findings | 503 |

Undriven claims by reason:

| Reason | Claims |
| --- | --- |
| no plantable reactive source | 851 |
| entrypoint import threw | 588 |
| no probe form: reactiveReads | 571 |
| synthesized call threw | 525 |
| no probe form: ownerRequirements | 447 |
| synthesized call did not invoke the callback | 283 |
| no probe form: asyncBehavior | 71 |
| no probe form: store path | 47 |
| parameter member was not invoked | 45 |
| no unambiguous summary for the mode | 32 |
| runtime re-runs nothing in this mode | 29 |
| probe session aborted by package code | 25 |
| callback ran more often than the call site | 25 |
| probe session wrote no report | 10 |
| no probe form: callback arguments | 9 |
| planted write was never re-read | 5 |
| callback re-ran with nothing written | 2 |

`no probe form: reactiveReads` and `no probe form: ownerRequirements` are family-A compiler proofs that verification retains; *undriven* means no independent generic runtime probe exists for them, not that the verified contract discarded those static claims. The other rows must be read by their named reason: some become unknown, while a failed claim or incompleteness remains a blocker.

### Why a `kind` observation is missing

`kind` is the one claim schema v1 has no unknown sentinel for, so an unobserved one blocks rather than converting — which makes *why* it was unobserved the number the rule's next revision turns on. An **observation of absence** (`export-missing`: the namespace loaded and the binding was not in it) says the export does not exist in that artifact, so there is no consumer claim about that mode to certify. Every other non-observation is a **gap** — an import that threw, a session that died, a mode never attempted, a mode where no unambiguous summary resolves — and a gap must keep blocking. Every number in this section counts gaps only: a mode that was observed and *disagreed* is a failing claim, and it has its own section below rather than a row here, because amendment A9 forbids the two sharing a number.

- Rows with at least one gap in a stated `kind` mode: 71
- `kind` obligations with at least one gapped stated mode: 2124

| Why the mode produced no passing `kind` observation | (claim, mode) pairs |
| --- | --- |
| entrypoint import threw | 3873 |
| no unambiguous summary resolves in the mode (no kind claim exists) | 32 |
| export-missing in this mode | 23 |

| Mode | Gapped `kind` obligations |
| --- | --- |
| `server` | 2089 |
| `development` | 700 |
| `production` | 584 |
| `client` | 555 |

### `kind` claims the probe contradicted

A mode whose observation **exists and disagreed** with the contract. Nothing above counts these, and nothing in any relaxation of the `kind` rule may absorb them: the package answered the claim differently, which is a generator bug or a package change, and neither is fixed by narrowing a mode away or converting a claim to unknown. They refuse the whole document today and must keep doing so.

- Rows with at least one contradicted `kind` claim: 0
- `kind` claims contradicted in at least one mode: 0

## The probe environment

An entrypoint whose module cannot be imported yields no observation at all. 33 of the corpus's rows had at least one entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or TypeScript loader, and only the packages the corpus manifest installs beside the probed one. Some of these throws are facts about the package; others are facts about that environment, and the two are not separated here.

| Import failure | Claims left undriven |
| --- | --- |
| ResolveMessage: Cannot find package 'react' imported from <path> | 239 |
| ResolveMessage: Cannot find package 'solid-js' imported from <path> | 89 |
| ResolveMessage: Cannot find package '@solid-primitives/utils' imported from /private/t | 84 |
| Error: [solid-devtools]: Debugger hasn't found the exposed Solid Devtools API | 69 |
| ResolveMessage: Cannot find package 'server-only' imported from <path> | 60 |
| ResolveMessage: Cannot find package 'solid-start:get-manifest' imported from /private/ | 11 |
| ResolveMessage: Cannot find package 'esbuild' imported from <path> | 10 |
| SyntaxError: Export named 'onSettled' not found in module '<path> | 10 |
| ResolveMessage: Cannot find package '@rsbuild/core' imported from <path> | 3 |
| ResolveMessage: Cannot find package 'vite' imported from <path> | 3 |
| ResolveMessage: Cannot find package 'solid-start:seroval-plugins' imported from /priva | 2 |
| ResolveMessage: Cannot find package 'solid-start:routes' imported from <path> | 2 |
| ResolveMessage: Cannot find module '@solid-primitives/rootless' | 2 |
| ResolveMessage: Cannot find package 'virtual:solidbase' imported from <path> | 1 |
| ResolveMessage: Cannot find module '@solid-primitives/timer' | 1 |
| SyntaxError: Export named 'CONSTANTS' not found in module '<path> | 1 |
| ResolveMessage: Cannot find module '@solid-primitives/scheduled' | 1 |

### The globals the probe worker faked

A module that reads `window` while it is being evaluated throws in a bare Node process, the worker stops, and every claim of that entrypoint goes undriven — so nothing at all is observed about the package. The worker therefore defines a small inert browser surface before it imports anything, in the `client`, `development` and `production` sessions only.

**A claim observed under the shim is a weaker observation than one made in a browser.** The fake `document` renders nothing, the fake `matchMedia` never matches, the fake `navigator` says it is this checker. A package that branches on any of that was observed on the branch the fake sent it down. Every `<contract>.probe.json` and `<contract>.verify.json` records the per-mode list of faked names, so where the distinction matters the record says so rather than the number implying a browser.

`server` sessions are never shimmed: an import that throws on `window` under `--conditions node` is a truthful observation of that entrypoint in that mode, and faking it there would manufacture a pass the package never earned.

- Rows where at least one session faked at least one global: 393

| Faked global | Rows |
| --- | --- |
| `IntersectionObserver` | 393 |
| `MutationObserver` | 393 |
| `ResizeObserver` | 393 |
| `cancelAnimationFrame` | 393 |
| `document` | 393 |
| `getComputedStyle` | 393 |
| `history` | 393 |
| `localStorage` | 393 |
| `location` | 393 |
| `matchMedia` | 393 |
| `requestAnimationFrame` | 393 |
| `screen` | 393 |
| `sessionStorage` | 393 |
| `window` | 393 |

### Worker processes

A worker stops at its first throw and the mode is restarted for what is left — the only way to un-halt a Solid 2.0 development runtime. A restart is not a failure; a row that needed many is the shape behind a slow or timed-out probe.

| Figure | Count |
| --- | --- |
| Worker processes started | 16662 |
| Of those, restarts after a throw | 13890 |
| Sessions that died (crash, timeout, unreadable output) | 71 |

## The install environment

Each row installs the pinned package, the Solid runtime the manifest row pins, and the non-optional peers the installed artifact's own `package.json` declares. Peers are installed in a second Bun invocation so that no peer range can take part in resolving the pinned versions; if it moves a pin anyway, the pinned-only tree is restored and the row is recorded as such.

| Figure | Rows |
| --- | --- |
| Solid 2 rows given the `@solidjs/web` half of the runtime the row pinned only half of | 53 |
| Rows with a completed peer install | 34 |
| Peer packages installed | 49 |
| Rows whose peer install failed or moved a pin | 0 |

A package that **imports something it declares nowhere** — not a dependency, not a peer — is outside what any install policy can supply, and is reported above as an import throw rather than fixed here. Completing an undeclared import would mean this harness choosing a version the package never named.

## Probe failures: claims the package answered differently

A **failure** is the strongest thing this measurement produces. The contract states a claim, the probe drove it, and the package did something else — a generator bug or a package change, never an environment gap and never an unreachable claim. Verification refuses the whole contract on one of these, deliberately: converting a contradicted claim to the unknown sentinel would hide it.

0 failing claim(s) across the corpus, by shape:

| Claim, claimed, observed | Claims |
| --- | --- |

## Conversion volume

A conversion replaces one export's whole claim domain with the `{"status":"unknown"}` sentinel because the probe neither observed nor statically proved it.

| Figure | Count |
| --- | --- |
| Claim domains converted to unknown | 833 |
| Exports carrying an unknown in the verified rows, at generation | 1653/4393 (37.63%) |
| Exports carrying an unknown in the verified rows, after verification | 1991/3740 (53.24%) |

How much a verified contract actually certifies from observation:

| Figure | Count |
| --- | --- |
| Verified rows carrying at least one probed behavioral row | 21/308 (6.82%) |
| Probed behavioral row markers kept across the whole corpus | 99 |
| Inferred row markers dropped by verification | 4610 |
| Probed markers discarded as unwitnessed by this run's report | 174 |
| Entrypoints verification refused inside a promoted document | 38 |
| Verified rows carrying at least one such refusal | 10 |

The last two rows are a **cost made visible, not a regression**. An entrypoint whose `kind` claims this run did not observe is refused and omitted, exactly as `contract generate` already refuses an entrypoint it cannot certify, so the package's other entrypoints are not sunk by one unimportable subpath. A refused entrypoint is absent from the contract, which is an explicit uncertifiable result at the consumer rather than a wrong claim; a document where *no* entrypoint would certify anything is still refused whole. The exports it dropped are their own state in the composite below, still inside its denominator: a certified *share* that rose because unobservable exports left the population would be measuring nothing.

Converted domains by field:

| Field | Conversions |
| --- | --- |
| `callbacks` | 410 |
| `returns` | 392 |
| `asyncBehavior` | 31 |

## The composite a consumer feels

Of every export the corpus's generated contracts describe:

| State | Exports |
| --- | --- |
| (a) certified by a verified contract | 1749/7117 (24.57%) |
| (b) honest unknown inside a verified contract | 1991/7117 (27.98%) |
| (c) dropped from a verified contract with its refused entrypoint | 653/7117 (9.18%) |
| (d) inside a contract that never reached `verified` | 2724/7117 (38.27%) |

(c) is the cost of amendment A9 stage 1 stated as a consumer-facing number: the row verified, and these exports are absent from the document it promoted, so importing one is an explicit uncertifiable result. They stay in the denominator — a certified *share* that rose because unobservable exports left the population would be measuring nothing. (d) is every export of a contract that was generated and then refused, timed out, or errored before a probe report existed. Rows whose `bun install` or `contract generate` failed describe no exports at all and are in none of the four states.

## Wall time

| Phase | Rows | Median | p90 | Max | Mean |
| --- | --- | --- | --- | --- | --- |
| install | 416 | 13 ms | 542 ms | 4511 ms | 125 ms |
| generate | 413 | 76 ms | 284 ms | 6422 ms | 210 ms |
| probe | 396 | 110 ms | 494 ms | 24107 ms | 427 ms |
| verify | 396 | 22 ms | 25 ms | 42 ms | 22 ms |
| pipelineWithoutInstall | 413 | 221 ms | 897 ms | 27947 ms | 641 ms |
| total | 416 | 255 ms | 1340 ms | 27976 ms | 866 ms |

`install` may run against Bun's warm package cache, so it is a lower bound; `pipelineWithoutInstall` is the number that describes the checker's own cost.

## Rows that never reached verification

| Stage | Rows |
| --- | --- |
| `bun install` failed | 3 |
| `contract generate` failed | 16 |
| `contract probe` errored before writing a report | 0 |
| no Solid runtime the row could honestly be probed against | 1 |
| timed out under the harness budget | 0 |

The manifest pins the runtime each row runs against, and for these it pins no `solid-js` — `@solidjs/signals` *is* the reactive core, so there is no second package to settle a probe with. Pairing one in would be this harness auditing a combination the corpus deliberately did not. They are their own class rather than an error:

- `@solidjs/signals@2.0.0-rc.1|solid2|head`

Generation failures by class:

| Class | Rows |
| --- | --- |
| `unclassified` | 12 |
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
| `@kobalte/core@2.0.0-alpha.0|solid2|only` | kobalte | `incompleteness` | 24 | incompleteness, probe-report-includes-evidence-write | entrypoint import threw x333 |
| `@kobalte/solidbase@0.6.13|solid1|only` | kobalte | `attested-closure-note` | 1 | attested-closure-note | entrypoint import threw x44 |
| `@solid-devtools/extension-adapter@0.12.1|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-devtools/frontend@0.15.4|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x3 |
| `@solid-devtools/logger@0.9.11|solid1|only` | solid-devtools | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@solid-primitives/controlled-props@0.1.4|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x6 |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x28 |
| `@solid-primitives/controlled-props@1.0.0-next.3|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x28 |
| `@solid-primitives/countdown@1.0.9|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/cursor@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@2.1.8|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/date@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x52 |
| `@solid-primitives/drag-drop@0.1.0-next.0|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x52 |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 13 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/event-listener@3.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 13 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x44 |
| `@solid-primitives/favicon@1.0.0-next.1|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x44 |
| `@solid-primitives/fetch@2.5.2|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/graphql@3.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/history@0.2.5|solid1|only` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/history@1.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/history@1.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/immutable@2.0.0-next.0|solid1|only` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|floor` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/keyed@3.0.0-next.2|solid2|head` | solid-primitives | `kind-observed` | 2 | kind-observed | entrypoint import threw x24 |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/mediastream@1.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/refs@1.1.4|solid1|only` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/refs@3.0.0-next.2|solid2|floor` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/refs@3.0.0-next.2|solid2|head` | solid-primitives | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|floor` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `@solid-primitives/resize-observer@4.0.0-next.3|solid2|head` | solid-primitives | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
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
| `@solidjs/start@2.0.3|solid1|only` | official-solid | `closure-note` | 11 | attested-closure-note, closure-note | entrypoint import threw x312 |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|floor` | official-solid | `attested-closure-note` | 1 | attested-closure-note | — |
| `@solidjs/vite-plugin@3.0.0-next.31|solid2|head` | official-solid | `attested-closure-note` | 1 | attested-closure-note | — |
| `@tanstack/ai-solid-ui@0.7.18|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x36 |
| `@tanstack/charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x1 |
| `@tanstack/solid-charts@0.14.0|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x1 |
| `@tanstack/solid-form@2.0.0-alpha.2|solid1|only` | tanstack | `incompleteness` | 7 | incompleteness, probe-report-includes-evidence-write | — |
| `@tanstack/solid-pacer-devtools@0.14.0|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed | entrypoint import threw x2 |
| `@tanstack/solid-router-devtools@1.167.1|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-router-devtools@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x8 |
| `@tanstack/solid-router-ssr-query@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-router-ssr-query@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-router@1.170.29|solid1|only` | tanstack | `kind-observed` | 3 | kind-observed | entrypoint import threw x23 |
| `@tanstack/solid-router@2.0.0-rc.1|solid2|only` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x120 |
| `@tanstack/solid-start-client@1.168.28|solid1|only` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x10 |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x20 |
| `@tanstack/solid-start-client@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 4 | kind-observed | entrypoint import threw x10 |
| `@tanstack/solid-start-config@1.120.20|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x4 |
| `@tanstack/solid-start-server@1.167.35|solid1|only` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x39 |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|floor` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x78 |
| `@tanstack/solid-start-server@2.0.0-rc.1|solid2|head` | tanstack | `kind-observed` | 2 | kind-observed | entrypoint import threw x39 |
| `@tanstack/solid-store@0.11.1|solid1|only` | tanstack | `incompleteness` | 4 | incompleteness, probe-report-includes-evidence-write | — |
| `solid-js@2.0.0-rc.1|solid2|head` | official-solid | `incompleteness` | 16 | incompleteness, probe-report-includes-evidence-write | no unambiguous summary resolves in the mode (no kind claim exists) x1 |
| `solid-recharts@1.0.1|solid1|only` | solid-recharts | `kind-observed` | 2 | kind-observed | no unambiguous summary resolves in the mode (no kind claim exists) x3 |
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
| `@solid-devtools/shared@0.20.0|solid1|only` | 106 | 26 | 20 | 0 | — |
| `@solid-devtools/transform@0.10.4|solid1|only` | 2 | 0 | 0 | 0 | — |
| `@solid-devtools/ui@0.10.3|solid1|only` | 13 | 1 | 0 | 0 | `.`, `./icons` |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|floor` | 7 | 3 | 2 | 0 | — |
| `@solid-primitives/a11y@1.0.0-next.3|solid2|head` | 7 | 3 | 2 | 0 | — |
| `@solid-primitives/active-element@2.1.6|solid1|only` | 5 | 4 | 1 | 0 | — |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|floor` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/active-element@3.0.0-next.2|solid2|head` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|floor` | 10 | 6 | 2 | 0 | — |
| `@solid-primitives/analytics@2.0.0-next.2|solid2|head` | 10 | 6 | 2 | 0 | — |
| `@solid-primitives/async@0.0.101-next.3|solid2|floor` | 6 | 4 | 5 | 0 | — |
| `@solid-primitives/async@0.0.101-next.3|solid2|head` | 6 | 4 | 5 | 0 | — |
| `@solid-primitives/audio@3.0.0-next.2|solid2|floor` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/audio@3.0.0-next.2|solid2|head` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/autofocus@0.1.5|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/bounds@0.1.7|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|floor` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/bounds@1.0.0-next.2|solid2|head` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/broadcast-channel@0.1.1|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|floor` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/broadcast-channel@1.0.0-next.2|solid2|head` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/clipboard@1.6.6|solid1|only` | 9 | 3 | 3 | 0 | — |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|floor` | 9 | 2 | 3 | 0 | — |
| `@solid-primitives/clipboard@2.0.0-next.17|solid2|head` | 9 | 2 | 3 | 0 | — |
| `@solid-primitives/connectivity@0.4.6|solid1|only` | 3 | 3 | 0 | 0 | — |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|floor` | 6 | 4 | 1 | 0 | — |
| `@solid-primitives/connectivity@1.0.0-next.2|solid2|head` | 6 | 4 | 1 | 0 | — |
| `@solid-primitives/context@0.3.2|solid1|only` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/context@2.0.0-next.2|solid2|floor` | 4 | 3 | 0 | 0 | — |
| `@solid-primitives/context@2.0.0-next.2|solid2|head` | 4 | 3 | 0 | 0 | — |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|floor` | 5 | 5 | 5 | 0 | — |
| `@solid-primitives/controlled-signal@1.0.0-next.3|solid2|head` | 5 | 5 | 5 | 0 | — |
| `@solid-primitives/cookies@0.0.3|solid1|only` | 4 | 3 | 0 | 0 | — |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/cookies@1.0.0-next.2|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/cursor@0.1.4|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/date-difference@1.0.2|solid1|only` | 9 | 2 | 0 | 0 | — |
| `@solid-primitives/db-store@1.1.4|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/debounce@1.3.0|solid1|only` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/deep@0.3.7|solid1|only` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/deep@1.0.0-next.3|solid2|floor` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/deep@1.0.0-next.3|solid2|head` | 4 | 1 | 0 | 3 | — |
| `@solid-primitives/destructure@0.2.4|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|floor` | 1 | 1 | 0 | 0 | — |
| `@solid-primitives/destructure@1.0.0-next.2|solid2|head` | 1 | 1 | 0 | 0 | — |
| `@solid-primitives/devices@1.3.1|solid1|only` | 6 | 6 | 6 | 0 | — |
| `@solid-primitives/devices@3.0.0-next.2|solid2|floor` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/devices@3.0.0-next.2|solid2|head` | 4 | 4 | 4 | 0 | — |
| `@solid-primitives/event-bus@1.1.4|solid1|only` | 11 | 8 | 4 | 0 | — |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|floor` | 11 | 7 | 4 | 1 | — |
| `@solid-primitives/event-bus@3.0.0-next.3|solid2|head` | 11 | 7 | 4 | 1 | — |
| `@solid-primitives/event-dispatcher@0.1.1|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-dispatcher@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-listener@2.4.6|solid1|only` | 11 | 11 | 3 | 0 | — |
| `@solid-primitives/event-props@0.3.1|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|floor` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/event-props@1.0.0-next.2|solid2|head` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/filesystem@1.3.4|solid1|only` | 15 | 10 | 4 | 0 | — |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|floor` | 15 | 7 | 4 | 0 | — |
| `@solid-primitives/filesystem@3.0.0-next.3|solid2|head` | 15 | 7 | 4 | 0 | — |
| `@solid-primitives/flux-store@0.1.1|solid1|only` | 4 | 3 | 2 | 0 | — |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|floor` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/flux-store@1.0.0-next.2|solid2|head` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/focus@1.0.0-next.4|solid2|floor` | 8 | 2 | 1 | 0 | — |
| `@solid-primitives/focus@1.0.0-next.4|solid2|head` | 8 | 2 | 1 | 0 | — |
| `@solid-primitives/form@1.0.0-next.2|solid2|floor` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/form@1.0.0-next.2|solid2|head` | 7 | 6 | 2 | 0 | — |
| `@solid-primitives/fullscreen@1.3.5|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|floor` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/fullscreen@2.0.0-next.3|solid2|head` | 3 | 1 | 2 | 0 | — |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|floor` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/geolocation@3.0.0-next.2|solid2|head` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/gestures@1.2.1|solid1|only` | 9 | 7 | 1 | 0 | — |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|floor` | 11 | 1 | 1 | 0 | — |
| `@solid-primitives/gestures@3.0.0-next.3|solid2|head` | 11 | 1 | 1 | 0 | — |
| `@solid-primitives/i18n@2.2.1|solid1|only` | 9 | 4 | 2 | 3 | — |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|floor` | 12 | 3 | 2 | 4 | — |
| `@solid-primitives/i18n@3.0.0-next.4|solid2|head` | 12 | 3 | 2 | 4 | — |
| `@solid-primitives/idle@0.2.3|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/idle@1.0.0-next.3|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/idle@1.0.0-next.3|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/input-mask@0.3.1|solid1|only` | 7 | 2 | 1 | 0 | — |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|floor` | 7 | 2 | 2 | 0 | — |
| `@solid-primitives/input-mask@1.0.0-next.2|solid2|head` | 7 | 2 | 2 | 0 | — |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|floor` | 5 | 1 | 1 | 0 | — |
| `@solid-primitives/interaction@1.0.0-next.4|solid2|head` | 5 | 1 | 1 | 0 | — |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|floor` | 12 | 4 | 4 | 0 | — |
| `@solid-primitives/intersection-observer@3.0.0-next.3|solid2|head` | 12 | 4 | 4 | 0 | — |
| `@solid-primitives/jsx-parser@0.2.0|solid1|only` | 4 | 2 | 3 | 0 | — |
| `@solid-primitives/jsx-tokenizer@1.1.4|solid1|only` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/jsx-tokenizer@3.0.0-next.2|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/keyboard@1.3.7|solid1|only` | 6 | 6 | 1 | 0 | — |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|floor` | 7 | 7 | 1 | 0 | — |
| `@solid-primitives/keyboard@2.0.0-next.5|solid2|head` | 7 | 7 | 1 | 0 | — |
| `@solid-primitives/keyed@1.5.3|solid1|only` | 6 | 6 | 4 | 0 | — |
| `@solid-primitives/lifecycle@0.1.2|solid1|only` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|floor` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/lifecycle@1.0.0-next.2|solid2|head` | 3 | 2 | 1 | 0 | — |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|floor` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list-state@1.0.0-next.2|solid2|head` | 2 | 2 | 2 | 0 | — |
| `@solid-primitives/list@0.1.2|solid1|only` | 2 | 2 | 1 | 0 | — |
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
| `@solid-primitives/memo@1.5.1|solid1|only` | 12 | 12 | 10 | 0 | — |
| `@solid-primitives/memo@2.0.0-next.2|solid2|floor` | 7 | 7 | 5 | 0 | — |
| `@solid-primitives/memo@2.0.0-next.2|solid2|head` | 7 | 7 | 5 | 0 | — |
| `@solid-primitives/mouse@2.1.7|solid1|only` | 8 | 8 | 1 | 0 | — |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|floor` | 8 | 6 | 1 | 0 | — |
| `@solid-primitives/mouse@4.0.0-next.3|solid2|head` | 8 | 6 | 1 | 0 | — |
| `@solid-primitives/mutable@1.1.1|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/mutable@3.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/mutation-observer@1.2.4|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|floor` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/mutation-observer@3.0.0-next.2|solid2|head` | 2 | 2 | 0 | 0 | — |
| `@solid-primitives/notification@1.0.0-next.3|solid2|floor` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/notification@1.0.0-next.3|solid2|head` | 4 | 2 | 2 | 0 | — |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|floor` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/orientation@1.0.0-next.2|solid2|head` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|floor` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/page-utilities@3.0.0-next.2|solid2|head` | 4 | 2 | 1 | 0 | — |
| `@solid-primitives/page-visibility@2.1.6|solid1|only` | 2 | 1 | 0 | 0 | — |
| `@solid-primitives/pagination@0.5.2|solid1|only` | 4 | 3 | 4 | 0 | — |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|floor` | 4 | 3 | 4 | 0 | — |
| `@solid-primitives/pagination@1.0.0-next.6|solid2|head` | 4 | 3 | 4 | 0 | — |
| `@solid-primitives/permission@1.3.2|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/permission@2.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/permission@2.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/pointer@0.3.6|solid1|only` | 7 | 7 | 0 | 0 | — |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|floor` | 7 | 4 | 1 | 0 | — |
| `@solid-primitives/pointer@1.0.0-next.2|solid2|head` | 7 | 4 | 1 | 0 | — |
| `@solid-primitives/presence@0.1.4|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/presence@1.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/presence@1.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/promise@1.1.4|solid1|only` | 4 | 3 | 2 | 0 | — |
| `@solid-primitives/promise@2.0.0-next.2|solid2|floor` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/promise@2.0.0-next.2|solid2|head` | 7 | 3 | 4 | 0 | — |
| `@solid-primitives/props@3.2.4|solid1|only` | 6 | 3 | 2 | 0 | — |
| `@solid-primitives/props@4.0.0-next.3|solid2|floor` | 8 | 4 | 2 | 0 | — |
| `@solid-primitives/props@4.0.0-next.3|solid2|head` | 8 | 4 | 2 | 0 | — |
| `@solid-primitives/queue@1.0.0-next.3|solid2|floor` | 6 | 5 | 6 | 0 | — |
| `@solid-primitives/queue@1.0.0-next.3|solid2|head` | 6 | 5 | 6 | 0 | — |
| `@solid-primitives/raf@2.3.5|solid1|only` | 4 | 4 | 3 | 0 | — |
| `@solid-primitives/raf@4.0.0-next.2|solid2|floor` | 4 | 4 | 3 | 0 | — |
| `@solid-primitives/raf@4.0.0-next.2|solid2|head` | 4 | 4 | 3 | 0 | — |
| `@solid-primitives/range@0.2.5|solid1|only` | 6 | 6 | 3 | 0 | — |
| `@solid-primitives/range@1.0.0-next.3|solid2|floor` | 7 | 6 | 5 | 0 | — |
| `@solid-primitives/range@1.0.0-next.3|solid2|head` | 7 | 6 | 5 | 0 | — |
| `@solid-primitives/reducer@0.0.101|solid1|only` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/resize-observer@2.2.0|solid1|only` | 7 | 5 | 2 | 0 | — |
| `@solid-primitives/resource@0.4.3|solid1|only` | 8 | 7 | 2 | 0 | — |
| `@solid-primitives/rootless@1.5.4|solid1|only` | 8 | 8 | 5 | 0 | — |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|floor` | 8 | 8 | 5 | 0 | — |
| `@solid-primitives/rootless@2.0.0-next.2|solid2|head` | 8 | 8 | 5 | 0 | — |
| `@solid-primitives/scheduled@1.5.3|solid1|only` | 6 | 6 | 5 | 0 | — |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|floor` | 6 | 6 | 5 | 0 | — |
| `@solid-primitives/scheduled@2.0.0-next.2|solid2|head` | 6 | 6 | 5 | 0 | — |
| `@solid-primitives/script-loader@2.3.2|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|floor` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/script-loader@3.0.0-next.2|solid2|head` | 1 | 1 | 1 | 0 | — |
| `@solid-primitives/scroll@2.1.6|solid1|only` | 5 | 2 | 0 | 0 | — |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|floor` | 6 | 2 | 0 | 0 | — |
| `@solid-primitives/scroll@3.0.0-next.4|solid2|head` | 6 | 2 | 0 | 0 | — |
| `@solid-primitives/selection@0.1.3|solid1|only` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/selection@1.0.0-next.2|solid2|floor` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/selection@1.0.0-next.2|solid2|head` | 2 | 1 | 1 | 0 | — |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|floor` | 10 | 10 | 6 | 0 | — |
| `@solid-primitives/sensors@1.0.0-next.3|solid2|head` | 10 | 10 | 6 | 0 | — |
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
| `@solid-primitives/storage@5.0.0-next.4|solid2|floor` | 11 | 4 | 1 | 0 | — |
| `@solid-primitives/storage@5.0.0-next.4|solid2|head` | 11 | 4 | 1 | 0 | — |
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
| `@solid-primitives/utils@6.4.1|solid1|only` | 75 | 51 | 13 | 0 | — |
| `@solid-primitives/utils@7.0.0-next.4|solid2|floor` | 99 | 29 | 16 | 5 | — |
| `@solid-primitives/utils@7.0.0-next.4|solid2|head` | 99 | 29 | 16 | 5 | — |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|floor` | 6 | 2 | 4 | 0 | — |
| `@solid-primitives/vibrate@1.0.0-next.2|solid2|head` | 6 | 2 | 4 | 0 | — |
| `@solid-primitives/video@1.0.0-next.3|solid2|floor` | 7 | 4 | 4 | 0 | — |
| `@solid-primitives/video@1.0.0-next.3|solid2|head` | 7 | 4 | 4 | 0 | — |
| `@solid-primitives/visibility-observer@2.0.1|solid1|only` | 2 | 2 | 1 | 0 | — |
| `@solid-primitives/websocket@1.4.0|solid1|only` | 6 | 2 | 2 | 0 | — |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|floor` | 10 | 5 | 5 | 0 | — |
| `@solid-primitives/websocket@2.0.0-next.3|solid2|head` | 10 | 5 | 5 | 0 | — |
| `@solid-primitives/workers@0.4.3|solid1|only` | 3 | 3 | 0 | 0 | — |
| `@solid-primitives/workers@2.0.1-next.1|solid2|floor` | 5 | 3 | 3 | 0 | — |
| `@solid-primitives/workers@2.0.1-next.1|solid2|head` | 5 | 3 | 3 | 0 | — |
| `@solidjs/element@2.0.0-rc.1|solid2|only` | 5 | 5 | 1 | 0 | — |
| `@solidjs/h@2.0.0-rc.1|solid2|only` | 9 | 1 | 0 | 0 | — |
| `@solidjs/image@0.1.0|solid1|only` | 1 | 0 | 0 | 0 | `.` |
| `@solidjs/meta@0.29.4|solid1|only` | 9 | 7 | 2 | 0 | — |
| `@solidjs/meta@1.0.0-next.2|solid2|floor` | 8 | 7 | 0 | 0 | — |
| `@solidjs/meta@1.0.0-next.2|solid2|head` | 8 | 7 | 0 | 0 | — |
| `@solidjs/router@2.0.0-next.17|solid2|only` | 30 | 28 | 21 | 3 | — |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|floor` | 3 | 3 | 0 | 0 | — |
| `@solidjs/start-devtools@1.0.0-next.3|solid2|head` | 3 | 3 | 0 | 0 | — |
| `@solidjs/testing-library@0.8.10|solid1|only` | 83 | 63 | 11 | 0 | — |
| `@solidjs/universal@2.0.0-rc.1|solid2|only` | 1 | 0 | 0 | 0 | — |
| `@solidjs/web@2.0.0-rc.1|solid2|head` | 321 | 67 | 49 | 7 | `.`, `./frames`, `./server-functions` |
| `@tanstack/ai-devtools-core@0.5.6|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/ai-solid@0.18.3|solid1|only` | 21 | 21 | 9 | 0 | — |
| `@tanstack/devtools-a11y@0.2.2|solid1|only` | 6 | 6 | 0 | 0 | — |
| `@tanstack/devtools-ui@0.7.1|solid1|only` | 8 | 0 | 0 | 0 | `.`, `./icons` |
| `@tanstack/devtools-utils@0.7.0|solid1|only` | 5 | 2 | 0 | 0 | — |
| `@tanstack/devtools@0.14.2|solid1|only` | 3 | 1 | 0 | 0 | — |
| `@tanstack/form-devtools@1.0.0-alpha.2|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/hotkeys-devtools@0.9.0|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@tanstack/pacer-devtools@1.4.0|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-ai-devtools@0.2.70|solid1|only` | 4 | 4 | 0 | 0 | — |
| `@tanstack/solid-db@0.2.37|solid1|only` | 207 | 123 | 10 | 0 | — |
| `@tanstack/solid-devtools@0.8.12|solid1|only` | 1 | 1 | 0 | 0 | — |
| `@tanstack/solid-form-devtools@1.0.0-alpha.2|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@tanstack/solid-hotkeys@0.10.0|solid1|only` | 64 | 13 | 5 | 0 | — |
| `@tanstack/solid-pacer@0.22.0|solid1|only` | 108 | 36 | 21 | 19 | — |
| `@tanstack/solid-query-devtools@5.101.4|solid1|only` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|floor` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-query-devtools@6.0.0-rc.0|solid2|head` | 2 | 2 | 0 | 0 | — |
| `@tanstack/solid-query-persist-client@5.101.4|solid1|only` | 8 | 4 | 2 | 0 | — |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor` | 8 | 4 | 2 | 0 | — |
| `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head` | 8 | 4 | 2 | 0 | — |
| `@tanstack/solid-query@5.101.4|solid1|only` | 57 | 40 | 24 | 6 | — |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|floor` | 57 | 47 | 28 | 6 | — |
| `@tanstack/solid-query@6.0.0-rc.0|solid2|head` | 57 | 47 | 28 | 6 | — |
| `@tanstack/solid-router-ssr-query@1.167.2-pre.0|solid1|only` | 1 | 0 | 0 | 0 | — |
| `@tanstack/solid-start@1.168.46|solid1|only` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|floor` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-start@2.0.0-rc.1|solid2|head` | 3 | 3 | 0 | 0 | `.`, `./client`, `./hydration`, `./plugin/rsbuild`, `./plugin/vite`, `./server`, `./server-entry` |
| `@tanstack/solid-table-devtools@9.2.0|solid1|only` | 3 | 1 | 0 | 0 | — |
| `@tanstack/solid-table@9.1.2|solid1|only` | 408 | 57 | 7 | 3 | — |
| `@tanstack/solid-virtual@3.13.37|solid1|only` | 17 | 8 | 3 | 1 | — |
| `@tanstack/table-devtools@9.2.0|solid1|only` | 10 | 10 | 1 | 0 | — |
| `corvu@0.7.2|solid1|only` | 43 | 26 | 25 | 0 | `./otp-field`, `./popover`, `./resizable`, `./tooltip` |
| `motion-solidjs@0.7.0-beta.4|solid2|floor` | 165 | 165 | 0 | 0 | — |
| `motion-solidjs@0.7.0-beta.4|solid2|head` | 165 | 164 | 0 | 0 | — |
| `solid-devtools@0.34.5|solid1|only` | 5 | 3 | 0 | 0 | — |
| `solid-js@1.9.14|solid1|only` | 120 | 72 | 34 | 14 | `.`, `./store` |
