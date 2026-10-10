# Consumer demand for contracts, measured — and nine closures scoped by it (2026-09-12)

- **Status:** measurement plus one demand-scoped recipe set and one reporting
  fix. No analyzer, generator or certifier code changed.
- **Question:** the withheld-closure ledger is 82.6% `noRecipe`
  (§ 81 of the reads-veto design), so the frontier is recipe authoring. Which
  recipes are worth writing? The corpus totals cannot say: a closure matters only
  where a *consumer* imports that export and a rule raises an obligation on it.
- **Answer:** demand is measurable today with no new instrument, it is
  concentrated in two utility packages, and the first demand-scoped recipe set
  closes `reads` on nine `@solid-primitives/utils@6.4.1` exports that thirty
  consumer projects import.

## 1. The instrument is SC9005 itself

With no accepted catalog, ordinary analysis raises `SC9005` at every import of a
package whose manifest uses Solid and that has no contract
(`package_requirements.rs`, `external_package_contract_requirements`). The
message names the module and the export:

~~~
the reactivity contract for @kobalte/utils has no entrypoint/export summary for
imported export mergeDefaultProps; …
~~~

Run the checker over real consumer projects, keep those findings, aggregate by
(module, export), and the result is the demand: which exports consumers actually
reach through a contract-requiring import. Once a package *has* a certified
contract the same finding narrows to the open domains
(`unknown-contract-claims:reactiveReads,returns,ownerRequirements`), so the same
sweep measures the residue after certification.

[`2026-09-12-consumer-demand-measurement.py`](2026-09-12-consumer-demand-measurement.py)
does the sweep and the cross-reference against the pinned ecosystem report. It
runs the checker and reads JSON; it certifies nothing.

## 2. The corpus: 118 real projects

Shallow clones of five upstream repositories, dependencies installed with their
own lockfiles (`pnpm install --frozen-lockfile --ignore-scripts`), every
`tsconfig.json` under them as one project:

| repository | projects | what it consumes |
| --- | ---: | --- |
| `solidjs-community/solid-primitives` | 70 | `@solid-primitives/utils`, `event-listener`, `rootless`, `static-store` |
| `corvudev/corvu` | 21 | `@corvu/utils`, `@corvu/dialog`, `solid-presence`, `solid-prevent-scroll` |
| `kobaltedev/kobalte` | 5 | `@kobalte/utils`, `@kobalte/core`, `@solid-primitives/props` |
| `solidjs/solid-docs` | 2 | `@kobalte/core`, `@kobalte/solidbase`, `@solidjs/router`, `@solidjs/start` |
| `corvudev/corvu` `web`, `kobaltedev/kobalte` `apps/docs` | (in the above) | applications, not libraries |

Debug binary, one process per project, 118 projects in under three minutes.
Status over the sweep: 83 `uncertifiable`, 25 `violation`, 10 `certified`.

## 3. What is demanded

| | |
| --- | ---: |
| distinct (module, export) pairs | 233 |
| SC9005 call sites naming an export | 2,056 |

By package, call sites:

| package | sites | projects |
| --- | ---: | ---: |
| `@kobalte/utils` | 942 | 2 |
| `@solid-primitives/utils` | 419 | up to 30 per export |
| `@solidjs/testing-library` | 133 | 2 |
| `@solid-primitives/props` | 78 | 2 |
| `@kobalte/solidbase` | 60 | 2 |
| `solid-heroicons` | 58 | 2 |
| `@tanstack/solid-router` | 40 | 3 |
| `@solidjs/router` | 38 | 3 |
| `@solid-primitives/event-listener` | 38 | 9 |

Two readings matter more than the totals:

- **`@kobalte/utils` is one consumer.** 942 sites are `@kobalte/core` and its
  docs importing their own sibling package. High volume, narrow reach.
- **`@solid-primitives/utils` is the hub.** `access` is imported by 30
  projects, `noop` by 26, `asArray` by 18, `tryOnCleanup` by 15, `trueFn` by 14,
  `createMicrotask` by 14, `entries`/`keys`/`createCallbackStack` by 13 each. An
  open domain here is felt across the whole `@solid-primitives` family.

