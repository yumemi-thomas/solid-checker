# The ecosystem rows' probe-recipe corpus

Hand-authored, claim-addressed runtime-probe recipes for the ecosystem
benchmark's real rows. Supply it with

~~~sh
bun scripts/ecosystem-benchmark/run.mjs --attempt-certification \
  --probe-recipe-corpus "$PWD/scripts/ecosystem-benchmark/probe-recipes" …
~~~

A corpus is an **input, never a root of trust** (ADR 0006 § Recipes,
CONTEXT.md § Recipe corpus). Omitting a scheduled gate refuses that gate; a
vacuous recipe only fails to veto and can never establish closure, which
remains the implementation census's job (ADR 0008). Rust derives every module's
construction digest from the bytes it copied, so this directory cannot present
one identity and run another.

## Why here

The runner's other hand-authored, checked-in inputs — `manifest.json`,
`sentinel.json`, `phase16-thresholds.json` — live in this directory, and the
runner is this corpus's only consumer. `benchmarks/ecosystem/` was rejected: it
holds the run's *outputs* (`report.json`, `report.md`), which are pinned by the
phase-20/21 ledgers, and an input under an output directory invites both to be
regenerated together. `fixtures/` was rejected too: every corpus fixture there
is scanned by `scripts/contract-corpus.mjs` and `scripts/coverage.mjs`, and
these recipes address published ecosystem packages rather than a fixture the
repository owns.

## Claim ids are content digests, and that is the corpus's weakness

A `claimId` binds the exact normalized claim — package, version, artifact case,
export, domain. Anything that moves the emitted contract document for one of
these packages moves the id, and the recipe then addresses nothing. Since
ADR 0036 such a candidate is served by a *synthesized* veto derived from the
export's Type Facts call signature (the run's material names it
`provenance: synthesized`), and only a candidate no signature can drive stays
*withheld* ("no recipe in corpus") with the domain open. Two more withholding
reasons exist beside it: `census refused: …` (the implementation census could
not decide the candidate) and `veto did not complete: gate …` (the run ended in
an error or a timeout). None of them fails the row; a *contradiction* still
does. Re-read the ids from a run's `…certification-audit.json`
`withheldClosures` array after any generator change.

## A module and its entry must agree on the event, and nothing used to check

`event_matches` requires the emitted event's **marker and class** to equal the
entry's `expectedEvent`. Disagree on either and no event ever matches — and the
failure is silent and fails *open*: a run that completes without a matching
event is a `CleanNonObservation`, which **satisfies** the mandatory gate, so the
closure certifies on the census alone. A recipe mistyped in one character stops
being able to veto and nothing says so.

`scripts/ecosystem-probe-recipes.test.mjs` now checks the agreement statically,
for this corpus and the fixture corpora. What it cannot check is whether the
emit is *reached*: a recipe whose emit sits behind a condition that never holds
fails the same way, and only a run that contradicts something finds it.

A module that deliberately cannot emit says so, with a coverage limitation
opening `NEVER EMITS:`. The waiver is checked in both directions — a module
that declares it and *does* emit fails too, because a stale declaration would
waive the marker check for a recipe whose marker later drifts.

**Every `reads` recipe in this corpus carries that declaration, and that is the
finding rather than the convention.** All five can only ever yield a clean
non-observation; all fourteen `creates`/`returns` recipes can emit. That split
is § 6 measured instead of argued: an unenumerated read is a read of a source
the export *owns*, and that is the half no recipe can instrument.

The five are pinned by name in that test, so a sixth silent recipe is a diff
rather than a discovery — and a `creates` or `returns` recipe going silent,
which is the case that would matter, cannot happen quietly.

## Adding one

`scripts/probe-recipe-scaffold.mjs` writes the transcription. Point it at a
certification plan or audit and it emits, for every candidate withheld as
`no recipe in corpus`, a module and its manifest entry addressed by the claim
id verbatim — including the `expectedEvent.marker` the module and the entry
have to agree on, which is the mistake nothing here catches (a mismatch makes
the gate unmatchable, and an unmatchable gate *passes*).

