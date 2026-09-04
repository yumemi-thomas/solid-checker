---
status: accepted
---

# A negative dialect authority about a *callee*

Date: 2026-09-03

## The gap

An implementation census of a behavioral call domain has to *terminate*.
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md` § 3.1
lists five terminators, and for a call to a Solid primitive the terminator is
the second one: "a dialect primitive under an integrity-bound negative table".
The fact it needs is negative and exact —

> this primitive publishes no operation of kind X

— and nothing accepted anywhere in this repository carried it. Both bundle
indexes have `"contracts": []`, `EMBEDDED_SOLID1_BUNDLES` is `&[]`, and both
first-party bundle producers validate their inputs and then return an empty
vector (`first_party_bundles.rs:289`, `:369`), so the audited documents reach
no analysis. They are retired proposals sitting in `pkg/contracts/bundled/**`,
compiled in and unread.

Without that fact every consumer that calls `createEffect` refuses `creates`,
which is every interesting consumer.

## Decision

Add the fact as an **explicit, named, integrity-bound tier**, in two halves.

**A negative table in `solid-dialect`.** Per dialect, a
`DialectNegativeAuthority` listing the exact archives that dialect audited —
name, version, SRI, and the archive's own `package.json` digest — and, per
canonical primitive and per call claim domain, the rows those audits *deny*.
Every row cites exactly which audited bytes it was read from, and a test
re-establishes that citation. `primitive_performs_no_operation` unions the
dialects with cross-dialect agreement; silence is never "no".

**Two citation kinds, and they are not equally mechanical.** This was one kind
when the ADR was accepted, and the amendment below added the second; the
difference is load-bearing and stating it plainly is part of the decision.

- `AuditedCitation::Summary` names a byte range of a normalized contract
  document. The cited range **is the claim**: it parses as the summary object
  carrying the domain's empty collection and the `closed` list that closes it,
  so the test re-reads the range and **re-derives the closure itself**. No human
  judgement sits between the bytes and the row, and a row whose citation moved,
  whose document changed, or whose domain is no longer closed there fails in
  that test.
- `AuditedCitation::Implementation` names a byte range of the archive's **own
  runtime bytes**, read by hand in a repository audit document. Here the cited
  range is *the definition a human read*, and the closure was that reading's
  conclusion — **not** the range's content. Nothing re-derives "this body
  performs no `create`" from a JavaScript function body, and the test does not
  pretend to: what its digests pin is the **subject** of the review — that the
  row still cites the same bytes of the same published file that the audit's
  named section walked — while the reasoning stays reviewable in the audit
  document and nowhere else. Concretely it checks, unconditionally, that the
  audit file contains the cited section heading verbatim; that
  `(archive_path, file_sha256)` matches the pinned entry in
  `benchmarks/package-contract-v2/phase0/rc3/*/files.json` and the range fits
  inside that file's pinned length; and that a checked-in copy of the range
  under `rust/crates/solid-dialect/audited-slices/` hashes to the row's
  `slice_sha256` and begins with the export's own definition. When
  `SOLID_CHECKER_RC3_ARCHIVE_ROOT` names an install it additionally re-reads the
  real archive and asserts the checked-in slice is still exactly
  `bytes[start..end]` of it — the one thing a checked-in copy cannot establish
  about itself. That arm may skip; it may not skip *silently*, so it fails under
  `SOLID_CHECKER_EXPECT_PROBE_PINS=1`, and both `scripts/verify.sh` and
  `make test-rust` export the root from the tsc-oracle install.

This kind exists because a row can be needed where no audited summary is
available: `solidjs-signals.json` audits twelve exports, and `createRoot`,
`createSignal`, `getOwner`, `onCleanup` and `untrack` — the five ADR 0008 names
as the actual next blocker on real consumer rows — are not among them.
Inventing a summary id, or adding a summary to a frozen audited document, would
be a fabricated authority; citing the bytes a human actually read, and saying
that is what happened, is not.

**A tier in the certifier.** `census_dialect_axiom_for_callee`
(`contract_certification/type_facts.rs`) is a peer of
`argument_slot_is_proven_invoking`'s Tier A: same position in the tier list,
same membership-reviewed discipline, same refusal of anything unlisted. It
answers only when the callee's *resolved declaration* strips to an
authenticated dependency snapshot source root and is a member of that archive,
the archive equals an audited tuple in all four fields, the resolved export is a
canonical primitive, and the table denies the domain. Its witness is
`census-dialect-axiom:<pkg>@<ver>#<sri-prefix>:<export>:<domain>`.

