# ADR 0036: Census-gated withholding and synthesized vetoes

- Status: accepted and implemented (2026-09-05); written before implementation
- Date: 2026-09-05
- Owners: policy-2 certification planning and the probe-gate binding
- Relation: ADR 0006 Stage 3 ("a recipe corpus for real packages"), applied to
  the two censused domains of ADR 0008 and ADR 0035. It changes what happens
  to a *proposed closure candidate* the transaction cannot decide; it changes
  nothing about a claim the transaction contradicts, about receipts already
  issued, or about the controlled execution profiles.

## Context

A generated document proposes closures; the certifier withdraws each into a
candidate, and a candidate meets three gates in order: a **recipe** (the corpus
must address its exact claim id, else recipe gating withholds it by name and
re-plans with the domain open), the **implementation census** (the Type Facts
proof that the closure holds), and the **mandatory veto** (a finite runtime run
of the recipe that may contradict the proposal but never supports it).

Only the first gate withholds. The other two refuse the whole row:

- a census that cannot decide the candidate is an `UnsupportedDemand`, and the
  row's every other proven claim — value shapes, callbacks, reads, the sibling
  exports — is published nowhere. `@solid-primitives/i18n@2.2.1` is that row
  today: one `creates` candidate meets a `SpreadAssignment` accessor form the
  census refuses by name, and the row refuses;
- a veto run that does not complete is `IncompleteGate`, and the row refuses.
  `@kobalte/utils@0.9.2` is that row today: one hand recipe's gate did not
  complete, and 49 other exports' claims are published nowhere.

And every candidate that passes the first gate does so on a hand-written module
addressed by a content digest that moves whenever the document moves.
`@kobalte/utils@2.0.0-alpha.0` certifies with 11 `creates` candidates withheld
for want of a recipe, and the `returns` domain of ADR 0035 doubles that queue
on every row where it applies.

The result is that the certifier's most expensive knowledge — a census that
proved a closure — reaches a receipt only where a person wrote a module, and
one undecidable candidate costs a row everything it did prove.

## Decision

### 1. A candidate the census cannot decide is withheld, not refused

When Type Facts verification refuses a `DomainExhaustiveness` demand whose
subject is a proposed closure candidate with `UnsupportedDemand`, the certifier
withdraws **that candidate** by name — a `WithheldClosure` whose `reason` is
`census refused: ` followed by the census's own refusal text — re-derives the
plan through the same `withheld_weakening` recipe gating uses, and acquires
again against the re-derived plan. The loop is bounded by the number of
candidates and terminates because each pass withdraws at least one. Any other
Type Facts error keeps refusing the row.

Why this is honest: an undecided closure is an **open domain**, and an open
domain is exactly what the weakened document publishes. Refusing the row
published a *stronger* negative — nothing at all about the export — on the
strength of a proof the certifier merely lacked. The withheld record names the
candidate and the premise it lacked, so the outcome is visible, not silent, and
the ranking scripts that read `withheldClosures` now rank census blockers
beside recipe blockers.

What does not change: ADR 0008's rule that the generator's walk must not
propose a shape the census refuses stands as a **design obligation** — a
withheld candidate is a wasted gate and a misleading proposal, and the fixture
pairs that pin walk–census agreement stay. It is no longer the row's fate.

### 2. A veto that does not complete withholds; a contradiction refuses

A gate whose runs end in `ErrorOrTimeout` withdraws its candidate the same way
(`reason: veto did not complete: ` and the gate id), re-plans, and runs the
remaining schedule again. A finite run that could not be driven to completion
observed nothing, and "observed nothing" is the open domain.

A **contradiction never withholds.** A recipe that provokes its falsification
marker has observed the package doing what the proposal denies; the document
proposed something false, and the row refuses exactly as before. The
asymmetry is the point: refusal is reserved for evidence *against* a claim,
withholding for the absence of evidence *for* one.

### 3. Vetoes are synthesized where no hand recipe exists

For a proposable candidate the supplied corpus does not address, the certifier
synthesizes a recipe module of its own and gates the candidate on it. The
synthesis reads the export's Type Facts call signature — arity, each
parameter's primitive domain, literal partitions, and callability — and the
export name, and nothing else; the module is deterministic in those inputs.

A synthesized module does what the checked hand recipes do, and states what it
observes:

- it imports the export from the package under test, marks the call window
  with the `call` enter/exit events, and calls the export on a finite sample
  drawn from the signature facts — a representative value per primitive
  domain, each literal partition case, a plain callable for a callable slot
  that records only whether it was invoked, `undefined` for an optional slot;
- every call is wrapped, and a call that throws is recorded as
  `synthesized veto: N of M sample calls threw`, a coverage limitation on the
  material rather than a failure of the run;
- for `returns: []` it emits the falsification marker when any call's result is
  not `undefined` — an exact contradiction of the claim;
