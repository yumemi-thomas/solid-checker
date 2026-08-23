# RFC 0002 amendment A9: `kind` has no unknown form

- **Status:** Accepted, staged. Stages 0 and 1 are implemented **and measured**
  (2026-08-24, 416 rows, checker `ddb0ecd8…`). **Stage 2's gate is now closed
  against it: measured payoff 0 rows, so it must not be built** — see
  "[Re-measurement plan](#re-measurement-plan)" for the measured outcome beside
  each prediction, and
  [docs/ecosystem-benchmark.md](../ecosystem-benchmark.md) for the run.
- **Date:** 2026-08-23
- **Amends:** [RFC 0002](0002-machine-verified-contracts.md) §"Claim taxonomy",
  amendment A1, and unresolved questions 2 and 5
- **Affects:** `packages/cli/scripts/contract-verification.mjs`,
  `packages/cli/scripts/verify-contract.mjs`,
  `scripts/ecosystem-benchmark/verify-corpus.mjs`,
  [docs/package-contracts.md](../package-contracts.md); under option A only,
  `schema/solid-reactivity.schema.json` and the Rust contract loader
- **Does not affect:** what a certifying contract means to a consumer, the
  conversion rule for every other claim domain, or `contract review`
- **Measured against:** `benchmarks/ecosystem/verification-report.json`,
  aggregated 2026-08-23T13:04:11Z, 416 probe rows, checker `068b04bb…`
  (contains the exported-class fix f2e40d32)

It is long enough to carry its own file rather than sit in RFC 0002's
"Amendments" list, because the decision it records is mostly the *rejection* of
three tempting options and the measurement that rejects them. A9's summary in
that list points here.

## Summary

Amendment A1 made an unobserved `kind` claim a promotion blocker, because
`kind` is the one claim schema v1 cannot convert to `{"status":"unknown"}`.
That rule is the largest single reason a contract does not machine-verify:
**77 of 146 refusals**, more than incompleteness (40) and probe failure (27)
combined.

This amendment asks whether the rule should be relaxed, and concludes: **not by
giving `kind` a sentinel, and not by making the blocker finer.** The measurement
says the refusals are not a *sayability* problem — they are 77 packages the
probe observed almost nothing about, and the three options the queue named all
convert them into contracts that are `verified` and empty. The honest
conversions available are two, and neither needs schema surface:

1. **Narrow on an observation of absence, never on a gap.** A mode in which the
   probe *loaded the namespace and found no such binding* (`export-missing`) is
   a mode in which the export does not exist; a `kind` claim is not stated for
   it. A mode in which the import threw, the session died, or the run was
   narrowed is a gap, and keeps blocking. This is the one place RFC 0002
   unresolved question 2's prohibition on "silently narrowing the stated modes"
   does not bite, because nothing is silent and nothing is unobserved.
2. **Refuse the unobservable entrypoint the way generation already refuses
   one**, rather than refusing the document.

Sized honestly, (2) is worth at most 10 of the 77 — and less, for reasons
stage 1 spells out — while (1) is worth at most 43, and the report this was
measured against **cannot tell us how much of the 43 is real**. So the
recommendation is staged, and stage 0 is measurement. Stage 0 has since shown
that the unclassified `other` bucket (834 claims) was not where the answer was
hiding either: it is one session-death class, and the question is a
(claim, mode) one that only the per-mode `kindGaps` figures can answer.

Separately: the **53 `kind: claimed value, observed function` failing claims
are not a sentinel question at all.** They are two generator defects, and one of
them is a live false-negative source in contracts that never go near
verification. Section "The 53" says why they must stay failures.

## Motivation

### What A1 established, and what it now costs

A1's rule: a `kind` claim not probed-passed in every mode its export is stated
for **blocks** promotion. Schema v1 requires `kind` on every export summary
(`schema/solid-reactivity.schema.json:100-102`), its two values are the whole
vocabulary, and `$defs.unknownClaim` is not among its permitted types — so
"not proven" is unsayable and there is no weaker document to promote. The
consequence A1 accepted deliberately: *a package this checker cannot import
cannot be machine-verified at all.*

The corpus prices that acceptance.

| Refusal root cause | Rows |
| --- | --- |
| `kind-observed` | **77** |
| `incompleteness` | 40 |
| `probe-failed` | 27 |
| `closure-note` | 2 |
| **total refused** | **146** of 416 |

`kind-observed` is also broader than its root-cause count: it is *a* blocker
class on **106** rows (357 blocker lines), the **sole** blocker class on 75, and
a co-blocker on 29 rows whose root cause is `probe-failed` (21) or
`incompleteness` (8). Those 29 do not convert under any option here — they have
an independent blocker — so the addressable population is 77.

### The 77, classified by which modes were unobserved

Parsed from each refusal's `firstBlocker`, whose parenthetical lists exactly
the modes with no passing `kind` observation for each export:

| Mode pattern | Rows | What it means |
| --- | --- | --- |
| every stated mode (`client, server, development, production`) | 34 | nothing at all was observed about these exports |
| `server` only | 27 | client/dev/prod observed and passed; server did not |
| `development` only | 6 | dev-condition artifact unobserved |
| `client, development, production` (server not stated) | 6 | every stated mode unobserved, browser-conditioned entrypoint |
| mixed (incl. 2 `no unambiguous summary resolves there`) | 4 | — |

Two figures bound what granularity can buy:

- **Entrypoint refusal saves 10 of 77.** Joining each refusal's kind-blocker
  count against `contractContent.entrypointsEmitted` in `report.json`: only
  **10** rows (5 all-mode, 5 server-only) have an emitted entrypoint left over
  after the blocked ones are removed. The other 67 are single-entrypoint
  packages whose only entrypoint is the blocked one.
