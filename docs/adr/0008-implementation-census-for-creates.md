---
status: accepted
---

# The implementation census for `creates`, and recipe-gated planning

Date: 2026-09-04

## The gap

ADR 0006 admitted `ProofFamily::DomainExhaustiveness` and then let
`require_census_decides_closure` refuse every behavioral call domain by name:
the censuses that discharged the family were censuses of the **declaration**,
and two exports with byte-identical declarations have byte-identical
declaration censuses. Closing `creates: []` on that evidence would have left a
probe's finite non-observation as the only thing separating a certified row
from a refused one.

The prerequisite chain of
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
then landed one link at a time: the domain's meaning
(`semantic-model.md` § creates — the published `create` operation, registering a
version-1 resource into a runtime outside the invocation), the producer's
enumeration guarantee over invoking forms and its local-declaration transcript
(ADR 0026, handshake protocol 14), and an integrity-bound negative table for
Solid 2.0 primitives (ADR 0007). Nothing consumed them. `exportsProven` stayed
0 of 3410.

## Decision

**Add the implementation census as a proof mode for the `creates` call domain
of a consuming package, and gate its planning on the recipe corpus.** Three
parts, each with an owner.

### 1. The generator proposes the candidate again — as a proposal

`inferred_contract.rs`'s `normalize_export` emits `creates: Complete([])` for a
`GenerationScope::ConsumingPackage` **function** export whose implementation the
generator's own IR walk cleared: `solid_reactive_ir::CreatesProposalWalk`,
which refuses (and so does not propose) on any call whose callee this build
cannot resolve to a symbol, any canonical dialect primitive for which no
dialect's audited negative authority carries a `creates` denial
(`solid_dialect::some_audit_denies_primitive`, deliberately a *name-level*
read that can only gate a proposal), and any callee bound to an accepted
dependency contract that does not close `creates` empty. A dialect-defining
archive keeps `Unknown`, as before.

`normalize_knowledge` weakens the empty `Complete` into a closure **candidate**.
A proposal is a claim to be proven, not a proof: the generator's word certifies
nothing, and the census below may refuse what it proposed. Silence — an
unresolved callee, an unaudited primitive — is "do not propose", never "close".

### 2. The census

`census_creates_domain` (`contract_certification/type_facts.rs`) discharges the
`DomainExhaustiveness` demand for `ClaimPath::Call(ClaimDomain::Creates)`, and
that arm only; `require_census_decides_closure` gained a
`ClosureCensus::Implementation` variant admitting exactly that path. The
predicate is the plan's § 3, stated as the negation the census establishes:

> `creates: []` certifies only if every invoking form in the export's
> transitive census, taken at the `MayExecute` reachability floor, is
> enumerated and resolved, and no resolved target performs a `create`
> operation.

Concretely, for the demanded export's `ExportImplementationTranscript`
(present and **complete, with an empty control-flow `unsupported` list** — no
`controlFlowUnsupported` relaxation, see item 0 — with its authenticated runtime
binding checked by `require_export_implementation` like every other
implementation-reading family):

0. **No withheld row can hide in the transcript.** The producer drops every
   `calls` row that lies in a region a `break` or `continue` makes
   non-universal — the whole target subtree of a `break` in a loop or `switch`,
   the body of a loop a `continue` sits in (`unsafeJumpRegionsLocked`,
   `locationWithheldByJump`). For the positive families that is the safe
   direction; for a zero upper bound it is the failure mode: `switch (kind) {
   case "mount": render(App, el); break; }` yields no row for `render`, the
   dropped call is a `CallExpression` so the uncensused-form census is silent
   too, and the only trace is the `switchReachability` marker. So
   `census_transcript_is_censusable` requires `complete` with an empty
   `control_flow.unsupported` at **every** depth of the recursion, and
   `census_transcript_frame` binds each transcript to its declaration node in
   the verifier's own Oxc parse of the authenticated bytes and refuses a node
   containing any `break`/`continue` — nested callables included, because the
   producer's control-flow census never enters one, so a jump there leaves no
   marker while still withholding rows. **This over-refuses every export whose
   frame has a loop, a `switch`, or a `try`** even where nothing was withheld
   (`closed-domain-probe-gate`'s `runCreatingOwner`, whose `try … finally`
   withholds nothing, now refuses on `tryReachability`). The proper fix is
   producer-side — emit a withheld row with `reach: unknown`, or as an
   uncensused form — and is the next producer slice; it is not taken here.