Demanded packages **absent from the ecosystem corpus** (so nothing can close
there until they are rows): `@solid-primitives/props` (78 sites),
`@solid-primitives/event-listener` (38), `@solidjs/router` (38),
`@solid-primitives/static-store` (25), `@solid-primitives/resize-observer` (10),
`@solid-primitives/media` (7), `solid-presence` (26), `solid-prevent-scroll` (10).
Build-tooling imports (`esbuild-plugin-solid`, `vite-plugin-solid`,
`@solidjs/testing-library`) are demand the analyzer raises on config and test
files and are not certification targets.

## 4. Demand crossed with the pin, per export

For every demanded export of a corpus package, the pinned report says which
domains certify and what blocks the rest. The top of the queue
(`sites / projects / closed / open → blocker`):

| export | sites | proj | closed | open → blocker |
| --- | ---: | ---: | --- | --- |
| `@kobalte/utils` `mergeDefaultProps` | 254 | 2 | – | `reads` noRecipe |
| `@kobalte/utils` `mergeRefs` | 164 | 2 | – | no ledger entry |
| `@kobalte/utils` `callHandler` | 112 | 2 | – | `creates` census (call through `handler[0](…)`), `reads` noRecipe |
| `@solid-primitives/utils` `access` | 74 | 30 | creates, reads | – (7.0 case); 6.4.1 `reads` noRecipe |
| `@kobalte/utils` `createGenerateId` | 60 | 2 | – | `creates` veto did not complete |
| `@solid-primitives/utils` `noop` | 50 | 26 | creates | `reads` noRecipe |
| `@solid-primitives/utils` `asArray` | 36 | 18 | creates | `reads` noRecipe |
| `@solid-primitives/utils` `tryOnCleanup` | 19 | 15 | – | `reads` noRecipe |
| `@solid-primitives/utils` `trueFn` | 19 | 14 | creates, reads | 6.4.1 `reads` noRecipe |
| `@solid-primitives/utils` `createMicrotask` | 15 | 14 | – | `creates` census iteration-protocol, `reads` noRecipe |
| `@solid-primitives/utils` `createCallbackStack` | 13 | 13 | – | `creates` census (call through `cb`), `reads` noRecipe |
| `@solid-primitives/utils` `accessWith` | 12 | 11 | creates | `reads` noRecipe |
| `@solid-primitives/utils` `isObject` | 10 | 10 | creates | `reads` noRecipe |

Two facts fall out. First, for the hub package the blocker is uniformly a
missing `reads` recipe — the exact kind of work § 81 said the frontier is made
of. Second, `@kobalte/utils` shows a different blocker on eleven exports,
`veto did not complete: gate …`, which is a harness or recipe-run failure rather
than a missing recipe, and is a separate investigation.

## 5. Nine recipes, scoped by that queue

`@solid-primitives/utils@6.4.1` was certified standalone (release binary, the
scratch project's Bun lockfile integrity, published `.` case
`artifact-case:9887e137…`), the audit scaffolded with
`scripts/probe-recipe-scaffold.mjs --domain reads`, and the scaffold run a
second time against the throwing corpus as its README prescribes:

| | count |
| --- | ---: |
| `reads` candidates withheld `no recipe in corpus` | 40 |
| unserviceable on the second pass (census refused) | 20 |
| serviceable | 20 |
| serviceable **and** demanded by a consumer | 9 |

The twenty unserviceable ones split three ways, and none is recipe work:

- **`domain-exhaustiveness … callSignatureNotUnique`** (`entries`, `keys`,
  `tryOnCleanup`, `isServer`, `isDev`, `isProd`, `isClient`, `defaultEquals`,
  `EQUALS_FALSE_OPTIONS`, `INTERNAL_OPTIONS`): the export's runtime transcript is
  open because its call signature is not unique — `entries` is `Object.entries`
  by alias, `tryOnCleanup` is a conditional alias of `onCleanup`. A producer
  premise, not an observation.
- **"a reads closure candidate must enumerate no operation, but the proposal
  names N"** (`arrayEquals`, `accessArray`, `filterNonNullable`,
  `handleDiffArray`, `lines`, `ndjson`): the generator proposed a *non-empty*
  reads enumeration, and the census decides empty enumerations only.
