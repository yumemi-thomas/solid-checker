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

## 12. Correction to § 11: the graph lane does record accepted edges

§ 11 said the edge "is recorded by the generator only when
`acceptedDependencies[specifier]` has the import's exact specifier text, and in
the graph lane it does not", and called it the same root cause as the 476,700
declines. Measuring the spread rather than the count refutes that:

| | probes |
| --- | ---: |
| reach `census_inherited_dependency_closure` at all | 23 |
| get **past** the edge lookup for at least one export | **23** |
| refuse at the edge lookup for at least one export | 5 |

Every probe that reaches the arm finds an edge for some export. The lane records
edges; `mergeProposalDependencies` keys `proposalDependencies` by
`viaSpecifier` and carries `artifactCase` and `acceptedContractDigest`, and 23
probes prove it works. The refusal is per export, not per lane, and it is
concentrated to the point of being one package: **1,128 of the 1,140 refusals
are `motion-solidjs`, across its three rows.**

The real mechanism is a **two-deep re-export chain**. `motion-solidjs` imports
`motion`; `motion` re-exports from `motion-dom` and `motion-utils`. The arm
resolves the re-exported name's bytes to `motion-dom`'s archive — correctly,
that is where they live, and `motion-dom` is a graph node — but
`motion-solidjs`'s own closure has an edge for `motion`, its direct import, and
none for `motion-dom`. `edges.filter(package == motion-dom)` is therefore empty.

So the obligation machinery is **direct-edge only** while composition is
transitive, and that is the gap. It is still not fixable at the arm: with no
edge there is no `DependencyCompositionRequirement` to discharge against, so
admitting the closure would record an obligation nothing checks (§ 11's revert
stands). Closing it means giving a transitive composition its own requirement —
a design change in dependency composition, not a plumbing fix, and worth ~1,128
records in one package rather than the architectural win § 11 implied.

## 13. The honest unit is claims, not records

Records multiply twice over: once per export of a wide package, and once per row
of the same package (`solid1|only`, `solid2|floor`, `solid2|head` are three
rows). `motion-solidjs` alone is 3,312 of the 10,406 records — 32% — from 532
claims across three rows.

| records | claims | packages | bucket |
| ---: | ---: | ---: | --- |
| 3,265 | 691 | 61 | `no recipe in corpus` |
| 2,388 | 649 | 52 | `census refused` (other forms) |
| 1,775 | 568 | 15 | `inherited closure` |
| 1,150 | 298 | 49 | `property-access-unknown-accessor` |
| 1,058 | 150 | 27 | `callbacks` enumerates none |
| 770 | 593 | 13 | `veto did not complete` |
| **10,406** | **2,940** | **76** | |

Two thirds of the headline is multiplicity. Any target set against the record
count is a target against package width and row structure; set it against the
2,940 claims.

## 14. What blocks `@kobalte/utils`, the top of the demand list

All 124 of its withheld closures are one wall, and it is not a census gap:

```
the worker threw: Error: Stripping types is currently unsupported for files
under node_modules, for "file://<workspace>/node_modules/@kobalte/utils/src/array.ts"
```

The artifact case resolves to the package's **`src/*.ts`**, not its `dist`, and
the pinned Node refuses to strip types under `node_modules` — a deliberate
restriction with no flag to lift. The probe cannot transpile its way out either:
the witness read those exact bytes, and transpiled bytes are not them.

This is the package carrying `mergeDefaultProps` (254 consumer sites),
`callHandler` (112) and `createGenerateId` (60) — the top of § 5's worklist. Its
`reads` and `creates` domains are blocked behind an interpreter restriction, not
behind anything this repository decides.

## 15. `property-access-unknown-accessor` has no single lever either

The 1,150 records are not one gap. Each names the subject derivation the
producer did not offer, and there are twelve:

| records | claims | derivation not offered |
| ---: | ---: | --- |
| 260 | 30 | `local-binding` |
| 148 | 19 | (reads-census premise: no reviewed subject root) |
| 128 | 74 | `nested-parameter` |
| 124 | 24 | `local-binding-from-call` |
| 108 | 34 | `call-result` |
| 105 | 23 | `written-parameter` |
| 89 | 20 | `not-a-reference` |
| 47 | 18 | `this-expression` |
| 42 | 7 | `local-binding-written` |
| 21 | 21 | `ambient-declaration` |
| 20 | 1 | `module-binding` |

Each is a Type Facts producer derivation plus its census arm — the shape of ADRs
0090, 0093, 0094 and 0106, one at a time, each changing published contracts and
needing its own fixtures. The largest is 30 claims.

## 16. Conclusion

Set against the 10,406 records there is no change that reduces them
dramatically, and the number itself is two-thirds multiplicity. Set against the
2,940 claims, the work decomposes into:

- **691 claims** of hand-authored runtime observation, which the design
  deliberately refuses to generate;
- **~250 claims** across twelve producer subject derivations, ADR-sized each;
- **150 claims** where the census is correctly refusing a false `callbacks: []`;
- **568 claims** of inherited closure, concentrated in one package and needing
  transitive dependency composition;
- **593 claims** behind the probe harness, of which the largest remaining group
  is an interpreter restriction (§ 14) no change here can lift.

The measurable levers that are neither unsound nor human-authoring are small and
numerous. A programme that closes them is a sequence of ADRs against the 2,940
claims, ordered by the consumer demand in § 5 — `@kobalte/utils` and
`@solid-primitives/utils` first — not a fix to this corpus number.

## 17. Withheld is a counter of candidacy, not of failure

The one measurement that explains every number above, and the reason a target
set against the withheld count cannot be met by improving anything:

| | closure candidates | certified closures | withheld |
| --- | ---: | ---: | ---: |
| pinned 2026-09-14 | 25,464 | 10,881 | 7,114 |
| fresh 2026-09-15 | 28,476 | 10,838 | 10,406 |
| delta | **+3,012** | −43 | **+3,292** |

This branch made **3,012 more closures proposable** — ADR 0109's returns census,
`fdfeb30b` no longer letting an open re-export withdraw a package's own exports,
`59c957b6`'s inherited composition — and withheld rose by almost exactly that
number. Certified is flat.

Withheld therefore counts *proposable closures the gates have not caught up
with*. It rises when the generator gets better and falls when a census, a
recipe or a producer derivation lands — or when the generator proposes less. It
is a backlog, and at this point in the work a **growing backlog is the signature
of progress**, not of regression.

Two consequences:

- **Reducing the withheld count is not a goal that can be served by improving
  the checker in the short term.** Every improvement to proposal coverage raises
  it. The only fast way down is to propose less, which is the one change that
  would be a genuine regression.
- **The metric to steer by is `certified / candidates`** — 10,881/25,464 = 42.7%
  before, 10,838/28,476 = 38.1% after. That ratio fell because candidacy grew
  while the gates stood still, which is exactly the backlog this document
  itemizes: 691 observations, ~250 producer derivations, 150 correct refusals,
  568 in one package, 593 behind the harness.

The −43 certified closures are worth their own look; they are consistent with
the four lost receipts in § 8 and are not explained here.

## 18. Resolved: the four lost receipts of § 8

§ 8 recorded four rows the 2026-09-14 pin certified and `a8fc8cbb` refused, all
with `retained floor main has no certified exports`, and said they were not
diagnosed and that the suspect was `59c957b6`. The suspect was wrong.

`inspectRetainedCertificationFloor` required the retained case's `exports` map
to be **non-empty**. `solid-devtools@0.34.5`'s `.` entrypoint resolves, on both
its `browser` and its `import` branch, to `./dist/index_noop.js` — a zero-byte
production no-op whose sha256 is `e3b0c442…`, the digest of the empty string,
carrying `"initialization": "inert"`. Its case has `exports: {}` because the
module exports nothing. That is the correct contract for it, and the floor read
it as "nothing was certified".

The field is still required and must still be a map — absent, `null`, an array
and a string all refuse, each pinned. Only emptiness is admitted, and nothing is
weakened: integrity at that point is the document digest the catalog entry
names, the receipt's `mainDigest` binding over the same bytes, and the
`matches.length !== 1` check requiring the case to equal one exact expected
selected input in package, selection, importer, artifact and both traces. The
emptiness test added nothing to those.

| probe | before | after |
| --- | --- | --- |
| `solid-devtools@0.34.5\|solid1\|only` | refused | **certified** |
| `@tanstack/solid-start@1.168.47\|solid1\|only` | refused | **certified** |
| `@tanstack/solid-start@2.0.0-rc.2\|solid2\|floor` | refused | **certified** |
| `@tanstack/solid-start@2.0.0-rc.2\|solid2\|head` | refused | **certified** |

This is the `−43` of § 17 going the other way: a real regression on the branch,
found by the gate that exists for it, and the only number in this document that
should have moved and now has.

## 19. Correction to § 14: the type-stripping wall is liftable

§ 14 called Node's refusal to strip types under `node_modules` "a deliberate
restriction with no flag to lift" and treated `@kobalte/utils`'s 124 claims as
closed by the interpreter. That was asserted, not tested. Tested, it is wrong.

Node's refusal keys on a `/node_modules/` segment in the **resolved realpath**,
and Node realpaths what it resolves. Against the pinned Node 24.11.1, with a
package whose `exports` names `./src/index.ts`:

| | placement | result |
| --- | --- | --- |
| A | real directory at `node_modules/dep` | refuses: stripping unsupported |
| B | `node_modules/dep` → symlink to `packages/dep` | **resolves and strips** |
| C | B again, with `--preserve-symlinks` | refuses |

C is the control: the mechanism is realpath resolution and nothing else. Bare
specifier resolution, the `exports` map and the conditions all work unchanged
through the symlink.

**This preserves byte identity**, which is why it is admissible where
transpiling is not: the file the probe executes is the same file the witness
read, reached by a different path. Nothing is recompiled and no digest moves.

Three things the harness would have to handle, all identified, none unknown:

1. **The reported resolution is the realpath.** `import.meta.resolve("dep")`
   returns `…/packages/dep/src/index.ts`, not the `node_modules` spelling, so
   `verify_reported_resolution` compares against a path that no longer contains
   the private `node_modules` prefix. The harness already canonicalizes for the
   macOS `/var` → `/private/var` case, so the machinery exists.
2. **`--preserve-symlinks` must never be passed**, and that should be asserted
   rather than assumed.
3. **The workspace census and tree digest** cover "the whole private
   `node_modules`" and refuse symlinks in several places. Moving the package
   copy out of `node_modules` and linking to it changes what those watch, and
   that is the part that needs design rather than a patch.

This is the highest-value item in the document: `@kobalte/utils` carries
`mergeDefaultProps` (254 consumer sites), `callHandler` (112) and
`createGenerateId` (60) — 426 sites across three exports, at the top of § 5's
worklist, and all 124 of its withheld claims are this one wall.

## 20. The `noRecipe` bucket is roughly half a mirage

§ 13 counted 691 `no recipe in corpus` claims and § 16 called them 691
hand-authored observations. Writing three of them says that estimate is too
high, and names why.

Scaffolding `@solid-primitives/utils@7.0.0-next.4`'s `reads` gaps produced **17
serviceable candidates and 7 the scaffold could already name as unserviceable**
(`property-access-unknown-accessor`, `iteration-protocol` and `coercion` premises
the census refuses — no recipe serves one). Three of the 17 were finished by
hand, chosen by consumer demand: `entries` (37 sites), `keys` (22),
`tryOnCleanup` (37). All three artifact cases carry **no
`runtime-accessor-installation` hazard**, which is the documented precondition
for a short recipe and the same one the checked-in corvu recipes cite.

Certifying against them:

| export | before | after |
| --- | --- | --- |
| `entries` | `no recipe in corpus` | `census refused: … reasons=["callSignatureNotUnique"]` |
| `keys` | `no recipe in corpus` | `census refused: … reasons=["callSignatureNotUnique"]` |
| `tryOnCleanup` | `no recipe in corpus` | `no recipe in corpus` (claim id moved, below) |

**Two of the three are unserviceable, and only writing the recipe revealed it.**
`entries` and `keys` are `Object.entries` and `Object.keys` in the published
bytes — overloaded built-ins, so the runtime implementation transcript has no
unique call signature and the census refuses whatever a recipe observes. This is
the throwing scaffold working exactly as designed: a candidate withheld as
`no recipe` is weakened out of the plan before its demands are discharged, so a
refusal underneath stays masked until a recipe puts it back.

The scaffold's own header measured the same ratio on this package from the other
direction — 45 scaffolds, 24 census-refused, 21 worth finishing. Two of three
here agrees with it. So of the 691 `noRecipe` claims, expect **roughly half to
unmask as census gaps** rather than resolve into observations: the recipe backlog
is nearer 350 claims, and an equal number are really census work wearing a
recipe's label. Neither estimate should be trusted further than the one package
both measurements come from.