## Why the callee side is sound where the self side is not

ADR 0005 defers the *positive* axiom — a dialect discharging an owner-requirement
demand about **its own** package's export. Its five objections were worked in
order and the fifth resolved against the premise. That fifth objection is the
one that matters here, and it does not transfer:

> The demand's evidence is the dialect's own vocabulary, reached through a
> filesystem path. […] There is no artifact evidence anywhere in that loop that
> the axiom would not itself be supplying.

The circle there is closed by *identity of subject*: the demand was about
`@solidjs/signals`' `onSettled`, it was derived from `solid_2.rs`'s name
vocabulary via `declaration_path_is_solid_package`, and the axiom would have
answered it from `solid_2.rs`'s rows about the same names. Premise and
conclusion were two spellings of one derivation.

A census terminator has a different subject. The claim under certification is
about **another package's** export — a consumer's `useDebounced`, say — and the
census question is "does any resolved target of this consumer's implementation
publish a `create`". The dialect answers about `createEffect`, which is not the
export under certification, was not the source of the consumer's demand, and
contributes no part of the consumer's proposal. The consumer's proposal closes
`creates` because *its own* transcript is complete and *its own* calls resolve;
the dialect only says that one of those resolved callees is not a counterexample.
Nothing the axiom supplies is also something it consumes.

Two further asymmetries make the same point mechanically:

- **The demand's origin is elsewhere.** ADR 0005's `onSettled` demand existed
  only because `find_missing_owners` granted primitive identity by filesystem
  path inside the defining archive. A consumer's `creates` closure candidate
  comes from `inventory_value_shape` over the consumer's own proposal; no
  dialect row participates in creating it.
- **The audited authority agrees rather than contradicting.** ADR 0005's
  disqualifying second finding was that the audited contract for the byte-identical
  bytes closes `creates: []` for `onSettled`, so the axiom would have
  discharged a demand the audit says does not exist. Here the audit's closure
  *is* the answer, restated — the tier asserts exactly what its audit asserts,
  about exactly the bytes that audit was read from. For a `Summary` row the
  audit is a normalized contract document and the restatement is mechanical;
  for an `Implementation` row it is a hand census recorded in a repository audit
  document, and the restatement is of a human's conclusion. Either way the tier
  adds no claim of its own, which is the property this paragraph is about.

## Why an audited archive may not answer about another audited archive

Root equality (`snapshot.root() == certified.root()` or matching
`provenance_root()`) is not the whole of the self-restriction. It refuses a
callee that resolves back into the artifact under certification's own
snapshot, but `solid-js`, `@solidjs/signals`, and `@solidjs/web` are three
separate npm packages that this dialect nonetheless treats as one unit for
self-axiom purposes — they are exactly `primitive_defining_package`'s three
names. Certifying `solid-js@2.0.0-rc.3` with a callee resolving into
`@solidjs/signals@2.0.0-rc.3` passes root equality cleanly: two different
snapshots, two different roots, both independently audited. It is also the
live phase-21 self-certification configuration, and it is exactly the
configuration ADR 0005 objection 5 is about — the demand and the discharge
would again derive from the same dialect rows, only spread across a package
boundary this dialect does not treat as meaningful for that question.

