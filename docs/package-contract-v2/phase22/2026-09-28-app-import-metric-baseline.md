# The app import metric: a baseline over 38 real Solid 2 applications

Measured 2026-09-28 at `061cc531` plus this change, which touches only
measurement tooling and documents. Every number below is **measured** unless it
is marked *estimated*. The harness is `scripts/app-import-metric.mjs`, run with
`make app-import-metric`. The corpus is pinned in
`scripts/ecosystem-benchmark/app-import-corpus.json`.

The owner decided on 2026-09-28 that package contracts succeed when they
certify what real Solid 2 applications import, counted by call site. This is
now the **primary** metric. `make certification-metric` counts every export of
the top 30 packages with equal weight, and it stays as the secondary number
(see [its baseline](2026-09-28-certification-metric-baseline.md)).

## Headline

| | value |
| --- | ---: |
| apps analysed / apps with a third-party import site | 38 / 37 |
| third-party import sites in application source | **1,872** |
| **certified, pooled** | **0 (0.0 %)** |
| certified, mean per app | 0.0 % |
| open (an accepted contract applies, a domain the site needs is open) | 220 (2 apps) |
| no contract | 1,652 (37 apps) |
| refused / unexplained | 0 / 0 |
| distinct third-party package versions | 84 |
| not in the headline: workspace sites / test-file sites / build-config sites | 1,629 / 678 / 35 |
| harness wall: cold run (fetch, install, analyse 38 apps) | about 44 min (*see Wall time*) |
| harness wall: warm rerun (installs and analyses cached) / `--measure` | 134 s / 1 s |

- **No import site in any app is certified.** 1,652 of the 1,872 sites (88 %)
  reach no accepted contract. The other 220 reach one that leaves `reads`,
  `returns` and `creates` open at exactly the exports the apps call.
- **Only two of the 38 apps reach an accepted contract at all.** Those are
  viviana-ui (`@tanstack/solid-router` rc.8, 213 sites) and
  oscartbeaumont-website (`@solidjs/meta`, 7 sites). Both are reviewed consumer
  environments whose trees the tier was regenerated for. For oscar, the 7 open
  and 5 no-contract sites are the 2026-09-27 report's numbers. For viviana, the
  2026-09-27 report found the same 213 `@tanstack/solid-router` sites at the
  acceptance gate. They now reach the tier's `.` bundles, which were added
  since, and stop at open claims.
- **Most of the demand is for packages the tier does not hold.** 1,036 sites
  (55 %) are in packages with no bundle at any version. The largest are
  `lucide-solid` (382), `@solidjs/router` (272, in 27 of the 38 apps),
  `solid-icons` (144) and `@tanstack/solid-query` (89).
- **Two lockfile readers refuse formats that current package managers write.**
  They block 218 sites in 12 apps before any contract is compared, and this is
  the cheapest wall to remove. pnpm ≥ 11 writes a two-document
  `pnpm-lock.yaml` (6 apps). Bun writes `bun.lock` `lockfileVersion: 1` (6
  apps). Both come back as "the installed package has no exact lockfile
  integrity".
- **Closing domains on the package side does not reach the apps yet.** 316
  sites are in a package version that `make certification-metric` measures.
  On the package side, 26 of those exports are clean, 34 partial and 241
  degenerate. So even if every admission wall were gone, the metric's ceiling
  from today's package answers would be **26 sites, 1.4 %** (*estimated*: it
  assumes admission changes nothing else).

## The corpus

38 applications, listed in `app-import-corpus.json` with commit, lockfile
digest, the tsconfig(s) measured, and why each one qualifies. They are:

- the three reviewed consumer environments, pinned at their environment
  commits: oscartbeaumont-website, viviana-ui `apps/web`, and kobalte's docs
  app `apps/docs`;
- 35 apps found on 2026-09-28 by GitHub code search.

The search looked for `package.json` files that name `@solidjs/web`,
`@solidjs/signals`, `@solidjs/start` or a 2.x `solid-js`. That gave 1,753
manifests in 305 repositories. The following were dropped:

- forks, archived repositories and template repositories;
- template, benchmark, fixture, playground and starter paths;
- libraries: a published package with `exports`/`main` that is not `private`;
- apps with no third-party Solid dependency, which would add nothing to the
  denominator.

The remaining candidates were reviewed by hand. Near-duplicates were dropped
(for example a copy of openbot), as were stale betas (before 2026-06) and one
self-described template.

Four reviewed candidates are recorded under `excluded`. ritsei, astral and tofu
commit no lockfile, so they cannot be installed frozen. TheTree's lockfile
disagrees with its manifests, so pnpm refuses a frozen install. One app,
every-deck-of-cards, carries `installArgs: ["--config.engine-strict=false"]`
and says why: its workspace sets `engineStrict`, and a dev dependency's
`engines` range excludes the harness's Node 24.11.1. The flag leaves the
frozen tree unchanged. For app-game the measured project is `apps/web`, the
deployed shell. The repository-root tsconfig spans every workspace package, and
the checker refuses it: `AST facts error: Oxc parse failed: Unexpected token`.
The failing file was not isolated.

| | |
| --- | --- |
| resolved `solid-js` (sites) | rc.0 51, rc.1 19, rc.3 184, rc.4 406, rc.6 163, rc.7 56, rc.8 499, rc.9 494 |
| package managers | pnpm 20 (10.x-12.x), bun 13 (`bun.lock` v1 ×9, v2 ×3, `bun.lockb` ×1), npm 4, yarn 1 |

Only 14 of the 38 apps are on rc.8 or rc.9. The audited release is rc.9, and
the tier's 729 bundles are certified on `@solidjs/signals` rc.0 (58), rc.3
(130) and rc.9 (541) only. Most of the demand is
on runtimes the tier has no bundle for (see wall 7).

## Method

`make app-import-metric` builds the release checker. It then runs
`bun scripts/app-import-metric.mjs --run` under `rust/target/app-import-metric/`:

1. **Fetch and install.** Each app is fetched at its pinned commit, and its
   lockfile digest is checked against the pin. It is installed with its own
   package manager, frozen, with scripts off: `pnpm install --frozen-lockfile
   --ignore-scripts` (the `packageManager` version), `bun install
   --frozen-lockfile`, `npm ci --ignore-scripts`, or `yarn --frozen-lockfile`.
   An install is reused while the commit, the lockfile digest and the command
   are unchanged (`apps/<id>.install.json`). Each measured project must then
   resolve a 2.x `solid-js`, walking `node_modules` the way `dialect.rs` does,
   or the app is refused. All 38 resolve 2.x.
2. **Analyse.** Each project is run with `SOLID_CHECKER_DAEMON=0
   solid-checker-rust --format json --project <tsconfig>`, once as shipped and
   once with `--no-bundled-contracts`, as the 2026-09-27 report did. Then
   `--check-contracts` is run for the per-package admission sentences. Analyses
   run one app at a time. Installs run in parallel.
3. **Extract sites.** An import site is one location at which the acceptance
   gate raises `SC9005` for a package export: one per value binding of an
   import declaration, and one per member of a namespace import. The
   denominator is read from the **tier-off** run, where every such site stops
   at the acceptance gate. That run reports nothing for a binding used only in
   types, or for a package whose manifest does not use Solid
   (`manifest_uses_solid`). Neither is a site, by the checker's own rule. A
   collapsed finding's sites are its primary and related locations, and each
   site's export is read from the source bytes. This is the method of
   `phase21/2026-09-12-consumer-demand-measurement.py`. 21 sites (1.1 %) name
   a declaration whose bindings the bytes do not settle one-to-one. They are
   counted under export `?`.
