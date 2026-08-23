# RFC 0002: Machine-verified contracts

- **Status:** Draft
- **Authors:** solid-checker maintainers
- **Date:** 2026-08-22
- **Affects:** [package-contracts.md](../package-contracts.md),
  [precision-backlog.md](../precision-backlog.md), `packages/cli`, `scripts/`,
  and — at Stage 3 only — the TypeFacts protocol and its pinned revision
- **Does not affect:** the analyzer's evidence enforcement, the loader's four
  discovery tiers, the contract schema's precedence rules, or what a certifying
  contract means to a consumer
- **Relates to:** [RFC 0001](0001-contract-registry.md), which distributes
  contracts. This RFC changes how they are produced.

## Summary

Today a generated contract certifies nothing until a human resolves every item
of its review plan. This RFC removes the human gate from the *certification*
path — not by trusting the generator's guesses, but by narrowing what a machine
is allowed to certify to two disjoint sources of proof:

1. what the generator proved statically and already fails closed on, and
2. what a probe **observed** by executing the claim against the installed
   package.

Everything else — every positive claim the machine can neither prove nor drive
— is converted to the existing `{"status":"unknown"}` sentinel *before*
promotion, which downgrades that claim domain to a demand-sensitive
uncertifiable finding at the consumer rather than to a guess.

Two things are proposed. A new command, `solid-checker contract probe`
(**does not exist**), which executes the drivable claims of a generated
contract against the installed release and records `probed` row evidence. And a
mechanical promotion to `evidence.kind: "verified"` — the evidence kind whose
reserved meaning in [package-contracts.md](../package-contracts.md) is already
exactly *"mechanical artifact/surface/behavior checks passed"*, and which no
command in this repository currently writes.

`contract review` remains, unchanged in meaning, as the **optional escalation**
that turns unknowns into reviewed claims and supplies the claims no machine can
produce. It stops being a gate and becomes an upgrade.

## Motivation

### What exists

`contract generate` writes a contract whose `evidence.kind` is always
`inferred` — hardcoded, in `packages/cli/scripts/generate-package-contract.mjs`,
as `{ kind: "inferred", generator: "solid-checker package generator" }`.
Consumers report it as `unverified`, and — the load-bearing fact — its
summaries **are not inserted into Reactive IR at all**. It cannot prove a
violation, discharge an obligation, or certify a consumer. Generation
deliberately never promotes an inferred claim.

Row evidence is different in a way this RFC depends on. `annotateClaimEvidence`
applies `row.evidence ?? { kind: "inferred" }` — a **default, not an
override** — so a row that already carries `probed` or `inherited-from`
evidence keeps it through a regeneration. And on the consumer side,
`claims_are_certifiable` in `rust/crates/solid-reactive-ir/src/lib.rs` treats
`probed`, `reviewed`, `inherited-from`, *and absent row evidence* as
certifiable, rejecting only `inferred`. Those two facts are why §2 can drop an
inferred marker rather than forge a per-row assertion, and why §4's
regenerate-and-re-probe loop is not obliged to re-probe what it already
observed.

The only promotion path is `contract review`, which is a recorded sequence of
per-item human decisions over the plan in `<contract>.review.json`. That plan
has eleven item kinds (`SECTIONS` in
`packages/cli/scripts/contract-review-plan.mjs`): `refused-entrypoint`,
`legacy-root-field`, `artifact-binding`, `no-export-summary`,
`unknown-sentinel`, `no-callback-row`, `callback-without-owner-row`,
`generated-owner-requirement`, `inherited-claim`, `conditional-environment`,
and `generated-summary`. The last is raised for *every* `(entrypoint, export)`
carrying any generated positive row, and exists precisely so that "every export
a promotion certifies is named by at least one decision". A contract with
exports cannot produce an empty plan.

`review-contract.mjs` writes exactly one evidence kind. Its `DECISIONS` comment
says why:

> `verified`, `trusted` and `attested` are deliberately absent from the whole
> command: they mean a mechanical artifact/surface/behavior check, an
> out-of-band trust decision, and a verifier-produced release identity. This
> records a human review, so `reviewed` is the only evidence kind it writes.

So `verified` has been reserved, specified, accepted by certification, and
never produced. This RFC produces it.

### Why the gate is the whole cost

Generation reachability is not the problem. The checked-in ecosystem report
(`benchmarks/ecosystem/report.md`, 305 manifest rows / 416 probes) shows the
generator describing the corpus well. Every one of those contracts is
`inferred`, and each one costs a human a full plan review before it does
anything at all. The cost is per *artifact*, not per package — a contract binds
to one version, and to one npm integrity when the project's lockfile can supply
it — so it recurs on every upstream release. `--transfer-from` reduces an
upgrade to a review of the diff, but only for entrypoints whose runtime-module
closure is byte-identical, and only where the previous review exists to
transfer.

The consequence is that an application developer importing a dozen Solid-aware
packages this checker does not bundle sees twelve `SC9005
package-contract-incomplete` findings and has twelve reviews standing between
them and a certifiable project. That is the entire adoption cost of the package
boundary.

### The registry is dedup, not authority

RFC 0001 proposes distributing reviewed contracts so one team's review reaches
another project. That is worth doing and this RFC does not replace it — but it
is amortization, not elimination: the corpus still needs someone to perform
each review, and RFC 0001's own unresolved question 7 is that nobody has
measured which packages to review first.

If verification is *mechanical*, it is also *reproducible*, and a registry
stops needing to be trusted about it: registry CI re-runs the verification
rather than believing the submitter. The registry becomes a cache for work
anyone could redo, plus a genuinely non-reproducible tier — human review — for
the claims a machine cannot reach. §5 develops this.

