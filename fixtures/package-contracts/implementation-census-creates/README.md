# implementation-census-creates

The end-to-end tracer for the `creates` implementation census
(`docs/adr/0008-implementation-census-for-creates.md`): a consuming package
whose function exports all *propose* `creates: []`, and a certifier that proves
the claim for some of them, and — since ADR 0036 — withholds it by name for the
others: a candidate the census cannot decide is withheld with the census's own
reason (`census refused: …`) and a candidate whose veto run does not complete
(`loopCall`, which loops forever on any truthy argument) is withheld as
`veto did not complete: gate …`, while the row certifies with those domains
open. Before ADR 0036 either outcome refused the whole row; a veto
*contradiction* still does.

It is a generator-corpus fixture (`corpus.json`), so `expected.json` and
`expected-proposal.json` pin the generator's side of the story: every function
export **except `unresolved` and `iife`** proposes
`creates: []` — the generator's own walk
of each implementation finds no call that a closed `creates` would contradict,
and those two end at a callee it cannot resolve, which is never evidence of
harmlessness — and the proposal plan carries one `{kind: "call", domain:
"creates"}` closure candidate per proposing export. (`memberParameterRooted`
proposes since ADR 0034 aligned the walk with the census's
`parameter-rooted-accessor` disposition.)

A proposing export's summary in `expected.json` states the closure and labels
it: `closed: ["creates"]`, `creates: []`, `proposedClosures: ["creates"]`. (Since
ADR 0035 the nine exports whose bodies never carry a value-returning completion
— `cycle`, `deep`, `labelledBreak`, `loopCall`, `setterOnParameter`,
`stdlibRefInvoker`, `switchBreak`, `viaHelperChain`, `whileBreak` — also
propose `returns: []` and carry a second closure candidate; see
`../implementation-census-returns`.) That
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
(no producer; asserts the thirty-four candidates survive into planning, each with
its veto and its `DomainExhaustiveness` demand, and that all thirty-four are
withheld by name when no corpus is supplied) and
`the_census_certifies_a_generated_creates_candidate_and_withholds_its_siblings`
(one hand recipe, for `plain`; since ADR 0036 every sibling candidate —
`creates` and the valueless exports' `returns` alike — is served by a
synthesized veto, and the test pins the resulting partition: fifteen `creates`
and seven `returns` closures, eighteen `creates` candidates and one `returns`
candidate withheld with the census's reason, and `loopCall` withheld in both
domains because its synthesized run never reports). They rebind exactly one field of the document, the
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
| `local-recursion-backedge` | the same exact stable declaration is already on the current census stack; the edge closes the finite graph while every non-cycle edge remains subject to its ordinary disposition |

## What each export is for

