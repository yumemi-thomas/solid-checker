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
Every row cites the document and the byte range of the summary object it was
read from, and a test re-reads that range and re-derives the closure, so a row
cannot drift from the bytes. `primitive_performs_no_operation` unions the
dialects with cross-dialect agreement; silence is never "no".

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
  *is* the answer, restated — the tier asserts exactly what the audited
  document asserts, about exactly the bytes it was read from.

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
| 2. Version keyed to the actually-audited bytes | Each row cites the document whose `package` block *is* the tuple, and a test asserts the two agree field by field. There is no second version anywhere for a row to be keyed against. |
| 3. `floor == MayExecute` only, floor in scope | `floor` is a parameter of the tier and the first gate; `ReachabilityFloor::Reachable` refuses, and the refusal is pinned. |
| 4. A corpus fixture exercising `from_plan` | **Not taken, and not needed for this tier.** That precondition existed because the *positive* axiom's identity derivation was unreachable from a unit test, `CertificationPlan` being unconstructible here. This tier takes the archive under certification as an `ArtifactSnapshot` rather than a plan, so the whole identity gate — including the integrity and manifest halves the corpus's fabricated `fixture:sha256:` would fail — is exercised by unit tests against the checked-in audited `package.json` bytes. A corpus fixture is still owed once the census *consumes* the terminator, because only then does a row's verdict move. |
| 5. The generator's evidence for the owner claim named | Not applicable: this tier discharges no owner requirement and answers no demand about the audited package's own exports. |

## Scope shipped, and what was deliberately withheld

Only `CallClaimDomain::Creates`, and only from the six hand-audited Solid 2.0
documents (four of which carry a row). `solid-dialect`'s `NEGATIVE_ROWS` carries the reason for every
withholding; three are worth restating because they are findings against the
audits rather than gaps in the table:

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

## Consequences

- A `creates` census terminates on 24 Solid 2.0 primitive callees (ADR 0008
  consumed the terminator). The primitives with no row — `createSignal`,
  `onCleanup`, `untrack`, `getOwner`, `createRoot` among them — are where the
  generator's proposal walk now falls silent, so most real consumers propose no
  `creates` closure at all until those audits carry a row.
- A 1.x consumer's `creates` census cannot terminate on any Solid callee at all
  until a real 1.x `creates` audit exists. That is a refusal, not a defect.
- Three audit-review items are now open against the audited documents rather
  than against the verifier, recorded in `docs/precision-backlog.md`: `hydrate`'s
  withheld `creates` row (`@solidjs/web`'s audit contradicts itself about
  `hydrate` and `render` in one document), the `returns` closures on `flush`,
  `latest`, and `snapshot` (each hands a value to its caller, which
  `semantic-model.md` § returns' settled decision makes a `return` operation,
  against the closed `returns: []` these three carry), and the `callbacks`
  closure on `latest` (`latest(fn)` calls `fn()` directly, an `invoke` by
  § callbacks, against the closed `callbacks: []` it carries).