4. **Classify.** Each tier-off site takes the state that the shipped run gives
   the same `(file, module, export)`:
   - **no contract**: still at the acceptance gate;
   - **open**: `unknown-contract-claims:<domains>`, an omitted export, no
     environment branch, or a callback passed at a call whose execution is
     unknown;
   - **refused**: obsolete policy 1, or an unbound claim;
   - **certified**: no finding, and the package is `accepted` in
     `packageSummaries`;
   - **unexplained**: no finding and no accepted package, never counted as
     certified.

   `solid-js`, `@solidjs/signals` and `@solidjs/web` belong to the dialect and
   are never sites. Three kinds of site are reported apart from the headline:
   workspace packages (those whose real path is outside every `node_modules`),
   sites in test files, and sites in build configs (`roleOfPath`). No tier-on
   finding lacked a tier-off site (`onOnly` 0), and nothing was unexplained.
5. **Walls.** A no-contract site is explained by the `--check-contracts`
   sentence when there is one, else against `pkg/contracts/accepted/index.json`.
   A no-contract site has one of these causes:
   - no bundle for the package;
   - another installed version;
   - the specifier;
   - the Solid runtime or another environment entry;
   - a lockfile integrity the reader cannot read.

   An open site gets one cause per open domain. The cause comes from the host-free
   `make certification-metric` run of the same day (`--package-metric`, 30
   packages, 416 s), joined by package version, entrypoint and export and
   classified by its `causeOf`. When the package or version is not in that
   corpus, the cause says so.

The harness certifies nothing and changes no product path. Its pure parts
(site attribution, classification, walls, aggregation, rendering, install
commands, corpus shape) are tested in `scripts/app-import-metric.test.mjs`.

## Per app

| app | `solid-js` | sites | open | no contract | workspace sites | analysis wall |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| app-game (`apps/web`) | rc.4 | 336 | 0 | 336 | 87 | 417.4 s |
| derp-media-server | rc.8 | 333 | 0 | 333 | 0 | 262.2 s |
| viviana-ui-web | rc.9 | 215 | **213** | 2 | 0 | 22.5 s |
| sefer | rc.9 | 170 | 0 | 170 | 0 | 10.6 s |
| error-menu-web | rc.3 | 96 | 0 | 96 | 0 | 13.2 s |
| readingroom | rc.8 | 92 | 0 | 92 | 0 | 1.3 s |
| probus-hk | rc.8 | 58 | 0 | 58 | 0 | 5.3 s |
| solid-groove | rc.3 | 57 | 0 | 57 | 0 | 373.4 s |
| ai-memory-ui | rc.6 | 50 | 0 | 50 | 0 | 2.6 s |
| civil | rc.6 | 48 | 0 | 48 | 0 | 18.0 s |
| sensor-sim | rc.7 | 44 | 0 | 44 | 0 | 1.2 s |
| spotify-desk-thing | rc.4 | 38 | 0 | 38 | 0 | 0.9 s |
| finds-team | rc.9 | 33 | 0 | 33 | 0 | 2.3 s |
| kui | rc.6 | 29 | 0 | 29 | 468 | 31.9 s |
| openbot | rc.0 | 28 | 0 | 28 | 1,064 | 624.9 s |
| donegeon-client | rc.0 | 23 | 0 | 23 | 0 | 3.4 s |
| en-passant | rc.9 | 22 | 0 | 22 | 0 | 5.2 s |
| kobalte-docs | rc.3 | 19 | 0 | 19 | 0 | 1.5 s |
| compass-ui | rc.1 | 19 | 0 | 19 | 0 | 12.5 s |
| jibe-run | rc.4 | 19 | 0 | 19 | 0 | 3.3 s |
| skyjtx-website | rc.8 | 16 | 0 | 16 | 9 | 6.0 s |
| helge-dev | rc.9 | 14 | 0 | 14 | 0 | 0.5 s |
| jar-hell-web | rc.4 | 13 | 0 | 13 | 0 | 0.9 s |
| oscartbeaumont-website | rc.9 | 12 | **7** | 5 | 0 | 3.5 s |
| beacon-web | rc.3 | 12 | 0 | 12 | 0 | 1.7 s |
| inferay | rc.6 | 10 | 0 | 10 | 0 | 6.5 s |
| queue-management-ui | rc.6 | 9 | 0 | 9 | 0 | 1.1 s |
| lutra-console | rc.9 | 8 | 0 | 8 | 0 | 0.6 s |
| expenses-app | rc.9 | 8 | 0 | 8 | 0 | 8.0 s |
| every-deck-of-cards | rc.7 | 7 | 0 | 7 | 0 | 1.0 s |
| solid-validation-site | rc.9 | 7 | 0 | 7 | 0 | 0.7 s |
| dashiboard-ui | rc.6 | 6 | 0 | 6 | 0 | 4.8 s |
| raybend-website | rc.6 | 5 | 0 | 5 | 0 | 0.7 s |
| jandibat-web | rc.9 | 5 | 0 | 5 | 1 | 3.2 s |
| spookysoftware-www | rc.7 | 5 | 0 | 5 | 0 | 4.0 s |
| hoxxes-briefing | rc.6 | 3 | 0 | 3 | 0 | 1.6 s |
| pekochan-blog | rc.6 | 3 | 0 | 3 | 0 | 35.6 s |
| a-worse-ereader | rc.4 | 0 | 0 | 0 | 0 | 0.6 s |