## The principle

**Wrong is dangerous; incomplete is safe.**

A contract's failure mode is asymmetric, and every decision below follows from
the asymmetry. Omitting an effect field is a *reviewed negative claim*: a
summary with no `callbacks` field certifies "this export never invokes a
caller-supplied callback", and a certifying contract's summaries are inserted
into Reactive IR. A contract that is wrong therefore silences the checker
exactly where it should fire, and the failure is invisible — a clean report.

An *incomplete* contract fails in the opposite direction and does so loudly.
The `{"status":"unknown"}` sentinel is not a weaker claim; it is the absence of
one. Unknown is not evidence and cannot be promoted. A consumer facing an
unknown domain opens a per-export obligation that is **demand-sensitive**: it
fires only where the unknown surface is actually touched. An unknown
`callbacks` domain produces an `SC9005` uncertifiable finding when a
potentially callable argument is supplied, and `slice(list, 0, 2)` stays clean.
The other known claim domains of that same export remain available.

So the rule this RFC enforces mechanically:

> A machine may certify exactly what it proved or observed. Every other
> positive claim it holds must become the unknown sentinel before promotion.
> Never a guess, never a downgrade that hides.

The second clause is the one with teeth. Converting to unknown *loses
information* relative to a human review — the machine may have inferred a
correct callback row and be unable to confirm it — and that loss must be
visible in the contract, in the probe report, and in what the consumer reports.
An unknown that reads as a certified negative is the exact failure the
principle exists to forbid, which is why the sentinel's JSON type deliberately
disagrees with each field's old type so that older readers fail closed rather
than misreading the omission.

## Claim taxonomy

This is the core of the proposal, and it must be grounded in what the machinery
actually does rather than in what a claim family sounds like.

The probe suite's claim vocabulary is **two strings**. In
`scripts/check-bundled-contracts.mjs` a claim is built as either
`callbacks[<parameter>]=<execution>` or `returns=<kind>`. There is no probe form
for a reactive read, an owner requirement, an async behavior, a callback
argument descriptor, or a nested return leaf. And `writeProbeEvidence` writes
`probed` row evidence onto exactly four places: the export summary, each
`callbacks[]` row, the top-level `returns` node, and recursively each
`variants[].summary` — never into `reactiveReads[]` or `ownerRequirements[]`
even though `schema/solid-reactivity.schema.json` gives both an evidence slot,
and never into `asyncBehavior`, which is a bare enum with no evidence slot at
all.

### The three families

**(A) Statically proven by construction.** Claims the generator derives from
exact compiler facts and already fails closed on. The negative claims live
here: the generator does not omit an effect field because it found nothing, it
omits one because it proved the behavior absent — and when it cannot, it emits
the unknown sentinel instead. The documented construction is explicit: *"When
an exported parameter escapes through an uncontracted external call whose
execution semantics are unknown, generation preserves the other proven claim
domains and emits `callbacks: { "status": "unknown" }` for that exact export.
It never emits an empty, falsely inert callback summary."* That extends to a
callee with no resolvable identity at all — `list.map(fn)` where `list` is one
of the exported function's own parameters. Also here: `ownerRequirements`,
derived from the compiler's canonical symbol identity and assigned only to the
immediate containing function body; `variants` and their conditions, read off
the export map; and the reactive-read rows the compiler facts make exact.

**(B) Runtime-confirmable.** Positive behavioral claims a probe can *observe*
by executing the claim against the installed release, in every applicable
condition mode, with initial and subsequent calls. Today this family is:
callback execution mode (`inline`/`deferred`/`tracked`), top-level reactive
return kind (`accessor`/`store-path`), and the export's runtime `kind`
(function versus value), which `describePackages` in
`scripts/lib/contract-probe-harness.mjs` already reads generically for any
package by importing each materialized entrypoint leaf and taking
`typeof value`.

**(C) Neither.** A positive claim that is not (A) and that the harness cannot
drive. This family is **converted to the unknown sentinel for its domain before
promotion**, and it is larger than it sounds.

### Where each schema-v1 claim falls

| Claim | Family | Grounding |
| --- | --- | --- |
| `kind: function` / `value` | B | `describePackages` reads runtime kind per entrypoint leaf, generically, in every mode |
| `callbacks[].execution` | B | claim string `callbacks[N]=<execution>`; probe bodies exist for all three modes |
| `callbacks[].owner` | **C, permanently** | the generator never emits one — owner rows go on the review checklist rather than being guessed — and no probe claim family covers owner |
| `callbacks[].arguments[]` descriptors | C today | no claim string, no probe shape; consumers are already demand-sensitive on every shape but an inline literal carrying `accessor` descriptors |
| `returns` (top-level `accessor` / `store-path`) | B | claim string `returns=<kind>` |
| `returns` nested `elements` / `properties` leaves | C today | `writeProbeEvidence` does not descend into leaves and no claim string names one |
| `returns` `kind: argument` | C today | an identity claim; observable in principle, no probe form |
| `reactiveReads[]`, including `parameter-member` | A where compiler facts are exact; otherwise C | no probe claim string exists; confirming one means synthesizing a reactive source and observing the subscription |
| `asyncBehavior` | C today | no claim string — **and no evidence slot in the schema**, so a probed async claim could not be recorded even if it were driven |
| `ownerRequirements[]` | A | compiler symbol identity; the slot exists, nothing writes `probed` into it |
| `variants[]` and their `conditions` | A | read off the export map; probes select modes through `modeApplies`, they do not discover branches |
| **an omitted effect field** (the negative claim) | **A only, and never B** | see below |

