# What blocks the open-claims gate, ranked by consumer demand

- Measured on: 2026-09-15 at `a8fc8cbb`.
- Inputs: the pinned `benchmarks/ecosystem/report.json` (2026-09-14, 381 probes,
  8,950 exports), `phase21/2026-09-14-consumer-demand-recensus.json` (146
  projects, 2,585 call sites), and a fresh `make ecosystem-regression` at this
  commit (418 probes, 349 complete contracts, 32 partial, 12m).
- Read-only measurement. No source, fixture or snapshot changed by it.

Two questions were asked: did ADR 0109 move `returns`, and what fraction of the
476,700 `unaccepted-external-dependency` declines would clear if dependency
contracts were accepted. The answers are "no, and the reason is precise" and
"that is the wrong denominator". Together they replace a six-figure headline
with a backlog of 248 named items, and they correct three things earlier reports
of this corpus — including several of mine — got wrong.

## 1. The gate reads four claims, and `callbacks` is not one of them

`push_unknown_contract_claims`
(`rust/crates/solid-reactive-ir/src/contracts.rs:992`) assembles
`unknown-contract-claims:` from exactly four conjuncts:

| claim | domain | condition |
| --- | --- | --- |
| `reactiveReads` | reads | unconditional — `reads_completeness_demanded()` is `const fn … { true }` |
| `returns` | returns | `returns_demanded`, scoped per call site by `returns_shed_symbols` |
| `ownerRequirements` | creates | unconditional |
| `asyncBehavior` | — | `summary.async_behavior.is_open()` |

`callbacks`, `cleanups`, `disposals`, `invalidates`, `throws`, `writes` and
`recursiveValue` never reach this function. Ranking `callbacks` beside `creates`
and `reads` as an open-claims constraint ranks a domain the gate does not read.
`callbacks` does block consumers, through a different finding —
`unknown-callback-execution`, 983 of the 2,585 census sites.

`asyncBehavior` has no entry in `unknownByDomain`, so its corpus closure rate is
**unmeasured**. Every conjunction below is an upper bound for that reason alone.

## 2. The corrected ceiling

Per probe over the pinned report, floor and ceiling for the conjunction:

| conjunction | floor | ceiling | of 8,950 |
| --- | ---: | ---: | ---: |
| `reads` | 1,019 | 1,019 | 11.39% |
| `creates` | 668 | 668 | 7.46% |
| `returns` | 139 | 139 | 1.55% |
| `reads ∧ creates` | 246 | 490 | 5.47% |
| `reads ∧ creates ∧ returns` | 0 | 35 | 0.39% |

`returns` is demanded at 36 of the 2,585 census sites (1.4%). So the operative
ceiling for nearly all demand is the 5.5% row, not the 0.39% one — an earlier
report quoted the latter as if it were general.

## 3. ADR 0109 created 821 `returns` candidates and closed none

`make ecosystem-regression` at `a8fc8cbb` against the 2026-09-14 pin:

| domain | closed, pinned | closed, fresh |
| --- | ---: | ---: |
| `returns` | 139 | **139** |
| `reads` | 1,019 | 1,019 |
| `creates` | 668 | 668 |
| `callbacks` | 1,060 | 1,072 |

Every decline count is byte-identical, `exportsProven` stays 0. Taken alone this
reads as "the ADR did nothing". It is not what happened. Withheld closures:

| | pinned | fresh |
| --- | ---: | ---: |
| total withheld | 7,114 | 10,406 |
| `returns` | **63** | **884** |
| `creates` | 1,912 | 3,015 |
| `reads` | 1,705 | 2,817 |
| `no recipe in corpus` | 1,381 | 3,265 |

and the composition of the `returns` bucket inverted:

| `returns` withheld by reason | pinned | fresh |
| --- | ---: | ---: |
| `veto did not complete` | 63 (100%) | 137 |
| `no recipe in corpus` | 0 | **711 (80.4%)** |
| `census refused` | 0 | 36 |

ADR 0109 removed the wall it aimed at — the census now decides a merged-props
root, and 821 new `returns` closure candidates exist that did not before. Of
those, 711 died at the next wall down: **no probe recipe names the claim, so the
mandatory veto cannot run, so the candidate is withheld.** Closure moves zero
because a withheld candidate is exactly as open as a refused one.

The same shift happened corpus-wide: `noRecipe` went from 19.4% to 31.4% of all
withheld closures. **The probe-recipe corpus is now the single binding
constraint on this corpus**, and this branch is what made it so.

## 4. `unaccepted-external-dependency` is a measure of `@kobalte/core`

The 476,700 (91.6%) `unaccepted-external-dependency` majority is a count of
(export × domain × callee) events, and three rows carry 90% of it:

| declines | row |
| ---: | --- |
| 333,772 | `@kobalte/core@0.13.13 \| solid1` |
| 63,784 | `@kobalte/core@2.0.0-alpha.0 \| solid2` |
| 18,840 | `@kobalte/solidbase@0.6.13 \| solid1` |

`record_opaque_frontier` sets `affected_exports: []` and
`affected_domains: all_domains()`, so one unaccepted import opens every domain of
every export in the artifact case. The figure scales with how broad a package is,
not with how much demand waits on it.

