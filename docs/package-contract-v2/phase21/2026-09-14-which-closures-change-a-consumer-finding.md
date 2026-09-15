# Which closures change a consumer-side finding (2026-09-14)

- **Status:** measurement and one instrument fix. No analyzer, generator or
  certifier code changed; no closure was authored.
- **Question:** row counts rank the campaign by what is *closeable*. The
  question they cannot answer is which closures change what a consumer sees.
- **Answer:** none of them, today. Over 146 real consumer projects, **2,585 of
  2,585** `SC9005` import-site findings stop at the *acceptance* gate — no
  receipt-accepted contract matches the import — and **zero** name an open
  claim domain. Closure depth is invisible to every one of these projects
  because no third-party contract is accepted anywhere in the corpus.

## 1. What a closure can change, mechanically

`push_unknown_contract_claims` (`contracts.rs`) is the only place a claim
domain becomes a consumer finding. It emits `SC9005` with
`analysis_context: unknown-contract-claims:<claims>` when, for an export whose
contract *is* accepted, `reactiveReads`, `returns`, `ownerRequirements`
(the `creates` domain) or `asyncBehavior` is open. If none is open it returns
and no finding is emitted.

Two consequences follow from the code and matter for any ranking:

- **The claims are conjunctive.** The finding disappears only when every
  demanded domain closes. Closing `reads` on an export whose `creates` is also
  open shortens the claim list and removes nothing.
- **Read *items* are not gated on closure.** The doc comment on
  `reads_completeness_demanded` says it outright: rules consume `reads` items,
  which arrive whether or not the domain is closed, and only `SC9005` consumes
  the completeness. A closed but empty `reads: []` enables no new violation
  finding — it removes an uncertifiable one.

`callbacks` reaches consumers by a second path, `interproc.rs`, which raises
the same defect kind at the *callback-argument* span rather than at every
import. So the four domains do not have the same site set even in principle.

## 2. What real consumers actually hit

146 projects: `solidjs-community/solid-primitives` (local checkout, 116
projects), `corvudev/corvu`, `kobaltedev/kobalte`, `solidjs/solid-docs`
(shallow clones, `pnpm install --frozen-lockfile --ignore-scripts`). Release
binary, one process per project. Status: 105 `uncertifiable`, 32 `violation`,
9 `certified`.

| `SC9005` by gate | findings |
| --- | ---: |
| acceptance gate (`no receipt-accepted contract matches this exact import`) | **2,585** |
| open claims (`unknown-contract-claims:…`) | **0** |
| callback execution (not an import site) | 983 |

The instrument could not previously see this distinction: both gates produce
the same *message*, and the 2026-09-12 sweep read messages only. The regex
matched, the table filled in, and the number it reported — "233 exports, 2,056
call sites" — was read as demand for closures when it was demand for
*contracts*. `2026-09-12-consumer-demand-measurement.py` now prints the gate
split first, so a corpus with no closure-sensitive demand says so in line two.

## 3. The demonstration, in the corpus's own numbers

Twenty-one demanded exports are already **ALL CLOSED** in the pin across every
domain, at **473 call sites** — `@solid-primitives/utils` `noop` (98 sites, 39
projects), `INTERNAL_OPTIONS` (91), `asArray` (51), `entries` (37), `trueFn`,
`accessWith`, `createMicrotask`; `@kobalte/utils` `visuallyHiddenStyles` (28).

Every one of those 473 sites still raises `SC9005` in this corpus. There is no
closure left to write for them and the finding is unchanged. That is the
finding stated as an experiment rather than as a code reading.

## 4. Why acceptance never happens

`pkg/contracts/bundled/README.md`: both dialect bundle indexes are empty, and
"external packages still require independently accepted contracts". Nothing
ships a third-party contract. The 18,350 certified claims live in
`benchmarks/ecosystem/report.json`, a benchmark artifact, and a consumer
reaches them only by running certification itself and registering the exact
document/receipt pair in its own `.solid-checker/accepted-contracts.json`.

Attempted directly on the highest-demand project in the corpus
(`kobalte/packages/core`, 471 sites on `@kobalte/utils@0.9.2` — the exact
version the corpus certifies) and not completed:

- the plain lane refuses at the root case
  (`recursive-value-shape … scrollIntoViewport: parameter-rooted read lacks
  positive original-input identity`);
- the published-graph lane, which the pin uses for this package, refuses
  earlier still — `no exact Bun text lockfile exists above` a pnpm store path.

**The second of those is now fixed.** [ADR 0108](../../adr/0108-a-lockfile-is-named-by-its-file-name.md)
makes the lockfile's file name the format decision and adds a `pnpm-lock.yaml`
reader to both the acquisition and authority sides; against the real lockfile,
`@solid-primitives/utils@6.4.1` issues a receipt into this same project's
catalog. The first is unchanged and is a census premise, not a packaging one:
`@kobalte/utils` certifies no root case under either package manager.

So a consumer using its own package manager *can* now run the path. Whether
running it moves a finding is § 5's conditional, and still unmeasured — this
corpus's highest-demand package is the one that does not certify.

## 5. The ranking, for after acceptance

Stated as a conditional, because § 2 says it is one. Of the 1,958 in-corpus
call sites, by the domains their export leaves open:

| domain open | call sites |
| --- | ---: |
| `callbacks` | 917 |
| `reads` | 834 |
| `creates` | 804 |
| `returns` | 36 |

| state | exports | sites |
| --- | ---: | ---: |
| every demanded domain closed | 21 | 473 |
| at least one open | 91 | 1,141 |
| no ledger entry at all | 47 | 344 |

Read with § 1's conjunction, the per-domain column overstates each domain:
563 of those sites have `callbacks`, `creates` **and** `reads` open at once, so
closing any one of the three moves nothing there. The largest single-domain
group is `callbacks` alone at 216 sites over 15 exports.

`@kobalte/utils` `mergeRefs` (164 sites) and `access` (66) have **no ledger
entry** — the certification never proposed them, so there is no closure to
rank. That is 230 sites of demand with nothing on the board, and it is a
larger gap than any single recipe cluster closed this week.

The per-export table is
[`2026-09-14-consumer-demand-recensus.json`](2026-09-14-consumer-demand-recensus.json),
committed this time: the 2026-09-12 run wrote its `demand.json` to a temporary
directory and the numbers had to be re-measured from scratch to answer this
question.

## 6. What this does not say

It does not say the closures are worthless. It says their value is gated behind
a step no consumer performs, and that the gate is distribution and acceptance
rather than depth. It also does not generalize past this corpus: five upstream
Solid repositories are not the ecosystem, and a project that *does* accept
contracts would see exactly the § 5 ranking.

The corpus is also not identical to 2026-09-12's — `solid-primitives` is a
newer local checkout with 116 projects rather than 70 — so the totals moved
(233 → 242 exports, 2,056 → 2,585 sites) for reasons unrelated to any change
in the checker.

## 7. Measured: accepting a contract changes nothing yet, for a second reason

§ 4 said a consumer could not run the acceptance path at all. ADR 0108 removed
that obstacle, so the experiment could finally be run — and the answer is still
no.

`@solid-primitives/props@3.1.11` was certified from `kobalte`'s own pnpm tree
(47 closures, `policy2-persistent-local`) into
`kobalte/packages/core/.solid-checker/accepted-contracts.json`, and the project
re-analyzed with `--receipt-trust-configuration` pointing at the issued trust
configuration. Before and after, to the finding:

| | before | after |
| --- | ---: | ---: |
| `SC9005` total | 632 | 632 |
| at the acceptance gate | 597 | 597 |
| `@solid-primitives/props` | 39 | 39 |

**The acceptance index is keyed on `(importer, specifier)`** —
`contract_semantics/consumer.rs`, reached from `accepted_package_contract_statuses`
via `contracts.contract(importer, &import.text)`. The importer is the exact
importing file, because the same specifier resolves differently from different
files, and an acceptance may only speak for the resolution it verified.

Certification binds its acceptance to an importer it creates itself: a synthetic
`.solid-checker-certification-<digest>.mjs` written inside the package
directory. Nothing in `kobalte/packages/core/src` matches it, so `bound == 0`
and every import stays at the acceptance gate. The fixture catalogs show the
shape that does work — `fixtures/reactive-ir/package-return-consumer` names
`App.tsx`, the consumer's own file.

So the mechanism is sound and the producer is the gap: there is no way to
certify *for* a consumer's importers. `certify-contract.mjs` takes no importer
argument, and the ecosystem benchmark never noticed because it inspects the
catalog it writes and never analyzes a consumer against it.

