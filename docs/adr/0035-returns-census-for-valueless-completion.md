# ADR 0035: A `returns` census for valueless completion

- Status: accepted and implemented (2026-09-05); written before implementation
- Date: 2026-09-05
- Owners: Type Facts producer and the policy-2 behavioral call-domain census
- Relation: the second behavioral call domain the implementation census of
  ADR 0008 decides, taken in the order `phase21/2026-09-03-implementation-
  census-plan.md` § 4.5 names. It applies `semantic-model.md` § returns'
  2026-09-03 decision that a valueless completion is not a `return` operation.
  It does not touch `reads`, whose proxy property-access forms make every
  parameter read a possible observation and are out of reach of the
  parameter-rooted principle of ADR 0034.

## Context

Every export in the ordinary three-row baseline has eight of nine call domains
open, and `exportsProven` is 0 on every real row because closing `creates`
alone closes no export. The census plan ranked the remaining domains by cost:
`returns` is "control-flow-decidable and the cheapest of the nine", `reads`
needs the proxy property-access forms, `throws` is not a census target under
version 1, and the accepted-dependency disposition needs two composition
breaks closed first.

`returns: [] closed` denies that one invocation of the export yields a value to
its caller. The semantic model fixed what that means on 2026-09-03: **a
valueless completion is not a `return`**. A bare `return;`, a body that falls
off its end, and a `void` export all complete without yielding a value, and
that is what lets `createEffect` and every void Solid 2.0 primitive close
`returns: []`. The census question is therefore not "does control leave this
function" — always yes — but "does any completion carry a value", which the
producer's control-flow census already enumerates as `ReturnSite` rows.

Two things make this domain cheaper than `creates`, and the ADR takes both:

1. **No callee disposition is needed.** A callee's return value reaches this
   export's caller only through this export's own completion. A helper that
   returns a value is not a counterexample to `returns: []` unless the export
   returns it, and then the export's own return site carries an expression and
   the census refuses on that site. So the census walks the export's own
   transcript and no other: no dialect table, no standard-library table, no
   accepted-dependency terminator, no recursion into local declarations.
2. **No new invoking-form classifier is needed.** The forms that reach `returns`
   are completion forms — a `return` statement with an expression, an arrow's
   expression body, a generator's `yield` and completion value, an `async`
   function's resolved value — and all but the last two are already `ReturnSite`
   rows. The two missing facts, whether the implementation is `async` or a
   generator, are syntactic and the producer states them.

What the generator already emits matters. Its `returns` summary is
`Known(Some(shape))` when it described a reactive or structured return and
`Known(None)` otherwise — and "otherwise" includes every export that returns a
plain value the reactive analysis has nothing to say about. `Known(None)`
normalizes to `Complete([])`, so today's plan sidecar carries an empty `returns`
closure candidate for value-returning exports such as
`implementation-census-creates`' `plain`. That candidate is not a claim the
generator derived; it is the absence of a reactive description weakened into
`Unknown` and then read back as a closure. A naive "make `returns` proposable"
would publish it for every such export, the census would refuse the
value-carrying return, and a certified row would become a refused one. The
proposal rule below therefore rests on a dedicated walk, exactly as `creates`
rests on `creates_walk_clean`, and never on `Known(None)`.

## Decision

### The claim this census decides

The implementation census decides `returns: [] closed` — the empty closure —
and nothing else in the domain. A `returns` closure that enumerates operations
is not proposed and, if a hand-written document proposes one, the census
refuses it exactly as `census_creates_domain` refuses a nonempty `creates`
enumeration: admitting it would certify the proposal's own word about a shape
this census never derived (objection 5 of ADR 0006).

### What a valueless completion is

The census certifies the empty closure when, on the demanded export's own
`ExportImplementationTranscript`, every one of these holds. Each failing
premise refuses **by name and location**; none is a silent pass.

1. **The implementation is a plain callable.** Its completion form, stated by
   the producer, is neither `async` nor a generator. An `async` function yields
   a promise to its caller on every completion, a generator yields an
   iterator, and an `async` generator yields both — a value the caller
   receives whatever the body does, so `returns: []` is false of them by
   construction. A completion form the producer does not classify
   (`unclassified`) refuses.
2. **The body is a block.** An expression-bodied arrow is a completion carrying
   its expression; the producer already records it as a value-carrying
   `ReturnSite`, and premise 3 refuses it. Stating it separately keeps the
   refusal's name honest.