- **`reads-census premise required`** at an accessor or iteration form
  (`createHydratableSignal`, `createHydrateSignal`, `createMicrotask`, `defer`):
  the same census legs §§ 57–63 of the reads-veto design already rank.

The nine written — `access`, `accessWith`, `asAccessor`, `asArray`, `chain`,
`createCallbackStack`, `isObject`, `noop`, `trueFn` — follow the
`solid-primitives-utils-access-reads-2bf41ff6.mjs` discipline: finite samples
that assert the export's answers, an owned getter that proves the observation
apparatus live on every run, and no emit, because every read reachable through
these exports is the caller's under ADR 0034. Each carries `NEVER EMITS:` and is
pinned by name in `scripts/ecosystem-probe-recipes.test.mjs`.

Measured by certifying the same case against the checked-in corpus:

| | before | after |
| --- | ---: | ---: |
| certified closure entries | 27 | 28 |
| `reads` closed | 0 | **9** |
| withheld | 59 | 50 |
| vetoes that threw or contradicted | 0 | 0 |

The pin in `benchmarks/ecosystem/report.json` is not refreshed here; that is a
deliberate `make ecosystem-benchmark`.

## 6. Two "silences" examined, one fixed, one a pinned decision

**`@tanstack/charts` certifying 1 of 113 entrypoints with no refusal** is not a
generator gap: run directly, the generator emits 112 of the 113 subpaths. The
row's probe deliberately asks for `./solid` alone (`manifest.json`,
`probes[].entrypoints`), and the coverage denominator did not know that, so the
row read `partial 1 of 113 (no root)` for ever. `lib/certified-coverage.mjs` now
carries the probe's request when there is one (`requestedEntrypoints`,
`requestedCertified`, `rootRequested`) and measures completeness against it, the
root being required exactly when it was requested; the report prints
`complete 1 of 1 requested (113 declared, no root)`. A probe with no list
produces the unchanged shape. Pinned in `run.test.mjs`. Three probes in the
manifest are scoped this way.

**`accepted dependency solid-js has no exact runtime binding for export X`**
(74 of the 113 such refusal strings in the pin, over 35 rows) is the collision of
two rules: `solid-js`, `@solidjs/signals` and `@solidjs/web` are
`CORE_RUNTIME_PACKAGES` with no contract by design, and `bindExport` requires an
accepted binding for any external re-export. It is **not** a silence and not a
campaign gap — it is the decision `fixtures/package-contracts/solid-reexport`
pins: "`solid-js` gets no exemption", because binding the re-export by name
would let any package republish Solid's semantics by spelling. Changing it is
an ADR, not a fix, and this measurement leaves it where it is. The non-core
cases (`@tanstack/router-core`, `motion-utils`, `@corvu/dialog`) die one step
earlier, at the planner's `csstype/index.js is not a file` leaf (91 leaves over
50 rows): a types-only package reached from a non-declaration `.ts` file
becomes a runtime node the graph cannot resolve.

## 7. What this says about where to spend effort

- The hub is `@solid-primitives/utils`, and its remaining `reads` frontier on the
  `.` case is now the eleven serviceable-but-undemanded exports plus the twenty
  census refusals above. The next recipe set with reach is not there; it is the
  demanded packages that are not corpus rows yet (§ 3).
- `@kobalte/utils`'s eleven `veto did not complete` exports are the cheapest
  unexplained blocker on the board and precede any recipe work on that package.
- The `callSignatureNotUnique` refusal on aliases of engine functions
  (`entries = Object.entries`) is a producer premise worth sizing across the
  corpus before it is written; this case alone holds ten claims.

## 8. `@kobalte/utils`: the `veto did not complete` exports, examined

§ 4 flagged eleven `@kobalte/utils` exports withheld as `veto did not complete:
gate …`. Read one level down, every one of the 63 such entries in the pin (51 on
`0.9.2`, 12 on `2.0.0-alpha.0`) carries the same worker error:

~~~
Stripping types is currently unsupported for files under node_modules, for
"…/node_modules/@kobalte/utils/src/array.ts"
~~~

