# Source analysis instead of package contracts: an experiment

Measured 2026-09-28 at `79dd090a` plus one experimental commit (an env-gated
export-summary dump, below). Release binary built with `make
build-checker-release`, checked-in `bin/solid-typefacts` (stamp matched), on AC
power with other agents running (load average 8 to 22), so walls are upper
bounds. Raw data, scripts and the three reduced false-positive cases (`minis/`)
are in the session scratchpad (`srcexp/`) and are not checked in. **[M]** means measured, **[E]** estimated.

The question (owner, 2026-09-28): can the checker cover most Solid 2 packages
much faster by running its existing project analysis (reactive IR and
interprocedural summaries) over dependency *source*, instead of growing the
package-contract pipeline (census, attribution, recipes, receipts; ADRs
0100-0140)?

## Headline

- **The pipeline already does this.** Its first stage, proposal generation,
  *is* the project analysis: `generate-package-contract.mjs` writes a scratch
  tsconfig with `allowJs` over the package's runtime closure and runs the same
  binary with `--emit-contract`, which reads `program.contract_exports`, the
  same interprocedural summaries a project gets (`main.rs`
  `contract_exports_for_entry_file`). So the choice is not "IR versus
  pipeline". It is whether to **trust the IR's summaries as they come out**
  instead of weakening them and re-proving closure.
- **The analysis is not what is slow.** **[M]** Generation takes 0.16-6.1 s per
  package and certification 15-192 s: about 95-99 % of each package's wall.
  Analysing the package source as a project takes 0.26-0.44 s one-shot, and
  2.99 s for `@kobalte/core` (136 files).
- **Trusting the raw summaries would roughly double "determined" exports, and
  part of that gain is wrong.** **[M]** Over 5 packages (170 exports), the raw
  IR summary states all four consumer domains for 53 exports (31 %); the
  pipeline certifies 23 clean (13.5 %). For `@solid-primitives/utils`, whose
  source analysis leaves no unresolved obligation, the IR states 35 and the
  pipeline certifies 19. Of the 17 exports only the IR determines, 6 are
  contradicted by the certifier's own census: the IR says `callbacks: []`, and
  the code invokes a caller-supplied callable (`chain`, `accessArray`,
  `contains`, `filterNonNullable`, `ofClass`, `wrapSetter`). Trusting that
  summary would silently drop a read or owner requirement.
- **Why:** `ContractClaim::default()` is `Known(T::default())`. A summary that
  observed nothing therefore says "none" in reads, returns and owner
  requirements. That is the closed-world default, not evidence. The IR reports
  `reads` Known for 100 % of exports and `returns` and owner requirements for
  97-100 %. Only `callbacks` (Open when the IR knows it does not know) and the
  `creates` walk carry signal.
- **Library code is also a stress test the project analysis does not pass
  yet.** **[M]** Analysing `@kobalte/core` and `@solidjs/router` as projects
  gives 97 *violations* inside their own source. Every one sampled is a false
  positive, and three reduce to 5-10-line cases of ordinary project code
  (below).
- **Recommendation: hybrid, not a switch.** Keep the contract pipeline as the
  trust boundary for published summaries. Add a *consumer-scoped whole-program
  mode*: for a dependency with no accepted contract, analyse its source inside
  the consumer's own program, so interprocedural analysis runs at the
  consumer's real call sites. There the closed world is true, because those
  call sites are the callers. In a two-package test this removed all 5
  `SC9005` uncertifiables, kept the true-positive `SC4001`, and cost +0.07 s.
  It needs the productization items listed in "Recommendation".

## 1. What the top 30 packages ship

For each package in `scripts/ecosystem-benchmark/certification-metric-corpus.json`
at its pinned version (`npm pack`), I read the exports map and the tarball
contents.

