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

`scripts/ecosystem-probe-recipes.test.mjs` pins the manifest's shape, that every
declared module exists and exports `runProbeSession`, and that no module names
`session` or `harness` in a position that hands either to a package.
