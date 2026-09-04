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

The three unresolved-callee shapes the corpus ranking measured as the walk
being *stricter than the certifier it feeds* were investigated and **left
declining**; each one aligned would have proposed a candidate this census
refuses. See "The walk is not stricter than this census" below, and the
per-shape evidence under "What still refuses".

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

### The decline records

The gate above answered one bit per export and said nothing about *why*. That
made the ADR's own "what still refuses" list — the five unaudited 2.0
primitives — an argument nobody could size: with 0 candidates on every measured
row, "audit more primitives" was a guess about which primitives, on how many
exports, in how many packages.

So each refusing call now carries a `CreatesDeclineKind`, and
`CreatesProposalWalk::declines_for` answers, for one export's span, the set of
blockers reachable from it. Exactly the dispositions the walk itself
distinguishes and no invented sixth:

- `dialect-silent { package, export }` — a canonical dialect primitive no
  audit denies `creates` for. **The number this exists to produce.** `export`
  is the exact spelling `some_audit_denies_primitive` was asked about, so an
  audit row for it is what clears the record; `package` comes from the
  compiler's own `ResolvedDeclaration::origin_module` for the callee, or, where
  the build resolved no declaration, from the module specifier of the import
  statement that exact callee *symbol* is the binding of. Never from the
  spelling: a `dialect-silent` record with a guessed package would misdirect
  the audit it ranks. Neither answering leaves it empty.
- `create-publishing-callee { package, export }` — an accepted dependency
  contract that does not close `creates` empty, named by the contract binding's
  own package and imported export.
- `unresolved-callee { shape }` — no symbol resolved. There is still no callee
  identity to name, but the callee expression's **shape** is recorded; see
  "The unresolved-callee shapes" below.
- `refusing-callee-fixpoint { declaration }` — the propagated case, naming the
  refusing project function's exact declaration span.

#### The unresolved-callee shapes

`unresolved-callee` turned out to be **about half of every decline on the
measured corpus** — 20,450 of 41,957 — while saying only "something here did
not resolve". That cannot distinguish a resolver gap worth closing from a callee
no analysis of the module could ever decide, so the kind carries an
`UnresolvedCalleeShape` (`rust/crates/solid-reactive-ir/src/creates_walk.rs`).
Its wire `kind` is still `unresolved-callee`, so every existing
`declinedClosuresByKind` count is unchanged and `shape` plus `spelling` are two
**appended** marker/sidecar columns; an eight-column line written by an older
emitter still parses, with both empty.

Every shape is decided from facts the build already computed — Oxc's member,
computed-member, identifier, parameter and binding-initializer tables, and the
IR's own entity lookups. **No producer or Type Facts demand was added.** The
decision order is part of the contract, because one call can satisfy two
predicates (`props[key]()` is computed *and* parameter-rooted):

| shape | what decides it | spelling carried |
| --- | --- | --- |
| `computed-member` | the peeled callee span is in `computed_members` | the receiver, where the object is a plain identifier; else empty — no static property spelling exists, which is the shape's content |
| `parameter-rooted` | `member_callee_receiver` answers a root symbol, and that symbol (or one up to four binding-initializer aliases away) is a parameter name of a function whose **body contains this call** | the leaf property |
| `member-property-unresolved` | non-computed member fact, and `entity_symbol` answers for the (peeled) object span | the property |
| `member-receiver-unresolved` | the same member fact with **no** entity symbol at the object span — an unresolved identifier receiver or an expression receiver such as `factory().method()` | the property |
| `undeclared-identifier` | the peeled callee is an identifier fact with no entity symbol — in practice a global | the identifier |
| `expression-callee` | the peeled callee span is exactly a `CallFact::span` or a `FunctionFact::span` | `call-expression` / `function-expression` |
| `other` | which of the remaining syntax tables holds the span, from a fixed vocabulary (`await-expression`, `conditional-expression`, `logical-expression`, `jsx-element`), and `unknown-expression` where none does | the syntactic kind |

`other` is deliberately not a bucket: it carries the syntactic kind, so a shape
the classifier does not model stays visible in the ranking instead of being
folded into a neighbour.

#### The walk is not stricter than this census