| class | packages | count |
| --- | --- | ---: |
| original TypeScript source shipped and reachable | `@tanstack/solid-query` (`@tanstack/custom-condition` → `src/index.ts`, 18 files), `@kobalte/utils` (`./src/*` subpath, 7 `.ts` files; its `solid` condition points at plain `dist/index.js`) | 2 |
| `solid` condition → JSX-preserved, type-stripped build | `@kobalte/core` (`./*` → `dist/*/index.jsx`, 136 `.jsx` chunks), `@solidjs/router` (`.` → `dist/index.jsx` over tsc-emitted `.js`/`.jsx`) | 2 |
| a source condition that points at unshipped files | all 24 `@solid-primitives/*`: `import/@solid-primitives/source` → `./src/index.ts`, and no `src/` in the tarball | 24 |
| └ what they ship instead | a type-stripped single-module ESM build (`//#region src/index.ts`, `@ts-self-types` → `.d.ts`), with no JSX and no babel lowering | 24 |
| plain JS only (hand-written `createComponent`, no JSX) | `@solidjs/meta`, `@solidjs/testing-library` | 2 |
| a babel-preset-solid lowered build (`template`/`insert`/`createComponent`) | only as the *default* condition beside a non-lowered alternative: `@kobalte/core` (`dist/*/index.js`), `@solidjs/router` (`dist/index.js` bundle), `@tanstack/solid-query` (`build/`) | 3 |
| ships `.d.ts` | all | 30 |

So **29 of 30 ship analysable, non-lowered code**. Only **2 of 30 ship original
typed source**. The `solid` condition is not the norm (2 of 30), and the
primitives' source condition dangles. For 24 packages the analysable code is
JavaScript whose types live in separate `.d.ts` files. Analysing it as a
project types every parameter `any`, unless runtime and declaration roots are
paired, which is what `artifact_resolution` already does for the pipeline.

## 2. How to feed package source to the project analysis

- The file set is the TypeScript program itself: `tsgo/project.go`
  `buildProgram` parses the tsconfig's `include`/`files`, and `SourceFiles`
  returns every non-declaration file. `node_modules` is excluded only
  implicitly, because installed packages reach the program as `.d.ts`. There is
  no include-node_modules flag, and `.js`/`.jsx` are analysed under `allowJs`.
- Export summaries are built only for files inside the tsconfig's directory
  (`contracts.rs` `path_within_project`), and are keyed by **exported name
  only** (`contract_export_summaries` aggregates into
  `BTreeMap<String, ContractExport>`). With 68 entrypoints, `@kobalte/core`'s
  `Root`s collide: 597 rows were dumped against 607 metric exports.
- **Least invasive route, used here:** a throwaway project per package. Its
  `package.json` depends on the exact package version plus the audited triple
  (`overrides` pins `@solidjs/signals` 2.0.0-rc.9; without it npm installs
  rc.10 and the run reports `SC9014`). The package's source directory is copied
  to `lib/`, and the tsconfig uses `allowJs`, `jsx: preserve`, `jsxImportSource:
  @solidjs/web` and `include: lib/**`. Babel bundles are removed. For
  `@tanstack/solid-query`, an entry without `export * from '@tanstack/query-core'`
  (the generator refuses to expand an external `export *`) scopes it to the
  package's own 18 exports.
- **Closure variant:** the dependency's `dist` is copied to `deps/` and wired
  with tsconfig `paths`, so its bodies are project code. `.d.ts` files are
  removed so relative imports resolve to bodies, not declarations.
- **Experimental dump** (separate commit, drop it):
  `SOLID_CHECKER_EXPERIMENT_EXPORT_DUMP=<file>` with
  `SOLID_CHECKER_EXPERIMENT_ENTRY_FILES=<a>:<b>` writes, for each export of each
  entry file, what `contract_exports_for_entry_file` returns. That is the
  generator's own input, *before* SC9 attribution and before proposal
  weakening. It is only reached one-shot: `SOLID_CHECKER_DAEMON=0`, because
  release builds use the daemon by default.

## 3. Running it on 5 packages (+ `@kobalte/core`)

### Wall times [M]

| package | files | functions analysed | source analysis, one-shot | pipeline: install / generate / certify |
| --- | ---: | ---: | ---: | --- |
| `@solid-primitives/utils` | 14 | 191 | 0.37 s | 1.8 / 0.6 / 96.8 s |
| `@solid-primitives/event-listener` | 7 | 27 | 0.32 s | 0.0 / 0.2 / 43.9 s |
| `@kobalte/utils` | 7 | 13 | 0.35 s | 2.0 / 1.0 / 24.9 s |
| `@solidjs/router` | 23 | 306 | 0.44 s | 0.0 / 1.5 / 15.1 s |
| `@tanstack/solid-query` | 18 | 149 | 0.26 s | 0.0 / 0.7 / 43.4 s |
| `@kobalte/core` | 136 | 2,731 | 2.99 s | 2.2 / 6.1 / 192.4 s |