The scale, for this one project:

| package | sites | distinct importing files |
| --- | ---: | ---: |
| `@kobalte/utils` | 471 | 204 |
| `@solidjs/testing-library` | 66 | 38 |
| `@solid-primitives/props` | 39 | 39 |
| `solid-presence` | 13 | 13 |
| **total** | **597** | **251** |

251 entries for one project, one of 146. Whatever closes this is not hand-written
catalogs; it is either certification accepting a set of importers, or an
acceptance identity that binds to the resolved artifact rather than the
importing file. Which of those is sound is an ADR, not a patch — the importer
key is what stops one file's resolution speaking for another's.

## 8. Correction: a contract *can* raise a usage defect, and here is the path

§ 6 and the summary that went with it said no accepted contract was observed
producing a finding about how a consumer uses the package. That was drawn from
fixture snapshots, and snapshots are the wrong instrument for the question: they
show what fires in the fixtures that exist, not what the rules consume.

Read from the rule inputs instead, the path is there and is unit-tested at both
ends:

1. A contract operation of `kind: cleanup` (or an effect) whose
   `owner.requires` is `required` and whose `source` is not `created` projects
   to `ContractOwnerRequirement { operation: Cleanup }` —
   `contracts.rs::project_owner_requirements`, pinned by
   `a_cleanup_requirement_projects_from_the_cleanups_domain_without_opening_it`.
2. `owners.rs` reads exactly that through `lookup.contract_owner_requirements`
   at any call **outside an owner-providing region**, and pushes an owner
   requirement for the call site.
3. That becomes `SC4001 missing-owner`, whose remedy names `onCleanup` for a
   cleanup requirement.

So a package export that calls `onCleanup` internally — `makeEventListener` and
`createEventListener` in `@solid-primitives/event-listener`, `tryOnCleanup` in
`utils`, the `rootless` singletons — called at module top level or from a plain
helper, is a listener that is never removed. **Without a contract the checker
cannot know the package cleans up and stays silent; with one it raises the
defect at the call site.** That is a usage bug found because a contract was
accepted, not a silence removed.

What the fixture evidence *did* show remains true and is the other direction:
`v1-reactivity`'s contract for `described` states its first argument is tracked,
which **certifies** a call that would otherwise be uncertifiable, while
`observe` — described by nothing — keeps raising
`v1/reactive-source-uncaptured`. Contracts resolve uncertainty both ways.

Not yet demonstrated end to end on a real package: that needs an
`@solid-primitives/event-listener` contract whose `creates`/cleanups domain
closes with the requirement, and a consumer calling it unowned. That is the
measurement to run, and it is now a specific one rather than an open question.

## 9. The measurement, run: the contract states nothing about the primitive

§ 8 named the specific test — an `@solid-primitives/event-listener` contract
whose owner requirement makes `SC4001` fire at an unowned call. Run on
2026-09-15 against the real installed `@solid-primitives/event-listener@2.4.5`
(pnpm tree, graph lane, fresh release of the B′ binding).

**It certifies — 44 closed entries, `reads` 35, `creates` 22, `callbacks` 16,
`returns` 10 — and says nothing at all about its own primitives.** Every one of
those 44 is a dependency node: `access`, `accessArray`, `asArray`, `clamp`,
`chain`, `createMicrotask` — the `@solid-primitives/utils` helpers reached
through the graph. `makeEventListener` and `createEventListener` publish

~~~json
{"call": {}, "shape": "callable"}
~~~

no claim in any domain. No owner requirement, so `contract_owner_requirements`
returns nothing, so `missing-owner` cannot fire however the acceptance is
delivered.

This is § 8 of `2026-09-12-consumer-demand-measurement.md` again, on a different
package: 35 of `@kobalte/utils`' 59 exports publish that same empty summary.

### Why, and why it is structural rather than incidental

The audit refuses the package's own primitives at the census — its reasons name
`onCleanup` 63 times. `@solid-primitives/event-listener@2.4.5` is a Solid **1.x**
package, and § 8.1 of that measurement recorded the cause: the 1.x dialect's
negative authority was withdrawn as inadmissible under ADR 0005, so "the census
refuses every 1.x callee by name", and `creates` can close only for an export
that calls *no* Solid primitive at all.

An owner requirement arises precisely from calling a Solid primitive —
`onCleanup`, `createEffect`. So on Solid 1.x an owner requirement is not merely
unproven, it is **unstatable**: the fact that would produce it is the fact the
census refuses. The helpers that do close (`clamp`, `asArray`) are the ones that
require no owner and can raise no such defect.

The mechanism itself is live on **Solid 2.0**, where the audited core does state
these: `"requires": "required"` appears in the bundled `solid-js`, `signals` and
`web` contracts. So the path is not theoretical — it is blocked for 1.x packages
specifically.

### What this settles

A consumer of a Solid 1.x package gets **silence removal** from a contract, not
defect detection: SC9005 goes away for the exports that close, and nothing new is
raised, because the exports that could raise something state nothing. The
delivery work of 2026-09-14/15 (ADR 0108, B′) is necessary and is not what stands
in the way here.

The next measurement is the same one against a Solid **2.0** package, where the
owner authority exists. If it fires there, the 1.x creates audit is the single
thing standing between this system and usage-level feedback for the 1.x
ecosystem — which is most of it today.

## 10. The Solid 2.0 half: the contract *does* state the owner requirement

§ 9 left one question: the owner-requirement path is blocked on Solid 1.x
because the requirement arises from calling a Solid primitive, which is what the
1.x census refuses. Does it work where the dialect has an admissible authority?

Measured 2026-09-15 against a real published Solid 2.0 package —
`@solid-primitives/utils@7.0.0-next.4`, peer `solid-js@^2.0.0-rc.0`, installed
into a fresh **pnpm** project (which is itself only certifiable because of
ADR 0108) and certified through the graph lane.

**It does.** 64 closed entries, and the emitted document carries, for export
`createMicrotask` at entrypoint `.`:

~~~json
{
  "id": "owner-requirement-0",
  "kind": "cleanup",
  "owner": { "requires": "required", "requiresCleanup": "required",
             "source": "ambient-at-call" }
}
~~~

That is exactly the shape `project_owner_requirements` converts into
`ContractOwnerRequirement { operation: Cleanup }` — `kind: cleanup`,
`requires: required`, and a `source` that is not `created` — the same triple its
unit test pins. `owners.rs` reads it at any call outside an owner-providing
region, and that is `SC4001 missing-owner`.

The package's source is the reason: `createMicrotask` calls `onCleanup` directly
(`dist/index.js:191`). On 1.x that call is what silences the export; on 2.0 it is
what the contract reports.

**So the usage-level feedback is real and is a Solid 2.0 capability.** A consumer
that calls `createMicrotask` at module scope has registered a cleanup no owner
will ever run, and only a contract can tell the checker that.

### Not yet observed firing, and why

A consumer project was built (`createMicrotask` called at module scope, contract
staged, `--conditions import` declared through a flag added for this) and the
acceptance still did not bind: the analysis reports `SC9005` with the
*no-summary* branch, so the catalog is accepted but the artifact case selected
for that import is not the one carrying the summary. That is hand-assembly of a
graph-lane case set into a consumer tree — a delivery problem in the harness of
this measurement, not a question about the contract, whose content is quoted
above.

What remains to see `SC4001` printed is making the acceptance bind the right
case, which is the same delivery work as B′ rather than anything about whether
the feedback exists.

## 11. Re-measured: the reason in § 9 was wrong

§ 9 explained `@solid-primitives/event-listener`'s empty summaries by the Solid
1.x dialect having no admissible `creates` authority, citing § 8.1 of
`2026-09-12-consumer-demand-measurement.md`. **Both halves of that are wrong,
and the audit it was read from says so on its face.**

**The 1.x `creates` audit exists.** `123e8d31` (2026-09-13, the day after the
measurement quoted) landed sixteen hand-audited `NEGATIVE_ROWS` in
`solid-dialect/src/solid_1x.rs` for `solid-js@1.9.14`, each citing exact bytes
of `dist/solid.js` by offset and slice digest, across six export-condition
bundles — `createSignal`, `createMemo`, `createEffect`, `batch`, `onCleanup`,
`mergeProps` and ten more. § 8.1 was true when written and stale by the next
day; § 9 read a dated measurement as current state.