The distinction "Why the callee side is sound" above rests on is *subject*:
the claim under certification must be about a package the dialect's rows
merely observe from outside, contributing nothing the demand itself is made
of. That distinction collapses once the artifact under certification is
itself one of the dialect-defining archives. `census_dialect_axiom_for_callee`
therefore refuses outright a second way: `audited_archive_for_snapshot(certified)`
succeeding refuses the terminator regardless of which archive the callee
resolves into. This is a **consumer-side** restriction — it says nothing about
whether `solid-js`, `@solidjs/signals`, or `@solidjs/web` can certify at all,
only that this negative table may not be the thing that does it. Inside the
dialect-defining archives the runtime *is* the definition these rows were read
from; deciding a claim there from the same rows is circular in the way ADR
0005 objection 5 names, however many package boundaries sit between the
export under certification and the callee. Only a proof mode that traces the
archive's own actual execution — rather than restating this dialect's
vocabulary about itself — could decide a domain closure inside that set, and
no such mode is wired to this tier.

## What this is not

- **Not a positive authority.** No row can establish that an operation exists,
  and there is no shape of input that makes the table assert one. A refusal is
  a `None`, which makes the census refuse the domain by name.
- **Not a way to close a domain on the audited package's own certification.**
  `primitive_performs_no_operation`'s doc comment says so, and
  `census_dialect_axiom_for_callee` refuses outright when the callee's archive
  is the archive under certification (equal snapshot root or provenance root),
  **and separately** when the artifact under certification is itself an
  audited archive of any dialect at all, whatever the callee resolves into.
  Root equality alone would let `solid-js@2.0.0-rc.3` certify with a callee
  resolving into `@solidjs/signals@2.0.0-rc.3` — two different snapshots, two
  different roots, both audited — which is the live phase-21
  self-certification configuration ADR 0005 objection 5 is about; see "Why an
  audited archive may not answer about another audited archive" below. ADR
  0005 stays `deferred`.
- **Not a name-only premise.** `target_module` is the written import specifier
  and is never consulted. `primitive_defining_package` — the generation-scope
  predicate that compares a bare name — stays one-directional withholding, as
  ADR 0005's later section records.
- **Wired into exactly one census.** Since ADR 0008 the `creates`
  implementation census consults this tier as its `dialect-axiom` disposition,
  after the parameter-rooted and standard-library dispositions and before local
  recursion; `require_census_decides_closure` still refuses every other
  behavioral call domain by name.

## Preconditions from ADR 0005, and how each is met

| ADR 0005 | Status here |
| --- | --- |
| 1. Integrity-bound tuple, naming the disagreeing field | `AuditedArchive` carries name, version, SRI and manifest digest; `audited_archive_for_snapshot` compares all four and returns `AuditedArchiveDisagreement::{Name, Version, Integrity, Manifest}`. The manifest digest is re-derived from the authenticated snapshot's own `package.json`, never read from a resolver's report. |
| 2. Version keyed to the actually-audited bytes | A `Summary` row cites the document whose `package` block *is* the tuple, and a test asserts the two agree field by field. An `Implementation` row cites a file of the same archive by its digest in `benchmarks/package-contract-v2/phase0/rc3/*/files.json`, whose sibling `registry-metadata.json` `dist.integrity` is the tuple's own `integrity`. There is no second version anywhere for a row to be keyed against. |
| 3. `floor == MayExecute` only, floor in scope | `floor` is a parameter of the tier and the first gate; `ReachabilityFloor::Reachable` refuses, and the refusal is pinned. |
| 4. A corpus fixture exercising `from_plan` | **Not taken, and not needed for this tier.** That precondition existed because the *positive* axiom's identity derivation was unreachable from a unit test, `CertificationPlan` being unconstructible here. This tier takes the archive under certification as an `ArtifactSnapshot` rather than a plan, so the whole identity gate — including the integrity and manifest halves the corpus's fabricated `fixture:sha256:` would fail — is exercised by unit tests against the checked-in audited `package.json` bytes. A corpus fixture is still owed once the census *consumes* the terminator, because only then does a row's verdict move. |
| 5. The generator's evidence for the owner claim named | Not applicable: this tier discharges no owner requirement and answers no demand about the audited package's own exports. |