| export | census outcome | why |
| --- | --- | --- |
| `plain` | **certifies** with a recipe | `callback(0)` is parameter-rooted; `mapAll` is a local recursion whose `Array.from(items)` and `values.map(callback)` are standard-library members handed only parameter-rooted values; `never()` after the `return` is proven **unreachable** by the producer's control-flow census (`census-total:5:1`) |
| `viaHelperChain` | **certifies** with a recipe | three local-recursion hops, the innermost parameter-rooted |
| `cycle` | **certifies** with a recipe | `cycleA` calls `cycleB` calls `cycleA`; the exact stable revisit closes the zero-upper-bound graph, and the boolean argument makes the mandatory sample finite |
| `deep` | refuses: depth | nine local-recursion hops, past the `MAX_COMPOSITION_DEPTH` (8) bound; refused rather than approximated |
| `unresolved` | refuses: unresolved callee | `externalGlobal` is an identifier no declaration binds: no declaration, no parameter root, no disposition. The refusal names the call. (The generator's own walk also refuses to *propose* for it — an unresolved callee is never evidence of harmlessness — so the corpus proposal leaves its `creates` open and the tracer closes it by hand to pin the census's refusal.) |
| `taggedTemplate` | refuses: uncensused form | a `TaggedTemplateExpression` invokes its tag and appears in no `calls` row; the producer records it as `tagged-template` and the census refuses on it |
| `spreadUntyped` | refuses: uncensused form | `joinAll(...items)` drives the iteration protocol on `items`, an unannotated ordinary parameter and therefore `any`. `any` enumerates no members, and "the checker could not find `[Symbol.iterator]`" is never "iterating this reaches no user code", so the producer records the `SpreadElement` as `iteration-protocol` (`docs/typefacts/adr/0026-…`) and the census refuses on it. **Arguments do not otherwise matter for `creates`** — a call is dispositioned by its callee — but a spread is an invoking form of its own, not an argument |
| `spreadArgs` | **certifies** with a recipe | byte-identical to `spreadUntyped`'s body, and the pair that shows the iteration arm is decided by the operand's *type*: `args` is a **rest** parameter, so its own type is `any[]` however its elements are typed. Iterating an array drives `Array.prototype[Symbol.iterator]` and the array iterator it returns, both engine code, so no form is recorded and the census closes the domain. A census that classified iteration by syntax refused this |
| `noRecipe` | certifies | byte-for-byte `plain`'s body, and no hand recipe for *this* export's claim. Since ADR 0036 the certifier synthesizes the veto from the export's Type Facts call signature, so the candidate is planned, the census proves it and the row certifies with `creates` closed; before ADR 0036 recipe-gated planning withheld it by name (`no recipe in corpus`), which is still what happens when no harness is configured at all |
| `overloaded` | certifies | byte-for-byte `plain`'s body, declared with **two overloads** and no hand recipe. The export states no single call signature; since 2026-09-06 synthesis takes the complete declared overload set and samples every member, so the candidate is planned, the census proves it and the row certifies with `creates` closed. A partial overload set would synthesize nothing and the candidate would stay withheld by name |
| `loopCall` | **certifies** with a recipe | a bare `while` with no jump in it. The producer cannot give a reachability *lower* bound inside a loop body, so it reports `iterationReachability` — classified `reachability-lower-bound` — and `mount(el)` is on the wire at `reach: unknown`, which the `MayExecute` floor admits. This is the shape ADR 0008 item 0 over-refused on real code (`@solid-primitives/i18n`'s `flatten` and `chainedTranslator`) |
| `switchBreak` | **certifies** with a recipe | `mount(el); break;` inside a `switch` case. The `break`'s target is the `switch` that owns it, so the producer covers that construct as the region the jump makes non-universal and reduces the row's reach to `unknown` — it used to **drop** the row, and since a dropped `CallExpression` leaves no uncensused-form row either, the marker was the only trace. `mount` is now dispositioned by local recursion, which is what certifies it: never a relaxed marker |
| `whileBreak` | **certifies** with a recipe | the same, with the `break` owned by a `while` |
| `labelledBreak` | refuses: unaccounted flow | `break outer` out of a plain labelled *block*. No enclosing loop or `switch` of the frame owns that target, and the target is what bounds every region-based repair either census applies to a jump, so the marker is `jumpReachability`, classified `flow-unaccounted`. `mount(el)` is on the wire here too — the refusal is not about a missing row but about a frame nobody modelled, and it is what keeps the relaxation above from being a blanket one |
| `stdlibRefInvoker` | refuses: unseen callable | `Array.from(items).forEach(work)`: `forEach` invokes its slot 0 (reviewed invoker table) and `work` is a module-local function reference — neither a parameter nor a callable literal inside the transcript — so the standard-library disposition refuses the call by name |
| `reflectApply` | refuses: by-reference member | `Reflect.apply(work, undefined, args)` transfers control to its first slot; the member refuses by qualified name whatever the slots prove |
| `constBound` | **certifies** | `boundHelper(callback)` resolves to an arrow a `const` holds. Since 2026-09-06 the verifier binds the arrow to the declarator that holds it — the initializer is a function literal and the identifier is the whole binding — and holds that identifier to the same two checks as a named declaration (unwritten, declared once), then walks the arrow's transcript; `callback(0)` inside it is parameter-rooted. Before that every callable a variable held refused as having no binding identifier |
| `callInitialized` | refuses: value-initialized binding | `wrappedHelper` is a `const` holding what `wrap(…)` returned. The identifier binds, but its initializer is not a function or arrow literal, so the census refuses by name rather than tracing the value |
| `reassignedHelper` | refuses: written binding | `function helper` is reassigned at module level; the producer still resolves `helper(el)` to the declaration, and the verifier's own parse of the authenticated bytes finds the write and refuses to walk a declaration not proven to be the code that runs |
| `memberParameterRooted` | **certifies** with a recipe (ADR 0034) | `source.read()`: the call is `parameter-rooted` and the read of `.read` — an uncensused `property-access-unknown-accessor` form — is `parameter-rooted-accessor`, because the producer roots its subject at parameter 0, a plain, unwritten binding of this declaration. Whatever getter or trap the read reaches sits on the caller's object |
| `toStringTagViaCall` | **certifies** with a recipe (ADR 0034) | `Object.prototype.toString.call(value)`: the receiver is in the reviewed this-protocol table (`Object.toString`, reach `Get(this, @@toStringTag)`) and `this` is the unwritten parameter, so the site is decided before the by-reference owner rule refuses it |
| `writtenBeforeRead`, `writtenAfterRead` | refuse: uncensused form | the parameter is reassigned in the declaration, so the producer states no subject root; the disposition is deliberately not flow-sensitive, which is why the real `scrollIntoView` pair still refuses |
| `moduleReceiverRead` | refuses: uncensused form | a read on an untyped module-level value: no parameter root |
| `nestedCallableParameterRead` | refuses: uncensused form | `items.map(item => item.value)`: `items.map` is rooted at the export's parameter, `item.value` at the arrow's own — the invoker's value, not this invocation's |
| `callNonLibraryReceiver`, `callLibraryOutsideTable` | refuse: by-reference member | `.call` on a local function, and on `Array.prototype.slice`, which is a library member outside the reviewed table; both stay refused under the owner rule |
| `setterOnParameter` | **certifies** (ADR 0040) | `source.value = 1` on a parameter-rooted receiver. The setter that may run was installed by the caller on the object it passed, so it is the caller's code exactly as its getter is (ADR 0034's premise, in write position); the receipt records the site as `parameter-rooted-accessor-write`, which is precisely what a `writes` census must refuse when it exists |
| `updateOnParameter` | **certifies** (ADR 0040) | `source.value += 1` runs the caller's getter and then the caller's setter. One form, one site, the same premise |
| `chainCallbacks` | **certifies** (ADR 0042) | the `chain` shape five ecosystem packages publish verbatim. `callbacks` is the caller's iterable, so its `Symbol.iterator` and the `next` calls after it are the caller's; `args` is a rest parameter, whose array the *engine* builds, so spreading it records no form at all; and `callback` is what that iterable yielded, so calling it is `parameter-rooted-element`. Three premises, none about this module's code |
| `chainModuleCallbacks` | refuses: uncensused form (ADR 0042) | the same loop over a module-level array. Nothing roots the iterable at a parameter, so neither the iteration nor the callee has a premise |
| `awaitIterateParameter` | refuses: uncensused form (ADR 0042) | `for await…of` over the caller's iterable. The async protocol reaches `Symbol.asyncIterator` and the promise machinery, which no ADR has reviewed, so the form refuses whatever it is rooted at |
| `spreadParameter` | **certifies** (ADR 0041) | `{ ...source }` reads every own enumerable property of the caller's object, invoking each getter among them. The operand is parameter-rooted, so those getters are the caller's exactly as a named read's are |
| `spreadWrittenParameter` | refuses: uncensused form (ADR 0041) | the same spread on a parameter the declaration writes. The binding may hold something other than the caller's argument by the time it is read, and the root premise is not flow-sensitive |
| `destructureParameter` | **certifies** (ADR 0041) | `const { first, ...rest } = source`: each binding element reads a property of the caller's object and the rest element reads whatever own properties remain of it, all rooted at the same parameter |
| `destructureModuleValue` | refuses: uncensused form (ADR 0041) | the same destructuring of the module-level untyped value. No parameter roots it — the boundary is provenance, and it has not moved |
| `setterOnModuleValue` | refuses: uncensused form (ADR 0040) | the same write on the module-level untyped value `moduleReceiverRead` reads. No parameter roots it, so no premise covers the accessor and it refuses exactly as it did before — the boundary is provenance, not position. The receiver must stay *untyped*: written through a module-level object literal the compiler resolves, the member binds as a data property and no form is recorded at all, which would test nothing |
| `typedCoercion` | **certifies** (ADR 0038) | `v > max ? max : v < min ? min : v` over parameters `index.d.ts` declares `number`. The form census is classified under the export's *declared* signature — the one the consumer compiles against and the synthesized veto samples from — on a checked twin of the file, so the operands are `number` and no coercion is recorded; the receipt carries a `census-premise:` site per parameter |
| `untypedCoercion` | refuses: uncensused form (ADR 0038) | `value + 1` where the declaration says `unknown`: the premise binds, and `unknown` is not provably a non-object, so the coercion stands under it |
| `returnedCallbackCoercion` | **certifies** (ADR 0038) | `(p) => p * step`: the declared return type `(p: number) => number` types the returned arrow's parameter contextually, and `step` is declared `number` |
| `declaredMemberCoercion` | **certifies** (ADR 0038) | `axis.max - axis.min` with `axis: { min: number; max: number }`. The two reads stay unknown accessors — a declaration file is not runtime bytes — and are `parameter-rooted-accessor` (ADR 0034); the subtraction's operands are `number` under the premise |
| `helperCoercion` | **certifies** (ADR 0038, helper premises) | `subtract(a, b)` is a local recursion whose `x - y` is over the *helper's* parameters, which have no declared signature. The root's premised census records the argument types at the call — `number`, `number` under the declared signature — as `callArgumentPremises`; the verifier demands the helper's transcript under exactly those types (`parameterPremises` on the local-declaration demand, protocol 23), the helper's own spelled twin re-establishes them, and the coercion clears at depth 1. The receipt carries a `census-premise:` site for the helper's parameters beside the root's |
| `helperSpreadCoercion` | refuses: uncensused form (ADR 0038) | `subtract(...[a, b])`: a spread displaces every slot at or after it, so the call carries no argument premise and the helper is censused over its own `any`. The spread itself clears — `[a, b]` is a `number[]` under the premise |
| `helperUntypedArgument` | refuses: uncensused form (ADR 0038) | `subtract(a, JSON.parse("1"))`: slot 1 is `any` and is stated nowhere, so `y` stays `any` on the helper's twin and `x - y` refuses at depth 1. A premise never widens a slot the caller's own types left open |
| `omittedBoxScale` | **certifies** (ADR 0051) | `geometryHelper(value, 0, 1, 0)` omits its fifth argument, so the helper premise carries `undefined`; `boxScale !== undefined` narrows the first `scaleGeometryPoint` call's scale slot to explicit `never`. The leaf's arithmetic has no coercion, and its primitive completion dispositions the parent addition. The receipt carries the helper census premise at slot 1 as `never` and the parent's `primitive-coercion` site |
| `unknownBoxScale` | refuses: uncensused form (ADR 0051) | the explicit fifth argument is `unknown`, so the guarded leaf call's scale remains possibly object-valued and its multiplication is an actual coercion |
| `untypedBoxScale` | refuses: uncensused form (ADR 0051) | the explicit fifth argument is `any`, which states no non-object premise; the same leaf multiplication remains an actual coercion |
| `iife` | refuses: unresolved callee | an immediately-invoked function expression. Its body is lexically inside the export and already walked, so the generator has no counterexample to name — and the census refuses the row by name: the producer resolves its callee to nothing at all. Which is why the walk keeps declining `expression-callee` rather than treating it as spurious |