- for `creates: []` it emits the marker when the call window adds an own
  property to `globalThis`, the convention every checked hand recipe for that
  domain already uses. This is **not an exact observation** of a `create`
  operation, and the module's coverage limitation says so. The exact
  observation — a dialect primitive whose `creates` the audited negative table
  does not deny, invoked during the window, seen through the worker's existing
  loader hook over the authenticated Solid copies in the workspace — is the
  follow-up this ADR names and does not take.

Provenance. Synthesized modules are written into a private corpus directory of
the transaction and loaded through the same `RecipeCorpus` path as hand
recipes: Rust derives the construction digest from the bytes it copied, the
corpus root binds them into the plan, and the manifest entry states
`provenance: synthesized`. A hand recipe always wins for a claim id it
addresses. The synthesized corpus never lives inside the analyzed package and
never hands `session` or `harness` to the package: the recording callable
closes over a counter, not over the harness.

Ordering. Synthesis needs the signature facts, which arrive with Type Facts
acquisition, and acquisition is bound to a gated plan. The transaction
therefore runs: gate on the hand corpus → acquire and census (with the
withholding loop of § 1) → synthesize for every still-withheld candidate whose
export-value transcript carries a call signature → re-gate the original plan on
the merged corpus → acquire and census again → run the gates (with § 2) →
finalize. A row with no recipe-less candidate acquires once, as today.

### What a synthesized veto is not

It is not evidence for the closure — nothing ever is; the census is. It is not
a substitute for a hand recipe where one exists. It does not run under any
controlled execution profile: those select one hand recipe by claim id and are
unchanged. And it is not a guarantee of coverage: a package whose export cannot
be driven with a synthesized sample is recorded as such and its candidate is
withheld under § 2.

## Implementation (2026-09-05)

- `certify_value_only` is a bounded loop: gate on the hand corpus, acquire and
  census (a census refusal maps through `census_refusal_withholding` to a
  withheld record and a re-plan), synthesize once for the still-withheld
  candidates with a call signature (`synthesized_vetoes.rs`), re-gate on the
  merged corpus, run the gates (an incomplete gate or a per-session worker
  timeout maps through `incomplete_gate_withholding`), finalize. The case-set
  variant keeps its batch and hands any plan that needs a second pass to the
  per-plan loop.
- `ProbeHarnessError::SessionTimeout { claim_id }` names the one session whose
  worker did not report; `WithheldClosure.reason` gains the two prefixes; the
  recipe manifest gains the optional `provenance` field; `VerifiedTypeFactsEvidence`
  retains each export's unique call signature for the synthesizer.
- The census fixture's partition under synthesized vetoes is pinned in
  `the_census_certifies_a_generated_creates_candidate_and_withholds_its_siblings`;
  every census-refusal tracer test asserts the withholding and its reason.

Measured (`docs/2026-09-05-census-gated-withholding-and-synthesized-vetoes.md`):
the three real rows all certify — two of them refused before — and the alpha
row's eleven queued candidates all close through synthesized vetoes.

## Alternatives considered

- **Keep refusing the row on a census refusal.** This is what kept the walk and
  the census aligned by force, and it cost `@solid-primitives/i18n` its whole
  row over one accessor form. Alignment stays an obligation; the row's fate is
  no longer the enforcement mechanism.
- **Withhold on contradiction too.** Rejected: a contradiction is the only
  runtime evidence the transaction ever has *against* a document, and a
  generator that proposed a false closure must not be published for that row.
- **Skip the veto for synthesized-free candidates.** Rejected in ADR 0035 and
  again here: a closure that never met a runtime would be the first of its
  kind.
- **Synthesize from the declaration shape alone.** The proposal's
  `ValueShape::Callable` carries no arity or parameter facts, so the only
  sample available would be a blind cross product. The Type Facts signature
  costs a second acquisition on rows that need it and buys a relevant sample.
- **Blind sampling without facts.** A large cross product of generic values
  per arity is finite but mostly noise, and its throw rate would make the
  coverage limitation the main content of the material. Not taken.

## Consequences

- **Yield.** Rows that refuse today on one undecidable candidate or one
  incomplete gate certify with that candidate withheld by name; the two real
  rows named above are the first. Candidates with no hand recipe are censused
  and vetoed instead of queued; the alpha row's 11 become closures or named
  withholdings. `exportsProven` does not move — seven domains stay open on
  every real export.
- **Planning.** `WithheldClosure.reason` gains two values beside
  `no recipe in corpus`. `certify_value_only` and the case-set variant run the
  loops of § 1–§ 2 and the two-phase acquisition of § 3.
- **Corpus.** The recipe manifest gains an optional `provenance` field; absent
  means hand-authored, as every existing manifest is.
- **Fixtures.** `implementation-census-creates` pins that a census-refused
  export now certifies its row with the candidate withheld and named
  (`the_probe_gate_tracer_census_refuses_*` assertions move from "the row
  refuses" to "the candidate is withheld with the census's reason"), that a
  contradiction still refuses, and that an export with no hand recipe certifies
  through a synthesized veto. `implementation-census-returns` pins the exact
  `returns` observation both ways.
- **Not changed.** Receipt identity for rows that certify today without any
  withheld candidate; every controlled execution profile; the census itself.