## Scope shipped, and what was deliberately withheld

Only `CallClaimDomain::Creates`, from the six hand-audited Solid 2.0 documents
(four of which carry a row) and, since 2026-09-04, from one hand implementation
census over `@solidjs/signals@2.0.0-rc.3`'s own runtime bytes
(`docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md`;
signed off by delegation, 2026-09-04). `solid-dialect`'s `NEGATIVE_ROWS`
carries the reason for every withholding; these are worth restating because they
are findings against the audits rather than gaps in the table:

- **`(solid-js, createEffect, Creates)` was withdrawn on 2026-09-04.** This is
  the one shipped row the § creates decision cost, and it is a defect in the
  audited document rather than in the table. `solid-js.json` closes
  `creates: []` for `createEffect`, but that document captures the
  `browser/development` case only: `solid-js/dist/server.js:868-870` routes
  `createEffect` to `serverEffect`, which calls `processResult` — and so
  `ctx.serialize(id, deferred.promise, deferStream)` — whenever the caller
  passes `options.ssrSource`, and `semantic-model.md` § creates'
  **[Decision 2026-09-04]** settles that that registration **is** a `create`.
  The reach is guarded (`node`/`worker`/`deno` ∧ an `async` render context ∧
  `ssrSource ∈ {server, hybrid}` ∧ a thenable or async-iterable compute result ∧
  an owner with an id ∧ no `NoHydrate` ancestor) and it is reachable
  *type-correctly*: `solid-js/types/client/hydration.d.ts:42` augments
  `EffectOptions` with `ssrSource` and `:568` re-declares the export carrying
  it, so no consumer needs a cast to get there. A `(package, export, domain)`
  row has nowhere to put the guard, so it goes to the withheld list rather than
  being qualified. `(solid-js, createSignal, Creates)` is withheld for the same
  reason and was never shipped. See "Open: the table is not guard-aware" below.
- **`hydrate`'s `creates` row is withheld although the audit closes it.**
  `@solidjs/web@2.0.0-rc.3`'s `hydrate` reaches `render` on every path, and
  `render` calls `registerDelegatedRoot(element)` unconditionally before it
  opens its root — the exact act the sibling `render` summary in the same
  document models as the `create` operation `register-delegation`. The audit contradicts itself about two
  functions in one document and the published bytes side with `render`.
- **The `returns` domain is withheld wholesale.** `snapshot`'s summary is
  `shape: "plain"` — it hands its caller a value — and closes `returns: []`;
  `flush` and `latest` do the same. `semantic-model.md` § returns defines a
  `return` operation as "the export yielding a value to its caller", so the
  audits are using `returns` for emission-like operations and recording the
  ordinary synchronous return in `shape`. Until that is reconciled the domain's
  closures deny nothing this table can restate.
- **The whole Solid 1.x table is empty.** The nineteen
  `pkg/contracts/bundled/solid-v1/*.json` documents close `creates: []` for all
  129 exports, and that closure is not an audit: the hand-audited 1.9.14
  contract before commit `474c101f` was schema 1, which had **no claim domains
  at all** — `kind`, an optional `returns` shape, and `callbacks`. The closure
  was introduced by the migration over a domain the audit never examined, which
  is the negative claim manufactured from missing knowledge ADR 0005 names as
  inadmissible.
- **Four of the rows are unreachable through the natural import path.**
  `solid-js@2.0.0-rc.3`'s `types/index.d.ts` re-exports `affects`, `isPending`,
  `latest`, and `refresh` from `@solidjs/signals` rather than declaring them —
  its own line 1 is `export { …, affects, …, isPending, …, latest, …, refresh,
  … } from "@solidjs/signals";`. A consumer that writes `import { latest }
  from "solid-js"` has its declaration resolve into `@solidjs/signals`'s own
  tree, so the tier's identity gates bind `@solidjs/signals` as the archive and
  ask *its* table for the row — and
  `solidjs-signals.json` audits twelve exports, none of them these four.
  `solid-js.json`'s rows for them are real and correctly audited; they are
  simply not the archive this import path's declaration resolves into. Open
  until `@solidjs/signals` is audited for these four names directly, or the
  tier is changed to consult the written entrypoint's own binding instead of
  the resolved declaration's archive — see `docs/precision-backlog.md`'s
  "Four rows dead via cross-archive re-export".