**A gotcha worth recording: claim ids are not stable across corpus changes.**
`tryOnCleanup` scaffolded as `claim:v1:sha256:7780abb2…` and certified as
`claim:v1:sha256:4168c6d8…`. The id is content-addressed over the proposal, and
admitting the `entries` and `keys` candidates changed the proposal, which moved
every id in it. Scaffold against the audit of the run the corpus will actually
be used with, and re-scaffold after any change that admits or weakens a
candidate.

Nothing from this experiment is committed to `probe-recipes/`: two of the three
address claims no recipe can serve, and the third addresses an id that no longer
exists.

## 21. Delivered: the acceptance gate reports once per package (B)

Measured on `solid-primitives-next/site`, default settings, no accepted
contracts:

| | findings | violations | uncertifiable | acceptance-gate | sites covered |
| --- | ---: | ---: | ---: | ---: | ---: |
| before | 191 | 15 | 176 | 88 | 88 |
| after | 120 | 15 | 105 | **17** | **88** |

Eighty-eight acceptance-gate findings named 17 packages; 64 of them were the
same sentence about `@solid-primitives/utils`. They now collapse to one finding
per package, anchored at the first site with every other site in
`related_locations` — 88 sites before, 88 after, in 17 findings. Total output
falls 37%; the 15 proven defects are untouched. No fixture snapshot moves,
because a package with a single unaccepted site keeps its original wording.

## 22. Scoped, not delivered: bundling the certified contracts (C)

Every top-demand package **certifies today** — `@kobalte/utils`,
`@solid-primitives/utils`, `@solidjs/meta`, `@solidjs/router`, `@corvu/utils`
are all `certified` in the 2026-09-15 corpus run. Users never see those
contracts, and this is why.

**Bundling is admissible.** `policy2_artifact_acceptance_root` binds package
name, version, integrity, requested entrypoint and the sorted export conditions
— and **no importer and no path**. One bundle therefore matches every project
that installs that exact artifact, which is the property a bundle needs.

**The authentication half is complete.** `issue_builtin_policy2_receipt` issues
a `ReceiptIssuerKind::BuiltIn` receipt, `BuiltInReceiptEntry` is the compiled-in
authority (a digest over the whole canonical receipt), and
`load_authenticated_policy2_embedded_contract` canonicalizes, authenticates,
normalizes, requires one artifact case and accepts.

**The delivery half is absent**, in four linked places:

- `first_party_bundles.rs`: `const EMBEDDED_BUNDLES: &[EmbeddedBundle] = &[]`;
- `contract_interface.rs`: `load_receipt_issued_embedded_contract` decodes the
  document, checks the receipt version, and then returns
  `Err(ReceiptAuthenticationRequired)` unconditionally — it never reaches the
  authenticated loader beside it;
- `pkg/contracts/bundled/{solid-v1,solid-v2}/bundle-index.json` both carry
  `"contracts": []`, which is what `solid-contract-bundles` regenerates from the
  empty `EMBEDDED_BUNDLES`;
- the contract documents already sitting in `pkg/contracts/bundled/solid-v1/`
  for `debounce`, `rootless` and `scheduled` are **orphans**: no index names
  them and nothing loads them. `dialect.rs`'s use of the index is a consistency
  assertion, not a delivery path.

**One design question blocks the wiring.**
`load_authenticated_policy2_embedded_contract` takes
`expected: &Policy2ReceiptBindings`, which carries `importer`, `specifier` and
`resolved_import_root` — project-specific values a compiled-in bundle cannot
know, and must not assert. So the expected bindings have to be built from the
*consumer's* own resolution and the bundle selected by matching
`artifact_acceptance_root`, which puts the lookup in the admission path
(`admitted_project_artifacts`) rather than in the bundle loader. That is a
decision about where bundle selection lives, not a patch.

**One product question follows.** A bundle is pinned to an exact version *and*
integrity. Shipping them means the checker carries a table of (package, version)
→ contract that helps users on those versions and nobody else, with a refresh
cadence to decide. That is a maintenance commitment, not a one-off.

## 23. Correction to § 22: `first_party_bundles` is not where C goes

§ 22 named `EMBEDDED_BUNDLES` and `load_receipt_issued_embedded_contract` as
C's four blocking pieces. Three of the four are real; the *location* is wrong,
and wiring that loader would have been a mistake.

`bundled_first_party_contract_index` says so in its own doc comment:

> Retired bundle-loader compatibility seam. Ordinary native, daemon and WASM
> analysis no longer calls this function (ADR 0027). Both source lists are
> empty; these historical checks must not be described as active runtime
> authentication.

`solid1_bundles_with_measurements` validates the Phase 14 authority documents
and then returns `Ok(Vec::new())` unconditionally, and
`policy1_checked_corpora_have_no_active_receipt_issued_bundles` pins both
generators empty. The path is decommissioned, not unfinished.

**ADR 0027 also says where third-party contracts belong.** Its subject is the
built-in runtime foundation — `solid-js`, `@solidjs/signals`, `@solidjs/web` —
which ordinary analysis takes from the *dialect*, never from a package
contract. Its first paragraph draws the line the other way for everything else:
"Package contracts describe external packages." So `@kobalte/utils` and
`@solid-primitives/utils` are not first-party bundles and must not be delivered
through a seam built for core.

**The correct location is the one the local tier already uses**: a compiled-in
source of accepted contracts feeding `AcceptedContractIndex` beside
`discovered_catalog_paths`, with applicability decided by
`admitted_project_artifacts` — which already recomputes
`policy2_artifact_acceptance_root` from the *installed* identity and refuses
when it does not reproduce the signed root. That is the importer-free match a
bundle needs, and it is already written.

So C's remaining work is:

1. a compiled-in accepted-contract tier that feeds the same index the local
   tier feeds, selected by `admitted_project_artifacts` (authentication is
   `load_authenticated_policy2_embedded_contract`, which is complete and
   tested);
2. a generation pipeline that takes a corpus-certified contract, issues a
   built-in receipt for the current verifier build, and compiles in the
   document, receipt, bindings and an independently attested entry digest;
3. the version/refresh policy, unchanged from § 22.

Note for (2): the receipt is build-pinned — `authenticate_policy2_receipt`
compares `entry.verifier_build_digest` to the receipt payload's — so bundles are
re-issued per checker build. The legacy conformance corpus cannot supply them;
it is policy-1-era material, which is what the pinning test's name records.

