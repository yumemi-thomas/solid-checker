# The `callbacks` census: the largest discarded domain in the corpus, admitted

- **Status:** implemented. `ClaimDomain::Callbacks` is admitted to
  `PROPOSABLE` for the empty enumeration, with its own implementation census and
  synthesized veto. **+34 certified closures** corpus-wide with no row losing
  one (§ 7), and **8 SC9005 findings discharged** across 3 of 14 catalog-bearing
  fixtures (§ 8).
- **Date:** 2026-09-12.
- **Question:** `reads` cannot close in bulk and its demand cannot be scoped
  (§ 13 of the SC9005 design). Which domain is next, and what would it cost?

## 1. `callbacks` was the largest candidate domain in the corpus, and all of it was discarded

Measured over all 418 rows, release binary, generation only, 381 valid proposal
plans. `ClaimDomain::PROPOSABLE` admits three domains; the generator's candidates
for the others stay in the plan sidecar, so they can be counted:

| domain | candidates | share | admitted? |
| --- | ---: | ---: | --- |
| **`callbacks`** | **1,111** | **37.0%** | **no — discarded** |
| `reads` | 990 | 33.0% | yes |
| `creates` | 516 | 17.2% | yes |
| `returns` | 267 | 8.9% | yes |
| `owner-productions` | 62 | 2.1% | no — discarded |
| `object-properties` / `tuple-items` | 54 | 1.8% | no — discarded |
| total | 3,000 | | |

Admitted 1,773; discarded 1,227, of which 1,111 are `callbacks`. The unresolved side
agrees exactly: 7,839 `unresolvedClaims` + 1,111 candidates = 8,950 export
occurrences.

Concentrated where the product's use case is: `@kobalte/core` 140,
`@solid-devtools/shared` 123, `@solid-primitives/utils` 83 × 2, `@corvu/utils` 40.

For comparison, the freshly pinned corpus contains **24** `reads` closures in total,
every one from `@solid-primitives/utils@7.0.0-next.4`, the single package that has
received hand-authored recipes.

## 2. Why this domain and not another

Of the six domains outside `PROPOSABLE`, the rule-demand proposal's own answer names
`writes`, `invalidates`, `throws` and `disposals` as having *"no behavioral consumer
in the current rule path"* — deletion candidates rather than work. That leaves
`callbacks` and `cleanups`.

`callbacks` has the longest consumption path in the demand matrix —
`project_callbacks` → `source_discovery` → `interproc` invocation edges →
`execution_role::allowed_callback_spans` → `owners::owner_callback_edges` — and is
itself an SC9005 source: *"potentially callable arguments can then create SC9005."*

§ 80.3 already owes it work: 189 withheld `creates` closures are *"a call through a
caller-supplied callable: unprovable in this domain by design, because the semantic
model puts the caller's function body outside this export's behavior and gives it to
`callbacks`."* The `creates` census defers to a domain with no census.

## 3. Why a veto is possible here and provably is not for `reads`

Veto design § 6 rules out a synthesized `reads` veto because an unenumerated read is a
read of a source the **export** owns, and a synthesized veto *"can only instrument
values the caller supplied."*

**A callback is a caller-supplied value.** The exact property that makes `reads`
unmechanizable is the one this domain's observation stands on, so § 6 does not reach
it. What that observation is, and the ordering trap in writing it, is § 5.

## 4. The census, as built

`census_callbacks_domain` runs the **same call walk** the `creates` census runs —
extracted as `census_call_walk` so the two read one traversal rather than two — and
reads the dispositions that arm excuses.

`CensusDisposition::ParameterRooted` is already documented as *"the callee is proven
to be a caller-supplied callable … this export's act is the **invocation**, which is a
`callbacks` item."* `creates` excuses it because the callable's body is the caller's
behaviour. This domain enumerates that act, so the disposition `creates` excuses is
the item this census counts, and an empty enumeration is proven by there being none.

`CensusRun::caller_supplied_invocations` counts them. The walk decides, then:

- zero — the claim is proven, and the site
  `typefacts-implementation-census:callbacks:caller-supplied-invocations:0` records it;
- nonzero — **refused**, and named: *"the callbacks closure candidate enumerates no
  invocation, but the implementation census dispositioned N call(s) into the
  parameter-rooted family: this export invokes callable(s) its caller supplied."*
  Not a premise gap. The walk decided, and what it decided is that the claim is false.