1. **Handshake protocol ≥ 14**, or refuse. Serde cannot separate an absent
   `uncensusedInvokingForms` from a present empty one, so the protocol is the
   discriminator that makes the empty list the producer's positive claim.
2. **Any uncensused invoking form whose reach is not `Unreachable` refuses by
   name**, kind and location in the reason. A tagged template, a spread
   argument (iteration protocol), an accessor, a JSX lowering — anything the
   call census does not record — reaches a callable the census cannot
   disposition.
3. **Every `calls` row at the floor gets exactly one disposition**, in this
   order, and the first row with none refuses by name:

   ```rust
   enum CensusDisposition {
       Unreachable,      // reach == Unreachable; MayExecute admits Reachable and Unknown
       ParameterRooted,  // callee_parameter is Some — an empty callee_sources is NOT this
       StandardLibrary,  // declaration.standard_library
       DialectAxiom,     // census_dialect_axiom_for_callee (ADR 0007), carrying its own site
       LocalRecursion,   // declared in this artifact's own runtime source set
   }
   ```

   The `standard-library` premise, stated: a default-library member cannot
   register a version-1 resource into a Solid runtime, **and it transfers no
   control to a non-censused callable**. `lib.*.d.ts` is the engine's
   description of itself; a `create` is a registration of a version-1 resource
   kind into a runtime that acts on it; the engine has no such kind to register
   and no Solid runtime to register it with. The second half is what
   `census_standard_library_admits` establishes, because the first half says
   nothing about user code the engine runs on the census's behalf:

   - **By reference or by text, refused by qualified name.** `Function` /
     `FunctionConstructor`, every member of `Function`, `CallableFunction`,
     `NewableFunction` (`call`, `apply`, `bind`), `Reflect.apply`,
     `Reflect.construct`, and `eval` run a value or a string that is not an
     argument *slot*. This is a **denylist beside a reviewed allowlist**, and
     the split is deliberate: the allowlist is the producer's own reviewed
     invoker table (`invoking_positions.go`, read back through
     `DefaultLibraryInvoker::from_wire`), which answers *which slot* a member
     invokes and so can demand a proof per slot; it has no row shape for "the
     receiver" or "the text", and a full allowlist of every default-library
     member that transfers control to nothing would be a review of the whole
     library that nobody has done. So reviewed rows prove slots, these names
     refuse outright, and everything else is admitted only under the next two
     rules.
   - **Every reviewed invoking slot must be proven.** When the producer names
     the member in its invoker table (`default_library_invoker`,
     `invoked_arguments`) — `forEach`, `map`, `then`, `setTimeout`,
     `new Promise(executor)`, … — each slot the table says it invokes is either
     rooted at a parameter of this implementation (`argument_parameters[slot]`,
     the caller's code under the § 3.2 rule) or a callable literal whose every
     `argument_callables` location lies inside the transcript's own frame,
     where its calls are rows of this very walk. A slot the producer traced to
     nothing — an imported `render`, a module-local `function work` (the tracer
     follows `const` bindings only), a member read — refuses; an invoker string
     outside the reviewed table refuses.
   - **Every slot the producer saw a callable in must be proven the same way**,
     whether or not the member is a reviewed invoker: `Array.from(items,
     mapFn)`, `JSON.parse(text, reviver)`, `text.replace(re, fn)` invoke or
     store what they are handed and the census can prove neither.

   `Array.prototype.map(callback)` in the fixture is therefore admitted because
   `callback` is parameter-rooted, not because the callback's body is
   "dispositioned where it is written" — a body outside the frame is not
   dispositioned at all. **What this does not close, stated:** a member may
   reach user code through a *protocol method* on a value it is handed or
   receives — `JSON.stringify(o)` → `o.toJSON`, `Array.from(iterable)` →
   `iterable[Symbol.iterator]`, `arr.sort()` → element `toString`,
   `Promise.resolve(thenable)` → `then`. The producer classifies the operator
   and template spellings of that reach (`coercion`, `iteration-protocol`) but
   not the call spellings, and this side has no fact about a non-callable
   argument's shape to refuse on. That is an open producer-side gap, recorded
   in `docs/precision-backlog.md`, not a premise this disposition claims.
   `Construct` calls follow the same dispositions; an absent call kind refuses.

   Arguments and spreads do not matter for `creates`: a call is dispositioned by
   its **callee**. A spread refuses anyway, because it is an invoking form of
   its own (item 2), not because it is an argument.

4. **Local recursion** mirrors `require_composed_operation_chain`: identity by
   symbol + source file + exact span, never by name; a visited set seeded with
   the demanded export; a revisit refuses as a cycle; `MAX_COMPOSITION_DEPTH`
   (8) hops refuses rather than approximating. The callee's declaration must
   strip to the artifact's own (non-dependency) snapshot root and be a path the
   verified closure manifest calls runtime source — a declaration file the
   archive ships is a description of code, not code. Its transcript is acquired
   through `ExportValueDemand.localDeclarationLocation` in the **same pinned
   session**, one batch per depth (`acquire_census_local_transcripts`);
   verification, which has no session left to ask, refuses a declaration that
   was not acquired.

   The producer resolves a named function to its *identifier* and answers a
   local-declaration demand only for the exact declaration **node**. The
   verifier binds identifier to node by its own Oxc parse of the authenticated
   runtime bytes (`census_local_declaration_node`: a node whose span is the
   resolved span, else the one node whose name span is), refusing on no node or
   two. This only chooses what to *ask*: the producer refuses an answer whose
   resolved declaration lies outside the demanded node
   (`declarationIdentityUnbound`), and the census keys every lookup by the node
   it demanded. The parse is over the text the producer counted offsets on: a
   leading UTF-8 byte-order mark is **stripped before the parse**, because
   typescript-go's file decoder (`internal/vfs/internal/internal.go`,
   `decodeBytes`) removes it before the source text exists, so every producer
   `Location` counts from the first byte after the mark; parsing the raw bytes
   would put the verifier three bytes behind on a BOM'd file and bind — or
   refuse — the wrong node. Identity stays exact; nothing is offset.

   **The binding must be proven to hold the declaration**
   (`census_local_binding_is_stable`). The producer's answer is a fact about
   the file as bound — which declaration the identifier's symbol has — and the
   census reads that declaration as the code the call *runs*, which is a fact
   about the binding's value at the call. `function helper() {} … helper =
   (el) => render(App, el); … helper()` separates the two. So, from the
   verifier's own facts over the authenticated bytes, a local-recursion callee
   refuses by name and location when anything **writes** its binding — an
   assignment or update expression whose target contains a reference to it
   (`AstFacts::assignments` + `reference_declarations`), or a `for…in`/`for…of`
   head that assigns it (the new `AstFacts::iteration_targets`) — or when the
   same name is **declared again** in the file (another function declaration, a
   variable declarator, a class), which the binder merges into one symbol whose
   running declaration the census cannot choose.

   **An arrow or function-expression helper is not recursed into.**
   `const helper = () => …` resolves, on the producer's side, to the arrow
   node itself; the verifier binds that node (its span is the resolved span),
   and the node has no binding identifier of its own — `FunctionFact::name` is
   `None` — so `census_local_binding_is_stable` refuses it by name: what runs
   is whatever the variable holds, and this census does not trace variables.
   Pinned by `creates_census_refuses_a_local_binding_that_is_written_redeclared_or_anonymous`.
5. **The proposal's `creates` item set must be empty**, the sibling of
   `require_export_value_enumeration_matches_census`: a candidate is empty by
   construction, and a nonempty one claims a closure over operations the census
   never enumerated (ADR 0006 objection 5).

**Witness.** This family is universal, not existential: every call carries
`census-call:{path}:{start}:{end}:{kind}:{reach}:{disposition}` (the dialect
disposition carries the tier's own `census-dialect-axiom:` site instead), every
recursed helper carries
`census-local-declaration:{path}:{start}:{end}:{symbol}:sha256:{transcript}`,
and `census-uncensused-forms:0` plus `census-total:{calls}:{depth}` close the
census. Sites are sorted and deduplicated. The evidence-root envelope is
unchanged and `POLICY_DIGEST` did not move.

### 3. Recipe-gated planning

A closure candidate the census **proves** spawns a mandatory probe veto, one
per candidate (`probe_gates.rs`), and a scheduled veto with no recipe in the
corpus refuses the gate — and therefore the row (`MissingGate`). Returning
`creates` candidates to every consumer proposal would have turned every real
row into a refused one, because consumer recipes cannot yet be written (below).

`CertificationPlan::recipe_gated(corpus)` therefore runs **before** Type Facts
acquisition, in `certify_value_only`, `certify_value_only_case_set`, both
published-graph lanes, and the CLI's planning output. For every `creates`
candidate whose semantic claim id the supplied corpus names no recipe for — or
when no harness is configured at all — it opens the domain in the selected
proposal, re-runs the policy's own candidate inventory, demand derivation, and
artifact-witness derivation over the weakened proposal, and records a
`WithheldClosure { artifact_case, export, domain: "creates", semantic_claim_id,
reason: "no recipe in corpus" }`. The finalized contract carries the records;
`main.rs` prints one `solid-checker:withheld-closure=` line per record;
`certify-contract.mjs` writes them into the certification audit's
`withheldClosures`, and the ecosystem report's `certificationAttempt` gains an
additive `withheldClosures` count.

**Graph lanes: identities stay, composition proves the weakening.** Gating a
dependency node changes the proposal its receipt certifies but **not** its
canonical node identity or the graph root. The alternative — rebinding the
node's `semantic_digest` to the gated digest — was rejected because the parent's
closure edge names the dependency proposal it was generated against
(`accepted_contract_digest`), that digest is hashed into every dependency
demand of the parent's demand graph, and the edge lives in the parent's
authenticated closure manifest; none of that may be rewritten by a gate on the
dependency. So `PlannedGraphNode` keeps the `accepted_candidate` it was planned
with, and `authenticate_dependency_receipt` requires: the identity's digest is
the edge's accepted digest; the accepted proposal weakened by the node's
withheld records (`withheld_weakening`, the one definition both the gate and
composition use) has exactly the receipt's `semantic_digest` and the receipt's
own binding digest; and the plan actually certified is that same document. A
parent demand that *relied* on a withheld closure still refuses on its own
(`DependencyClosure` → `MissingClosedClaim`). Pinned by
`a_gated_dependency_receipt_composes_as_the_exact_weakening_of_the_accepted_contract`
(no producer) and
`published_graph_with_a_withheld_dependency_candidate_certifies_end_to_end`
(pinned producer). Before this correction every graph whose dependency node had
a withheld candidate failed `ReceiptMismatch`.

**The generator's gate follows local call edges to a fixpoint.**
`CreatesProposalWalk` is lexical over each export's span, so `export function
f() { helper() }` beside `function helper() { createSignal() }` would have
proposed for `f` and been withheld or refused later by name. It now follows the
IR's resolved call edge (`callee_symbol` → `function_for_symbol`) and marks a
call into a function whose span contains a refusing call as refusing itself,
iterated to a fixpoint. Still a proposal input; the census decides the same
callee again against authenticated bytes. Eleven candidates across nine corpus
fixtures were withdrawn by this (listed in `docs/precision-backlog.md`).

**Why this is not a weakening.** Nothing that could certify closed before is
lost: a candidate with a recipe is planned, censused, vetoed, and certified
exactly as before; a candidate without one was never going to close — it could
only refuse a row whose every other claim was proven. What moves is where the
missing recipe shows up: as a named, audited withholding with the domain
**open**, instead of a refused row. The domain really is open: the canonical
main the receipt binds is encoded from the weakened proposal, the demand graph
the receipt names is derived from it, and no demand ever claimed the closure.
Nothing edits a demand graph in place. Only `creates` is gated, because it is
the only behavioral call domain with a census; every other call domain still
refuses by name at witness acquisition, before any gate is consulted.

## What still refuses

- **The actual next blocker on real rows: no Solid 2.0 negative row for
  `createSignal`, `onCleanup`, `untrack`, `getOwner`, or `createRoot`**
  (`rust/crates/solid-dialect/src/solid_2.rs`: the `creates` rows are `action`,
  `createMemo`, `onSettled`, `createEffect` and the other audited exports; those
  five have none). Almost every real consumer export calls one of them, so the
  generator's walk falls silent and **no candidate is proposed** — before any
  recipe, workspace, or census question arises. Filling that is an audit,
  recorded as an open item, not a census change.
- **Every export whose frame carries a control-flow marker** — a loop, a
  `switch`, or a `try` at any depth of the recursion, and any `break`/`continue`
  inside the bound declaration node, nested callables included — refuses even
  where no row was withheld (item 0). Producer-side fix pending.
- **A standard-library member handed a callable the census cannot see**, and
  the by-reference members (`Function.*`, `CallableFunction.*`,
  `NewableFunction.*`, `Reflect.apply`/`construct`, `eval`, `Function`) —
  including a module-local `function work` handed to `forEach`, which the
  producer's argument tracer does not follow. The protocol-method reach
  (`toJSON`, `Symbol.iterator`, element `toString`, `then`) on a non-callable
  argument is **not** refused and is an open producer-side gap.
- **A local declaration with no binding identifier** (an arrow or function
  expression a variable holds), **a written binding**, and **a redeclared
  name** — refused at the local-recursion step by name and location.
- **Every behavioral call domain other than `creates`**: `reads` (needs the
  proxy property-access forms, § 4.4), `writes`, `callbacks`, `cleanups`,
  `disposals`, `invalidates`, `returns`; `throws` is not a census target under
  version 1 at all. `ClosureCensus::Implementation` refuses them by name.
- **A dependency export without an audited negative row.** The plan's first
  terminator — a dependency whose `creates` is closed by an authenticated
  receipt — is not implemented, because § 4.5's accepted-dependency disposition
  has not been taken. Such a callee refuses by name.
- **Every Solid 1.x callee.** The 1.x negative table is empty (ADR 0007): the
  nineteen bundled 1.x documents' `creates: []` closures were introduced by a
  schema migration over a domain the audit never examined. A 1.x consumer's
  `creates` census terminates on no Solid callee.
- **Inside the dialect-defining archives.** The tier refuses to answer about
  `solid-js`, `@solidjs/signals`, or `@solidjs/web` under certification, so
  their own `creates` cannot close through this census (ADR 0007 § "Why an
  audited archive may not answer about another audited archive").
- **Four rows dead via cross-archive re-export** (`affects`, `isPending`,
  `latest`, `refresh` imported from `solid-js` resolve into `@solidjs/signals`,
  whose table lacks them) — unchanged from ADR 0007.
- **An uncensused form at the floor**, including a spread argument.
- **A callee the verifier cannot bind to a declaration node**, and a local
  declaration whose transcript is incomplete or open — the `controlFlowUnsupported`
  relaxation the positive families take is not available at any depth here.
- **A transcript whose declaration is not in the artifact's own runtime
  source**, the demanded export's included: the census walks authenticated
  runtime bytes and nothing else.

## What this does not yet buy on real rows

**Consumer probes cannot import the dependency in the private workspace.** The
private probe directory holds the analyzed package's snapshot copy, the harness,
and the recipes, and nothing else (`probe_harness.rs`); a consumer package's
own `import "solid-js"` resolves to nothing there, and any resolvable ancestor
`node_modules` refuses the gate by design. So no recipe can be written for a
real consumer row today. The targeted re-measurement shows the earlier
blocker, though: on every measured real row **no `creates` candidate was
proposed at all** — 0 candidates, not 0-withheld-of-many — because each of
those exports calls a 2.0 primitive with no negative row (above) or a 1.x
primitive, and the generator's walk is silent there. Every verdict is
unchanged and the withheld count is 0 on every row. Once the audits carry the
missing rows, candidates will appear and be withheld here until recipe
synthesis and authenticated dependency copying exist (ADR 0006 Stage 3). The
fixture, which ships its recipes, is where the whole chain is proven end to
end.

## Pinned

- `fixtures/package-contracts/implementation-census-creates`: `plain`
  (parameter-rooted, standard-library, local-recursion, `never()` proven
  **unreachable** — certifies with a nonempty gate root, every call witnessed,
  `census-total:5:1`), `viaHelperChain` (three hops; `census-total:4:3`),
  `cycle`, `deep` (nine hops), `unresolved`, `taggedTemplate`, `spreadArgs`,
  `switchBreak` and `whileBreak` (withheld rows, refused on the
  `switchReachability` / `iterationReachability` marker), `stdlibRefInvoker`
  (`forEach(work)`, refused as an unseen callable), `reflectApply` (refused by
  qualified name), `reassignedHelper` (refused as a written binding) — all
  refused by name — and `noRecipe` (withheld; certifies with `creates` open and
  the empty gate root of the gated plan). Its generator snapshots pin the
  proposals: every export but `unresolved` proposes.
- `fixtures/package-contracts/closed-domain-probe-gate`: `run` **certifies**
  `creates: []` through the census (parameter-rooted); `runCreatingOwner`, which
  does the same thing inside `try … finally`, **refuses** on the
  `tryReachability` marker (item 0's over-refusal, pinned as such);
  `primitive-consumer/`'s `runAfterSettle` calls `onSettled` and is refused by
  name as an unresolved callee — planned with its stub as an accepted dependency
  edge so the candidate survives closure replay, and refused because the private
  project materializes the consumer alone. The stub's `onSettled(callback: () =>
  void | (() => void))` matches the audited `@solidjs/signals@2.0.0-rc.3`
  declaration in parameter name and type.
- Unit tests in `type_facts::tests` pin each disposition (a `Construct`
  disposition included), the dialect disposition against a synthesized root
  matching the audited `@solidjs/signals@2.0.0-rc.3` tuple, the refusals, the
  node binding, the cycle, the unsupported-marker refusal at depth 0 and depth
  1, the nested-callable jump refusal, the standard-library slot proofs and
  denylist, the written / redeclared / anonymous binding refusals, and the
  byte-order-mark offset; `contract_certification::tests` pins recipe-gated
  planning without a producer, gated-dependency composition without a producer
  and end to end, and every fixture export above through the pinned producer.
  Under `SOLID_CHECKER_EXPECT_PROBE_PINS=1` a missing `SOLID_TYPEFACTS_BIN`
  fails those tracers loudly instead of skipping them.
- `scripts/coverage.mjs`'s `checkDialectStubs` scans nested fixture
  directories, so `closed-domain-probe-gate/primitive-consumer/node_modules/solid-js`
  is held to the same presence/parseability/tracking check as a root stub.