3. **Every return site at the `MayExecute` floor is bare.** A `ReturnSite`
   whose `value` is present carries an expression, and the census refuses on it
   when its reach is `reachable` or `unknown`. A value-carrying return whose
   reach is `unreachable` — inside a branch the producer proved dead by literal
   truthiness, or after a `throw` — performs nothing and is admitted, with a
   witness site naming it `unreachable`, exactly as an unreachable call is in
   the `creates` census. `return;` and falling off the end are the completions
   the claim describes and need no site.
4. **The control-flow census is classified and admissible.** Every
   incompleteness row is `reachability-lower-bound`; `flow-unaccounted` refuses.
   The same relaxation ADR 0029 took for `creates` applies here for the same
   reason: a loop, a `switch` or a `try` that only lowers the reachability
   bound still has every `return` inside it enumerated, with reach `unknown`,
   and premise 3 reads that row like any other. A jump whose target no
   enclosing construct owns has no region to bound, and refuses.
5. **The transcript is complete for this purpose.** Its `controlFlow` census is
   present, its declaration is bound, and it clears the same completeness and
   authenticated runtime binding every implementation-reading proof family
   clears (`require_export_implementation`). A missing census is an absence,
   never an enumeration of zero returns.

Nothing about nested callables is a premise. A `return` inside a nested
function is that function's completion; the producer's scan stops at callable
declarations, and a nested callable's value reaches the caller only if the
export returns it, which premise 3 catches. A `throw` is a completion without a
value and is not a `return`. A construct invocation (`new f()`) is not in
scope: `call` claims describe the selected call signature, and a construct
signature is a different invocation of a different signature.

### What the proposal side does

`ClaimDomain::PROPOSABLE` gains `Returns`. The generator republishes a `returns`
closure only for a function export whose **valueless-completion walk** is
clean: the generator's own syntax facts show a block-bodied, non-`async`,
non-generator function whose own body — nested callables excluded — contains
no `return` statement carrying an expression. That walk is a proposal input
and never a proof, in the same sense as `creates_walk_clean`: it decides
whether the generator has seen anything a `returns: []` proposal would
contradict, and silence is "do not propose". An export the walk does not clear
keeps today's behavior exactly — its `returns` knowledge weakens into the plan
sidecar as measurement — so no row that certifies today can lose that
certification to a candidate this ADR adds.

Every proposed `returns` closure is **recipe-gated under the mandatory veto**
like `creates`: a candidate with no recipe in the corpus is withheld by name and
the domain stays open, and a recipe that provokes its marker vetoes the row.
The veto marker is the recipe's own, per the corpus's `expectedEvent`; the
convention this ADR fixes for the checked corpora is a recipe that calls the
export and emits `return-value` when the result is not `undefined`. A finite
clean run never establishes the closure; the census does.

### Producer facts

`ExportImplementationTranscript` gains `completionForm`, one of `plain`,
`async`, `generator`, `async-generator`, or `unclassified`, stated from syntax
alone — the `async` modifier and the asterisk token on the implementation's
own declaration. It is stated on every transcript, so a consumer never reads
absence: the Type Facts handshake protocol moves from 18 to 19, and a verifier
build that speaks less refuses this census by name. Nothing else about the
transcript changes.

### Controlled execution profiles

Unchanged. Each profile still requires exactly one recipe-selected mandatory
`creates` gate. A `returns` candidate on the same export is withheld when the
request's corpus carries no recipe for it, and so never enters the schedule
the profile reads. The scoped closures those profiles measured are unaffected.

## Alternatives considered

- **Decide the full `returns` enumeration.** Comparing the proposal's `return`
  operations against the census's own derived shapes would need a shape
  derivation the transcript does not carry as a reviewed enumeration (the
  `ReturnSite.value` facts are type descriptors, not the semantic model's
  output shapes). Refusing nonempty closures keeps the census honest and costs
  nothing the empty closure needed. Not taken.
- **Read the selected signature's result type instead of a completion-form
  fact.** A `Promise<void>` result would flag an `async` function, but a
  declared signature can be annotated and a non-`async` function can return a
  promise it constructs; the census would be reading a type where the model
  asks a syntactic question. The producer states the syntax. Not taken.
- **Skip the mandatory veto for `returns`.** The census is complete on its own
  premises, and a recipe that calls the export and checks `undefined` proves
  little. But the veto is not a source of closure evidence for any domain; it
  is the finite contradiction check every proposed closure passes through, and
  exempting one domain would create the first closure that never met a runtime.
  Not taken.
- **Do `reads` first.** It is the domain the rules engine consumes most, but
  the semantic model makes a property read on any parameter a possible read of
  a proxy, so the parameter-rooted principle cannot dispose it and every
  real export that touches a parameter's members would refuse. Deferred with
  its blocker named: a `reads` census needs the proxy property-access forms and
  a premise about *which* parameters can carry a proxy.

