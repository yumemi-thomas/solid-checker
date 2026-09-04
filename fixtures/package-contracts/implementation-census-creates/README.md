# implementation-census-creates

The end-to-end tracer for the `creates` implementation census
(`docs/adr/0008-implementation-census-for-creates.md`): a consuming package
whose function exports all *propose* `creates: []`, and a certifier that proves
the claim for some of them, refuses it by name for others, and withholds it by
name for one.

It is a generator-corpus fixture (`corpus.json`), so `expected.json` and
`expected-proposal.json` pin the generator's side of the story: every function
export **except `unresolved`, `memberParameterRooted` and `iife`** proposes
`creates: []` — the generator's own walk
of each implementation finds no call that a closed `creates` would contradict,
and those three end at a callee it cannot resolve, which is never evidence of
harmlessness — and the proposal plan carries one `{kind: "call", domain:
"creates"}` closure candidate per proposing export.

A proposing export's summary in `expected.json` states the closure and labels
it: `closed: ["creates"]`, `creates: []`, `proposedClosures: ["creates"]`. That
label is the difference between a generator's proposal and a reviewed negative
claim, and stating the closure is what makes the candidate reach the certifier
at all (ADR 0008 § 1 "How the proposal is published"). Because the label is
part of the summary, a proposing and a non-proposing export never share a
summary id even when their semantics are otherwise identical — which is why
`iife` and `cycle` sit in different summaries here.