~~~sh
bun scripts/probe-recipe-scaffold.mjs \
  --plan /tmp/run/certification-plan-0.json \
  --corpus "$PWD/scripts/ecosystem-benchmark/probe-recipes" \
  --domain reads --specifier seroval
~~~

**Run it twice.** A first pass cannot tell you which candidates a recipe can
serve: one withheld as `no recipe in corpus` is weakened out of the plan before
its demands are discharged, so its census never runs and any refusal underneath
stays masked. A throwing scaffold unmasks it — the candidate stays in the plan,
its census runs, and a `census refused: …` is reported ahead of the incomplete
gate. So scaffold into a **scratch** corpus, certify once against it, then run
the scaffold again over that audit: it prints one `unserviceable` line per
candidate nobody should finish. On `@solid-primitives/utils@7.0.0-next.4`'s `.`
case that was 24 of 45.

What it cannot write is the observation, so each emitted module **throws**
until its `UNFINISHED` guard is deleted. That is deliberate: a recipe that
completes without emitting its marker is a clean non-observation, and the
closure certifies on the census alone — a generated module that observed
nothing would certify by omission. A throw withholds the candidate instead,
exactly as no recipe does. `ecosystem-probe-recipes.test.mjs` fails on a
scaffold that reaches a commit.

## The rule nothing here can check

**A recipe must never hand `session` or `harness` to the package under test.**
The transcript API is the one legitimate path by which package code could reach
the transcript. None of these modules does; nothing detects it if one starts.

## What each recipe is for, and what it measured (2026-09-04 … 2026-09-10)

The nine `solid-primitives-utils-*-reads-9887e137.mjs` recipes (2026-09-12)
address `@solid-primitives/utils@6.4.1`'s published `.` runtime case — the
version thirty consumer projects import, per
`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.md`.
The case has forty `reads` candidates; the second scaffold pass found twenty
the census cannot decide (`callSignatureNotUnique` aliases such as
`entries = Object.entries`, proposals with a non-empty enumeration, and the
usual accessor legs), and of the twenty it can, these nine are the ones a
consumer actually names: `access`, `accessWith`, `asAccessor`, `asArray`,
`chain`, `createCallbackStack`, `isObject`, `noop`, `trueFn`. Each follows the
`access` recipe's discipline — samples that assert the answers, an owned getter
proving the apparatus live, no emit because every reachable read is the
caller's under ADR 0034 — and each declares `NEVER EMITS:`. Certifying the case
against this corpus closes all nine `reads` domains (27 → 28 certified entries,
59 → 50 withheld, no veto threw).

The eleven further `solid-primitives-utils-*-reads-9887e137.mjs` recipes
(2026-09-13) finish the same case. The second scaffold pass over the thirty-one
`reads` candidates the nine had left withheld found exactly twenty the census
refuses -- `domain-exhaustiveness` on the aliases and constants (`entries`,
`keys`, `tryOnCleanup`, `isDev`, `EQUALS_FALSE_OPTIONS`, …), proposals with a
non-empty enumeration (`arrayEquals`, `filterNonNullable`, `handleDiffArray`,
`lines`, `ndjson`, `accessArray`), and accessor or iteration legs
(`createHydratableSignal`, `createMicrotask`, `defer`) -- and eleven it can
decide: `clamp`, `compare`, `falseFn`, `isNonNullable`, `json`, `number`,
`ofClass`, `pipe`, `reverseChain`, `safe`, `withAccess`. Five of those are
consumer-demanded; the other six are written anyway because a recipe on this
dependency node closes the entry in every row that depends on it (forty-two
in the 418-row corpus), and each header says which it is. The pure-primitive
ones (`clamp`, `compare`, `json`, `number`) keep objects out of their samples
on purpose: `Math.max`, `<`, and `Number` coerce a caller's object through its
own `valueOf`, and that read is the caller's under ADR 0034, not a
contradiction. Certifying the case against this corpus closes all eleven
(44 → 55 closed domains, 50 → 40 withheld, nothing lost, no veto threw); the
twenty refused candidates are the census's to move, and no recipe can.