### Negatives are not probeable, and that is the deepest constraint

A probe can *falsify* a negative claim and can never *verify* one. Observing
that `leading`'s scheduler factory argument is invoked disproves "never invokes
a caller-supplied callback". Observing nothing proves nothing: the export might
invoke the callback on the third call, under a condition the probe did not
enumerate, or on an argument shape the probe did not synthesize.

Negatives are also the dangerous claims — they are what suppresses findings —
and they are the most numerous, because every export certifies a negative in
every domain its summary omits.

So the entire negative surface of a machine-verified contract rests on family
(A): the static soundness of the generator's fail-closed construction. No probe
budget changes that. §4 says what bounds it and what does not.

One mechanical falsifier does exist and is worth keeping. The bundled suite
already computes `discoveredClaims` — probes that passed for a claim the
contract does not state — and reports them as `INCOMPLETENESS` **failures**,
not as writes. That path is the only automated check anywhere in the repository
capable of contradicting a negative claim, and §1 makes it a promotion blocker.

## Detailed design

### 1. `solid-checker contract probe <contract>`

**Status: does not exist.** `packages/cli/bin/solid-checker.mjs` dispatches
`contract generate` (with `--missing` routed to
`scripts/generate-missing-contracts.mjs`), `contract review`, and
`contract check` (the discoverable spelling of the native `--check-contracts`).
`probe` is a fourth branch and is Node-only.

It takes a contract `contract generate` wrote, plus the installed package it
describes, and for every claim in family (B) it constructs and runs a probe
against the installed release. It writes `probed` row evidence for each claim
that passed in every mode the claim is stated for, and it writes nothing else
into the contract. It is opt-in and never runs as part of `contract generate`.

**Why a separate command, and not a flag.** Generation's design property,
stated twice in [package-contracts.md](../package-contracts.md), is that
*"package code is never imported or executed"*. Probing necessarily executes
it. Folding probing into `contract generate` would silently convert a static
analysis command into one that runs arbitrary dependency code, including
import-time side effects. Keeping it a distinct opt-in command makes the trust
decision explicit — it is the same trust as running one's own dependencies, but
it must be *taken*, not inherited from a flag default.

**Recommended isolation.** The probe should be run in a sandbox: a container or
VM, no ambient credentials, no network egress, a scratch copy of the install
tree. This RFC recommends it and does not enforce it; enforcing it portably is
unresolved question 4.

**What a probe needs to know, and therefore where "drivable" ends.** This is
the honest core of the command, and the existing machinery understates the
difficulty badly. `scripts/lib/contract-probe-harness.mjs` is 122 lines and
supplies only four things: the export-surface reader, a recorder that runs a
body under a disposable owner and records `{ok, mode, calls}`, and the stdout
report. Everything that makes a probe a probe is hand-authored per export in
the dialect workers. In `scripts/contract-probes.mjs`, driving three claims
requires knowing that `createProjection` takes `(draft => …, {value: 0})`, that
`runWithOwner` takes `(getOwner(), cb)` so the callback is parameter **1**, and
that `mapArray` takes a *signal of an array* as parameter 0. It also requires
knowing that a 2.0 development build rejects writes made from a parent-owned
test root, which is why `writeOutsideOwner` exists.

There is therefore **no generic probe driver in this repository**, and building
one is the substance of Stage 1 rather than an extension of what exists. A
generic driver must answer, for an arbitrary export:

- **Argument synthesis.** What to pass in the non-callback positions so the
  call reaches the callback at all. A contract records a callback's
  *parameter index*, so the driver knows which slot is the callback; it records
  nothing about the other slots. The only sound synthesis is from the package's
  own declarations, which this generator never resolves (it analyzes runtime
  targets and never resolves the `types` condition — which is also why
  `artifacts.declaration` is never emitted).
- **Callback identification.** Solved: `callbacks[].parameter` is exact.
- **Runtime settling, per dialect.** 2.0 settles with `flush()`; 1.x has no
  such function. The driver must settle the dialect the *consumer's project*
  resolves, not a checked-in worker's.
- **Applicable modes.** `modeApplies` already derives them per entrypoint from
  its `conditions`, and `--conditions`-scoped contracts already restrict what a
  probe may claim. But a package may legitimately state fewer modes than it
  resolves — Solid 1.x's `node` build is a genuinely different artifact where
  `createEffect` never runs — and today that is hand-declared as `probeModes`
  on a dialect manifest entry. Nothing derives it.
- **What counts as observing the claim.** `inline`, `deferred` and `tracked`
  classify **attribution, not timing**: an `inline` probe puts the export call
  inside a memo and checks the memo re-runs when a signal the callback read
  changes; a `deferred` probe checks it does not. That shape *is* generic once
  the arguments exist.

The conclusion the RFC commits to: **argument synthesis is the boundary of
"drivable"**, and a claim whose call the driver cannot construct is family (C).
A zero-argument-besides-the-callback export is drivable; `createProjection(fn,
{value: 0})` is not, without a source of the second argument. What fraction of
real claims that excludes is unmeasured — unresolved question 1.

**Probes confirm; they never write behavior.** `--write` in the bundled suite
"records passing modes as `probed` row evidence on claims that already exist".
`contract probe` inherits that rule verbatim. A probe that observes a claim the
contract does not state is an `INCOMPLETENESS` report and a **promotion
blocker**, never a new row: a probe observation is a single-mode, single-shape
sighting, and turning it into a contract claim would be exactly the guess this
RFC forbids. It is, however, proof that a *negative* claim is wrong, which is
why it blocks rather than merely warns.

