# Implementation census for behavioral call domains — semantics first

Date: 2026-09-03
Status: semantics settled; the producer re-kinding and consumer filter of § 2.2
landed (its validation rule and the six authority files are blocked — see
§ 2.2's status block); **§ 4.1's two producer facts landed** at handshake
protocol 14, with the `complete` rename deliberately not taken (see § 4.1's
status block and `docs/typefacts/adr/0026-…`); § 6's audit row added with its
test pins; **§ 4.2's dialect table landed** for the `creates` domain and the
Solid 2.0 archives only, unwired (see § 4.2's status block and
`docs/adr/0007-census-dialect-axiom-tier.md`); **§ 4.3's `creates` census landed**
(`docs/adr/0008-implementation-census-for-creates.md`; see § 4.3's status block)
Scope of this document: the decision, its evidence, and the prerequisite chain.
No code, fixture, or snapshot changed in the slice that wrote it.

ADR 0006 defers "the implementation-census premise for behavioral call
domains", and `require_census_decides_closure` refuses each of them by name
(`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:6630-6637`):

> implementation-census premise required: closing the *N* call domain needs a
> complete `ExportImplementationTranscript`, every `calls` target resolved, and
> no resolved target able to perform the domain's operation.

Before that premise can be built, the domain has to mean one thing. It did not.
This document settles the meaning, records the disagreement it resolves, and
states the order the remaining work has to happen in.

---

## 1. The disagreement, read from both sides

### 1.1 What the generator emits

`normalize_export` builds `creates` from `summary.owner_requirements`
(`rust/crates/solid-facts-backend/src/inferred_contract.rs:276-295`): one
`OperationKind::Create` per `ContractOwnerRequirement`, id
`{prefix}owner-requirement-{index}`, passed to `apply_owner_requirement`
(`:505-517`), which sets

```rust
operation.owner.requirements.owner = Requirement::Required;
operation.owner.source = OwnerSource::AmbientAtCall;
operation.owner.productions = KnowledgeSet::Complete(Vec::new());
```

and then `requirements.cleanup = Required` for a `Cleanup`/`SettledCleanup`
requirement or `requirements.child_owners = Required` for an
`Effect`/`Boundary` one. The requirement itself comes from
`find_missing_owners` (`rust/crates/solid-reactive-ir/src/owners.rs:667`, call
walk at `:771-880`), which walks every call in the archive and pushes a
requirement for a `createEffect` / `createRenderEffect` / `createTrackedEffect`
/ `onCleanup` / `onSettled` call, or for a callee whose own contract carries
one, that is not inside an owner-providing region; the generator reads the
result as `program.missing_owners`
(`generated_owner_requirements_by_symbol`, `main.rs:6285-6333`).

So a generated `create` operation was an **owner requirement**. It registered
nothing: it named no resource, and its `productions` was closed empty. The
generator emitted only four kinds at all — `Read`, `Return`, `Create`,
`Invoke` — and hard-wired `writes`, `invalidates`, `throws`, `cleanups`, and
`disposals` to `Unknown`. (Both sentences are past tense as of the producer
slice: § 2.2 item 1 landed, so the four kinds are now `Read`, `Return`,
`Invoke`, `Cleanup`, `creates` is always `Unknown`, and `cleanups` is
`Partial` when it carries an item.)

Two bundled documents carry that shape:

| document | export | `creates` |
| --- | --- | --- |
| `pkg/contracts/bundled/solid-v1/debounce-root-default.json` | `createDebounce`, `default` | `["owner-requirement-0"]`, closed; `requires: required`, `requiresCleanup: required`, `source: ambient-at-call`, `productions: []` closed, no `resources` |
| `pkg/contracts/bundled/solid-v1/rootless-root-default.json` | `createRootPool` | identical shape |

Both are generator output. `rootless-root-default.json` still carries 42
`proposal:@solid-primitives/rootless:…` resource ids, which only
`inferred_contract` produces. They are the only two documents in
`pkg/contracts/bundled/**` that contain the string `owner-requirement-`.

### 1.2 What the hand audits do

The Solid-core documents — `solid-v2/solid-js.json`,
`solid-v2/solidjs-signals.json`, `solid-v2/solidjs-web.json` and its
per-condition siblings, and all fourteen `solid-v1/solid-*.json` — use
`creates` for exactly three operations across the whole corpus, and each one
names a resource it registers with an environment outside the call — the
document, or the server runtime:

| document | export | operation | `kind` | `resources` | owner |
| --- | --- | --- | --- | --- | --- |
| `solid-v2/solidjs-web.json` | `render` | `register-delegation` | `create` | `["browser-root"]` | `source: created`, `productions: [browser-root]`, `requires: required` |
| `…--server-functions-node-server.json` | `createServerReference` | `register-reference` | `create` | `["server-reference"]` | `source: none`, `requires: forbidden`, `productions: []` closed |
| `…--server-functions-browser-client.json` and the node one | `createServerReference` | `transform-reference` | `create` | `["server-reference"]` | `source: none`, `requires: forbidden`, `productions: []` closed |

`render` is the **one** audited `create` that also names an owner: its
`register-delegation` is a `creates` item *and* carries `source: created` with
`productions: [browser-root]`. The two `createServerReference` operations name
no owner at all.

Everywhere else an owner comes into existence, the audits publish no `create`.
What they publish instead is not uniform, and the three shapes matter, because
only the first is the shape the generator should have been emitting:

**Shape 1 — recorded on the operation that runs under the owner.** An `owner`
resource plus `owner.source: "created"` and an `owner.productions` entry on an
`invoke` that is a `callbacks` item, with `creates: []` closed beside it:

- `solidjs-signals.json` `createTrackedEffect` — `callback` is `kind: "invoke"`,
  `source: created`, `resource: createTrackedEffect-leaf-owner`,
  `productions: [that]`, `requires: required`, `requiresChildren: forbidden`;
  `creates: []` closed.
- `solid-js.json` `For` / `Match` / `Repeat` / `Show` — `accessor-child` and
  `raw-child`, both `invoke`, both `source: created` over `row-owner` with the
  production declared; `creates: []` closed.
- `solidjs-web.json` `render`'s and `hydrate`'s own render callbacks —
  `render-callback` and `hydrate-callback`, both `invoke`, both
  `source: created` over `browser-root`.
- The *generated* `rootless-root-default.json` does the same for its callback
  owners: `createSubRoot`, `createSharedRoot`, `createBranch`,
  `createSingletonRoot`, `createHydratableSingletonRoot`, `createDisposable`,
  and `createRootPool` each have a `callback-0` `invoke` with
  `source: created` and a nonempty `productions`, and `creates: []` closed. The
  generator's own `owner_created` helper (`inferred_contract.rs:467-501`) is
  what writes that, and it never produces a `create` operation.

Those, plus `render`'s `register-delegation`, are the **only** thirteen
`source: created` operations in `pkg/contracts/bundled/**`.

**Shape 2 — declared as a resource, with the creation recorded nowhere.** The
summary declares the `owner` resource and its operations reference it, but *no*
operation carries `source: created`, so nothing in the document states that
this call brought the owner into existence:

- `solid-js.json` `createEffect` — declares `effect-owner` (kind `owner`) and
  `effect-cleanup` (kind `cleanup`) as resources, has `repeated-compute`
  *capture* `effect-owner` (`source: captured`) and `dispose-effect` dispose
  it, and closes `creates: []`. Its six operations' owner sources are
  `captured`, `ambient-at-call`, `ambient-at-execution` ×2, and `none` ×2 —
  none is `created`, and every `productions` is `[]`.
- `solidjs-signals.json` `onSettled` — declares `onSettled-leaf-owner`, its
  `callback` is `source: ambient-at-execution` with `productions: []`, and it
  closes `creates: []`. That is the disagreement ADR 0005 § "2026-09-03"
  already resolved on the generator's side by withholding the domain for a
  dialect's own primitive-defining archive. `@solidjs/signals` contains
  **exactly one** `source: created` operation in the whole document,
  `createTrackedEffect`'s.

**Shape 3 — nothing at all.** All fourteen audited `solid-v1/solid-*.json`
documents (74 summaries; the whole 19-document directory has 89, with the same
absence throughout) carry **no**
`owner` field on any operation, **no** `resources` entry, and close only
`callbacks`, `reads`, `creates`, and `returns` — while `createRoot`,
`createSignal`, `onCleanup`, `createMemo`, `createStore`, `createMutable`,
`render`, and every other 1.x primitive sits under `creates: []` closed.

No reactive-graph resource of any kind is a `create` in the audits:
`createSignal`, `createStore`, `createOptimistic`, `reconcile`, `snapshot`, and
`createProjection` declare `reactive-source` resources with `creates: []`
closed; `createMemo` and `Loading` declare `async-computation` resources with
`creates: []` closed; `action` and `createOptimistic` declare `transition`
resources with `creates: []` closed.

**This is why the predicate cannot be "brings a resource into existence".**
Shapes 2 and 3 are the majority of the corpus, and under that predicate every
one of them would be a document that closes `creates: []` while creating
something — i.e. a corpus-wide contradiction. `creates` is therefore defined
over the published `create` **operation** (`semantic-model.md` § creates), and
"this reactive-graph resource began to exist on this call" is a positive fact
version 1 has **no domain for**. That gap is recorded in
`docs/precision-backlog.md`; it is not repaired by widening `creates`.

### 1.3 What the model already said

- `semantic-model.md` § Ownership: "Owner production, owner requirement, owner
  source, owner capability, and owner lifetime are distinct facts." And:
  "Creating an owner does not prove the operation itself required one."
- `wire-format.md:277-288`: "Owner production is a separate locally closed
  domain", with its own `closed: ["productions"]`.
- `contract_semantics.rs:788` lists `OperationClaimDomain::OwnerProductions`
  as its own operation-claim domain, and generated proposals already emit
  closure candidates for it: across
  `fixtures/package-contracts/**/expected-proposal.json` there are 16
  `{kind: "operation-axis", domain: "owner-productions"}` candidates alongside
  216 `{kind: "call", domain: "creates"}` ones.
- `validate_call_claims`
  (`rust/crates/solid-reactive-ir/src/contract_semantics/validate.rs:1030-1128`)
  binds `creates` to `OperationKind::Create` in both directions.

So the model had *already* given owner production its own home, its own
closure, and its own closure-candidate axis. The generator put owner
*requirement* into `creates` anyway, and thereby made `creates` mean two
incompatible things at once.

---

## 2. Decision: option (b) — `creates` is the published `create` operation

**`creates` is the domain of published `create` operations, defined by the act
those operations record and not by the English word "create".** A `create` is
the export **registering a version-1 resource into a runtime outside this
invocation** — a browser document or a server runtime — so the resource stays
live there after the call returns, reachable by that runtime rather than only
through a value the call handed back, naming what it registered in the
operation's own `resources`. Both qualifications are load-bearing: the
registered thing must be a version-1 resource *kind*, and the registry must be
a *runtime that acts on it* — a package's own private module variable is
neither (§ 3.3). The three audited instances are the whole extension in the
corpus today: `render`'s `register-delegation` (`browser-root`) and
`createServerReference`'s `register-reference` and `transform-reference`
(`server-reference`).

Three things it is **not**:

- **A summary's `resources` declaration.** Declaring a resource is not an
  operation, so it is never a `creates` item and never contradicts
  `creates: []`. This is what the whole audited corpus does — see § 1.2
  shapes 2 and 3.
- **Owner production**, which is the `owner.productions` domain of whichever
  operation runs under the produced owner.
- **Owner requirement**, which is the per-operation `owner.requires` /
  `requiresChildren` / `requiresCleanup` triple that `semantic-model.md`
  § Ownership already defines, attached to the operation that needs the owner.

**Not settled by this, and deliberately so:** version 1 has no domain in which
"this reactive-graph resource began to exist on this call" is a positive,
closable fact. `createSignal`, `createStore`, `createMemo`, `action`,
`createEffect`, `createTrackedEffect` and `onSettled` all establish version-1
resources and all close `creates: []`, so the model records their coming into
existence nowhere. That is a genuine model gap, recorded in
`docs/precision-backlog.md`, and it is **not** repaired by widening `creates` —
doing so would contradict the majority of the audited corpus.

The predicate is written out in `semantic-model.md` § "What a closed call
domain denies".

### 2.1 Why (b) and not (a) or (c)

**(a) — `creates` means owner requirement, and the audits are wrong.** Rejected.
It would require correcting `solid-js.json` `createEffect`,
`solidjs-signals.json` `createTrackedEffect`/`onSettled`, `solid-js.json`
`For`/`Match`/`Repeat`/`Show`, and every `solid-v1/solid-*.json` primitive that
registers on an owner — the majority of the audited corpus, against the
documents that are the semantic model of record — and it would leave the three
audited `create` operations, which are unambiguously external registrations
naming `browser-root` and `server-reference`, with nowhere to live. It also
cannot explain why the audits publish `owner.productions` at all.

**(c) — two domains.** Rejected as unnecessary. Owner requirement already has a
representation (`owner.requires` and its two siblings) that every audited
document uses and that `validate_call_claims` already accepts; adding a
`requires` claim domain would be a second spelling of an existing fact, would
need a new `ClaimDomain` variant and a new canonical-stream position, and would
move the semantic digest of every contract. There is nothing to buy.

**[Correction 2026-09-03] (c) is reopened for exactly one case.** That
rejection holds wherever there *is* an operation to hang the triple on, which
covers every audited document. It does not cover a **free-standing** owner
requirement: an export that must be called under an ambient owner because it
registers a computation on it has no operation of its own to carry the field —
the registration is not a `create` (§ creates), and inventing one so the field
has a host is what item 1's first draft did and what this slice reverted. That
case is now withheld by name and has no representation at all, so (c) is the
repair for it. What it needs is unchanged from the rejection: a new
`ClaimDomain` variant, a canonical-stream position, and a semantic-digest move
for every contract — which is why it is a separate slice and not this one.

**(b) is also the only one an implementation census can decide.** Under (a) the
census question is "does this export require an ambient owner", which is a
question about the *callers* this archive cannot see — it is exactly the
inference `find_missing_owners` performs and exactly the inference ADR 0005
found wrong for `onSettled`: the `getOwner()`-guarded arm read as an
unconditional consumer obligation is § "2026-09-03" (`:312-319`), and the
circularity of deriving the owner claim from the rows that would discharge it
is § 4 (`:137-160`). (§ 5, "Zero rows moved", is about demand advancement, not
about the guarded arm.) Under (b) the question is "does any resolved target
publish an operation of `kind: create`", which is a question about *this
archive's own implementation*, which is what a census is.

### 2.2 What changes, exactly

**Status after the producer slice (2026-09-03, corrected).** Items 1, 2, and 5
are **done**; items 3 and 4 are **blocked together**, and item 4's
fixture-snapshot obligation is discharged as "did not move". Item 1 landed as
**withholding both non-cleanup roles**, not as the re-kinding its first draft
described — see the rewritten item below for why the `Effect` `create` was
wrong. Item 1's cleanup publication and item 5's filter are in
`inferred_contract.rs`'s `owner_requirement_operation` and
`contracts.rs`'s `project_owner_requirements`, item 5 taking the first of the
two options it names (read `cleanups` for its items, insert no
`ClaimDomain::Cleanups`). Item 3's validation rule is written as a `NOT YET`
comment in `contract_semantics/validate.rs` rather than as code, because with
it in place 14 `solid-facts-backend --lib` tests fail on the two item-4
documents — and item 4 cannot be discharged either: the correction is an
authority **re-capture** rather than a regeneration (no live issuer, sidecar
bytes absent from the repository, and `require_census_decides_closure` refuses
every closed call domain a fresh certification would have to re-close), so all
six files are left untouched. The measurements, the failing test names, and the
file:line evidence are in `docs/precision-backlog.md`'s
2026-09-03 entry "The generator stopped publishing an owner requirement as a
resourceless `create`". Item 4's fixture contract
(`reactive-ir/package-callback-consumer`) still carries the shape and is still
refused before decode by its `obsolete-policy1` catalog entry; coverage
confirmed its snapshot did not move. The fixture pair item 5 asks for is not
constructible while every consumer fixture's contract is cut, and is pinned by
`owner_requirement_projection_tests` instead.

The changes (b) implies, for the producer slice:

1. **`inferred_contract.rs`'s owner-requirement loop and
   `apply_owner_requirement` stop emitting a resourceless `create`.** A
   `Cleanup`/`SettledCleanup` requirement becomes a `kind: "cleanup"` operation
   in **`cleanups`**, with `owner: {source: ambient-at-call,
   requires: required, requiresCleanup: required}` and **no resource**. An
   `Effect` or `Boundary` requirement is **withheld by name**: no operation, no
   closure candidate, `creates` left open, and a record naming the export, the
   role, and the reason in the generator's refusal sidecar.

   **An `Effect` requirement is not a `create`, and the first draft of this
   item was wrong to make it one.** That draft had it become a `create` in
   "the audits' own shape" — `resources: [<child owner>]`,
   `owner: {source: ambient-at-call, requires: required,
   requiresChildren: required, productions: [<that owner>]}`. There is no such
   audited shape. § creates defines `create` as registering a version-1
   resource into a runtime **outside** the invocation, says the three audited
   registrations (`render`'s `register-delegation`,
   `createServerReference`'s two) are the whole extension in the corpus, and
   says "Do not repair it by widening `creates`". Registering a computation on
   the *caller's* owner is precisely what the audits publish beside
   `creates: []` **closed**. Attaching a child-owner resource also defeats item
   3's separator: the mechanical rule is "a `create` naming no resource is a
   contradiction", and manufacturing a resource so the rule passes turns the
   separator into a formality.

   So the requirement is withheld, and the **model gap** is recorded instead:
   a free-standing owner requirement — an export that must be called under an
   owner because it registers a computation on it — has no operation kind that
   can carry it in schema version 1, and the audits record no consumer-level
   owner requirement anywhere. Option (c) below is the repair. `semantic-model.md`
   § creates carries the decision and states what is lost: a *generated*
   proposal no longer carries the `Effect` positive fact, so a consumer
   `SC4001` derived from a **generated** dependency contract is unavailable
   until the new domain exists. Nothing live changes — no generated contract is
   accepted anywhere today — and the hand-audited path is untouched.

   **The published cleanup shape has no audited precedent either, and says so.**
   Every `kind: cleanup` operation in the bundled corpus — `solid-js`'s
   `replace-cleanup`, `@solidjs/signals`'s `returned-cleanup`,
   `@solidjs/web`'s `ref-cleanup`, `--web-node-server`'s
   `retract-declaration` — is `requires: forbidden`, `source: none`, because
   each describes a cleanup the *runtime* runs rather than one the export
   installs on its caller's owner. The generated shape is chosen for the fact:
   the requirement is a `Requirement` triple on the operation that needs the
   owner, the installing act is a cleanup, and `require_owner_operation_call`
   witnesses it from the archive's own `onCleanup` call. It names **no
   resource**, as `returned-cleanup` and `ref-cleanup` also do not — a resource
   declaration is a positive fact of its own (`PositiveFactSubject::Resource`,
   demanded as `ProofFamily::RecursiveValueShape`) and no witness exists for a
   resource axis, so declaring one refuses the row at witness acquisition.

   **`Boundary` is reachable, and is withheld for a second, independent
   reason.** `OwnerRequirementOperation::Boundary` has exactly one live origin:
   the JSX loop at `rust/crates/solid-reactive-ir/src/owners.rs:1213-1231`,
   which pushes the internal string `"boundary"` for a `jsx_elements` entry
   whose tag `dialect.is_async_boundary` accepts and which is not inside an
   owner-providing region. Those candidates reach `program.missing_owners`, and
   `main.rs`'s generated-owner-requirement indexing (`:6332`) consumes them, so
   **an archive that ships an async-boundary JSX element does reach this arm**.
   (An earlier draft called it unreachable on the strength of
   `project_owner_requirements` emitting `Effect`/`Cleanup` only and
   `solid-facts-backend/src/dialect.rs:660` emitting `Cleanup` — but that line
   is inside `mod tests`, a sample fixture, and is no evidence about the live
   engine.) It stays withheld because it is a *compiler lowering* fact and the
   producer's census records neither JSX elements nor their lowering, so
   nothing could discharge it.
2. **Schema.** No change. Both target shapes are already expressible in
   `schemaVersion: 1`: `resources` on an operation, `owner.productions`,
   `owner.requires*`, and `kind: "cleanup"` are all existing fields with
   existing validation. This is why (b) is additive-free rather than additive:
   it *narrows* what the generator emits into shapes the audits already use.
   `schemaVersion` and `semanticModelVersion` both stay 1.
3. **A validation rule:** a `create` operation naming no resource is a
   contradiction. This is the mechanical separator between the audits' use and
   the generator's, and it belongs beside the existing kind/domain checks in
   `validate_call_claims`
   (`rust/crates/solid-reactive-ir/src/contract_semantics/validate.rs:1030-1128`).
   **It rejects three checked-in documents on the day it lands** — the two
   bundled ones in their three tracked copies each, and one fixture contract
   that is a live analyzer input — so it cannot be committed ahead of item 4.
   Land the rule and the document corrections together.
4. **The two bundled documents — and they cannot simply be "regenerated".**
   `debounce-root-default.json` (`createDebounce`, `default`) and
   `rootless-root-default.json` (`createRootPool`) each carry one
   `owner-requirement-0`. Under (b) each becomes a `cleanups` item — both
   requirements derive from an `onCleanup` call and both carry
   `requiresCleanup: required` — with a `cleanup` resource, and `creates`
   becomes `[]` closed.

   Running the corrected generator is **not** an available repair. These bytes
   are frozen Phase 14 authority: `first_party_bundles.rs` `include_bytes!`s
   them from `benchmarks/package-contract-v2/phase14/solid-v1-authority/`
   (macro at `:84-95`, index at `:97-99`, the twenty-document list at
   `:101-122`), and the function that would rebuild them,
   `solid1_bundles_with_measurements` (`:303`), decodes and cross-checks every
   authority document and then returns `Ok(Vec::new())` (`:369`) — as does its
   Solid 2 peer `solid2_rc3_bundles_with_measurements` (`:198`, empty at
   `:289`). `EMBEDDED_SOLID1_BUNDLES` is `&[]` (`:124`). There is no live
   generation path to re-run; the correction is either to the **Phase 14
   authority capture itself** or to a **fresh generation against the real
   `@solid-primitives/debounce` and `@solid-primitives/rootless` packages at
   the audited versions**, and whichever is chosen must be stated in the commit
   message, because the two produce different provenance.

   **Six checked-in files, plus one fixture contract.** Each document exists in
   three tracked locations:

   - `benchmarks/package-contract-v2/phase14/solid-v1-authority/debounce-root-default.json`
   - `benchmarks/package-contract-v2/phase14/solid-v1-authority/rootless-root-default.json`
   - `pkg/contracts/bundled/solid-v1/debounce-root-default.json`
   - `pkg/contracts/bundled/solid-v1/rootless-root-default.json`
   - `rust/crates/solid-dialect/contracts/solid-v1/debounce-root-default.json`
   - `rust/crates/solid-dialect/contracts/solid-v1/rootless-root-default.json`

   The `pkg/` and `solid-dialect/` copies are byte-identical to each other and
   differ from the `benchmarks/` copy in exactly one field: they carry a
   `sidecars.proof.sha256`
   (`5eadb9af8d94b5fb899d9755c376df459f78565a9473b2c8fd20096b9708ed47` for
   debounce, `4ab8db269d5fe0b6e88feb62cc35f2038baec03c87bd81fd456f0ca039cab511`
   for rootless) where the authority copy has `sidecars: {}`. Correcting the
   claim therefore also invalidates those two proof-sidecar digests, which a
   hand edit would silently desynchronize. The `benchmarks/` copies are
   referenced by no script under `scripts/` and by no `Makefile` target — they
   are gated only through the Rust decode path above — so a correction that
   updates `pkg/` and `solid-dialect/` and forgets `benchmarks/` leaves the
   compiled-in authority still asserting the old claim, and no gate says so.

   The **fixture** contract
   `fixtures/reactive-ir/package-callback-consumer/node_modules/reactive-package/solid-reactivity.json`
   carries the same shape (`runOwnedEffect`'s `owner-requirement-0`: `kind:
   create`, no `resources`, `source: ambient-at-call`, `requires: required`,
   `productions: []`) and is a live analyzer input, so item 3's validation rule
   rejects it. It must be corrected in the same commit, and
   `fixtures/findings-snapshots/reactive-ir__package-callback-consumer.json`
   re-derived by coverage rather than assumed unchanged. Note that fixture is
   *already* degraded relative to its README: the snapshot is five
   `SC9005 package-contract-incomplete` uncertifiables and no `SC4001` or
   `SC1001`, because the fixture's `.solid-checker/accepted-contracts.json`
   entry is `"status": "obsolete-policy1"` after the proof-policy-2 cut
   (`662dd7ba`). The README's claims that `runOwnedEffect()` "must produce
   `SC4001`" and `Bad` "must produce its existing `SC1001`" no longer hold;
   correct the README in the same slice or record the discrepancy.
5. **`project_owner_requirements` (`rust/crates/solid-reactive-ir/src/contracts.rs:333-364`),
   the analyzer's only consumer of `creates`,** must move to reading
   `cleanups` and `creates` per (b). It currently iterates `creates` items and
   projects a `ContractOwnerRequirement` from any operation with
   `owner.requirements.owner == Required` (`:349`), mapping `Cleanup`/`Dispose`
   kinds to `Cleanup` and everything else to `Effect` (`:350-355`).

   **Scope the `cleanups` read so it does not newly open a claim.** The
   function's other job is to insert `ClaimDomain::Creates` into `open_claims`
   when `creates` is not closed (`:340-342`). Extending it to `cleanups` the
   same way would insert `ClaimDomain::Cleanups` for **every** Solid 1.x
   contract: all 89 summaries across the 19 `pkg/contracts/bundled/solid-v1/`
   documents omit `cleanups` entirely, which is `Unknown`. (The generator no
   longer hard-wires `cleanups: KnowledgeSet::Unknown`: it publishes the domain
   `Partial` when it has a cleanup requirement to put in it and `Unknown`
   otherwise. The measured consequence below is unchanged, because a `Partial`
   domain is still not closed.)

   Measured consequence, which is *not* a new `SC9005`:
   `push_unknown_contract_claims` (`contracts.rs:602-630`) labels only four
   domains — `Reads`, `Returns`, `Creates`, `Throws` — and returns without
   pushing a defect when its `claims` list is empty (`:631-633`). So a bare
   `Cleanups` insert makes `open_claims` non-empty at each of the three call
   sites (`:842`, `:929`, `:1012`) without producing any finding. What it
   *would* break is `contract_document.rs:3544`, which asserts that a proven
   non-callable value export leaves **no** call-path domain open — the clearing
   block at `contracts.rs:88-104` removes only `Callbacks`, `Reads`, `Returns`,
   and `Creates`. Therefore: either do not insert `Cleanups` into `open_claims`
   at all (read the domain's items for the requirement projection only), or add
   both a `"cleanups"` label in `push_unknown_contract_claims` and a clearing
   line in the value-export block — and in the second case accept that every
   1.x contract import gains an `SC9005`, which is a precision regression that
   needs its own decision and its own snapshot review. Do not take the second
   option incidentally.

   **A latent defect to verify in that slice, not proven here:** the filter at
   `:349` reads `requires` without consulting `owner.source`. `@solidjs/web`'s
   audited `render` has `register-delegation` in `creates` with
   `requires: required` *and* `source: created` — an operation that runs under
   the owner it made, which needs no ambient owner. On the reading of `:349`
   alone that projects as a consumer owner requirement, which would make a
   top-level `render(() => <App/>, el)` report an owner-less effect.
   `semantic-model.md` § Ownership is explicit that these are distinct facts.

   **It is latent because no bundle is issued at all.** Both bundle indexes
   have `"contracts": []`, `EMBEDDED_SOLID1_BUNDLES` is `&[]`, and both
   `*_bundles_with_measurements` functions validate and then return
   `Ok(Vec::new())` (`first_party_bundles.rs:289`, `:369`), so the audited
   `render` summary never reaches `project_owner_requirements`. The one fixture
   whose contract *does* carry a `requires: required` create,
   `reactive-ir/package-callback-consumer`, is cut to `obsolete-policy1` and
   produces `SC9005` instead of a projected requirement. Nothing exercises the
   filter today.

   **The original item 1 would have made it live**, which is the reason it was
   corrected: had `Effect` requirements been stamped `source: created`, every
   generated consumer obligation would have taken exactly `render`'s shape and
   the missing `source` test would have become the difference between firing
   and not firing on real code. With item 1 as corrected above — requirements
   stamped `source: ambient-at-call` — the narrowing
   `source == AmbientAtCall && requires == Required` is the right filter for
   generated contracts *and* excludes audited `render`. But that filter also
   silently drops any future audited operation that legitimately requires an
   ambient owner while recording some other `source` (`captured`,
   `ambient-at-execution`), so state the intent rather than the enum
   comparison: **a requirement projects when the operation requires an owner it
   does not itself supply.** Concretely, filter on
   `requires == Required && source != Created(_)`, not on
   `source == AmbientAtCall`. Reproduce both cases with fixtures — an audited
   `render` call at module top level that must stay clean, and a generated
   `ambient-at-call` requirement that must still report `SC4001` — before
   changing the filter.

### 2.3 No existing receipt or `semanticDigest` moves — how that is known

- **Receipts.** Both `pkg/contracts/bundled/solid-v1/bundle-index.json` and
  `pkg/contracts/bundled/solid-v2/bundle-index.json` have
  `"contracts": []`. The array is the "receipt-issued bundle index"
  `rust/crates/solid-facts-backend/src/dialect.rs:538-554` checks, and it is
  empty, so no bundled document is receipt-bound. `auditPhase19Cut`'s
  `activePolicy2Receipts` is asserted `== 0`
  (`scripts/package-contract-v2-phase19-report.mjs:192`). There is no receipt
  to move.

  The **stronger** reason, and the one to rely on: no first-party bundle is
  issued at all. `EMBEDDED_SOLID1_BUNDLES` is `&[]`
  (`first_party_bundles.rs:124`), and **both** bundle producers validate their
  inputs and then return an empty vector —
  `solid2_rc3_bundles_with_measurements` (`:198`, `Ok(Vec::new())` at `:289`)
  and `solid1_bundles_with_measurements` (`:303`, `Ok(Vec::new())` at `:369`).
  An empty `contracts` array could be refilled by a later commit; an empty
  return from the only two producers means nothing downstream of them runs
  today.
- **`semanticDigest` of the two corrected documents does move**, necessarily:
  they will state a different claim. That is a document correction, not a
  compatibility break, and it is the point — the current documents assert an
  operation that registers nothing. Their `sidecars.proof.sha256` values move
  with them (see § 2.2 item 4), and the `benchmarks/` authority copies — which
  carry `sidecars: {}` and are the bytes actually compiled in — must be
  corrected in the same commit or the compiled authority keeps the old claim.
- **Every other document's digest is untouched**, because (b) adds no field to
  the canonical stream. Contrast the `composedFrom` change recorded in
  `docs/precision-backlog.md:1312-1350`: folding an `Option` into
  `canonical::operation` stamps a discriminator whether or not the field is set,
  which moved 47 of 82 generator fixtures and would have broken every issued
  receipt, and was repaired with two disjoint digest families separated by their
  domain string (`SEMANTIC_DIGEST_DOMAIN` and
  `SEMANTIC_DIGEST_DOMAIN_COMPOSED`). (b) needs none of that: it changes which
  shapes the generator chooses among shapes the stream already encodes.
- **The frozen golden vector**
  `sha256:23c3aef34b18c809cbfe185cb53ed4b37275ab6486da190b37f4e18d8291c2b9`
  (`semantic-model.md`, `contract_semantics/tests.rs:1015`) is a hand-built
  model value, not generator output, so the generator change cannot move it.
  The producer slice must confirm that by running `ir-lib`; if it does move,
  the domain-separated path above is the only admissible repair and the slice
  stops until it is taken.
- **Generator fixtures will move.** 13 files under `fixtures/` contain
  `owner-requirement-`, of which 10 are `expected.json` /
  `expected-proposal.json` pairs for five fixtures
  (`callback-untracked-wrapper`, `callback-deferred-untracked-chain`,
  `dialect-detection`, `multi-role-callback-parameter`,
  `dialect-defining-archive/@solidjs/router-shaped`). Those snapshots travel
  with the generator commit. The other three are
  `package-contracts/dialect-defining-archive/@solidjs/signals/index.ts` and
  its `README.md` (prose and source, which the census slice reviews) and
  `reactive-ir/package-callback-consumer/node_modules/reactive-package/solid-reactivity.json`,
  which is a **live analyzer input** rather than a generator expectation and is
  handled in § 2.2 item 4.

---

## 3. The census predicate for `creates`, consumer packages only

Stated as the negation the census must establish, for one export, one artifact
case, one guard, on `GenerationScope::ConsumingPackage`:

> **`creates: []` certifies only if: every invoking form in the export's
> transitive census, taken at the `MayExecute` reachability floor, is
> enumerated and resolved, and no resolved target performs a `create`
> operation.**

The wording is deliberate on both halves. "Performs a `create` operation" is
the decidable question — does any resolved target publish an operation of
`kind: "create"`, per `semantic-model.md` § creates — and it is *not* "brings a
version-1 resource into existence", which the audited corpus refutes as a
predicate: `createSignal`, `createStore`, `createMemo`, `action`,
`createEffect`, `createTrackedEffect` and `onSettled` all establish version-1
resources and all close `creates: []`. A consumer that calls `createSignal`,
`createMemo`, or `createEffect` therefore certifies `creates: []`; a consumer
that calls `@solidjs/web`'s `render` does not, because `render` publishes
`register-delegation`.

Spelled out:

1. The export's own `ExportImplementationTranscript` is `complete`, **and**
   the producer additionally attests that its census enumerated every invoking
   form — see § 4.1, because `complete` does not mean that today.
2. Every `ImplementationCall` in `calls` whose `reach` the floor admits is
   resolved: non-empty `target`, a `declaration`, and one of the terminating
   dispositions in § 3.1. **The floor is `MayExecute`** — everything not
   provably `Unreachable`, i.e. `Reachable` *or* `Unknown`
   (`ReachabilityFloor::admits`,
   `rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:3382-3397`).
   An `Unknown`-reach call is **in** the census and must be resolved like any
   other; it is not excused by its uncertainty. This is the same floor § 4.2
   item 3 requires of the dialect negative table, and for the same reason: a
   closed domain asserts a zero *upper* bound, so anything that may execute
   bears on it.
3. No resolved target performs a `create` operation — a published operation of
   `kind: "create"`. A target that merely declares a resource in its summary's
   `resources`, or that carries `owner.source: created` with a nonempty
   `owner.productions`, performs no `create` and terminates the census
   affirmatively.
4. Every non-call invoking form the producer marks is resolved the same way,
   or the domain refuses **by name** — never by silence. The list in
   `semantic-model.md` is open-ended, so this obligation is over the forms the
   producer's classifier *covers*, and any form it does not classify is itself
   a refusal.
5. The census derives its own enumeration of the export's `create` operations
   and requires the proposal's set to equal it, exactly as
   `require_export_value_enumeration_matches_census` does for root choice
   alternatives (`type_facts.rs:6673-6710`). A proposal that closes `creates`
   with a different item set than the census enumerated refuses. Without this
   the closure would be the proposal's claim rather than the census's, which
   is objection 5 of ADR 0006 § "Which closed claim domains the family may
   reach".

### 3.1 What terminates the census

Each resolved target must fall into exactly one of these, or the domain
refuses:

- **A dependency export with a receipt-closed claim for this domain.** The
  dependency's own `creates` is closed by an authenticated receipt whose policy
  digest and verifier build equal the parent's, per rule 5 of
  `2026-09-01-dependency-composition-scoping.md` § 4: a claim is never assumed
  transitively. A dependency whose `creates` is *open* terminates nothing and
  refuses.
- **A dialect primitive under an integrity-bound negative table.** See § 4.2,
  which now exists: `census_dialect_axiom_for_callee` answers this terminator
  for the `creates` domain and the audited Solid 2.0 archives, and refuses
  everything else. A 1.x Solid callee, and any domain other than `creates`,
  still falls through to the refusal below.
- **A reviewed default-library member.** `DefaultLibraryInvoker::from_wire`
  resolved the callee by default-library symbol identity, and the verifier's
  own table says the member performs no `create`. An unrecognized invoker
  string refuses rather than being trusted — the existing discipline in
  `argument_slot_is_proven_invoking` (`type_facts.rs:3856-3873`).
- **Recursion into a same-snapshot callee**, under `require_composed_operation_chain`'s
  discipline (`type_facts.rs:4642-4735`): a `visited` set that refuses a
  revisit, `MAX_COMPOSITION_DEPTH == 8` (`:4576`), refusal of a
  self-composition, and resolution by declaration *identity* — symbol, source
  file, exact byte range, plus the snapshot-replayed runtime export name —
  never by callee name.
- **`reach == Unreachable`**, which the floor excludes: a call the
  implementation provably never reaches performs nothing. `MayExecute` admits
  `Reachable` or `Unknown`; a `Reachable` floor (a cardinality lower bound
  above zero) admits only `Reachable`.

Anything else — a computed callee, a callee rooted in a reassigned binding, an
unresolved module, an external package with no accepted contract — refuses the
domain by name.

### 3.2 The callback rule

**A callee rooted at a parameter — caller-supplied — does not belong to this
export's `creates`, and its unresolvability does not refuse the domain.**

The export's act is the *invocation*, which is a `callbacks` item; what the
caller's function creates is created by the caller's code, in the caller's
artifact, under the caller's own contract. `createEffect` is the audited
authority: `creates: []` and `reads: []` are both closed while `initial-compute`
is `tracking: tracked`, which is coherent only under this rule.

Two consequences the census must implement rather than assume:

- "Rooted at a parameter" is `ImplementationCall::callee_parameter`, a
  `ParameterValueSource`, and it must be *that* fact — not an empty
  `callee_sources`, which means only that the producer traced nothing
  (`rust/crates/typefacts/src/invocation.rs:391, 411-432`). An untraced callee
  is unresolved and refuses; a callee proven to be parameter *N* is excluded.
- The exclusion covers the callable's body, not the export's handling of what
  it returns. If the export registers a returned cleanup, that registration is
  the export's own `cleanups` operation — `onSettled`'s `returned-cleanup` and
  `createEffect`'s `replace-cleanup`.

### 3.3 The probe-gate fixture: settled, and what the census slice owes it

**Settled, not open.** `fixtures/package-contracts/closed-domain-probe-gate`'s
`runCreatingOwner` (`index.js:25-33`) sets a module-level `currentOwner` to a
fresh object literal `{ disposals: [] }`, calls the caller's `callback`, and
restores the previous value in a `finally`. The package has no Solid dependency
at all. Under the settled definition in `semantic-model.md` § creates, an
object literal is not a `create` operation and a private module variable is not
a version-1 resource registered with any runtime — nothing in Solid's runtime
consults `currentOwner`, and `onCleanup` inside the callback would register on
Solid's ambient owner, not on it. The only call in either export is
`callback()`, a callee rooted at a parameter, excluded by § 3.2. So **both
`run` and `runCreatingOwner` census as `creates: []` closed**, and there is no
second reading to choose between.

That means the fixture's premise is wrong, not undecided. `index.js:13-18`,
`index.d.ts:15-18`, and `README.md:28-29` all assert that the two exports have
"opposite reactive-ownership behavior" and that this is why a `creates: []`
claim about either is refused. The refusal is real, but its cause is
`require_census_decides_closure` refusing the domain **by name** for want of a
census — not a behavioral difference between the two exports, of which, in
`creates` terms, there is none.

Two things the census slice owes this fixture:

1. **Correct those three comments and the README row.** State that the pair is
   indistinguishable to a `creates` census *because neither performs a `create`
   operation*, and that the domain is refused because no census premise exists,
   not because the exports differ.
2. **Add a sibling export that is a genuine positive**, so the census has
   something to refuse on and, later, something to certify against. It must
   call a real dialect primitive — e.g. `render` from `@solidjs/web`, whose
   audited summary publishes `register-delegation` — which requires adding a
   `node_modules/solid-js` (and `@solidjs/web`) stub to this fixture. Note the
   two standing traps: dialect selection follows the nearest
   `node_modules/solid-js/package.json`
   (`rust/crates/solid-facts-backend/src/dialect.rs`), and a new fixture
   `node_modules` is silently excluded from `git add` without its own
   `.gitignore` exception lines — so run coverage rather than trusting a
   `verify-delta` plan after touching it.

**ADR 0006's pin is unaffected.** The pinned behaviour — a `creates: []`
proposal refuses as `UnsupportedDemand` at witness acquisition, before any
probe is launched (`docs/adr/0006-probe-harness-binding.md:776-779`) — holds
exactly as written, because the refusal is domain-by-name and independent of
what the export does. Only the bullet's *characterisation* ("an export that
really does create an owner") becomes inaccurate under the settled definition,
and correcting that sentence travels with the fixture comments in the census
slice.

---

## 4. Prerequisite chain

Strictly ordered. Each step's output is the next step's premise.

### 4.0 Semantics — this document

`semantic-model.md` § "What a closed call domain denies" and § 2 above.

### 4.1 Producer

**Status after the producer slice (2026-09-03).** The two facts below **landed**
at handshake protocol 14 (schema digest
`sha256:0d246a6cf7682e3f756df3ce54569cfca2dfeb57006a3198dabad51e43f96fc4`), and
the third bullet — the `complete` rename — was **deliberately not taken**.
`docs/typefacts/adr/0026-v1-uncensused-invoking-forms-and-local-declaration-transcripts.md`
carries the decision, the kind vocabulary, and the limits; the dated entry in
`docs/precision-backlog.md` carries the precision status. Per bullet:

1. **`complete`: measured and documented, not renamed.** The seven gates are
   now written out in the Rust doc comment on
   `ExportImplementationTranscript::complete` and in ADR 0026, together with
   the sentence that matters — `complete` says nothing about the calls census
   being total, and a consumer that read it as an enumeration guarantee would
   be unsound. The field keeps its name: renaming a field every consumer reads
   is a larger break than this slice takes, and the guarantee now has its own
   field, so nothing depends on the name to be sound. **Partially discharged.**
2. **Markers: done, with a refusing default.**
   `ExportImplementationTranscript.uncensusedInvokingForms` carries one row per
   invoking form the call census does not record, over twelve closed kinds. The
   obligation this bullet actually states — "classify every form it walks and
   emit a refusing marker for any form it cannot classify" — is met by the
   classifier's *default*: a token invokes nothing, a named kind gets its kind,
   a kind on the reviewed `nonInvokingNodeKinds` list gets no row, and
   **everything else** is `unclassified-invoking-form` carrying the compiler's
   node-kind name. Two forms are absent by decision rather than omission: a
   `Proxy` trap is a property of the runtime value rather than of syntax and is
   outside any producer census, and `f?.(x)` is already a `CallExpression`.
   **Done.**
3. **Transcripts for non-exported local declarations: done.**
   `ExportValueDemand.localDeclarationLocation` names a function-like
   declaration by exact source range and is answered by
   `ExportValueTranscript.localDeclaration`. Identity is bound by the
   **resolved** declaration lying inside the demanded span — the answer's own
   location is the demand echoed back, and binds nothing on its own. **Done,
   and not wired to any census.**

A same-day review pass then corrected the slice, without moving the protocol:
`await` was failing open on any type whose `then` the checker could not find
(a union, an unconstrained type parameter, an index signature); destructuring
*assignment* was recording nothing at all, because `[a, b] = src` and
`({ a } = src)` are the same node kinds as the value forms and all of them sat
on the non-invoking list; the accessor premise was reading `.d.ts` declarations
as if they were runtime bytes; the identity binding above was a tautology; and
a demand naming a declaration file was refused by span rather than by file. The
schema digest moved with those edits and the reviewed list is now 123 entries.
`docs/precision-backlog.md`'s "Review-fix pass" section is the record.

What is **not** done, and blocks § 4.3: no consumer reads either field. The
census predicate of § 3 remains unwritten, and
`require_census_decides_closure` refuses every behavioral call domain by name,
unchanged.

- **`complete` does not mean what a census predicate needs it to mean.**
  `exportImplementationTranscriptLocked`
  (`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go:191-251`)
  reaches `transcript.Complete = true` (`:249`) only after clearing seven
  separate gates, each of which appends an `OpenReasons` entry and returns
  early instead: the source file is available (`sourceUnavailable`, `:199`);
  the node at the queried location is an **exact identifier**
  (`identifierNotExact`, `:205`); `GetSymbolAtLocation` resolves it
  (`symbolUnresolved`, `:211`); `canonicalSymbol` resolves the alias chain to a
  target (`aliasUnresolved`, `:216`); the value type has **exactly one** call
  signature (`callSignatureNotUnique`, `:223`); the selected signature has an
  implementation declaration **with a body** (`implementationUnavailable`,
  `:229`); and that implementation has a resolved declaration
  (`declarationUnavailable`, `:234`). Only then is the control-flow census
  consulted, and `Complete` is withheld if `ControlFlow.Unsupported` is
  non-empty (`controlFlowUnsupported`, `:246`).

  So `complete: true` is a *conjunction* — exact identifier, resolved symbol,
  canonical target, unique call signature, available body, resolved
  declaration, and a fully censused control flow — which is considerably more
  than "no unsupported branch". What it still asserts **nothing** about is
  invoking forms: the call census (`implementationCallCensusLocked`, `:346`)
  records only `ast.IsCallExpression` and `ast.IsNewExpression` (filter at
  `:373-375`). Reading `complete` as an enumeration guarantee over invoking
  forms would be unsound. Relax the field to an explicit spelling —
  `resolved_with_control_flow_censused`, or `control_flow_only_open` — and give
  the enumeration guarantee its own field, so a consumer cannot mistake one for
  the other.
- **Explicit markers for every non-censused invoking form**, so the census
  refuses by name instead of concluding from silence: tagged templates,
  getters and setters reached by property access, decorator applications, the
  iteration protocol (`for…of`, `for await…of`, spread, array destructuring,
  `yield*`), `using` and `await using` scope exit reaching `Symbol.dispose` or
  `Symbol.asyncDispose`, `instanceof` reaching `Symbol.hasInstance`, JSX
  elements and `html` templates, `await` on a non-native thenable, coercions
  reaching `Symbol.toPrimitive`/`valueOf`/`toString`, and `Proxy` traps.
  **That list is open-ended too**, per the fourth shared rule of
  `semantic-model.md` § "What a closed call domain denies": the producer's
  obligation is not to implement this enumeration but to **classify every form
  it walks and emit a refusing marker for any form it cannot classify**, so
  that a form nobody has thought of yet refuses rather than passes. A
  classifier whose default is "ignore" fails this requirement however long its
  list. Per-domain cost: `creates` and `callbacks` need all of them; `reads`,
  `writes`, and `invalidates` additionally need the proxy property-access and
  assignment forms of that section; `returns` needs the generator and `async`
  completion forms (and, per the § returns decision, must *not* treat a
  valueless completion as a return); `disposals` and `cleanups` need the
  `using` forms; `throws` is unbounded and is not a census target (§ 4.5).
- **Transcripts for non-exported local declarations.** The census recurses into
  same-snapshot callees, and most of them are module-local functions the
  current transcript surface only reaches through an export.

### 4.2 Dialect

**Status after the dialect slice (2026-09-03): landed, narrower than this
section asked for, and unwired.** `docs/adr/0007-census-dialect-axiom-tier.md`
is the decision and carries the reasoning; the dated entry in
`docs/precision-backlog.md` carries every row, every citation, and every
deliberate silence. What exists:

- `solid-dialect`'s `DialectNegativeAuthority` — per dialect, the audited
  archive tuples (name, version, SRI, and the archive's own `package.json`
  digest, all four taken verbatim from the audited document's `package` block)
  and the rows those audits deny, each row citing the document and the byte
  range of the summary object it was read from. A test re-reads the range and
  re-derives the closure, and a second test derives the whole table from the
  documents and requires the shipped set to equal the derivable set minus a
  named withholding list.
- `solid_dialect::primitive_performs_no_operation(archive, export, domain)` —
  the shared free function, taking the caller's already-bound `&AuditedArchive`
  tuple (not a bare name) and unioned across the dialects that audited *that
  exact archive*, with cross-dialect agreement and silence never "no".
  Documented as a **negative** authority admissible only as a census
  terminator.
- `census_dialect_axiom_for_callee`
  (`contract_certification/type_facts.rs`) — the tier, a peer of
  `argument_slot_is_proven_invoking`'s Tier A, with the witness spelling below
  and eight separate refusal conditions. **Not wired into
  `require_census_decides_closure`**, which still refuses every behavioral call
  domain by name.

Three narrowings, each a decision with its evidence in ADR 0007:

1. **`creates` only.** The other seven kinded domains are withheld wholesale.
   `returns` has a systematic conflict with § returns (`snapshot` is
   `shape: "plain"` and closes `returns: []`), `callbacks` has a demonstrated
   counter-example (`latest` closes it while calling its argument, which this
   repository already records as an intentional audit divergence), and the
   remaining five are simply unaudited for this purpose. `throws` has no
   variant at all: it constrains no operation kind, so "publishes no operation
   of kind X" has no X for it.
2. **Solid 2.0 only.** The Solid 1.x table is empty. The nineteen
   `pkg/contracts/bundled/solid-v1/*.json` documents close `creates: []` for
   all 129 exports, and that closure was introduced by the `474c101f`
   migration over a domain the preceding schema-1 hand audit had **no field
   for** — schema 1 carried `kind`, an optional `returns` shape, and
   `callbacks`, and nothing else. Two structural tells confirm it: every
   solid-v1 summary closes exactly the generator's four domains and omits the
   five it hard-wired to `Unknown` (§ 1.1), and `createSignal`'s summary
   carries `shape: "callable"` for a tuple-returning function.
3. **`hydrate` withheld though the audit closes it.** rc.3's `hydrate` reaches
   `render` on every path, and `render` calls `registerDelegatedRoot(element)`
   unconditionally before it opens its root — preceded only by a throw guard,
   `resetErrorHalt()`, `enforceLoadingBoundary(true)`, and a `let disposer;` —
   the act the sibling summary models as the `create` operation
   `register-delegation`. The audit contradicts itself within one document; the
   bytes side with `render`.

Item 4 below (the corpus fixture) is **not** taken and is re-scoped: it existed
because the *positive* axiom's identity derivation was unreachable from a unit
test, `CertificationPlan` being unconstructible here. This tier takes the
archive under certification as an `ArtifactSnapshot`, so the whole four-field
identity gate is unit-tested against the audited `package.json` bytes checked
in under `benchmarks/package-contract-v2/phase0/rc3/`. A corpus fixture is owed
once the census *consumes* the terminator, because only then can a row's
verdict move.

The section as originally written:

A **negative** table per audited dialect package: for an exact
`name@version` bound by the audited SRI, per canonical export, per claim
domain, "this export performs no operation in this domain". It is a peer of
`argument_slot_is_proven_invoking`'s Tier A
(`type_facts.rs:3856-3866`) — same position in the tier list, same
membership-reviewed discipline, same refusal of anything unlisted — and it must
satisfy the "sound form of the premise" in
`docs/adr/0005-dialect-axioms-about-the-dialects-own-package.md`:

1. **Integrity-bound tuple.** name, version, *and* the audited SRI compared
   against `SnapshotedPackage::package_integrity`
   (`contract_certification.rs:1487-1490`), field-by-field with a
   naming-the-field disagreement, mirroring
   `contract_certification/dependencies.rs:1222-1240`. A snapshot matching the
   coordinate but not the integrity refuses and says which field disagreed.
2. **Version keyed to the actually-audited runtime bytes**, with the dialect's
   own citations naming the same bytes (ADR 0005 precondition 2, whose
   line-number correction is measured but not applied).
3. **`floor == MayExecute` only**, with `floor` in scope at the decision site,
   and a pinned refusal under `ReachabilityFloor::Reachable`.
4. **A contract-corpus fixture exercising `from_plan`**, which is also the
   live demonstration that the corpus's fabricated
   `fixture:sha256:<manifest digest>` (`scripts/contract-corpus.mjs:60`) is
   *refused* by an integrity-bound gate.

Witness spelling: `census-dialect-axiom:<pkg>@<ver>#<sri>:<export>:<domain>`.

This table is a *negative* premise only. ADR 0005 stays `deferred`: nothing
here grants a positive discharge, and the generator-side withholding for a
dialect's own primitive-defining archive
(`GenerationScope::DialectDefiningPackage`, `inferred_contract.rs:112-145`)
remains a one-directional withholding, not identity.

### 4.3 Census for `creates`, `GenerationScope::ConsumingPackage`

**Status after the census slice (2026-09-04): landed.**
`docs/adr/0008-implementation-census-for-creates.md` is the decision. What
exists, against § 3:

- `census_creates_domain` (`contract_certification/type_facts.rs`), reached
  from `require_census_decides_closure`'s new `ClosureCensus::Implementation`
  arm for `ClaimPath::Call(ClaimDomain::Creates)` **only**; every other call
  domain keeps refusing by name (§ 4.4-4.5).
- § 3 item 1: the demanded export's `ExportImplementationTranscript`, complete
  with an **empty control-flow `unsupported` list at every depth** and no
  `break`/`continue` inside the bound declaration node — no
  `controlFlowUnsupported` relaxation, because the producer withholds call rows
  a jump makes non-universal and the marker is their only trace (ADR 0008 item
  0; over-refuses loop/switch/try frames until the producer emits withheld
  rows) — at handshake protocol 14 so a present-and-empty
  `uncensusedInvokingForms` is the producer's positive claim;
  item 4: any uncensused form the `MayExecute` floor admits refuses by kind and
  location; item 2: every `calls` row at the floor (Unknown reach in) gets
  exactly one of the five § 3.1 terminators — `unreachable`,
  `parameter-rooted` (§ 3.2, `callee_parameter` only), `standard-library`
  (admitted only when the member transfers control to no callable the census
  cannot see: reviewed invoking slots and callable-carrying slots must be
  parameter-rooted or literals inside the frame; `Function.*`,
  `CallableFunction.*`, `NewableFunction.*`, `Reflect.apply`/`construct`,
  `eval`, `Function` refuse by name), `dialect-axiom` (§ 4.2's tier),
  `local-recursion` (a named declaration whose binding nothing writes or
  redeclares; an arrow a `const` holds refuses) — or the domain refuses on the
  first call with none; item 5: a candidate whose `creates` item set is
  non-empty refuses. The "dependency export with a receipt-closed claim"
  terminator is **not** implemented: § 4.5's accepted-dependency disposition
  has not been taken, so a resolved dependency export without an audited
  negative row refuses by name.
- Local recursion mirrors `require_composed_operation_chain`: identity by
  symbol + source file + exact span, a visited set seeded with the demanded
  export, cycle refusal, `MAX_COMPOSITION_DEPTH` (8) hops. The helper's
  transcript is acquired through `ExportValueDemand.localDeclarationLocation`
  in the same pinned session, one batch per depth; the verifier binds a named
  declaration to its node span by its own Oxc parse of the authenticated bytes,
  and the producer refuses an answer whose resolved declaration lies outside
  the demanded node.
- Witness: one `census-call:` site per call, one `census-local-declaration:`
  site (with the transcript's SHA-256) per recursed helper,
  `census-uncensused-forms:0`, `census-total:{calls}:{depth}`; same
  `DomainExhaustiveness` evidence-root envelope, `POLICY_DIGEST` unmoved.
- The generator proposes the candidate again — `creates: Complete([])` for a
  consuming package's function export whose own IR walk finds no call that a
  closed `creates` would contradict (`CreatesProposalWalk`) — and **recipe-gated
  planning** (`CertificationPlan::recipe_gated`) withholds a candidate whose
  claim has no recipe in the supplied corpus before any demand or gate exists,
  so § 5's "every newly reachable candidate needs a probe recipe or refuses by
  name" is met without a refused row: the domain stays open and the withholding
  is named in the certification audit.
- § 3.3's two debts: the fixture's comments and README row are corrected
  (`run` certifies; `runCreatingOwner` refuses on its `try` marker, an
  over-refusal ADR 0008 records), and the sibling positive is
  `closed-domain-probe-gate/primitive-consumer/` (`runAfterSettle` →
  `onSettled`), which the census refuses by name.

The section as originally written:

§ 3 above. Reuses `require_composed_operation_chain`'s visited-set, depth-8,
declaration-identity discipline; derives its own enumeration and refuses when
it disagrees with the proposal's item set.

### 4.4 Census for `reads`

Same machinery, plus the proxy property-access forms, which is why it follows
rather than accompanies `creates`.

### 4.5 Then, in order

- **Accepted-dependency disposition.** Break A and Break B of
  `2026-09-01-dependency-composition-scoping.md` § 3.3: no repository gate ever
  engages the accepted lane (`contract generate` defaults
  `acceptedDependencies` to `{}`), and
  `VerifiedDependencyComposition::authenticate` hashes the demand's
  `semantic_claim_id` without resolving it (`dependencies.rs:1830-1842`).
  Until both are closed, the "dependency export with a receipt-closed claim"
  terminator of § 3.1 cannot be exercised.
- **`throws` and `returns`.** `returns` is control-flow-decidable and is the
  cheapest of the nine. `throws` is not a census target at all under version 1
  (see § "What a closed call domain denies"): the model has no `throw`
  operation kind, no document carries a positive item, and every expression
  form can throw. It is a model question before it is a census question.

---

## 5. Measured cost

- **216** `{kind: "call", domain: "creates"}` closure candidates and **185**
  `{kind: "call", domain: "reads"}` candidates, across **56** fixtures each, in
  `fixtures/package-contracts/**/expected-proposal.json` (80 proposal files, 66
  with any candidate). Sibling counts for scale: `returns` 186, `callbacks`
  181, `owner-productions` 16, `object-properties` 13, `tuple-items` 3.
- Every newly reachable candidate needs a probe recipe or refuses by name:
  a scheduled gate with no recipe is `MissingGate` (ADR 0006 § Recipes), and
  the corpus is an input rather than a root of trust, so nothing about the
  refusal can be waived by omitting the gate.
- **Consumer recipes are unprobeable today.** The private probe directory is
  populated as `node_modules/<package-name>/…` — the analyzed package's
  snapshot copy and nothing else — plus `harness/` and `recipes/`
  (`rust/crates/solid-facts-backend/src/contract_certification/probe_harness.rs:1580-1600`).
  A consumer package's recipe cannot import the package under test, because
  the package's own `import "solid-js"` resolves to nothing inside the private
  layout, and any resolvable ancestor `node_modules` refuses the gate outright
  by design. Until the private workspace carries the *authenticated dependency
  closure*, `creates` closure for a consumer package is reachable only where
  the veto schedule is empty, which is where a `MissingGate` refusal applies
  instead.

---

## 6. The row added to the Phase 19 demand-authority audit

**Added by the producer slice (2026-09-03), together with the test edit it
requires.** The row below is now
`domain-exhaustiveness/implementation-census` in
`docs/package-contract-v2/phase19/proof-demand-authority-audit.json`, with its
`producerField`, `completenessGuarantee`, and `policy2Gap` rewritten to
describe what landed rather than what was missing. Three pins in
`scripts/package-contract-phase19.test.mjs` moved with it, each with a comment:
`demands` 43 → 44, `producerExtensionRequired` 4 → 5, and
`certificationReadyFamilies` **7 → 6**.

That third move was not anticipated by this section and is the interesting one.
`domain-exhaustiveness` was counted certification-ready because both its rows
were "already exact", even though closing a behavioral call domain had no
enumeration guarantee over invoking forms anywhere in the system — which is
precisely what `require_census_decides_closure` says by refusing every such
domain by name. Adding an honest "producer extension required" row to the
family corrects the count. It is a correction, not a regression.

`families` and `policyDigest` are **unmoved**: the row joins an existing
family, so `proof-policy-v2.json`'s applicability table is untouched and no
receipt's policy binding is re-labelled. A *new family* would have moved the
digest, which is a separate cut and was not taken.

The original text of this section, for the record: the brief allowed a row
only if the gate tolerated an additive row without moving pins. It does not —
so the row was recorded here and deferred until a slice could take the
accompanying JavaScript edit, which this one did.

`scripts/package-contract-phase19.mjs`'s `auditPhase19DemandAuthority`
(`:127-203`) validates each row structurally and requires every policy family
to be represented, and it returns counts. Those counts are pinned literally:

```js
// scripts/package-contract-phase19.test.mjs:121
assert.deepEqual(auditPhase19DemandAuthority(), {
  demands: 43, families: 18, alreadyExact: 32,
  producerExtensionRequired: 4, unsupported: 7,
  certificationReadyFamilies: 7
});
```

(that block is the *pre-slice* state; it now reads `demands: 44`,
`producerExtensionRequired: 5`, `certificationReadyFamilies: 6`.)

and `scripts/package-contract-v2-phase19-report.mjs:193` asserts
`authority.demands === demandAuthority.demands.length`. An additive row moves
`demands` from 43 to 44 and one status count, failing that test — a JavaScript
change the *semantics* slice could not make, which is why the item waited for
the producer slice. A *new family* is worse: families come from
`proof-policy-v2.json`'s applicability table, whose `policyDigest`
`sha256:f0dfd235055d1aba95f1de513eeee8109178a186fb2be3901d1f3092a42bb278` is
pinned in the same test, and changing it re-labels every receipt's policy
binding. The row added joins an existing family, so that digest did not move.

The row as drafted here, before the producer slice rewrote its three prose
fields to describe what landed:

```json
{
  "id": "domain-exhaustiveness/implementation-census",
  "family": "domain-exhaustiveness",
  "status": "producer extension required",
  "producerField": "ExportImplementationTranscript.complete/calls",
  "source": "apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go",
  "completenessGuarantee": "none yet: `complete` is set whenever the control-flow census has no unsupported branch, and the call census records only CallExpression and NewExpression, so it carries no enumeration guarantee over invoking forms.",
  "policy2Gap": "Closing a behavioral call domain needs an enumeration guarantee over every invoking form plus transcripts for non-exported local declarations; until both exist require_census_decides_closure refuses the domain by name."
}
```