Over the 418-row corpus (release binary, 1200 s row timeout) against the
same tree with the nine-recipe corpus: `no recipe in corpus` 11,176 → 10,714
(exactly 11 × 42), certified closure entries 6,636 → 6,678, uncapped `reads`
closures 52 → 96, no row below baseline, Solid 2 unchanged to the entry.

The sixty-two `solid-primitives-utils-*-reads-{2bf41ff6,f81b5488,6cd714eb}.mjs`
modules (2026-09-13) finish `@solid-primitives/utils@7.0.0-next.4`'s `.` case in
the same way. The ecosystem corpus certifies that one `dist/index.js` under
three artifact cases — `2bf41ff6…` on solid-js@2.0.0-rc.0 rows, `f81b5488…` and
`6cd714eb…` on rc.3 rows — and a claim id is case-bound, so each case gets its
own copy of each module: the private workspace writes one file per recipe
entry, and the corpus test requires declared modules and files to agree
one-to-one. The second scaffold pass over the forty-five `reads` candidates
found twenty-three the census refuses (the same aliases, constants,
non-empty enumerations and accessor legs as 6.4.1, plus `contains`,
`globalRegistry` and `wrapSetter`) and twenty-two it can decide: the twenty
exports 6.4.1 already had recipes for, whose bodies are the same code compiled
differently, plus `afterPaint` (no `requestAnimationFrame` under the pinned
interpreter, so its samples assert the caller's callback never runs) and
`createIdGenerator` (host clock and random source; neither a reactive source
nor anything the recipe owns). Four of the twenty-two already had `2bf41ff6`
recipes, hence eighteen new modules there and twenty-two for each rc.3 case.

The twelve `floating-ui-utils-*-reads-9bc68a12.mjs` recipes (2026-09-13) address
`@floating-ui/utils@0.2.12`'s ESM `.` case, the one the corpus reaches from three
rows and the one a standalone certification selects (the package has no Solid
dependency, so the closure does not fragment). Of its twenty-four `reads`
candidates the census refuses twelve -- the `Math` aliases and the placement
constant arrays (`domain-exhaustiveness`), four proposals with an operation, and
`getOppositeAxisPlacements`'s coercion -- and decides twelve: pure functions
over the caller's placement strings (`getSideAxis`, `getAxisLength`,
`getOppositeAxis`, `getAlignmentAxis`, `getExpandedPlacements`, `clamp`), and
four that read the caller's object (`expandPaddingObject`, `getPaddingObject`,
`rectToClientRect`, `getAlignmentSides`), where the recipe counts the reads on
owned getters -- four, four, four and two per call -- and does not emit, because
they are the caller's under ADR 0034. `evaluate` and `createCoords` follow the
`access`/`noop` pattern. Certifying the case against this corpus closes all
twelve (17 → 29 closed domains, 41 → 29 withheld, no veto threw).

The seventy-three modules of 2026-09-13's second batch follow the frontier the
re-pinned report showed after ADR 0099: 1,550 `reads` entries withheld for want
of a recipe, on 219 artifact cases, the four largest of which are served here.

- `solid-primitives-utils-*-reads-1bea9ecd.mjs` (23): `@solid-primitives/utils@7.0.0-next.4`'s
  *own* corpus rows certify `dist/index.js` under a fourth artifact case, so the
  twenty-two decidable recipes of the `2bf41ff6` set are carried over verbatim
  with that case's claim ids. `arrayEquals` is copied too although its
  candidate is `census refused` there (the proposal names an operation): a
  recipe is what turns `no recipe in corpus` into the refusal underneath, the
  convention the first five recipes set.
- `solid-primitives-utils-immutable-*-reads-b70ad6d1.mjs` (12) and
  `solid-primitives-utils-colors-*-reads-8a0d569b.mjs` (11): the package's
  `./immutable` and `./colors` entrypoints, each its own artifact case. The
  second scaffold pass found twenty-three and seven candidates the census
  refuses -- non-empty enumerations (`drop`, `map`, `lighten`, `mix`, …),
  coercion forms (`add`, `divide`), element-access legs (`pick`, `omit`) -- and
  these twenty-three it can decide. The colour manipulation functions take a
  parsed `Color`, not a string; a first draft passed strings and the package
  threw `toFormat is not a function`, which the gate reported as
  `veto did not complete` -- the recipe's misuse, not the package's defect,
  and the samples now parse first.
