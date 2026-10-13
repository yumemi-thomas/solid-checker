# What the rc.9 campaign changes for real Solid 2 consumers

Measured 2026-09-27. The checker ran over three consumer trees, once under
each of two builds:

- **before**: `1a96cdff`. rc.3 is the audited Solid 2 triple. The tier has 206
  bundles, and its kobalte environment delivers 21 packages.
- **after**: `3b0d99f4`. rc.9 is audited (ADR 0127), with the rc.9 negative
  rows and ADRs 0128 and 0129. The tier was regenerated with the three consumer
  environments: 228 bundles, and 16 (+5 with no web) bundles on the rc.9
  triple.

Every number below is **measured** unless it is marked *estimated*. The
scratch root is
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/1aa3e519-86c0-43fe-acd0-487816ac2c51/scratchpad/rc9sweep/`,
written `$SW` below. Nothing there is checked in.

## Headline

- **Two accepted contracts reach a consumer that they did not reach before:**
  `@solid-primitives/form` in kobalte, and `@solidjs/meta` in
  oscartbeaumont-website. Together they add **37 import bindings resolved
  through an accepted contract**: kobalte goes from 270 to 300, and oscar from
  0 to 7. **Both come from tier environments.**
- **viviana-ui gets nothing from the tier.** Its environment delivered
  `@tanstack/solid-router` only as `@tanstack/solid-router/ssr/client`, and
  `apps/web` imports `@tanstack/solid-router` at 213 sites. Its 562
  acceptance-gate sites do not move.
- **`SC9014` moved on every consumer, in both directions.** kobalte (rc.3) gains
  it in all 7 projects, and its two Solid-free projects lose `certified`
  because of it. oscar (rc.9) loses it in all 3 projects. viviana (rc.9) goes
  from 2 gaps in 11 projects to 1 gap, the `sharedConfig` re-export, in 9
  projects.
- **No violation moved anywhere** (424, 3,816 and 18). No finding of any other
  id appeared. One id disappeared: kobalte's 30 `SC9011` findings on
  `createFormResetListener`, which are now described by form's contract.
- The new negative rows, the withdrawn rc.3 rows, and ADRs 0128 and 0129 move
  **no consumer finding directly**. With the tier off, the two builds differ
  only in `SC9014` (see Controls).

## The corpus

The clones are in `$SW/consumers/`. Each was fetched at the commit its
`scripts/ecosystem-benchmark/consumer-environments.json` entry records, and
each lockfile's sha256 equals the entry's `lockfileDigest`. They were installed
with `CI=true pnpm install --frozen-lockfile --ignore-scripts`, using the
project's own pnpm. Scripts: `$SW/install.sh`, logs `$SW/install-*.log`.

| environment id | repository @ commit | pnpm | install wall |
| --- | --- | --- | ---: |
| `kobalte-solid2-e9d426d4` | `kobaltedev/kobalte` @ `e9d426d4` | 11.20.0 (`packageManager`) | 4 s |
| `viviana-ui-main-b005c00a` | `proyecto-viviana/ui` @ `b005c00a` | 11.22.0 (`packageManager`) | 24 s |
| `oscartbeaumont-website-main-60823453` | `oscartbeaumont/website` @ `60823453` | 10.33.2 (none declared; lockfile v9.0) | 15 s |

`$SW/resolved.cjs` resolved the Solid runtime from every importer the entries
name (kobalte `packages/core` and `packages/utils`, viviana `apps/web`, oscar
`.`). It resolved `@solidjs/signals` from the resolved `solid-js`. It compared
each version with the entry, and each lockfile integrity with the entry's
integrity.

| consumer | `solid-js` | `@solidjs/signals` | `@solidjs/web` | delivered packages reachable from the importer |
| --- | --- | --- | --- | --- |
| kobalte | 2.0.0-rc.3 ✓ | 2.0.0-rc.3 ✓ | 2.0.0-rc.3 ✓ | every reachable one matches version and integrity |
| viviana-ui | 2.0.0-rc.9 ✓ | 2.0.0-rc.9 ✓ | 2.0.0-rc.9 ✓ | `@tanstack/solid-router` 2.0.0-rc.8 ✓ (start-client and start-server are transitive) |
| oscartbeaumont-website | 2.0.0-rc.9 ✓ | 2.0.0-rc.9 ✓ | 2.0.0-rc.9 ✓ | `@solidjs/meta` 1.0.0-next.2 ✓ |

### Projects

Projects were found with the demand script's `projects_under`: every
`tsconfig.json` at depth 4 or less, excluding `node_modules`, `template`,
`tests` and `.git`. That gives 7 kobalte and 11 viviana projects. oscar's
`tsconfig.json` is a solution file with `"files": []`, so its two referenced
configs, `tsconfig.app.json` and `tsconfig.node.json`, were added. That makes
21 projects. Driver: `$SW/drive.py`.

## Method

Both builds were compiled with `make build-checker-release` in their own
worktrees, each with its own `rust/target`, seeded by APFS clone:

| build | commit | `solid-checker-rust` sha256 | build wall |
| --- | --- | --- | ---: |
| before | `1a96cdff` | `9e9a5fe1…` | 38 s |
| after | `3b0d99f4` | `f4606d86…` | 39 s |

Both used the same `bin/solid-typefacts`, sha256 `fc93317f…`. Its stamp matched
both trees, and `build-typefacts` did not rebuild it.

Each project was run with
`SOLID_CHECKER_DAEMON=0 solid-checker-rust --format json --project <tsconfig>`.
These are one-shot analyses, so no retained daemon answer crosses builds. The
runs were sequential: the whole before sweep, then the whole after sweep, then
the same two again with `--no-bundled-contracts`. The machine was on AC power,
`powermode 0`, with load about 1.3 at the start. Outputs:
`$SW/runs/{before,after,before-notier,after-notier}/`.

- **Counting.** `SC9005` sites use the checked-in demand script
  (`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.py`,
  at `3b0d99f4`, the same for both builds). Its `site_count` and
  `import_sites` count a collapsed finding's primary and related locations as
  sites. Gates are named by `analysisContext`, as in the 2026-09-26 report.
  Script: `$SW/summarize.py`, output `$SW/summary.txt`.
- **"No accepted contract" sites** are the sites at the acceptance gate
  (`no receipt-accepted contract matches this exact import`).
- **Sites resolved through an accepted contract** are the acceptance-gate
  import bindings that the same build raises with the tier off, minus those it
  still raises with the tier on. Every binding that left the gate did so for a
  package the tier-on run reports `accepted` in `packageSummaries`. Script:
  `$SW/resolved_sites.py`, output `$SW/resolved-sites.txt`.
- **Moved findings** are compared by `(project, id, analysisContext, message,
  primary path, primary start, related count)`. Output:
  `$SW/moved-before-after.json`.

## Results

### Per consumer

| | kobalte before | kobalte after | viviana before | viviana after | oscar before | oscar after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| projects: violation / uncertifiable / certified | 4 / 1 / 2 | 4 / **3** / **0** | 8 / 3 / 0 | 8 / 3 / 0 | 1 / 2 / 0 | 1 / **1** / **1** |
| violation findings | 424 | 424 | 3,816 | 3,816 | 18 | 18 |
| uncertifiable findings | 1,031 | **1,010** | 8,776 | **8,774** | 13 | **12** |
| `SC9005` findings | 254 | 256 | 3,255 | 3,255 | 5 | 7 |
| `SC9005` sites, total | 787 | 847 | 3,808 | 3,808 | 14 | 14 |
| — acceptance gate ("no accepted contract") | **311** | **281** | 562 | 562 | **13** | **6** |
| — open claims | 400 | 490 | 0 | 0 | 0 | 7 |
| — callback execution (not an import site) | 76 | 76 | 3,246 | 3,246 | 1 | 1 |
| import bindings resolved through an accepted contract | **270** | **300** | 0 | 0 | **0** | **7** |
| distinct accepted package contracts | 13 | **14** | 0 | 0 | 0 | **1** |
| projects with `SC9014` | 0 | **7** | 11 | **9** | 3 | **0** |
| `SC9014` gaps listed | — | 1 | 2 | **1** | 2 | — |
| wall, sum over projects | 30.6 s | 30.5 s | 125.3 s | 124.4 s | 1.1 s | 1.1 s |

kobalte's totals cover all 7 projects. The root `tsconfig.json` overlaps
`packages/core`, so every core move shows up twice.

**The importers the environments name.** kobalte `packages/core`: accepted
contracts 13 → 14, acceptance-gate sites 123 → 108, `SC9011` 19 → 4, status
`violation` in both. kobalte `packages/utils`: no `SC9005` in either build. It
is `uncertifiable` in both, from one `SC9012`, and after also from `SC9014`.
viviana `apps/web`: identical except for `SC9014`, which it loses. The status
is `violation` in both. oscar `tsconfig.app.json`: acceptance-gate sites
12 → 5 and one accepted contract. The status is `violation` in both, from 14
`SC2003`.

### `SC9014` and its gap lists

| consumer | before (audited rc.3) | after (audited rc.9) |
| --- | --- | --- |
| kobalte (rc.3) | absent | all 7 projects, 1 gap: *"… are older than the audited release, 2.0.0-rc.9: the answers their review gave still apply, but new rules and precision work are measured on the audited release only"* |
| viviana-ui (rc.9) | all 11 projects, 2 gaps: the `solid-js@2.0.0-rc.9` re-export of five undeclared names, and *"`@solidjs/signals@2.0.0-rc.9` has negative rows granted only for the creates domain of getOwner, onCleanup, untrack, runWithOwner and createRoot"* | 9 projects, 1 gap: the re-export gap, *"…; this project uses sharedConfig from solid-js"*. `apps/web` and `packages/solid-stately` lose the notice. |
| oscartbeaumont-website (rc.9) | all 3 configs, the same 2 gaps | absent |

## What moved, and why

Every moved finding is listed. Nothing else differs between the two tier-on
sweeps.

| where | before → after | cause |
| --- | --- | --- |
| kobalte, 7 projects | `SC9014` absent → present | **rc.9 audited notice** (ADR 0127 § 3: rc.0–rc.8 get the notice) |
| kobalte `packages/tailwindcss`, `packages/vanilla-extract` | `certified` → `uncertifiable` | the same notice, and nothing else. Neither project imports Solid; see Spot checks |
| kobalte `packages/core` and root | 1 acceptance-gate `SC9005` for `@solid-primitives/form` (15 import sites) + 15 `SC9011` on `createFormResetListener` → 2 open-claims `SC9005`: `reactiveReads,ownerRequirements` (15 import sites) and `callbacks` (30 call-argument sites) | **tier environments**: `kobalte-solid2-e9d426d4` was re-derived in `2a7015d0` and now delivers form 1.0.0-next.3 and event-listener 3.0.0-next.5. The tier holds a form bundle on (rc.3, rc.3, rc.3), and it binds |
| oscar, 3 configs | `SC9014` present → absent | **rc.9 audited notice**. The project reaches none of the five re-exported names, so ADR 0127 § 2 scopes the gap away |
| oscar `tsconfig.json` (`files: []`) | `uncertifiable` → `certified` | the same. It carried only the notice |
| oscar `tsconfig.app.json` | 1 acceptance-gate `SC9005` for `@solidjs/meta` (7 import sites) → 3 open-claims `SC9005` (`reactiveReads,returns,ownerRequirements`) for `Link` (1), `Meta` (3), `Title` (3) | **tier environments**: `oscartbeaumont-website-main-60823453` delivers meta 1.0.0-next.2 on the rc.9 triple, a bundle the before tier did not have (it had rc.0 and rc.3/rc.6 only) |
| viviana, 9 projects | `SC9014` message: 2 gaps → 1 | **new negative rows** (signals rc.9 carries rc.6's 24 rows, so its gap closes) and the **import-scoped re-export gap** (ADR 0127 § 2) |
| viviana `apps/web`, `packages/solid-stately` | `SC9014` present → absent | the same. Neither reaches `sharedConfig` or the other four names |

**ADRs 0128 and 0129** do not show up as a direct consumer move. They change
what certifies through the `solid-js` and `@solidjs/web` rc.9 graph nodes. The
meta rc.9 bundle's environment includes both nodes, so it is *estimated*, but
not isolated here, that the bundle depended on them. The ADR 0128 text says
that without 0128 every graph composing `solid-js@2.0.0-rc.9` refused at that
node. No bundle was re-certified to separate the two.

**The withdrawn rc.3 rows** (`Show` reads and creates, `Loading` creates,
`render` and `hydrate` reads) moved no kobalte finding. The tier-off control
shows kobalte differing only in `SC9014`.

### What still stops the rc.9 consumers

- **viviana `apps/web`**: 213 acceptance-gate sites on `@tanstack/solid-router`
  and 2 on `@tanstack/solid-start/server-entry`. The tier's only
  `@tanstack/solid-router` bundles are 2 for the `ssr/client` specifier. That
  specifier is not imported by the app.
- **oscar**: `@solidjs/router` (3 sites) and `@solidjs/router/fs` (2) in the
  app. The environment records `@solidjs/router` 2.0.0-next.26 as unmatched,
  because the manifest row is next.30. In the node config,
  `@solidjs/vite-plugin` (1).

## Spot checks against source

1. **kobalte, `@solid-primitives/form`.**
   `packages/core/src/checkbox/checkbox-root.tsx` imports
   `createFormResetListener` from `@solid-primitives/form` at byte 373. It
   calls `createFormResetListener(ref, () => state.setIsSelected(…))` at byte
   4753. The installed `dist/form-reset-listener.js` is
   `createEffect(() => element(), (element) => { … form.addEventListener("reset", handler …); return () => … })`:
   it reads the accessor in a tracked compute, needs an owner, and runs the
   handler later. The contract leaves exactly `reactiveReads`,
   `ownerRequirements` and `callbacks` open. The 30 callback sites are the two
   function arguments of each of the 15 calls, in 15 files: for example `ref`
   at 4753–4756 and the handler at 4758–4822. **Real.** The finding replaces
   an `SC9011` that said the callee "has no … package contract entry", which is
   no longer true.
2. **oscar, `@solidjs/meta`.** `src/routes/(public).tsx:1` is
   `import { Link, Meta, Title } from "@solidjs/meta"`, and `addy/index.tsx`
   and `invoicer/index.tsx` each import `{ Meta, Title }`. That is 7 bindings,
   matching Link 1, Meta 3 and Title 3. **Real.**
3. **viviana, the scoped re-export gap.** The notice in `packages/geist` cites
   `packages/solidaria-components/src/Tabs.tsx` at byte 1008. That is the
   `sharedConfig,` specifier in its `import { … } from "solid-js"`, and
   `src/utils.tsx:41` imports it as well. No file under `apps/web/src` names
   `sharedConfig`. **Real**, but see the caveat on unbuilt workspace packages.
4. **Flagged, not a false violation: `SC9014` on Solid-free projects.** kobalte
   `packages/tailwindcss` has one file (`src/index.ts`), `filesAnalyzed: 0`,
   and no `solid-js` import or dependency. Its only finding is the notice,
   anchored at the root `node_modules/solid-js/package.json` that the dialect
   walk finds above it. It was `certified` before and is `uncertifiable` now.
   The same held for oscar's empty solution config before the change. The
   notice follows ADR 0110's dialect selection, so it is by design. But
   "cannot certify the project" is a claim about a project whose certification
   no Solid gap can affect. This is a precision question for the notice, not a
   defect in any analysis.

No false positive was found among the moved findings. No violation moved, so
no moved finding needed a `tsc`-duplication check.

## Controls

The `--no-bundled-contracts` runs, under both builds:

- **before-notier → after-notier.** The only differences are `SC9014`, the
  same moves as in the table above, and the three status changes it causes.
  So everything else that moved with the tier on is the tier's.
- **Tier on vs off, per build.** kobalte's acceptance-gate sites are 581 with
  the tier off under both builds. With it on they are 311 before and 281
  after, so 270 and 300 bindings resolved. The 30 more are form's 15 bindings,
  counted in root and core. oscar is 13 off and 6 on after (meta's 7), and 13
  on before. viviana is 562 in all four runs.
- kobalte has 449 violations with the tier off and 424 with it on, under both
  builds. The tier's 25-violation effect there predates this campaign and did
  not change.

## Wall time

| run (21 projects, sequential, release, one-shot) | kobalte | viviana-ui | oscar | total |
| --- | ---: | ---: | ---: | ---: |
| before | 30.6 s | 125.3 s | 1.1 s | 157.0 s |
| after | 30.5 s | 124.4 s | 1.1 s | 156.0 s |
| before, tier off | 22.3 s | 123.8 s | 0.8 s | 146.9 s |
| after, tier off | 22.3 s | 123.7 s | 0.8 s | 146.8 s |

This is one sample per cell. The largest single project is viviana
`packages/solid-spectrum` (68.6–69.4 s). The rc.9 changes cost no measurable
wall.

## Caveats

- **viviana's workspace packages are unbuilt.** `--ignore-scripts`, as on
  2026-09-26, leaves `packages/*/dist` absent. The ~250 `apps/web` imports of
  `@proyecto-viviana/*` therefore resolve to packages whose `exports` point at
  missing files. The absence of `SC9014` there, and apps/web's absolute
  numbers, describe that tree, not a built one. The before/after difference is
  unaffected, because both builds saw the same tree.
- **Three consumers.** They are the three consumer environments. No other rc.9
  consumer was swept. The tier's 120 (+12) bundles on (rc.3, signals rc.9,
  rc.3), which is what a fresh rc.3 install resolves today, reach none of
  them: kobalte's lock pins signals rc.3.
- **Site units differ by gate.** An acceptance-gate site is an import binding.
  An open-claims site is an import binding or a call argument, depending on the
  domain. That is why kobalte's `SC9005` site total rises (787 → 847) while its
  acceptance-gate sites fall. The resolved-bindings row counts import bindings
  only.
- **The ADR 0128/0129 attribution to the meta bundle is estimated.** It was not
  isolated by re-certification.
- **The consumer branches move.** The commits above are the measured state.
