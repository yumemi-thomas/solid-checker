# ADR 0110: The checker analyzes Solid 2 only

- Status: accepted (2026-09-16); written after step 1's measurement and before
  steps 2-4 are implemented
- Date: 2026-09-16
- Owners: dialect selection at the process boundary, the v1 rule catalog and
  vocabulary table, the eslint-era upstream-compat surface, the
  accepted-contract tier, and the fixture and ownership corpora
- Relation: no existing ADR argues for carrying both dialects, so this is a new
  decision rather than a reversal. It withdraws step 2.5 of
  `docs/2026-09-16-retire-solid-1x-plan.md` on the evidence in § 4. ADR 0027
  (core runtime has no package contract by design) is load-bearing for § 4 and
  is not changed.

## Context

The checker carries two dialects. Retiring the 1.x one is cheap where it is
genuinely 1.x — the `solid-v1` crate, `solid_1x.rs`, a gated slice of
`upstream_compat` — and expensive where 1.x is merely the stub a regression
happens to run under. The plan's premise was that most of the second kind pins
dialect-neutral mechanics and can simply be ported.

Step 1 measured that premise instead of assuming it, by porting what could be
ported while the 1.x control still existed. The premise held for one corpus and
failed for the others:

| Corpus | Ported | Not portable |
| --- | --- | --- |
| `fixtures/package-contracts/` | 21 of 27 | 6 |
| `fixtures/reactive-ir/` + `engine/` | 2 of 19 | 17 |
| `fixtures/ownership-cases/cases.json` | 0 of 271 | 271 |
| backend test fixtures | 0 of 9 | 9 |

The ports were not a formality. They found a real defect in shared code
(`interproc.rs`'s `direct_own_call` recorded on one derivation arm instead of
read off the call site, so a *capitalized* export silently lost a `callbacks`
closure its lowercase twin got), and they found that two of 2.0's signatures
make the 1.x spelling a type error rather than a silent difference —
`createEffect(compute)` returns `never`, and the plain `createSignal` overload
takes `Exclude<T, Function>`, which an unconstrained generic cannot satisfy.
Both were found by type-checking against the audited `solid-js@2.0.0-rc.3`
install, and by nothing else; the hand-written stubs accepted them.

## Decision

### 1. A project on Solid 1.x is refused, never analyzed as 2.0

`dialect.rs` resolves the nearest `node_modules/solid-js/package.json` and, when
the resolved major has no dialect in this build, falls back to the 2.0 default.
That fallback is a hole, not a design: it analyzes a 1.x project under the 2.0
catalog and tells it nothing. It is reachable today in the
`--no-default-features --features dialect-v2` build and becomes the *only*
behaviour once the dialect is deleted.

A resolved major this build carries no dialect for produces **one project-level
`uncertifiable` result naming the `package.json` that decided it**, and no other
findings. An absent or unclassifiable install keeps today's 2.0 default: that is
an absence, not a contradicted answer, and it is what every request without an
installed `solid-js` has always received.

`Detection` (`Installed` / `Unsupported` / `Defaulted`, each resolution case
carrying the manifest path) already exists behind `detect_detailed` and is
asserted in both directions. The emission is the remaining work, and it lands in
the same slice as the deletion — never after it, because the window between the
two is exactly the silent-analysis behaviour this section forbids.

### 2. Two rules are dropped without replacement

`v1/jsx-no-undef` (SC8005) and `v1/prefer-classlist` (SC8013) are the only two of
the v1 catalog's 18 with no v2 counterpart. They are removed rather than
re-homed: `classList` does not exist in 2.0, and `jsx-no-undef` answers a
question 2.0's `jsxImportSource` arrangement asks differently. Both go in the
removed-rule ledger so an existing `rule-options.json` naming them keeps loading.

### 3. The eslint-plugin-solid parity corpus goes with the dialect

254 of the 271 `solid-v1` ownership cases are `upstream/*` — that plugin's own
`__valid__NN` / `__invalid__NN` cases, transcribed. The plugin targets Solid 1.x.
Re-pointing its cases at the 2.0 catalog would assert that upstream's
1.x-defined expectations hold for a version upstream does not support: not
parity with upstream, a new claim wearing upstream's name. 179 of the 271 are
negatives, so porting them and watching the gate stay green would prove nothing
either.

The repository already encodes this — the 37 `solid-v2` cases contain no
`upstream/*` id. Upstream parity is a 1.x concern here; product-owned cases are
authored per dialect.

**Consequence that must be carried out with the deletion**, not discovered
after it: AGENTS.md instructs that "retained behavior and intentional
divergences must be pinned in `fixtures/ownership-cases/cases.json`" and pins
`upstream_compat` to eslint-plugin-solid commit `6d3bc311`. Both lose their
subject. AGENTS.md is part of step 3's edit list.