The predicate spans the whole parameter-rooted family — the accessor, iterable,
element, coercion and `hasInstance` members as well as the direct call — because each
runs a getter, a `Symbol.iterator`, a `Symbol.toPrimitive` or a `Symbol.hasInstance`
that reached this export on a caller-supplied value, and that body is the caller's
too. It over-refuses by design: `drop = (list, n = 1) => list.slice(n)` is refused,
because a caller may pass an object whose `slice` is its own. Refusing a true closure
costs a candidate; certifying a false one is the failure this project exists to avoid.

Only the **empty** enumeration is proposed. A described invocation carries timing,
tracking and owner the walk does not derive, so `inferred_contract` withholds it and
the enumeration stays partial exactly as it did before the domain became proposable —
the same shape the `returns` filter beside it already had.

## 5. The veto, as built

`Observation::EmptyCallbacks`, marker `callback-invocation`. The machinery was already
present and unused: `module_source` emits `const invoked = { count: 0 }` and
`Sample::Callable` renders as that counting `callback` for every parameter slot the
selected signature admits a callable at.

**The emit is from inside the callback, not from a checkpoint after the sample loop.**
A synthesized entry carries `"drain": [{ "kind": "microtasks", "maxTurns": 1 }]` and
the drain runs *after* `runProbeSession` returns, so a queued invocation lands after
any post-loop check would have run — and a missed invocation is a `CleanNonObservation`
that certifies the very claim it should have contradicted. Emitting once, from the
callback, observes invocation whenever it happens and keeps a loop-invoked callback
inside the session's event budget.

ADR 0023 holds by construction rather than by a rule of its own: the walk dispositions
*calls*, and retaining a callable in a collection or on a returned object is not one.

`candidate_observation` reads `ExportSemantics::callbacks()`, not
`operation_claim(ClaimDomain::Callbacks)` — the domain carries
`KnowledgeSet<CallbackInvocation>` of its own and is not an operation claim.

## 6. One latent trap this uncovered

`withheld_weakening` matched the gated domains against a **second hand-written list**
of `creates | returns | reads`, while its own doc comment says they *"are exactly
`ClaimDomain::PROPOSABLE`"*. Admitting a fourth domain made it refuse every withheld
record naming that domain, failing the whole certification with `UnknownDomain`
instead of gating. It now derives from the constant.

The refusal was loud, so this was a latent trap rather than a live defect — but it is
the dual-derivation hazard this repository names elsewhere, sitting in the gate that
decides what a receipt certifies.

## 7. Measured

All 418 rows, release binary, checked-in recipe corpus, against the pin taken the
same day on the same profile.

| | before | after | delta |
| --- | ---: | ---: | ---: |
| **export-cases with a proven `callbacks` domain** | **0** | **408** | **+408** |
| `certifiedClosures.count` | 6,495 | 6,529 | +34 |
| certified rows | 381 | 381 | 0 |
| refused rows | 18 | 18 | 0 |
| run wall | 649 s | 746 s | +15% |

**No row fell below the baseline.**

The two closure figures measure different things and the smaller one is the one that
undersells. `certifiedClosures.count` counts *entries* — an export-case that certifies
at all — so it moves only for the 34 that previously certified nothing. The 408 is
export-cases that now carry `callbacks` as a **proven** domain; the other ~374 were
already certified on `creates` or `returns` and gained a second proven domain, which
the entry count cannot see.

What did **not** improve is breadth: the same 381 rows certify and the same 18 refuse.
No package that failed to certify now succeeds. This lever adds depth to rows that
already certified.

Per-domain over rows whose lists are uncapped, so the comparison is exact:

| domain | before | after |
| --- | ---: | ---: |
| `callbacks` | 0 | **243** |
| `creates` | 540 | 540 |
| `reads` | 16 | 16 |
| `returns` | 200 | 200 |

### 7.1 The first measurement said +94, and 60 of those were false

The counter was incremented where the walk dispositions a **call**. The producer's
call census records `ast.IsCallExpression` and `ast.IsNewExpression` only, so a
getter, an iteration-protocol member, an `await`-`then` or a coercion on a
caller-supplied value reaches the walk as an *uncensused invoking form* on a
different path (`semantic-model.md`, Decision 2026-09-03) — and every one of those
forms is named by the `callbacks` domain's own definition. Four form-disposition
sites recorded to `run.sites` without counting, so an export invoking a
caller-supplied getter certified `callbacks: []`.

