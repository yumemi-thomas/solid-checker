# The contract story, assessed against its two goals

- Written 2026-09-23. Every tier, demand and report number here is printed by
  [`2026-09-23-what-a-consumer-can-use.mjs`](2026-09-23-what-a-consumer-can-use.mjs),
  which reads the compiled-in tier in `pkg/contracts/accepted/` (last
  regenerated in 47566ff8), the frozen demand rows, and the pinned
  `benchmarks/ecosystem/report.json` (finished 2026-09-14).
- The recipe and ADR 0112 figures (127 degenerate sites, 89.0%, 80 of 366
  recipes) are quoted from the 2026-09-18 re-pins of `coverage-census.json` and
  `probe-recipe-addressing.json` and the backlog entries of that date, none of
  which was committed when this was written.
- Run over the tier, the script reproduces the pinned coverage census to the
  digit (440 / 442 / 270 / 722 / 84, and 34 owner-requirement sites). The join
  below is the census's own, asked a different question.

The two goals, as stated:

1. certify most Solid 2.0 packages;
2. report a finding when a consumer uses a package in a way the package does
   not support.

## Verdict

The trust model holds and should be kept. Neither goal is met. Certification
issues an acceptance receipt for nearly every package it attempts, but the
contracts mostly close nothing a consumer needs, and in the one A/B measured no
violation on consumer code came from a contract. The method behind every recent
gain (one reviewed premise per invoking form, one hand-written recipe per claim)
is not on course for the first goal. The second needs claims the tier does not
carry and, for package-specific rules, a vocabulary the model does not have.

## What holds

- **A stated claim is established.** Closed claims are bound by an acceptance
  receipt to one artifact case, a probe gate can only veto, unknown is never
  read as negative, and the census keeps a determined negative apart from a
  degenerate summary.
- **One seam serves every rule.** `project_export_semantics`
  (`rust/crates/solid-reactive-ir/src/contracts.rs`) projects an accepted
  export into the same interprocedural indexes the analysis builds for project
  code, so a closed claim domain reaches every rule without a rule written for
  contracts.
- **Delivery needs no user action.** The compiled-in tier applies by artifact
  identity, so a project whose lockfile reproduces a bundle's artifact analyses
  against it without certifying anything.

## Goal 1: certifying Solid 2 packages

**Receipts, yes.** In the pinned report's Solid 2 half (139 packages, 250
probes), 230 of 241 attempted certifications verify, and 210 of those put every
declared entrypoint under receipt.

**Content, mostly not.** Across the same rows, none of 3,985 exports has every
claim domain closed. Five domains are unknown on all 3,985 (`writes`,
`invalidates`, `throws`, `cleanups`, `disposals`) because no implementation
census decides them. A consumer reads four others (`callbacks`, `reads`,
`returns`, `creates`), and the shipped tier shows how far those get:

| the tier: 137 bundles, 16 packages | exports |
| --- | ---: |
| every export | 1,166 |
| degenerate: states and closes nothing | 1,026 (88.0%) |
| values the contract states are not callable | 41 |
| callables closing 0 / 1 / 2 / 3 of the four | 1,039 / 52 / 21 / 13 |
| callables closing all four | **0** |

Across the callables, `reads` is closed 73 times, `creates` 33, `callbacks` 20
and `returns` 7.

**Component libraries are blank, and their wall is composition.**
`@kobalte/core@2.0.0-alpha.0` is 972 of the tier's exports, and 953 of them are
degenerate. Of the 73,991 closures the pinned report records it declining,
63,784 are `unaccepted-external-dependency`. A component closes a domain only
once every dependency export it reaches is accepted and closed, so closure
multiplies across the dependency graph, and the tier closes all four domains
on no callable anywhere, dependency or not.

**Coverage grows one reviewed decision at a time.** Most ADRs from 0034 on
admit one census premise, one invoking form or one recovery path, and the
recent ones are priced in tens of demanded sites. Forty-one hand-written `reads`
recipes closed 84 (2026-09-18), ADR 0112 closed 59, and ADR 0103 closed none
until 0112 narrowed the guard in front of it. The census's remaining 127
degenerate sites are four further reviewed decisions, one of which needs a new
human audit reading of `@solidjs/signals.onCleanup` (`docs/precision-backlog.md`,
"Where the remaining 127 degenerate sites sit"). Two costs recur on top:

- A recipe is keyed by semantic claim id, a content digest, so it does not
  survive a change to the proposal or to the artifact. Of the 214 in-scope
  recipes, 80 address a live claim and 134 are stale
  (`benchmarks/ecosystem/probe-recipe-addressing.json`).
- A bundle applies to one exact `(package, version, integrity, entrypoint,
  conditions)`, and the tier's packages are alpha and `next` releases. A user
  on the next publish of any of them falls back to no contract, silently.

ADR 0001 chose independent proof so the normal certification path would need no
human review, because the alternatives "either admit silent false-negative
certification or prevent package-scale automation". The frontier is now human
work per export and per version.

## Goal 2: findings when a package is misused