**Output.** A probe report beside the contract —
`<contract>.probe.json` — recording, per `(entrypoint, export, claim)`, the
modes attempted, the modes passed, the call counts, and for every family-(C)
claim the reason it was not drivable. It also records the identities the result
is a function of: the installed `(version, integrity)`, the generator identity
the review plan already carries (`generatorIdentity()`, spelled
`<package>@<version>` from the CLI's own manifest), and a probe-driver
identity of the same shape. The report is not a loader input and nothing
certifies from it. It is the audit trail for what the machine believed and
could not confirm, and it is where the "converted to unknown" record lives
(see §2 and unresolved question 5, since the contract itself cannot carry it).

### 2. Mechanical promotion to `verified`

A second new mode, `solid-checker contract probe <contract> --promote verified`
(or an equivalent `contract verify`; the spelling is bikeshed). It refuses —
one clear line each, contract untouched — unless **all** of the following hold.

1. **Every positive claim is probed or converted.** For each export summary
   and each variant summary, every family-(B) claim carries `probed` row
   evidence covering every mode the claim is stated for. Every claim not so
   covered has had its whole domain replaced by `{"status":"unknown"}`.
   Conversion is per *domain*, not per row, because the sentinel is a field
   value: one unconfirmable callback row converts the export's entire
   `callbacks` field. This is deliberately lossy and is the price of the
   sentinel being the only "not proven" spelling schema v1 has. It is also
   already how the generator behaves across conditional branches —
   `mergeClaimRows` returns `{status:"unknown"}` when *either* side is unknown,
   so unknown is contagious by construction rather than by this RFC's
   invention.
2. **No probe failed, and no incompleteness was reported.** A failed probe
   means the package does not behave the way the contract says; that is a
   generator bug or a package change, and neither is fixed by converting the
   claim to unknown. It fails the promotion outright. An incompleteness report
   contradicts a negative claim and does the same.
3. **No `inferred` row evidence survives.** `claims_are_certifiable` rejects
   any inferred row inside an otherwise-certifying document, so a promotion
   that left one would produce a contract the loader refuses to certify. Rows
   whose claim was converted to unknown vanish with the field; rows in family
   (A) have their marker **deleted**, exactly as `dropInferredRowEvidence` does
   under `--promote reviewed` today — a row with no evidence of its own
   inherits the document's, and writing a per-row marker would claim an
   assertion nobody made. `probed` and `inherited-from` markers are left
   untouched.
4. **No closure note on any emitted entrypoint.** A `notes` entry in the review
   plan's `generation.entrypoints` block means the runtime-module closure could
   not be fully enumerated — a relative or `#` specifier that names no runtime
   module, a conditional `imports` branch generation cannot choose between, a
   non-literal dynamic `import()`, unreadable bytes. A note is an omission, and
   an entrypoint carrying one already transfers nothing. It must block
   auto-verification outright: the summaries were derived from a file set the
   generator itself declines to claim it enumerated, and no probe covers the
   negative claims that file set determines. This is the
   [walked-not-attested residue](../precision-backlog.md) and Stage 3 is its
   fix.
5. **Artifact binding is recorded where schema v1 can carry it.** That means:
   the contract's emitted entrypoints resolve to exactly one runtime artifact
   inside the contract's own directory. Where they do not, the review plan's
   `contract artifact binding` line already says why, and the promotion
   proceeds without a hash. **This condition is vacuous exactly where it
   matters most** — see the honest note below.
6. **The document passes `--validate-contract`,** written to a temporary file
   in the contract's directory and renamed over it only after validation, the
   way `--promote reviewed` already does.

Refused entrypoints do **not** block. A refused entrypoint is absent from the
contract, so a consumer importing it already gets an explicit uncertifiable
result rather than a wrong claim. `contract generate` already exits 0 for a
decided refusal and appends `; N entrypoint(s) refused and omitted`; nothing
about mechanical promotion changes that a partial contract is a safe contract.

**The honest note on condition 5.** `contract generate --missing` writes
project-owned contracts under `.solid-checker/contracts/<package>/`, which sit
outside the package by construction, so their artifact path could only be
spelled with `..`, which the loader rejects. Every contract on the mainstream
adoption path is therefore *unbindable*, and condition 5 is satisfied
vacuously. Such a contract binds to a version string, plus npm integrity when
the project has a `lockfileVersion` 2 or 3 npm lockfile — and on pnpm or Yarn,
to nothing but a version string. Probing narrows this a little in a way hashing
cannot: a `probed` row is an observation of *the bytes that were installed at
probe time*. It does not say which bytes those were. This is a real hole and it
is not one this RFC closes.

**Why `verified` and not `reviewed`.** The `generated-summary` plan item exists
so that no row reaches **`reviewed`** evidence without a human decision naming
its export. Mechanical promotion does not weaken that invariant; it declines to
enter that tier. `verified`'s reserved meaning is *"mechanical
artifact/surface/behavior checks passed"*, which is precisely what conditions
1–6 are. The uncomfortable part is that the invariant's *consumer-side*
consequence is unchanged: certification accepts `verified`, `reviewed`,
`trusted`, and `attested` identically, and `contract check` reports a
project-owned contract as `local` regardless of which of them it carries. A
project cannot today say "certify only human-reviewed contracts". That is
unresolved question 6.

### 3. What blocks auto-verification, in one list

- a failed probe in any stated mode;
- an incompleteness report (a probe observed a claim the contract denies);
- a closure `notes` entry on any emitted entrypoint;
- surviving `inferred` row evidence;
- a document that does not validate.

And what does *not* block:

- a refused entrypoint (absent, hence already uncertifiable at consumers);
- an unbindable artifact (recorded on the plan, see above);
- an undrivable claim (converted to unknown, hence already uncertifiable at
  consumers, demand-sensitively);
- a missing callback `owner` row (missing owner is fail-closed `SC9012` at the
  consumer, never inherited-owner proof).

### 4. Upgrades

A `verified` contract needs no review transfer. The upgrade is:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
solid-checker contract probe .solid-checker/contracts/reactive-package/solid-reactivity.json \
  --promote verified
```

Two commands, no human decision, reproducible by anyone with the same installed
artifact. `--transfer-from`, the `.previous.json` snapshot move, and the whole
review-state machinery remain for the human-reviewed tier, which still needs
them for exactly the reason RFC 0001 §9 gives.

**The cost is probe time per upgrade**, and it is not small: every claim, in
every applicable mode, with initial and subsequent calls, plus an install and
an import of the package under each mode. The bundled suite pays this for two
dialects and a handful of packages; a project paying it for every dependency on
every lockfile refresh will notice. Whether probe results can be cached against
`(package, version, integrity, generator identity)` — and whether that cache is
a local artifact or exactly the thing RFC 0001's registry should distribute —
is §5.

Mixing tiers has one sharp edge. `--transfer-from` compares expanded export
summaries with `probed` markers *surviving* the projection, so a re-probe that
records a different mode set on a row changes the summary and blocks the
transfer. That is correct — different observations are a different review
basis — but it means a human-reviewed contract that also carries probed rows
loses the version-bump fast path whenever probe coverage shifts.

### 5. Registry interplay

RFC 0001's registry is designed around a fact that machine verification
removes: that a reviewed contract is *expensive and non-reproducible*, so the
registry must be trusted about it, hence signatures, a trust set, a lock, and
governance modeled on DefinitelyTyped.

For a machine-verified contract none of that is load-bearing in the same way.
Verification is a pure function of `(installed artifact, generator identity,
probe driver identity)`. The registry's CI can therefore **re-run it** rather
than believe the submitter — the entry is a cache of a computation, and a
submitter who lies is caught by recomputation, not by policy. That collapses
the interesting half of RFC 0001's threat model for this tier: content
addressing still matters (it is what makes the cache checkable), signatures
matter much less, and governance stops gating throughput.

The registry therefore serves two tiers with different properties:

- **Machine-verified contracts: a dedup cache.** Reproducible, re-verified in
  CI, publishable without human review, and safe to bulk-generate across the
  ecosystem corpus. The value is saving every consumer the probe time of §4,
  not vouching for the claim.
- **Human-reviewed contracts: the resolved-unknowns tier.** Not reproducible,
  therefore genuinely requiring RFC 0001's signatures, trust set, and named
  reviewers. This is where a converted-to-unknown `callbacks` domain becomes a
  reviewed callback row, and where callback `owner` rows — which no machine can
  ever produce — come from.

The two tiers must not silently substitute for one another, which reinforces
unresolved question 6: a consumer needs to be able to say which tier certifies
their project, and today the evidence kinds are consumer-indistinguishable.

## What replaces the human hedge

The human review is not merely a rubber stamp being removed; it is the only
thing currently standing between a generator bug and a wrongly certified
negative claim. Naming what takes over, and what does not, is the honest part
of this RFC.

**Probes execute the actual claim, in every stated mode, twice.** Not a
signature check and not a heuristic: the `tracked` claim on `createMemo` is
confirmed by writing a signal the callback read and observing a re-run. Modes
are independent runs (`client`, `server`, `development`, `production`), and a
claim that passes in one mode and fails in another is a conformance failure —
"a surfaced environment mismatch, not a reason to omit that mode or silently
weaken the contract". This is strong evidence, and it covers only family (B).

**Incompleteness reports falsify negatives.** The `discoveredClaims` path is
the one automated mechanism that can contradict "this export invokes no
callback". It is a sampling check, not a proof, and it only sees behavior the
probe driver happened to elicit.

**The differential harness pins source-vs-boundary parity.** `make
contract-differential` analyzes a package implementation as project source,
generates its contract, promotes the rows *inside the test*, and asserts the
consumer's findings are identical. It catches behavior lost at the contract
boundary. It must be described accurately: `scripts/contract-differential.mjs`
is **one synthetic package with three exports** — `runMixed`, `ownedEffect`,
and an anonymous default — deliberately built as "a small, executable parity
probe rather than a second contract implementation". It is a regression gate on
the boundary, not a corpus.

**The torture corpus gates engine changes.** `.github/workflows/contract-corpus.yml`
runs on changes under `rust/crates/solid-reactive-ir/`,
`generate-package-contract.mjs`, and the corpus fixtures, with checked-in
expected outputs reviewed like snapshots. It is **six fixtures** —
runtime-mutated namespaces, conditional semantic branches, getter-backed
exports, deep re-export barrels, declaration/runtime disagreement, and
environment conditions. It catches an engine change that moves a known case; it
says nothing about a case nobody wrote.

**Closure notes fail closed**, and block promotion outright per §2 condition 4.

**Stage 3 upgrades the closure from walked to attested.** The closure in a
review plan's `generation.entrypoints` block is produced by
`packages/cli/scripts/runtime-module-closure.mjs`, a scanner in the Node
process — *not* the file list the analyzing program opened. It is now
fail-closed for every static specifier form it recognizes, but the residue is
real: "a syntax walk can still disagree with the compiler in ways neither side
reports — a `paths` mapping, a resolution the bundler condition resolves
differently, a specifier form the scanner classifies as external that the
analyzed program in fact opened." The exact fix is a TypeFacts protocol
addition emitting the compiler's own module list.

**And here is what none of them do.** Until Stage 3, generator soundness is the
trust root, and even after Stage 3 it remains the trust root for the *analysis*
— Stage 3 attests which files were read, not that the conclusions drawn from
them are right. A generator bug can emit a wrong family-(A) claim, including a
wrong negative claim, and every mechanism above can pass. The differential
harness would catch it only if the bug reproduces in its one fixture; the
torture corpus only if it reproduces in one of six; probes only if the bug is
in family (B) or elicits an incompleteness. The mitigations **bound** the
exposure; they do not eliminate it.

A human review does not eliminate it either — a reviewer confirms a generated
claim they did not derive independently, and `confirm` is the most common
decision — but a reviewer reads the implementation, and that is a genuinely
different error source from the one that produced the claim. Removing the gate
removes that independence. This RFC accepts that trade because the alternative
is a corpus of `inferred` drafts that certify nothing, and because the failure
mode of the *machine* path is bounded by the unknown-conversion rule in a way
the human path is not: a reviewer can certify a negative by hand, and the
machine cannot.

## Alternatives considered

**Trust `inferred` outright — promote generated contracts as-is.** Rejected,
and not a close call. It is the one change that would make the checker's
failure mode silent suppression by default. The generator's own construction
argues against it: it emits `callbacks: {"status":"unknown"}` precisely because
it *knows* it cannot prove that domain, and promoting the document wholesale
would either certify that marker (which the schema forbids — unknown cannot be
promoted) or require deleting it, which is the exact "certify the negative"
edit that `contract review`'s `absent` decision exists to make deliberate.
Evidence enforcement is not decorative: inferred summaries are not inserted
into Reactive IR at all, and that is the property doing the work.

**Let probes discover behavior — write new rows from observations.** Rejected.
`--write` on the bundled suite already refuses to do this, and the refusal is
documented as load-bearing: "it never discovers or adds an uncontracted
behavior", and a discovered claim is reported as `INCOMPLETENESS`. A probe
observation is a single shape, in one mode, with the arguments the driver
happened to synthesize. Promoting it to a contract row would generalize a
sighting into a claim about every call — which is a guess with a runtime
flavor, not evidence. The asymmetry is the point: an observation that
*contradicts* a contract is conclusive (the behavior happened), while an
observation that *extends* it is not (the behavior happened once, here).

**Keep the human gate.** Rejected by decision, and what is lost should be on
the record rather than argued away:

- Callback `owner` rows. The generator does not guess them, no probe covers
  them, and a reviewed `leaf` row is what preserves the fact that cleanup,
  flush, and nested primitive creation are forbidden in an owner such as
  `onSettled`. A machine-verified contract systematically cannot discharge
  owner, cleanup, or leaf obligations; consumers stay on `SC9012`. This is a
  permanent capability gap between the tiers, not a Stage-4 item.
- Every family-(C) claim the machine held and could not confirm. A reviewer
  reading the implementation would have kept it; the machine converts it to
  unknown and the consumer gets `SC9005` where the surface is touched.
- Independent error detection. See above.
- The `generated-summary` invariant, which guarantees a human named every
  certified export. Machine promotion declines that tier rather than weakening
  it, but the consumer cannot currently tell the tiers apart.

**Add a `machine-verified` evidence kind rather than reusing `verified`.**
Rejected. `verified` is already specified as "mechanical artifact/surface/
behavior checks passed", already accepted by certification, and already
deliberately unwritten by every command. A new kind would be a schema change
partitioning clients for a distinction the reserved vocabulary already draws.

## Unresolved questions

1. **Argument synthesis limits — how much is actually drivable.** The boundary
   of "drivable" is whether a driver can construct a call that reaches the
   callback, and the only sound source for the other arguments is package
   declarations this generator never resolves. What fraction of real claims
   that excludes is **unmeasured**. The unknown-density half of the question is
   now measured (2026-08-22, `docs/ecosystem-benchmark.md`): of 409 emitted
   contracts, 300 (73%) are fully proven drafts — no unknown, no refusal, no
   closure note — and 5,415 of 8,113 exports (67%) carry no unknown at all,
   with Solid Primitives at 88% of exports proven. The unknowns are dominated
   by one shape: a single all-five-domains-unknown summary attached to hundreds
   of export names at once (452 of `@kobalte/core`'s 610), produced where
   generation cannot join an anchor to one exported identity — a generator
   improvement, not a probing question. What remains unmeasured is
   *drivability*: the corpus's 4,199 positive behavioral rows (1,636 callback
   executions, 1,200 return trees, 990 reactive reads) are what Stage 1's probe
   reports will measure. If most of those convert to unknown, machine
   verification produces honest contracts that certify little, and the human
   tier remains the useful one for positives.
2. **Import-time side effects and environment-dependent packages.** Probing
   imports the package. A package that touches the filesystem, opens a socket,
   or registers a global on import does so during verification. A package
   requiring a DOM, or a server-only package requiring a request context, may
   be undrivable in some or all of the four modes — and "undrivable in mode X"
   must convert the claim, not silently narrow the stated modes, or the
   contract would claim semantics for an environment nobody observed.
3. **Probe flakiness and contract churn.** A flaky probe must fail closed, so
   the claim converts to unknown. But regeneration is idempotent and probing is
   not: a re-probe that passes flips the domain back to a claim, changing the
   contract's bytes. For a committed project-owned contract that is a spurious
   diff; for a human-reviewed contract carrying probed rows it makes every
   resolution stale and blocks `--transfer-from`. Options include requiring N
   consecutive passes, recording flakiness explicitly and refusing to promote,
   or pinning probe results per artifact so a re-probe is not re-run. Undecided.
4. **Sandboxing `contract probe`.** The recommendation to isolate is not an
   enforcement mechanism. Whether the command should refuse to run outside a
   detectable sandbox, ship a container recipe, or simply warn loudly — and
   what a CI integration is supposed to do — is open. Note that a project
   already executes its dependencies' install scripts and test suites, so the
   marginal risk is real but bounded; the argument for enforcement is that
   *verification* is a new reason to execute code that a consuming project may
   otherwise never run.
5. **Recording converted unknowns distinctly.** A domain the machine converted
   because it could not drive it is operationally different from one the
   generator never inferred: review tooling should be able to say "the machine
   believed `callbacks[0]=inline` here and could not confirm it", which is
   exactly the hint a human reviewer wants. Schema v1 cannot carry it, and it
   is blocked three times over. `$defs.evidence`, `$defs.claimEvidence`, and
   `$defs.unknownClaim` are all `additionalProperties: false`, and
   `$defs.unknownClaim` permits the single property `status`. The loader's
   unknown-field failure is the **malformed** path, which fails the analysis
   outright rather than refusing the contract and continuing, so a new field
   hard-fails older clients — the identical problem RFC 0001 §4 hit with
   `evidence.verifier`. And even setting the schema aside, `isUnknownClaim` in
   `packages/cli/scripts/contract-review-plan.mjs` tests
   `value?.status === "unknown" && Object.keys(value).length === 1`, so a
   sentinel carrying a `reason` would stop being recognized as a sentinel by
   the generator's merge, the review plan, and promotion alike. The proposal
   here is therefore to keep the record in `<contract>.probe.json` and out of
   the contract. Whether that is sufficient for review tooling, or whether this
   and `evidence.verifier` together justify a schema v2, is open and should be
   decided once rather than twice.
6. **Consumer-visible tiering.** Certification accepts `verified`, `reviewed`,
   `trusted`, and `attested` identically, and `contract check` reports a
   project-owned contract as `local` for all of them. A project that wants
   "certify only human-reviewed contracts", or a report that distinguishes
   mechanically verified coverage from reviewed coverage, has no way to ask.
   A `--min-evidence` selector on `--certify` and a `detail` field on the
   report are the obvious shapes; both are new consumer-side surface and belong
   in their own decision.
7. **Negative-claim trust.** Probes falsify but never verify a negative, so
   every omitted effect field rests on the generator's static soundness plus,
   at Stage 3, an attested closure. Stage 3 attests *which files were read*, not
   that the conclusions are right. Whether anything else can be done — a
   mutation-testing gate on the generator, a much larger torture corpus, a
   second independent implementation of the negative construction — is open,
   and the answer determines how much weight `verified` can honestly bear.
8. **Per-package `probeModes`.** A contract may legitimately state fewer than
   four modes, and today that is hand-declared on a dialect manifest entry.
   Nothing derives it for an arbitrary package, and a driver that assumes four
   would report failures against claims the contract never made. Deriving the
   stated modes from the entrypoint's export-map conditions is probably right
   and is not obviously complete.
9. **Probe-result caching and its identity.** §4's cost argues for caching
   probe results by `(package, version, integrity, generator identity, probe
   driver identity)`. That cache is either a local artifact or precisely what
   RFC 0001's registry should distribute for the machine-verified tier — and if
   it is the latter, the registry needs a probe-driver identity in its entries,
   which RFC 0001's entry shape does not currently carry.

## Staged adoption plan

**Stage 1 — `contract probe`, evidence only.** Implement the command: the
generic probe driver, argument-synthesis limits with an explicit undrivable
classification, per-entrypoint mode derivation, `probed` row evidence for
family-(B) claims, incompleteness reporting, and `<contract>.probe.json`.
**No promotion change**: contracts stay `inferred` and still certify nothing.
This stage is where the unmeasured question 1 gets answered, because the probe
report per package *is* the measurement of what is drivable. Nothing in Rust
changes. `contract review` is untouched.

**Stage 2 — mechanical `verified` promotion.** Add `--promote verified` with
the unknown-conversion rule and the five blockers of §3. Document the tier in
[package-contracts.md](../package-contracts.md), including that `contract
check` cannot distinguish it from `reviewed` (question 6) and that callback
`owner` rows are permanently out of reach. Record the residues in
[precision-backlog.md](../precision-backlog.md). Still no Rust change: the
loader already accepts `verified`, and `certification` already rejects inferred
rows inside a certifying document.

**Stage 3 — compiler-attested closure.** Extend the TypeFacts protocol so the
analyzing program returns the module list it actually resolved, and use it in
place of `runtime-module-closure.mjs`'s syntax walk. This is the only stage
that touches Rust and TypeFacts, and it **moves an upstream pin**: TypeFacts is
pinned by revision, so this follows [monorepo.md](../monorepo.md) — update the
pin and its notice, rebuild with `scripts/build-typefacts.sh`, and re-run the
process-test set. Until it lands, condition 4 of §2 blocks auto-verification
for every entrypoint with a closure note, and generator soundness is the trust
root for every negative claim. After it lands, the closure record becomes an
attestation rather than a reconstruction — and generator soundness is still the
trust root for the claims, which question 7 owns.

## Amendments

The RFC above is the design as proposed. Implementation found four places where
it was wrong or underspecified, and each was resolved *against* the text rather
than by bending the implementation to it. They are recorded here rather than
edited into the sections above, so that a reader can still see what was
proposed and what the proposal turned out to miss.

### A1. `kind` is family (B) with no sentinel, so it blocks

**The table says** `kind: function / value` is family (B), grounded in a
generic runtime read of `typeof value` per entrypoint leaf.

**What was missing** is what happens when that observation does not exist. §2's
condition 1 says every family-(B) claim is "probed or converted", and `kind` is
the one claim schema v1 cannot convert: the field is required on every export
summary, its two values are the whole vocabulary, and `$defs.unknownClaim` is
not among its permitted types. There is no weaker document to promote. The
implementation initially read that as an exemption, with the comment that a
disagreeing runtime kind is a failed probe and therefore blocks — which is true
and vacuous, because a probe that observed *nothing* disagrees with nothing. A
package whose entrypoint threw on import produced a report with zero passed
claims and verified anyway.

**The amendment:** a `kind` claim not probed-passed in every mode its export is
stated for is a **promotion blocker**. `kind` is therefore family (B) with a
third outcome, alongside "kept" and "converted": *blocks*.

The consequence is deliberate and is the point: **a package this checker cannot
import cannot be machine-verified at all.** Mechanical verification is an
observation of the installed artifact, and an artifact that will not load has
not been observed. `contract review` remains available and is where a human's
reading of such a package belongs.

### A2. `probed` markers must be witnessed by the run doing the verifying

**§2 condition 1 says** every family-(B) claim must "carry `probed` row
evidence covering every mode the claim is stated for".

**What was missing** is that a `probed` marker is a durable property of the
*document*, not of the run. It says nothing about which run wrote it. So
probe-healthy → probe-observes-nothing → verify certified every marker the
first run had left behind, and the report the promotion actually consumed
witnessed none of them.

**The amendment**, in two places:

- **At verification.** Every `probed` marker in the document must be witnessed
  by a passing claim of the same identity in the consumed report, covering at
  least the marker's modes. An unwitnessed marker converts its domain exactly
  as an unprobed row does, and the sidecar records it under
  `staleProbedMarkers`. Conversion rather than a blocker follows §2's own
  sentence — "every other positive claim it holds must become the unknown
  sentinel" — because from this run's point of view an unwitnessed marker and
  an absent one are the same state; and because blocking would make a
  legitimately narrow run (`--modes client`) unable to verify anything rather
  than able to verify less.
- **At `--write`.** §1's rule that an existing `reviewed` or `inherited-from`
  marker is never overwritten was only half a rule. A claim *this* run drove
  that did not pass now has its marker refreshed with what this run observed,
  or removed when this run observed nothing, and the report records each
  supersession. A claim this run did not attempt keeps what it had, and the
  verification-side check above is what stops that reaching the verified tier.

A related residue, from the same reading: a summary-level `probed` marker is
computed from the `callbacks[]` rows and the top-level `returns`, so it must be
recomputed whenever those claims are converted (here) or deleted (by a review
that certified them absent). Both paths do that now.

### A3. Family labels in the report must match what verification does

**The table says** `reactiveReads[]` is "A where compiler facts are exact;
otherwise C", and `ownerRequirements[]` is A.

**What was missing** is that the probe report labelled both **C**, while
`convertUnconfirmedClaims` treated them as **A** and kept them. Family (C)'s
definition in this document is "converted to the unknown sentinel for its
domain before promotion", so the report was telling a reader the opposite of
what the promotion did.

**The amendment:** the report labels them **A**, with a reason string that says
why no probe covers them rather than implying one should. Family (A) therefore
includes claims that are *undrivable and certified anyway*, which is what the
generator's fail-closed construction earns them. Callback `owner` rows,
callback argument descriptors, nested return leaves, `store-path` and
`argument` returns, and `asyncBehavior` remain (C).

### A4. Discovery is a precondition of certification, not an optimization

**§ "Negatives are not probeable" says** the discovery probe is "the only
automated check anywhere in the repository capable of contradicting a negative
claim", and §1 makes an incompleteness finding a promotion blocker.

**What was missing** is that `--no-discovery` turns that blocker into a
tautology: it lists zero findings because nothing looked, and the probe report
did not record that it had been skipped, so `<contract>.verify.json` reported
the incompleteness blocker as checked.

**The amendment:** the probe report records `discovery: {enabled, parameters}`,
and `contract verify` **refuses** a report whose discovery is disabled — or
which records no discovery state at all, since "it probably ran" is not an
observation. `--no-discovery` is an investigation flag; it cannot produce a
report anything certifies from. Discovery also runs for `value` summaries now,
which are the maximal negative claim and were exempting themselves from their
own falsifier.

### A5. `returns=accessor` needs a distinguisher a forwarding closure fails

**§ "What counts as observing the claim" says** the `inline`/`deferred`/
`tracked` shape is generic once the arguments exist, and treats the return-kind
claim as the same kind of observation.

**What was missing** is that the return observation plants its signal read
inside *the callback the contract states*, so a plain forwarding closure —
`(cb) => () => cb()` — re-reads that signal on every read of the returned value
and satisfies a reactivity-only test transitively.

**The amendment:** the observation also measures caching — how many times the
planted callback runs across two reads of the returned value inside one
evaluation of a single tracked scope. A memo accessor recomputes at most once;
a forwarding closure runs the callback per read and the claim is `undriven`,
never passed. An *uncached* derived accessor (1.x `mapArray`'s plain tracked
function) now lands undriven as well, which is the safe direction: unproven and
converted, rather than certified by a property a forwarding closure shares.

### A6. `evidence.calls` is a measurement

The worker stamped a per-probe-type constant, so a `deferred` claim recorded
`calls: 2` for a single invocation. It counts now. This is a correction, not a
design change, and is recorded here only because the number reaches the
contract.