## 24. Correction to § 14 and § 19, and the actual wall

§ 14 said `@kobalte/utils`'s 124 withheld claims are Node's type-stripping
refusal, and § 19 said lifting it would reopen the package's 426 consumer call
sites. The first is true of the claims. The second is **wrong**, and testing the
symlink against the real package is what showed it.

**The symlink fix works.** `@kobalte/utils@0.9.2` installed from the registry,
pinned Node 24.11.1:

| | `.` → `dist/index.js` | `./src/props.ts` |
| --- | --- | --- |
| real directory in `node_modules` | resolves | **refuses**: stripping unsupported |
| symlinked to a realpath outside | resolves | **resolves** |

**But it unblocks entrypoints no consumer imports.** All 22 of the package's
type-stripping failures are `./src/*.ts` artifact cases. A consumer writes
`import { mergeDefaultProps } from "@kobalte/utils"`, which is `.`, and `.`
resolves to `dist/index.js`, which needs no stripping and never did.

**The generated contract has no `.` entrypoint at all.** It carries 20, every
one of them `./src/*.ts`. The refusal audit says why:

```
entrypoint ".", stage artifact-case, class dependency-composition:
accepted dependency @solid-primitives/keyed has no exact runtime binding
for export Key
```

`dist/index.js` opens with `export { Key } from '@solid-primitives/keyed'`. One
re-exported name with no runtime binding in its dependency's contract refuses
the whole public entrypoint, and with it every export the package has.

**This is the wall, and it is not local to `@kobalte/utils`.** Across the
corpus's 418 probes there are 341 artifact-case refusals — 173
`dependency-composition`, 168 `published-artifact`. **Ninety-seven of them are
on `.`, across 40 packages**, and the single largest cause is this one:

| refusals on `.` | cause |
| ---: | --- |
| 46 | `accepted dependency <dep> has no exact runtime binding for export <name>` |
| 26 | `solid-checker:unresolved-dependency-module=<@tanstack/…>` |
| 7 | `@solidjs/signals has no exact runtime binding for export $PROXY` |
| 6 | `resolved target <root>/dist/index.jsx is not a file` |

Seven of those 40 packages are ones real consumers import, and they carry
**1,947 call sites** — `@kobalte/utils` 942 and `@solid-primitives/utils` 820
between them.

So the reason a user gets no useful feedback about these packages is neither
closure depth, nor recipes, nor the interpreter: **the public entrypoint never
produces a contract**, because one re-exported name in it cannot be bound. Every
closure measurement in this document is downstream of that, and the `./src/*`
cases those measurements describe are entrypoints nobody imports.

## 25. The `Key` refusal, diagnosed: a core-runtime asymmetry across three censuses

§ 24 read `accepted dependency @solid-primitives/keyed has no exact runtime
binding for export Key` as a binding failure. It is not one, and the message
overstates on both halves: `bindExport` fails whenever
`acceptedDependencies[specifier]?.exports?.[name]?.[axis]` is falsy, which
includes **the dependency not being supplied at all**, and then reports that an
"accepted dependency" lacks a binding.

Resolving `@solid-primitives/keyed@1.5.3` on its own answers `Key` with a valid
runtime binding, so the second half is false. Supplying `keyed` by hand moves
the refusal to `@solid-primitives/map`'s `ReactiveMap`, then to
`@solid-primitives/utils`'s `access` — `@kobalte/utils`'s `.` re-exports from
seven packages and needs all seven. The chain terminates at:

```
@kobalte/utils  ->  @solid-primitives/utils  ->  solid-js/web   (isServer)
```

**`solid-js/web` has no package contract by design** (ADR 0027), so a binding
demand on it can never be satisfied. `canonicalClosure` already exempts core
specifiers — "the built-in runtime foundation is not an unknown dependency" —
and `bindExport` does not. That asymmetry is the root, and it is what refused
`@solid-primitives/utils@6.4.1|solid1|only`'s `.` in the corpus, verbatim:
`accepted dependency solid-js/web has no exact runtime binding for export
isServer`.

**Exempting core in `bindExport` is necessary and not sufficient.** With it,
resolved in isolation with all seven dependencies supplied:

| | before | after |
| --- | --- | --- |
| `@solid-primitives/utils` | refused | resolves, 39 exports |
| `@kobalte/utils` `.` | refused | resolves, **59 exports** |
| `mergeDefaultProps`, `callHandler`, `createGenerateId`, `Key` | — | present |
| `isServer` | — | absent, core-owned |

An unbound name is already the handled path — `if (!runtimeTarget ||
!declarationTarget) continue` drops it and keeps every other export — which is
ADR 0027's "missing native behavior stays unknown".

But the A/B on the real row shows why that change cannot ship alone.
`@solid-primitives/utils@6.4.1|solid1|only`, same probe, fix stashed and
restored:

| | `.` refusal |
| --- | --- |
| without | `accepted dependency solid-js/web has no exact runtime binding for export isServer` (class `dependency-composition`) |
| with | `contract identity does not match the resolved import: resolved artifact has no exact runtime/declaration binding for export "isServer"` (class `published-artifact`) |

The refusal moves from unsatisfiable to a **disagreement between censuses**:
the JS resolver now omits `isServer`, while Rust's `bind_exports`
(`artifact_resolution.rs:989`) still requires every name in the emitted
document's export map to bind, and the emitter still puts `isServer` there.
`coreRuntimeSpecifier`'s own comment says these censuses "must agree byte for
byte", so half the change is worse than none. It was implemented, measured both
ways, and reverted.

**The complete fix is symmetric across three places**: exempt core specifiers in
`bindExport`, mirror it in Rust's `bind_exports`, and stop the emitter naming a
core-owned re-export in the document at all — because under ADR 0027 the package
has no standing to claim anything about it. The third is a change to what a
published contract *contains*, so it needs corpus verification rather than a
patch.

Worth noting what this does not explain: `@kobalte/utils` end-to-end still
refuses at `keyed`/`Key` even with the graph lane on, because the lane does not
supply `@solid-primitives/keyed` as an accepted dependency for the root's `.`
case. That is a second, independent gap on the same entrypoint.

## 26. There is no keyed graph-lane gap; the refusal that named it is stale

§ 25 closed by recording a "second, independent gap": `@kobalte/utils`'s `.`
still refusing at `@solid-primitives/keyed`/`Key` with the graph lane on,
because the lane did not supply that dependency. Instrumenting
`mergeProposalDependencies` says otherwise. For the `.` case the lane supplies
all seven of the entrypoint's re-export dependencies, and `keyed` arrives with
its exports and with `Key` among them:

```
MERGE n=7 [["@solid-primitives/event-listener",11,false],
           ["@solid-primitives/keyed",6,true],
           ["@solid-primitives/map",4,false], ["@solid-primitives/media",6,false],
           ["@solid-primitives/props",6,false], ["@solid-primitives/refs",8,false],
           ["@solid-primitives/utils",40,false]]
```

(count of resolution exports, then whether `Key` is one of them.)
`@solid-primitives/keyed@1.5.3` also generates cleanly on its own — one `.`
entrypoint, `Key` among six exports, zero refusals — so nothing about it is
missing anywhere.

**The `Key` message is a pass-1 artifact.** `refusals.json` is written by the
initial generation, before any dependency is supplied; the graph lane's own
outcome is recorded separately, in `graphPreparation.entrypointRecovery`. Read
there, `.` is requested — `cases` carries it under both condition sets — and is
absent from `expectedCases` and `publishedCases`. What the recovery actually
refused is a different case entirely:

```
caseRefusals: ./src/scroll-into-view.ts, stage witness-acquisition,
family recursive-value-shape — "parameter-rooted read lacks positive
original-input identity"

combinedRefusal: published graph case-set finalization failed: Type Facts
certification failed for graph node @kobalte/utils@0.9.2
(./src/scroll-into-view.ts)
```

So one `./src/*` case failing Type Facts certification aborts the **whole**
case-set finalization, and `.` never publishes — while the refusal census the
operator reads still names `keyed`/`Key` from a pass that had no dependencies at
all.

This is the third refusal message today that named the wrong subject: "accepted
dependency X has no exact runtime binding" when X was never supplied (§ 25),
"the re-exported name's dependency is not one exact replayed edge" for a
transitive chain (§ 12), and now a stale pass-1 census surviving into the
published refusals. Each cost hours. The pattern is worth fixing on its own
terms: **a refusal census that a later pass has superseded should say so**, and
`this is what pass 1 saw` is different information from `this is why the
transaction failed`.

The real remaining question for `@kobalte/utils` is therefore whether one case's
`recursive-value-shape` refusal should abort the case set, or whether the set
should publish the cases that did certify. That is a different investigation
from the one § 25 pointed at, and nothing about `keyed` is part of it.

## 27. The case set already publishes what certified; `.` is refused on its merits

§ 26 left the question "should one case's `recursive-value-shape` refusal abort
the case set, or should the set publish the cases that did certify". Measured,
the set already publishes what certified, and ADR 0070's machinery does exactly
what it says. Instrumenting `certifyRecoverableCaseSelection` for
`@kobalte/utils@0.9.2|solid1|only`:

```
SUBDIV cases=24 accepted=[0..16,18,19,22,23] refusals=[17,20,21]
strategy: independent-prepared-selection
publishedCases: 21
```

Twenty-one of twenty-four cases publish. The combined attempt refuses, the
retained baseline refuses, `existingPublication === false` admits a subset, the
prepared set is subdivided, the accepted union is independently re-certified,
and it is published. No case that certified is lost, and the final publication
did not fail — there was no `FINALFAIL`.

**The three that refuse, refuse on their own evidence**, all in the same family:

| case | family | reason |
| --- | --- | --- |
| `./src/scroll-into-view.ts` | `recursive-value-shape` | parameter-rooted read lacks positive original-input identity |
| `.` (`import`) | `recursive-value-shape` | `contains`: operation value path is locally open (complete=true, presence=Absent, callability=Unknown, reasons=[]) |
| `.` (`import, solid`) | `recursive-value-shape` | as above |

So `@kobalte/utils`'s `.` entrypoint is not blocked by dependency composition,
by the graph lane, by `keyed`, or by case-set publication policy. It is refused
by Type Facts during live graph export-value verification, on export
`contains`, because an operation value path stays locally open with
`presence=Absent, callability=Unknown` and **no reasons recorded**.

That empty `reasons=[]` is the next thread: a locally open value path that
cannot say why it is open is the same class of unhelpful refusal as the three
misleading messages in § 26, and it is the only thing now standing between this
package's 942 consumer call sites and a contract.

**Nothing was changed for this section.** The instruction it answers was to make
the case set publish what certified; it already does, and the 21-case
publication is the proof. Writing a fix would have meant changing behaviour that
is correct to chase a symptom whose cause is three layers away.

## 28. Re-measured: how much feedback a consumer gets today

The question the day opened with, asked again at the end of it. § 15 of
`2026-09-14-which-closures-change-a-consumer-finding.md` answered it at
**14.3%** of call sites with any stated operation and **1.5%** with an owner
requirement. This section repeats that census against today's binary.

### Method, and one correction to § 15's

Demand is the pinned recensus (`2026-09-14-consumer-demand-recensus.json`):
1,958 in-corpus call sites over 159 (package, export) pairs. Supply is a fresh
certification of each demanded package through the ordinary benchmark lane with
the freshly built debug checker, classifying every demanded export against the
emitted document as `operations`, `closed-empty`, `degenerate` or `absent`.

One rule differs from § 15, and it matters. `@kobalte/utils` declares two
entrypoints **and a wildcard**, so its certification emits twenty documents
named `./src/array.ts`, `./src/dom.ts` and so on. § 15 credited those summaries
to consumers who wrote `import { contains } from "@kobalte/utils"`. They cannot
be: `policy2_artifact_acceptance_root` binds the **entrypoint**, so a summary
published at `./src/dom.ts` does not answer an import of `.`. This census counts
an export only at an entrypoint a consumer can actually name — the root and the
package's declared subpaths, never a wildcard-reached source path.

The correction does not move § 15's headline, because those entrypoints stated
nothing then. It moves the shape of the silence: `@kobalte/utils`' 942 sites
were scored `closed-empty`/`degenerate` and are really `absent`.

### The result