The `unresolved`, `taggedTemplate`, `spreadUntyped`, `labelledBreak`,
`stdlibRefInvoker`, `reflectApply`, `reassignedHelper`, `callInitialized`,
`iife`, the seven ADR 0034 boundary refusals and the two ADR 0038 ones arrive
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

- **`plain`, `noRecipe` and `overloaded` must stay byte-identical in body.**
  The difference between their rows is the corpus and the declaration, and
  only those; `overloaded` must keep exactly two overloads in `index.d.ts`.
- **`cycle` uses a boolean stop argument.** A numeric `depth > 0` guard on an
  untyped JavaScript parameter would itself be a coercion form and obscure the
  graph premise. Boolean truthiness and literal arguments keep the sample
  finite without adding another invoking form.
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
  a later statement.** Before 2026-09-06 a `const` arrow refused as an
  anonymous callable before any write was consulted; it is now followed
  through its binding and a write refuses it on the same terms, so the row
  keeps the `function` spelling to pin the declaration-node case on its own.
- **`constBound`'s `boundHelper` must be an arrow that is the whole
  initializer of a `const` binding one plain identifier, unwritten and
  declared once**, and **`callInitialized`'s `wrappedHelper` must be
  initialized by a call.** The first pins the one indirection the census takes
  through a variable; the second pins that it takes no other.