Every app has 0 certified sites, so the per-app share is 0.0 % for all 37 apps
with sites. The "analysis wall" is the sum of the tier-on and tier-off runs.
Three apps hold 47 % of the sites: app-game (mostly older `@solid-primitives`),
derp-media-server (277 `lucide-solid` icon imports) and viviana. The mean per
app weights each app equally and is also 0.

## Per package (top 20 by sites)

| package | apps | sites | open | no contract | in tier |
| --- | ---: | ---: | ---: | ---: | --- |
| `@tanstack/solid-router@2.0.0-rc.8` | 3 | 308 | 213 | 95 | yes |
| `lucide-solid@1.45.0` | 1 | 277 | 0 | 277 | no |
| `solid-icons@1.2.0` | 5 | 144 | 0 | 144 | no |
| `@solidjs/meta@1.0.0-next.2` | 17 | 132 | 7 | 125 | yes |
| `lucide-solid@1.47.0` | 1 | 103 | 0 | 103 | no |
| `@tanstack/solid-query@6.0.0-rc.3` | 4 | 66 | 0 | 66 | no |
| `@tanstack/solid-router@2.0.0-rc.4` | 2 | 65 | 0 | 65 | other version |
| `@solid-primitives/resize-observer@4.0.0-next.3` | 1 | 59 | 0 | 59 | yes |
| `@solidjs/router@2.0.0-next.24` | 2 | 59 | 0 | 59 | no |
| `@solid-primitives/event-listener@3.0.0-next.3` | 1 | 53 | 0 | 53 | other version |
| `@solidjs/router@2.0.0-next.26` | 7 | 47 | 0 | 47 | no |
| `@solidjs/router@2.0.0-next.21` | 6 | 47 | 0 | 47 | no |
| `@tanstack/solid-router@2.0.0-rc.6` | 1 | 47 | 0 | 47 | other version |
| `@solidjs/router@2.0.0-next.18` | 4 | 44 | 0 | 44 | no |
| `@solid-primitives/utils@7.0.0-next.4` | 1 | 42 | 0 | 42 | yes |
| `@solid-primitives/raf@4.0.0-next.2` | 1 | 39 | 0 | 39 | no |
| `@kobalte/core@2.0.0-alpha.2` | 5 | 29 | 0 | 29 | yes |
| `@solidjs/router@2.0.0-next.23` | 3 | 26 | 0 | 26 | no |
| `@tanstack/solid-query@6.0.0-rc.0` | 3 | 23 | 0 | 23 | no |
| `@kobalte/core@2.0.0-alpha.0` | 3 | 18 | 0 | 18 | other version |

In total, `@solidjs/router` has 272 sites across ten installed versions
(next.16 to next.26, plus a 1.x-era 0.16.2). None of them is the next.30 that
the certification metric measures. `@tanstack/solid-router` has 437 sites
across four versions. The full table, 84 package versions, is in
`rust/target/app-import-metric/metric.md`.

