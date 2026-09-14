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