- **`typedCoercion`'s parameters must be unannotated in `index.js` and
  `number` in `index.d.ts`**, and **`helperCoercion`'s coercion must sit in a
  helper the export calls, not in the export, with `subtract` itself
  unannotated and shared with `helperSpreadCoercion` and
  `helperUntypedArgument`.** The first pins that the census reads the declared
  signature the consumer compiles against rather than the implementation's own
  `any`; the second pins that the premise reaches a helper only through the
  argument types at the reaching call — the same helper certifies from one
  caller and refuses from the two whose calls carry no premise for a slot. A
  JSDoc type on any of the three functions would be a different premise: the
  producer refuses to annotate over an existing one.
- **`dependency-consumer/plainConsumer` must not call into `solid-js`.** The
  gate has to be *reached* for that fixture to say anything about the
  workspace, and a dependency callee refuses the census by name first.
- **`dependency-consumer/node_modules/solid-js` must stay tracked and must
  keep exporting a working `record`.** It is copied into the private workspace
  and executed there; the recipe refuses the gate on any other answer, and an
  untracked stub removes the only fixture that proves a consumer recipe can
  import the package under test at all.
- **The ADR 0043 negatives must sit on `untypedRegistry`, never on an object
  literal.** Written `defaultedFromModuleValue(source = { value: 1 })` the
  compiler binds `value` as a **data property of a literal in this file**, so
  the producer records no form at all and the export certifies without the
  census ever reaching the premise. All three defaults —
  `defaultedFromModuleValue`, `patternParameterDefault`,
  `patternElementDefault` — did exactly that on their first draft. The same
  vacuity that `setterOnModuleValue` guards for ADR 0040: a negative over a
  member of a value this module wrote has to be a member the compiler cannot
  bind, or it pins nothing.