### Open: the table is not guard-aware

A row is keyed by `(package, export, domain)` and nothing else, so it asserts
the denial for **every** guard and **every** artifact case of the archive. A
normalized contract document does not have this limitation: a closure is over
one artifact case under one guard, so a document can close `creates: []` for
`solid-js`'s browser case and publish the operation for its server case. The
table flattens exactly that distinction away.

That is why `(solid-js, createEffect)` had to be *withdrawn* rather than
narrowed, and why `(solid-js, createSignal)` cannot be granted on an audit that
found its browser bodies clean. Both are single exports whose reach depends on
the resolved runtime condition and on an option the caller passes, and the row
can say neither. **Not attempted here**: making the row carry a condition or
guard predicate is a change to the row shape, to `denies`, to
`primitive_performs_no_operation`'s cross-dialect agreement, and to
`census_dialect_axiom_for_callee`'s identity gate — which today binds the
archive and would have to bind the *condition* as well, meaning the certifier
would have to know which `exports` condition the consumer's own build resolves.
It is the same gap as the `browser/production` per-condition approximation
above, reached from the other direction, and it is recorded in
`docs/precision-backlog.md`.

Until it is closed, the rule for this table is the blunt one: **any guarded
reach to the domain's operation withholds the row.**

## Consequences

- A `creates` census terminates on 28 Solid 2.0 primitive callees (ADR 0008
  consumed the terminator): the 23 remaining document-derived rows plus the five
  the 2026-09-04 implementation census closed — `@solidjs/signals`'
  `createRoot`, `createSignal`, `getOwner`, `onCleanup` and `untrack`, which
  ADR 0008 named as the actual next blocker. Those five are the primitives real
  consumers call most, so a consumer census that previously fell silent on the
  first `createSignal` can now terminate; expect proposals that were refused for
  an unaudited primitive to become provable.
- One row was lost in the same change: `(solid-js, createEffect)`. A consumer
  calling `createEffect` from `solid-js` now refuses its `creates` census where
  it previously terminated, which is a real regression in reach and the correct
  answer — the previous termination rested on a document that had read one
  condition of four.
- A 1.x consumer's `creates` census cannot terminate on any Solid callee at all
  until a real 1.x `creates` audit exists. That is a refusal, not a defect.
- Four audit-review items are now open against the audited documents rather
  than against the verifier, recorded in `docs/precision-backlog.md`: `hydrate`'s
  withheld `creates` row (`@solidjs/web`'s audit contradicts itself about
  `hydrate` and `render` in one document), the `returns` closures on `flush`,
  `latest`, and `snapshot` (each hands a value to its caller, which
  `semantic-model.md` § returns' settled decision makes a `return` operation,
  against the closed `returns: []` these three carry), the `callbacks`
  closure on `latest` (`latest(fn)` calls `fn()` directly, an `invoke` by
  § callbacks, against the closed `callbacks: []` it carries), and
  `solid-js.json`'s `creates: []` on `createEffect`, which its own archive's
  server condition contradicts (the withdrawal above).
- The five implementation-audited rows carry one approximation of their own,
  distinct from the `browser/production` one: they are keyed to
  `@solidjs/signals` because that is where the declaration lives, while a
  consumer importing those names from `solid-js` under the `node` condition
  executes `solid-js/dist/server.js`'s own bodies. The audit read those bodies
  too and reached the same verdict, which is what makes the rows sound on that
  path — but the tier binds the declaration's archive and cannot see the split,
  so the rows rest on the audit having read both. It is the mirror image of the
  "Four rows dead via cross-archive re-export" item, and belongs beside it.