They are all `./src/*.ts` **source** artifact cases — Kobalte publishes its
TypeScript alongside `dist/`, and the pinned Node will not strip types under
`node_modules`. That is exactly the disposition [ADR 0009](../../adr/0009-typescript-source-probe-disposition.md)
accepted on 2026-09-04: keep the refusal, substitute no transpiled or sibling
case, at the measured cost of "40 candidates unprobeable". Nothing here is a
harness defect, and nothing is to be built.

The case a consumer actually imports is the JavaScript root, and it is in
better shape than the row suggests:

- The four `Key` refusals in the row's `artifactCaseRefusals` are the
  generation-stage census. The published-graph lane supersedes them: the pin
  already records `rootCertified: true` over 22 entrypoints, and a standalone
  graph-lane certification of `.` on the fresh release binary prepares 78 nodes,
  refuses nothing, and issues in 39 s.
- What the root document *says* is the problem. Of its 59 exports, 35 publish
  `{"call": {}, "shape": "callable"}` — no claim in any domain — and they
  include `mergeDefaultProps`, `callHandler`, `composeEventHandlers`,
  `createGenerateId`, `mergeRefs`, `access`. The 20 that close `creates` are
  pure helpers (`clamp`, `contains`, `isFunction`, …).

Per demanded export, the blocker:

| export | sites | blocker |
| --- | ---: | --- |
| `mergeDefaultProps` | 254 | `dialect-silent` on `solid-js` `mergeProps` |
| `mergeRefs`, `access`, `accessWith`, `chain` | 164 + | re-exports of `@solid-primitives/*`, composed through the graph lane |
| `callHandler`, `composeEventHandlers` | 160 | `handler[0](handler[1], event)`: a call through a caller-supplied callable, which the `callbacks` domain owns by design |
| `createGenerateId`, `focusWithoutScrolling`, … | 60 + | source-case vetoes only (ADR 0009); the JS case declines `unresolved-callee` (`some` on an unresolved receiver) or `refusing-callee-fixpoint` |

### 8.1 The finding underneath: Solid 1.x has no admissible `creates` audit

`dialect-silent` on `mergeProps` is not a missing vocabulary row —
`solid_1x.rs` knows the name. It is that the 1.x dialect's negative authority is
empty by decision: its former summary table was withdrawn as inadmissible under
ADR 0005 (shape errors, closures manufactured from missing knowledge), and
"until then the census refuses every 1.x callee by name". So on Solid 1.x,
`creates` can close only for an export that calls **no** Solid primitive at all.

Measured over the pin's `contractContent.declinedClosuresByKind` and
`dialectSilentBlockers`:

| | solid1 | solid2 |
| --- | ---: | ---: |
| rows | 168 | 250 |
| rows with a dialect-silent decline | 68 | 85 |
| dialect-silent declines | 5,721 | 3,153 |
| top spellings (blocked exports) | `useContext` 405, `splitProps` 245, `createEffect` 198, `mergeProps` 157, `on` 98, `onMount` 69, `createRenderEffect` 50 | `useContext` 258, `createEffect` 155, `merge` 110, `omit` 103, `runWithOwner` 53 |

Seven 1.x spellings account for over 1,200 blocked exports, and every solid1
consumer in § 2 (all of corvu, kobalte, and the 1.x half of solid-primitives)
sits behind them. That is the single largest lever this measurement found, and
it is not a premise, a recipe, or a harness fix: it is the hand audit of
`solid-js@1.9.14`'s primitives that `docs/precision-backlog.md` already lists
as an open item against the audit rather than the verifier. Its sizing is the
table above.

**Done on 2026-09-12/13:**
[`audits/2026-09-12-solid-1x-1.9.14-core-primitives-creates.md`](../audits/2026-09-12-solid-1x-1.9.14-core-primitives-creates.md)
reads those seven spellings plus `batch`, `createComputed`, `createMemo` and
`createSignal` out of all six published `solid-js@1.9.14` bundles and lands
eleven `creates` rows in `solid_1x.rs`. Its measured effect, and the reason it
is smaller than this table predicts, is § 12 of that document.
Five further rows (`createContext`, `getOwner`, `mapArray`, `onCleanup`,
`untrack`) followed on 2026-09-13 — §§ 13–17 there, +44 closure entries.