**What an import sees.** An open claim domain raises `SC9005` (error,
uncertifiable) as follows:

- `reads` or `creates` open: at the import binding.
- `returns` open: at the import binding, unless every reference to it is a call
  whose result is discarded.
- `callbacks` open: at each call that passes a callable argument.

(`push_unknown_contract_claims` in `contracts.rs`; the argument obligation in
`interproc.rs`.) Over the 1,152 demanded sites whose export is in the Solid 2
surface:

| what the import finds open | sites |
| --- | ---: |
| nothing, and every one is a non-callable value such as `isServer` | 158 (13.7%) |
| `reads` or `creates`: SC9005 wherever the name is imported | 555 (48.2%) |
| only `returns` or `callbacks`: SC9005 on some uses | 439 (38.1%) |
| nothing, on a callable | **0** |

`creates` is open on 531 of the 555 and `reads` on 427. `returns` is open on 436
of the 439.

**The census asks a weaker question.** It files a summary as determined once it
states one operation or closes one domain. So 742 of its determined sites are
callables that still leave a consumer domain open, and 303 of those raise SC9005
wherever imported. Meanwhile 18 of its degenerate sites,
`@solid-primitives/platform`'s `is*` constants, are values a consumer uses with
nothing open (by `project_export_semantics`; not reproduced against a consumer
project). The in-surface share it reports rises whenever any one domain
closes: 76.6% for this tier, and 89.0% for the later recipe and ADR 0112 run,
whose catalogs have not been regenerated into the tier. A site that run moved by
closing `reads` alone stays in the 555 unless `creates` closed with it.

**In the one A/B measured, no violation came from a contract.** That was
`kobalte`'s `packages/core`, analysed with and without the tier
([the first Solid 2 consumer measurement](2026-09-17-the-first-solid2-consumer-measurement.md)).
Every violation rule kept the same count. SC9005 rose from 45 to 139:
acceptance-gate findings fell from 16 to 13, and 97 findings naming open claim
domains appeared.

The claims that could produce a misuse finding, and how many exports in the
tier carry each:

| positive claim in the tier | what it feeds | exports |
| --- | --- | ---: |
| an owner requirement | missing-owner | 13 (34 demanded sites) |
| a reactive read of an argument | strict-read-untracked, the async-boundary rules | 34 |
| a returned accessor | the call's result becomes a reactive source | 19 |
| a callback invocation | whose scope the callback's reads belong to | 26 |

Only three callback rows prove a tracking clear (`createPureReaction`,
`createBranch`, `createDisposable`). None of them is in the measured demand,
and nothing reads the clear: `callback_wrapper_at` (`interproc.rs`) keeps a
package's `inline` row transparent, deliberately (47566ff8).

## What would change the answer

In the order they would move the goals. Items 4 to 6 are the owner's decisions.

1. **Measure what a consumer gets.** Give the census the three buckets above,
   next to its own, and re-sweep demand over Solid 2 consumers (the coverage
   baseline's open decision 1). A number that rises when any one domain closes
   cannot tell whether goal 1 is getting closer.
2. **Close `creates` and `returns`, not more `reads`.** Leaving the 555 needs
   both `reads` and `creates`, and `creates` is the one open more often.
   `returns` holds 436 of the other 439. The last campaign's 84 sites were
   `reads` closures.
3. **Treat the component wall as composition.** A component library closes only
   as far as its dependency graph does, so the dependency layer comes first,
   through the graph lane that already exists for it.
4. **Decide whether a package claim should clear the same bar as project code.**
   A helper in the project's own source is summarized by the interprocedural
   graph, and its answer is used directly. What the graph cannot resolve still
   fails closed, as its own finding (`reactive-dispatch-unresolved`). The same
   helper published in a package needs
   that summary as a contract proposal, then an implementation census over the
   runtime bytes, then a probe gate per closed domain. Part of the difference is
   earned: published artifacts are untyped JavaScript whose declarations need
   not match the bytes, and one artifact serves every consumer. But the
   difference is also why most steps since ADR 0034 have been per-form premises.
   If most Solid 2 packages is the goal, this is the decision to reopen. It is a
   change to ADR 0001, not another premise.
5. **Decide whether package-specific rules are in scope.** The model states
   usage conditions only as reactive semantics: owner requirements with the
   capabilities they need, a callback's tracking and schedule, a returned shape.
   It has no claim domain for "must render under this provider", "call once", or
   an argument protocol. A package author cannot supply one either, because a
   claim is accepted only when the verifier establishes it from the artifact.
   Adding one changes the semantic model ADR 0004 froze, and needs its own proof
   demand.
6. **Decide what SC9005 should cost a user.** It is an error, it is what most
   imports of a Solid package produce today, and on Kobalte the tier made it
   more frequent rather than less.

## Reproduce

    bun docs/package-contract-v2/phase22/2026-09-23-what-a-consumer-can-use.mjs

The script is read-only: it runs no checker and certifies nothing. Its header
states its two approximations, and both overstate what a consumer gets.