`CensusRun::record` now records the site and the count in one place, so they cannot
disagree about what the walk saw. The corrected gain is **+34**; the 60 closures the
fix removed were unsound. The findings delta in § 8 is unchanged by it, so what the
fix removed was count without value.

The three existing domains are unchanged to the entry. The apparent `creates` −16 in a
naive count of *listed* entries is per-row list capping, not a regression.

Verified: 479 backend and 236 IR lib tests, 42 contract/diagnostics process tests,
workspace `clippy --all-targets -D warnings`, `fmt --check`, `git diff --check`. Two
existing tests were updated for the new domain, both in the idiom the `reads`
admission of 2026-09-10 established in the same assertions.

## Caveats

- The pin in `benchmarks/ecosystem/report.json` is the **before** state. Re-pinning is
  a deliberate `make ecosystem-benchmark` and was not done here; the regression gate
  passes because no row lost a closure.
- Most callbacks candidates are still withheld — the walk's own refusals (uncensused
  invoking forms, unresolved callees) reach this domain exactly as they reach
  `creates`. The +94 is what survives all of it.
- The over-refusal in § 4 is deliberate and unmeasured: how many true closures the
  wider family costs is a separate question, and narrowing it needs a review per
  member.
- Single run.

## 8. What it is worth to a consumer

A closure count is a proxy. The question this project keeps having to answer for a
closure lever is whether the closed domain changes a **finding**, and for this one it
does.

`callbacks` is not one of SC9005's conjuncts. It is demand-scoped at the call site
through `Indexes::unknown_contract_callback_export`, and the obligation is raised only
where a call actually hands over a potentially-callable argument (SC9005 design § 1).
So a closed `callbacks` domain discharges that obligation exactly at such a call, and
reopening it must restore the finding there and nowhere else.

Measured by difference over the catalog-bearing fixtures, each minted onto a policy-2
receipt and analyzed twice — as the contract stands, and with `callbacks` reopened in
the document:

| fixture | SC9005 closed | reopened |
| --- | ---: | ---: |
| `package-callback-consumer` | **0** | 4 |
| `package-callback-arguments-consumer` | 4 | 6 |
| `v1-reactivity` | 1 | 3 |
| the other 11 | unchanged | unchanged |

**Eight uncertifiable findings are discharged by the closure**, and
`package-callback-consumer` goes from clean to four findings when it is taken away.
The eleven indifferent projects are the control: they pass no potentially-callable
argument to a contracted import, so the demand is never raised and reopening the
domain changes nothing — which is the call-site scoping working.

`contract_closure_process::a_callbacks_closure_discharges_the_call_site_obligation_that_reopening_restores`
pins it. It asserts only that *something* moves, and prints the per-fixture counts:
pinning the numbers would break on every fixture added for an unrelated reason.

## 9. The cost, and why there is no waste in it to remove

Admitting the domain costs **+2,268 s of cumulative `witnessAcquisition`** across the
418-row corpus (5,341 s → 7,609 s), plus 232 s of proposal generation. Nothing else
moves. Investigated for waste, and there is none of the three kinds worth looking for:

- **Duplicate demands.** None. On `seroval` the plans carry 167 demands under 167
  distinct ids; the callbacks census demands the same implementation transcript the
  creates census does, and it is discharged once.
- **Demands for candidates that can never certify.** None. Recipe gating
  *"withholds by name before anything is demanded of it"*
  (`CertificationPlan::certify_value_only`), so the 2,999 callbacks candidates that
  get no synthesized veto are weakened out before planning and cost nothing.
- **Probe sessions for candidates the census refuses.** Not where the time goes —
  the whole increase is Type Facts witness acquisition, not the probe gate.

What remains is real work for real candidates: exports that had no `creates`,
`returns` or `reads` candidate now have a `callbacks` one, so their implementation
transcript is acquired for the first time. **The cost is inherent to censusing a
fourth domain, not a defect.**

The only lever is proposing fewer candidates, and the principled version of it is
generator-side: a `CallbacksProposalWalk` mirroring `CreatesProposalWalk`, so an
export whose walk will refuse is never proposed and never demanded — exactly the job
`creates_walk_clean` does for its domain. `creates_walk_clean` itself is **not** a
usable gate here: an export can have a clean creates walk and still invoke a
caller-supplied callable (`drop = (list, n = 1) => list.slice(n)` creates nothing and
invokes `slice`), so reusing it would trade true closures for time on a predicate that
does not mean what this domain needs. Scoped, not built.