| package | version | sites | stated operation | owner requirement |
| --- | --- | ---: | --- | ---: |
| `@kobalte/utils` | 0.9.2 | 942 | 0 (0.0%) — `.` refuses | 0 |
| `@solid-primitives/utils` | 6.4.1 | 820 | **257 (31.3%)** | 20 |
| `@kobalte/solidbase` | 0.6.13 | 60 | 0 | 0 |
| `@solid-primitives/rootless` | 1.5.4 | 39 | 0 | 0 |
| `@solidjs/start` | 2.0.3 | 24 | 0 | 0 |
| `@solid-primitives/platform` | 0.2.1 | 18 | 0 | 0 |
| `@kobalte/core` | 0.13.13 | 16 | 0 | 0 |
| `@solid-primitives/trigger` | 1.2.4 | 9 | 0 | 0 |
| `@solid-primitives/marker` | 0.2.2 | 8 | **8 (100%)** | 4 |
| `@solid-primitives/scheduled` | 1.5.3 | 5 | **5 (100%)** | 5 |
| `@solid-primitives/memo`, `keyed`, `storage`, `@solidjs/meta` | — | 9 | 0 | 0 |
| `permission`, `tween`, `timer`, `context` | — | 10 | not measurable | — |

**270 of 1,890 call sites — 14.3%. Twenty-nine — 1.5% — carry an owner
requirement.** Both numbers are *identical* to § 15, to the site.

By state, what a consumer's import meets today:

| state | sites | share |
| --- | ---: | ---: |
| an operation is stated | 270 | 14.3% |
| determined: the export states nothing | 556 | 29.4% |
| degenerate: nothing was determined | 118 | 6.2% |
| absent: the entrypoint refused, so no statement exists | 1,006 | 53.2% |

### What today's work did move, and why it is not in this number

Three things moved, none of them into the coverage column:

- **`@solid-primitives/utils`' `.` case stopped refusing** (§ 25). In the
  composed lane it went from refused to emitting 39 exports; its demanded
  exports went from 19 `degenerate` to 2. That is the same 31.3%, with 17
  exports moving from *nothing determined* to *determined to state nothing* —
  an honesty gain, not a coverage gain.
- **`@kobalte/utils` now produces operations for 9 of its 41 demanded
  exports** — `createGenerateId` (60 sites), `focusWithoutScrolling` (34),
  `contains` (24), `snapValueToStep`, `scrollIntoViewport`,
  `createGlobalListeners`, `addItemToArray`, `getAllTabbableIn`, `isFocusable`:
  **136 sites, one of them an owner requirement**. Every one of those summaries
  is published at a `./src/*.ts` entrypoint that no consumer imports. The `.`
  case that would carry them refuses.