- **Per-export omission saves little more.** Of the rows with exactly one
  kind-blocker line (so the count is exact), **46** have *every* export of that
  entrypoint unobserved and only **15** are partial.

So for the large majority of the 77, "grant `kind` a weaker form" and "omit the
unobservable part" both terminate in the same place: a document that verifies
because it says nothing.

### What the verified tier currently rests on

This is the number that decides the risk appetite. Of 261 verified contracts,
**3 (1.15%)** carry even one probed *behavioral* row; 739 claim domains were
converted to unknown; 2,955 inferred row markers were dropped. A verified
contract today is, almost entirely, the generator's statically-proven negatives
plus **the `kind` observation**. `kind` is not one claim among many in that
tier — it is nearly the only thing a probe actually witnessed. Weakening it
weakens the only observational content the tier has.

### The inert-runtime rule does not explain these refusals

`kind` was deliberately left ungated by the runtime-capability rule (13074665):
a `kind` probe reads `typeof` off the module namespace
(`packages/cli/scripts/contract-probe-worker.mjs:777-780`), which needs no
reactive re-run, so an inert mode is a perfectly good mode to observe `kind` in.
The corpus confirms the exemption holds mechanically: the undriven reason
`runtime re-runs nothing in this mode` accounts for **49** claims, **none** of
them `kind` — the driver only stamps that reason on `callbacks[n]` observations
(`packages/cli/scripts/contract-probe-driver.mjs:732`). Inertness contributes
**zero** of the 77. These are import, session, and presence failures.

### The measurement gap that blocked the decision

The choice between blocking and narrowing turns entirely on *why* a mode
produced no `kind` observation, and the report this was measured against could
not answer it. A `kind` probe has exactly four non-observing outcomes
(`packages/cli/scripts/contract-probe-driver.mjs` `OUTCOME_REASON`):

| Outcome | Is it an observation? |
| --- | --- |
| `export-missing` — namespace imported, binding absent | **yes** |
| `import-failed` | no |
| `session-failed` (crash, timeout, unreadable report) | no |
| mode never attempted (`--modes`) | no |

`verify-corpus.mjs` buckets undriven reasons by prefix and had a rule for
`import of …` (637 claims, 34 rows) but **none for `export-missing` or
`session-failed`** — both fell into `other`, which was **834 claims**, the
second-largest bucket in the corpus. Since only 34 rows corpus-wide had any
import throw, the 27 server-only and 6 browser-triple refusals are *not*
import throws, and the split between "observed absent" and "session died" was
precisely the unknown. Stage 0 closes that gap; it is why stage 0 exists.

Stage 0 also found the gap was one level deeper than this paragraph assumed. The
undriven distribution is over *claims*, and a `kind` claim observed in one mode
and absent in another settles as `passed`, so it never appears there at all —
the 834 turned out to be a single session-death class. The question is a
(claim, mode) question, which is why `kindGaps` reads the per-mode observations
and not the reason distribution.

`docs/package-contracts.md` already asserts the shape from a sample — *"no
export differs in `typeof` between the client and the server artifact — the
differences are presence, which becomes `export-missing` and so undriven"* — but
a sampled assertion is not the corpus figure, and this design is not built on
it.

### The 53 failing `kind` claims are a generator defect, not a sentinel gap

Of 63 failing claims, 53 are `kind: claimed value, observed function`. Reading
all 53 rows out of `probeFailures.rows`, they are two distinct defects:

**(i) 45 class-shaped exports** the exported-class fix still misses —
`ListCollection`, `ListKeyboardDelegate`, `Selection`, `SelectionManager`
(`@kobalte/core`); `ReactiveMap`/`ReactiveWeakMap`, `ReactiveSet`/
`ReactiveWeakSet`, `TriggerCache` (Solid Primitives, both probe pins);
`ResponseEnvelope` (`@solidjs/web`, all three entrypoints, both pins); and 25
across `@tanstack/*` — `AsyncBatcher`, `Debouncer`, `Queuer`, `RateLimiter`,
`Throttler` and the `*DevtoolsCore`/`*DevtoolsPanel` family. The measured binary
**contains** f2e40d32, so `binding_declares_class`
(`rust/crates/solid-reactive-ir/src/contracts.rs`) is not reaching these. Every
one of them is re-exported across a module or package boundary
(`@tanstack/solid-pacer/./batcher` → `@tanstack/pacer`), which is where the
alias/initializer walk has to land, and where it evidently does not.

**(ii) 8 plain functions** — `@solid-devtools/locator`'s `addClickInterceptor`,
`addHighlightingSource`, `addLocatorModeSource`, `highlightedComponent`,
`highlightingEnabled`, `locatorModeEnabled`, `setTarget`, `useLocator` — stated
`kind: value` with no class anywhere. A distinct gap: callability unresolved
through the barrel, so `promote_callable_export`
(`rust/crates/solid-reactive-ir/src/contracts.rs`) found no entity and left
`value`.

**Why these must stay failures, and why they are urgent independently of
verification.** `validate_export` (`rust/crates/solid-reactive-ir/src/lib.rs`)
structurally bars a `kind: "value"` summary from carrying *any* claim domain —
not even an unknown one. A `value` summary is therefore the **maximal negative
claim**: reads nothing reactive, returns nothing reactive, invokes no
caller-supplied callback, requires no owner. `addHighlightingSource(fn)` and
`addClickInterceptor(fn)` take callbacks. So the generator is publishing, in an
ordinary `inferred` contract that any reviewer could promote, a certified
negative for functions that are not inert. That is a false-negative source in
the *reviewed* tier, not a verification inconvenience.