On the demand surface it does not appear. Weighting each open gate-relevant
domain by the consumer call sites behind it:

| sites | why the domain stayed open |
| ---: | --- |
| 1,040 | `veto did not complete: gate …` |
| 734 | `no recipe in corpus` |
| 160 | `creates census refuses an unresolved callee` |
| 71 | `creates census refuses a resolved callee that is neither …` |

`dependencyWithheld` is **0** across all 399 certification attempts in the pin.
The answer to "what fraction of the 476,700 would clear" is that the question
does not bind: none of the exports consumers actually import is waiting on it.

## 5. The backlog is 248 records on 68 exports

Of the 2,585 census call sites, 1,958 are in the measured corpus:

| sites | state |
| ---: | --- |
| 689 (35.2%) | every gate-relevant domain already closed |
| 925 (47.2%) | at least one gate-relevant domain open |
| 344 (17.6%) | no ledger entry at all |

Crossing the 925 against the pin's 7,114 withheld closures: **68 of the 74 open
exports — 877 of 925 sites, 94.8% — carry a named withheld record saying exactly
why.** Six exports (48 sites) have none: the candidate was refused before it
became a candidate, which is the pre-0109 `returns` shape.

| bucket | records | exports | sites reached |
| --- | ---: | ---: | ---: |
| `no recipe in corpus` | 89 | 43 | 692 |
| `census refused: …` | 87 | 37 | 332 |
| `veto did not complete: …` | 72 | 28 | 502 |

Exports appear in more than one bucket, so sites do not sum. Ten exports carry
over half the reach:

| sites | export | blocked gate domains |
| ---: | --- | --- |
| 254 | `@kobalte/utils.mergeDefaultProps` | creates: veto; reads: noRecipe |
| 112 | `@kobalte/utils.callHandler` | creates: census; reads: noRecipe |
| 60 | `@kobalte/utils.createGenerateId` | creates: veto; reads: noRecipe |
| 48 | `@kobalte/utils.composeEventHandlers` | creates: census; reads: noRecipe |
| 37 | `@solid-primitives/utils.tryOnCleanup` | reads: noRecipe |
| 37 | `@solid-primitives/utils.entries` | reads: noRecipe |
| 34 | `@kobalte/utils.focusWithoutScrolling` | creates, reads, returns: veto |
| 28 | `@kobalte/utils.visuallyHiddenStyles` | reads: veto |
| 24 | `@kobalte/utils.contains` | creates, reads: veto |
| 22 | `@solid-primitives/utils.createCallbackStack` | creates: census |

## 6. What each bucket costs

**`no recipe in corpus` — largest reach, and it cannot be scripted.** The corpus
carries 327 checked-in recipes. `scripts/probe-recipe-scaffold.mjs` emits one
module per gap, but every emitted module **throws until an author deletes its
`UNFINISHED` guard** — by design: `evaluate_runtime_probes` treats a recipe that
runs without emitting its marker as a `CleanNonObservation`, i.e. the mandatory
veto passing, so a scaffold that merely called the export would silently convert
"nobody observed this" into "nothing contradicted it". These are hand-authored
runtime observations.

**`veto did not complete` — operational, not semantic.** 502 sites. The named
causes corpus-wide are `the worker threw: ReferenceError: window is not defined`
(63 `reads` records), `Error: Stripping types is currently …`, `synthesized veto
cannot run for this artifact case: probe harness config …`, and `the probe worker
could not resolve "…"`. A DOM-bearing probe environment and a type-stripping fix
would address most of it with no new semantics.

**`census refused` — the only genuinely semantic bucket.** 332 sites. Named forms,
largest first: `property-access-unknown-accessor` (`PropertyAccessExpression` and
`ElementAccessExpression`), `coercion (BinaryExpression)`, a binding initialized
by an undecidable expression, and a call through a nested callable's parameter.

## 7. Fixtures are not a forecast of the ecosystem

The >50% closure rates recorded in earlier sessions are real and belong to
`fixtures/package-contracts/` — 93 documents, 572 export references:

| domain | fixtures | ecosystem |
| --- | ---: | ---: |
| `creates` | 80.9% | 7.5% |
| `callbacks` | 68.7% | 11.8% |
| `reads` | 61.0% | 11.4% |
| `returns` | 17.8% | 1.6% |

Fixture stubs are self-contained, so they have no unaccepted dependency frontier
and no probe-recipe gap. The two corpora agree on one thing, and it is the useful
one: `returns` is the worst domain in both.

## 8. Open: four lost receipts on this branch

`make ecosystem-regression` **fails** at `a8fc8cbb`: four rows the 2026-09-14 pin
certified now refuse, with zero certification fixes.

| probe | previous | current |
| --- | --- | --- |
| `@tanstack/solid-start@1.168.47\|solid1\|only` | certified | refused |
| `@tanstack/solid-start@2.0.0-rc.2\|solid2\|floor` | certified | refused |
| `@tanstack/solid-start@2.0.0-rc.2\|solid2\|head` | certified | refused |
| `solid-devtools@0.34.5\|solid1\|only` | certified | refused |

