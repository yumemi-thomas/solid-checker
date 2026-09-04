# Census blockers A and B, sized — and the plumbing gap that sits above both

Read-only diagnosis, worktree `codex/phase19a-authenticated-proof-policy` at
`c047f837`. No tracked file changed by the measurement; three temporary
instrumentation patches were added, measured, and reverted (named in
§ 6, with the rebuild that removed them).

Measurements were made with a debug checker from `make build-checker-debug`
(a bare `cargo build` produces a binary that refuses Type Facts certification)
and `bin/solid-typefacts` rebuilt by the same target — the Rust Type Facts
client is part of the producer's source manifest, so editing
`rust/crates/typefacts/src/session.rs` moved the producer stamp; reverting
restored it.

Sample: 33 ecosystem rows selected from `benchmarks/ecosystem/report.json`,
18 `solid1` and 15 `solid2`, over seven families (solid-primitives 15,
official-solid 6, tanstack 4, corvu 4, solid-devtools 3, kobalte 2,
solid-recharts 2, motion-solidjs 1), chosen to mix rows with heavy decline
records (`solid-recharts@1.0.1` 1308, `@solidjs/start-devtools` 1224,
`@solidjs/router@1.0.0` 751) with zero-decline controls (`@corvu/utils`,
`@corvu-next/utils`, `@solid-primitives/signal-builders`, `motion-solidjs`,
`@tanstack/devtools-ui`, `solid-js@2.0.0-rc.3`). Reports were written outside
the repository. **Every row's `outcome`, `class`, `signature`,
`declinedClosures`, `exportsTotal` and `exportsProven` was byte-identical to
the checked-in report on all three instrumented runs** — the instrumentation
moved no verdict.

---

## 0. Headline, because it changes which question is worth asking

> **Slice 1 has since landed.** The generator now publishes its `creates`
> closure candidate in the emitted document, labelled `proposedClosures`, and
> the census runs on a generated candidate. See ADR 0008 § 1 "How the proposal
> is published" and the 2026-09-04 entry at the top of
> `docs/precision-backlog.md` for the mechanism, the digest families, the
> snapshot cost, and the 20-row rerun. Every measurement below stands as taken;
> only the remedy in § 4 Slice 1 is now history rather than plan.

Neither A nor B is the binding constraint. A third gate sits above both, and it
is a plumbing gap rather than a semantic one.

**The generator's `creates` closure candidate never reaches certification.**
`normalize_inferred_contract_with_candidates_and_external_targets`
(`rust/crates/solid-facts-backend/src/inferred_contract.rs:62-92`) calls
`export.open_proposed_closure()`, which **weakens the closed domain in the
emitted document** and records the candidate in the *proposal plan sidecar*
(`<output>.proposal.json`). The certifier rebuilds its candidate universe by
calling `open_proposed_closure()` again — inside
`ProofPolicy::inspect_candidates`
(`rust/crates/solid-reactive-ir/src/contract_semantics/certification.rs:299-347`)
— over that same, already-weakened document, and a domain that is already open
yields no candidate. The plan sidecar is never handed to the native planner:
`ContractCertificationPlanningRequest`
(`rust/crates/solid-facts-backend/src/main.rs:179-200`) carries `proposal`
(the document) and has no plan or candidate field, and it is
`deny_unknown_fields`; `certificationPlannings`
(`packages/cli/scripts/certify-contract.mjs:611-633`) passes
`proposal: generated.output` and nothing else.

Measured, over the sample's 627 planned artifact cases:

| measurement | value |
| --- | ---: |
| `creates` closed in the proposal document handed to `--plan-contract-certification` | **0 of 32,901 export slots** |
| the same after `select_and_bind` applied every closure hazard | 0 of 32,901 |
| closure candidates `recipe_gated` saw (331 invocations) | **0**, `creates=0`, `paths={}` |
| `creates` closure candidates present in the *plan sidecar* for `@kobalte/utils@0.9.2\|solid1\|only` | **33** (plus 41 `reads`, 41 `returns`, 40 `callbacks`) |
| `creates` closure candidates in the plan sidecar for `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | **7** |

So candidates *are* produced on real rows — ADR 0008's "on every measured real
row no `creates` candidate was proposed at all — 0 candidates" is no longer
true — and they are lost between the generator's plan sidecar and the
certifier's candidate universe. No `DomainExhaustiveness` demand is derived, no
recipe gate is consulted, no `WithheldClosure` is recorded, and
`census_creates_domain` is never called. That is consistent with the whole
corpus: across all 418 checked-in rows, `demandCountsByFamily` never names
`domain-exhaustiveness`, `withheldClosures` is 0 everywhere, and
`exportsProven` is 0 everywhere.

**The entire census chain is proven only against a synthetically closed
candidate.** `census_fixture_plan`
(`rust/crates/solid-facts-backend/src/contract_certification.rs:9190-9219`)
calls `plan_for_test_package_closing(..., &[(closed_export,
ClaimDomain::Creates)], ...)`, which builds a candidate contract with `creates`
already closed. Nothing in the repository exercises the
generate-then-certify path for a closure candidate — which is exactly the
"Break A" observation of
`2026-09-01-dependency-composition-scoping.md` § 3.3 ("no repository gate runs
`certify` over a fixture"), now shown to hide a live gap rather than only an
unexercised lane.

---

## 1. A's true scope

The census's item-2 premise (ADR 0008) refuses **every** uncensused invoking
form the `MayExecute` floor admits. To size that per export over shipped
JavaScript, one temporary probe recorded every implementation transcript the
producer answered during ordinary contract generation
(`validate_implementation_transcript`, `rust/crates/typefacts/src/session.rs`):
1,244 distinct transcripts across the sample.

**Population caveat, stated because it bounds every number below.** These are
the transcripts generation actually asked for. A minority sit in dependency
files a probe's program pulled in (`node_modules/solid-js/dist/refresh.js`, for
instance) rather than in the analyzed package. They are *not* filtered to
`ConsumingPackage` function exports.

### 1.1 Item 0 and item 2, per transcript

| bucket | transcripts | share |
| --- | ---: | ---: |
| `complete`, **no** uncensused form at the floor — items 0+2 clear **today** | **250** | 20.1% |
| `complete`, **only** `property-access-unknown-accessor` — A is the sole item-2 blocker | **248** | 19.9% |
| `complete`, A **plus** another uncensused kind | 364 | 29.3% |
| `complete`, only non-A kinds | 59 | 4.7% |
| `complete: false` — refused before item 2 is reached | 323 | 26.0% |

`complete: false` and a non-empty `control_flow.unsupported` coincided exactly
(0 transcripts were `complete` with a marker), which is gate 7 of `complete`
doing its job; ADR 0008 item 0's *additional* verifier-side refusal (any
`break`/`continue` in the bound frame, nested callables included) was **not**
measured, so every "clears item 0" count below is an upper bound.

Form occurrences at the floor, over the same 1,244 transcripts:

| kind | occurrences | transcripts |
| --- | ---: | ---: |
| `property-access-unknown-accessor` | 7,459 | 640 |
| `coercion` | 1,112 | 217 |
| `jsx-element` | 484 | 103 |
| `iteration-protocol` | 443 | 260 |
| `instanceof` | 55 | 50 |
| `get-accessor` | 19 | 7 |
| `await-then` | 19 | 4 |

### 1.2 The answer to "if A were closed, how many exports become censusable?"

Closing A at item 2 moves the exports that clear items 0+2 from **250 to 498**
(+248). It does not by itself make them censusable, because item 3 must then
disposition every floor call. Counting a call as dispositionable when it is
parameter-rooted (`calleeParameter`), standard-library
(`declaration.standardLibrary`), or has a resolved declaration in a
non-`.d.ts` file under the analyzed package's own root (the only shape
`LocalRecursion` can take):

| | exports | every floor call dispositionable | no floor calls at all |
| --- | ---: | ---: | ---: |
| `complete`, no form (today) | 250 | **188** | 18 |
| `complete`, A-only (unlocked by closing A) | 248 | **66** | 13 |
| `complete`, A + other | 364 | 80 | 10 |

So the ceiling A buys is **+66 exports** on top of the **188 that already
clear every premise this diagnosis can evaluate** — and every one of those 188
is stuck for the reason in § 0, not for A. Both figures remain upper bounds:
the local-recursion arm recurses into the helper, whose own transcript must
pass the same premises (26% of transcripts are `complete: false`), the
standard-library disposition additionally demands a proof per reviewed invoking
slot, and item 5 needs a candidate that today never arrives.

**A refuses only because of itself for 248 exports (19.9%); 423 more (364 + 59)
are refused by another uncensused kind as well, and 323 are refused before item
2 by `complete`.**

---

## 2. Is A's refusal necessary as written?

### 2.1 The call-position case is not admissible for `creates`

`obj.m()` evaluates `Get(obj, "m")` and then calls the result. If `m` is an
accessor, the **getter body runs**, and a getter is arbitrary user code: nothing
in the language or in the semantic model makes a getter read-only.
`get onChange() { createEffect(track); return this._fn; }` performs a `create`
inside the getter, and the census has enumerated neither the getter nor the
returned callable. The proposed reasoning — "calling a getter that returns a
function invokes the getter (a read) and then the function, whose own body is
not censused" — grants the second half and denies the first: the getter's own
body is exactly as uncensused as the function's, and a `creates` closure is a
**zero upper bound** over the whole invocation. Admitting the call position
would therefore certify `creates: []` for an export whose evaluation may run
code the census never saw. It is not admissible.

It is also not worth much if it were. Of 7,459 accessor forms at the floor,
**1,502 are in the callee position of a recorded call and 5,957 are not** —
reads, object/JSX spreads, destructured members. Per transcript, of the 640
carrying any accessor form: 107 are all-callee, 310 all-read, 223 mixed. So a
call-position-only narrowing clears item 2 for at most **107 of 640** accessor-
blocked transcripts (16.7%), and of the 248 A-only transcripts, 64 are
all-callee, of which only **11** also have every floor call dispositioned.

(The callee-position test compares spans: a form sharing a call's start offset
and ending strictly inside it. In `a.b.c()` the inner `a.b` access shares the
same start, so this **over-counts** callee position — the 1,502 and 107 are
upper bounds too.)

Worse, admitting the form does not admit the *call*. Of the 1,173 floor calls
whose own callee sits on an unresolved-accessor property access, only **219
(18.7%) carry `calleeParameter`**. The rest have no resolved declaration and no
parameter root — so with A admitted at item 2 they refuse at item 3 anyway,
with no disposition. The `parameter-rooted` mass ADR 0008 already documents as
"the census would have decided this call" is one fifth of the call-position
population.

### 2.2 What fact would make it admissible

Not a narrower *position*; a narrower *cause*. `accessorFormLocked`
(`apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms.go:438-468`)
and `accessorKindForSymbolLocked` (`:588-606`) collapse three distinct
situations into one kind:

1. `GetSymbolAtLocation` resolved **no symbol** (an `any` receiver, a computed
   key, an index signature);
2. it resolved a symbol whose declarations are **not the snapshot's runtime
   bytes** (`declarationCarriesRuntimeBytesLocked` false — a `.d.ts` that may
   describe a `.js` getter, which is every property read on a value whose type
   comes from a dependency's declarations);
3. the form reads *every* own enumerable property (object spread, JSX prop
   spread, object rest).

Only a fact that answers "this member is not an accessor" clears any of them,
and the two candidate shapes are:

- **Callee value provenance** (`docs/typefacts/adr/0025-v1-callee-value-provenance.md`),
  which names the value being called rather than the property spelling. It
  decides case 1 where the value's origin is traceable, and it is the fact
  `computed-member` already waits on.
- **A runtime-bytes accessor census over the declaring artifact.** Case 2 is
  not the checker's ignorance about the *shape* — the declaration exists, it
  just is not runtime source of *this* archive. A dependency whose own
  authenticated snapshot is in the closure could answer "no accessor
  declaration in the bytes that run", which is the same premise ADR 0026
  already grants the default library. That is a producer plus dependency-
  composition slice, not a census change.

Splitting the kind so the three causes are distinguishable is cheap and is
**measurement, not a fix**: it would say which cause dominates on real rows
(this diagnosis cannot — the producer emits one kind), and therefore which of
the two facts above is worth building. Neither is worth starting before § 0 is
closed.

---

## 3. B's mechanics

**Where it is raised.** Twice, by the two closure builders that must agree.
CLI-side, `packages/cli/scripts/artifact-resolution.mjs:2388-2405`: for each
bare external specifier, `const accepted = acceptedDependencies[specifier.text]`
— and when there is none, a hazard with `kind:
"unaccepted-external-dependency"`, `affectedExports: []` (meaning *all*) and
`affectedDomains: [...DOMAIN_NAMES]` (all nine). Verifier-side,
`rust/crates/solid-facts-backend/src/contract_certification/module_closure.rs:357-386`:
`record_external` matches the specifier against `supplied_dependencies`, and an
empty match calls `record_opaque_frontier`, which pushes the same hazard. Its
effect is `ClosureManifest::open_domains`
(`rust/crates/solid-facts-backend/src/artifact_resolution.rs:417-426`) feeding
`export.open_call_domains(...)` at `:908`, inside `bind_exports` — so every
export of that artifact case loses every call domain.

**What "accepted" requires.** The specifier must appear in
`acceptedDependencies` at generation, which
`generatePackageContract`
(`packages/cli/scripts/generate-package-contract.mjs:1207-1241`) fills from one
of two mutually exclusive lanes: the **accepted lane**
(`--accepted-contracts` plus an authenticated contract catalog *and* a receipt
trust configuration — all three or it throws) or the **private proposal lane**
(`--proposal-dependencies` plus a private proposal-dependency catalog, which
`contract certify` builds per graph node).

**Why real rows fail it.** The ecosystem runner's `generateContract`
(`scripts/ecosystem-benchmark/run.mjs:1743-1771`) passes `--package-root
--integrity --output --certification-importer --entrypoint` and **neither
lane**. This is Break A of the scoping study, and it is not a switch the runner
can flip: it holds no receipts to pass. Each probe mints a fresh
`persistent-local` issuer with a random seed into a temporary authority
directory (`run.mjs:1786-1797`) and the whole output tree is deleted when the
probe ends, so there is no receipt store across probes at all.

**Quantified.** Over the sample's 627 planned artifact cases:

| measurement | value |
| --- | ---: |
| artifact cases carrying ≥1 `unaccepted-external-dependency` hazard | **524 of 627** (83.6%) |
| packages with ≥1 such case | **29 of 29** |
| packages with an unaccepted `solid-js` / `@solidjs/*` edge | **28 of 29** (only `@solid-devtools/overlay` has none) |
| artifact cases with **no** such hazard | **103** |

And the control that settles B's ordering: **those 103 hazard-free artifact
cases still have `creates` closed for 0 exports in the proposal handed to
certification.** B would remove a candidate if one existed — it reopens
`creates` in `select_and_bind`, before `inspect_candidates` — but on 103 cases B
is not in play and the candidate is missing anyway. B is downstream of § 0.

**Would passing "the receipts it already has" fix it? No — there are none, and
the chain terminates where it cannot be certified.** Turning an unaccepted
specifier into an accepted dependency edge does not make the row easier: it
converts "domains open" into a hard demand for an authenticated dependency
receipt. That demand is visible on the rows that do plan dependencies —
`dependencyPlan.leaves` across the checked-in report carries **719
`authenticated-receipt-unavailable`** leaves on **47 of 418 rows**, and
`@solidjs/signals` is named on **24** of them; **8** rows' unavailable receipts
are *only* the dialect-defining archives. Those archives are exactly what
cannot be certified: ADR 0007's tier refuses to answer about `solid-js`,
`@solidjs/signals` and `@solidjs/web` under certification, and
`2026-09-03-solid-js-self-certification-diagnosis.md` records that
`solid-js@1.9.14|solid1|only` refuses at witness acquisition on
`recursive-value-shape`. The private proposal lane can *content-address* a
dependency proposal without a receipt (that is how the 20 published-graph rows
work, 11 of which certify), so it could clear the hazard for a non-Solid
dependency — but for the `solid-js` edge every row carries, it would still need
a proposal for an archive the tier refuses to answer about.

---

## 4. Ordering, and the minimum path to one proven `creates: []`

Neither A nor B is first. The order is forced by where the candidate dies.

**Slice 1 — carry the closure candidate from generation into certification.**
*(Landed. The second option below is the one taken, plus a label so the
emitted proposal stays distinguishable from a reviewed document.)*
The proposal plan sidecar's `closureCandidates` must reach
`inspect_candidates`, either by adding the plan to
`ContractCertificationPlanningRequest` and re-closing its named claims on the
candidate before `inspect_candidates` runs, or by having generation emit the
domain still closed and letting `inspect_candidates`'s existing
`open_proposed_closure` perform the one weakening it was written for. Either
way the semantic digest of what is planned changes, so it is a deliberate
change with corpus-snapshot cost, not a patch. **Scope: one slice, plus its own
gate** — a fixture certified through the real generate-then-certify path, which
no gate does today (Break A). Until this lands, no closure claim of *any*
domain can be certified from a generated proposal, and the census cannot run
once.

**Slice 2 — pick a first row whose artifact case carries no closure hazard.**
B is not required for the first proven row: 103 of 627 sampled artifact cases
have no `unaccepted-external-dependency` hazard (`@kobalte/utils` alone
contributes 67 of its 111 cases), and those cases keep whatever domain the
proposal closed. **Scope: row selection, no code.** Closing B for the other 524
is a separate, larger slice, and it terminates on the dialect archives (§ 3).

**Slice 3 — a recipe for that row's claim.** A candidate the census *proves*
spawns a mandatory probe veto, and a scheduled veto with no recipe refuses the
gate and the row (`MissingGate`). With no corpus configured,
`CertificationPlan::recipe_gated` withholds the candidate instead — the domain
stays **open**, so the row certifies but proves nothing. **A first row reaching
a proven `creates: []` therefore needs a recipe in a corpus**: hand-written for
that one row, or ADR 0006 Stage 3 recipe synthesis for the general case. The
authenticated dependency-closure copying into the probe workspace **has
shipped** (ADR 0006 § "The authenticated dependency closure",
`fixtures/package-contracts/implementation-census-creates/dependency-consumer`),
so a consumer recipe can already `import` the package under test and that
package can resolve its own dependencies inside the private workspace. That
part is covered.

**Slice 4 — then, and only then, A.** With slices 1-3 in place the census
finally runs on real transcripts, and the measured ceiling is: 188 exports
already clear every premise this diagnosis can evaluate, and closing A raises
that to 254 (§ 1.2). A is a quarter-of-the-population blocker, not the first
one, and § 2 says the cheap version of closing it (a call-position narrowing)
is unsound *and* worth at most 11 exports.

**Slice 5 — the walk's silent majority, which is larger than A.** A second
temporary probe partitioned every export at
`attach_generated_owner_requirements`
(`rust/crates/solid-facts-backend/src/main.rs:6584-6626`): of 1,267 distinct
(entrypoint, export) pairs, 103 are `value` exports and of the 1,164 function
exports **137 (11.8%) the walk cleared**, **142 (12.2%) declined with records**,
and **885 (76.0%) got no verdict at all** — the export's canonical symbol
resolved, but neither `clean_creates_walk_by_symbol` nor
`creates_walk_declines_by_symbol` contains it, so `creates_walk_clean` is
`false` and `creates_walk_declines` is empty. Two mechanisms can produce that
and this diagnosis does not separate them: the `function_symbols` index is
built from each `FunctionFact`'s **name node**, so an export implemented by an
arrow or function expression bound to a variable is never keyed; and in a
bundled artifact the export entity's canonical symbol may differ from the
symbol of the declaration's own name. Either way the ranking that
`scripts/dialect-audit-yield.mjs` produces is a ranking over 12% of the
function exports, not over the population. Per package this is stark:
`@corvu/utils` 2 clean / 0 declined / 54 silent, `motion-solidjs` 0 / 0 / 165,
`@tanstack/devtools-ui` 0 / 0 / 49, against `@kobalte/utils` 60 / 12 / 1.

---

## 5. What we are **not** doing yet, and why

- **Not changing the candidate plumbing in this slice.** § 0's fix moves what a
  planned contract asserts and therefore its semantic digest; it needs its own
  slice with a fixture gate over the real generate-then-certify path, and it
  will move corpus snapshots. It is not a small obviously-correct edit, so this
  diagnosis implements nothing.
- **Not narrowing `property-access-unknown-accessor` by position.** § 2.1 shows
  it is unsound for `creates` — the getter's own body is uncensused — and § 2.2
  shows the sound version needs a producer fact. The measured yield of the
  unsound version is 11 exports.
- **Not splitting the kind's three causes yet.** It is worth doing *as
  measurement* once the census actually runs; before that it would rank causes
  for a census nothing reaches.
- **Not engaging the accepted-dependency lane for the ecosystem runner.** It
  has no receipts to pass, and the terminal edge on 28 of 29 sampled packages
  is a dialect-defining archive the tier refuses to answer about. The 103
  hazard-free artifact cases are the way to a first row instead.
- **Not chasing the 885 silent exports in this slice**, but recording that they
  outnumber both decline populations combined and that every existing
  audit-yield ranking is computed over the 12% the walk actually reached.
- **Not re-running the full corpus.** `make ecosystem-benchmark` was not run;
  every corpus-wide figure here is read from the checked-in
  `benchmarks/ecosystem/report.json`, and every new figure is from the 33-row
  sample whose verdicts reproduce that report byte-for-byte.

---

## 6. Instrumentation, and its removal

Three temporary patches, all reverted, with the debug checker and
`bin/solid-typefacts` rebuilt from the reverted source afterwards:

1. `rust/crates/typefacts/src/session.rs` — `census_blocker_probe`, called from
   `validate_implementation_transcript`, appending one line per acquired
   implementation transcript (form kinds at the floor, accessor position, call
   dispositions, `complete`, control-flow markers) to
   `SOLID_CHECKER_CENSUS_BLOCKER_PROBE`. **Note:** the Rust Type Facts client is
   inside the producer's source manifest, so this patch changed
   `bin/solid-typefacts.buildinfo`'s `sourceDigest`; reverting and rebuilding
   restored it.
2. `rust/crates/solid-facts-backend/src/main.rs` — a walk-partition line per
   export in `attach_generated_owner_requirements`, to
   `SOLID_CHECKER_WALK_PROBE`.
3. `rust/crates/solid-facts-backend/src/contract_certification.rs` — two lines
   to `SOLID_CHECKER_GATE_PROBE`: the closed-`creates` count of the proposal
   before and after `select_and_bind` with the artifact case's unaccepted
   specifiers, and the closure-candidate inventory `recipe_gated` sees.

The `--keep-temp` reproductions that read the emitted plan sidecars and closure
manifests needed no patch.