(The ADR citation was also wrong. ADR 0005 is about a Solid **2.0** dialect
axiom for `@solidjs/signals`, is `deferred`, and its objection 5 is settled
*against* the premise. What it names inadmissible is the `creates: []` the
schema-1→v2 migration manufactured into the bundled 1.x JSON — which is
precisely why `solid_1x.rs` cites archive bytes instead, and why its derivation
test asserts the JSON-derivable set contributes nothing.)

**What actually happened.** Read from the same audit rather than from the
earlier doc:

| | |
| --- | ---: |
| closure candidates from `@solid-primitives/utils` | 56 |
| candidates from `solid-js` | 8 |
| candidates from **event-listener itself** | **0** |
| `declinedDependencyRecords` | 176 |
| `declinedDependencySpecifiers` | 4 |
| `partialProposalFrontier` | true |

The four specifiers are exactly this package's own modules reaching its
dependency — `./dist/eventListener.js:@solid-primitives/utils` and three
siblings. `declinedDependencyFrontier` (`certify-contract.mjs:1956`) builds that
object only from records whose `kind` is **`unaccepted-external-dependency`**,
so all 176 are that.

So `makeEventListener` publishes `{"call": {}, "shape": "callable"}` because
every closure on its modules **declined on an unaccepted external dependency**,
never reaching a census at all. No `dialect-silent` appears anywhere in the
audit, and the dialect's negative authority is never consulted. The audit's own
refusal strings are all census refusals belonging to `utils` exports, not to
this package.

`unaccepted-external-dependency` is a known wall — `docs/precision-backlog.md`
records it as one of "the next walls" behind the 1.x authority work, with the
graph lane as the intended answer. This run *was* the graph lane
(`--dependency-graph-lane --recover-entrypoints`) and it still declined, with
`retainedProposalCases: 0`. Why the lane did not compose the dependency it had
already certified as a node is the open question, and it is a composition
question rather than a dialect one.

**What survives from § 9 and § 10.** The observations do; the explanation does
not. `makeEventListener` really does state nothing, and
`@solid-primitives/utils@7.0.0-next.4` really does state an owner requirement on
`createMicrotask`. But the difference between them is not 1.x versus 2.0 — the
second package was certified with its dependency composed, and the first was
not. § 10's conclusion that usage-level feedback is "a Solid 2.0 capability" is
not supported by this pair, and is withdrawn pending a comparison that holds the
composition constant.

## 12. Why nothing binds: the export's implementation is in another file

§ 11 asked why the graph lane declined the dependency. It did not decline it.
`composedArtifactCases: 1`, and the `declinedDependencyFrontier` object is trace
metadata recorded in the **success** branch of the composition path
(`certify-contract.mjs:2044`) — a record of what *triggered* composition, not a
final state. § 11 read a historical record as an outcome, the same class of
mistake as § 9 reading a dated measurement as current state.

What the final document shows instead: **all eleven exports carry the identical
summary** `{"call": {}, "shape": "callable"}` — including `preventDefault`,
`stopPropagation` and `stopImmediatePropagation`, which call no Solid primitive
and never touch `@solid-primitives/utils`. Those three would close under either
of the earlier explanations. The emptiness is systemic to the case and upstream
of any census.

The certified artifact is a **pure re-export barrel**. `dist/index.js` is six
`export *` lines and carries no implementation at all, which is also why it is
byte-identical to `dist/index.d.ts` (`sha256:9ccef4b1…` for both). Every export
it names is implemented in a sibling module — `makeEventListener` in
`dist/eventListener.js`, the three wrappers in `dist/callbackWrappers.js` — and
those siblings are *not* identical to their own declarations.

A paired comparison across the packages certified this session:

| package | `index.js` vs `index.d.ts` | implementations in the case's own artifact | claims |
| --- | --- | --- | --- |
| `@solid-primitives/utils@6.4.1` | differ | yes | 56 candidates |
| `@solid-primitives/utils@7.0.0-next.4` | differ | yes | 64 closures, one owner requirement |
| `@kobalte/utils@0.9.2` | differ | **partly** — bundled locals plus cross-package re-exports | 20 of 59 close; **35 are `{"call": {}, "shape": "callable"}`** |
| `@solid-primitives/event-listener@2.4.5` | identical | **none** | 0 of 11 |

`@kobalte/utils` is the row that separates the two candidate causes. It is not
digest-identical, and it still leaves 35 exports empty — the ones it re-exports
rather than implements. So the predictor is not the identical digest; it is
**whether the export's implementation lives in the bytes of the case being
certified**. The identical digest is a symptom of a barrel having no
implementation, not the mechanism.

### This is the `motion-utils` wall, again

`2026-09-14-remaining-frontier-census.md` records the same shape for
`motion-utils`: `easeIn` is *bound* in `ease.mjs` while the body it names is
written in `cubic-bezier.mjs`, and `census_implementation_subject` requires the
stated declaration's path to end with the snapshot's runtime binding path — so
two different files of one package refuse. That measurement cost an ADR attempt
(0108) that was built and reverted the same day.

`makeEventListener` is that case with a barrel in front of it: the declaration
binds at `dist/index.d.ts`, the implementation is in `dist/eventListener.js`,
and those are different files. Nothing about the dialect, the dependency, or the
census is reached.

### What this settles, and what it does not

Settled: the three explanations offered in §§ 9–11 — 1.x authority, dependency
decline, lane decline — are all wrong, and the observation they were attached to
has a much more ordinary cause that this repository already had written down.

Not settled by direct test: no non-barrel entrypoint of this package exists to
certify (its `exports` map has only `.`), so the argument above rests on the
four-package comparison plus the recorded `motion-utils` precedent rather than
on an isolated experiment. A package that publishes both a barrel and a deep
entrypoint would isolate it in one run, and is worth finding before this is
treated as established.

## 13. Falsified: a barrel resolves through to its siblings

§ 12 closed by asking for "a package that publishes both a barrel and a deep
entrypoint" to isolate its claim in one run. `@solid-primitives/utils@6.4.1` is
that package, and the run falsifies § 12.

Its `exports` map has two entries. `.` resolves to `dist/index.js`, which
carries 46 local declarations. `./immutable` resolves to
`dist/immutable/index.js`, which is **six `export *` lines and nothing else** —
a pure re-export barrel with no implementation of its own, the exact shape § 12
said cannot produce claims. Its six siblings import only relative paths: the
whole `./immutable` subtree names **no external specifier at all**.

Four runs, release binary, plain root lane except where stated:

| run | package | entrypoint | artifact shape | external specifiers reachable | exports | with operations |
| --- | --- | --- | --- | --- | --- | --- |
| `bar-root` | `@solid-primitives/utils@6.4.1` | `.` | 46 local declarations | `solid-js`, `solid-js/web` | 40 | 14 |
| `bar-imm` | `@solid-primitives/utils@6.4.1` | `./immutable` | **pure `export *` barrel** | **none** | 35 | **3** |
| `el-plain` | `@solid-primitives/event-listener@2.4.6` | `.` | pure `export *` barrel | `@solid-primitives/utils` | 11 | 0 |
| `el-graph` | same, `--dependency-graph-lane --recover-entrypoints` | `.` | pure `export *` barrel | `@solid-primitives/utils` | 11 | 0 |

`withArrayCopy`, `withCopy` and `withObjectCopy` each carry an `invoke`
operation on their callback argument. All three are written in
`dist/immutable/copy.js` — a *sibling* file, reached through the barrel.

**A barrel resolves through to its siblings.** § 12's predictor — "whether the
export's implementation lives in the bytes of the case being certified" — is
false, and the `motion-utils` precedent it leaned on does not transfer.

### The § 12 argument against the dependency was also invalid

§ 12 dismissed § 11's dependency explanation on the grounds that
`preventDefault`, `stopPropagation` and `stopImmediatePropagation` "call no
Solid primitive and never touch `@solid-primitives/utils`", so they would close
under it. Two facts undercut that.

First, an empty summary is not one thing. `./immutable`'s 35 exports split three
ways:

| summary | count | example |
| --- | --- | --- |
| `{"call": {}}` — degenerate, nothing determined | 20 | `add`, `push`, `sort` |
| `closed: ["callbacks", "creates"]`, no operations — determined to state nothing | 12 | `clamp`, `filter`, `merge` |
| operations recorded | 3 | `withCopy` |

All **eleven** of `event-listener`'s are degenerate `{"call": {}}`. That is not
"a correct contract for a function with nothing to say" — `clamp` shows what
that looks like, and it is a different value.

Second, the comparison § 12 needed is available and points the other way.
`preventDefault` is `callback => e => { e.preventDefault(); callback(e) }`;
`withCopy` is the same shape — take a callback at an argument position, invoke
it. `withCopy` closed with `callback-0`. `preventDefault` is degenerate. Same
lane, same binary, same structural idiom, opposite outcome.