The pipeline columns come from one host-free `certification-metric` run over
all 30 probes at this commit, with a 289 s harness wall and probes running
concurrently. It reproduced the baseline: 43 of 957 exports clean (4.5 %),
3.4 % per-package mean, 4.4 % download-weighted.

### What the raw summary determines, per export [M]

"Determined" in a domain means the IR summary has `Known` there. For `creates`
it means the `creates` proposal walk found no blocker (the IR has no `creates`
claim of its own). A value export counts as determined in every domain. The
pipeline column counts the metric's `clean` bucket, and per domain an export
whose domain is not among its open causes.

| package | exports | IR callbacks / reads / returns / creates / owner | IR all four | pipeline callbacks / reads / returns / creates | pipeline clean | both | IR only | pipeline only |
| --- | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: |
| `@solid-primitives/utils` | 99 | 83 / 99 / 99 / 37 / 99 | 35 | 31 / 81 / 22 / 35 | 19 | 18 | 17 | 1 |
| `@solid-primitives/event-listener` | 11 | 11 / 11 / 11 / 0 / 11 | 0 | 0 / 0 / 0 / 0 | 0 | 0 | 0 | 0 |
| — with `utils` as source (closure) | 11 | 11 / 11 / 11 / 4 / 11 | 4 | (same) | 0 | 0 | 4 | 0 |
| `@kobalte/utils` | 10 | 9 / 10 / 10 / 10 / 10 | 9 | 3 / 6 / 3 / 6 | 3 | 3 | 6 | 0 |
| `@solidjs/router` | 32 | 25 / 32 / 32 / 1 / 32 | 1 | 4 / 0 / 6 / 4 | 0 | 0 | 1 | 0 |
| `@tanstack/solid-query` (own exports) | 18 | 11 / 18 / 18 / 8 / 18 | 8 | 5 / 1 / 4 / 5 | 1 | 1 | 7 | 0 |
| **5 packages** | **170** | | **53 (31 %)** | | **23 (13.5 %)** | 22 | 31 | 1 |
| `@kobalte/core` | 596 joined | 513 / 596 / 579 / 114 / 579 | 111 | 12 / 13 / 9 / 15 | 7 | 7 | 103 | 0 |

**Demand-weighted.** This joins the pinned 2026-09-14 SC9005 demand by
(package, name), so the weights come from Solid 1.x-era consumers. Of
`@solid-primitives/utils`'s 827 demanded sites, IR-determined exports cover 582
and certified-clean exports 517. For `@kobalte/utils` the figures are 214 and
156 of 220. The other packages have 0-16 demanded sites.

**These IR numbers are upper bounds.** The dump is taken before attribution:
the generator later opens a domain for every unresolved obligation reached from
an export. Source analysis left obligations unresolved in every package except
`utils` (0) and the closure variant (0): `@kobalte/utils` 1, `event-listener`
5, `solid-query` 16, `router` 64, `@kobalte/core` 383. `@kobalte/core`'s 111 is
therefore not meaningful. The pipeline shows why: its 103 IR-only exports are
blocked mostly by unaccepted dependencies (`@solid-primitives/utils`,
`@kobalte/utils`, `@solid-primitives/controlled-signal`,
`@internationalized/number`) and `dialect-silent` `merge`.

**Why the IR determines more, per pipeline cause.** This covers the 31 IR-only
exports of the 5 packages. An export can carry several causes, so these are
cause rows, not exports:

| cause the pipeline records | cause rows | what it means for trusting the IR |
| --- | ---: | --- |
| `callbacks: invokes a caller-supplied callable` (missing claim form) | 8 (8 exports) | **IR summary wrong.** It says `callbacks: []`, and the body invokes a caller callable through a returned closure or array elements (`chain`, `accessArray`, `composeEventHandlers`, …) |
| `returns` / `callbacks` / `creates never proposed` | 18 | IR says "nothing". The generator had no claim form to propose, so nothing was checked. `chain`'s `returns: null` hides a returned invoker, so it is wrong there; `createIdGenerator` is plausibly right |
| `withheld operation` (`recursive-value-shape`, `operation-reachability`) | 11 | not wrong, unproved: the returns census cannot prove the value shape |
| `proposed, not certified` / recipe / census refusal / declined / unaccepted dependency | 37 | mostly unproved. `reads: declined (runtime-accessor-installation)` on 7 `solid-query` exports means the census found an installed accessor the IR summary does not describe, so those look wrong too |