A proposal is a claim to be proven, not a proof; what the certifier does with
each candidate is driven by
`contract_certification::tests::the_probe_gate_tracer_*census*` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`, against the
pinned Type Facts producer.

Two of those tests plan from **this fixture's own `expected.json`** rather than
from a synthesized closed candidate, which is the only way the
generate-then-certify path is exercised anywhere in the repository:
`the_generated_census_fixture_carries_every_creates_candidate_into_planning`
(no producer; asserts the fourteen candidates survive into planning, each with
its veto and its `DomainExhaustiveness` demand, and that all fourteen are
withheld by name when no corpus is supplied) and
`the_census_certifies_a_generated_creates_candidate_and_withholds_its_siblings`
(one recipe, for `plain`, so recipe gating withholds the other thirteen and the
census decides `plain`). They rebind exactly one field of the document, the
package integrity token, because the corpus generates `fixture:sha256:…` and a
certification transaction requires the published archive's own integrity.

## The predicate

`creates: []` certifies only if every invoking form in the export's transitive
census, taken at the `MayExecute` reachability floor, is enumerated and
resolved, and no resolved target performs a `create` operation
(`phase21/2026-09-03-implementation-census-plan.md` § 3). Every transcript the
census reads must classify every construct whose control flow the producer did
not fully model as `reachability-lower-bound` — the construct is walked in full,
every call inside it is on the wire, and only the *guarantee* is missing, which
a may-execute census does not need. A `flow-unaccounted` construct refuses by
marker and location. Every call the export reaches is then given exactly one
disposition, and the first call with none refuses the domain by name:

| disposition | what it names |
| --- | --- |
| `unreachable` | the producer's control-flow census proves the call never runs |
| `parameter-rooted` | the callee is proven to be a caller-supplied callable; its body is the caller's behavior |
| `standard-library` | the callee resolved, by default-library symbol identity, to a member of `lib.*.d.ts`; the engine registers no version-1 resource into a Solid runtime, **and** the member transfers control to no callable the census cannot see: every slot the reviewed invoker table says it invokes, and every slot the producer saw a callable in, is parameter-rooted or a callable literal inside the transcript; `Function.prototype.{call,apply,bind}`, `Reflect.apply`/`construct`, `eval` and `Function` refuse by name |
| `dialect-axiom` | the audited Solid 2.0 negative table denies the primitive's `creates` (ADR 0007); not reachable in this fixture, see below |
| `local-recursion` | the callee is declared in this artifact's own runtime source under a binding identifier nothing in the file writes or redeclares, and the census recurses into its transcript |

## What each export is for

| export | census outcome | why |
| --- | --- | --- |
| `plain` | **certifies** with a recipe | `callback(0)` is parameter-rooted; `mapAll` is a local recursion whose `Array.from(items)` and `values.map(callback)` are standard-library members handed only parameter-rooted values; `never()` after the `return` is proven **unreachable** by the producer's control-flow census (`census-total:5:1`) |
| `viaHelperChain` | **certifies** with a recipe | three local-recursion hops, the innermost parameter-rooted |
| `cycle` | refuses: cycle | `cycleA` calls `cycleB` calls `cycleA`; the revisit is refused by declaration identity (symbol, file, exact span), never by name |
| `deep` | refuses: depth | nine local-recursion hops, past the `MAX_COMPOSITION_DEPTH` (8) bound; refused rather than approximated |
| `unresolved` | refuses: unresolved callee | `externalGlobal` is an identifier no declaration binds: no declaration, no parameter root, no disposition. The refusal names the call. (The generator's own walk also refuses to *propose* for it — an unresolved callee is never evidence of harmlessness — so the corpus proposal leaves its `creates` open and the tracer closes it by hand to pin the census's refusal.) |
| `taggedTemplate` | refuses: uncensused form | a `TaggedTemplateExpression` invokes its tag and appears in no `calls` row; the producer records it as `tagged-template` and the census refuses on it |
| `spreadArgs` | refuses: uncensused form | `joinAll(...args)` drives the iteration protocol on `args`; the producer records the `SpreadElement` as `iteration-protocol` (`docs/typefacts/adr/0026-…`), and the census refuses on it. **Arguments do not otherwise matter for `creates`** — a call is dispositioned by its callee — but a spread is an invoking form of its own, not an argument |
| `noRecipe` | **withheld** | byte-for-byte `plain`'s body; the corpus supplied to the transaction carries no recipe for *this* export's claim, so recipe-gated planning withholds the candidate by name, the domain stays open, and the row certifies with an empty probe-gate schedule |
| `loopCall` | **certifies** with a recipe | a bare `while` with no jump in it. The producer cannot give a reachability *lower* bound inside a loop body, so it reports `iterationReachability` — classified `reachability-lower-bound` — and `mount(el)` is on the wire at `reach: unknown`, which the `MayExecute` floor admits. This is the shape ADR 0008 item 0 over-refused on real code (`@solid-primitives/i18n`'s `flatten` and `chainedTranslator`) |
| `switchBreak` | **certifies** with a recipe | `mount(el); break;` inside a `switch` case. The `break`'s target is the `switch` that owns it, so the producer covers that construct as the region the jump makes non-universal and reduces the row's reach to `unknown` — it used to **drop** the row, and since a dropped `CallExpression` leaves no uncensused-form row either, the marker was the only trace. `mount` is now dispositioned by local recursion, which is what certifies it: never a relaxed marker |
| `whileBreak` | **certifies** with a recipe | the same, with the `break` owned by a `while` |
| `labelledBreak` | refuses: unaccounted flow | `break outer` out of a plain labelled *block*. No enclosing loop or `switch` of the frame owns that target, and the target is what bounds every region-based repair either census applies to a jump, so the marker is `jumpReachability`, classified `flow-unaccounted`. `mount(el)` is on the wire here too — the refusal is not about a missing row but about a frame nobody modelled, and it is what keeps the relaxation above from being a blanket one |
| `stdlibRefInvoker` | refuses: unseen callable | `Array.from(items).forEach(work)`: `forEach` invokes its slot 0 (reviewed invoker table) and `work` is a module-local function reference — neither a parameter nor a callable literal inside the transcript — so the standard-library disposition refuses the call by name |
| `reflectApply` | refuses: by-reference member | `Reflect.apply(work, undefined, args)` transfers control to its first slot; the member refuses by qualified name whatever the slots prove |
| `reassignedHelper` | refuses: written binding | `function helper` is reassigned at module level; the producer still resolves `helper(el)` to the declaration, and the verifier's own parse of the authenticated bytes finds the write and refuses to walk a declaration not proven to be the code that runs |
| `memberParameterRooted` | refuses: uncensused form | `source.read()` — the shape the corpus ranking called the generator's own strictness, and the pin that says otherwise. The producer *does* state `calleeParameter` (parameter 0, path `["read"]`), so the `parameter-rooted` disposition would decide the call; the export is refused before that, because reading `.read` off a value whose type is unknown is an uncensused invoking form (`property-access-unknown-accessor`) — recorded exactly when the compiler resolves no symbol for the property, which is the same condition that makes the generator's walk decline the callee |
| `iife` | refuses: unresolved callee | an immediately-invoked function expression. Its body is lexically inside the export and already walked, so the generator has no counterexample to name — and the census refuses the row by name: the producer resolves its callee to nothing at all. Which is why the walk keeps declining `expression-callee` rather than treating it as spurious |

The `unresolved`, `taggedTemplate`, `spreadArgs`, `labelledBreak`,
`stdlibRefInvoker`, `reflectApply`, `reassignedHelper`,
`memberParameterRooted` and `iife` refusals arrive
before any recipe matters, at witness acquisition. Their tests still supply a recipe
(`probe-recipes/refused-export.mjs`), deliberately: without one the certifier
would withhold the candidate and the row would certify with the domain open,
which is the right outcome for a missing recipe and the wrong test for a census
refusal.

## What the census does *not* decide here

- **The dialect tier is not reached.** No export calls a Solid primitive. The
  `node_modules/solid-js` stub selects the Solid 2.0 dialect and declares
  nothing; the disposition itself is pinned by
  `type_facts::tests::creates_census_dispositions_every_admitted_call_by_its_callee`
  against a synthesized root matching the audited `@solidjs/signals@2.0.0-rc.3`
  tuple, and its refusals — a coordinate over other bytes, the archive under
  certification being itself audited — by the tier's own tests.
- **A certified row does not mean the veto observed anything.** The probe gate
  only declines to contradict the claim; the census is what proves it.

## `dependency-consumer/` — the authenticated dependency closure

A nested package whose module top level does `import { record } from "solid-js"`,
which is the shape every real consumer row has and the one no probe could reach
until the private probe workspace carried the transaction's authenticated
dependency closure (`docs/adr/0006-probe-harness-binding.md` § "The
authenticated dependency closure"). Its own `node_modules/solid-js` stub is the
authenticated snapshot that gets copied into the workspace and, unlike the
`primitive-consumer/` stub in `closed-domain-probe-gate`, is genuinely **run**
there.

Two exports, and the split is load-bearing:

- `plainConsumer` is the census subject. Its one call is parameter-rooted, so
  the implementation census proves `creates: []` and the mandatory veto
  actually runs — the only way the workspace mechanism gets exercised end to
  end. It must **not** call `record`: a callee resolving into a dependency
  archive is refused by name, the gate would never be reached, and the fixture
  would prove nothing about the workspace.
- `callsDependency` is never demanded closed. `probe-recipes/dependency-consumer.mjs`
  calls it and throws unless the answer is the stub's own `recorded:probe` —
  because resolving is not running, and a specifier that resolved *somewhere*
  while the bytes were not the authenticated ones has to fail the run.

The recipe declares `dependencySpecifiers: ["solid-js"]`, so Rust additionally
requires the worker's echoed resolution for that specifier to name a file
inside the authenticated private copy. The negative arm is the same package,
the same recipe, the same accepted dependency edge and **no** authenticated
snapshot: the gate refuses by name
(`a_dependency_with_no_authenticated_snapshot_refuses_the_probe_gate_by_name`).

It lives in its own package rather than beside `plain` because those exports'
recipes must keep proving the *no-dependency* path: a top-level
`import "solid-js"` in this fixture's own `index.js` would make every one of
them depend on the closure this nested package exists to isolate.

## The recipes

`probe-recipes/` holds the modules; the tests write `recipes.json` themselves,
because claim ids are content digests of the normalized claim and are derived
from the plan's own gate schedule. Each module imports its package by bare
specifier — which resolves to the private copy inside the harness — calls one
export with plain closures, and emits the `call` enter/exit pair that proves it
ran. None hands `session` or `harness` to the package.

## What must stay true

- **`plain` and `noRecipe` must stay byte-identical in body.** The difference
  between their rows is the corpus, and only the corpus.
- **`cycle` must stay guard-free.** A `depth > 0` guard is an operator
  application on an untyped operand, which the producer records as a `coercion`
  form; the census would refuse on that row before reaching the cycle, and the
  test would pass for the wrong reason.
- **`deep` must stay at nine hops or more**, one past the bound.
- **`unresolved` must stay a bare undeclared identifier.** An import from an
  unaudited dependency is not the same case: a bare specifier that resolves to
  no accepted dependency is an `UnacceptedExternalDependency` closure hazard
  that opens every domain at closure replay, and no candidate ever reaches the
  census.
- **`switchBreak` and `whileBreak` must keep their `break`, and `loopCall`
  must have none.** The pair with a `break` is where a row really was
  withheld — the region the jump covers — and `loopCall` is where nothing was:
  together they pin that the census reads the *row* rather than the marker.
  Dropping either loses half of that.
- **`labelledBreak` must keep a `break` to a plain labelled block.** A `break`
  the enclosing loop or `switch` owns is the admissible case above; only a
  target no construct of the frame owns produces the `flow-unaccounted` class,
  and without that arm the relaxation has no negative control.
- **`stdlibRefInvoker` must hand `forEach` a function *declaration*.** A
  `const work = () => …` is followed by the producer's argument tracer and
  would refuse for a different reason (a callable outside the transcript);
  the row pins the untraced-reference case.
- **`reassignedHelper`'s `helper` must be a `function` declaration written by
  a later statement.** A `const` arrow would refuse as an anonymous callable
  before any write is consulted.
- **`dependency-consumer/plainConsumer` must not call into `solid-js`.** The
  gate has to be *reached* for that fixture to say anything about the
  workspace, and a dependency callee refuses the census by name first.
- **`dependency-consumer/node_modules/solid-js` must stay tracked and must
  keep exporting a working `record`.** It is copied into the private workspace
  and executed there; the recipe refuses the gate on any other answer, and an
  untracked stub removes the only fixture that proves a consumer recipe can
  import the package under test at all.