### One hypothesis now fits every row, and it is not tested

`callbackWrappers.js` is **not** in `declinedDependencySpecifiers`; the four
that are are `components.js`, `eventListener.js`, `eventListenerMap.js` and
`eventListenerStack.js`. So if the decline were per *module*, `preventDefault`
would have to close, and § 12's objection would stand.

But the audit records `composedArtifactCases: 1`. The unit certified for
`./dist/index.js` is one composed case built from all six siblings, not six
units. If a decline degenerates the **case**, every export the barrel names goes
degenerate regardless of which sibling declined — `preventDefault` included.
That fits all four rows above, and all of §§ 9–12's observations, including the
one used to reject it.

The graph lane does not clear the decline: `el-graph` records the same four
specifiers, the same 176 `declinedDependencyRecords`, and the same 11 degenerate
exports. So `--dependency-graph-lane --recover-entrypoints` is not the remedy,
whatever the mechanism is.

### Status, plainly

This is the fifth explanation offered for one observation, and four of the
previous ones were asserted from the first evidence that fit. The
case-versus-module reading is stated here as a hypothesis that survives every
row currently measured, not as a finding.

What is established by direct experiment: a barrel resolves through to its
siblings (`bar-imm`), degenerate and closed-empty are distinct outcomes, and the
graph lane does not clear a declined dependency.

The decisive test, not run: supply `@solid-primitives/event-listener` with an
**accepted** `@solid-primitives/utils` contract so that nothing in its subtree
declines, and read the eleven exports again. If they populate, the decline
causes the degeneracy and the unit is the composed case. If they stay
degenerate, this hypothesis dies with the other four. That test needs the
artifact-identity admission path (B′, landed `d7f7c56e`) to carry an acceptance
across two certification work directories, which is itself new and unproven — so
it is a piece of work, not a spot check.

## 14. The decisive test, run: accepting the dependency changes nothing

§ 13 named the test and declined to run it. It has now been run, and it kills
§ 13's hypothesis.

### How the treatment was applied

`certifyContract` deletes its scratch directory in an unconditional `finally`
(`certify-contract.mjs:3928`), so the per-node generations it performs are
invisible after the fact. They were observed by pointing `TMPDIR` at a
scratchpad directory and running an `rsync` poller against
`solid-checker-certify-*` for the duration of the run, copying out every
`*.refusals.json`, `*.certification-inputs.json` and `proposal-dependencies.json`.
No source was modified; the run is the ordinary graph lane.

The graph lane builds each node with its dependencies' contracts in a private
proposal catalog, dependency-first. The captured catalogs show exactly that:

| graph node | catalog entries | specifiers accepted |
| --- | --- | --- |
| node 0 — `@solid-primitives/event-listener` | 11 | `@solid-primitives/utils`, `solid-js`, `solid-js/web` |
| node 1 — `@solid-primitives/utils` | 2 | `solid-js`, `solid-js/web` |
| node 3, node 6 — `solid-js`, `solid-js/web` | 1 | `solid-js` |

So the root **is** regenerated with an accepted `@solid-primitives/utils`. That
is the treatment § 13 asked for, and the graph lane already performs it.

### The result

| root generation | `unaccepted-external-dependency` records | refusals | exports with operations |
| --- | --- | --- | --- |
| partial proposal, no dependency contract | **176** | 0 | 0 of 11 |
| graph node 0, `@solid-primitives/utils` accepted | **0** | 0 | **0 of 11** |

Accepting the dependency clears **every** decline — 176 to 0, in both passes of
the run — and moves nothing. The certified root document carries exactly **one**
summary object, `{"call": {}, "shape": "callable"}`, shared by all eleven
exports.

The same graph, the same transaction, the same binary:

| node | exports | degenerate | closed-empty | with operations |
| --- | --- | --- | --- | --- |
| `@solid-primitives/utils@6.4.1` `.` | 40 | 5 | 21 | **14** |
| `solid-js@1.9.14` `.` | 54 | 40 | 0 | **14** |
| `solid-js@1.9.14` `./web` | 72 | 68 | 0 | **4** |
| `@solid-primitives/event-listener@2.4.6` `.` (root) | 11 | **11** | 0 | **0** |

**The decline is a symptom, not the cause.** § 13's hypothesis — that a decline
degenerates the whole composed case — is dead. It was the fifth explanation, and
it is the fifth to fail.

### What is left standing

Measured, not inferred:

- A pure `export *` barrel resolves through to its siblings: `@solid-primitives/
  utils ./immutable` is six `export *` lines over six sibling files and
  certifies 35 exports, 3 carrying operations (§ 13).
- An unaccepted external dependency is not the cause: accepting it clears 176
  decline records and produces the identical document (this section).
- `@solid-primitives/event-listener`'s own module yields *no semantic content
  whatsoever* — one summary object for eleven exports — while every other node
  in the same graph yields plenty.

No sixth explanation is offered here. The four candidate causes proposed across
§§ 9–13 — 1.x authority, dependency decline, lane decline, implementation in
another file — have each been tested and each has failed, and the honest state
of this observation is that its cause is not known. Anything further should
start from the generator's own analysis of `dist/eventListener.js`, which is
where the content is missing, rather than from the certification machinery
downstream of it.

## 15. The coverage census: what a contract states about what consumers import

§§ 9–14 chased one empty document through five wrong explanations. That was the
wrong shape of question. The question this report exists to answer is whether a
consumer with contracts for the packages they use gets useful feedback, and that
is a coverage number, not a mechanism.

### Method

The demand side was already measured (§ 3): 2,585 consumer call sites, 242
distinct exports, 146 projects, of which 1,958 sites name a package installed in
the corpus. Aggregated by package, demand is extremely concentrated — **two
packages carry 80% of all call sites**, and the whole in-corpus tail is 18
packages.

For each of those 18, the installed version was resolved from the corpus, the
package was certified once at its root entrypoint through the ordinary plain
lane, and every demanded export was classified against the emitted document:
`operations` (the contract states at least one), `closed-empty` (determined to
state nothing), `degenerate` (`{"call": {}}`, nothing determined), or `absent`.
`@solid-primitives/utils` also publishes `./immutable`, certified separately and
folded in, because certifying only `.` would have scored 24 of its demanded
exports as absent when they are simply behind another entrypoint.

### The result

| package | version | call sites | ops | closed-empty | degenerate | absent | sites with a stated operation |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `@kobalte/utils` | 0.9.2 | 942 | **0** | 18 | 23 | 0 | 0 / 942 (0.0%) |
| `@solid-primitives/utils` | 6.4.1 | 820 | 8 | 24 | 19 | 3 | 257 / 820 (**31.3%**) |
| `@kobalte/solidbase` | 0.6.13 | 60 | — | — | — | — | refused: `.` is not exported by the package |
| `@solid-primitives/rootless` | 1.5.3 | 39 | 0 | 0 | 4 | 0 | 0 / 39 (0.0%) |
| `@solidjs/start` | 2.0.0 | 24 | 0 | 0 | 2 | 7 | 0 / 24 (0.0%) |
| `@solid-primitives/platform` | 0.1.2 | 18 | 0 | 0 | 5 | 0 | 0 / 18 (0.0%) |
| `@kobalte/core` | 0.13.12 | 16 | 0 | 0 | 5 | 0 | 0 / 16 (0.0%) |
| `@solid-primitives/trigger` | 1.0.11 | 9 | 0 | 0 | 2 | 0 | 0 / 9 (0.0%) |
| `@solid-primitives/marker` | 0.2.2 | 8 | 2 | 0 | 0 | 0 | 8 / 8 (100.0%) |
| `@solid-primitives/scheduled` | 1.5.3 | 5 | 2 | 0 | 0 | 0 | 5 / 5 (100.0%) |
| `@solid-primitives/memo` | 1.5.1 | 4 | 0 | 0 | 2 | 0 | 0 / 4 (0.0%) |
| `@solid-primitives/keyed` | 1.2.2 | 2 | 0 | 0 | 1 | 0 | 0 / 2 (0.0%) |
| `@solid-primitives/context` | 0.2.3 | 2 | — | — | — | — | refused: closure module `solid-js/types/reactive/signal.js` not found |
| `@solidjs/meta` | 0.29.4 | 2 | 0 | 0 | 1 | 0 | 0 / 2 (0.0%) |
| `@solid-primitives/storage` | 4.4.0 | 1 | 0 | 0 | 1 | 0 | 0 / 1 (0.0%) |
| `permission`, `tween`, `timer` | — | 6 | — | — | — | — | no version resolvable from the corpus |