- **`defaultedFromParameter`'s default must be a bare reference to another
  parameter, `defaultedFromDefaulted`'s must name a *defaulted* one, and
  `patternRestParameter` must keep a plain sibling beside its rest element.**
  The three pin the derivation boundary: one reviewed hop from a rooted
  parameter, not a chain, and a rest element's object is the engine's however
  its siblings are rooted.
- **`localBindingFromParameter` must bind twice.** One declaration would pin
  the leg but not the fixpoint — the second declaration roots only once the
  first has, and a single-pass implementation would pass a one-hop fixture.
- **The ADR 0044 tables must be read with a computed key, and `key`/`index`
  must stay `any` in `index.d.ts`.** A literal key binds a data property and
  records no form at all — which is the very premise these exports make
  explicit, so a literal-key fixture would certify vacuously and pin nothing.
  `accessorTable` must keep its getter, `protoTable` its `__proto__:` member,
  and `mutableTable` its module-level reassignment: each is the one thing that
  disqualifies an otherwise data-only literal. `ownTableMemberRead` must read
  one level further than `ownTableRead` over the *same* table, so the pair
  pins that the premise roots a direct reference and never a chain.
- **The ADR 0045 helper must hand back its own argument, and the library
  negative must call an `any`-returning member.** Written
  `scaleBy(value, factor) { return value * factor }` the completion is a
  `number` whatever the parameters are — `*` always yields one — so the call
  site is already a primitive and no coercion form is recorded at all. Written
  `Math.min(base, 1) + base` the same thing happens through the declarations.
  Both drafts certified without the census reaching the premise.

  **This trap has now caught three ADRs in a row**, in three dresses: a literal
  default whose members the compiler binds (ADR 0043), a literal key that binds
  a data property (ADR 0044), and an operand the checker already types as a
  primitive (ADR 0045). Before believing any negative in this fixture, check
  that the form it is supposed to refuse on is actually recorded — a fixture
  that pins a refusal over no form pins nothing at all.
