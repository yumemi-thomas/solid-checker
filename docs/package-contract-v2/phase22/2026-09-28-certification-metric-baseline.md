# The certification metric: a baseline over the top 30 Solid 2 packages

Measured 2026-09-28 at `f67a9a31` (after ADR 0138, before ADR 0139). Every
number below is **measured** unless it is marked *estimated*. The harness is
`scripts/certification-metric.mjs` and `make certification-metric`; the corpus
is pinned in `scripts/ecosystem-benchmark/certification-metric-corpus.json`.

Until now coverage was reported consumer by consumer (the census's frozen
demand, the 2026-09-26/27 consumer sweeps), which weights the answer by a few
heavily used packages and by what one consumer happens to import. This metric
asks the owner's question directly: of the export surface of the Solid 2
packages people install most, how much does the checker certify clean, and
what blocks the rest?

## Headline

| | value |
| --- | ---: |
| exports certified clean, **mean per package** (30 packages, equal weight) | **3.4 %** |
| exports certified clean, **weighted by weekly downloads** | **4.4 %** |
| exports certified clean, pooled | 41 of 918 (4.5 %) |
| buckets: clean / partial / degenerate / uncertified | 41 / 198 / 679 / 0 |
| misuse-capable exports (a positive claim a rule can consume) | 141 |
| — by class: invoke / argument read / returned accessor / owner requirement | 77 / 41 / 35 / 25 |
| packages not certifiable at all | 1 (`@solidjs/testing-library`) |
| packages with at least one clean export | 6 of 30 |
| harness wall, 30 probes (`make certification-metric`, release binary) | 280 s run + 2 s measurement |

- **Almost nothing is certified clean.** 24 of 30 packages have no clean
  export at all. The six that do are `@solid-primitives/utils` (19 of 99),
  `@tanstack/solid-query` (11 of 52), `@kobalte/core` (5 of 568),
  `@kobalte/utils` (3 of 10), `@solid-primitives/storage` (2 of 11) and
  `@solid-primitives/props` (1 of 8). Most of the 41 are constants, symbols and
  manager objects (`isServer`, `focusManager`, `COLOR_MODE_STORAGE_KEY`); the
  callables among them are small helpers (`noop`, `clamp`, `access`,
  `asArray`, `isObject`, `callHandler`).
- **The exports that are not clean are open in every domain.** 759 of the 877
  non-clean exports have all four consumer domains (callbacks, reads, returns,
  creates) open; only 14 have one. So no single wall clears many exports on its
  own; see the greedy combination below.
- **The largest per-package wall cannot be read from the run.** 19 of 30
  packages certify on the published-graph lane, which keeps no decline and no
  unresolved-claim record per graph node. That absence blocks, averaged over
  the 30 packages, 61.6 % of a package's exports, and 86 exports are blocked by
  nothing else.
- **The largest pooled wall is a one-line tooling defect.** `@kobalte/core`'s
  graph recovery never runs: every native transaction refuses with
  `missing field lockfile`, so 525 of its 568 exports stay behind unaccepted
  dependencies (`@solid-primitives/utils`, `@kobalte/utils`,
  `@solid-primitives/controlled-signal`, `@internationalized/number`). With
  the line changed locally, the recovery publishes all 136 cases, but the
  package's clean share only moves from 0.9 % to 1.2 %: its answers join the
  unobservable graph-lane wall. See wall 2 and the experiment.
- Two runs of the harness over the same binary gave identical buckets and
  headline numbers; one export moved between two wall classes (the counts
  below are from the second run and may differ by one from the first).

## The corpus

`bun scripts/certification-metric.mjs --select` ranks by last-week npm
downloads (2026-09-20..26, `api.npmjs.org/downloads/point/last-week`) every
`solid2` row of `scripts/ecosystem-benchmark/manifest.json` (141 packages,
generated 2026-09-27) plus a reviewed list of Solid 2 packages outside the
manifest's families. It skips the runtime foundation and build tooling and takes
the first 30. Each package is pinned to the manifest row's exact version and
integrity and measured by its `head` probe (solid-js/web/signals 2.0.0-rc.9, the
audited triple), or by its `only` probe when the row has one compatible Solid
release. `--print-probes` refuses a manifest that no longer agrees with the pin.