**14.3%** — 270 of the 1,890 measured call sites import an export whose contract
states any operation at all.

The distribution is bimodal, not uniform. Two tiny packages score 100%; one
mid-size package scores 31%; **everything else scores zero**, including
`@kobalte/utils`, which alone carries 48% of all consumer demand and states
nothing about a single one of its 41 demanded exports.

### The number that actually matters is smaller

"States an operation" is not the same as "can raise a finding about how you use
it". Across every demanded export in the census, the contracts state 29 `invoke`,
15 `read`, 7 `return` and 7 `cleanup` operations — and only **four exports carry
an owner requirement**, the thing that turns a contract into a defect in the
consumer's own code (`SC4001`):

| export | package | call sites |
| --- | --- | ---: |
| `createMicrotask` | `@solid-primitives/utils` | 20 |
| `createMarker` | `@solid-primitives/marker` | 8 |
| `debounce`, `throttle` | `@solid-primitives/scheduled` | 5 |

**29 of 1,890 call sites — 1.5%.** That is the ceiling on useful consumer
feedback from package contracts against this corpus today, and it is reached
only *after* the acceptance gate of § 4 is solved, which no consumer passes at
all right now.

### What this settles

The answer to the question this report was opened to answer is: **no, not yet,
and not by a small margin.** A user who certified every package they import
would get contract-derived feedback at 1.5% of their call sites.

It also reframes §§ 9–14. `@solid-primitives/event-listener` stating nothing is
not an anomaly worth five explanations — it is the *majority* behaviour. Thirteen
of the fifteen measured packages state nothing about the exports their consumers
actually call. Debugging one of them was answering a question nobody needed
answered; the generator's coverage across packages is the problem, and it is
visible without any mechanism hunt.

## 16. Why `@kobalte/utils` states nothing

§ 15 named this the lever: one package, 942 call sites, 48% of all consumer
demand, and zero of its 41 demanded exports state anything. This section answers
why, from the machinery's own census rather than from reading source.

First, the scale of the silence. The certification builds a **78-node** graph and
certifies 632 closures in it — and only three artifact cases produce a single
closure candidate between them (`@solid-primitives/utils`, `solid-js`,
`solid-js/web`). `@kobalte/utils`' own two artifact cases produce **zero**
candidates. Everything certified in that run belongs to something else.

Its 59 exports split: **0** with operations, 24 closed (determined to state
nothing), 35 degenerate. Every one of the 35 has an explicit recorded reason, and
they partition cleanly with no overlap:

| cause | exports | where recorded |
| --- | ---: | --- |
| cross-package re-export | 9 | artifact-case refusal |
| declined at generation | 13 | `declinedClosures`, closure-proposal stage |
| withheld at certification | 12 | `withheldClosures` |
| non-callable (`EventKey`, a `var` object) | 1 | — |

### 1. Re-exported names do not bind (9 exports)

`dist/index.js` opens with eight `export … from` lines pulling `access`,
`accessWith`, `chain`, `mergeRefs`, `combineProps`, `createEventListener`,
`createMediaQuery`, `Key` and `ReactiveMap` out of five other packages. All nine
are degenerate, and the artifact case says why:

```
accepted dependency @solid-primitives/keyed has no exact runtime binding
for export Key
```

This is the binding wall already recorded for core-runtime re-exports, reached
here through an ordinary package.

### 2. The `creates` census cannot resolve a call whose receiver is a parameter (13)

41 decline records, **every one in the `creates` domain**: 22 `unresolved-callee`
and 19 `refusing-callee-fixpoint`. The `unresolved-callee` rows carry the shape
that defeated them — 13 `member-property-unresolved`, 7 `parameter-rooted`, 2
`computed-member` — and the source at those offsets is the point:

```js
node.contains(element)                    // parameter-rooted
node.matches(selector)                    // parameter-rooted
eventTarget.addEventListener(type, ...)   // parameter-rooted
handler[0](handler[1], event)             // computed-member
polygon.map(point => point.join(","))     // member-property-unresolved
```

`@kobalte/utils` is a **DOM utility library**. Its functions take a DOM node as a
parameter and call a method on it. The concrete receiver exists only at the
consumer's call site, so there is no declaration inside the package to resolve,
and the census fails closed — which is what the precision contract requires it to
do.

The 19 `refusing-callee-fixpoint` rows are that refusal *propagating*:
`callHandler` refuses on `handler[0](handler[1], event)`, and then
`composeEventHandlers` refuses at its call to `callHandler`. Refusal is
transitive through the local call graph, which is why a purely syntactic
predictor ("calls another local function") matches 43 of 50 local exports without
being the mechanism.

### 3. The same wall at certification, in different words (12)

The remaining twelve are withheld later, again entirely in `creates`:

| reason | rows |
| --- | ---: |
| `creates census refuses an uncensused invoking form: property-access-unknown-accessor` | 16 |
| `creates census finds no function-like declaration node for "getComputedStyle"` | 6 |
| `creates census refuses an uncensused invoking form: coercion (BinaryExpression)` | 2 |

`getComputedStyle`, `navigator`, `window` — DOM globals with no body to analyse.
`isMac`, `isIOS`, `isWebKit`, `isFocusable`, `isTabbable` and friends are all
here.

### What this means

`@kobalte/utils` does not state nothing because of a barrel, a dependency, a
lane, or a bug. It states nothing because **almost everything it does is call a
method on a DOM value it was handed**, and the `creates` census is required to
fail closed on exactly that. The package is close to a worst case for this
analysis, and it happens to be the most-imported package in the corpus.

That generalises the § 15 result rather than explaining it away: a contract
pipeline whose census cannot see through parameter-rooted DOM calls will state
nothing about most of a UI component library's utility layer, and UI component
libraries are what Solid consumers import.

### One thing worth a second look

`mergeDefaultProps` is the single most-called export in the whole corpus — 254
sites — and it is *closed*, not degenerate: the analysis looked at it and
determined it states nothing. Its body is `return mergeProps(defaultProps, props)`.
`mergeProps` returns a reactive proxy, and destructuring that proxy is a
reactivity defect of precisely the kind this checker exists to prove. Whether
"returns a reactive proxy whose properties must not be destructured" is
expressible in the current operations model is not settled here, and is worth
establishing before anyone concludes the 24 closed exports are all correctly
closed.

## 17. Two tests of § 16's implication, both against it

§ 16 read as though a `creates` refusal explains a silent package, and as though
the re-export binding wall were a lever. Both were checked.

**A `creates` decline does not silence a package.** Declines are ubiquitous —
every package in the graph carries them, `solid-js` most of all:

| package | declines | domains | still states |
| --- | ---: | --- | --- |
| `solid-js` | 288–378 | `reads` (`runtime-accessor-installation`) | 14 exports with operations |
| `@solid-primitives/utils` | 4 | `creates` (`unresolved-callee`) | 20 operations, 4 kinds |
| `@solid-primitives/media` | 23 | `creates` + `reads` | — |
| `@kobalte/utils` | 41 | `creates` only | **nothing** |

`@solid-primitives/utils` carries the *same* decline kind in the *same* domain as
`@kobalte/utils` and still states 10 `invoke`, 7 `read`, 2 `return` and 1
`cleanup`. So "the census refused" is not an explanation on its own; the
difference is density and what is left over, not the presence of refusals.

**Binding the re-exports would recover 68 call sites, not 942.** The nine names
`@kobalte/utils` re-exports were traced to their source packages and certified
there:

| name | source | at source | sites |
| --- | --- | --- | ---: |
| `mergeRefs` | `@solid-primitives/refs` | **closed** | 164 |
| `access` | `@solid-primitives/utils` | ops | 66 |
| `accessWith` | `@solid-primitives/utils` | ops | 2 |
| `createEventListener` | `@solid-primitives/event-listener` | **degenerate** | 2 |
| `createMediaQuery` | `@solid-primitives/media` | **degenerate** | 2 |
| `Key` | `@solid-primitives/keyed` | **degenerate** | 2 |
| `chain`, `combineProps`, `ReactiveMap` | — | closed / degenerate | 0 |

Only `access` and `accessWith` state anything at their source. Fixing the
binding wall recovers **68 of 942 sites — 7%**. The reactive content
`@kobalte/utils` re-exports is mostly absent at the source too.

So the silence is not one defect with one fix. It distributes across three
populations — correctly closed pure helpers, structurally unresolvable
parameter-rooted DOM calls, and a binding defect worth 7% — and no single lever
moves the § 15 number much.