Giving `kind` a sentinel would let all 53 convert, which is exactly the outcome
RFC 0002 §3 and docs/package-contracts.md forbid: *"a failed probe … is a
generator bug or a package change, and neither is fixed by converting the claim
to unknown."* **No option in this amendment may absorb a contradicted `kind`.**
Expected honest sentinels from the 63 failing claims: **zero**. They stay
failures until the generator is fixed, and fixing it is a separate queue item
with its own fixtures.

## Who consumes `kind`, and the false-negative surface

Read before judging any option, because it bounds what an unknown `kind` could
mean.

`kind` has **no dispatch site in the analyzer**. Searching the workspace, every
`function`/`value` comparison is in generation or validation:

- `rust/crates/solid-reactive-ir/src/lib.rs` — `validate_export` rejects any
  kind that is not `"function" | "value"`, for the document; and a `value`
  summary may carry no claim domain, known or unknown.
- `rust/crates/solid-facts-backend/src/main.rs` —
  `mark_summary_claims_unknown` returns early unless `kind == "function"`;
  `promote_entry_callable` and same-identity export merging bias to `function`.
- `rust/crates/solid-reactive-ir/src/contracts.rs` — `value_contract_export`,
  `class_contract_export`, `promote_callable_export`.

At the call site the analyzer dispatches on the **claim domains**, not on
`kind`: `rust/crates/solid-facts-backend/src/diagnostics.rs` looks the export up
and skips it when every domain is a known default. An absent export name (or an
absent entrypoint, via `exports_for_module`) yields no summary, so the symbol
stays an uncontracted external — the fail-closed pre-contract state.

Two consequences fall straight out:

1. **An unknown `kind` cannot mean anything on its own.** Because `value` is the
   structural spelling of "carries no domains", an unknown `kind` would have to
   be treated exactly like `function` (permissive on domains) to be sound. And
   it would have to **cascade**: an export whose `kind` was never observed but
   whose `callbacks` field is merely *omitted* still certifies "invokes no
   caller-supplied callback" — the strongest negative in the vocabulary — for an
   export the probe never imported. So an unknown `kind` is only honest if every
   domain of that summary becomes unknown too. At which point the summary
   carries no information, and is *informationally identical to omitting the
   export*. That equivalence is the load-bearing result of this amendment.
2. **`mark_summary_claims_unknown` would silently skip it.** It returns early
   for any kind that is not `"function"`, so a new third value would bypass the
   generator's own fail-closed marking. Any option A implementation must fix
   that line first, or it ships the cascade bug it needs to avoid.

## Option A — a schema-v1 unknown sentinel for `kind`

**What changes.** `schema/solid-reactivity.schema.json` gains either a third
enum value (`"unknown"`) or a `oneOf` with `$defs.unknownClaim`. The Rust loader
learns it, the generator's marking learns it, the verifier converts instead of
blocking, and — per the cascade above — conversion of `kind` must force every
domain of that summary to unknown.

**Payoff.** Up to 77 refusals become verified (261 → 338, 62.7% → 81.3%).
Failing claims that become honest sentinels: **0** (forbidden, above). But by
the cascade, 46 of the 77 produce contracts every one of whose exports carries
nothing but unknowns. The number moves; the certified content does not.

**Soundness.** The cascade makes it sound *if implemented completely*. Every
incomplete implementation is unsound in the worst direction — a certified
negative for an unimported export. The failure is invisible: a clean report.

**Compatibility — this is the disqualifier.** `kind` is `required` with
`additionalProperties: false`, so it can be neither omitted nor re-typed
compatibly. Any new spelling makes `validate_export` fail, and that error is
raised inside `decode` → `expand` → `validate`
(`rust/crates/solid-facts-backend/src/contract_document.rs`) as a
`BackendError::Contract`. That is the **malformed** path RFC 0002 unresolved
question 5 already identified: it *fails the analysis outright* rather than
refusing the one contract and continuing. A `kind` sentinel therefore does not
merely fail closed on older checkers — it takes the whole run down, for every
project that has the newer contract on disk. This is the identical wall
RFC 0001 §4 hit with `evidence.verifier` and UQ5 hit with a sentinel `reason`.

**Verdict: rejected, and folded into the schema-v2 decision UQ5 already frames.**
Not because a sentinel is wrong in principle, but because schema v1 cannot carry
it, the informational payoff is nil once the cascade is honest, and paying a
hard-fail compatibility cost for a metric with no content behind it is the
wrong trade.

## Option B — per-entrypoint (and per-export) granularity in the verifier

**What changes.** No schema change. The kind check already computed its findings
**per entrypoint** and pushed one blocker line each; `collectBlockers` then
refused the document. Instead, an entrypoint whose `kind` claims were not
observed is **refused and omitted from the promoted document**, exactly as
`contract generate` already refuses an entrypoint it cannot certify. The verify
sidecar records the refusal.

**Why this is the well-trodden path.** docs/package-contracts.md, "Refused
entrypoints versus failed generation": *"An entrypoint the generator cannot
certify is refused and omitted, the other entrypoints are still emitted … A
refused entrypoint is absent from the contract, so a consumer importing it gets
an explicit uncertifiable result rather than a wrong claim."* And the same
document already lists a refused entrypoint under **what does not block**.
Verification refusing an entrypoint for the same reason generation does is a
consistency fix, not new semantics.

**Payoff, measured.** **10 of 77.** 67 of the refusals are single-entrypoint
packages; refusing the only entrypoint yields a document with no entrypoints,
which is not a verified contract in any useful sense and stays a refusal.
Pushing to per-*export* granularity adds at most the 15 partial rows, and
carries an unresolved consumer question: `no-export-summary` is an
**entrypoint**-granular review item, and whether an export absent from a
*present* entrypoint raises `SC9005` — rather than silently resolving to no
summary at the consumer — is not established. Entrypoint granularity has a
documented consumer meaning; export granularity does not yet.