Not in the headline: the 678 test-file sites are `@solidjs/testing-library`,
the vite plugins, and app packages imported by tests. The 35 build-config sites
are `vite-plugin-solid`, `@solidjs/vite-plugin`, `@tanstack/solid-start` (its
vite plugin), `@solidjs/prerender`, `astro-solid-next` and
`vite-plugin-solid-svg`.

## Walls, ranked by the sites they block

### By class

| class | sites blocked | solely | apps |
| --- | ---: | ---: | ---: |
| no contract: package not in the tier | 1,036 | 1,036 | 37 |
| open: `reads` / `returns` / `creates` (each) | 220 | 0 | 2 |
| no contract: lockfile integrity not read | 220 | 220 | 13 |
| no contract: installed version not in the tier | 165 | 165 | 4 |
| no contract: no bundle on the installed Solid runtime | 121 | 121 | 9 |
| no contract: another environment entry differs | 110 | 110 | 1 |

Every open site has all three domains open. No single domain therefore clears
any site ("solely" 0).

### The top ten walls

| # | wall | sites | apps | top exports | fix size (*estimated*) |
| ---: | --- | ---: | ---: | --- | --- |
| 1 | **`lucide-solid` has no contract.** It is a Solid-1.x-peer icon package (`peerDependencies: solid-js ^1.4.7`) installed into Solid 2 apps, at 1.45.0, 1.47.0 and 1.33.0 | 382 | 3 | `icons/x`, `icons/search`, `icons/loader-circle` (all `default`) | **policy (ADR)** first: whether and how a 1.x-peer package is certified against a 2.x runtime. Then one certification per version. The per-icon modules are uniform, so one family recipe could cover them |
| 2 | **`@solidjs/router` has no contract at any version.** The certification metric measures next.30, where 0 of 32 exports are clean, and it is not bundled. Apps install next.16 to next.26 | 272 | **27** | `useNavigate` 51, `useLocation` 41, `useParams` 34, `createRouter` 29, `/fs` `defineFileRoute` 22 | **large**: certification and a tier bundle per installed version and environment (10 versions). The package-side closure is also still 0 % clean |
| 3 | **`@tanstack/solid-router` rc.8 is accepted but leaves `reads`, `returns` and `creates` open** in viviana. The package is not in the certification-metric corpus, so the cause per domain is not recorded | 213 | 1 | `createFileRoute` 181, `Link` 12, `Outlet` 8 | **tooling, then ADRs**: add the package to the certification-metric corpus so the walls can be ranked. Then fix the claim forms it names |
| 4 | **The lockfile integrity is not read from a pnpm lockfile with a `---` document marker.** pnpm ≥ 11 writes one when it records `packageManagerDependencies`. `refuse_pnpm_yaml_beyond_subset` (`contract_certification/dependencies.rs`) refuses any document marker | 144 | 6 | `createFileRoute` 31, `@solidjs/meta` `Title` 24 | **small, tooling**: read the lockfile document that holds the project importers, and refuse ambiguity. It needs one fixture per shape. No ADR is needed, unless choosing the document is a trust question |
| 5 | **`solid-icons` 1.2.0 has no contract.** It peers `solid-js: *` and is 1.x-era | 144 | 5 | `hi` `HiSolidPlus`, `bi`, `fi` icons | the same as wall 1 |
| 6 | **`@tanstack/solid-query` rc.0 and rc.3 have no contract.** The certification metric measures rc.4 (11 of 52 exports clean), and it is not bundled | 89 | 7 | `useQuery` 29, `useQueryClient` 18, `useMutation` 15 | **medium**: bundle it per installed version and environment |
| 7 | **No bundle is on the installed Solid runtime.** `@solidjs/meta` on `@solidjs/web` rc.4, rc.6 and rc.7 has bundles on rc.0 and rc.9 only (67 sites). `@kobalte/core` alpha.2 on signals rc.8 has a bundle on rc.9 (25). Scheduled, storage, timer and marker account for the rest | 121 | 9 | `Title`, `Meta`, `Tooltip`, `Dialog` | **medium**: consumer-environment runs for the corpus apps' exact trees (`derive-consumer-environment.mjs`, `make consumer-environment-runs`). Each needs an audited or archived runtime (ADR 0127) |
| 8 | **The lockfile integrity is not read from `bun.lock` `lockfileVersion: 1`.** `bun_package_integrity` (`diagnostics.rs`) accepts only version 2. There are 72 such sites, plus 2 from a binary `bun.lockb` | 74 | 6 | `Title` 20, `Meta` 11, `createFileRoute` 11 | **small, tooling**: read v1 if its package tuple carries the integrity at the same index, which must be verified against published v1 lockfiles. `bun.lockb` stays unreadable |
| 9 | **The installed `@tanstack/solid-router` is not the certified rc.8.** It is rc.4 (65 sites), rc.6 (47) or beta.29 (17) | 129 | 4 | `useLinkProps`, `createFileRoute`, `useNavigate` | **medium**: tier bundles per version (consumer-environment runs), or wait for the apps to move |
| 10 | **app-game pins older `@solid-primitives`.** `event-listener` is next.3 where the tier has next.5 (53 sites). `resize-observer`'s environment then differs on the same package (59). `utils`'s environment differs on `@floating-ui/core` 1.7.5 against 1.8.0 (42) | 154 | 1 | `createWindowSize` 39, `createEventListener` 25, `access` 17 | **medium**: a consumer-environment run for this app's tree. 25 of these sites are `utils` exports that are clean on the package side, which makes this the only wall behind which certified sites are waiting (*estimated*) |