Of the two remaining kobalte blockers, `callHandler`'s is by design
(`semantic-model.md` gives the caller's callable to `callbacks`), and the
re-exports are the graph lane's to compose. The standalone consumer probe of
this catalog was inconclusive for a mechanical reason worth recording: the
receipt binds the `solid` export-condition branch the certifier selected, and a
consumer resolving through `import` reports "no receipt-accepted contract matches
this exact import" — exact-case selection working, not a defect; certify with
`--conditions` matching the consumer to measure the residue.

## 9. The remaining `@solid-primitives/utils@6.4.1` candidates, and a harness lever that is not one (2026-09-13)

With the 1.x audit landed, the withheld ledger of the 418-row corpus stands at
16,149 entries: 11,176 `no recipe in corpus`, 4,621 `census refused`, the rest
vetoes that did not complete. Of the `reads` half of the recipe gap (8,199
entries), 7,467 sit on dependency nodes shared by many rows, so one recipe on a
hub case closes every dependent row's entry — forty-two per recipe on the `.`
case of `@solid-primitives/utils@6.4.1`, against one per recipe for 654 of the
1,439 distinct recipe targets, which are root exports nobody depends on.

**The second scaffold pass over the thirty-one remaining `reads` candidates**
(`scripts/probe-recipe-scaffold.mjs`, scratch corpus, one certification, then
the script again against that audit) split them exactly:

| census verdict | candidates | exports |
| --- | ---: | --- |
| `domain-exhaustiveness` | 10 | `EQUALS_FALSE_OPTIONS`, `INTERNAL_OPTIONS`, `defaultEquals`, `entries`, `isClient`, `isDev`, `isProd`, `isServer`, `keys`, `tryOnCleanup` |
| proposal names an operation | 6 | `accessArray`, `arrayEquals`, `filterNonNullable`, `handleDiffArray`, `lines`, `ndjson` |
| accessor or iteration leg | 4 | `createHydratableSignal`, `createHydrateSignal`, `createMicrotask`, `defer` |
| decidable, recipe missing | 11 | `clamp`, `compare`, `falseFn`, `isNonNullable`, `json`, `number`, `ofClass`, `pipe`, `reverseChain`, `safe`, `withAccess` |

The three most-demanded exports left (`tryOnCleanup` 19 sites, `entries` 18,
`keys` 13) are all in the first row: `entries = Object.entries` and
`tryOnCleanup = isDev ? … : onCleanup` are aliases the census cannot make
exhaustive, and no recipe changes that. The eleven decidable ones were
written (five demanded, six not; each header says which — the six are worth
it because of the forty-two-row multiplier). Certifying the case against the
extended corpus: 44 → 55 closed domains, 50 → 40 withheld, nothing lost, no
veto threw. The case is now closed for `reads` on every candidate the census
can decide; the twenty others are the census's frontier, not a recipe's.

Over the 418-row corpus (release binary, 1200 s row timeout) against the
same tree with the nine-recipe corpus: `no recipe in corpus` 11,176 → 10,714
(exactly 11 × 42), certified closure entries 6,636 → 6,678, uncapped `reads`
closures 52 → 96, no row below baseline, Solid 2 unchanged to the entry.

**A harness lever that turned out to be ADR 0009.** 139 entries (112
`callbacks`, 27 `returns`, across the four `@tanstack/solid-query*` rows) are
withheld as `synthesized veto cannot run … export condition
"@tanstack/custom-condition" is not a plain condition name`, from
`plain_condition_name` in `probe_harness.rs`. Widening the charset would move
them, not close them: the artifact case that condition selects is
`src/index.ts` in every one of those packages (TanStack publishes the
condition for its own workspace source), so the pinned interpreter would refuse
it next as `Stripping types is currently unsupported` — the retained refusal of
ADR 0009. Left as is; the refusal text is accurate about *why* the veto cannot
run and the witness that certifies the row is the `import` case.

## 10. Two more hubs, and what the recipe-less `callbacks` candidates are (2026-09-13)

**`@solid-primitives/utils@7.0.0-next.4`.** The corpus certifies its one
`dist/index.js` under three `.` artifact cases — `2bf41ff6…` on the four
solid-js@2.0.0-rc.0 rows, `f81b5488…` and `6cd714eb…` on rc.3 rows — and a claim
id is case-bound, so a recipe has to exist per case: sixty-two entries over
twenty-two exports (the twenty 6.4.1 already covers, same code compiled
differently, plus `afterPaint` and `createIdGenerator`), each case with its own
copy of the module because the private workspace writes one file per entry.
The second scaffold pass refused twenty-three of forty-five. The claim ids came
straight out of the ecosystem report's `withheldClosureDetails`, so no
reproduction of each row's dependency closure was needed; a standalone smoke
certification (its own fourth case) closed all twenty-two with no veto thrown.

**`@floating-ui/utils@0.2.12`.** No Solid dependency, so one case (`9bc68a12…`)
serves the corpus and the standalone run alike. Twelve of twenty-four
candidates decidable; twelve recipes; 17 → 29 closed domains standalone.

**`@corvu/utils@0.4.2`** was sized and left: its eighteen exports appear under
eighteen artifact cases across four rows, so each recipe would need up to
eighteen copies for at most four entries each. Case fragmentation, not census
decidability, is the ceiling there; a corpus format that lets one module serve
several claim ids would lift it, and the loader already keys entries by claim
id alone — only the corpus test's one-file-per-entry rule and the private
workspace's copy step would have to follow.

**The 2,977 recipe-less `callbacks` candidates.** `synthesize` (ADR 0036)
builds a veto only for a candidate whose export stated a call signature
(`evidence.call_signatures`), so a candidate with none is left as `no recipe in
corpus`. On all three hubs certified standalone the recipe-less `callbacks`
candidates are exactly the non-callable exports — `EQUALS_FALSE_OPTIONS`,
`INTERNAL_OPTIONS`, `isClient`, `isDev`, `isProd`, `isServer` on both utils
versions; `alignments`, `placements`, `sides` on floating-ui — and on every one
of them the `reads` census refuses the same export as
`domain-exhaustiveness … callSignatureNotUnique`. So a veto would not have
closed them either: a value export has no invocation to sample and the census
has no call walk to run over it. Closing `callbacks: []` and `reads: []` for a
data-only value export is a small premise of its own — the producer already
states "data-only literal" for parameter defaults (ADR 0090) — and it is the
whole of that 2,977, not a recipe gap.

**Pinned.** The corpus report was regenerated with all of the above
(`make ecosystem-benchmark`, 1200 s row timeout, 854 s wall): 6,722 → 6,790
certified closure entries, uncapped `reads` closures 96 → 256, `no recipe in
corpus` 10,714 → 10,166, row statuses unchanged, no row below the previous run.

## 11. The `callbacks` refusals were the definition; the lever sat one step earlier (2026-09-13)

§ 10 named the recipe-less `callbacks` candidates and ADR 0099 closed them.
What remained was 1,155 refusals reading "enumerates no invocation, but the
implementation census dispositioned N call(s) into the parameter-rooted
family", and the refusal text now carries the member split: accessor 1,266
sites, direct call 710, iteration and element 540, coercion 127, `hasInstance`
62; 154 entries are direct calls and nothing else. Every one is a correct
verdict — `semantic-model.md` § callbacks lists the getter, the iteration
protocol and the coercion as invocations — so narrowing the family was never a
lever, and describing those items needs timing the walk does not derive.

The population the walk *does* derive an item for never reached the ledger:
exports whose interprocedural summary already carried an `inline` row for a
direct call of a parameter (`access`, `accessWith`, `pipe`, `safe`,
`withAccess` on `@solid-primitives/utils`, `evaluate` on `@floating-ui/utils`,
`withCopy`/`withArrayCopy`/`withObjectCopy` on `utils@7`'s `./immutable`),
which the generator's filter dropped from proposal and left partial. ADR 0100
proposes and confirms exactly that shape — `from` a bare parameter `at` the
call event on the same stack, read in the export's own frame — and refuses by
name on everything else. Corpus effect: +556 candidates, +18 certified closure entries (9,752 → 9,770),
uncapped `callbacks` closures 339 → 365, 93 new candidates refused for a getter
beside the direct call, no row below the pin; the full split is in
`docs/precision-backlog.md` (ADR 0100 entry). Small, and honestly so: the
next `callbacks` levers are dependency composition for the tracked same-stack
row and ADR 0092's per-argument provenance for the helper frame.