**Soundness.** Highest of the three. Nothing new is certified; strictly less is.
The one risk is the empty document: verification must refuse a document that
would certify nothing rather than promote it. Confirmed rather than assumed: the
loader rejects an empty `entrypoints` map
(`rust/crates/solid-reactive-ir/src/lib.rs`, `self.entrypoints.is_empty()`) and
an entrypoint with an empty `exports`, so `--validate-contract` is a backstop —
and the verifier raises its own blocker first, so the refusal says what happened
instead of complaining about document shape. "Would certify nothing" has to be
the actual test rather than "every entrypoint was refused": an entrypoint with an
empty export map raises no refusal to count, so counting refusals against the
entrypoint total let one stand in as a survivor.

**Verdict: accepted, but it is a small correctness win, not the answer to 77.**

## Option C — mode narrowing

**What changes.** The mode set a `kind` claim is required in — `statedModes`
over `modeApplies` — is narrowed to the modes where the analyzed artifact is
actually the thing that mode resolves.

**Where it collides with the RFC.** Unresolved question 2 is explicit:
*"'undrivable in mode X' must convert the claim, not silently narrow the stated
modes, or the contract would claim semantics for an environment nobody
observed."* Taken literally, option C is prohibited — and for gaps it should be.

**And it collides with the schema.** Narrowing a *claim* to a subset of modes
means expressing it as a `variants[]` entry with conditions. But a variant's
summary requires its own `kind`, and a consumer without an explicit
runtime-condition selector never resolves through a variant set — it reads the
base summary, which also requires `kind`. So option C cannot express "kind
known under `browser`, unknown under `node`" without option A's sentinel at the
base. **Option C as stated is not implementable in schema v1.**

**The one narrowing that is sound, and needs no schema at all.** Distinguish a
*gap* from an *observation of absence*. `export-missing` is recorded only after
`importNamespace` succeeded and `!(probe.export in resolved.namespace)`
(`packages/cli/scripts/contract-probe-worker.mjs`) — the namespace loaded, and
the binding is not in it. That is a positive observation that **the export does
not exist in that mode**. An export that does not exist cannot be called, so
there is no consumer claim about that mode to certify or withhold, and requiring
a `kind` observation for it is requiring `typeof` of a binding that is not
there.

So the rule this amendment can defend:

> A `kind` claim is not stated for a mode in which the probe **observed that the
> export is absent** from the artifact that mode resolves. Every other
> non-observation — `import-failed`, `session-failed`, a mode the run never
> attempted — remains a gap and keeps blocking.

This is not UQ2's silent narrowing: nothing is silent (the absence is a recorded
observation with its own outcome), and nothing is unobserved. It also composes
with docs/package-contracts.md's finding that the client/server difference in
practice *is* presence.

**Payoff.** Bounded above by the 43 rows whose kind gap is not all-mode
(27 server-only + 6 dev-only + 6 browser-triple + 4 mixed) — and **not
measurable from the report this was measured against**. The reason turned out not
to be the one stated here: `export-missing` and `session-failed` were not sharing
the `other` bucket, because a claim observed in one mode and absent in another
settles as `passed` and never enters the undriven distribution at all (stage 0
resolves that bucket to one session-death class, 834 of 834). The unmeasurable
part is the *(claim, mode)* level, which is what `kindGaps` now reads and what
the re-measure will report. It could be most of 43 or almost none of it.

**Soundness.** Sound for `export-missing`; unsound for anything else, and the
implementation's whole burden is keeping those apart. One residual risk to state:
`export-missing` in mode M plus `kind: function` observed in mode N certifies
`function` for a summary a consumer reads without a mode selector, i.e. also for
M. That is safe *because* the export does not exist in M — but it is only safe
while nothing else in the summary claims mode-specific behavior, which is
already the variants question and already fail-closed.

**Verdict: the observed-absence form is accepted in principle and gated on stage
0; the general form is rejected.**

## Option D — the decision: measure, then B, then narrow-on-absence

Staged, because stage 2's payoff is unknown until stage 0 runs, and stage 0 is
nearly free.

### Stage 0 — classify the non-observations (measurement only) — implemented

Break `other` apart so the design has a number.
`scripts/ecosystem-benchmark/verify-corpus.mjs` now has reason rules for
`export-missing`, for every session-death shape, and for the remaining
`UNDRIVABLE` / `OUTCOME_REASON` / `settleClaims` strings that reached `other`,
and `verify-corpus.test.mjs` asserts that totality **against the driver's own
tables** rather than a copied list, so the next new reason string fails the test
instead of quietly widening `other`. Two rules are deliberately shaped as
families (`the probe process …`, `spawnSync …`) so a reworded session failure
lands in a *named* bucket.

Beside that, every row carries a `kindGaps` breakdown — `{claims, modes,
reasons, contradictions}` over the (`kind` claim, unobserved mode) pairs, read
from the probe report's own per-mode observations — surfaced per refusal,
aggregated corpus-wide, and rendered as "Why a `kind` observation is missing".

**A contradiction is a sibling object, not a sibling label.** A mode whose
observation exists and *disagreed* is counted only in `contradictions` and
renders as its own markdown section: the two must never share a number, because
absorbing a contradicted `kind` is what this amendment forbids, and sharing
`claims`/`modes` while separating only `reasons` is that failure with a label on
it — under headings that read "unobserved", with 53 contradicted `kind` claims
across 20 corpus rows to be counted there. One claim can be gapped in one mode
and contradicted in another, and both numbers stay true.

Two of the four non-observing outcomes in the table above are not per-mode
observations at all, and each is a labelled category rather than a silence: a
mode **the run never attempted** (derived from the probe report's own `modes`
list, so it is empty for a full corpus run and exactly the narrowed-away modes
otherwise), and a mode where **no unambiguous summary resolves** (which produces
no `kind` claim at all — `buildProbePlan` records a family-(C) `summary` claim
naming the mode, and that is what the measurement reads).