### Findings inside the package source [M]

| package | violations | uncertifiable |
| --- | ---: | ---: |
| `@solid-primitives/utils` | 0 | 1 (`SC9014`, the rc.9 `sharedConfig` gap) |
| `@solid-primitives/event-listener` | 0 | 5 (`SC9005`: the tier leaves claims of `@solid-primitives/utils` open); 0 in the closure variant |
| `@kobalte/utils` | 0 | 1 (`SC9012` `exported-parameter-member-dispatch`) |
| `@tanstack/solid-query` | 0 | 26 |
| `@solidjs/router` | 2 | 74 |
| `@kobalte/core` | 95 (89 `SC1001`, 6 `SC2001`) | 343 |

Every sampled violation is a false positive, and three reduce to ordinary
project code (`.tsx`, audited triple, release binary):

1. **`SC2001` on a JSX event handler inside a control-flow child.**
   `<Show when={start()}><div onMouseEnter={() => setHovered("x")} /></Show>` in
   a component body reports "called inside owned scope"; the same `<div>` outside
   `<Show>` does not. Event handlers are legal writes (the rule page says so).
   `@kobalte/core` `resizable` has 6.
2. **`SC1001` for an arrow in an array-literal argument that is never called in
   the body.** `const run = localChain([() => console.log(n())])`, where
   `localChain` only invokes the elements from the closure it returns, and
   `run()` runs from `onClick`. This is reported as a read in the rendering
   function; `const direct = () => n()` is not. Router's `createOutlet(() =>
   routeStates() && root)` is the same shape. It also fired in the consumer test
   below for `chain([...])` in a component whose chain only runs in a handler.
3. **`SC4001` behind an owner guard.** `getOwner() && onCleanup(...)` in an
   exported helper that is called at module scope reports "onCleanup is called
   without a reactive owner", but the guard means it is never called without
   one. Router `data/action.js:273`.

Not reduced: `@kobalte/core`'s `SC1001` in handlers such as `onPointerDown` that
are passed to a polymorphic component prop (e.g. `color-area/index.jsx:49`).
They are probably the same family as item 1. Not runtime-probed.

### Consumer-scoped whole-program: one test [M]

The consumer is `App.tsx`, which uses `chain`, `accessArray`, `access`,
`withAccess` and `createMicrotask` from `@solid-primitives/utils` and
`makeEventListener` from `@solid-primitives/event-listener`, in positive and
negative cases. It was analysed two ways: *tier* (installed packages,
compiled-in accepted tier) and *source* (both packages' `dist` JS as project
files via `paths`).

| | tier | source |
| --- | --- | --- |
| `SC9005` (contract leaves a claim unknown / no summary) | 5 | 0 |
| true positive: module-scope `createMicrotask` → `SC4001` | yes, at the consumer's line | yes, **but reported at `utils/index.js:191`**, inside the dependency |
| `SC1001` false positive for `chain([...])` in a handler-only component | yes | yes (item 2 above) |
| `SC9012` on `accessArray(list)` (`.map` on an `any`-typed parameter) | 0 | 2 (types lost: JS without its `.d.ts`) |
| `access(n)` in a component body (the IR and the contract both say `p0: inline`) | not reported | not reported (not investigated) |
| wall | 0.31 s | 0.38 s |

## 4. Soundness posture: what the project analysis would need for library code

These are the places where the IR's summaries assume a closed world, or assume
facts that do not hold for an export called by unknown code:

1. **Default-`Known` claims.** `ContractClaim::default()` is `Known(empty)`, so
   "observed nothing" and "proved nothing happens" are the same value.
   Publishing a summary needs Open-by-default with explicit closure, which is
   exactly what `open_proposed_closure` plus certification do today.
2. **Returned and stored callables.** The summary vocabulary cannot say
   "returns a callable that invokes parameter 0's elements" (`chain`,
   `composeEventHandlers`) or describe a callable a class instance keeps (ADR
   0139). Inside one program the IR follows the returned closure. Across a
   summary boundary it cannot.
3. **Unknown callers and argument shapes.** Exported parameters are
   caller-controlled. Member dispatch on them is already an obligation
   (`EXPORTED_PARAMETER_MEMBER_DISPATCH`, `SC9012`), and exported helpers are
   seeded unowned under `--program-boundary open`. But a JS-only package types
   every parameter `any`, so dispatch obligations multiply unless declaration
   types are paired with runtime bodies.
4. **Module state and build conditions.**
   `tryOnCleanup = isDev ? (fn) => getOwner() ? onCleanup(fn) : fn : onCleanup`
   behaves differently in development and production. Registries (`actions`,
   `globalRegistry`) and module-level signals are shared across all consumers.
   The artifact case and host (ADR 0140) decide which bytes run. The project
   analysis has no notion of a condition-selected artifact.
5. **Transitive dependencies.** A package's behaviour depends on its
   dependencies, and without their source or accepted contracts every such call
   is an obligation (`event-listener`: 5 → 0 with `utils` as source;
   `@kobalte/core`: 383).
6. **Name-keyed export map.** Same-named exports of different entrypoints
   collide (`@kobalte/core`).
7. **Attribution and ownership of findings.** A defect proved inside dependency
   code must be reported at the consumer's call site, and library-internal
   findings must be suppressed. Otherwise every consumer inherits the 97
   `@kobalte/core`/router false positives above.
8. **Byte identity.** Analysed bytes must be the installed bytes for the
   consumer's resolved conditions. This is cheap from the lockfile and
   integrity, but it is today's receipt/closure identity by another route.
9. **Precision.** Items 1-3 of the false-positive list are project-analysis
   defects that library code exposes at scale. Trusting the IR without an
   independent certifier moves every such defect into every consumer.

## Recommendation

**Hybrid. Do not switch.**

- **Do not replace certification with trusted IR summaries.** It would appear
  to double or triple coverage (13.5 % → 31 % on five packages, more on
  kobalte) at the cost of known-wrong claims: at least 8 of 31 IR-only exports
  are contradicted by the certifier's own census, and more are "nothing
  observed" defaults. The pipeline's slowness is certification (95-99 % of
  wall), which is where its soundness comes from, not the analysis.
- **Add consumer-scoped whole-program analysis for unaccepted dependencies.**
  Inside one consumer program the callers are known, so the IR's closed-world
  reasoning is valid, and returned closures such as `chain`'s are followed at
  the real call site instead of needing a claim form. That is the class of wall
  the metric names most often (`missing claim form` blocks 68.1 % of the
  package-weighted headline, `unaccepted dependency` 534 exports). The whole of
  `@kobalte/core` analyses in 3 s, and a small dependency in well under 0.1 s
  extra.
- **Fix the three false-positive families first.** They are ordinary
  project-code defects (items 1-3 above), and they are also the precondition
  for analysing any library body inside a consumer.

**Work to productize whole-program mode [E]:**

| item | estimate |
| --- | --- |
| three FP fixes, each with positive and negative fixtures | 3-5 days |
| dependency roots: artifact selection by the consumer's conditions/host, reusing `artifact_resolution`; add the selected runtime files as program roots | 3-5 days |
| runtime/declaration pairing, so JS bodies get their `.d.ts` types (24 of 30 packages need it) | 1-2 weeks; the riskiest item, and a Type Facts producer change |
| finding attribution: dependency-internal findings suppressed, and a proof crossing into dependency code reported at the consumer's call site | 1 week |
| a per-dependency summary cache keyed by installed integrity and condition set, so the whole-program cost is paid once | 3-5 days |
| a precedence rule: an accepted contract answers first, whole-program only for what no receipt covers; fail closed on dynamic dispatch, module state or condition-dependent bindings | 2-3 days |
| gates: coverage fixtures, a consumer sweep comparing tier vs whole-program on the 30-package corpus | 3-5 days |
| **total** | **about 5-7 weeks** for one engineer |

Not established here: runtime probes of the three false positives; the
attributed (post-SC9) IR numbers for the four packages with unresolved
obligations; whole-program on a real consumer with a deep graph (`@kobalte/core`
plus its 19 dependencies); and why neither mode reports `access(n)` in a
component body.