### 4. The accepted-contract tier is keyed to the package artifact, and survives

The plan proposed replacing `pkg/contracts/accepted/` with "the Solid 2 bundle
set". There is no such set for 21 of the 54 bundles: `@solidjs/start` 2.0.3 (11)
and `@kobalte/solidbase` 0.6.13 (10) have no `solid2` benchmark row to
regenerate from.

That sub-step is **withdrawn**, on three checked facts rather than on judgement:

- **No bundle binds a dialect.** Neither `pkg/contracts/accepted/index.json` nor
  a receipt object contains `dialect`, `solid-v1`, `solid-v2`, `solidVersion` or
  `languageVersion` anywhere. A bundle's identity is
  `(packageName, packageVersion, packageIntegrity, runtimeTarget,
  declarationTarget, exportConditions)` — every field a property of the shipped
  artifact.
- **Nothing regenerates the tier during verification.** It reaches the analyzer
  through `include_bytes!` in `accepted_bundles/embedded.rs` and is written only
  by `make accepted-bundles`. `check-bundled-contracts.mjs` walks
  `pkg/contracts/bundled` and `rust/crates/solid-dialect/contracts`, not this
  tier.
- **A contract describes runtime behaviour, which the checker's scope does not
  change.** `@solidjs/start` 2.0.3's bytes invoke what they invoke. A consumer on
  Solid 2 can import that artifact, and the receipt that proved its behaviour is
  as valid after this ADR as before it.

So the tier stays at 54 bundles and step 2 has nothing to do here.

**The one real limit, recorded rather than papered over:** a contract's
*content* was derived under a vocabulary, even though its identity was not. A
bundle whose derivation depended on 1.x-only vocabulary can no longer be
**regenerated** after the deletion. These bundles are therefore frozen: valid,
receipt-proven, and not reproducible from source by a v2-only checker. Anything
that assumes regenerability — a future re-issue, a schema migration that
re-derives rather than re-signs — has to treat them as archival inputs.

### 5. The demand denominator is frozen, and the 1.x ecosystem rows are evidence

The plan's step 0 was to state that the 1.x benchmark rows have successors as
`solid2` rows. **That is false and must not be written.** Of 214 benchmark
packages, 93 appear under both targets, 46 are solid2-only, and **75 are
solid1-only** — including SolidStart itself, every `@tanstack/solid-*`, all of
`@corvu`, `solid-devtools` and `@solidjs/testing-library`. Those are current
shipping packages with no Solid 2 release, not abandoned ones.

The consumer demand recensus (2,585 call sites, 1,958 in corpus) measures **what
consumers call**. That is a fact about the ecosystem, not about which language
version the checker analyzes, and it is not made false by this ADR. The
denominator stays frozen at those numbers.

The consequence is a reporting rule: **a coverage percentage measured after this
retirement is not comparable to one measured before unless the denominator is
held.** Narrowing the corpus to solid2-only rows would raise every percentage in
`contract-coverage-census` without proving a single new statement. The pinned
baseline to compare against is `ownerRequirement: 31`, 599 operations stated of
1,940 measured sites.

## Consequences

Deleted with the dialect: `rust/dialects/solid-v1/`, `solid_1x.rs`,
`carries_eslint_era_rules()` and the two modules it gates, the `dialect-v1`
Cargo feature and its `scripts/verify.sh` arms, `docs/rules/v1/`, the 254
upstream ownership cases, the 1.x halves of six fixture divergence pairs, and
the nine backend 1.x fixtures.

Kept: `Version::V1` in `solid-dialect` — the § 1 refusal needs to recognize 1.x
in order to refuse it.

Five tests lose their subject and each has an answer already:
`the_dialect_pair_reports_different_findings_from_identical_sources` is deleted
outright (its every v2-side assertion is pinned span for span by
`fixtures/findings-snapshots/reactive-ir__dialect-solid-2.json`); two
`DIALECT_INDEPENDENT` loops narrow their scope and keep their claim; and two
`#[cfg(all(feature = …))]`-gated library tests go with the feature.

What the retirement does **not** buy: the checker does not become simpler where
it matters. `upstream_compat`'s shared modules stay, renamed; the
`static_event_values_are_attributes` and DOM-slot-folding behaviours become
unconditional rather than disappearing, and `fixtures/reactive-ir/eslint-compat`
is their only regression pin under 2.0.

## Reversal

Sections 4 and 5 are decisions made from measurement rather than from product
intent, and they are the two a reader is most likely to want to revisit. Both
are reversible without code: § 4 by regenerating the tier from solid2 rows and
accepting the loss of 21 bundles, § 5 by re-deriving the demand recensus from a
narrower corpus and restating the baseline. Sections 1-3 are not reversible
after step 3 without restoring the dialect.