- `motion-utils-*-reads-9a3a41a5.mjs` (27): the `motion-utils@12.39.0`
  dependency node all three `motion-solidjs` rows certify through. Its `.` case
  has thirty-eight `reads` candidates. Eight of them -- `easeIn`, `easeOut`,
  `easeInOut`, `backIn`, `backOut`, `backInOut`, `circOut`, `circInOut` -- are
  constants bound to a call result (`cubicBezier(…)`, `reverseEasing(…)`),
  and a recipe for any one of them makes the certifier refuse the *whole row*
  ("runtime implementation does not match the snapshot-replayed export
  binding"): the demand plans in the audit are digests, so the eight were found
  by bisecting the throwing scaffolds. They stay recipe-less by decision. Three
  more are census refusals (`SubscriptionManager`, `addUniqueItem`,
  `removeItem`). The twenty-seven left are arithmetic, predicates and
  higher-order helpers over the caller's functions (ADR 0034); `warning`,
  `invariant` and `warnOnce` are sampled on their passing branch only, so
  nothing is logged and the module's `Set` of warned messages is untouched.

Every module declares `NEVER EMITS:` for the reason the earlier batches do.
Certifying each entrypoint standalone against this corpus closes all
seventy-two decidable `reads` domains with no veto thrown; the corpus effect is
in the precision backlog.

The 2026-09-13 evening batch (27 modules) went to the mid-size dependency
nodes the ranking left, after the corpus's own frontier was re-measured:

- `corvu-utils-dom-*-reads-{f34410e8,fd42a1d9}.mjs` (4) and
  `corvu-utils-reactivity-*-reads-{f5afb395,fa65fd49}.mjs` (8): the
  `@corvu/utils@0.4.2` `./dom` and `./reactivity` entrypoints, each published
  under two artifact cases (the `solid` and `default` conditions), which five
  Solid 1 rows certify through. The second scaffold pass refused `combineStyle`
  (a spread-assignment accessor form) and `contains`/`sortByDocumentPosition`
  (non-empty enumerations); `afterPaint`, `callEventHandler`, `access`,
  `chain`, `mergeRefs` and `some` it can decide. `afterPaint` schedules through
  `requestAnimationFrame`, which the harness realm lacks, so the recipe installs
  a same-turn frame shim and says so in its limitation.
- `tanstack-store-*-reads-0bdfa1cb.mjs` (4): `@tanstack/store@0.11.1`, the node
  five `@tanstack/solid-*` rows share. `Store`, `ReadonlyStore`, `flush`,
  `shallow` and `toObserver` are census refusals; `batch`, `createAtom`,
  `createAsyncAtom` and `createStore` are decidable. `createStore`'s second
  argument is an actions *factory*; the first draft passed an object and the
  gate reported the package's `TypeError` as `veto did not complete`.
- `solid-primitives-utils-colors-*-reads-cfb40777.mjs` (11): the `./colors`
  recipes of the `8a0d569b` case carried, byte for byte apart from the header,
  onto the same module under `@kobalte/core@2.0.0-alpha.0`'s closure, whose
  claim ids the pin lists.

The cases the ranking put above these were left by decision: the
`@solid-primitives/utils` `.` cases hold the census-undecidable aliases and
non-empty enumerations the earlier passes named; `@corvu-next/utils`'s DOM
entrypoint and `@tanstack/devtools-ui`'s icons need a window.

The tier after that (26 modules, same evening) is mostly carrying, because
`@corvu/utils` is published under several closures and versions and the
bodies do not move between them:

- `corvu-utils-reactivity-*-reads-a0d201c0.mjs` (4): the 0.4.2 `./reactivity`
  module under the accordion, drawer and popover rows' own closures.
- `corvu-utils-reactivity-*-reads-{9e170a86,f240869a}.mjs` (8) and
  `corvu-utils-dom-*-reads-{84d6a8cd,2c64155d}.mjs` (4): `@corvu/utils@0.3.2`,
  the node three rows still certify through; its `access`, `chain`,
  `mergeRefs`, `some`, `afterPaint` and `callEventHandler` are byte for byte
  the 0.4.2 bodies, and the second scaffold pass against a 0.3.2 install
  confirmed the same census verdicts.
- `corvu-next-utils-dom-*-reads-{7496b629,95369190}.mjs` (4): the
  `@corvu-next/utils@0.1.4` fork's `./dom` under two Solid 2 rows, the same
  two bodies again, import specifier changed.
- `corvu-utils-{data-if,is-button,is-function}-reads-{1b8ce990,77550142}.mjs`
  (6): three pure predicates on the 0.4.2 `.` entrypoint, new recipes.

A carried module is a new file with a new claim id and a header naming its
own case; the corpus test's one-file-per-entry rule still holds, and the
copies are cheap because the body is a handful of lines.



The `seroval-create-reference-identity-development.mjs` recipe (2026-09-10)
addresses Seroval 1.5.6 `createReference`'s `returns` closure in the published
development ESM case, the one candidate the retained real certification left
withheld as `no recipe in corpus`. The census proves the export returns
parameter 1 by identity; ADR 0036's synthesis cannot drive a bare `<T>`, so a
hand recipe is the only way that gate runs. Eleven samples cross the
primitive/object boundary — both registries are plain `Map`s, not `WeakMap`s.
Adding it closes `returns`, and the consumer's SC9005 goes from
`reactiveReads,returns` to `reactiveReads`; a falsifying variant refuses
certification naming the exact claim. Measured in
`docs/package-contract-v2/phase21/2026-09-10-real-package-returns-recipe.md`.

The `seroval-create-plugin-identity-*` recipes (2026-09-08, ADR 0075) address
the production and development Seroval 1.5.6 `createPlugin` returns closures.
Each passes valid plugin objects, including a frozen object, and emits only
when the result differs by identity. The complete return census proves the
closure; these finite samples only falsify it. Neither recipe passes session
or harness interfaces to Seroval. Separate module names are required because
the private workspace creates one file for each recipe entry.

The five `kobalte-utils-092-browser-*` recipes address `clamp`, `isArray`,
`isFunction`, `isNumber` and `noop` in the independent published root JS case.
Use `contract certify --entrypoint . --conditions browser --dependency-graph-lane`
with this corpus and the exact published package. All five mandatory vetoes
complete; five creates domains close. Other domains remain open. These claims
do not replace the source subpath claims in the three-row benchmark, and the
browser condition selects published runtime bytes without adding a DOM.
ADRs 0015–0021 describe the proof and dependency-workspace corrections.

ADR 0010 separates actual declaration-file imports from executable dependency
hazards. Alpha now offers six new JS creates candidates. The two `clamp`
recipes address its distinct import/solid cases; the other four JS candidates
and its seven source candidates remain withheld for missing recipes. The
historical first-run report is
`docs/package-contract-v2/phase21/2026-09-04-first-real-creates-certification.md`;
current measurements are recorded in ADR 0010, without repinning that ledger.

An isolated follow-up addresses each of the four remaining JS claims with a
scratch corpus. Both `getScrollParent` cases refuse the unknown-accessor census;
both `isPointInPolygon` cases refuse the iteration-protocol census. None reaches
its runtime gate. See `docs/2026-09-04-kobalte-remaining-js-candidates.md` for
exact claims, source spans and audits. The checked-in corpus remains unchanged:
these domains are open, and adding recipes alone cannot certify them.

| recipe | claim | measured outcome |
| --- | --- | --- |
| `seroval-create-reference-identity-development.mjs` | `seroval@1.5.6` `createReference` `returns`, published `dist/esm/development/index.mjs` | the withholding clears and `returns` closes; the consumer's SC9005 drops `returns` and keeps `reactiveReads` |
| `kobalte-utils-alpha-clamp-import.mjs`, `kobalte-utils-alpha-clamp-solid.mjs` | `@kobalte/utils@2.0.0-alpha.0` `clamp`, published `dist/index.js`, two exact cases | census and mandatory veto complete; two creates closures certify; other domains remain open |
| `kobalte-utils-noop.mjs` | `@kobalte/utils@0.9.2` `noop`, `./src/noop.ts` | the implementation census **proves** `creates: []`; the probe gate then refuses because the artifact case is a `.ts` file under the private workspace's `node_modules` and the pinned interpreter will not strip types there |
| `solid-primitives-utils-{access,array-equals,clamp,compare,true-fn}-reads-2bf41ff6.mjs` | `@solid-primitives/utils@7.0.0-next.4` `reads: []`, published `.` case `2bf41ff6…` | **nothing closes.** `arrayEquals` and `compare` are `census refused` and no recipe can serve them; `clamp`, `trueFn` and `access` run clean and then wait on a withheld `solid-js` claim |
| `solid-primitives-i18n-scoped-translator.mjs`, `solid-primitives-i18n-flatten.mjs`, `solid-primitives-i18n-chained-translator.mjs` | the three i18n creates candidates | the current row refuses first on `chainedTranslator`'s `property-access-unknown-accessor (SpreadAssignment)`; no gate executes |

The clamp recipes exercise numeric boundaries and defaults and observe added
own global keys during the calls. They cannot observe arbitrary allocations,
package initialization before recipe entry, DOM behavior, or every argument.
No session or transcript capability reaches package code. Only the independent
implementation census proves completeness; these finite observations can veto.

The five `solid-primitives-utils-*-reads-2bf41ff6.mjs` recipes (2026-09-11)
address `@solid-primitives/utils@7.0.0-next.4` `reads: []` claims on the
published `.` runtime case four benchmark rows share. They are the corpus's
first `reads` recipes for a real package, and they are short by right: the
artifact case carries no `runtime-accessor-installation` hazard, so the census
states syntactically that `dist/index.js` installs no accessor and there is no
owned trap to count (§ 12 of the reads-veto design record). `access` is the
exception and executes caller code, so it passes an accessor reading a getter
the recipe owns, asserts the getter ran, and does not emit — the read is the
caller's under ADR 0034, and the assertion proves the apparatus is live.

**Measured outcome: none of the five closes a domain**, and two of them can
never fire. `arrayEquals`' proposal names an operation, so it is not the empty
closure `PROPOSABLE` admits; `compare`'s `a < b` is a coercion form with no
reviewed subject root. Both are `census refused`, which no recipe can serve.
They are kept anyway, deliberately: without a recipe those candidates report
`no recipe in corpus` and the census refusal underneath stays hidden, so the
recipe is what makes the real blocker visible. The other three — `clamp`,
`trueFn`, `access` — run clean and then wait on a withheld `solid-js` claim,
which is why recipe work has to go bottom-up from `solid-js` rather than
starting at `utils`. Measured in
`docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
§ 43.

**Superseded in part by [ADR 0101](../../../docs/adr/0101-described-reads-enumeration.md)
(2026-09-13).** "The proposal names an operation" is no longer a refusal when
the operation is the generator's `parameter-member` row — a member invocation
on a caller parameter, `list.map(...)`, `a.compareDocumentPosition(b)` — and
every item is one; the census confirms the enumeration site for site and a
synthesized tripwire veto serves it. `arrayEquals`, `filterNonNullable`,
`accessArray`, `lines`, `contains` and `sortByDocumentPosition` now close on
every case here, hand recipe or not (where a hand recipe exists it still runs
as the mandatory veto). `handleDiffArray` and `ndjson` keep refusing: their
proposals name two operations one of which is not such a row. `compare`'s
coercion refusal stands. The empty enumeration is unchanged and still needs a
hand recipe, for the reason in "Why hand-authored" above.

## Twenty-four carried and DOM-predicate `reads` recipes (2026-09-14)

Two batches the 2026-09-13 depth plan had already sized, both of them among the
few candidates a scaffold pass had called *decidable* rather than census
refused, so the only question left was authoring.

Sixteen are carried. `@corvu-next/utils` is a fork of `@corvu/utils`, and its
`./reactivity` entrypoint re-exports `dist/chunk/ZV6G25TT.js`, which is
**byte-identical** to `@corvu/utils@0.4.2`'s in both published versions the
corpus certifies through (0.1.4 under the `@corvu-next/accordion` and
`@corvu-next/popover` rows, 0.1.5 under its own). That was diffed against both
installs before anything was written, because "the same bodies" is the whole
warrant for carrying a recipe: the four `access`, `chain`, `mergeRefs` and
`some` modules are the 0.4.2 ones with the import specifier changed, on four
artifact cases (88 detail rows).

Eight are new, on `@floating-ui/utils@0.2.12` `./dom` under
`@corvu-next/popover@0.1.5`'s case (64 rows). They are the eight of that
entrypoint's twenty exports a scaffold pass found decidable; the other twelve
refuse on `window` and `instanceof` forms, which no recipe can serve. Each of
the eight reaches every value it inspects off an argument, so what it reads is
its caller's under ADR 0034, and each sample passes a caller-owned node-like
object (`{ ownerDocument: { defaultView } }`, `{ parentNode, ownerDocument }`)
rather than anything the module owns.

Two things measured rather than assumed. `isWebKit` needs **no** host shim: the
depth plan expected a `CSS.supports` or `navigator` stand-in, but the body
short-circuits on `typeof CSS !== 'undefined'` and memoizes `false`, so the
recipe installs nothing and declares no shim. What does need declaring, and is
declared on all eight, is the realm: the harness has no `window`, so `isNode`,
`isElement` and `isHTMLElement` answer false and these samples exercise only
the non-DOM branch — `getNodeName` takes the mocked-node `#document` fallback,
which is why `isLastTraversableNode` answers true and `isTableElement` false
for any argument here. The samples say nothing about the branch a browser realm
takes. Every one of the twenty-four was executed against the published package
before the run, not against a stub.

**Measured outcome: all 152 rows close**, which is the first `reads` batch in
this corpus where every targeted row did. Corpus effect (release binary, 418
rows, `--timeout 1800`): certified closure entries 10,290 -> 10,352 (+62);
`reads`-closed entries 6,040 -> 6,192 (+152); `no recipe in corpus` `reads`
detail rows 2,242 -> 2,090; visible `reads` census refusals unchanged at 225,
so nothing moved from one withholding class to another; no row below the pin,
no status move; wall 1,153 s -> 1,198 s.

## Ten corvu `./create/*` `reads` recipes (2026-09-14)

The first batch written from a pass-2 census rather than from a reading of the
source (`../../../docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`),
and the batch that census ranked first: `@corvu/utils@0.4.2`'s
`./create/keyedContext` (three exports, two artifact cases, 96 rows),
`./create/once` and `./create/controllableSignal` (`default`, two cases each,
68 rows).

Three things had to be established before a line was written, because each
could have produced a plausible wrong recipe:

- **Which subpath each `default` case is.** The audit does not name a source
  file for a candidate the census *decides* — only for one it refuses. The
  `controllableSignal` pair was named by the `callbacks`-domain refusal text on
  the same artifact cases; `once` was settled by a falsifiable test rather than
  by choosing between `once` and `register` on the strength of a grep:
  `@corvu/disclosure@0.2.2`'s closure imports `controllableSignal`,
  `keyedContext` and `once` and nothing else, and all four `default` cases
  appear in it, so none of them can be `register`.
- **That the claim ids transfer.** A graph-lane certification of
  `@corvu/disclosure` reproduces all six corpus artifact cases, and all ten
  claim ids are byte-identical to the pinned report's.
- **That the closures actually close.** The depth plan expected these to "wait
  on a withheld `solid-js` claim" — the failure that made the five
  `solid-primitives-utils-*-reads-2bf41ff6` recipes above close nothing. A full
  certification against a scratch corpus carrying the finished modules closed
  all ten and withheld none, before any of this was checked in.

What the recipes assert: `createKeyedContext` is idempotent per key and ignores
a second default; `getKeyedContext` reads back `undefined` then the registered
context; `useKeyedContext` yields `undefined` for an unregistered key and the
registered default outside a provider; `createOnce` returns a closure *without*
invoking the supplied function, and the memo is built once; and
`createControllableSignal` starts at `initialValue`, follows its setter, and
defers to a supplied `value` accessor while still reporting `onChange`.

The keyed-context registry is module state that outlives a call, so each
module's keys carry its own artifact case. That is not cosmetic: an earlier
draft shared keys between the two case variants, and `getKeyedContext`'s
"unregistered key reads back `undefined`" assertion failed as soon as both ran
in one realm. The harness isolates realms, so it would likely never have
fired — which is exactly why the assertion should not depend on it.

**Measured outcome: all 164 rows close.** Corpus effect (release binary, 418
rows, `--timeout 1800`): certified closure entries 10,352 -> 10,420 (+68);
`reads`-closed entries 6,192 -> 6,356 (+164); `no recipe in corpus` `reads`
detail rows 2,090 -> 1,926; visible `reads` census refusals unchanged at 225;
no row below the pin, no status move; wall 1,198 s -> 1,316 s.

## Ten `@solid-primitives/refs` and `/scheduled` recipes, finishing Tier A (2026-09-14)

The last of the rows the 2026-09-14 pass-2 census found decidable:
`@solid-primitives/refs@1.1.4` (`defaultElementPredicate`, `getFirstChild`,
`getResolvedElements`, `resolveFirst` on `b97f9095`, 24 rows) and
`@solid-primitives/scheduled` (`createScheduled`, `debounce`, `leading` on
1.5.3's `e1a524fa` and 2.0.0-next.2's `6f867f0c`, 18 rows).

**The harness realm resolves Solid's client condition**, and that is the thing
to know before writing against either package. A scratch Node realm resolves
the *server* condition, where `isServer` is true, `defaultElementPredicate` is
a `"t" in item` membership test and `debounce` returns a no-op. Every sample
here passed against the published packages in that realm and was still wrong:
the first certification threw `ReferenceError: Element is not defined`, which
only `item instanceof Element` can raise. So the predicate recipe installs a
stand-in `Element` constructor -- the shim pattern the `afterPaint` recipes use
for `requestAnimationFrame` -- and every module in the batch declares the
client branch as its coverage limitation. An earlier draft declared the
opposite, which is worse than declaring nothing: it would have told a later
reader the samples covered a branch they never reach.

What the recipes assert: the predicate matches an `Element` instance and
rejects a plain object, `null` and a primitive; `getFirstChild` takes the first
match depth-first and invokes a zero-arity thunk; `getResolvedElements`
flattens nested matches in document order; `resolveFirst` resolves through the
memos it builds, sampled inside a recipe-owned root that is disposed;
`debounce` and `leading` return a callable carrying `clear` without invoking
the caller's callback at the call event, and `leading` fires once on the
leading edge; `createScheduled` invokes the caller's scheduler exactly once and
reports false untracked.

Closure was verified in a scratch corpus before check-in, on all three cases --
`b97f9095` through a graph-lane certification of `@kobalte/utils@0.9.2`,
`e1a524fa` through `@corvu/drawer@0.2.4`, and `6f867f0c` standalone as the root
row it is.

**Measured outcome: all 42 rows close.** Corpus effect (release binary, 418
rows, `--timeout 1800`): certified closure entries 10,420 -> 10,456 (+36);
`reads`-closed entries 6,356 -> 6,398 (+42); `no recipe in corpus` `reads`
detail rows 1,926 -> 1,884; visible `reads` census refusals unchanged; no row
below the pin, no status move; wall 597 s -> 619 s.

This closes Tier A authoring. What remains on those cases is census refusal, not
missing recipes: `Ref` and `resolveElements` (12 rows, unrooted property
access), `throttle`, `scheduleIdle` and `leadingAndTrailing` (18 rows, spread
and element access with no reviewed subject root), `./immutable` (22) and
`combineStyle` (66). `@solid-primitives/refs@3.0.0-next.0` (24 rows) stays
unmeasured behind the `motion-utils` snapshot-replay blocker.

`scripts/ecosystem-probe-recipes.test.mjs` pins the manifest's shape, that every
declared module exists and exports `runProbeSession`, and that no module names
`session` or `harness` in a position that hands either to a package.