Bucketing is label-only. Nothing in the pipeline reaches a verdict by reading a
reason string: every consumer forwards one as text (a conversion record, a
stdout line) or groups one for a distribution.

One prediction this amendment made about the 834 was **wrong, and the corrected
rules can say so without re-running anything**: the same 416-row journal the
report above was aggregated from, re-bucketed with the new rules, resolves
`other` **entirely** to `probe session aborted by package code` — 834 of 834, in
16 distinct shapes, each carrying the package's own `uncaughtException` /
`unhandledRejection` text and stack. `UNDRIVABLE.owner`, `export is not
callable` and the two `returns`-distinguisher reasons account for **zero**
claims in it. So the 834 is one class, and it is a session-death class: a gap,
which keeps blocking.

**And `export-missing` is absent from that distribution by construction, not
measured at zero.** `settleClaims` settles a claim observed in one mode and
absent in another as `passed`, so it contributes no undriven reason at all — the
undriven distribution is over *claims*, and stage 2's question is over
(claim, mode) pairs. That is precisely why `kindGaps` reads the per-mode
observations instead, and why stage 2's number still has to come from the
re-measure: the journal keeps no per-claim observations to recompute it from.
What the journal does establish is that the *undriven* bucket is not where the
addressable share was hiding. `export is not callable` and the two `returns`-distinguisher reasons
were unruled as well.

**This stage is a precondition, not a nicety.** Without it the 43 addressable
rows split unknowably between "sound to narrow" and "must keep blocking", and
implementing stage 2 blind would either leave the payoff on the floor or narrow
on gaps.

### Stage 1 — verification-time entrypoint refusal (option B) — implemented

`unobservedKindRefusals` (`packages/cli/scripts/contract-verification.mjs`)
returns per-entrypoint *refusals*; `withoutRefusedEntrypoints` drops them before
conversion, so nothing a refused entrypoint stated reaches the promoted bytes or
the probed-row count. The sidecar gains `refusedEntrypoints` (each with its
blocker and the exports whose `kind` was unobserved) and
`summary.refusedEntrypoints`; both are additive, and the sidecar's other counts
keep their meaning — they describe the promoted document, which is now the
smaller one.

**The document is refused when nothing would certify anything, which is not the
same as "every entrypoint was refused".** An entrypoint with an *empty export
map* certifies nothing either, and it raises no refusal to be counted — so
`certifyingEntrypoints` is the survivor test, and a contract with zero
entrypoints or with nothing but empty ones raises its own blocker
(`certifies-nothing`) here rather than reaching `--validate-contract` and being
rejected for its shape. That path is unreachable from a generated draft today;
it is the fail-closed floor, not a population.

**A refusal keeps its per-entrypoint evidence, and the classifying phrase leads
the line.** The document-level line says why the *document* went, and one line
per refused entrypoint says which entrypoints were unobservable and in which
modes — the shape A1's implementation had, restored, because
`blockers.raised` in the refusal sidecar is the only durable record of a refusal
and the corpus has a row with 91 of them. The corpus classifier truncates each
line to a 260-character head *before* classifying it, so the phrase it keys on
leads and the enumeration comes last: a phrase pushed past that cap by a long
entrypoint name would silently reclassify the row and corrupt the one count
stage 2's gate reads. The margin was not theoretical — the corpus's longest
entrypoint name inside a `kind` blocker is `./primitives/create-disclosure-state`
at 36 characters, which with the enumeration leading put the phrase at index 234
of 260.

**The review plan names the refusal.** The plan is re-derived from the promoted
bytes, so a refused entrypoint would otherwise leave the document *and* the plan
— and `contract review` never reads `<contract>.verify.json`, so nothing would
say the subpath had ever been claimed. `rewriteReviewPlan` therefore pushes the
`refused-entrypoint` item generation already uses for exactly this situation,
naming the exports the promotion dropped, kept open fail-closed by review
transfer. Without it, the sentence generation's own code gives as the reason for
that item — *"a partial contract must never be silent about what it omits"* —
would have been false of verification's refusals, and this document's claim that
an unobservable package "can still be reviewed" false with it.

**The corpus's composite keeps those exports.** A verification-refused
entrypoint's exports are their own state in "The composite a consumer feels",
still inside its denominator, because both verified states are counted off the
*promoted* document: dropping them would have raised the certified share for a
reason with no certification behind it, which is the one movement this
amendment's re-measurement forbids.

Payoff: **at most** 10 rows, and less than that in every direction worth
naming — which is why the durable value is not the count.

- **The 10 is an upper bound, and the carve-outs below subtract from it
  *correlated* with the population.** A closure note, a failed probe or an
  incompleteness finding anywhere in the package still refuses the document, and
  the rows stage 1 addresses are the multi-entrypoint ones: the corpus's
  kind-rooted refusals include five rows whose refusal spans more than five
  entrypoints and one spanning **91**. A package with dozens of entrypoints is
  exactly the kind most likely to also carry a closure note or one contradicted
  claim somewhere, so the overlap is not independent of the population.
- **The surviving half is the less behavioral half.** By construction the
  entrypoints that survive are the ones the probe *could* import, which for
  these rows is the browser- or types-shaped subpath rather than the
  behavioral one. So the newly verified documents will be disproportionately
  `kind`-only negatives, on top of a tier where 3 of 261 verified contracts
  carry any probed behavioral row at all.
- **The certified share must not be read as the payoff.** The corpus's composite
  keeps a verification-refused entrypoint's exports in its denominator, as their
  own state, for exactly this reason: a certified *share* that rose because
  unobservable exports left the population would measure nothing.