## 18. `mergeDefaultProps`: the contract is not the constraint

§ 16 flagged the corpus's most-called export — 254 sites, `closed` rather than
degenerate, body `return mergeProps(defaultProps, props)` — and asked whether
"returns a reactive proxy" is expressible. Four things were measured.

### 1. It is not "determined to state nothing"

```json
{"call": {"closed": ["creates"], "creates": [], "proposedClosures": ["creates"]}}
```

It closed **`creates` only**. It is silent on `reads`, `callbacks` and
`returns`. Checking this across both censused packages: **no export in either
closes all four domains.**

| package | closed-empty exports | domains actually closed |
| --- | ---: | --- |
| `@kobalte/utils` | 24 | 21 × `creates`; 3 × `callbacks`+`reads` |
| `@solid-primitives/utils` | 21 | 9 × `callbacks`+`creates`; 6 × `callbacks`+`reads`; 4 × `creates`; 2 × `reads` |

So § 15's "closed-empty = determined to state nothing" is too generous, and the
census's `closed` column should be read as *partially* closed. Nothing in this
corpus is fully determined.

### 2. It would not matter if the contract were perfect

`solid-js`'s own generated contract states nothing about `mergeProps`. Of its 54
root exports, 14 carry operations and **40 are degenerate** — including
`createSignal`, `createEffect`, `createMemo`, `createComputed`, `createRoot`,
`createRenderEffect`, `createSelector`, `batch`, `untrack`, `splitProps` and
`mergeProps`. The 14 that speak are mostly JSX components (`For`, `Index`,
`Show`, `Switch`, `Match`, `ErrorBoundary`) plus `children`, `createContext`,
`createResource`, `lazy`, `onCleanup`, `useTransition`, `from`, `createDeferred`.

### 3. The defect is not reported even with no wrapper at all

A project with solid-js 1.9.14 and nothing else:

```tsx
export function Direct(props: P) {
  const { label } = props;                            // SC1003 ✓
}
export function ViaMergeProps(props: P) {
  const merged = mergeProps({ x: 1 }, props);
  const { label } = merged;                           // nothing
}
export function ViaMergePropsInline(props: P) {
  const { label } = mergeProps({ x: 1 }, props);      // nothing
}
```

One finding, `analysisContext: "Direct"`. Destructuring a `mergeProps` result is
not reported in either spelling. **The `@kobalte/utils` wrapper hides nothing,
because the un-wrapped call is not covered either.**

### 4. It *is* expressible; the row is simply absent

`static_rules.rs:231` accepts three ways for a destructured object to be
reactive: the initializer's symbol is a known prop source; **a contract whose
`summary.returns` has `kind == "store-path"`**; or a dialect primitive for which
`returns_store` holds. And:

```rust
fn returns_store(&self, primitive: Primitive) -> bool {
    matches!(primitive, Primitive::CreateStore | Primitive::CreateMutable)
}
```

Two rows. `mergeProps` is not one of them. So this is not a limit of the
operations model — the model has `store-path` and the rule already reads it.
It is a missing audited dialect row, and the contract path would work the moment
a contract said `store-path`.

### What this changes

The lever § 15 was looking for is not in the contract pipeline at all. A perfect
contract for `mergeDefaultProps` changes nothing while `mergeProps` itself is
uncovered, and covering `mergeProps` is a **dialect** row plus its 2.0
counterpart — not a certification problem.

Worth noting for whoever picks this up: SC1003's own hint reads "To split or
default props, use `splitProps(props, ...keys)` and `mergeProps(defaults, props)`
instead of destructuring." The advice is sound — reading `merged.label` in JSX
tracks correctly, and the `Clean` case above stays clean. But a consumer who
follows it and then destructures the merged object gets no warning.

Not established here: whether eslint-plugin-solid 0.14.5 reports this. The
`upstream_compat` port carries no `mergeProps` row, and the single upstream
parity case using it (`upstream/reactivity__valid__05`) is a *valid* case, so
nothing currently pins the destructure-the-result behaviour in either direction.
That should be checked against the upstream source before a row is added.

## 19. What eslint-plugin-solid does with `mergeProps`, and why the § 18 flag retires

Read at the pinned revision `6d3bc311` per `.claude/skills/upstream-parity`.

### Upstream models it

`packages/eslint-plugin-solid/src/rules/reactivity.ts:740`:

```ts
} else if (matchImport("mergeProps", callee.name)) {
  const merged = id && getReturnedVar(id, context);
  if (merged) {
    scopeStack.pushProps(merged, currentScope().node);
  } else {
    warnShouldAssign(id ?? init);
  }
}
```

Two behaviours fall out. Assigned to a variable, the result becomes a
**props-kind reactive variable**. Destructured inline, `getReturnedVar` returns
`null` for an `ObjectPattern` and upstream reports `shouldAssign` — *"For proper
analysis, a variable should be used to capture the result of this function
call."*

A props-kind variable referenced outside a tracked scope in its declaration
scope reports `untrackedReactive`, and that mechanism **is** pinned upstream
(`test/rules/reactivity.test.ts`, invalid section):

```js
const Component = props => {
  const { value: valueProp } = props;   // untrackedReactive on `props`
  const value = createMemo(() => valueProp || "default");
  return <div>{value()}</div>;
};
```

solid-checker has parity on exactly that case —
`fixtures/ownership-cases/cases.json` pins it as
`upstream/reactivity__invalid__03` expecting `v1/no-destructure`, and § 18's
`Direct` measurement confirms SC1003 fires.

### The divergence, and what pins it

| shape | upstream 0.14.5 | solid-checker | pinned |
| --- | --- | --- | --- |
| `const { x } = props` | `untrackedReactive` | **SC1003** | both sides |
| `const m = mergeProps(a,b); const { x } = m` | `untrackedReactive` (same path) | nothing | neither |
| `const { x } = mergeProps(a,b)` | `shouldAssign` | nothing | neither |
| `const m = mergeProps(a,b); <div>{m.x}</div>` | valid | valid | both sides |

Upstream has exactly one `mergeProps` test and it is the **valid** one
(`reactivity__valid__05` here), so the two divergent shapes are unpinned
upstream as well as here. The divergence is real and derives from upstream
source, not from a pinned upstream expectation.

### Incidence in the corpus: zero

Before treating that as a gap worth closing, it was counted across the four
consumer repositories:

| | |
| --- | ---: |
| files containing a `mergeProps`/`mergeDefaultProps` call | 161 |
| calls | 167 |
| merged results bound to a variable | 162 |
| **inline destructures of a merge call** | **0** |
| **destructures of a merged variable** | **0** |

The detector is not vacuous: the same pattern finds 10 object-destructures of a
bare variable elsewhere in the same corpus.

### Conclusion

`@kobalte/utils`' `mergeDefaultProps` wraps a primitive that solid-checker does
not cover and upstream does. The divergence is genuine. It is also worth
**nothing on this corpus** — not one of the 167 merge calls destructures its
result, so adding the row would move no finding here.

§ 18 flagged this as the one unexamined item that could still be large. It is
not. The § 15 ceiling stands at 1.5% with no lever behind it, and the honest
next question is the one § 15 already posed: whether the checker's
contract-independent rules are where the value is.

## 20. The contract-independent rules, measured against the corpus

§ 15 measured what package contracts deliver. This measures what the rest of the
checker delivers on the same code, so the two can be compared.

### Method

A fresh `make build-checker-release` — the previous release binary predated
three Rust commits — run over every `tsconfig.json` in `corvu`, `kobalte` and
`solid-docs` (31 projects, 861 `.ts`/`.tsx` source files), default settings, no
presets, no accepted contracts. Findings deduplicated on `(rule, path, exact
span)`, because the monorepo roots re-include their packages' sources and the
raw total double-counts by roughly half.

`solid-primitives`, the fourth demand root, is not checked out here; this is
three of the four.

### Result: 965 unique findings

| rule | total | violation | uncertifiable | files |
| --- | ---: | ---: | ---: | ---: |
| `SC9005` package-contract-incomplete **[contract]** | 465 | 0 | 465 | 324 |
| `SC1001` strict-read-untracked | 175 | 71 | 104 | 68 |
| `SC9012` reactive-dispatch-unresolved | 149 | 0 | 149 | 58 |
| `SC4001` missing-owner | 90 | 0 | 90 | 64 |
| `SC7001` missing-effect-function | 42 | 0 | 42 | 34 |
| `SC2001` reactive-write-in-owned-scope | 18 | 18 | 0 | 11 |
| `SC8015` prefer-show *(preference)* | 9 | 9 | 0 | 4 |
| `SC2003` no-direct-mutation | 9 | 9 | 0 | 2 |
| `SC9011` reactive-source-uncaptured | 6 | 0 | 6 | 6 |
| `SC1007` reactive-handler-frozen | 2 | 2 | 0 | 1 |