The counts used for the pin were fetched at 10:05 JST from the same endpoints;
a live re-selection at 10:40 was rate-limited (HTTP 429) after the discovery
sweeps, so the pin was written with `--downloads` from those counts. The
download window is recorded in the corpus file.

**Source of truth.** Every selected package is a manifest row: the manifest's
families already cover every Solid 2 package with more than 213,363 weekly
downloads, apart from the skips below.

**Skipped above the cut-off** (recorded in the corpus file's `skipped`):

| package | weekly downloads | reason |
| --- | ---: | --- |
| `solid-js` | 6,277,534 | runtime foundation (ADR 0027): dialect vocabulary, never a package contract; `@solidjs/web` and `@solidjs/signals` rank below the cut-off and are excluded for the same reason |
| `babel-preset-solid` | 1,386,596 | build tooling (not in the manifest; 2.0.0-rc.2 is Solid 2 compatible) |
| `vite-plugin-solid` | 1,081,640 | build tooling (not in the manifest; 3.0.0-next.26) |
| `solid-refresh` | 1,021,607 | dev-server tooling (not in the manifest; 0.8.0-next.7) |
| `solid-use` | 317,306 | **addition not taken**: `solid-use@1.0.0-next.3` is Solid 2 compatible but belongs to no family in `lib/families.mjs`, so it has no manifest row; adding a family means re-running `make ecosystem-discover`, which moves the ecosystem-regression corpus, so it is left for a deliberate manifest change |
| `storybook-solidjs-vite` | 297,949 | test-runner tooling (not in the manifest) |

Other Solid 2 packages outside the families, all below the cut-off:
`cmdk-solid@2.0.0-rc.0` (17,810), `unplugin-solid@3.0.0` (5,463, tooling),
`@dschz/solid-flow@1.0.0-next.20` (2,581), `@rimelight/ui@0.0.70` (2,061). High-
download Solid packages checked by hand and found to have **no** Solid 2
release: `@corvu/utils`, `solid-presence`, `solid-transition-group`,
`solid-prevent-scroll`, `@neodrag/solid`, `@ai-sdk/solid`, `@opentui/solid`,
`@tanstack/solid-virtual`, `solid-list`, `@astrojs/solid-js`,
`@thisbeyond/solid-dnd`, `solid-motionone`, `solid-floating-ui`, `@sentry/solid`,
`@tanstack/solid-store`, `lucide-solid`, `@dnd-kit/solid`, `solid-sonner`,
`@tanstack/solid-form`, `@tanstack/solid-table`, `@zag-js/solid`,
`@ark-ui/solid`, `@modular-forms/solid`, `solid-icons` and others (checked with
the benchmark's own `selectRow` against the registry on 2026-09-28; that list
is not checked in, the corpus file's `external` list is).

**What the ranking means.** 24 of the 30 packages are `@solid-primitives/*`.
Whole-package weekly downloads are mostly Solid 1.x installs, many of them
transitive through kobalte and corvu; the pinned Solid 2 *releases* see little
traffic (`@solid-primitives/utils@7.0.0-next.4` 3,140 a week, most others
2,500-2,900, several under 50; each row's `releaseWeeklyDownloads` is in the
corpus file). The ranking therefore measures which packages the ecosystem will
bring to Solid 2, not current Solid 2 usage. Three packages are measured on an
`only` probe because their release admits one runtime: `@kobalte/core`
(solid-js/web 2.0.0-rc.3), `@kobalte/utils` (rc.0) and `@solidjs/router`
(rc.9). `@kobalte/core`'s rc.3 matters below.

## Method

`make certification-metric` builds the release checker, runs
`scripts/ecosystem-benchmark/run.mjs --solid 2 --timeout 1800
--attempt-certification --recover-entrypoints --probe-recipe-corpus
scripts/ecosystem-benchmark/probe-recipes --keep-temp` over exactly the 30
corpus probes (the census run's flags), and then
`scripts/certification-metric.mjs --run … --clean-retained`, which reads every
row's retained output directory, writes `rust/target/certification-metric/
metric.{json,md}` and deletes the retained trees. The measurement script
certifies nothing, and no product path changed: `git diff f67a9a31 --stat`
touches only the Makefile, the new script and its test, the corpus file and
this report. The run used the release build of `f67a9a31` and the checked-in
`bin/solid-typefacts` (stamp matched), on AC power with `powermode 0` but with
concurrent work on the machine (load average 6 to 41 during the run), so the
wall is an upper bound; with warm install and registry caches the 30 probes
took 270 s and 280 s in two runs.

**An export** is an `(entrypoint, name)` pair at an entrypoint a consumer can
name (the census's `nameableEntrypoint`), taken from the generated contract, so
an entrypoint certification refused still counts. The census keys by name only,
which would collapse `@kobalte/core`'s `./accordion` `Root` and `./dialog`
`Root`. Each export gets the strongest summary its own row certified across the
entrypoint's artifact cases (the census's best-answer rule):

- **clean**: the import finds nothing open (`consumerState` value or clean): a
  non-callable value, or a callable with callbacks, reads, returns and creates
  closed;
- **partial**: something stated or closed, something open;
- **degenerate**: a summary that states and closes nothing;
- **uncertified**: in the generated surface with no accepted summary (none this
  run);
- a package with no generated surface is **not certifiable** and scores 0.

**Walls.** Each open consumer domain of each non-clean export gets one status,
read strictly from the records of the artifact case that answered, by the method
of `phase22/2026-09-23-what-holds-an-import-open.mjs` (withheld closure,
withheld operation, generator decline, never proposed, proposed but not
certified). `causeOf` maps each status to a class and a wall. An export is
*blocked* by a wall when one of its open domains is, and *solely* blocked when
all of them are. `package-weighted` gives each export 1 / its package's surface
and averages over the 30 packages: the share of an average package's exports
the wall blocks, on the same weighting as the per-package headline.
Demanded sites join the pinned 2026-09-14 SC9005 demand by (package, name); that
sweep is over Solid 1.x-era consumers, so the site column is a demand hint, not
Solid 2 demand.

## Per package

| # | package | version | weekly downloads | exports | clean | partial | degenerate | clean share | misuse-capable | lane |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | `@solid-primitives/utils` | 7.0.0-next.4 | 4,367,997 | 99 | 19 | 70 | 10 | 19.2 % | 43 | plain |
| 2 | `@solid-primitives/resize-observer` | 4.0.0-next.3 | 3,668,668 | 7 | 0 | 0 | 7 | 0 % | 0 | graph |
| 3 | `@solid-primitives/event-listener` | 3.0.0-next.5 | 3,574,044 | 11 | 0 | 5 | 6 | 0 % | 5 | graph |
| 4 | `@solid-primitives/rootless` | 2.0.0-next.2 | 3,569,023 | 8 | 0 | 7 | 1 | 0 % | 5 | graph |
| 5 | `@solid-primitives/static-store` | 1.0.0-next.2 | 3,530,235 | 3 | 0 | 1 | 2 | 0 % | 1 | graph |
| 6 | `@solid-primitives/refs` | 3.0.0-next.2 | 2,511,450 | 8 | 0 | 6 | 2 | 0 % | 6 | graph |
| 7 | `@solid-primitives/keyboard` | 2.0.0-next.5 | 2,050,154 | 7 | 0 | 0 | 7 | 0 % | 0 | graph |
| 8 | `@solid-primitives/transition-group` | 2.0.0-next.2 | 2,008,891 | 2 | 0 | 2 | 0 | 0 % | 2 | graph |
| 9 | `@solid-primitives/media` | 4.0.0-next.2 | 1,839,839 | 6 | 0 | 0 | 6 | 0 % | 0 | graph |
| 10 | `@solid-primitives/props` | 4.0.0-next.3 | 1,778,317 | 8 | 1 | 2 | 5 | 12.5 % | 1 | graph |
| 11 | `@solid-primitives/keyed` | 3.0.0-next.2 | 1,771,591 | 6 | 0 | 6 | 0 | 0 % | 6 | plain |
| 12 | `@solid-primitives/map` | 1.0.0-next.2 | 1,728,755 | 4 | 0 | 0 | 4 | 0 % | 0 | graph |
| 13 | `@solid-primitives/trigger` | 3.0.0-next.2 | 1,722,232 | 3 | 0 | 3 | 0 | 0 % | 1 | graph |
| 14 | `@kobalte/core` | 2.0.0-alpha.2 | 1,711,177 | 568 | 5 | 22 | 541 | 0.9 % | 16 | plain (graph recovery refused) |
| 15 | `@kobalte/utils` | 2.0.0-alpha.0 | 1,698,460 | 10 | 3 | 5 | 2 | 30.0 % | 4 | plain |
| 16 | `@solid-primitives/storage` | 5.0.0-next.4 | 1,370,681 | 11 | 2 | 0 | 9 | 18.2 % | 0 | graph |
| 17 | `@solidjs/router` | 2.0.0-next.30 | 632,397 | 32 | 0 | 14 | 18 | 0 % | 9 | plain |
| 18 | `@solid-primitives/scheduled` | 2.0.0-next.2 | 596,233 | 6 | 0 | 5 | 1 | 0 % | 5 | plain |
| 19 | `@solidjs/meta` | 1.0.0-next.2 | 452,643 | 8 | 0 | 0 | 8 | 0 % | 0 | plain |
| 20 | `@solid-primitives/bounds` | 1.0.0-next.2 | 403,408 | 2 | 0 | 1 | 1 | 0 % | 0 | graph |
| 21 | `@tanstack/solid-query` | 6.0.0-rc.4 | 386,889 | 52 | 11 | 18 | 23 | 21.2 % | 12 | graph |
| 22 | `@solid-primitives/event-bus` | 3.0.0-next.3 | 315,799 | 11 | 0 | 2 | 9 | 0 % | 1 | graph |
| 23 | `@solid-primitives/timer` | 1.4.5-next.1 | 279,946 | 5 | 0 | 5 | 0 | 0 % | 5 | plain |
| 24 | `@solidjs/testing-library` | 1.0.0-beta.3 | 279,615 | — | — | — | — | 0 % | — | **not certifiable** |
| 25 | `@solid-primitives/memo` | 2.0.0-next.2 | 273,171 | 7 | 0 | 5 | 2 | 0 % | 5 | graph |
| 26 | `@solid-primitives/i18n` | 3.0.0-next.4 | 246,650 | 12 | 0 | 11 | 1 | 0 % | 6 | plain |
| 27 | `@solid-primitives/scroll` | 3.0.0-next.4 | 239,172 | 6 | 0 | 0 | 6 | 0 % | 0 | graph |
| 28 | `@solid-primitives/active-element` | 3.0.0-next.2 | 218,823 | 3 | 0 | 0 | 3 | 0 % | 0 | graph |
| 29 | `@solid-primitives/websocket` | 2.0.0-next.3 | 215,211 | 10 | 0 | 7 | 3 | 0 % | 7 | plain |
| 30 | `@solid-primitives/audio` | 3.0.0-next.2 | 213,363 | 3 | 0 | 1 | 2 | 0 % | 1 | graph |

"plain" is the `reused-proposal`/`generated-proposal` lane and "graph" the
`published-graph` lane. `@solidjs/testing-library` refuses whole: its graph
node `aria-query@5.3.0` has no runtime ESM exports (`lib/index.js` is CJS), so
no artifact case can be prepared. Misuse-capable exports concentrate in
`@solid-primitives/utils` (43: 27 argument reads, 16 invokes), `@kobalte/core`
(16) and `@tanstack/solid-query` (12).

## Walls

### By class

| class | exports blocked | solely blocked | package-weighted | packages | demanded sites |
| --- | ---: | ---: | ---: | ---: | ---: |
| unaccepted dependency | 533 | 100 | 6.6 % | 3 | 18 |
| dialect-silent | 406 | 0 | 10.1 % | 8 | 21 |
| graph lane: unrecorded | 148 | 86 | **61.6 %** | 19 | 53 |
| declined (generator) | 142 | 7 | 16.9 % | 9 | 12 |
| missing claim form | 133 | 24 | 15.4 % | 12 | **363** |
| census refusal | 113 | 1 | 17.6 % | 18 | 168 |
| withheld operation | 86 | 1 | 20.4 % | 16 | 67 |
| recipe (no recipe, or veto incomplete) | 74 | 0 | 27.1 % | 15 | 18 |
| no record (proposed, not certified) | 30 | 2 | 6.1 % | 7 | 55 |
| attribution catch-all | 21 | 0 | 6.1 % | 9 | 48 |

The pooled column is dominated by `@kobalte/core` (563 of the 877 non-clean
exports); the package-weighted column is what the per-package headline sees.

### Greedy combination (upper bound)

Adding one class at a time, the class that leaves the most non-clean exports
with no remaining cause:

| step | clear | non-clean exports with no remaining cause (of 877) |
| ---: | --- | ---: |
| 1 | unaccepted dependency | 100 |
| 2 | + dialect-silent | 473 |
| 3 | + graph lane: unrecorded | 559 |
| 4 | + declined | 626 |
| 5 | + missing claim form | 658 |
| 6 | + census refusal | 717 |
| 7 | + withheld operation | 767 |
| 8 | + recipe | 828 |

These are upper bounds. A cleared cause can uncover the one it was masking: a
recipe lets its census run, an accepted dependency exposes the dependency's own
open claims, a retained graph record names a cause that is already in this
table. Steps 1-2 are almost entirely `@kobalte/core`.

### The walls themselves

| class | wall | exports | solely | package-weighted | demanded sites | packages |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| graph lane: unrecorded | returns | 130 | 0 | 55.7 % | 50 | 19 |
| graph lane: unrecorded | creates | 135 | 0 | 55.3 % | 16 | 19 |
| graph lane: unrecorded | callbacks | 103 | 0 | 46.4 % | 50 | 19 |
| graph lane: unrecorded | reads | 103 | 3 | 36.3 % | 0 | 15 |
| recipe | no probe recipe (reads) | 72 | 0 | 27.0 % | 18 | 14 |
| withheld operation | operation census refused: recursive-value-shape | 80 | 1 | 19.4 % | 61 | 15 |
| declined | runtime-accessor-installation | 64 | 7 | 9.9 % | 0 | 4 |
| census refusal | uncensused invoking form: property-access-unknown-accessor | 46 | 0 | 8.9 % | 25 | 13 |
| missing claim form | callbacks never proposed | 42 | 0 | 6.6 % | 103 | 9 |
| missing claim form | returns never proposed | 100 | 1 | 6.4 % | **288** | 8 |
| declined | unresolved-callee | 81 | 0 | 6.0 % | 4 | 8 |
| unaccepted dependency | `@solid-primitives/utils` | 246 | 45 | 4.7 % | 10 | 2 |
| missing claim form | callbacks: invokes a caller-supplied callable | 22 | 0 | 4.4 % | 93 | 7 |
| dialect-silent | `solid-js:createMemo` | 21 | 0 | 3.0 % | 0 | 4 |
| missing claim form | creates never proposed | 69 | 0 | 2.7 % | 144 | 4 |
| dialect-silent | `solid-js:createSignal` | 66 | 0 | 2.7 % | 20 | 6 |
| attribution catch-all | callee in `@solidjs/signals` (`omit`, from `@solidjs/meta` and `@kobalte/core`) | 8 | 0 | 2.5 % | 2 | 2 |
| attribution catch-all | dialect row scoped to the browser host (`createSignal`) | 4 | 0 | 2.4 % | 0 | 4 |
| dialect-silent | `@solidjs/web:merge` | 213 | 0 | 1.3 % | 0 | 1 |
| unaccepted dependency | `@kobalte/utils` | 187 | 28 | 0.6 % | 2 | 1 |
| dialect-silent | `solid-js:omit` | 80 | 0 | 0.4 % | 0 | 1 |

`metric.md` lists the first 40 walls and the obligation locations of every
attribution catch-all; `metric.json` carries each export's causes.

Two graph-preparation refusals that no wall row shows: `@solid-primitives/keyed`
imports `@solid-primitives/utils` at runtime but declares it only as a
devDependency, so no graph can be prepared ("`@solid-primitives/utils` is not
installed above …"): a publishing defect the checker is right to refuse.
`@solidjs/router`'s peer `filesystem-routing` is not installed by the benchmark's
install, so its graph lane refuses the same way: a harness install gap.

## The top five walls and what fixing them costs

**1. The graph lane keeps no per-node record** — 148 exports, 19 packages,
61.6 % package-weighted, 86 exports blocked by nothing else. The lane
regenerates each node's proposal in private scratch and retains only withheld
closures and withheld operations, so a domain the node never proposed (or the
generator declined) leaves no trace in `run.json` or the audit. Substituting
the plain lane's record (`metric.md`, "read from the plain lane") explains
almost nothing: for 77 of the 148 exports the plain lane stopped only at the
unaccepted dependency the graph lane went on to accept, for 41 (all
`@tanstack/solid-query`) it recorded nothing, and only 30 carry a plain-lane
cause that could survive acceptance (dialect silence 24, a decline 6). The
graph lane's roots also state little: 6 of its 19 packages publish a root in
which every export is degenerate (`resize-observer`, `keyboard`, `media`, `map`,
`scroll`, `active-element`), and `storage` and `event-bus` are 9 of 11
degenerate. **Fix size:
tooling.** Retain the root node's `declinedClosures` and `unresolvedClaims` in
the certification audit (output only; no semantics, no ADR). It unlocks no
export by itself; it is what makes the metric's largest per-package wall
rankable, and it should come first for that reason.

**2. `@kobalte/core`'s graph recovery refuses on a missing request field** —
the unaccepted-dependency class, 533 exports (525 in `@kobalte/core`). Its
entrypoint recovery is requested with retained proposal roots, and every native
transaction refuses at witness acquisition with
`solid-checker-rust: missing field \`lockfile\``. The cause is in
`packages/cli/scripts/retained-proposal-graphs.mjs:63`, which still builds the
root node as `{ key, bunLockPath: lockfile, lockLocator }`: `a1926b71` (ADR 0108,
2026-09-14) renamed the key the request builder reads to `lockfilePath`
(`certify-contract.mjs` `graphNodeExecutionInput`), so the serialized node has no
`lockfile` and `ContractCertificationGraphNodeRequest` (`main.rs`,
`deny_unknown_fields`, `lockfile: String`) rejects it.
`test/retained-proposal-graphs.test.mjs:56` asserts the old key, which is why
nothing failed. It is the only corpus row that reaches this path, here and in
the pinned 2026-09-14 benchmark report. **Fix size: one line plus the test**
(not made here: this task changes no product path). What it unlocks was
measured with the line changed locally and reverted; see
"The `lockfile` experiment" below.

**3. Dialect silence on `creates`** — 406 exports in 8 packages (385 in
`@kobalte/core`). Four distinct gaps, none a checker bug:

- `merge` (213 exports with the `@solidjs/web` spelling, 20 with `solid-js`'s)
  and `omit` (80) have **no** negative `creates` row at any audited version in
  `rust/crates/solid-dialect/src/solid_2.rs`;
- `createSignal` (66): its rows are `RowScope::HostTarget(Browser)`, and a
  package certification requests `[import]`, which does not name the browser
  host; the same scoping surfaces as the four attribution catch-alls that end
  "the dialect row … is scoped to the `browser` host target";
- `createMemo` (21): rows exist for the `@solidjs/signals` spelling; the
  `solid-js` re-export the packages import is not covered.

**Fix size:** dialect audit work for `merge`/`omit` (new cited negative rows,
rc.3 and rc.9), plus **one ADR** on which host target a package certification
assumes, which is also what the `createSignal` rows need. `@kobalte/core` is
certified on rc.3 because its release admits nothing else, so its rows have to
exist at rc.3 to help it.

**4. Missing claim forms** — 133 exports in 12 packages, and the most demand of
any class (363 demanded sites): `returns` never proposed (100 exports, 288
sites), `creates` never proposed (69, 144), `callbacks` never proposed (42,
103), and `callbacks` withheld because the export invokes a callable its caller
supplied (22, 93: the candidate says "no invocation", the census finds a
parameter-rooted call, and no invoke claim is proposed). **Fix size: one ADR
per form**, each the size of ADR 0113 (plain return) or ADR 0115 (argument
containers): a returned function, a structured output, and an invocation claim
for a parameter-invoking helper.

**5. Withdrawn recursive-shape operations** — 86 withheld-operation exports,
80 of them `operation census refused: recursive-value-shape` (15 packages,
19.4 % package-weighted): the primitive returns census refuses an
implementation whose value shape is recursive. **Fix size: a census rule (ADR).**

Next by package weight: `runtime-accessor-installation` declines (64 exports in
4 packages, 30 in `@solidjs/router`): the facts layer records a module hazard
where a module installs accessors (`Object.defineProperty` and its relatives),
and in `@kobalte/core` that module is the bundler's `dist/rolldown-runtime`
chunk every component reaches. A rule that recognises a bundler's export helper
is an ADR. Then the census refusals
(`property-access-unknown-accessor` 46 + 11, `domain-exhaustiveness` 11 with 74
demanded sites).

## The `lockfile` experiment

To size wall 2, `retained-proposal-graphs.mjs:63` was changed locally to
`lockfilePath: lockfile`, the `@kobalte/core` probe was certified alone with the
harness's flags, and the line was reverted before anything else ran (the
committed tree does not carry it; `git status` shows no change under
`packages/`).

| `@kobalte/core@2.0.0-alpha.2` (rc.3) | as shipped | key fixed locally |
| --- | ---: | ---: |
| lane | generated-proposal (recovery refused) | **published-graph** |
| artifact cases published | 132 of 136 | **136 of 136** |
| exports in the surface | 568 | 607 |
| clean / partial / degenerate | 5 / 22 / 541 | 7 / 41 / 559 |
| clean share | 0.9 % | 1.2 % |
| misuse-capable exports | 16 | **32** (invoke 11, argument read 10, returned accessor 8, owner 6) |
| main blocking class | unaccepted dependency (525), dialect-silent (385) | graph lane: unrecorded (593, 531 solely) |
| probe wall | 217 s (in the 30-probe run) | 153 s (alone) |

The surface grows by 39 exports because the four cases the plain lane did not
publish now are. The fix doubles what a misuse rule could consume in the
package and moves the per-package headline by about 0.01 points (*estimated*
from one package in 30). Every answer the graph lane adds is again behind
wall 1, which is the reason to fix wall 1 first: after it, this package's
walls become rankable at all.

## Would a recipe generator pay off?

**Counts (measured):** 1,046 closure claims withheld as `no recipe in corpus`
and 130 whose veto did not complete, across every certified node of the 30
rows. The 130 are harness failures, not missing recipes: 75 `Stripping types is
currently unsupported` (Node refuses to strip types for a module the probe
loads), 31 `synthesized primitive-return veto: no sample`, 21 `no jsx-free
premise covers …`, 3 `synthesized identity veto: no sample`. In the cases that
answer the corpus's own exports, recipe causes block 74 exports in 15 packages
(`reads` 72 of them).
**No export is blocked by recipes alone** (0 solely blocked): every one of the
74 also has another open domain, blocked by the graph lane's missing records
(37), a withdrawn operation (32), a census refusal (20), a missing claim form
(19), a generator decline (11), dialect silence (9) or the catch-all (9).

A `no recipe` reason says the claim was weakened out *before* its census ran,
so a recipe first unmasks the census refusal underneath; on 2026-09-18 the
two-pass scaffold found 70 of 914 withheld candidates serviceable (7.7 %). A
generator that writes a recipe per withheld claim and uses the probe run as its
check would automate that two-pass procedure and turn the 1,046 into either
certified closures or named census refusals. *Estimated* yield today: at most
the 7.7 % serviceable share of 1,046, about 80 closures, clearing no export on
its own until walls 1, 4 and 5 move. **Recommendation:** worth building as the
two-pass *diagnostic* (it names the walls underneath), not as a coverage lever;
schedule it after walls 1-4.

## Should project-side certification be the default delivery path?

Yes, with the tier as a cache rather than the path. The evidence:

- the compiled-in tier matched **0** dependency environments across 151 real
  Solid 2 consumer projects (2026-09-26), and the rc.9 campaign's regenerated
  tier reached two packages in two consumers, both through reviewed consumer
  environments (2026-09-27);
- project-side certification on kobalte `packages/core` took acceptance-gate
  sites from 131 to 85 (55 to 9 without the test-only testing library) in about
  80 s end to end (2026-09-26);
- this metric certifies each package once, in the benchmark's own install. A
  consumer runs its own versions: `@kobalte/core` is measured on rc.3 because
  its release admits nothing else, and the consumers swept on 2026-09-27 run
  rc.3 and rc.9.

What the metric adds is the ceiling: the delivery path can only hand a consumer
what certification states, and today that is 3.4 % of an average package's
exports. Delivery and capability are separate levers; the order below works on
capability, and project-side certification is the default way to deliver
whatever it reaches.

## Recommended order of work

1. **Retain the graph node's declines and unresolved claims** in the
   certification audit (tooling, wall 1). It makes the 19 graph-lane packages'
   walls rankable and changes no answer. Re-run `make certification-metric`
   afterwards: the walls below are ranked without those 19 packages' causes.
2. **Fix the retained-proposal `lockfile` key** (one line and its test, wall 2).
   Measured: `@kobalte/core` publishes all 136 cases on the graph lane and its
   misuse-capable exports go from 16 to 32; its clean share barely moves until
   step 1 shows what the graph lane leaves open. Cheap enough to do alongside
   step 1.
3. **Dialect `creates` rows for `merge`/`omit`, and an ADR on the certification
   host target** (wall 3). The greedy curve's first two steps are wall 2's
   class and this one, almost all in `@kobalte/core`; the experiment shows that
   clearing wall 2 moves those exports into wall 1 rather than clearing them, so
   the curve's bound is loose there.
4. **The missing claim forms** (wall 4): returned functions and structured
   outputs, then an invocation claim for parameter-invoking helpers. These
   carry the most consumer demand of any class.
5. **The recursive-value-shape census** (wall 5), then the bundler
   export-helper hazard.
6. **The recipe generator as an automated two-pass**, after 1-4. Before it,
   the 130 veto failures are harness work, not recipe work: 75 are Node's
   `Stripping types is currently unsupported`.
7. Separately, the corpus: add `solid-use` through a family (a deliberate
   manifest change) and install declared peers in the benchmark
   (`filesystem-routing`).

The lead reruns this metric after ADR 0139 lands (`make certification-metric`,
about five minutes). This report does not predict which walls it moves; compare
the class table and the per-package buckets, not only the headline, because a
fix that moves exports between walls leaves the headline flat, as the
`lockfile` experiment did.