What stage 1 durably buys, then, is **consistency** — generation and
verification refusing on the same unit, so one unimportable subpath stops sinking
twenty observed ones, and a reader of two artifacts is no longer told two
different things about the same fact — and, with stage 0, a **decidable gate**
for stage 2. That is the honest case for it; the row count is not.

**What deliberately stays document-wide.** A failed probe and an incompleteness
finding refuse the whole document even when both name a claim of an entrypoint
this run would refuse anyway: each means the package answered a claim
differently, which is a generator bug or a package change, and scoping them to
the entrypoint would let a contradiction be *dropped* rather than fixed. A
closure note stays document-wide for the reason it always did — fail-closed on a
file set the generator declines to claim it enumerated — so a note on a
`kind`-refused entrypoint still refuses the document, and stage 1's payoff is
bounded by the rows without that overlap. All three are the conservative
direction, and all three keep the corpus's `probe-failed`, `incompleteness` and
`closure-note` root-cause counts comparable across the re-measure.

### Stage 2 — narrow on observed absence (option C, restricted) — gate closed, do not build

**Not implemented, and — as of the 2026-08-24 re-measurement — not to be
implemented at all.** The design was: exclude, from the modes a `kind` claim must
be observed in, exactly those modes whose probe outcome for that
`(entrypoint, export)` was `export-missing`, recording the exclusion in the verify
sidecar per claim so a reader sees *"not required in `server`: the export is not
in that namespace"* rather than a silently smaller mode set. Payoff: whatever
stage 0 measures, bounded by 43.

The gate was not ceremony, and it fired. Stage 0's numbers say the gaps are
overwhelmingly import failures and session deaths: **45 of 6,962 gap
(claim, mode) pairs are absences, 0 of the 64 rows this stage exists to serve
carries even one**, and the three rows that do carry an absence have an
independent `probe-failed` blocker. Measured payoff **0 rows**. See
"[Stage 2's gate, closed](#stage-2s-gate-closed)".

Its one implementation prerequisite: the driver needs to surface the *outcome*
(not only the English reason) on the claim record, so the verifier can branch on
it without matching prose. Stage 0's buckets are a measurement, and a verdict
must not be taken by reading a sentence.

### Not in this amendment

The generator defects behind the 53 failing `kind` claims — cross-boundary class
resolution, and callability through a barrel — are their own queue item. They
are the largest *content* defect the corpus shows, and one of them
(`@solid-devtools/locator`) publishes a maximal certified negative for
callback-taking functions today.

## Compatibility statement

- **`schemaVersion` stays 1.** The accepted stages touch no schema file. No new
  field, no widened enum, no re-typed field.
- **No consumer change.** A refused entrypoint is already absent from a contract
  and already an explicit uncertifiable result at the consumer. No new document
  shape reaches the analyzer, so no older checker can encounter anything it does
  not already handle.
- **Verified documents get strictly smaller, never more certifying.** Stage 1
  removes entrypoints; stage 2 would remove a *requirement* on the verifier, not
  a claim from a document. Nothing newly certifies.
- **The verification report's top-level totals keep their meaning.** Stage 0 and
  stage 1 add report fields (`kindGaps`, `verificationRefusedEntrypoints`,
  per-row `refusedEntrypoints`); `verified`, `refused`, `conversions` and
  `probedRowsKept` are unchanged in definition, so the dated tables in
  docs/ecosystem-benchmark.md stay comparable across the re-measure.
- **Two report shapes do change, deliberately.** "The composite a consumer
  feels" gains a state — the exports a promotion dropped with a refused
  entrypoint — and the old (c) becomes (d); the denominator is the drafts'
  export count, so (a) is comparable across the change only in the direction
  that matters, which is that it cannot rise by exports leaving. And the
  refusal sidecar's `blockers.checked` gains `certifies-nothing`, since that is a
  blocker the taxonomy did not have.
- **Option A, for the record, is not schema-v1-compatible in either spelling.**
  `kind` is required with `additionalProperties: false`, and any unrecognized
  value fails `validate_export` inside `decode`, which is the malformed path that
  fails the whole analysis rather than refusing one contract. It belongs with
  `evidence.verifier` and UQ5's sentinel `reason` in a single schema-v2 decision,
  taken once.

## Fixtures

Per AGENTS.md, each semantic branch needs a positive and a negative plus the
distinction cases. These are verifier-level and harness-level, so they live with
`scripts/contract-verify.test.mjs` and
`scripts/ecosystem-benchmark/verify-corpus.test.mjs` rather than in
`fixtures/package-contracts/`.

**Stage 1** (`scripts/contract-verify.test.mjs`)

- positive: a two-entrypoint contract, one entrypoint's `kind` unobserved →
  no document blocker, the refusal names that entrypoint, and the other
  entrypoint's probed rows survive with no conversions.
- negative: the same contract with *both* entrypoints unobserved → the
  document-level line naming the empty surviving set, plus one line per refusal.
- negative (the common shape): a single-entrypoint contract with the same gap →
  still refuses whole. 67 of the 77 are this shape, so per-entrypoint
  granularity must not turn it into a document that verifies because it says
  nothing. The three pre-existing `kind` blocker cases are all this shape, and
  their titles now say which branch they cover.
- distinction: a refused entrypoint is **absent** from the promoted document,
  not present-and-empty — the consumer sees no summary for it at all, which is
  the fail-closed pre-contract state rather than a document asserting it has
  nothing to say.
- the sidecar names every refusal, and its `summary.exports` counts the promoted
  document, so the two numbers together say the document is smaller than the
  draft.
