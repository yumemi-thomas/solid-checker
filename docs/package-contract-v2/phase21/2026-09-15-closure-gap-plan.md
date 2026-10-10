# Plan: why a correct contract still certifies nothing, and what can move

Status: **steps 1 and 2 implemented, 2026-09-15**; step 3 not started. Follows
§ 24 of `2026-09-14-which-closures-change-a-consumer-finding.md` and the
2026-09-15 re-export entry in `docs/precision-backlog.md`. See § 6 for what
landed, where it differs from this plan, and what is still unverified.

Baseline: `kobalte/packages/core` against `@kobalte/utils@0.9.2`, one release
binary, `--runtime-condition import`: 1037 open-claims findings. Of those, 427
are on the nine cross-package re-exports:

| export | callbacks | reads/returns/owner mix | source package | source closes |
| --- | ---: | ---: | --- | --- |
| `mergeRefs` | 153 | 82 | `@solid-primitives/refs@1.1.4` | `creates` only |
| `access` | 152 | 33 | `@solid-primitives/utils@6.4.1` | `creates` only |
| `accessWith` | 1 | 1 | `@solid-primitives/utils@6.4.1` | `callbacks`, `creates` |
| `createEventListener` | 2 | 1 | `@solid-primitives/event-listener` | nothing |
| `Key`, `createMediaQuery` | 0 | 1 + 1 | keyed, media | nothing |

The other 610 (`callHandler` 196, `mergeDefaultProps` 127,
`createGenerateId` 60, tail) are `@kobalte/utils`'s own exports and are not
this issue.

## 1. Level A verdict: the projection keeps closures; re-emission discards them

**The projection does not drop closures.** `project_accepted_export`
(`rust/crates/solid-reactive-ir/src/contracts.rs:53`) builds each domain with
`project_callbacks` / `project_reactive_reads` / `project_return` /
`project_owner_requirements`, and every one of them inserts the domain into
`open_claims` only when `!knowledge.is_closed()`:

```rust
let knowledge = export.callbacks();
if !knowledge.is_closed() {
    open.insert(ClaimDomain::Callbacks);
}
```

A closed domain therefore projects as `ContractClaim::Known(items)` with the
domain *absent* from `open_claims`, which is exactly the consumer-side spelling
of "closed": `unknown_contract_callback_export` (`indexes.rs:468`) and
`push_unknown_contract_claims` (`contracts.rs:942`) both test
`is_open() || open_claims.contains(domain)`. `creates` is carried a second
time as `creates_closed_empty`. So a consumer analysing against an accepted
catalog sees the dependency's closures intact. The deliberate narrowings in
that function are:

- the value-export arm (`contracts.rs:64-99`): *closes* vacuous call domains on
  a proven non-callable value, guarded by `shape_may_be_callable`. That is a
  widening for values, not a loss, and it is load-bearing (it is what keeps a
  composed proposal from refusing with "value export cannot have function
  effects");
- `composed_owner: None, composed_from: None` in `project_reactive_reads`
  (`contracts.rs:230-236`): drops intra-package composition provenance because a
  cross-package composition has no premise. Deliberate and correct;
- `project_owner_requirements` keeping only obligation-imposing operations,
  which is why `creates_closed_empty` exists (`lib.rs:1026` documents it).

**The loss is at re-emission.** `contract_exports_for_entry_file`
(`solid-facts-backend/src/main.rs:6041`) now takes the projected summary first,
then hands it to `promote_entry_callable`, `attach_generated_owner_requirements`
and eventually `normalize_export` (`inferred_contract.rs`) — the *same* path a
locally inferred summary takes. That path:

1. maps `Known(items)` to `KnowledgeSet::Complete(items)`;
2. `open_proposed_closure` weakens every `Complete` to partial (the
   proposal-generator boundary, `contract_semantics.rs:690`: "cannot publish
   any of them as accepted closure");
3. re-proposes closure only for domains passing the **local** proposal filters:
   `creates` needs `summary.creates_walk_clean`, `returns` needs
   `returns_walk_clean`, a described `callbacks` needs
   `direct_callback_parameters` (`callbacks_enumeration_is_confirmable`), while
   the *empty* `callbacks` and `reads` enumerations pass vacuously.

`attach_generated_owner_requirements` (`main.rs:7372-7385`) sets those walk
flags from the export's *local* symbol. A cross-package re-export has no local
symbol, so both flags are `false` — "silence is do not propose" — and the
dependency's certified `creates: []` never reaches the document. That is why
`access` and `mergeRefs` publish no `closed` array at all.

The vacuous filters expose the other half. The post-fix audit
(`$SP/p2-ku-fix/audit1.json`) shows `@kobalte/utils`'s node **did** propose
closures for `Key` (callbacks), `chain` (callbacks), `access` and `accessWith`
(reads), and the certifier refused each one:

> census refused: creates census refuses a transcript at depth 0 whose
> declaration "Key" at …/@solid-primitives/keyed/dist/index.js:4390..4393 is
> not in this artifact's own runtime source: the census walks authenticated
> runtime bytes and nothing else

That is `census_transcript_frame` (`type_facts.rs`, ~11692), and it is right:
the parent's census must never walk another archive's bytes. So today's
propagation is inconsistent — accidental for empty enumerations, absent for
`creates`/`returns`/described callbacks — and in every case unprovable at the
parent, because the parent has no proof mode for a re-export.

**Verdict: defect, in two parts.** (a) A projected summary is normalized as if
it were a local inference, so closures are dropped or re-proposed by the wrong
filters. (b) No certifier path can discharge a closure on a re-exported name.
The refusal itself is not the bug.

**Correct propagation rule.** A re-exported name's domain is closed at the
parent exactly when the dependency's contract, for the exact bound
`(semantic identity, export identity)` the parent's `verified_exports` already
records as receipt evidence (`contract_certification.rs:1160`), closes it with
items equal to the projection. The proof is **composition from the
dependency's receipt**, never the parent's census. The two dependency kinds
differ only in when that receipt exists:

- *Receipt-accepted dependency* (a pass-2 catalog from pass 1, any
  `--accepted-contracts` catalog): the closure is certified already. The parent
  still publishes it as a `proposedClosures` entry with provenance, because
  every emitted document is a proposal and its receipt must bind the dependency
  receipt it relies on.
- *Proposal dependency* (graph lane, `mergeProposalDependencies`): the
  dependency's `closed` is itself only proposed (`proposedClosures`), so the
  parent's inherited closure is an offer that the certifier discharges only when
  the dependency node's *certified* receipt closes that domain. If gating
  withholds it at the dependency, it opens at the parent too — the same
  discipline `authenticate_dependency_receipt` applies to dependency demands
  ("what the dependency's receipt certifies is exactly the accepted proposal
  with the withheld domains opened", `dependencies.rs:964`).

**Hazard to verify in step 2.** With a proposal dependency,
`project_accepted_export` reads `is_closed()` on a document whose closure is
merely proposed, so the parent's *local* exports that call `access` take that
closure as knowledge at generation time. Composition must reopen those local
claims when the dependency withholds; confirm it does before relying on it.

## 2. Level B diagnosis: the census refused, correctly

Recorded in both `$SP/p2-ku-b1/audit0.json` and `audit1.json`, node
`@solid-primitives/utils@6.4.1`, artifact case `9887e137…`, export `access`,
domain `callbacks`:

> census refused: the callbacks closure candidate describes call-time
> invocation(s) of parameter(s) 0, but the implementation census dispositioned
> 2 call(s) into the parameter-rooted family (parameter-rooted 1,
> parameter-rooted-accessor 1): the parameter-rooted-accessor member (1) is an
> invocation of caller-supplied code the enumeration does not describe

Not a declined census, not a missing recipe, not an unmet veto. The census ran
to a verdict. The source as certified (`dist/index.js:63`):

```js
export const access = (v) => typeof v === "function" && !v.length ? v() : v;
```

`v()` is the `parameter-rooted` direct call the enumeration describes. `v.length`
is a `property-access-unknown-accessor` form — the compiler binds no data
property on the union-typed receiver — rooted at parameter 0, so ADR 0034
dispositions it `parameter-rooted-accessor`. ADR 0100's
`confirm_described_callbacks` rule 2 refuses any parameter-rooted member other
than the direct call. The model backs it: `semantic-model.md` § callbacks lists
"a getter or setter reached by property access" as an invoking form the closure
must cover, and a caller can install `Object.defineProperty(fn, "length",
{ get })` on a function (configurable), so "`length` is a data property" is not
a proof. An ADR-0034-style excusal ("a getter is the caller's code") is valid for
`creates` and invalid for `callbacks`: an untracked read inside a caller's
getter is precisely what this domain exists to reveal.

**What would discharge it.** A *described accessor item*, extending ADR 0100:
an item `from` parameter 0 through path `["length"]` whose operation is an
accessor read `at: call`, `same-stack`, `untracked`. Four owners have to move:

- producer: `UncensusedInvokingForm` carries `subject_parameter` and the
  rooting kind but **no member path** (`typefacts/src/invocation.rs:964-1010`),
  so site-for-site confirmation needs a protocol bump;
- generator: `interproc.rs` derives no parameter accessor-read rows; a
  `direct_parameter_member_reads` proposal input in the family of
  `direct_callback_parameters` is needed;
- certifier: `described_callbacks` refuses member paths today; rule 2 must match
  accessor sites to items by `(parameter, path)`; the synthesized veto
  (`Observation::DescribedCallbacks`) needs a getter-bearing sample argument;
- consumer: `project_callbacks` matches `ValueSource::Parameter { index, .. }`
  and ignores `path`, so a member-rooted item would be misread as an inline
  invocation of the bare parameter. The consumer semantics of "the argument's
  getter runs untracked at call" must be decided, since that is what turns the
  152 into either certified code or a different, true finding.

The `reads` half of `access` is withheld for an unrelated reason — "veto did
not complete: the probe worker could not resolve `@kobalte/utils` … reached
transitively rather than declared by the analyzed package" (b1) and "probe
recipe scaffold … is unfinished" (fix) — a recipe-corpus scoping problem on
dependency nodes in the graph lane. Tracked separately; it alone moves none of
the 33 because `returns` is also open at the source.

## 3. Steps

### Step 0 — pin the prediction before any code

Write the expected census diff down: steps 1-2 move **exactly 1** row
(`accessWith`, callbacks) and change the message on ~118 rows (33 `access`,
82 `mergeRefs`, `Key`, `accessWith`, `createMediaQuery` lose
`ownerRequirements` from their claim list). Method: the § 24 recipe — the census
command with the old and new case-set catalogs, then a row diff on
`(rule, primaryLocation, analysisContext, message)`. Falsifier: any `callbacks`
row on `access` or `mergeRefs` moving means a closure was published that the
source does not certify, and the change is unsound.

### Step 1 — re-emission preserves an inherited closure as a proposed, attributed closure

Where: `solid-facts-backend/src/main.rs` (`contract_exports_for_entry_file`)
and `inferred_contract.rs` (`normalize_export`). Add a fail-closed provenance
field to `ContractExport` (`inherited_from: Option<AcceptedReexportIdentity>`,
default `None`, never encoded from a local summary). In `normalize_export`,
when it is `Some`, do not run the local proposal filters at all: propose
exactly the domains the projection has closed (each `Known` domain absent from
`open_claims`, plus `creates` when `creates_closed_empty`), and record the
dependency export identity beside the candidate in the plan sidecar so the
certifier can discharge by composition. Empty vacuous proposals (`Key`, `chain`
today) go through this same path, so nothing is proposed by accident any more.

Wire: `proposedClosures` already exists (`contract_document.rs:422`). The
provenance belongs in the certification plan / candidate record, not the main
document, unless the certifier needs it to survive discovery — decide with the
`SEMANTIC_DIGEST_DOMAIN_COMPOSED` precedent in mind, because a new main-document
field changes every receipt.

Checks:
- `scripts/contract-dependency-reexport.test.mjs`: give `depkg`'s `clean` a
  closure (its `handler()` body already yields a confirmable described
  callbacks enumeration and a clean `creates` walk) and assert `reexporter`'s
  `clean` publishes the same `closed` and `proposedClosures`, and that
  `mixed`'s `opaque` publishes none. This is the only harness with a
  dependency-catalog surface; `fixtures/package-contracts/` cannot host it.
- a unit test in `inferred_contract.rs` on `normalize_export` with an inherited
  summary and walk flags `false`.
- `make contract-corpus` must not move: no corpus fixture generates against a
  dependency, so any moved snapshot is a bug.

Movement: **0**. Alone, this only turns silent drops into withheld candidates
at the parent node, because step 2 does not exist yet. Land it with step 2.

### Step 2 — the certifier discharges an inherited closure by composition

Where: `contract_certification/dependencies.rs` (graph lane) and the standalone
path that reads a receipt-accepted catalog. For a candidate carrying
step 1's provenance, skip the census and discharge exactly when:

1. the parent's `verified_exports` binds the export's runtime identity to the
   dependency node's artifact-case module and export (receipt evidence today);
2. that dependency's *certified* receipt (post-gating) closes the domain for
   that export;
3. the parent's items equal the projection of the dependency's.

Otherwise withhold with a named reason ("inherited closure: dependency receipt
leaves `callbacks` open for `access`"), so a withheld dependency domain opens at
the parent. Dependency-first node order already exists.

Checks:
- a two-package graph test beside the existing re-export graph fixtures
  (`contract_certification.rs` ~5823, "A dependency package whose entrypoint
  re-exports `VALUE`"): positive (dependency closes, parent re-exports, parent's
  receipt closes); negative (dependency withheld → parent withheld); negative
  (parent's items differ → refuse).
- the step-0 census diff after re-certifying `@kobalte/utils@0.9.2` with
  `pass2.mjs --graph-lane` and selecting the `/exports/./import/default` case.
- gate: `make verify` (Rust certification tests plus the scripts step).

Movement: **1** (`accessWith` callbacks) plus the ~118 message-only changes.
Falsifier: `0` means the wrong case was bound or composition never ran;
`>1` means something closed that the source does not certify.

### Step 3 — the described-accessor ADR (Level B)

Scope as in § 2. This is an ADR across producer, generator, certifier and
consumer, with a protocol bump. Prerequisite measurement before writing it:
of the 152 `access` callbacks sites at kobalte/core, how many pass an arrow
accessor versus an object or function that could carry a getter, since that
decides what the consumer does with the new item.

Movement: **152** (`access` callbacks), only with steps 1-2 landed. Elsewhere,
the 2026-09-13 corpus pin counts 62 refused `access` candidates and 93 in the
accessor-beside-direct-call class. Falsifier: after the ADR, the spu node's
`access` callbacks row still withheld means the veto or the site match refused;
read the reason before touching the consumer.

Not part of this plan: `mergeRefs` (235) needs a described *element* item
(`for (const ref of refs) ref(el)`, `parameter-rooted-element` /
`-iterable`), which is a different ADR; the graph-lane recipe scoping behind
the `reads` withholdings; the `fallback-all` ladder; `mergeDefaultProps`.

## 4. Movement summary

| step | moves | message-only | of 1037 reachable after |
| --- | ---: | ---: | ---: |
| 1 (alone) | 0 | 0 | 1037 |
| 1 + 2 | 1 | ~118 | 1036 |
| 3 (after 1 + 2) | 152 | — | 884 |

Everything else on the re-export lever is `mergeRefs` (235), whose closure
needs the element ADR, and the four small names with nothing at the source.

## 5. The honest option

Steps 1-2 are worth doing **as a correctness fix**: the contract publishes what
its dependency certified, the accidental unprovable proposals stop, and the
proof rests on a receipt rather than a walk that must refuse. Label the commit
that way; it moves one finding.

Step 3 is where the 152 are, and there is no shortcut to it. The census verdict
on `access` is a correct negative under the semantic model, and the only
evidence that would close the domain does not exist yet: a described accessor
item confirmed site for site and vetoed against a getter-bearing sample. Do not
narrow the model to reach it. If the ADR is not scheduled, record in the
backlog that `access`'s 152 and the accessor-beside-direct-call class are
blocked on it, with these numbers, and move on to `callHandler` (196) and
`mergeDefaultProps` (127), which are `@kobalte/utils`'s own exports and need no
re-export machinery at all.

## 6. What landed (2026-09-15)

**Step 1 — re-emission preserves an inherited closure.** Implemented as written,
with one deviation.

- `ContractExport::inherited_from: Option<InheritedExportOrigin>`, set only by
  `project_accepted_export` and never encoded into a document
  (`solid-reactive-ir/src/lib.rs`, `contracts.rs`).
- `normalize_export` closes `creates` and `returns` from
  `ContractExport::inherited_closure(domain)` instead of the local walk flags,
  and the proposal filter chain in
  `normalize_inferred_contract_with_candidates_and_external_targets` skips the
  three *confirmability* filters for an inherited summary. The **hazard**
  filters still apply: a hazard is a fact about this package's own module
  closure, which an inherited claim does not answer.
- Deviation: the provenance is **not** in the plan sidecar. It travels to the
  emit boundary as `solid-checker:inherited-closure=`, beside
  `DECLINED_CLOSURE_MARKER` and `WITHHELD_OWNER_REQUIREMENT_MARKER`. The plan
  put it in the sidecar "so the certifier can discharge by composition"; the
  certifier does not read it, because a provenance string a document carries
  about itself is the self-report the precision contract refuses as proof. It
  rebinds the re-export from the parent's own snapshot-verified runtime binding
  instead. What the record answers is the auditor's question — which of a
  package's proposed closures rest on a dependency's receipt — which nothing
  else in either artifact distinguishes. No main-document field, no plan-format
  change, no receipt moves.

**Step 2 — the certifier discharges by composition.** Implemented as written.
`census_inherited_dependency_closure` (`type_facts.rs`) sits with
`census_default_library_alias_export` and `census_not_callable_export`, before
the census arms. Condition 3 — "the parent's items equal the projection of the
dependency's" — is answered by *re-running the generator's own derivation*
(`inherited_export_projection`) and comparing, rather than by a separately
written equality; a second notion of "the projection" is where an admitting
mistake would live. The recorded obligation travels as
`inherited-closure-dependency:` and is discharged in
`authenticate_dependency_receipt`'s caller exactly as `census-dependency-creates:`
is, so a withholding dependency opens the domain at the parent through the
existing `composed_from_withheld_dependency` path.

**What is not verified.** The § 3 step-0 prediction — 1 row moving, ~118
messages changing — is **not measured**. It needs the `kobalte/packages/core`
corpus, the two pass-2 catalogs and the `pass2.mjs` scratch harness, none of
which are in this repository; reproducing it requires network installs, which
this repository's operating guide refuses as a way to investigate. Recorded as
an external-artifact blocker in `docs/precision-backlog.md`. The falsifier is
unchanged and still the thing to run first: any `callbacks` row on `access` or
`mergeRefs` moving means a closure was published that the source does not
certify.

**Scope this does not reach.** The standalone lane (a receipt-accepted catalog,
no dependency plans) has no composition premise: the arm answers `Ok(None)`, the
census refuses, and the candidate is withheld — fail-closed, the same open
domain as before, one extra pass. Step 3, the described-accessor ADR holding
`access`'s 152, is untouched and recorded in the backlog with its numbers and
its refusal text.