**109 contract-independent violations** and 391 contract-independent
uncertifiables, against 465 contract-dependent uncertifiables. By repository:
kobalte 89, corvu 15, solid-docs 5; five of the 109 are in test files.

Set against § 15: the contract pipeline produced 0 violations here and 465
"cannot tell". The rules that need no contract produced 109 proven-defect
claims. **That is the entire product-value comparison, and it is not close.**

### Quality: one confirmed false-positive class

`SC2003 no-direct-mutation` fires 9 times, all in `@kobalte/core`, and all nine
are wrong:

```tsx
ref()!.style.transitionDuration = "0s";   // collapsible-content.tsx:107
inputRef()!.value = formattedValue;       // number-field-root.tsx:291
```

`ref` is a signal accessor returning an `HTMLElement`. Writing
`.style.transitionDuration` on a DOM element is ordinary, correct Solid —
kobalte does it deliberately, with a comment explaining why. The finding claims
*"Solid hands out a readonly proxy, so the write is dropped"* and hints *"Props
are readonly by design"*, and neither applies: the write target is a DOM node,
not a props or store proxy, and the write is not dropped. These are
`kind: "violation"`, so they are proven-defect claims, not hedged ones.

### A suspicion that did not survive testing

Reading the `SC1001` findings in situ suggested a second false-positive class —
signals read inside event handlers and inside `createEffect`. A minimal case
says otherwise:

| shape | fires? |
| --- | --- |
| signal read inside an event handler | **no** |
| signal read inside `createEffect` | **no** |
| signal read once at component setup | yes, `uncertifiable` |

So `SC1001`'s core discrimination is right, and its 71 corpus violations are
**unassessed**, not presumed wrong. Reading findings next to source is not
evidence; this is the same mistake §§ 9–13 kept making, caught here before it
reached a conclusion.

### Two robustness notes

`kobalte/packages/core` — the single most important project in the corpus —
initially **refused outright**: `policy-2 acceptance receipt requires
authenticated issuer provenance`. The cause was a `.solid-checker/` catalog
directory left behind by this session's own certification runs. A stale catalog
does not degrade to "ignore it and analyse anyway"; it fails the whole project.
Moving the directory aside recovered all 149 of that project's findings.

And `SC4001`'s 90 findings are all `uncertifiable` — "I cannot prove this effect
has an owner", not "this effect leaks". The minimal case shows it firing on a
`createEffect` inside an ordinary component. That is fail-closed behaviour
working as specified, but from a user's seat 856 of the 965 findings say
"cannot tell", and that ratio is the honest headline for the current output.

## 21. Assessing the 71 `SC1001` violations

§ 20 left these unassessed after a suspicion about them failed a minimal test.
They have now been assessed by reproduction rather than by reading.

### The split

| | count |
| --- | ---: |
| `"X" is read **directly** in C` | 42 |
| `"X" is read **through** H in C` | 29 |

and by the enclosing context the message names:

| enclosing context | direct | through a helper |
| --- | ---: | ---: |
| `rendering function` (component setup) | 27 | 5 |
| a named function — `onPointerDown`, `onHoverOutside`, `onInput`, `onFocus`, `onBlur`, `onItemLeave`, `toggle`, `focusContent`, `tabIndex`, `fill`, `stroke`, `borderWidth`, `strokeWidth`, `highlighted`, `half`, `open` … | 15 | **24** |

### A reproduced false-positive class: one call of indirection drops the execution role

Same file, same handler, same accessor, same element — the only difference is
whether the read is direct or one call away:

```tsx
const [ref, setRef] = createSignal<HTMLElement>();
const widthOf = () => ref()?.clientWidth;

const onPointerDown = () => {
  const direct = ref();          // silent — correct
  const viaHelper = widthOf();   // SC1001 *violation*
};

return <div ref={setRef} onPointerDown={onPointerDown} />;
```

And the same asymmetry for a derived accessor used in JSX, which is the
`tabIndex` / `fill` shape:

```tsx
const doubled = () => n() * 2;
const direct = () => (n() > 0 ? 0 : -1);           // silent
const viaHelper = () => (doubled() > 0 ? 0 : -1);  // SC1001 *violation*
return <><span tabIndex={direct()} /><span tabIndex={viaHelper()} /></>;
```

Both reads are correct Solid: an event handler runs at event time and a derived
accessor is tracked where JSX calls it. The direct read is exempted; the
propagated one is not. The evidence line shows where it goes wrong — it judges
the **call site** rather than the enclosing scope:

```
"ref" is a reactive accessor
widthOf reads the reactive accessor
the call to widthOf propagates that read into onPointerDown
the call is outside every compiler-tracked JSX region and deferred callback
```

The last line is true of the *call* `widthOf()` and irrelevant: the call sits
inside `onPointerDown`, whose role already exempts a direct read of the same
accessor two lines above. Interprocedural propagation carries the read but not
the role.

**This accounts for the 24 via-helper findings in a named context.** The
mechanism is verified on two shapes; that each of the 24 named contexts is
genuinely fresh-at-call-time is inferred from the context name, not proven
case by case.

### What is not a false positive

The 32 findings in a `rendering function` (27 direct, 5 through a helper) are
the rule's core shape — a read at component setup, frozen — and nothing here
contradicts them. The remaining 15 direct reads in named contexts are
**unassessed**: minimal reproductions of two of their shapes (a read inside an
arrow stored as an object property, and a prop bound to a native event handler)
came back `uncertifiable` rather than `violation`, so the corpus versions prove
something the minimal ones do not, and reading them is not evidence.

### A separate observation: two rules, one span

`SearchItem.tsx:18-19` carries `SC1001` *and* `SC1007` on the same spans, and a
minimal case reproduces the pair. `SC1007` says the listener is installed once;
`SC1001` says the read sees the current value once. On a prop bound to a native
event handler those are close to the same claim about the same bytes. AGENTS.md
permits a different claim about the same code and forbids a duplicate one, so
this is worth deciding rather than leaving.

### Revised corpus picture

| | count |
| --- | ---: |
| reproduced false-positive class (role lost through a helper) | **24** |
| core shape, setup-time reads, no evidence against | 32 |
| unassessed | 15 |

Against § 20's 100 contract-independent violations, a fix for the propagation
asymmetry would remove about a quarter of them.

## 22. Can the `@kobalte/utils` contract resolve SC1001's 24? No, and each link is measured

§ 21 found 24 SC1001 false positives; `cef049db` fixed the shape reachable by
execution-role classification and did not move the corpus, because the corpus
binds its handlers through a component prop and through `addGlobalListener`.
The remaining route is the package contract. It is blocked at four points.

### 1. Only a minority of the 24 are contract-mediated at all

| binding of the enclosing function | count | contract could help? |
| --- | ---: | --- |
| `addGlobalListener(...)` from `@kobalte/utils` | 4 | in principle |
| `onPointerDown={…}` on `<Polymorphic>` — a `@kobalte/core` component prop | 4 | different package |
| derived accessors called from JSX (`tabIndex`, `fill`, `stroke`, `borderWidth`, `strokeWidth`, `highlighted`, `half`, `open`) | 9 | **no external call involved** |
| other named handlers | 7 | mixed |

Nine of the 24 never cross a package boundary; no contract can reach them.

### 2. Acceptance rejects the contract that exists

`@kobalte/utils@0.9.2` was certified (§ 15) and the resulting case set fed back
to `kobalte/packages/core` with `--accepted-contracts` and
`--receipt-trust-configuration`. The result is **identical to the baseline**:

| | baseline | with the accepted contract |
| --- | ---: | ---: |
| `SC9005` | 632 | 632 |
| of which `"no receipt-accepted contract matches this exact import"` | — | 597 |
| `SC1001` violations | 62 | 62 |

The receipt's binding names `importer` = the synthetic certification project's
shim module, and carries **no `artifactAcceptanceRoot`** — the B′ field added
this session (`0da407d9`) that would admit by artifact identity instead of by
importer path. The case set predates it.

### 3. The contract states nothing for the exports that matter

```
createGlobalListeners : {"call": {}, "shape": "callable"}
callHandler           : {"call": {}, "shape": "callable"}
composeEventHandlers  : {"call": {}, "shape": "callable"}
```

### 4. Two of the three shapes are not expressible, and the third is a real defect