The corpus-wide shape ranking measured 765 of 885 blocked consumer exports on
three shapes this census looked able to decide — `parameter-rooted` (384
exports, 123 rows), `member-property-unresolved` (381 / 96) and
`expression-callee` (52 / 30) — and concluded that the generator's pre-check
was stricter than the certifier it feeds, so closing the asymmetry would return
candidates. **The conclusion does not hold, and the mechanism is worth stating
because it moves the blocker somewhere else entirely.**

A member callee reaches the walk's unresolved branch exactly when the compiler
resolves **no symbol for the property**. That is the same condition under which
the producer records the same property access as an **uncensused invoking
form** — `property-access-unknown-accessor`
(`apps/solid-typefacts/internal/typefacts/tsgo/uncensused_invoking_forms.go`,
`accessorFormLocked`): with no symbol there are no declarations to inspect, a
`.d.ts` `read(): unknown` may perfectly well describe a `.js` getter, and
absence is not evidence of a plain data property. Item 2 above refuses **every**
uncensused form the floor admits, by kind and location. So for such an export
the census does not reach its dispositions at all:

- `parameter-rooted` *would* decide the call — the producer states
  `calleeParameter` (parameter index and property path) for
  `source.read()` — and the export is refused before that, on the form.
- `member-property-unresolved` is the same condition without the parameter
  root.
- `expression-callee` is refused for its own reason (below).

The walk's declines on those shapes are therefore **exactly the calls this
census refuses**, not calls it disposes, and aligning the walk would have
planned candidates that refuse at witness acquisition — turning certified rows
into refused ones. `implementation-census-creates`'s `memberParameterRooted`
and `iife` pin both refusals, and the shape declines stay
(`creates-decline-records`). What the ranking really located is a
**producer-side** gap: an untyped receiver in shipped JavaScript, which is what
the analyzed runtime artifact of an ecosystem package is.

**There is no `unaccepted-import` shape, and that is a measurement, not an
omission.** It was implemented first and it cannot fire: an import of an
unresolvable bare specifier, a deep subpath, or a missing default still gives
its local binding an alias symbol, so such a callee *resolves* and never reaches
the unresolved branch. A namespace import's member call reaches
`member-property-unresolved` with the receiver resolved. An unaccepted
dependency is a closure hazard decided at certification — a different decision
from this walk's. Two arms of the vocabulary above are likewise not produced by
any known source on this build: `expression-callee` spelled `call-expression`
(a higher-order `factory()()` resolves, because TypeScript answers an entity at
the inner call) and every `other` spelling but `await-expression` (a conditional
or logical callee resolves too — `EntitySymbols::at` answers with an *operand's*
symbol at a compound span). They are retained so that a callee which stops
resolving lands in the right shape rather than in the catch-all.

Per row the shapes reach `contractContent.unresolvedCalleeShapes`, ranked by
**distinct consumer exports blocked** with the call-site count beside it and
every concrete spelling listed; `scripts/dialect-audit-yield.mjs` prints the
aggregate as a second table under the dialect-silent one, and the report's
contract-content section gains an "Unresolved-callee shapes" table.
`fixtures/package-contracts/creates-decline-records` pins one export per shape.

**The set is transitive, and it has to be.** An export whose only refusing call
is a module-local helper's `createEffect` would otherwise report
`refusing-callee-fixpoint` and name no primitive — and that is the shape a real
consumer package has, so the measurement would be empty on precisely the rows
it was built for. `declines_for` therefore follows the same resolved local call
edges the fixpoint followed, bounded by depth 8 and a visited set, and a
propagated record keeps **its own** location inside the helper.

**Measurement, never evidence.** No claim is decided from a record, none is
encoded into a contract document, and `POLICY_DIGEST` does not move. A
`dialect-silent` record is the audits' *silence* about a spelling and an
`unresolved-callee` record is this build's own ignorance — and a *shape* is only
what this build observed about the callee expression, never a claim about what
the callee does; neither says the callee performs a `create`. The records are recorded only where a proposal was
actually on the table — a `ConsumingPackage` function export — because a
primitive-defining archive and a `value` export have no implementation walk to
blame, and listing their structural silence would put rows in the ranking that
no audit could ever clear.