Walls 4 and 8 mask what lies behind them. The harness's own environment probe
replays ADR 0123's walk with versions only. By that probe, 106 of their 218
sites would then match a bundle's recorded environment, and 66 would stop at
the Solid runtime (*estimated*). An admitted bundle then leaves the site open,
not certified, unless its export is closed (see the ceiling above).

**Walls 4 and 8 removed (2026-09-28, later the same day).** The pnpm reader now
reads pnpm 11's env document, and admission's Bun reader now reads `bun.lock`
version 1 (ADR 0108's amendment). The same release-checker commands were
rerun on the 12 affected apps, before (`227a7360`) and after the fix. The
before run reproduced this report's 144 + 74 sites exactly. After the fix,
none of the 218 sites stops at a lockfile, and **none is certified**:

| where the 218 sites went | sites | apps |
| --- | ---: | --- |
| **open**: `@tanstack/solid-router` rc.8 (31), `@solidjs/meta` next.2 (11); `ownerRequirements`, `reactiveReads` and (except one site) `returns` stay open | 42 | finds-team, solid-validation-site, lutra-console, helge-dev |
| no contract: the dependency environment differs (`@tanstack/router-core` installed 1.171.32, certified 1.171.22) | 64 | sefer |
| no contract: no bundle on the installed Solid runtime (`@solidjs/meta`, `@kobalte/core` alpha.2, four `@solid-primitives`) | 64 | readingroom, raybend-website, civil, solid-groove, jibe-run, solid-validation-site, lutra-console |
| no contract: the installed version is not in the tier (`@kobalte/core` alpha.0/alpha.1, `@tanstack/solid-router` beta.29) | 48 | readingroom, compass-ui, openbot, jibe-run, civil |

The only lockfile shape still unread in the corpus is beacon-web's binary
`bun.lockb` (2 sites).

The other open wall is `@solidjs/meta` in oscar (7 sites). Here the package
side did measure the installed version, so each domain has a cause:

- `creates` is declined as `refusing-callee-fixpoint`;
- `reads` is withheld for want of a probe recipe;
- `returns` is `operation census refused: recursive-value-shape`.

## Recommended order of work

1. **Fix the two lockfile readers** (walls 4 and 8; small, tooling). This is
   the only step that removes a wall without new certification, and it
   decides admission for 218 sites in 12 apps. It raises no headline by
   itself, because what it admits is open, but every later step depends on
   admission being read correctly.
2. **Measure what the apps call, on the package side.** Add
   `@tanstack/solid-router`, `@tanstack/solid-query` (at the installed
   versions), `lucide-solid` and `solid-icons` to the certification-metric
   corpus, or to a sibling corpus driven by this metric's demand. Then join
   again. Today 1,556 of the 1,872 sites are in package versions with no
   package-side record.
3. **Close the claim forms the demanded exports hit.** These are
   `createFileRoute`, `Link`, `Outlet`, `useNavigate`, and meta's `Title` and
   `Meta`: the recursive-value-shape returns census, the refusing-callee
   fixpoint for `creates`, and the missing `reads` recipes. Each is one ADR, as
   in the certification-metric baseline's walls 4 and 5. This is the only step
   that can move a site to certified, and without it the ceiling stays at
   about 1.4 %.
4. **Cover the installed versions and runtimes** (walls 7, 9 and 10; medium):
   consumer-environment runs from the corpus apps' own lockfiles for the
   packages already in the tier. This affects 396 sites. It is the mechanism
   that brought the 220 open sites into reach at all.
5. **Certify `@solidjs/router`** at the versions apps install (wall 2; large,
   27 of 38 apps). It follows step 3, because the package side is 0 % clean at
   next.30.
6. **Decide the 1.x-peer policy** (walls 1 and 5; an ADR, 526 sites). Icon
   packages are the largest single demand, and they are trivial components.

## Wall time

| step | wall |
| --- | ---: |
| installs, 38 apps, up to 6 in parallel (sum of install walls / longest) | 4,402 s / 492 s |
| analyses, tier on + off, sequential (sum over apps) | 1,895 s |
| contract reports (`--check-contracts`), sum | 132 s |
| cold `--run`, first pass (38 apps; 6 installs cached from an aborted attempt) | 36.8 min |
| warm `--run` (every install and analysis cached) | 134 s |
| `--measure` | about 1 s |

The walls are upper bounds. Other agents were certifying on the same
14-core machine throughout, and the installs shared the network. Four apps take
88 % of the analysis time: openbot (625 s), app-game (417 s), solid-groove (373
s) and derp-media-server (262 s). The remaining 34 apps take 0.5 to 36 s each. A
full cold run is about 44 minutes (*estimated*): the first pass plus
app-game's and every-deck-of-cards' reruns.

## Caveats

- **The corpus comes from a search.** The apps were found by code search on
  one day. Many are small, and three hold 47 % of the sites, so the per-app
  mean is the more robust number. It is also 0.
- **Unbuilt workspace packages.** Scripts are off, as in the 2026-09-26/27
  sweeps, so workspace packages with a build step (viviana, openbot, kui) have
  no `dist`. Their imports are first-party and outside the headline either
  way.
- **Monorepo scope.** For app-game, only `apps/web` is analysed. Third-party
  imports that live inside its 60 workspace packages are not counted.
- **Classifying by file path.** Test and build-config sites are identified by
  path. A test helper under another name counts as an app site: 10
  `@solidjs/testing-library` sites remain in the headline.
- **The environment probe is the harness's own.** The walls use the checker's
  admission sentence wherever `--check-contracts` gave one. The "behind wall
  4 and 8" estimate uses the probe, which checks versions and not integrity.
- **The same tier throughout.** The tier is the one compiled into `061cc531`
  (729 bundles). A concurrent change to contract generation is not measured
  here.

## Reproducing

```sh
make certification-metric CERTIFICATION_METRIC_HOSTS=   # optional, for the package-side join
make app-import-metric APP_IMPORT_PACKAGE_METRIC=rust/target/certification-metric/metric.json
# or, after a run: bun scripts/app-import-metric.mjs --measure --package-metric …
```

Outputs are written to `rust/target/app-import-metric/metric.{json,md}`.
`APP_IMPORT_CLEAN=1` deletes the clones afterwards. The extracted sites are
kept, so `--measure` still works. Re-pin the corpus with `--pin <draft.json>`.