A minimal package, certified through the generator directly:

| shape | result |
| --- | --- |
| `callHandler(e, h) { h(e) }` | **states the claim** — `callbacks: [{from: {arg: 1}}]`, all four domains closed |
| `addListener(target, type, listener) { target.addEventListener(type, listener) }` | closes reads/creates/returns, **no callbacks claim** — the listener goes to a parameter-rooted DOM method |
| `createListeners() { … return { add } }` | **nothing about the returned object's method** |

`addGlobalListener` is shape 2 reached through shape 3, so the four
contract-mediated findings need both of the shapes that produce nothing.

**And a genuine defect in shape 1.** Adding the real `callHandler`'s second
branch removes the claim the same function otherwise carries:

```js
export function callHandler(event, handler) {
  if (handler) {
    if (isFunction(handler)) handler(event);      // resolvable invocation
    else handler[0](handler[1], event);           // computed-member, declines
  }
}
```

| | `callbacks` claim | `closed` |
| --- | --- | --- |
| without the second branch | `[{from: {arg: 1}}]` | `callbacks, reads, creates, returns` |
| with it | **none** | `reads, returns` |

One `unresolved-callee` decline in the **creates** domain removes an
independently-proven **callbacks** claim. `handler(event)` invokes parameter 1
whatever the other branch does. This is worth fixing on its own — `callHandler`
has 112 call sites in the corpus, the third-most-demanded export — but note the
honest limit: the claim is a lower bound, so it should be *stated without
closing the callbacks domain*, and closing it while a branch is unresolved
would be unsound.

### What this settles

"Fix the `@kobalte/utils` contract so those 24 resolve" cannot be done. Nine of
the 24 involve no package boundary, four belong to `@kobalte/core`, acceptance
rejects the contract regardless of its content, and the four that remain need
two shapes the format does not express. The one actionable defect found on the
way — a creates decline erasing a proven callbacks claim — is real, reproducible
in twelve lines, and resolves none of the 24.

## 23. The open-claims gate, mapped — and § 22's "defect" corrected

With artifact admission working (`3426ed5a`), `kobalte/packages/core` moves 471
acceptance-gate findings to **1037 open-claims** findings. This is what they
ask for.

### Correction: § 22's callbacks-claim defect is not a defect

§ 22 concluded that "a creates decline erases an independently-proven callbacks
claim" and called it worth fixing. Two controlled runs falsify that.

| export | body | creates decline | callbacks claim |
| --- | --- | --- | --- |
| `clean(handler)` | `handler()` | no | **stated** |
| `unrelatedDecline(node, handler)` | `node.setAttribute(…)`; `handler()` | **yes** | **stated** |
| `declineOnCallback(handler)` | `typeof` guard; `handler[0](handler[1])` | **yes** | **stated** |
| `outerGuard(handler)` | `if (handler)` + the above | **yes** | **stated** |
| `extraParam(event, handler)` | same, but `handler[0](handler[1], event)` | yes | **absent** |

A creates decline does not erase the claim — three exports keep it while
declining. The discriminator is narrower: the claim is dropped when **another
parameter flows into the unresolved callee**. `outerGuard` and `extraParam` are
structurally identical apart from `event` being passed to `handler[0](…)`.

That is still an inconsistency worth recording — two functions of the same shape
disagree on whether the provable "argument 1 is invoked" survives — but it is
not the creates/callbacks bleed § 22 described, and the fix is a design choice
rather than an obvious repair: stating it in both cases widens a branch-
dependent claim, dropping it in both raises the open-claims count.

### What the 1037 actually ask for

| export | findings | domain(s) wanted | where a fix would live |
| --- | ---: | --- | --- |
| `mergeRefs` | 235 | callbacks; reactiveReads, returns, ownerRequirements | **re-export binding** — `@solid-primitives/refs`, and its contract is `closed` at source, so binding alone gains nothing |
| `access` | 185 | callbacks; reactiveReads, returns, ownerRequirements | **re-export binding** — `@solid-primitives/utils`, which *does* state operations at source |
| `callHandler` | 196 | callbacks; reactiveReads, ownerRequirements | the parameter-flow drop above |
| `mergeDefaultProps` | 127 | reactiveReads, returns | § 18 — wraps `mergeProps`, itself degenerate in solid-js's own contract |
| `createGenerateId` | 60 | callbacks; reactiveReads, returns | local |
| `snapValueToStep`, `clamp`, `contains`, `composeEventHandlers`, `focusWithoutScrolling` | 104 | mixed | local |

### There is no single fix

The 1037 split across three subsystems with different owners, and **none is a
majority**:

- **re-export binding** — 420 findings (40%), but only `access`'s 185 would
  actually gain claims, because `mergeRefs` states nothing at its source either.
  Certification currently refuses the whole artifact case here
  (`accepted dependency @solid-primitives/keyed has no exact runtime binding for
  export Key`), so this is a certification blocker, not a claim gap.
- **the parameter-flow drop** — 196 findings (19%), a design decision.
- **the DOM-call census** (§ 16) — the local exports, where the refusals are
  correct fail-closed behaviour on `node.contains(el)`-shaped calls.

"Fix the open-claims gate" is therefore not one change. The gate itself is
right: it says the contract does not state what the consumer needs, and it
does not. What is missing is contract *content*, in three places at once.

## 24. Re-export binding: the wall was not where § 16 put it

§ 23 named re-export binding the largest single lever (420 of 1037) and recorded
the blocker as a **certification refusal** — `accepted dependency
@solid-primitives/keyed has no exact runtime binding for export Key`. That is
wrong for the configuration the measurement actually used, and the correction
matters because it moves the work from certification into generation.

### The binding is not the wall

Calling `resolvePackageArtifacts` directly settles it in one run. With no
accepted dependencies, `@kobalte/utils` refuses exactly as § 16 quoted. Supply
`@solid-primitives/keyed`'s own resolution as an accepted dependency and the
refusal moves to the next specifier; supply all seven and the package resolves
with `Key`, `access` and `mergeRefs` bound to their exact dependency modules.
The 2-pass graph-lane run already does this — its 78-node graph has one node per
`(importer, specifier)` edge, and the `@kobalte/utils` contract it produced on
2026-09-14 **contains** all nine cross-package re-exports in its export map.

They are bound. They state nothing. Those are different facts, and § 16
conflated them because the refusal text was the only evidence in view.

### Two mechanics, both in emission

**Named re-exports never consulted the dependency's contract.**
`contract_exports_for_entry_file` asked `Program::contract_exports` first. The
project analysis emits a fragment for every export specifier, and an external
`export { Key } from "@solid-primitives/keyed"` has no local declaration to
walk, so the fragment degrades to `{"call":{}}`. That degenerate entry always
exists, so `accepted_reexport_summary_for_name` — written for exactly this
case — was reachable only for `export *`. Asking the accepted identity first
fixes it.

**An open sibling then erased what survived.** With the order flipped, the
projection for `access` arrives as `cb=Known([parameter 0, inline])`,
`open={Returns}` — and the emitted contract was still byte-identical. The step
between is unresolved-claim attribution: re-exporting a name whose dependency
contract leaves domains open raises `PackageContractExportMissing` *at the
re-export statement*, which encloses no function, so the ladder falls through to
`fallback-all` and marks every export unknown. `@kobalte/utils` raises 18 of
these from its nine re-exports. The claims it inherits are restored after every
attribution channel has run, because an ESM re-export binding is immutable and
its target lives in the dependency's archive.

### What changed, exactly

Re-certifying `@kobalte/utils@0.9.2` with the same inputs:

| | before | after |
| --- | ---: | ---: |
| exports stating something | 24 / 59 | **26 / 59** |
| exports changed | — | 2 |

`access` and `accessWith`, both gaining `callbacks: [{arg 0}]` with an
`invoke` / `same-stack` / `untracked` operation. Nothing else moved — and that
is § 23's own prediction holding: of the 420 findings attributed to this lever,
only `access`'s 185 had a source contract stating anything. `mergeRefs` (235)
is `closed` and empty at `@solid-primitives/refs`, so binding it correctly
publishes nothing, correctly.

### What is still not known

Whether those 185 consumer findings resolve. The inherited claim is an
*operation*, not a *closure*: `access` publishes no `closed` array, so a
consumer demanding `reactiveReads`, `returns` or `ownerRequirements` still meets
the open-claims gate with a narrower list. Measuring that needs the
`kobalte/packages/core` census re-run against a catalog that binds its
importers; the isolated harness used here does not. **The 1037 has not been
re-measured.**