They travel the road `WithheldOwnerRequirementRecord` already had: out of
`normalize_export`, through `ProposalArtifacts`, onto one
`solid-checker:declined-closure=` line per record at the emit boundary,
parsed by `generate-package-contract.mjs` into the proposal refusal audit's
additive `declinedClosures` array (locations folded to `<package-root>`, as
`stableRefusalReason` folds a refusal's), validated by
`scripts/contract-corpus.mjs`, and summarized per ecosystem row as
`contractContent.declinedClosures`, `declinedClosuresByKind`, and
`dialectSilentBlockers`. `scripts/dialect-audit-yield.mjs` ranks those across
every row by how many **distinct consumer exports** each `(package, export)`
primitive blocks, with the row count beside it; that script is the answer to
"what do we audit next".

**Why the sidecar pins them.** `declinedClosures` counts toward the corpus
gate's `auditedCases`, so a decline cannot appear, change kind, or vanish
unreviewed — the same discipline the other three arrays get. The cost is real
and accepted: a record carries byte offsets, so editing a fixture's source
moves its decline snapshot, and 20 corpus fixtures now carry one (83 records:
30 `dialect-silent`, 35 `unresolved-callee`, 18 `refusing-callee-fixpoint`),
each `unresolved-callee` one also pinning its shape and spelling.
That churn is
the yield made visible: adding an audit row is *supposed* to move every
snapshot whose exports it unblocks. It also measures a *refused* alignment:
excusing the `parameter-rooted` shape retired 12 of those fixtures'
`expected-refusals.json` and returned 19 `creates` candidates across 13
fixtures — every one of them an export this census then refuses on the
uncensused-form premise, which is exactly why the alignment was not kept.

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

- **The generator's walk still declines a default-library member it resolved
  no declaration for, and there is no fact that would let it stop.** The
  census's `standard-library` disposition reads
  `ResolvedDeclaration::standard_library` on the callee's resolved declaration.
  The walk only ever declines a callee for which
  `SemanticLookup::callee_symbol` (`indexes.rs`) answered nothing, and for a
  member callee that answer *is* `resolved_declaration_symbol` — the resolved
  declaration's own symbol — which the producer sets for every declaration node
  it resolves (`resolved_calls.go`'s `resolvedDeclaration`). So a
  default-library callee that resolves is already proposed and always was
  (`implementation-census-creates`'s `Array.from(items)` and
  `values.map(callback)` are not declines), and one that declines carries no
  `standard_library` flag to read. Measured rather than argued: over a 40-row
  ecosystem sample carrying 16,522 `unresolved-callee` records, **141 declining
  call sites had any resolved declaration at all, and none of those declarations
  was standard-library**. What is left is the property's spelling, which names no
  declaration — so this stays refused rather than guessed. The
  `member-property-unresolved` mass is a *resolver* gap (an untyped receiver in
  shipped JavaScript), not a missing disposition.
- **An export whose callee reads a property the compiler resolves no symbol
  for**, which is every `parameter-rooted`, `member-property-unresolved`,
  `member-receiver-unresolved` and `computed-member` decline the generator's
  walk records: the same access is an uncensused invoking form
  (`property-access-unknown-accessor`) and item 2 refuses it before any
  disposition is tried. The `parameter-rooted` disposition would have decided
  the call itself — the producer does state `calleeParameter` for
  `source.read()` — which is why this is stated here rather than left implicit:
  the walk and the census agree, and the blocker is the producer's inability to
  tell an accessor from a data property on an untyped receiver. Pinned by
  `memberParameterRooted`.
- **An immediately-invoked function expression**, which the generator's walk
  keeps declining as `expression-callee` although its body is lexically inside
  the export's own walked span. The census refuses the row by name: the producer
  resolves its callee to nothing at all, so there is no declaration, no
  parameter root, and no disposition. Excusing it in the walk would propose a
  candidate the census refuses, turning a certified row into a refused one.
  Pinned both ways: `iife` in the census fixture, `expressionCallee` in the
  decline fixture.
- **The actual next blocker on real rows: no Solid 2.0 negative row for
  `createSignal`, `onCleanup`, `untrack`, `getOwner`, or `createRoot`**
  (`rust/crates/solid-dialect/src/solid_2.rs`: the `creates` rows are `action`,
  `createMemo`, `onSettled`, `createEffect` and the other audited exports; those
  five have none). Almost every real consumer export calls one of them, so the
  generator's walk falls silent and **no candidate is proposed** — before any
  recipe, workspace, or census question arises. Filling that is an audit,
  recorded as an open item, not a census change. *Which* audit is no longer a
  guess: the decline records below make each silent primitive name itself, and
  `scripts/dialect-audit-yield.mjs` ranks them by how many consumer exports
  each one blocks. (The five have since moved — the 2026-09-04 audit added
  rows for all of them and withdrew `createEffect`'s — which is exactly why
  the ranking, and not a list in this ADR, is the durable answer.)
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

**Consumer probes could not import the dependency in the private workspace —
this is now done.** At this ADR's cut the private probe directory held the
analyzed package's snapshot copy, the harness, and the recipes, and nothing
else (`probe_harness.rs`); a consumer package's own `import "solid-js"`
resolved to nothing there, and any resolvable ancestor `node_modules` refuses
the gate by design, so no recipe could be written for a real consumer row. The
workspace now carries the transaction's **authenticated dependency closure**
beside the analyzed package's copy — only snapshots the transaction already
authenticated, one version per name or a refusal, each tree watched, and a
recipe's declared dependency specifiers echoed back and required to land inside
the authenticated copy — so a consumer recipe can import the package under test
and that package can resolve its own dependencies. The mechanism, the
multi-version decision, and what the echo does *not* prove are
`docs/adr/0006-probe-harness-binding.md` § "The authenticated dependency
closure"; the end-to-end fixture is
`fixtures/package-contracts/implementation-census-creates/dependency-consumer`.

What that does **not** move is the blocker below it. On every measured real row
**no `creates` candidate was proposed at all** — 0 candidates, not
0-withheld-of-many — because each of those exports calls a 2.0 primitive with
no negative row (above) or a 1.x primitive, and the generator's walk is silent
there. Every verdict is unchanged and the withheld count is 0 on every row, and
the dependency closure did not change either: with no candidate there is no
gate, and with no gate nothing is copied. Once the audits carry the missing
rows, candidates will appear and be withheld here until recipe *synthesis*
exists (ADR 0006 Stage 3) — the authenticated dependency copying it also waited
on is done. The fixtures, which ship their recipes, are where the whole chain
is proven end to end.

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
  the empty gate root of the gated plan). Two more pin why the generator's walk
  keeps declining the shapes the corpus ranking called decidable:
  `memberParameterRooted` (`source.read()` — refused on the
  `property-access-unknown-accessor` form, although its `calleeParameter` would
  have given the `parameter-rooted` disposition) and `iife` (refused as an
  unresolved callee of its own). Its generator snapshots pin the
  proposals: every export but `unresolved`, `memberParameterRooted` and
  `iife` proposes. Its nested
  `dependency-consumer/` pins the probe workspace's authenticated dependency
  closure: `plainConsumer` is censused and vetoed while the package's own
  top-level `import "solid-js"` resolves inside the authenticated private copy,
  and the same row with no authenticated snapshot for that dependency refuses
  the gate by name.
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
- `fixtures/package-contracts/creates-decline-records`: the decline records,
  one export per kind. `./clean`'s `proposes` still proposes (the control);
  `.`'s `dialectSilent` declines `dialect-silent` on `solid-js`'s
  `createEffect`, `viaSilentHelper` declines twice —
  `refusing-callee-fixpoint` at the call plus the helper's own
  `dialect-silent` at its own location — and `unresolvedCallee` declines
  `unresolved-callee` with the call's location. `parameterRooted`
  (`source.read()`) is the second control: it declines *nothing* and proposes,
  because the census disposes that call itself, while `parameterAliasRooted`
  (one binding alias away) and `nestedParameterRooted` (a parameter of a nested
  arrow) keep recording the `parameter-rooted` shape — the two sub-cases the
  producer's `calleeParameter` does not answer. The control lives in its own
  entrypoint deliberately: `index.js`'s top-level `import "solid-js"` is an
  `UnacceptedExternalDependency` closure hazard that opens every domain of
  that artifact case whatever the walk found, so a control beside the declines
  would prove nothing. Its stub cannot satisfy the audited-archive identity, so
  what `dialect-silent` pins there is the *dialect's canonical-primitive
  recognition*, not the tier — see the fixture README.
- `creates_walk::tests` pins the transitive report through a local call edge,
  the mutual-recursion termination, the module-specifier-to-package reduction,
  and that a walk which never ran names no blocker;
  `contract-workflow.test.mjs` pins the marker parse (per target, relativized,
  refused when truncated); `scripts/dialect-audit-yield.test.mjs` pins the
  ranking against a synthesized report, including that a row carrying no
  records is *named* rather than counted as zero.