- evidence: an eight-entrypoint all-refused contract raises one named line per
  refusal (scaled down from the corpus's 91-line row), and every line still
  classifies as `kind-observed` after the harness's 260-character truncation —
  including with a 46-character entrypoint name, where the margin used to run
  out.
- the empty-set floor: an entrypoint with an empty export map is not a survivor,
  and a contract with no entrypoint at all is refused here rather than by
  `--validate-contract`.
- the plan: a verification refusal leaves a `refused-entrypoint` item naming the
  entrypoint and the exports it dropped, and the surviving entrypoint's items
  keep their ids.
- end to end: `verifyContract` over a two-entrypoint draft with one unobservable
  subpath — the promoted bytes, the sidecar on disk, and the rewritten plan and
  checklist.

**Stage 1's own laundering guard** (`verify-corpus.test.mjs`): the document-level
blocker line classifies as `kind-observed`, and the `certifies-nothing` line as
itself, so neither can fall through to the unclassified bucket the measurement
would then under-report.

**Stage 0** (`scripts/ecosystem-benchmark/verify-corpus.test.mjs`)

- totality over `UNDRIVABLE`, `OUTCOME_REASON` and `EXECUTION_UNATTRIBUTABLE`,
  the session-death shapes, and the `settleClaims` fallbacks: none may bucket to
  `other`.
- an observation of absence buckets apart from every gap, and a reworded session
  failure still lands in a name.
- **a session death whose text quotes `'x' is not exported by y`** — the
  canonical bundler message, which a session failure forwards verbatim from the
  child's stderr — is bucketed as a session death and not as an observation of
  absence, in all four session shapes, with an import throw carrying the same
  text as the control.
- `kindGapsFor` counts a contradiction in `contradictions` and in **neither**
  `claims` nor `modes`, and a claim gapped in one mode and contradicted in
  another lands once in each.
- an attempted mode with no observation is its own gap; a mode the *run* never
  attempted is its own labelled gap; a mode where no unambiguous summary resolves
  is read from the plan's `summary` claim rather than being absent.
- the composite keeps a verification-refused entrypoint's exports in its
  denominator, as their own state (5/21, not 5/7).

**Stage 2**, when its gate opens: `export-missing` in `server` plus observations
elsewhere promotes with the exclusion recorded; `import-failed`, `session-failed`
and a never-attempted mode each still block on the same bytes the positive
passes; `export-missing` in *every* stated mode blocks, because an export absent
everywhere is a generator defect and `contract check`'s missing-export path owns
it.

**Snapshots and pins.** None in `fixtures/`, `schema/`, or
`pkg/contracts/bundled/`: the accepted stages change no generated artifact and
no analyzer behavior, so no findings snapshot and no ownership-gate case moves.

## Re-measurement plan

The plan, as written before the run: re-run `verify-corpus.mjs` over the same 416
rows with the same pinned binaries, and report the honest directions alongside the
flattering one. Stage 1's payoff was a *prediction* until that run — nothing
elsewhere in this document measured it. **That run has since happened; its results
are the next subsection, and each prediction below is answered there.**

### Measured outcome (2026-08-24, 416 rows, checker `ddb0ecd8…`)

The run happened. Baseline was the 2026-08-24 export-kind-proof state
(267 verified / 129 refused / 24 failing claims / `kind-observed` root cause 74),
against an engine **byte-identical** to it, so every movement is the verifier's.
Each prediction below is answered in place; the full account is in
[docs/ecosystem-benchmark.md](../ecosystem-benchmark.md).

| Prediction | Measured | Verdict |
| --- | --- | --- |
| verified up, +10 at most from stage 1 | 267 → **275** (+8), nothing lost | held, under the bound |
| entrypoints refused at verification: 0 → some | **30**, across 8 rows | held |
| `kind-observed` root cause down, never to 0 | 74 → **64** | held |
| the undriven `other` bucket → 0, all one class | **671 → 0**, all `probe session aborted by package code` | held; the *834* was stale, see below |
| blocker lines classed `kind-observed` down, unattributable | 322 → **160** | held; read the row count (88 → 68) |
| failing claims must NOT move: 24, of which 13 `kind` | **24, of which 13 `kind`**, same five shapes | held |
| `incompleteness` and `probe-failed` root causes must not move | **38** and **15** | held |
| the `kind-observed` co-blocker rows must stay refused | all **14** still refused | held |
| per surviving row, no document may certify more | 267 surviving rows: conversions, `probedRows`, promoted exports and `verificationRefusedEntrypoints` **identical on every one** | held |
| `verificationRefusedEntrypoints` on a previously verified row: 0 | **0** on all 267 | held |
| publish once: how many newly verified rows carry a probed behavioral row | **0 of 8**; corpus stays 3 of 275 | the answer is the bad one |

Three things this document got wrong or under-specified, recorded because the
point of a re-measurement is to catch them:

- **The 834 was stale.** The undriven `other` bucket was 834 in the journal this
  document was written against, but **671** in the state stage 1 actually
  baselines on — the export-kind proof shrank the claim plan in between. It
  resolved 671 → 0, entirely to `probe session aborted by package code`, which is
  the predicted *shape* at a corrected magnitude. Re-bucketing the baseline
  journal reproduces the same 671, so the classification is a property of the
  rules rather than of the run.
- **The stage-1 shortfall is fully explained, and this document named the cause
  in advance.** Of the previous state's 74 kind-rooted refusals, 18 were
  multi-entrypoint: **8 converted**, **8 had no surviving entrypoint at all**, and
  **2 had a survivor but kept refusing on a closure note** —
  `@solidjs/start@2.0.3` (12 entrypoints) and `@tanstack/charts@0.14.0` (110
  entrypoints, 91 blocker lines, the row this document singled out). Their root
  cause moved `kind-observed` → `closure-note`, which is the whole of that count's
  2 → 4. The carve-out being correlated with the population is exactly what
  "Payoff" predicted.
- **The 29 co-blocker rows were 14 by the time stage 1 ran.** The count came from
  the 77-row state; the export-kind proof had already removed half of them. All 14
  still refuse, which is what the invariant was actually testing.

### Stage 2's gate, closed

**Measured payoff: 0 rows. Stage 2 must not be built**, on this document's own
gate condition — *"if stage 0 shows the server-only gaps are session deaths
rather than absences, stage 2 buys nothing and must not be built."*

Of **6,962 gap (claim, mode) pairs** across 84 rows, only **45 (0.65%)** are the
`export-missing` observation of absence stage 2 would narrow on. The rest are
import throws (3,878), sessions aborted by package code (2,629), sessions that
wrote no report (328), and unresolvable summary sets (82). Sharper still:

- **3 rows of 84** carry any `export-missing` gap, and in none of the three is it
  the only gap reason, so excluding the absences would promote none of them.
- All three (`solid-js@1.9.14` and both `@solidjs/web@2.0.0-rc.1` probes) are
  root-caused **`probe-failed`** — an independent blocker no option here may
  absorb — so they are outside the addressable population by definition.
- Against the **64 rows root-caused `kind-observed`**, the population stage 2
  exists for: **zero** carry a single `export-missing` pair.

The bound this document set was "at most 43 rows, and the report cannot tell us
how much of the 43 is real." The answer is none of it. The gaps really are import
failures and session deaths, and the one narrowing this amendment could defend
has almost nothing in the corpus to narrow.

Separately, the contradiction separation held exactly as required: **7 rows, 13
claims, 52 (claim, mode) pairs**, and that 13 is the same 13 as the failing-claim
total's wrong-`kind` count — no contradiction leaked into a gap table, and no gap
was counted as a contradiction.

**Should go UP**

- verified rows: 261 → 271 after stage 1 (the 10 measured survivors); after
  stage 2, up to +43 more, and the actual figure is stage 0's answer, not this
  document's.
- entrypoints refused at verification: 0 → the count stage 1 introduces. A
  rising number here is the *cost* being made visible, not a regression.

**Should go DOWN — and if they do not, the change is wrong**

- refusals root-caused `kind-observed`: 77 → lower, but **never to 0**. Zero
  would mean gaps are being narrowed away, not just absences.
- the undriven `other` bucket: 834 → **0**, with all 834 named as
  `probe session aborted by package code`. Recomputed from the same journal
  rather than predicted (see stage 0), so a re-measure that disagrees means a
  reason string moved, not that the classification improved.
- blocker lines classed `kind-observed`: 357 → lower, but by an amount no
  reading of it can attribute. A refused document now raises one summary line
  *plus* the per-entrypoint attribution HEAD wrote, so a row's line count went
  from *r* to *r + 1* while rows leaving the refused set remove theirs entirely.
  **Read the row count, not the line count.**

**Must NOT move**

- failing claims: **63**, of which **53 `kind`**. A drop here without a
  generator change is the sentinel-absorption failure this amendment forbids;
  assert it explicitly in the re-measurement, not by eye.
- `incompleteness` and `probe-failed` root causes: 40 and 27.
- the 29 rows where `kind-observed` is a co-blocker must stay refused.
- **per surviving row**, no promoted document may certify more than it did
  before: conversions and `probedRowsKept` for a row that verified under the
  old rule must be identical under the new one, because stage 1 only ever
  *removes* entrypoints from a document that was already promotable. Stated as
  corpus totals — 739 conversions, `probedRowsKept` 3 rows / 3 markers — the
  invariant is **wrong and will fire falsely**: `accumulate` counts both only
  for `outcome === "verified"`, so 10 newly verified rows necessarily add
  theirs. Read it row by row against the previous journal, or rebase the totals
  explicitly on the newly verified set; a corpus total that rose is not the
  entrypoint-kept regression this is trying to detect.
- `verificationRefusedEntrypoints` on a row that verified under the old rule:
  **0**. A refusal there would mean an entrypoint was dropped from a document
  the old rule promoted whole, which is a document that now certifies *less*
  than a run already checked — a different bug from the one above, and equally
  worth catching.

**Also worth publishing once**, because it is the number that will decide
whether this whole line of work matters: how many of the newly verified
contracts carry any probed behavioral row. If it stays at 3, machine
verification is certifying negatives and `typeof`, and the honest conclusion is
that the human tier is still the useful one for positives — RFC 0002 unresolved
question 1's own stated risk.

## What this amendment does not resolve

- **The `kind` sentinel question itself is deferred, not answered.** It is
  blocked on the same schema-v2 decision as `evidence.verifier` (RFC 0001 §4)
  and the sentinel `reason` (UQ5), and should be taken once, for all three.
- **Per-export omission** stays out until it is established whether an export
  absent from a present entrypoint raises `SC9005` at the consumer or resolves
  silently to no summary.
- **A package the probe cannot import at all remains unverifiable**, which is
  A1's deliberate consequence and is unchanged. Measured 2026-08-24: **the
  remaining 43 are gaps, not absences** — so this is the whole population rather
  than a part of it, and 64 rows stay refused on `kind-observed` with no stage
  left that could reach them. `contract review` is where an unimportable package
  belongs.
- **A closure note still refuses the document**, including on an entrypoint
  stage 1 would otherwise refuse on its own. Measured: it cost exactly **2 of the
  10** candidate rows — `@solidjs/start@2.0.3` and `@tanstack/charts@0.14.0`.
- **The generator defects behind the failing `kind` claims** are unaddressed here.
  The 53 have since been reconciled to **13** by the export-kind proof pass (25
  corrected, 15 withdrawn with a refused entrypoint), and those 13 are still
  wrong: one family, a binding whose type is a class reached only through a value
  expression, needing a constructability fact from the Type Facts producer. The
  `@solid-devtools/locator` case that published a maximal certified negative for
  callback-taking functions is among the 15 whose entrypoint generation now
  refuses outright.