## Implementation (2026-09-05)

- **Producer** (`apps/solid-typefacts`, handshake protocol 18 → 19, schema
  digest moved). `ExportImplementationTranscript.completionForm` is stated from
  the implementation's own syntax on every transcript that reached its
  implementation: `plain`, `async`, `generator`, `async-generator`, or
  `unclassified` for a declaration kind the classifier does not review.
- **Verifier** (`type_facts.rs`). `require_census_decides_closure` admits
  `Call(Returns)` under the implementation census; `census_returns_domain`
  refuses a nonempty enumeration and a build below protocol 19, then
  `census_returns_transcript` reads the completion form, requires a present
  control-flow census whose every incompleteness is `reachability-lower-bound`,
  and walks the return sites: a bare site is a `census-return:…:bare` witness,
  a value-carrying site the producer proved unreachable is `value-unreachable`,
  and any other value-carrying site refuses with its location and reach.
  `recipe_gated` gates every proposable domain and names the withheld domain
  from the claim path.
- **Generator** (`solid_reactive_ir::returns_walk`, `inferred_contract.rs`).
  `ClaimDomain::PROPOSABLE = [Creates, Returns]`. The valueless-completion walk
  clears a block-bodied, non-`async`, non-generator function whose own body —
  returns strictly inside a nested callable excluded — carries no `return` with
  an expression; a function export the walk clears and whose reactive analysis
  described no return proposes `returns: []`. A described return keeps its
  positive operation and is never republished as a closure. `Known(None)`
  without a clean walk now normalizes to `Unknown` directly, which withdrew
  **179** spurious `returns` closure candidates from the corpus's plan sidecars
  (they reappear as the open claims they always were) and added none; **53**
  exports across 28 fixtures gained `closed: ["creates", "returns"]`, every
  one reviewed against its source, and the one export with a void and a
  value-returning artifact case closed only in the void case.
- **Fixtures.** `implementation-census-returns`: four certifying exports through
  a checked recipe, five refusing by name (`value-carrying completion` with
  reach `reachable` or `unknown`, `async implementation`, `generator
  implementation`). `implementation-census-creates`'s nine void exports now
  also propose `returns: []`. (The `const` arrow export first exposed that the
  generator bound walk verdicts to function declarations only; that binding
  gap was fixed the same day, see `docs/precision-backlog.md`.)

Measured on the ordinary three-row baseline
(`three-row-adr0035/after.json`, SHA-256
`570be772a0533c2098a81c147eb74abbe03a65777076d55bafbd12e8dfd65b1a`): every row
keeps its class, reason and withheld-closure count. No Kobalte alpha export
proposes `returns: []` — the published JavaScript exports the walk can bind all
carry a value-returning completion — so the real-row yield of this domain waits
on exports that are void by construction, and `exportsProven` stays 0 as the
Consequences predicted.

## Consequences

- **Yield.** Every void export whose census passes gains a second closed
  domain once a recipe addresses it. `exportsProven` still does not move —
  seven domains remain open on every real export — and this ADR does not
  claim otherwise. Its measurable outputs are the closed `returns` domains on
  the fixture and on any real row with a recipe, and the count of `returns`
  candidates the generator now publishes rather than sidecars.
- **Producer.** One field, one protocol bump (18 → 19), schema digest moves.
- **Verifier.** `require_census_decides_closure` admits `Call(Returns)` under
  the implementation census; `census_returns_domain` is a second entry beside
  `census_creates_domain` with `census-return:` witness sites; `recipe_gated`
  gates both domains and names the withheld domain from the claim path.
- **Generator.** `PROPOSABLE = [Creates, Returns]`; a valueless-completion
  walk beside the `creates` walk; the republish rule above. Every corpus
  fixture export the walk clears gains a `returns` proposed closure, so the
  corpus snapshots move by that label alone; review the non-updating run
  before updating. The generator's `Known(None)`-means-no-reactive-return
  convention is left as it is and recorded in the backlog: it is harmless
  while weakened, and correcting it is a semantic-model question about what
  the generator's `returns` shape describes.
- **Fixtures.** A new `implementation-census-returns` fixture pins each premise
  positively and negatively: bare completion, early bare return, bare return
  inside a loop, an unreachable value-carrying return, a nested callable that
  returns a value (all certify), and a value-carrying return, an
  expression-bodied arrow, an `async` function, a generator, and a value return
  inside a loop (all refuse by name).
- **Not changed.** `reads`, `writes`, `callbacks`, `cleanups`, `disposals`,
  `invalidates`, `throws`; every controlled execution profile; every receipt
  identity for a row that certified before this ADR.