All four carry the same reason: **`retained floor main has no certified
exports`**, thrown by `packages/cli/scripts/retained-certification-floor.mjs:57`
when a freshly certified graph node publishes an artifact case with an empty
`exports` map.

Not diagnosed. The prime suspect is `59c957b6` ("prove a re-exported name's
closure from its dependency's receipt"), which is precisely about what a
cross-package re-export contributes to an emitted document, and both affected
roots are barrel-heavy; `fdfeb30b` is the other candidate. This is a hypothesis
from the commit range, not a bisect.

## 9. Triage of the 10,406 withheld closures

Every bucket over 1,000 records, read at its source rather than by its count.

| records | claims | bucket | what it is |
| ---: | ---: | --- | --- |
| 1,817 | 223 | `inherited closure` / domain-exhaustiveness | **blocked, correctly.** § 8 below |
| 1,058 | 150 | `callbacks` enumerates no invocation, census found parameter-rooted calls | **a correct refusal.** The generator proposed `callbacks: []` — "invokes no caller-supplied code" — and the implementation census found the export *does* invoke caller-supplied callables (`parameter-rooted`, `parameter-rooted-accessor`, `-element`, `-iterable`). Publishing the proposal would publish a false negative claim. Moving it means widening the generator's `callbacks` walk to enumerate those forms, which changes published contracts |
| 1,002 | 279 | `uncensused invoking form: property-access-unknown-accessor` | a `creates` census gap: a call through `obj.prop(...)` whose accessor the census cannot decide |
| 3,265 | 691 | `no recipe in corpus` | hand-authored runtime observations. The scaffold emits a module per gap that **throws until an author deletes its `UNFINISHED` guard**, because `evaluate_runtime_probes` reads a recipe that runs without emitting its marker as `CleanNonObservation` — the veto passing. 691 distinct semantic claim ids, so 691 observations, not 3,265 |
| 770 | — | `veto did not complete` | operational. 408 of them were one refused condition name; see § 10 |

**No single safe change dramatically reduces this number.** Three of the five
buckets are the system working: a correct refusal of a false claim, a deliberate
refusal to manufacture observations, and a premise that cannot be discharged.
The two that are genuinely reducible are the `property-access-unknown-accessor`
census gap (1,002) and the graph-lane edge (1,817, plus a large share of the
476,700 declines) — both real work, neither a patch.

## 10. Fixed here: a scoped export condition refused the whole probe

`plain_condition_name` admitted only `[A-Za-z0-9._-]`, so
`@tanstack/custom-condition` — a real condition of `@tanstack/solid-query` and
`@tanstack/solid-query-persist-client` — refused the probe before it ran: 408
withheld closures, 3.9% of the corpus, 108 of them `returns`, on one name.

The charset was protecting the observation package's *directory path and npm
package name*, which interpolated the condition. Those now use the candidate's
index, the observer is asked and answers in indices, and the condition appears
once as a JSON string serde escapes. `,` stays refused because the gate
identity's `reproduction-conditions:` component joins on it.

Measured on the larger row, `@tanstack/solid-query-persist-client`:

| | withheld | `harness configuration is invalid` | `returns` open |
| --- | ---: | ---: | ---: |
| before | 720 | 246 | 89 |
| after | 710 | 0 | 79 |

Ten `returns` closures closed. The other 236 did not become closures: 211 became
`Stripping types is currently unsupported for files under node_modules`, the
pinned interpreter's own limitation and the next wall. **This removed a false
blocker and revealed a true one; it is not itself a reduction.** Reporting it as
one would be reporting the reclassification as progress.

## 11. Why the largest bucket cannot be fixed where it fails

`census_inherited_dependency_closure` refuses a re-exported name whose
dependency is not "one exact replayed dependency artifact edge of this plan".
1,140 `creates` closures die there, 1,128 certifying `motion` 12.43.0. The
`motion-solidjs` row has zero `DependencyArtifact` demands across all 31 of its
demand plans, and both of its kept closures carry zero accepted edges beside
five `unaccepted-external-dependency` hazards — so in the graph lane the arm is
unreachable.

Admitting the closure on the matched graph node's own certified identity looks
sound at that arm: premise 2 has already proved from replayed bytes that the
module, export name and span are that node's, and the node is certified in the
same transaction. It was implemented and reverted. `authenticate_dependency_receipt`
discharges an inherited obligation by iterating `DependencyCompositionRequirement`s,
each of which **is** an accepted edge, matching on package, artifact case and
accepted contract digest. With no edge there is no requirement, the obligation
is recorded and never checked, and the parent publishes exactly the unprovable
propagation `59c957b6` removed.

The edge is recorded by the generator only when
`acceptedDependencies[specifier]` has the import's exact specifier text
(`packages/cli/scripts/artifact-resolution.mjs`), and in the graph lane it does
not. That is where this bucket is fixed, and it is the same root cause as the
476,700 `unaccepted-external-dependency` declines.
`an_inherited_closure_withholds_when_the_graph_node_is_not_a_closure_edge` pins
the refusal so the next attempt starts from the measurement rather than from the
arm.