- **Consumer-facing noise fell 37%** (§ 21's collapse), with no information
  lost. That is feedback quality at the sites that already get a finding, and
  the census cannot see it.

### The size of the one remaining blocker

If `@kobalte/utils`' `.` case certified, those 136 sites would move — the corpus
goes **14.3% → 21.5%** and owner requirements **1.5% → 1.6%**, from a single
package. The content already exists; only the entrypoint that consumers name is
missing.

That case refuses for two reasons, both recorded: `Key` has no exact runtime
binding from the accepted `@solid-primitives/keyed` (§ 26 — the refusal is
pass 1's, and the published `keyed` document does carry `Key`), and `contains`
holds an operation value path locally open with `reasons=[]` (§ 27). One
entrypoint, two named causes, 942 call sites — 48% of all demand in the corpus.

### The honest headline

**No. Coverage has not moved: 14.3% of call sites get a statement and 1.5% can
raise a finding, exactly as yesterday** — and that is still only reachable after
the acceptance gate of § 4, which no consumer passes at all today. What moved is
that the silence is better-determined, the noise is smaller, and the largest
single blocker is now sized, named, and one entrypoint wide.

## 29. `contains` is unblocked; the wall behind it is the census's own model

§ 28 sized the last blocker: certify `@kobalte/utils`' `.` case and the corpus
goes 14.3% → 21.5% on content that already exists. This section takes it apart.

### The reproduction

The benchmark's 24-case run is not needed. One entrypoint reproduces it in about
a minute, and the certification refuses at the same demand:

```
contract certify --package-root <installed @kobalte/utils> --integrity <pinned>
  --entrypoint '.' --dependency-graph-lane --issuer-configuration <local>
```

Without an issuer configuration the pipeline refuses at `receipt-issuance`
*before* witness acquisition ever runs — `stageDurationsMs` carries only
`artifactAcquisition` and `proposalGeneration` — so three earlier runs of mine
that "passed" had proved nothing at all. A scoped certification harness needs
the issuer, or it is measuring the absence of one.

### Blocker 1: an unnamed alternative is not alternative 0 — fixed

`contains(parent: Node | undefined, child: Node | null)` reads
`parent.contains`. Instrumenting the refusal prints the census the verifier
actually had:

```
value = "Node | undefined", alternatives = [0, 1]
  alt=0 path=[contains] presence=Absent   callability=Unknown  complete=true subtree=true
  alt=1 path=[contains] presence=Required callability=Callable complete=true subtree=true
```

**Alternative 0 is `undefined`.** The demand names no alternative — the IR's
recorded shape is a parameter member, not a choice — and `translate_value_path`
initialised `alternative = 0` for both cases, so `undefined` refuted a member
read the implementation performs on the `Node` arm, and with it the entrypoint
942 consumer call sites import.

The alternative is now `Option<usize>`. Named, it selects that fact as before.
Unnamed, every alternative carrying the path is verified, and one that proves
the member *absent* is not a counterexample: a value of that alternative cannot
be the one the operation read. Every remaining alternative still has to carry
the path closed and with the demanded callability, and a path absent on all of
them refuses — saying so, rather than reporting a proved absence as
`locally open (… reasons=[])`. That was § 27's complaint, and the answer is that
`reasons=[]` was never missing: the producer *refuses* an absence fact carrying
any positive or open field, so an empty reason list is what absence is required
to look like. The message was reading an absence as an opening.

Applied to all three sites that shared the conflation — operation value,
exported value, selected-call recursive — not only the one the corpus hit
(`e41d8dbc`). 534 backend lib tests, the three process suites, coverage (96
projects, 555 findings) and the contract corpus (100 fixtures) are unmoved.

### Blocker 2: a read through a reassigned parameter

With `contains` cleared, `.` refuses on `scrollIntoViewport`:

> `parameter-rooted read lacks positive original-input identity`

That refusal is **correct, and no message fix helps it**. The export reassigns
its own parameter:

```ts
while (targetElement && scrollParent && …) {
  scrollIntoView(scrollParent, targetElement);
  targetElement = scrollParent;          // the parameter, rewritten
  scrollParent = getScrollParent(targetElement);
}
```

`unwritten_parameter_binding` reads the producer's `unwrittenParameters`, a
whole-function fact: this parameter is written, so there is no binding to stand
on. The read that actually fails is on the *other* branch, where no
reassignment happens — but Type Facts publishes no write/read ordering, so the
verifier cannot distinguish "written somewhere in the body" from "written before
this read". Closing this needs a producer fact that does not exist today.

### Blocker 3: a path below a nested union cannot be addressed at all

Bypassing blocker 2 as an experiment (never committed) exposes the third, on the
same export:

> `operation value path is absent from the signature census (alternative=None,
> path=[containingElement, scrollIntoView])`

from `opts?.containingElement?.scrollIntoView?.({ block: "center" })`. The
census for `opts: ScrollIntoViewportOpts | undefined`:

```
  alt=0 path=[containingElement] presence=Absent   complete=true subtree=true
  alt=1 path=[containingElement] presence=Optional complete=true subtree=true
```

and **nothing below it**. `containingElement?: Element` is itself
`Element | undefined` — a union — and a `CallablePathFact` carries exactly one
`alternative`, the *root's*. There is no way to address a member below a nested
union, so the producer stops there and reports the subtree enumerated.

This is a limit of the census model, not of this package: **any export whose
analysis records an access path crossing a second union can never certify that
path**, however complete the evidence below it is. It is the first time this
has been written down, and it is worth more than this one package.

### What this leaves

`@kobalte/utils`' `.` needs `scrollIntoViewport`, and `scrollIntoViewport`
carries two claims the evidence cannot reach. Neither is a defect in the
verifier: both are the proposal claiming more than the model can carry. The
options are a design decision, not a fix:

- **Weaken the proposal.** A read whose path the census cannot address should be
  proposed with the shorter path, or with none — the ladder already exists
  (`path: (paths.len() == 1).then(…)` publishes "read through this parameter"
  when the accesses disagree). Smallest change, and it makes the contract say
  less rather than nothing.
- **Withhold at operation granularity.** Certification already withholds
  *closure candidates* and already re-certifies a subset of *cases*; the missing
  rung is dropping one unprovable operation and re-emitting the export. That
  changes the document bytes, so it needs the same independent re-certification
  the case subdivision does.
- **Extend the producer** with write/read ordering (blocker 2 only). Largest,
  and it is a Type Facts protocol change.

The prize is unchanged and now sits behind exactly one export: 136 sites — 132
without `scrollIntoViewport`'s own 4 — taking the corpus from 14.3% to about
21.5%.

## 30. Confirmed: operation-granularity withholding needs no regeneration

§ 29 left one question before any of its three options could be sized: can a
weakened document be re-emitted from the existing proposal, or does dropping an
operation force a regeneration pass? **It needs no regeneration**, and the
evidence is three facts already in the tree.

**1. The native planner is handed the document and nothing else.**
`executeNativeCertification` writes one request whose planning carries
`proposal: generated.output` — no `.proposal.json` plan sidecar, no claim list.
Demands are scheduled from the document's own summaries. Drop an operation
there and its demand is never scheduled.

**2. Subset publication is already a JavaScript projection of that document.**
`certifyIndependentCaseSelection`'s trial does exactly this:

```js
output = join(trial, "proposal.json");
writeFileSync(output, JSON.stringify(projectProposalCases(
  JSON.parse(readFileSync(generated.output, "utf8")), inputs)));
…
generated: { ...generated, output, certificationInputs: inputs }
```

`projectProposalCases` filters `entrypoints` and keeps only the referenced
`summaries`. Nothing is re-analyzed; the trial path becomes the planning's
`proposal`. Operation granularity is the same move one level finer.

**3. Summary contents are editable — ids are opaque keys.** The emitter names a
summary `summary-<sha256 of its bytes>`, but the *decoder* never re-verifies
that: its own fixtures key summaries `"plain"`, `"fn"`, `"signal-pair"`. The
only structural rule is that every summary must be referenced — an unused one is
refused.

### The soundness rule is enforced by the format, not by discipline

§ 29 argued that dropping an operation must *open* its claim domain, or the
contract would assert "this export performs no reads". The wire format already
decides this, in `knowledge(items, closed)`:

| items | closed | meaning |
| --- | --- | --- |
| absent | false | **Unknown** — the domain says nothing |
| present | false | Partial — these are known, more may exist |
| present | true | Complete — the enumeration is exhaustive |
| **empty** | **false** | refused: "open domain has an empty collection" |
| **absent** | **true** | refused: "closed domain omits its collection" |

So "drop the operation and open the domain" is the representable transition, and
a drop that carelessly left the domain in `closed` is refused by the decoder.
`require_operation` in the IR validator refuses a dangling id from a domain
list, an edge or a trigger, so a partial projection cannot corrupt a document
either — it fails loudly.

### What remains before writing it

- The boundary being crossed is deliberate, not an oversight: *"Select whole
  unaccepted artifact cases, never individual claims."* Crossing it is an
  ADR-level decision.
- The projection needs a reference-integrity pass — remove the operation from
  its domain list, its edges, and anything triggering on it — which the
  validator checks rather than trusts.
- Each weakened attempt costs a fresh native transaction, as case subdivision
  already does, and per-operation fan-out may want a budget like
  `RECOVERY_GRAPH_CASE_BUDGET`'s 1,024 cases.

**Estimate: a day for the mechanism plus measurement, not a week.** The
expensive part — projecting a document and re-certifying it independently —
already exists and is already trusted.

## 31. Shipped: the operation is withdrawn, not the artifact case

§ 30 confirmed the mechanism was a day's work. This is what it turned out to be,
and what it bought.

### The rung

ADR 0036 gave the transaction a loop: acquire, and if the census refuses,
withdraw the candidate it names *by name*, re-plan, and try again. That loop
only knew about **closure candidates**. An operation whose own stated positive
fact could not be verified had nothing smaller to give up, so
`census_refusal_withholding` returned nothing and the whole artifact case
refused — taking every other export in it.

`positive_fact_refusal_withholding` is the same conversion for the positive
half, and `ExportSemantics::withhold_operations` is the weakening it feeds:

- the operation is removed, and **the domain that listed it is opened**,
  because a shorter list still marked closed asserts an absence the census never
  established — a *stronger* claim than the one being withdrawn;
- the withdrawal is transitive within the export: an operation triggered by a
  withdrawn one, an edge touching one, a callback invocation naming one or
  sourced from its output, and `composed_from` provenance pointing at one;
- a domain emptied by the withdrawal becomes `Unknown`, never an empty
  `Partial`.

That last rule was not foresight. The first implementation left an emptied
domain as `Partial([])` and two existing tests failed with *"partial knowledge
must contain positive evidence"* — the wire format refusing it exactly as § 30
predicted it would. The machine caught the error, which is the whole argument
for putting the rule in the format rather than in the author's head.

### What cannot change

**No document that certified before certifies differently.** The new withholding
is consulted only after `census_refusal_withholding` yields nothing *and* the
transaction was already returning an error. There is no path where it makes a
certified contract weaker; it only turns a refusal into a narrower contract.

**Every withdrawal is reported**: `solid-checker:withheld-operation=` on the
native stdout, `withheldOperations` in the CLI audit, beside `withheldClosures`.
A certified contract weaker than the proposal it came from has to say so, or the
weakening is indistinguishable from a generator that never made the claim.

### Two pins moved, and what they still pin

`unwritten_parameter_read_certification_binds_published_source` and
`duplicate_installation_graph_contexts_preserve_exact_parameter_reads` asserted
that a reassigned parameter *refuses*. It no longer does. What they were
protecting — that an unprovable parameter-rooted read is never published as a
proven fact — is unchanged, and is now asserted against the published document
and the withheld record instead of against a refusal message. That is a stronger
assertion than the old one: it reads what was published rather than what was
printed.

### The measurement

`@kobalte/utils@0.9.2|solid1|only`, through certification with the graph lane
and entrypoint recovery:

| | before | after |
| --- | --- | --- |
| cases published | 21 of 24 | **24 of 24** |
| `rootCertified` | false | **true** |
| case refusals | 3 | **0** |
| `.` exports published | — | **59** |
| demanded exports stating an operation | 0 of 41 | **11 of 41** |
| call sites with a stated operation | 0 / 942 | **312 / 942 (33.1%)** |
| call sites with an owner requirement | 0 | **2** (`createGlobalListeners`) |

`scrollIntoViewport` publishes with an empty operation list and an *open* reads
domain — the export is there, stating one claim fewer, which is what a consumer
needs and what a refusal denied them.

Against § 28's denominator of 1,890 measured call sites, the corpus moves from
**270 (14.3%)** to **582 (30.8%)**, and owner requirements from 29 to 31. The
second figure is honest but partial: kobalte's half is measured against its
*certified catalog*, while the other packages' 270 was measured against emitted
proposals. A full re-census against certified catalogs is the next measurement,
and it can only move the number up, since certification is where `.` entrypoints
like this one appear.

### What is unchanged

Both model limits from § 29 are still exactly where they were. A parameter the
body reassigns still cannot carry a proved read, and a path below a nested union
still cannot be addressed. What changed is the *consequence*: those claims are
now withdrawn from the document instead of destroying it. That is the right
outcome, and it is not a proof — the contract says less, and says that it says
less.

## 32. Correction to § 25: one rule, three places, two clauses

§ 25 said the core-runtime fix had to be symmetric across **three censuses** —
the emitted document, the JS resolver's `bindExport`, and Rust's `bind_exports`.
That framing was wrong twice over, and `822c2287` shipped on it.

Running `make ecosystem-regression` for the first time after that commit found
six certified rows that had stopped certifying. Fixing them one refusal at a
time looked like divergence — each census aligned exposed another — and I
proposed reverting the whole change. That was the wrong read. The chase
converged as soon as the actual invariant was stated:

> **A package's export surface is computed in three places — the emitter
> (`export_binds_core_runtime`), the JS resolver (`bindExport`), and the archive
> replay (`exported_names`). Each drops a name re-exported straight from the
> built-in runtime foundation, *unless the package is itself the foundation*.**

`822c2287` implemented the first clause in two of the three places and the
second clause nowhere. Every "new census" was one of those three missing half
the rule:

| symptom | which half was missing |
| --- | --- |
| `@solid-primitives/utils`: `replayedOnly ["isServer"]` | the replay had neither clause |
| `@solidjs/start`: `replayedOnly ["mount"]` | the replay did not follow a *local* re-export chain into core (`export { mount } from "./mount.js"`, and that file re-exports `hydrate` from `solid-js/web`) |
| `corvu`, `@corvu/popover`: 13 of `solid-js`'s own exports dropped | the replay had clause one but not clause two — I introduced this while fixing the row above |
| `@tanstack/solid-db`, `motion-solidjs`, `@corvu/drawer`: `suppliedOnly ["isServer"]` | the resolver ran the core check *after* the accepted-dependency lookup, and a graph lane supplies `solid-js/web` as an accepted node whenever the graph contains one, so the lookup succeeded first |
| `solid-js` `./web`: 72 replayed vs 59 supplied | the resolver had no clause two |

`bind_exports` — the third place in § 25's list — is not a fourth computation.
It is a *consistency check* on the other three, deliberately strict, and it
never needed changing.

A sixth symptom was not a census at all: `solid-js` and `@solidjs/web` stopped
being *queued* for certification, because removing their unsatisfiable
core-binding refusals removed the `dependency-composition` class that was their
only route into the benchmark's certification queue. Once every surface agreed,
they certified through the ordinary route and the symptom disappeared with the
others.

### Dependency composition was a real gap, in the other feature

One of the six was not the core fix at all. `authenticate_dependency_receipt`
re-derives a dependency's weakening from its accepted candidate and compares
digests; it knew only about withheld *closures*, so a dependency that had also
withdrawn an **operation** (§ 31) produced a digest for a document it never
certified. The withheld operations now travel the edge beside the closures and
are re-derived first, in the order the node applied them. That gap was created
by § 31 and found by this gate.

### The result

`make ecosystem-regression`, against the 2026-09-14 pinned baseline:

| | baseline | after |
| --- | ---: | ---: |
| certified rows | 381 | **388** |
| receipts lost | — | **0** |
| receipts gained | — | **+7** |
| wall | 10.5 min | 11.7 min |

The seven gained include `solid-js@1.9.14`, `solid-js@2.0.0-rc.3` and
`@solidjs/web@2.0.0-rc.3` — the three the fix had cost — plus
`@solid-primitives/flux-store`, `intersection-observer`, `local-store`, `until`
and `@solidjs/router`.

### Two things worth keeping

**Run `ecosystem-regression` after a change to the certifier, the producer, the
generator or the runner.** Its own header says so, and every gate I did run
after `822c2287` — coverage, the contract corpus, every Rust suite — was green
while six receipts were gone. The three-row corpus cannot see a receipt lost
elsewhere.

**"Each fix reveals another" is not evidence of divergence.** It is evidence the
invariant has not been stated yet. Five symptoms, one rule, two clauses; the
right move was to name the rule, not to revert the change.
