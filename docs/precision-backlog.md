# Precision backlog

## Reachability floors, and reading a parameter without calling it (2026-09-01)

The certifier was refusing evidence it already had, and the fix for that turned
out to depend on a fact the producer was not recording. Two changes consume a
fact the producer already emitted; one repairs a disagreement between two
censuses over the same syntax; and one — the reachability of a parameter use —
adds the missing fact, at the cost of a wire protocol break, because admitting a
read witness without it meant admitting reads from dead code.

An adversarial review of the first three found exactly that hole and four
smaller ones. What follows is the corrected round; every claim below is pinned by
a test that fails without it.

### One walk, so two censuses cannot disagree

The call census and the parameter-use census both classify a node by where it
sits in an implementation body, and they used to walk it separately — the call
census threading reachability, the use census threading nothing at all. They now
share `walkImplementationBodyLocked`
(`apps/solid-typefacts/internal/typefacts/tsgo/invocation_transcripts.go`), which
threads the two facts either census reads (nested-callable capture, fall-through
reachability) and hands each node to an `observe` callback. A `ParameterUse`
carries a `reach` field answered by that walk, so a `props.children` and a
`sink(…)` on the same statement can never be given different answers.

This is a wire break: **Type Facts handshake protocol 7 → 8**, frozen schema
digest `sha256:f1ecaab6c1e0f4369f34490a06b262cc01cc1e9a867946e93fb26def3aa7c494`,
`reach` required on `parameterUse`. A protocol-7 client refuses unknown
transcript fields and a protocol-7 producer omits a field this protocol
requires, so neither direction is a compatible extension.

Three correctness repairs came with the shared walk, each pinned:

- **`try { … } finally { return; }` no longer leaves the following statement
  reachable.** The finally block's own completion is conjoined into the arms'
  merge instead of being discarded (`conjoinReachability`), because a jump in a
  `finally` overrides the one it interrupted. Its *contents* are still visited at
  the reachability the `try` statement had, so `try { return } finally
  { cleanup() }` still reports `cleanup()` as reached
  (`TestCallCensusStopsAtAFinallyThatReturns`,
  `TestCallCensusRunsAFinallyThatFollowsAReturn`). `controlFlowCensusLocked` did
  **not** have this defect and was not changed: it treats a `try` as an opaque
  construct and `constructCompletesNormally` refuses any `return` inside one, so
  it answers `unknown` where the call census now answers `unreachable`. The two
  still differ there, in the safe direction — the control-flow census never
  claims more than the call census. Making it precise would mean giving it a real
  `try` arm, which also owns its `tryReachability` unsupported marker; that is
  not this slice.
- **A construct entered from dead code stays dead.** Both censuses returned
  `ReachUnknown` after a loop reached at `Unreachable`, quietly promoting code
  after a `throw` to "may execute". They now return `Unreachable`.
- **A branch a literal condition excludes is dead code.** `if (false) { … }` is
  as unreachable as code after a `return`, and calling it reachable is what let a
  property access there witness a read the export never performs.
  `literalBranchReachLocked` rules out the arm a decidable literal condition
  excludes — including the implicit `else`, so nothing falls through
  `if (true) { return }`. Applied identically in both censuses.

### The call census poisoned every statement after a loop; the control-flow census did not

`implementationCallCensusLocked`
(`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`)
returned `ReachUnknown` after *any* iteration or switch statement, so a single
`for (const [k, v] of Object.entries(props))` made every later call in the
implementation unusable as execution evidence. `controlFlowCensusLocked` had
already answered exactly this question for return sites — reachability *inside*
a construct stays unknown, reachability *after* one that can only complete
normally is what it was before it — with `constructCompletesNormally`. The call
census simply never received that fix, so the two censuses disagreed about the
same syntax.

It now calls the same helper: `Reachable` after the construct when the census
arrived `Reachable` and the construct provably completes normally; `ReachUnknown`
for the construct's children, and for a construct that cannot complete normally
(`while (true)` with no `break` out, a `return`/`throw`/escaping jump inside, a
nested construct control can never leave). Pinned by nineteen shapes in
`TestImplementationCallCensusKeepsCallsAfterAFallThroughConstructReachable`,
which also asserts that every call *inside* the construct stays `ReachUnknown`,
and by `TestImplementationCallCensusNeverPromotesAnUnreachableConstruct` for the
one-way direction. This lifts `createRenderEffect` and `onCleanup` in
`@solid-primitives/script-loader@3.0.0-next.2` back to `Reachable`, which is
what `require_owner_operation_call` asks for; that row now certifies.

**Which loop headers cannot end their loop** is read from the program's own
text, and two indirections were reading as exitable when they are not:
`const ALWAYS = true; while (ALWAYS)` (a unique, never-written `const`
declaration, the same gate `collectReturnedCallablesThroughBindingLocked` uses)
and `while (1 === 1)` (an equality comparison of two operands that each reduce
to a primitive the text spells, refused across primitive kinds so no coercion
rule is modelled). Both are now decided, so the dead code after such a loop is no
longer promoted.

**Retained approximation, stated rather than left implicit.** A condition no
literal reading decides — `while (flag)`, `while (fn())` — is still treated as
having an exit edge, so the statements after it keep the reachability they had.
A `flag` that is provably always true therefore still yields "reachable" for code
that never runs. This is deliberate: the alternative is asking the type checker
whether an arbitrary expression is truthy, which would make reachability depend
on inference rather than on the program's text. It is pinned as a decision by the
`nonLiteralConditionStaysOptimistic` row, and the doc comment on
`alwaysTruthyLiteralConditionLocked` — which previously described this direction
backwards — now states it. Closing it needs a real value analysis, not a wider
literal reader.

### `ReachUnknown` is adequate evidence for a claim whose lower bound is zero

`implementation_call_is_executed`, the `OperationKind::Read` branch, and
`require_owner_operation_call`
(`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`)
required `Reachability::Reachable` of every witness, for every demand. A call in
a `do … while` or `for … of` body is `Unknown` by construction — the body may run
zero times — so a demand claiming *"may happen, zero or more times per call"* was
refused by evidence of exactly the shape it claims.

**Policy decision** (signed off by the policy owner, and it is a claim-strength
decision, not a bug fix): a demand whose bound operation asserts no lower bound
is witnessed by a call the implementation *may* execute. `ReachabilityFloor`
names the two readings, `reach_admits_zero_lower_bound` states the premise, and
the floor is derived from the demand's own bound and plumbed to each call site.
It is never global.

**Relaxing takes positive evidence, and `min == Some(0)` is the whole of it.**
An operation carrying no cardinality at all has stated no bound, and silence is
not the claim "zero or more": it keeps the strict floor. This is deliberately
narrower than `Cardinality::strength`, which folds absence in with zero because
it answers a different question ("may this be assumed guaranteed", where the two
agree). The planner is where the difference is visible, and it is pinned there:
an operation with no stated cardinality is marked `has_cardinality: false` by
`inventory_export_facts` and is then demanded `operation-reachability` *without*
an `operation-cardinality` demand, so no bound of its is ever checked by anything
(`an_operation_that_states_no_cardinality_is_demanded_no_cardinality_family` in
`solid-reactive-ir`). No emitted demand was found where absent cardinality
coexists with occurrence semantics.

What keeps refusing, each with a test:

- `Reachability::Unreachable`, for any claim. Code after a `return` or a `throw`
  never runs and witnesses nothing. Neither floor admits it.
- an operation with `min >= 1`. The strict floor stands. This is the trap of the
  tier: it looks like the same relaxation and is not
  (`reachability_floor_follows_the_bound_operation_lower_bound`).
- the reachable-return-site premise inside `implementation_call_is_executed`,
  unchanged: a captured call still has to lie inside a callable a **reachable**
  return site carries (`loop_body_call_executes_only_under_the_zero_lower_bound_floor`).
- the `OperationKind::Return` branch's reachable-return requirement, untouched.
- recursive value-shape demands, which assert what a position *is* rather than
  how often it is reached, keep the strict floor explicitly. That decision now
  lives in `operation_input_value_shape_evidence`, split from the plan lookup so
  a test can hold it: `recursive_value_shape_evidence_keeps_the_strict_floor`
  fails if the floor there is relaxed.
- every fail-closed default in `callback_reachability_floor` — the floor
  `argument-binding` and `callable-path` take, the two families that assert no
  occurrence of their own. A non-callback subject, an ordinal past the callback
  list, an ordinal naming a callback bound to a different operation, and an
  operation absent from the export all read no bound and keep the strict floor
  (`callback_floor_reads_the_bound_operation_and_fails_closed_everywhere_else`).

`demand_id` does not move: it hashes the static policy manifest, the candidate
semantic digest and the artifact roots, never the verifier's implementation.
`@solid-primitives/marker`'s `mapMatch(text)` inside a `do … while` is the row
this clears.

### Reading a parameter is not calling it

`require_operation_evidence`'s `Read` branch inspected `implementation.calls`
only, so the only read it could see was one where the parameter *was the callee*.
`const mapFn = props.children` reads `props` and calls nothing;
`parameterUseCensusLocked` has been recording that exact fact all along
(`ParameterUse{kind: propertyAccess, captured: false}`), and the branch never
looked at `parameterUses`.

It now accepts either witness. A `Read` operation whose input is
`Parameter{index, path}` is discharged by a `ParameterUse` with a matching
parameter index and binding path, `captured == false`, and kind in
`{propertyAccess, directCall, aliasCall}`. Path matching is the same rule the
call branch uses: the observed path may be *longer* (reading `props.of.keys` is
reading `props`; using a destructured `{ children }` is using the object) and
never shorter, and every segment the demand names has to be that exact property.

**The use witness answers to the same floor as the call witness.** It reads the
`reach` the producer now records for every use, so a `props.children` after a
`return`, after a `throw`, or in a branch a literal condition excludes is
`Unreachable` and witnesses nothing for any claim, and a use in a loop body is
`Unknown` and witnesses only an operation whose own lower bound is zero
(`parameter_read_use_answers_to_the_same_floor_as_a_call`). Exempting this loop
from the floor was how a `min >= 1` read demand came to be dischargeable from
dead code.

What keeps refusing (`parameter_read_accepts_an_uncaptured_property_access_use`):
`storage` and `return`, which hand the value somewhere without looking into it;
`argumentKnown`, `argumentUnknown`, and `capture`, which do the same through a
call or a closure; `unknownEscape`, which is the census saying it could not
classify the use at all; and any captured use, for the reason a captured call is
refused — nothing proves the closure holding it runs. `@solid-primitives/keyed`'s
`MapEntries` is the row this clears; its only parameter-rooted *call*,
`props.of.keys()`, is captured inside the `mapArray` source arrow and stays
refused as such.

The refusal tail moved with the branch: `parameter-rooted read has no exact
implementation call` is now `… no exact implementation call or use`.

### Still open, and two of them are not what the diagnosis thought

- **`@tanstack/hotkeys@0.8.0::formatHotkey`** (demand
  `sha256:12a42717…`) stays refused, and *not* for a reachability reason: with the
  floor forced relaxed it still refuses. The read operation's input is
  `Parameter{index: 0, path: ["includes"]}` while the shipped access is
  `parsed.modifiers.includes(…)` — the demanded path drops the intermediate
  `modifiers` segment. The census is refusing correctly.
- **`@solid-primitives/keyed`'s `SetValues`** (`sha256:1ac6a2dd…` /
  `sha256:0baa9fc5…`) is the same defect: the demand asks for
  `Parameter{0, ["values"]}` where the code accesses `props.of.values()`. The
  transcript records the uncaptured `props` property access, which does not name
  `values`, so nothing witnesses it.

  Both belong with the wrong-demand class already filed against
  contract-proposal generation (`rust/crates/solid-reactive-ir`) — a read input
  path built from the last segment of a nested member access rather than the whole
  path. They leave the refusal list by being corrected, not by being discharged,
  and widening the match to "the demanded segment appears somewhere in the
  observed path" would be a guess, not a proof.
- **`@solid-primitives/marker`**'s next demand refuses with `operation-input is
  unsupported: implementation census only binds exact parameter-rooted operation
  inputs` — a pre-existing fail-closed path this slice does not touch.
- **An object rest element is named after its binding, not after a property that
  exists.** `parameterCensusRootsLocked` takes `element.Name()` as the property
  segment for every object binding element, so `function f({ a, ...rest })`
  yields the root path `["rest"]` — but `rest` is the remainder object, not
  `props.rest`. A demand for `Parameter{0, ["rest"]}` would therefore be
  witnessed by any use of `rest`. **Pre-existing**: the call census builds
  `bySymbol` from these same roots, so `parameter_value_source_matches` has
  always had it; the read-use witness is now a *second* consumer of the same
  roots, which widens the blast radius without introducing the defect. The fix is
  to emit no path segment for a rest element (or a distinct segment kind that no
  property demand can satisfy) — a producer change with its own fixture, not a
  verifier one. Unmeasured: no probed row is known to demand such a path.
- **A write to a property is recorded as a `propertyAccess` read of the object.**
  `props.children = 1` records `kind: propertyAccess, captured: false` on
  `props`, because `parameterUseKindLocked` classifies on the parent being a
  property access without asking whether that access is an assignment target or a
  `delete` operand. **Reviewed and deliberately not changed**: evaluating `props`
  is required to store into it, so the record is a true read of `props` at path
  `[]`, which is the only demand it can witness. Excluding it would need the
  observed path to *name* the written property, and it never does — the recorded
  path is the parameter's binding root, and `parameter_binding_matches` requires
  the observed path to be at least as long as the demanded one, so a write to
  `props.children` cannot witness a demand at `["children"]` today. The exclusion
  is vacuous until some matcher admits an observed path shorter than the demanded
  one; if that ever changes, this becomes real and the fix is a `write` flag on
  the use plus the written segment.
- The rows whose refusal is over-claimed inventory (`flux-store` ×3,
  `@solidjs/meta@0.29.4`) and whose invoking position is in another package
  (`@solid-primitives/until@0.1.1`) still refuse, with the same demand digests and
  reason tails. Every generated contract in the probed set is byte-identical
  before and after, including the newly certified `script-loader` row: the
  verdict moved, the contract did not.


## A callback invoked from a closure that flows into an invoking position (2026-09-01)

`implementation_call_is_executed` had exactly one premise for a *captured* call —
a call inside some nested callable: the call site lies **somewhere inside** a
callable that a reachable **return site** provably carries. That premise is the
whole of the returned-closure family, but it is not the only way a nested
closure runs, and — as the review of this entry's first draft established — byte
containment was never the right join for either half of it.
`@solid-primitives/autofocus` is the clean counterexample for the missing
premise: its entire body is

```js
createEffect(() => { const el = ref(); … })
```

and it returns nothing at all. The closure runs because `createEffect` runs it.
Every carried-callable list is empty, so `ref()` was refused as unexecuted and
the export's `invoke` claim stayed open — a missing premise, not an imprecise
one.

### The added premise, and the join that replaced containment

A captured call is also executed when the callable **immediately containing it**
is one an **already-executed call of the same implementation** hands to an
argument slot *proven to be invoked*.

The join is exact identity of that enclosing callable, not containment. A new
producer fact, `ImplementationCall.enclosingCallable`, names the innermost
callable containing each call (absent exactly when `captured` is false), and the
premise asks whether *that* range is carried — by a reachable return site, or at
a proven-invoking argument slot. Longer paths are reached by **composing** links,
never by observing that bytes nest, and the recursion is what composes them.

Containment was unsound, and one shape shows why:

```js
function storesInner(callback) {
  effect(() => {
    const inner = () => { callback(); };
    registry.push(inner);
  });
}
```

`callback()` lies inside the range `effect` invokes, and `callback` never runs:
the effect only *stores* `inner`. Containment cannot separate that from the
debounce shape, which nests identically —
`return (…) => { setTimeout(() => callback(…), wait) }` — and really does run.
Under composition they separate cleanly: the debounce case is two proven links
(the returned closure is carried by the return site; the arrow immediately
containing `callback(…)` is carried at slot 0 of a `setTimeout` whose own
enclosing callable is that returned closure), and the storing case has no
second link, because `Array.prototype.push` is on no invoking table. **A callable
that is merely defined, stored, pushed, or assigned breaks the chain**, and the
demand stays open. Pinned by
`a_merely_stored_inner_closure_breaks_the_invoking_chain`, which runs both
shapes through the same fact skeleton, and by
`TestEnclosingCallableNamesTheImmediatelyContainingCallable` on the producer
side.

The same discipline applies to the **return** branch: it too matches the exact
enclosing callable rather than containment, and the debounce/scheduled controls
still certify because they compose. The producer reports the argument side as
`ImplementationCall.argumentCallables`; a spread ends the exact slots — see
"What a nesting boundary is, what a slot index means" below.
The outer-call recursion is bounded at depth 8 / 256 nodes, and exceeding a
bound answers "not executed".

**The argument descent is narrower than a return site's.** A returned value hands
the caller everything inside it, so `carriedCallableLocationsLocked` may credit
every element of `[fn, clear]`, every property of an object literal, and
`Object.assign`'s target. An invoking argument slot may not:
`addEventListener("click", { handleEvent, spare })` calls exactly `handleEvent`
and never `spare`, so crediting both would assert execution of code the runtime
cannot reach. `singleCallableLocationsLocked` keeps only the identity-preserving
single-callable forms — the callable expression itself, the wrappers that erase
at runtime, and a single-declaration binding naming exactly one callable — and
the two descents are one function parameterized by `carriedCallableDescent` so
the difference is stated rather than drifted into. An object or array literal at
an argument slot therefore carries **nothing at all**, refusing the construction
rather than trying to pick the right member out of it.

### The three tiers, and where each stops

- **Tier A — dialect primitive.** The callee resolves to an exact `solid-js`
  import and `solid_dialect::unambiguous_callback_argument` fixes the slot as a
  callback. This is byte-for-byte the gate `require_parameter_callback_flow`
  already applied, so it adds no dialect surface. It refuses
  `createEffect(fn, initialValue)` at slot 1, a locally shadowed `createMemo`
  (whose `target_module` is not `solid-js`), and an unresolvable namespace member
  (which carries neither target nor module).
- **Tier B — reviewed default-library member.** A new producer fact,
  `defaultLibraryInvoker` + `invokedArguments`, emitted only when the callee
  resolves **by default-library symbol identity** — the way
  `isDefaultLibraryObjectAssignLocked` resolves `Object.assign` — to a member of
  the fixed table in `invoking_positions.go`. The verifier owns the table too:
  `DefaultLibraryInvoker::from_wire` refuses an unrecognized name outright, and
  the slot must appear in *both* the transmitted list and the verifier's own
  table, so a widened or forged claim from the producer process is not evidence.
  Refused on their merits: `removeEventListener` (removing a handler is not
  evidence anything runs), a user type's own `forEach`, an `any`-typed receiver,
  a locally shadowed `setTimeout`, and — the named trap —
  `navigator.geolocation.watchPosition`, which really does invoke its callback
  and is deliberately absent. Growing the table is an act of review; "the
  browser probably calls it" is never a premise.
- **Tier C — package-local helper.** The producer proves what the resolved local
  callee's own body does with its parameters (below) and the verifier consumes
  that fact rather than re-deriving semantics from a location. It refuses a
  callee that stores its parameter, one that returns it, one whose
  implementation is outside the analysed program, and any member or computed
  callee — `handlers[key](cb)` names no single canonical symbol.
- **Tier D — external package. Still open, deliberately.**
  `@solid-primitives/until` hands its condition to `createBranch` from
  `@solid-primitives/rootless`, and nothing in this artifact's transcript can
  prove what that function does. The only sound route is an accepted dependency
  contract for `@solid-primitives/rootless` composed through
  `ProofFamily::AcceptedDependencyComposition`. A "well-known package" name list
  is exactly the shortcut the precision contract forbids, so
  `sha256:18a2f9ab…` stays refused.

### The callee-parameter facts, and the strong/weak split

For a call whose callee resolves to a callable with a body in the analysed
program, the producer records three nested sets:

- `calleeDirectlyCalledParameters` — the parameter appears as the **callee** of a
  call in that body. The only one that by itself says the position is used as a
  function.
- `calleeStronglyInvokedParameters` — directly called, or forwarded as a **bare
  identifier** at slot *j* of a further local callee whose slot *j* is itself
  strongly invoked. Every hop is a plain forward and the chain terminates in a
  direct call.
- `calleeInvokedParameters` — the weak closure: strongly invoked, or the
  parameter reaches a reviewed default-library invoker, or it is forwarded to a
  local callee whose slot is merely invoked. This says the value *runs*; it does
  not say the callee calls it.

The split is load-bearing. `require_parameter_callback_flow` — the strong
`callable-path` fallback — accepts only
`calleeStronglyInvokedParameters`. Accepting the weak fact there would let a
chain that terminates in `addEventListener` discharge a claim that the position
is *callable*, which silently turns the `callable-path` family into
`argument-binding`. `require_parameter_flow` gains nothing: its argument branch
already accepts "argument of an executed call with a resolved target", so adding
S4 there would be redundant width. Bounds: depth 4, a per-body node budget, a
symbol cycle guard, and a per-callee-symbol memo that stores only an *exact*
answer — one reached with no depth cut, cycle cut, or exhausted budget — because
a truncated answer is bound to the context that truncated it.

A parameter that is returned, stored on a property, or pushed onto an array
appears in none of the three, including behind a condition
(`function maybe(fn, on) { if (on) queue.push(fn); }`): every credit is a call
position or a plain forward into one, never "flows somewhere".

Two restrictions on *where in the body* a credit may come from carry the weight
of all three sets, and both were review findings against the first draft:

- **A call site inside a nested callable is credited only by composition.** A
  call the body writes down is not a call the body makes.
  `function storeLater(fn) { registry.push(() => { fn(); }); }` never invokes
  `fn`, and crediting the call site because its bytes sit inside the body made
  the closure-wrapped forward indistinguishable from a direct call — defeating
  the property-storage refusal above by the single act of wrapping the stored
  value in an arrow, and feeding both the Tier-C execution premise and the
  S4-strong `callable-path` branch. So the site counts exactly when the callable
  *immediately* containing it is handed to a slot something proves invoking, on
  a call the body itself executes, recursively (see "Composition inside the
  callee body" below). `registry.push` proves nothing about what it is handed,
  so the stored closure still breaks the chain and the demand stays open.

  There is deliberately **no return-site route** inside a callee body. A callee
  that returns a closure hands it to its own caller, which here is the body
  being analysed — and that body only *called* the callee, so nothing runs the
  closure. `function returnsClosure(fn) { return () => fn(); }` proves nothing
  about `fn`.
- **An unreachable call site contributes nothing.** Statements after a `return`
  or a `throw` are text, not execution. `ReachUnknown` is still credited, which
  is deliberate and consistent with the in-body guarded-call precedent below and
  with the claim strength stated in the next section. The walk mirrors
  `implementationCallCensusLocked`'s reachability so the two agree about what
  "this body executes that call" means.

### Claim strength: this premise says *can execute*, and nothing may read more

The Tier-B table is a **may-invoke** table by construction — the producer's own
comment says "zero or more times" — and the premise it feeds is a *can execute*
premise. That is a deliberate match, not an oversight, and it is written down
here so no consumer quietly reads a lower bound out of it:

- `p.then(onFulfilled, onRejected)` lists **both** slots. At most one handler
  ever runs, and neither runs if the promise never settles.
- `items.forEach(cb)` invokes nothing when `items` is empty; `.some` / `.find`
  / `.findIndex` / `.every` short-circuit, and `.sort` never calls its
  comparator for length ≤ 1.
- A guarded call inside a callee body (`if (flag) fn()`) is credited at
  `ReachUnknown`, the same strength the in-body direct-call premise has always
  had.

What the premise licenses is exactly the `Invoke` operation's own claim: this
argument is a callback position the implementation runs, with **no lower bound
on how many times**. `OperationKind::Invoke` demands nothing stronger, and the
inventory records min 0 / max many. Anything that later wants "at least once"
needs a different fact, not a stronger reading of this one — and the tier tables
stay membership-reviewed rather than grown by analogy, because a table entry is
a statement about a specific member's runtime.

### Open policy question, inherited rather than introduced

`callable-path` asserts the callback binding's value **is callable**.
`@solid-primitives/utils` ships `access` as
`(v) => typeof v === "function" ? v() : v`, and discharging
`@solid-primitives/date-difference::createDateDifference` through it proves "the
implementation calls this position as a function on some path", not "this
position is always a function" — the published typing is
`MaybeAccessor<number | Date | string>`, which genuinely is not always callable.

That looseness is **not new**: an in-body `typeof date === "function" ? date() : date`
passes `require_parameter_callback_flow` today, unchanged, and has since the
family existed. S4-strong inherits exactly that strength through
`calleeStronglyInvokedParameters` and introduces nothing weaker. The open
question for the policy owner is whether `callable-path` should instead mean
"always callable"; if it should, the *existing* guarded direct-call acceptance
needs tightening first and `createDateDifference` becomes a wrong demand rather
than a discharged one. Recorded here rather than silently settled.

### `addEventListener` cannot be resolved in a shipped bundle

`@solid-primitives/gestures@1.2.1::registerPointerListener`
(`sha256:08be4b78…`) was expected to clear through Tier B and does **not**. Its
runtime artifact is bundled JavaScript, and certification's private project runs
it with `allowJs: true, checkJs: false`, so `registerPointerListener`'s own
`node` parameter is `any` and `node.addEventListener` resolves to no symbol at
all. That is the same `any`-typed receiver Tier B already refuses by design, so
the row stays open with no fact emitted — the table declining to assert what it
cannot resolve, not a gap in the table.

The consequence is general: **the member half of the Tier-B table is largely
unreachable for bundled artifacts**, and only bare globals (`setTimeout`,
`requestAnimationFrame`, …) survive the erasure, because a global's own
declarations are in the default library however the calling file is typed. That
is why `@solid-primitives/utils::afterPaint` is provable and
`registerPointerListener` is not. Pinned by
`TestDefaultLibraryInvokerOverAShippedRuntimeArtifact`, which runs both shapes
through a real `.js` artifact with the certification project's options. Closing
it would need the *declared* signature's parameter type transferred into the
untyped implementation — a new cross-artifact semantic claim, out of scope here
and not attempted.

### A construction runs what it carries, and is still not a call

The implementation call census recorded call expressions only, so a `new`
expression was not in `implementation.calls` at all and the ES executor position
had nothing to carry a fact — the `PromiseConstructor` row the Tier-B table
lists was dropped as unreachable. Under containment that gap was invisible;
under composition it cost a row outright, because
`@solidjs/signals::action` is

```js
function action(genFn) {
  return (...args) => { … return new Promise((resolve, reject) => { const it = genFn(…args); … }); };
}
```

`genFn(…)` is enclosed by the Promise executor arrow, and with no fact saying
that arrow runs the chain stopped there. (Containment had "discharged" it by
never asking what runs the executor — the same unsound step, on a case where the
answer happens to be right.)

**Construct expressions are now in the census**, with the same fields a call
carries: reachability, `enclosingCallable`, target resolution through the same
exact-symbol machinery, and the argument facts. The reviewed table gains its one
construct row, in its own table keyed separately from the call tables, because
the two questions differ: `Promise(fn)` is not a call the language allows and
`new setTimeout(fn)` is not a construction it allows. `new Promise(executor)`
invokes argument 0 — the ES specification requires the executor to run
synchronously, before the constructor returns — resolved by default-library
symbol identity like every other row. Still refused: `new UserClass(cb)` (not
the library symbol), a locally shadowed `class Promise`, and
`new Promise(...args)`, whose spread fixes no slot and therefore carries no
proven callable.

**Both kinds run what they carry; only one of them is a call.** The kind travels
with every census entry (`ImplementationCall.kind`), and the two halves of the
verifier read it differently on purpose:

- the execution premise — `implementation_call_is_executed` and
  `argument_slot_is_proven_invoking` — is kind-agnostic. It asks whether code
  runs, and a construction runs its executor.
- every claim whose witness says the implementation *calls* a value asks
  `is_call_expression` first: the invoke flow, the callback flow, the
  parameter-read evidence, the owner-primitive call, and the recursive-parameter
  call. `new Cls(cb)` is a different claim about `cb` than `cls(cb)` and none of
  those families was reviewed for it, so a construction satisfies none of them —
  including the observed-callee list in the owner-requirement refusal message,
  so a refusal tail does not move either. A construct site also states **no**
  callee-parameter facts: those are claims about a resolved *function's* body,
  and a constructor resolves differently.

An *absent* kind deserializes to `CallKind::Unknown`, which those consumers
refuse: absence is never read as "call". An *unrecognized* one does not become
`Unknown` at all — `CallKind` carries no `#[serde(other)]` arm, so it fails
deserialization and the whole transcript is rejected, which is the harder of the
two failures and the intended one. (An earlier draft of this entry claimed both
spellings default to `Unknown`; they do not, and the pin that was supposed to
cover it used `"unknown"`, a *recognized* variant. Corrected and pinned by
`an_unrecognized_call_kind_rejects_the_transcript_rather_than_defaulting`.)
Pinned by `TestConstructExpressionsJoinTheCensusUnderTheirOwnTable`,
`a_construction_executes_what_it_carries_and_is_still_not_a_call`, and
`a_construction_satisfies_no_claim_whose_witness_says_call`, which mutation-kills
each of the six kind gates on its own.

`@solidjs/signals::action` clears as a result, and the three probes it was
masking return to their earlier frontier: `@solidjs/signals@2.0.0-rc.3` and both
`@solid-primitives/intersection-observer@3.0.0-next.3` probes are refused on
`flatten`'s `callable-path` again (`sha256:14ebad21…`, `sha256:71e902d9…`,
`sha256:b84ccad6…`).

### Composition inside the callee body, and the premise the producer refuses to decide

The Tier-C / S4 producer facts used to stop at every nested callable, and one
real chain paid for it. `@solid-primitives/timer` proves
`createIntervalCounter`'s `timeout` callable through
`createPolled` → `createTimer`, and `createTimer` calls its `delay` parameter at

```js
createEffect(prevDelay => { … const currDelay = delay(); … });
```

— a direct call, but inside the arrow `createEffect` runs, not in `createTimer`'s
own body.

The callee-body walk now applies **the same composition premise the verifier
applies to an exported implementation**, one body further in: a parameter call
site inside a nested callable counts exactly when that callable is carried — by
the same identity-preserving single-callable descent — at a proven-invoking slot
of a call or construction in the callee body whose own site composes in turn.
Same bounds as the verifier's premise (depth ≤ 8, 256 nodes), plus a cycle guard
on the call sites themselves, which a self-referential
`const f = () => { g(f); }` makes reachable. The invariant the fixer round
established is preserved rather than weakened: the chain asks each link
separately, so `registry.push(() => { fn(); })`, the closure-wrapped forward, and
the expression-bodied `fn => () => fn()` all still emit **nothing**.

The strength ladder is untouched. Composition changes *where* a call may sit,
never what a chain proves: the terminal must still be a direct call of the
parameter and every interprocedural hop a plain identifier forward, so a chain
that reaches `addEventListener` inside an effect closure is invoked and still not
strongly invoked. `calleeDirectlyCalledParameters` also keeps its narrow meaning
— the parameter is called in the body's **own frame** — because a call the body
reaches only through a closure it hands away is a different claim.

**The last link is often a dialect fact, and the producer may not decide it.**
`createEffect`'s slot 0 is a callback position; the producer knows no framework
vocabulary, and reading one out of a module and a name is exactly the shortcut
the precision contract forbids. So it states the syntax exactly and defers:
`calleePendingInvocations` carries the same two claims, each with the
invoking-slot premises it still needs — module, exported name, slot, argument
count — and the verifier answers them on the table Tier A already owns
(`unambiguous_callback_argument`, `solid-js` only). A premise it does not
recognize leaves the claim unproven, an entry with no premises is malformed
rather than unconditional, and a conjunction holds only if every premise does. A
callee that names no module carries no premise at all, so a local helper, a
member call, a computed callee and a bare global stay refused. Alternatives are
capped at four routes of at most four premises each; a claim needing more is
refused rather than transmitted.

`sha256:209b1b7f…` (`timer@1.4.4`) certifies again, and `sha256:7f353a40…`
(`timer@1.4.5-next.1`, floor and head) clears back to its own next frontier,
*"operation-input is unsupported: implementation census only binds exact
parameter-rooted operation inputs"*. Pinned by
`TestCalleeBodyComposesThroughProvenInvokingPositions` and
`a_deferred_dialect_premise_is_answered_here_or_the_claim_stays_open`.

One consequence worth naming: the earlier note that "dialect invoking positions
inside a callee body are not part of the producer's weak closure" no longer
holds — a helper that hands its parameter to `createEffect` is now credited
(weakly), through the same deferred premise. What stays open is anything whose
missing premise is *not* a dialect slot: an external package's helper is Tier D
either way.

The array-iteration row is also narrower than "`Array.prototype`'s iteration
methods": it is gated on the `Array` and `ReadonlyArray` containers, and the
typed arrays' identically shaped methods are not on the reviewed table, so they
stay open.

### What a nesting boundary is, what a slot index means, and why the answer must not depend on the demand order

Three defects in the extension's own machinery, all of the same family the
rounds before them closed: a *syntactic* fact standing in for a *semantic* one.

**Every function-like body is a nesting boundary.** `isCallableDeclaration` was
`ArrowFunction | FunctionExpression | FunctionDeclaration | MethodDeclaration`,
and every walk that asks "is this a nested callable" asked it — the call census,
the parameter-use census, the control-flow censuses, and the extension's own
callee-body walk. A getter, a setter, a constructor and a class's static block
own bodies too, and none of them was one:

```ts
function stashGetter(cb: () => void) {
  registry.push({ get value() { cb(); return 1; } });
}
```

stores an object and calls nothing, and the census reported `cb()` as an
uncaptured call `stashGetter` itself makes — `captured == false` with
`reach == reachable` is the *strongest* form of every claim built on the census,
and `calleeDirectlyCalledParameters = [0]` discharged the S4-strong branch for
every caller of `stashGetter`. This is not an exotic shape: object-literal
getters are the standard compiled-JSX lowering for a lazy prop
(`createComponent(X, { get children() { … } })`), so every certified artifact
shipping compiled JSX is in it. The predicate is now the compiler's own closed
enumeration — `ast.IsFunctionLikeDeclaration` plus
`ast.IsClassStaticBlockDeclaration`, shimmed for the purpose — so a kind a
future compiler revision adds is covered without an edit here, rather than a
hand-kept list that omits whichever kind has not bitten yet. Pinned in both
walks and in all four member shapes by
`TestEveryFunctionLikeBodyIsANestingBoundary`.

**A spread ends the exact slots.** Every producer site that turned an argument
expression into a slot index counted a `SpreadElement` as exactly one slot, so a
spread of *n ≠ 1* elements understated every later position by *n − 1* — toward
*lower* slots, which is the over-proof direction, because the reviewed tables
list low slots as invoking. `target.addEventListener(...pair, cb)` writes `cb`
second and passes it third, where the runtime reads an options bag and invokes
nothing; the producer credited it as the listener. The producer cannot repair the
shift, because a spread's length is a runtime property of the spread expression.
So the answer is a **floor, never a renumbering** (`exactArgumentSlots`): slots
before the first spread keep their exact meaning and are stated normally, and
nothing at or after it is stated at all. Applied at all five positional sites —
`argumentCallableLocationsLocked`, the census's `argumentParameters` (which keeps
one entry per written argument, so its length still means the syntactic count
every consumer reads, and withholds only the displaced entries),
`calleeBodyWalk.carriedLocked`, `creditForwardedParametersLocked`, and
`slotInvokingProofLocked`. The deferred premise is stricter still: it transmits an
argument *count* as well as a slot, and a dialect answer can turn on that count
(`mergeProps` reads every source below it; `createResource` gives argument 0 a
different role at one argument than at two), so a spread anywhere in the list
withholds the premise entirely — no prefix makes the runtime count knowable.
Pinned, including the do-not-over-poison direction (a spread *after* a slot
leaves that slot exact), by `TestASpreadEndsTheExactArgumentSlots` and
`TestASpreadWithholdsTheDeferredPremise`.

**The callee fact set is a pure function of the program.** The callee memo
answered per callee symbol, and stored only *exact* answers — but whether an
answer is exact depends on the depth it was computed at, because the walk
refuses below `maxCalleeInvocationDepth`. A warm entry therefore lent its
headroom to a later, deeper question: over one unchanged program and unchanged
binaries, an export eight forwarding hops deep carried a pending fact when the
seven shallower exports had been demanded before it in the same session, and
carried none when it was demanded alone or first. The demand list is named by no
receipt, no gate-cache key and no proof digest, so a package could certify in one
run and be refused in the next with nothing changed. The memo is now keyed by
**callee symbol and depth**, which is the whole question: an exact answer met no
cut, so it depends on neither the interprocedural cycle guard's contents nor the
caller's history, and a hit returns exactly what recomputing at that depth would.

The alternative — dropping the depth bound in favour of the cycle guard and the
node budget — was rejected deliberately: it is also deterministic, but it makes
deep chains *productive* that the reviewed bound refuses, which is a soundness
widening, and this round's job is to close over-proofs rather than open new
reach. Keying by depth is the strictly narrowing repair (a warm entry could only
ever have strengthened an answer, so nothing can newly certify), and the cost is
recomputation at up to four depths. Pinned by
`TestCalleeFactsDoNotDependOnDemandOrder`, which answers one program in three
demand orders — deepest alone, ascending, deepest first — and compares the
emitted facts byte for byte.

### The census's concise-body exemption, removed

`implementationCallCensusLocked` used to exempt the implementation's own body
from the nesting test, which for a `const`-declared arrow export with a concise
body means exempting the callable that *is* the return value. On
`export const wrap = (cb: () => void) => () => cb();` the census stamped the
`cb()` site `captured: false` — and three consumers read that field directly and
mean *this body calls it*: the `Read` operation's evidence, the owner-primitive
call, and the recursive-parameter subject. `wrapOwner = cb => () => { onCleanup(cb); … }`
satisfied `implementation-owner-call:` for an implementation that only *returns*
a closure that would.

The exemption is gone: every callable is a boundary. Nothing is lost on the
premise side, and that is why the change is safe rather than merely stricter —
the reachable return site carries exactly the arrow that now encloses the call,
so `implementation_call_is_executed` still composes through `enclosingCallable`
and the returned-closure family is unchanged. Only the claims whose witness says
*call* refuse it now. The two walks that disagreed by intent — the callee-body
walk never copied the exemption — now agree by construction. Pinned by
`TestAConciseCallableBodyIsStillANestedCallable`, which asserts both halves: the
honest `captured`, and the return site carrying exactly that callable.

`parameterUseCensusLocked` keeps the exemption, deliberately and narrowly: it is
a different fact with a different consumer set (`ParameterUseKind::Capture` is a
*refusal* trigger for operation reachability, so the exemption there costs
reach rather than soundness in the families reviewed here), and moving it was not
part of this round. Named below as open.

### Still open after the extensions

- **Tier D — an external package's invoking position.** `sha256:18a2f9ab…`
  (`@solid-primitives/until`) needs an accepted dependency contract, not a name
  list.
- **`addEventListener` in bundled JavaScript**, `sha256:08be4b78…` and the
  member half of the Tier-B table generally.
- **A construct site states no callee-parameter facts.** A constructor whose
  body invokes its own parameter proves nothing here; that resolution was not
  reviewed. No probe in this set depends on it.
- **An object or array literal at an argument slot carries nothing**, so a
  bundle handed to an invoking slot proves nothing about any member of it.
- **A `new` expression's dialect tier.** The deferred premise is emitted for
  call sites only: no construct-position dialect vocabulary was reviewed.
- **Any slot at or after a spread is refused rather than resolved**, and a call
  with a spread anywhere states no deferred premise. `schedule(...pair, cb)`
  really does put `cb` at a fixed position when `pair` has a fixed length; the
  producer cannot read that length, so the honest answer is silence. This is an
  over-refusal, and it is the one the precision contract asks for.
- **A static block is treated as a nesting boundary though it runs eagerly.**
  `class Holder { static { cb(); } }` inside a function body really does run
  `cb()` when the class is evaluated. Stamping it `captured` costs reach and
  never soundness, and the eager-evaluation claim was not reviewed; recorded so
  that a later round widens it on purpose rather than by accident.
- **`parameterUseCensusLocked` still exempts a concise callable body.** The call
  census no longer does. The parameter-use fact feeds a different family, and
  changing it would move `ParameterUseKind::DirectCall` to `Capture` for
  `const`-declared concise-arrow exports, which the operation-reachability family
  reads as an open escape. Not measured in this round's acceptance set, so not
  changed in it.


## A parameter-member read was rooted at its last segment (2026-09-01)

A generated read operation's input is `Parameter { index, path }`, and the
implementation census matches that `path` as a **prefix** of the access it
observes (`type_facts::parameter_value_source_matches`). The generator built the
path from the *last* segment of the member chain alone, so a read of
`parsed.modifiers.includes(m)` published `["includes"]` — a claim that the
parameter has an `includes` property. The observed access begins with
`modifiers`, so the prefix never matched: the demand was not merely imprecise,
it was unwitnessable by any runtime, and the row could only ever refuse.

Two independent packages surfaced it:

- `@tanstack/hotkeys@0.8.0::formatHotkey`, whose shipped `dist/format.js:34`
  reads `parsed.modifiers.includes(modifier)`. The demand
  `sha256:12a42717da8d6151…` carried `["includes"]`; it now carries
  `["modifiers", "includes"]`. That digest is gone from the certification audit.
- `@solid-primitives/keyed`'s `SetValues`, whose `props.of.values()` published
  `["values"]` and now publishes `["of", "values"]`.

`indexes::member_callee_receiver` now walks the whole chain from the callee to
its root and answers the path from that root outward. The rules that keep every
emitted segment a real property of the parameter:

- **the root must be a plain identifier.** `EntitySymbols::at` answers a symbol
  for any span the compiler emitted an entity at, and at a conditional,
  sequence, logical, or call expression that symbol is some *operand's* — not
  the value the chain walks through. Trusting it attributed properties of the
  chain's *result* to a binding that never has them:
  `(k ? options.a : options.b).c.slice(n)` was published as parameter `k`
  reading `["c", "slice"]`, and `options().slice(n)` as `options` having a
  `slice` property. Both are refused now, and the refusal is what makes the
  truncation rule below sound: a path is only ever a true prefix of the real
  access, never a path through a different value;
- a segment the walk cannot name exactly — a computed access `a[b]`, or a
  property whose source text the fact table does not carry — truncates the path
  to the **longest exact prefix from the root**, and never skips a segment or
  guesses one. `props[key].values()` answers `[]`; `props.of[key].values()`
  answers `["of"]`;
- a chain longer than `MEMBER_CALLEE_PATH_LIMIT` (32 segments) is truncated the
  same way, to its first 32 segments from the root;
- the read row is **kept in every one of those cases, not dropped**. Dropping it
  would turn an unresolved access into the negative claim "this export performs
  no parameter read", because the reads domain is emitted `Complete` once it is
  known at all. (Refusing the *whole* access, as the compound-root and
  computed-callee gates do, is a different thing: the export's other accesses
  still publish, and an access that no row describes leaves the parameter
  unclaimed rather than negated.);
- a computed *callee* (`props.of[key]()`) is still refused outright, unchanged;
- a callee that is not a member expression at all is refused, so an ordinary
  `notify(callback)` is never read as a member access on its callee.

**Why a truncated path stays sound under the exact matcher too.** The prefix
argument covers `type_facts::parameter_value_source_matches`
(`actual.path.len() >= expected.path.len()`), which is what the read
operation's own evidence uses (`require_operation_evidence`, `OperationKind::Read`).
Its sibling `parameter_value_source_exact` adds `actual.path.len() == path.len()`,
and read inputs do reach it — at `require_operation_recursive_subject`, where a
recursive-value demand rooted at an `OperationInput` is discharged by a
reachable call whose callee is *exactly* the stated source. That is still a
comparison against the path this contract **states**, never against the access
the path was cut from, so a shorter path remains a weaker claim rather than a
different one. The dangerous shape would be a demand that *extends* the stated
path before matching it exactly, and that is unreachable for a parameter-rooted
read input:

- `contract_semantics::certification::inventory_value_shape` treats
  `ValueShape::Parameter` as a **leaf**, so the only recursive-value demand a
  parameter input produces carries an empty value path — pinned by
  `a_parameter_operation_input_inventories_one_demand_at_the_empty_path`;
- a demand rooted at an `OperationInput` with a *non-empty* value path exists
  only for a structured input (object, tuple, array, choice, promise), and there
  `type_facts::parameter_source` answers `UnsupportedDemand`, which the `?` in
  `require_operation_recursive_subject` propagates — the demand fails rather
  than being discharged, so the `source_path.push(property)` extension below it
  never runs on a truncated read path;
- the `require_parameter_callback_flow` route is additionally gated on
  `callable.asserts_callable()`, and `recursive_value_callability` answers
  `DemandedCallability::Unknown` for every `Parameter` shape;
- the remaining exact-match consumers take `callback.from`, which
  `inferred_contract::normalize_export` always builds with an empty path.

The argument is therefore bounded by two premises rather than by the prefix rule
alone: `Parameter` stays a leaf of the value inventory, and the chain root stays
an identifier. A change to either reopens the channel.

The row still names a path only when every contributing access agrees on it
exactly, and agreement is now decided on whole paths rather than last segments —
`props.source.slice()` and `props.other.slice()` no longer collapse into one
agreed claim about `slice`. `contracts::project_reactive_reads` also stopped
truncating with `path.last()` when reading an accepted contract back in, which
would otherwise have round-tripped a correct path down to its last segment.

Pinned by `fixtures/package-contracts/parameter-member-read-path`, and by unit
tests in `indexes.rs` covering the full-path emission, the single-segment
control, both unnameable-segment truncations, both sides of the 32-segment
boundary, all three compound roots, a call at the root, and the two callee
gates. Reverting the walk to last-segment rooting, dropping the identifier gate,
or turning the depth limit back into a refusal each fails those tests.

Remaining approximations, all deliberate and all fail-closed:

- **Disagreeing accesses publish no path rather than their longest common
  prefix.** `props.of.keys()` and `props.of.get(k)` in the same export agree on
  `["of"]`, and an LCP would be a sound and stronger claim than the empty path
  published today. It is left alone here because
  `@solid-primitives/keyed@1.5.3`'s `MapEntries` read row is the pinned control
  for the unnamed spelling; moving to an LCP is a separate precision slice.
- **Optional links are invisible to the fact model.** `solid-facts` records no
  optionality on `MemberFact`, so `props.of?.values()` is rooted identically to
  `props.of.values()` and publishes `["of", "values"]`. The claim stays sound —
  when the chain short-circuits the access never happens and the demand simply
  goes unwitnessed — but the generator cannot currently distinguish the two. A
  dedicated optionality fact is the follow-up.
- **A multi-segment path is not resolved at consumer call sites.**
  `interproc`'s per-call-site resolution of `parameter.member()` answers for one
  property of the argument. A longer path needs the value at the intermediate
  segment first, so it now records a `parameter-member-path-unresolved` dispatch
  obligation and contributes no read, where it previously resolved the last
  segment against the argument itself — a different property of a different
  value.
- **A truncated access weakens the whole row when it disagrees with a sibling.**
  An export that both reads `options.good.slice(n)` and walks a chain past the
  depth limit publishes two disagreeing paths, so the row names none — the
  export's `["good", "slice"]` claim is lost to the truncation of the other
  access. Keeping the deeper access as a row is still the right trade (dropping
  it would negate it), but a longest-common-prefix rule would recover both; it
  is the same follow-up as the disagreement item above.
- **Producer and consumer peel different TypeScript sugar.**
  `AstFacts::peel_ts_sugar_span` peels parentheses, `as`, `satisfies`, type
  assertions and `!`, so the IR correctly roots `(options as Opts).source.slice(n)`
  and `options!.source.slice(n)` at `options`. The Type Facts producer's
  `parameterValueSourceLocked`
  (`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`)
  unwraps only `ParenthesizedExpression` and answers `nil` for the rest, so
  those *correct* demands can never be witnessed and the row refuses. Pre-existing
  and fail-closed, and it belongs beside the optional-chaining note above: both
  are populations of structurally unwitnessable read demands, not wrong ones.
  Closing it means teaching the producer the same peeling.


## 2026-09-01 — A primitive callback slot no longer roots an ungrounded invoke claim

`primitive_callback_execution` in
`rust/crates/solid-reactive-ir/src/interproc.rs` answers *how* a callback at a
primitive's argument would run relative to the exported call. The contract
inventory in the same module read a row there as permission to publish an
`invoke` operation rooted at whatever exported parameter was forwarded into that
slot. The row says nothing about whether the slot holds a callback, and two
shapes in the measured ecosystem published claims about values the shipped code
never invokes:

- `@solid-primitives/flux-store`'s
  `createFluxStore(initialState, createMethods)` claimed
  `callbacks: [{from: {arg: 0, path: []}}]`, `kind: invoke`, `tracking: tracked`,
  `at: {schedule: queued}`. The body's only use of argument 0 is
  `createStore(initialState)` (`0.1.1 dist/index.js:25-32`,
  `1.0.0-next.2 dist/index.js:24-31`). Solid 1.x has no compute form for
  `createStore` at all; Solid 2.0 has one, but only in the derived
  `createStore(fn, initial, options?)` overload, whose plain twin declares its
  first parameter `NoFn<T> | Store<NoFn<T>>`.
- `@solidjs/meta@0.29.4`'s
  `Stylesheet = props => createComponent(Link, mergeProps({rel}, props))`,
  declared `Component<Omit<JSX.LinkHTMLAttributes<HTMLLinkElement>, "rel">>`,
  claimed the same shape on its props parameter, from a `(MergeProps, _)` row
  that matched every argument index. `@solidjs/router@1.0.0`'s `A` and `Route`
  are the same shape.

The certification census refused all four, correctly and permanently: `invoke`
evidence for a value nothing invokes cannot exist, so no widening of the proof
side could ever have discharged them. These rows leave the refusal set by
**withdrawal**, not by discharge.

`primitive_slot_roots_parameter_invoke` now requires three premises before the
inventory constructs the claim. The value must not be *proven* non-callable
(`Callability::Unknown` is the absence of an answer and withdraws nothing, so
untyped JavaScript distributions are unaffected). A slot whose behavior is
conditional on callability — `mergeProps`, and 2.0's `createSignal`,
`createStore`, `createOptimistic`, `createOptimisticStore`, each of which has a
plain form declared to exclude functions beside a derived form that takes a
compute — additionally requires callability *proven*, because an unproven answer
is the missing premise rather than a licence to assert. And the dialect must own
the slot, which is what separates 1.x's `createStore(store?, options?)` and
`createSignal(value, options?)` from their 2.0 namesakes.

**There is deliberately no arity premise, and an earlier draft's was wrong.**
It dropped the store pair's row at `argument_count < 2`, on the ground that 2.0's
derived form requires the seed store at argument 1. The runtime does not
dispatch on arity: `createStore(first, second, third)` returns the derived store
whenever `typeof first === "function"` (`@solidjs/signals` `dist/dev.js:9371`;
`solid-js@2.0.0-rc.3 dist/server.js:896` routes the same shape through
`createProjection`, and `createOptimisticStore` at `:912` delegates to
`createStore`). The premise's own justification — that `NoFn<T>` makes a
one-argument callable `createStore` a type error, so no `tsc`-clean fixture could
contain it — was also wrong: `NoFn` is the *client* entrypoint's. The published
**server** entrypoint (`types/server/signals.d.ts:136-143`) declares the plain
form `createStore<T extends object>(store: T | Store<T>, options?)` with no
`NoFn`, and a function type satisfies `T extends object`, so
`createStore(compute)` compiles clean there and really is derived. That case is
now pinned end to end by
`fixtures/package-contracts/callback-slot-derived-store-server`
(`projectSeedless`), beside its `plainStore` negative at the same arity. The rule
is callability and only callability.

`createMemo`, `createProjection`, `createTrackedEffect` and `dynamic` take the
compute at argument 0 in every overload, so they keep the weak premise and keep
publishing on untyped artifacts. The row table itself is unchanged: it is also
the reach of the wrapper-chain fold, where a *missing* row makes the chain refuse
and the inner slot's own answer stand — a stronger claim, not a weaker one
(`fixtures/package-contracts/callback-deferred-untracked-chain`'s
`unestablishedScheduleShape` pins that, and deleting the `mergeProps` row
regressed it from open to a false same-stack claim before the premises were moved
to the inventory).

Named negative cases, pinned by the dialect fixture pair
`fixtures/package-contracts/callback-slot-props-forwarding` (1.x) and
`callback-slot-derived-store` (2.0), plus
`callback-slot-derived-store-server` for the server entrypoint: a props object at
either `mergeProps` argument; `createStore(initial)` and
`createStore(initial, options)` under 1.x even with a provably callable argument;
`createSignal(fn)` under 1.x, which stores the function as the signal's value;
`createStore(initial)` and `createStore(initial, {name})` under 2.0, at both the
plain form's arity and the derived form's; `createOptimisticStore(initial)` and
`createOptimistic(initial)` under 2.0; and `createSignal(initial)` under 2.0 with
an object-typed argument. The positives that must survive: a `mergeProps` source
proven callable through a read signature (`WithLazyExtras`) *and* one proven
through the signature-less `Function` supertype
(`WithOpaqueExtras`, `Callability::UntypedCallable`); 2.0's
`createStore(compute, seed)`, `createStore(compute)` on the server typings,
`createOptimisticStore(compute, seed)`, `createSignal(fn)` and
`createOptimistic(fn)`; and `createMemo` in both dialects.

### Remaining approximations

- **`createFluxStore`'s real callbacks are still not inventoried.**
  `createMethods.getters` and `createMethods.actions` are argument 1 behind a
  property path, and the generator's `ContractCallback` carries a parameter index
  with no path (`inferred_contract.rs` hard-codes `path: Vec::new()`). Withdrawing
  the false row leaves the export's callback domain **open**, not closed-empty, so
  nothing false is asserted in the other direction — but the export remains
  uncertifiable rather than correctly described. Re-rooting needs the model to
  grow a path, which is a separate change.
- **An artifact with no types at all keeps nothing at a conditional slot.**
  The conditional premise is a pure function of the compiler's callability
  answer for the argument span, and a JavaScript distribution whose parameters
  carry no inline annotation and no JSDoc answers nothing at all — a sibling
  `index.d.ts` types the *declaration* file, not the runtime artifact's parameter
  spans. Measured, at this branch's base, on a package whose `index.js` is
  compiled from the same source as `callback-slot-derived-store-server`'s
  `index.ts`: both the seedless and the seeded derived `createStore` lose their
  row, and both come back the moment the same `index.js` carries
  `@param {(store: Cart) => void}` (proven `Callable`) or `@param {Function}`
  (proven `UntypedCallable`). So the withdrawal is exactly the
  absent-proof case and is the fail-closed direction — but any package that
  genuinely forwards a callback into `mergeProps` or 2.0's
  `createSignal`/`createStore` family from an untyped distribution silently
  loses a true claim, and nothing in the repository would notice. Closing it
  needs a callability fact the runtime artifact's own shape can supply.
- **The `MergeProps` row is not a dialect inconsistency**, contrary to an
  earlier draft of this entry. `Solid1x::callback_execution_at` answers
  `Some(Execution::Tracked)` for `MergeProps` at every
  `argument < argument_count`, *ahead* of the `callback_executions` lookup,
  because `mergeProps(...sources)` is variadic and the flat table cannot say so;
  `merge_props_function_sources_are_variadic_tracked_computations`
  (`rust/crates/solid-dialect/src/solid_1x.rs`) pins exactly that, and it makes
  the `MergeProps => DuringCall` arm of `Solid1x::tracked_callback_timing`
  reachable rather than dead. Solid 2.0 spells its own primitive
  `Primitive::Merge` and carries no `MergeProps` row, so no 1.x row leaks across
  the dialect seam. Nothing here is open; the note is retained because the
  earlier claim sent the dialect owner after a non-problem.

## 2026-09-01 — A `tracked` callback row takes its schedule from the dialect, not from the word

`inferred_contract.rs`'s `callback_operation` mapped every `execution:
"tracked"` row onto `(Schedule::Queued, Tracking::Tracked)`, hardcoded, with no
consultation of `Dialect::tracked_callback_timing`. `tracked` is an
*attribution* word — it says the runtime subscribes the callback's reads — and it
carries no schedule column at all, deliberately: 1.x `createMemo` and
`createRenderEffect` run their compute during the creating call while 1.x
`createEffect` queues it, and 2.0 disagrees with 1.x on `createEffect`. Reading
`queued` out of the word published a promise the runtime breaks before the
export even returns. `Solid1x::tracked_callback_timing` states
`CreateMemo | CreateRenderEffect | CreateComputed | CreateResource | MergeProps
=> DuringCall`; `Solid2` states
`CreateMemo | CreateSignal | CreateOptimistic | CreateProjection | CreateEffect |
CreateRenderEffect => DuringCall` and `CreateTrackedEffect => AfterCall`.

`ContractCallback` now carries a `schedule: Option<CallbackSchedule>` beside the
word. `interproc.rs`'s `composed_tracked_schedule` derives it from the wrappers
the callback actually sits under: a wrapper the dialect says merely queues wins
outright, a chain whose tracked wrappers all run during their own call is
`same-stack`, and a wrapper the dialect states no timing for leaves the schedule
`Unestablished`. `Unestablished` emits the invoke operation with **no execution
point at all** rather than a guessed one — the attribution claim is proven and
survives on its own. `contracts.rs`'s `project_callbacks` carries an ingested
contract row's own schedule through the same field, so re-emitting a row
republishes what the contract said instead of the default.

Rows this corrected, all measured at this branch's base:

| row | was | is | dialect fact |
| --- | --- | --- | --- |
| `callback-slot-props-forwarding` `WithLazyExtras` / `WithOpaqueExtras` (1.x `mergeProps`) | `queued` | `same-stack` | `MergeProps => DuringCall` |
| `callback-slot-props-forwarding` `derive` (1.x `createMemo`) | `queued` | `same-stack` | `CreateMemo => DuringCall` |
| `callback-slot-derived-store` `derive` (2.0 `createMemo`) | `queued` | `same-stack` | `CreateMemo => DuringCall` |
| `callback-slot-derived-store` `makeDerivedSignal` / `makeDerivedOptimistic` (2.0) | `queued` | `same-stack` | `CreateSignal`/`CreateOptimistic` `=> DuringCall` |
| `callback-slot-derived-store` `projectStore` / `projectOptimisticStore` (2.0 store pair) | `queued` | *no execution point* | 2.0 states none for either store primitive |
| `@solidjs/router@1.0.0` `createAsync` / `createAsyncStore` (ecosystem, not a repository snapshot) | `queued` | `same-stack` | the chain's tracked wrappers are all `DuringCall` |

### Remaining approximations

- **Two other producers of the word still leave the schedule unstated, and the
  consumer's `queued` default stands for them.** Both are pre-existing and
  neither is an inventory claim:
  - the *direct-invocation* rung of `interprocedural_contributions` (the
    `chain_execution` fallback around `interproc.rs:1211`), which is why
    `fixtures/package-contracts/callback-deferred-untracked-chain`'s
    `memoInsideUntrack` — `untrack(() => createMemo(() => handle()))`, a chain
    whose only tracked wrapper is 1.x `createMemo`'s `DuringCall` — still
    publishes `queued` where `same-stack` is the true answer. Its sibling
    `trackedShape` (`createEffect`) publishes `queued` correctly, so the fixture
    does not distinguish the two today;
  - `contract_callback_execution`'s `ExecutionRole::TrackedJsx` arm
    (`lib.rs`), a compiler-lowering role with no wrapper chain to compose.
- **The `Unestablished` schedule is not yet represented in the certification
  census.** An invoke operation with no execution point states less than one
  with a schedule, which is the honest reading, but no proof family currently
  demands or discharges the missing column.

## Six ways the demand-honesty round proved less than it claimed (2026-09-01)

An adversarial review of the round below found two facts that were *false*, one
witness with no premise, one guard the verifier trusted instead of checking, one
premise discharged by an unrelated fact, and one false positive. Every fix here
makes a fact stricter; none teaches anything to agree.

### Reachability after a construct ignored constructs nested inside it

`constructCompletesNormally`
(`apps/solid-typefacts/internal/typefacts/tsgo/invocation_transcripts.go`) read
the loop-exit-edge question of the construct it was classifying and of no other
node. A `while (true)` inside a `try`, a `for (;;)` inside a `for … of`, an
endless loop in a `switch` clause: control entering any of them never reaches
the bottom of the construct wrapping it, and the statement after that construct
was nonetheless reported `reachable`. Reachability is a *proof of execution*
downstream — `implementation_call_is_executed` gates callback-execution evidence
on it — so this was a false certification, not a precision loss.

The scan now asks a separate question of every nested loop / `try` / `switch`:
`constructTraps`, "can control leave this at all". A construct control can never
leave falsifies its container. A jump *out* of the nested construct is
deliberately not a trap — the enclosing scan already classifies that jump, which
is what keeps `outer: for (…) { while (true) { break outer } }` falling through.
Recursion is memoized and depth-capped, and the cap answers "trapped", the
conservative direction. Pinned by five shapes in
`TestFallThroughConstructsKeepFollowingStatementsReachable`.

### `while (1)` and `while (!0)` were credited with an exit edge

`isTrueKeyword` matched `KindTrueKeyword` and nothing else, so every other
spelling of an endless loop kept its fall-through edge and produced the same
false `reachable`. `alwaysTruthyLiteralCondition` now decides literal
conditions: `true`, a non-zero numeric literal, a non-empty string literal, and
`!` applied to an always-falsy literal. It deliberately does **not** consult the
type checker's truthiness of an arbitrary expression — `while (keys)` keeps its
exit edge, which is the Unknown-preserving direction. The doc comment claimed to
answer `false` while answering `true`; it now states the pinned behaviour,
including that a label wrapping a construct *is* the construct.

### A returned closure's *mentions* discharged a call it never contains

`ReturnSite.captures` was the union of parameter indices the returned callables
mention. The consumer used it as the premise "this callback may be invoked by
the returned value" for any call marked `captured` — a flag that means only
"inside *some* nested callable". Nothing linked the call's enclosing callable to
any returned one, so a `callback()` in a closure the implementation never
returns was discharged by a returned closure that merely *names* `callback`.

The wire fact is now `carriedCallables`: the exact source ranges of the callables
each return site provably carries, and the consumer joins by containment. A call
inside a carried range is reached by invoking the returned value; a call outside
every range is not, whatever anything mentions. Containment is transitive on
purpose, because a returned debounced function schedules its callback from an
arrow it hands to `setTimeout`. Pinned by
`TestCarriedCallablesBindContainmentNotMention` (producer) and
`implementation_flow_requires_direct_or_returned_closure_execution` (consumer).
Protocol 7's schema digest moves with it.

### A reassigned function declaration still resolved to its own body

`collectReturnedCallablesThroughBindingLocked` accepted any callable declaration
the symbol owned. A hoisted `function fn() { callback() }` binds a *mutable*
variable, so `fn = () => {}; return fn` returned a closure the descent still
described as calling `callback`. The descent now refuses when the declaring file
writes to the binding anywhere, decided by the compiler's own
`ast.GetAssignmentTarget` so a shadowing inner `fn` is not mistaken for a write
to this one. Scanning the declaring file alone is exact for what matters: an
imported binding is read-only and TypeScript refuses an assignment to one.

### A root-path demand with no callability assertion discharged on nothing

The tri-state's `Unknown` is the absence of a callability premise. At a
*non-empty* path the sibling premises carry the demand — the path must be in the
producer census, complete, present. At the **empty** path there is no census
entry either, so `(root, Unknown)` reached `require_root_callability`'s
unconditional first arm and recorded the positive fact as proved by nothing.

The first repair was to refuse that combination outright, and it was wrong in
the other direction: D1 makes *every* implementation-derived root shape
(`Reactive`, `Tuple`, `Object`, …) demand `Unknown`, so an unconditional refusal
makes those facts permanently unprovable. Measured on the ecosystem corpus that
cost 25 certified rows — among them `@solid-primitives/scheduled`
`createScheduled`, which clears every one of its callable-path demands and then
died on the root shape.

`require_verifiable_root_premise`
(`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`)
now supplies the missing premise instead of refusing: at `(root, Unknown)` the
producer's *observation* of that value must be closed — no open reason in the
value or any alternative, every finite partition complete, the primitive domain
not the explicit `unknown` marker, and a callability actually answered. A
producer that says "I did not finish looking at this value" still proves
nothing and the fact stays open.

That is sound rather than a weakening. The fact discharged is the IR's shape
claim, and the demand asserts nothing about callability, so nothing is asserted
onto the declaration — the keyed class of contradiction (an
implementation-derived shape demanding non-callability of a declaration that
says otherwise) cannot recur, because there is no assertion to disagree with.
What the closed answer establishes is the one thing the root needs: that the
producer exhaustively observed the value the fact is about. Non-root Unknown
paths are untouched throughout.

`require_recursive_subject` (the selected-call path) has the same shape and is
deliberately not changed here: it verifies the operation binding and the formal
parameter's existence before reaching the root arm, so it is not premise-free.
It is the narrower version of the same question and is still worth closing.

### The overload set's completeness was trusted, not verified

`require_export_call_signatures` documented that "the producer reports an
overload set all-or-nothing" and checked nothing. It now refuses a transcript
that populates both `callSignature` and `callSignatures`, and requires the set's
own ordinals to be exactly `0..len-1` with every signature agreeing that
`overloadCount == len`. That is exact for the ambient overload sets package
typings actually contain; an overloaded function *with an implementation body*
reports `overloadCount == len + 1` and is refused as "not the complete declared
set", which fails closed. The producer's own gate is now one testable function
(`completeOverloadSet`) rather than loop control flow, so a `break` that becomes
a `continue` can no longer widen the answer.

### Remaining approximations from this slice

- **The `ObjectConstructor` container arm of `isDefaultLibrarySymbolLocked` has
  no reachable negative case.** Given the receiver check that precedes the only
  call passing a container, `Object.assign` cannot resolve to a default-library
  `assign` declared anywhere but `ObjectConstructor`, so no TypeScript source
  distinguishes the arm from its removal. The "every declaration is a default
  library" quantifier beside it *is* falsifiable and is pinned by a
  `declare global { interface ObjectConstructor { assign… } }` augmentation.
- **A call inside a never-invoked closure nested within a carried callable still
  discharges.** Containment answers "reached by invoking the returned value" for
  the callable itself and everything lexically inside it; an orphan arrow
  declared but never called inside a returned closure is inside that range. This
  is strictly narrower than the union it replaces and matches the premise the
  round set out to bind, but it is not the exact "is invoked" relation.
- **A construct is refused whenever a nested construct traps, without asking
  whether that nested construct is on every path.** `switch (x) { case 0: while
  (true) {} default: break }` does fall through on the `default` arm and is
  answered `unknown`. Over-reporting a trap costs precision only.

## Three certification demands stopped asserting things the IR cannot know (2026-09-01)

A demand is a claim the analyzer asks the compiler to confirm. Thirty-two rows
of the "Type Facts demand is locally open" refusal set were not missing producer
facts: they were demands asserting something false, which no honest producer can
ever confirm. All three fixes are on the demand side. Nothing was taught to
agree.

### A boolean callability forced every unclassified shape to claim non-callability

`recursive_value_is_callable` in
`rust/crates/solid-reactive-ir/src/contract_semantics/certification.rs` was
`matches!(shape, Callable | Reactive)`. Because the demand field was a `bool`,
every other `ValueShape` — `Parameter`, `Tuple`, `Object`, `Array`, `Store`,
`Cleanup` — became the assertion `callable: false`, i.e. *this runtime value is
provably non-callable*. The IR had made no such claim; the shape simply is not
the `Callable` constructor.

Measured contradictions:

- `@solid-primitives/utils` `accessArray` demanded non-callability of
  `Parameter { 0, path: ["map"] }`. That is `Array.prototype.map`. The compiler
  answers `callable`, correctly, and the row refused with "operation value path
  has the wrong callability".
- `@solid-primitives/gestures` `getCenterOfTwoPoints`, the same shape on
  `["getBoundingClientRect"]`.
- `@solid-primitives/spring` `createDerivedSpring` demanded non-callability of a
  return root the package's own `.d.ts` declares `Accessor<T>`.
- `@solid-primitives/keyed` `Entries` is the mirror. The IR modelled its return
  as `Reactive { accessor }` from the *implementation*'s `createMemo(...)`; the
  *declaration* says `JSX.Element`, which is non-callable. Declaration and
  implementation legitimately disagree, so a demand derived from one and
  verified against the other is unprovable by construction.

`DemandedCallability` (`Unknown` / `Callable` / `NonCallable`) replaces the
boolean. Only three constructors assert, and they are exactly the three grounded
in a *type-kind* observation rather than in implementation analysis:
`Callable` and `Component` (a closed `ExportKindProof` call or construct
signature — see `reconcile_entry_export_kind`, which refuses the export outright
when no closed type answers) and `Plain` (that proof's closed negative). Every
other constructor reaches the model through `return_shape`/`reads` in
`rust/crates/solid-facts-backend/src/inferred_contract.rs`, from the
implementation, and now asserts `Unknown`.

`Unknown` is the *absence* of a premise, not a weaker one. Every other part of
the same demand still verifies: the value path must still exist in the producer
census, still be complete, closed and present, and the operation's flow evidence
is unchanged. It also withdraws the `CallablePath` family for that subject,
because a demand that does not claim callability has no callable path to prove.
The consumers are `require_root_callability`, the new
`require_path_callability`, the two branch gates in
`require_operation_recursive_subject`, `require_export_recursive_subject` and
`require_recursive_subject`, all in
`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`.

Pinned by `only_type_kind_shapes_assert_a_demanded_callability` and
`an_unasserted_callability_demands_no_callable_path_family` (certification.rs)
and `an_unasserted_callability_verifies_the_path_without_a_callability_premise`
(type_facts.rs), which asserts both halves: the previously-contradictory demand
now verifies, and an absent or locally open path still refuses.

### Demanding "the" signature of an overload set asked for a nonexistent object

`require_export_call_signature` refused with "exported callable has no unique
compiler signature" whenever `GetSignaturesOfType` returned more than one.
`@solid-primitives/cookies` `createServerCookie`, `@corvu/utils`'s default and
`@solidjs/router`'s `action` are genuine two-overload exports; there is no
single signature to name, and picking one would answer a different question than
the one asked.

The generalization is universal, not existential: **a premise that holds for
every overload holds for the export**, whichever one a caller selects. So the
producer now reports `callSignatures`, the complete set, when the type has more
than one (`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`),
and `require_export_call_signatures` returns all of them for the caller to
require of each. The set is all-or-nothing at the producer: a signature whose
current declaration cannot be selected empties the whole field, so "every
overload" can never silently narrow to "every overload we could describe".

This is a wire break — a protocol-6 client rejects unknown transcript fields —
so `TypeFactsHandshakeProtocol` is 7 and the frozen schema digest moves with it.

### A destructuring pattern was resolved as an alias of the value it destructures

`direct_value_symbols` in `rust/crates/solid-reactive-ir/src/indexes.rs` ran the
`initializer_identifier` rung — the one that makes `const alias = original`
carry `original`'s identity — before looking at the binding's shape. For
`const { href } = props`, that answered **`props`** for a call written
`href(...)`. The callback derivation then found `props` in the parameter list and
published a `callbacks` row saying parameter 0 is invoked: the props object
itself, called as a function. `@solidjs/router`'s `Navigate` published exactly
that, and certification refused it correctly — the call is on a member, and no
compiler fact can confirm a props object is invoked.

The rung is now gated on `BindingShape::Identifier`. The object-slot branch below
it still resolves a destructured property on its own evidence; a slot with no
inspectable value keeps the binding's own symbol, so the false row is not
published at all. Pinned by the `destructured-parameter-callback` generator
fixture (`Parameter` positive, `ObjectPattern`/`ArrayPattern` silent,
`MemberAlias` control).

### Remaining approximations from this slice

- **A member-bound callback cannot be expressed.** The honest positive claim for
  `const { onData } = props; onData(1)` is a callback at `arg: 0, path:
  ["onData"]`. `ContractCallback` (`rust/crates/solid-reactive-ir/src/lib.rs`)
  carries only a parameter index, so the semantic model has no member path for a
  callback binding. The generator publishes silence — the `callbacks` domain
  stays open and a consumer fails closed — rather than a false row. Extending
  `ContractCallback` with a member path, and populating it from the object-slot
  evidence, would recover the claim.
- **`@solidjs/meta` `Stylesheet` is a different defect, not this one.** It still
  refuses with "callback parameter has neither an exact direct call nor an exact
  dialect callback flow" for the same `arg: 0, path: []` shape, but its source is
  `props => <Link rel="stylesheet" {...props}/>` — no destructuring, and nothing
  invokes `props`. The claim reaches `Stylesheet` through a JSX spread into
  `Link`, whose own behaviour is `MetaTag`'s `get name()` getter read inside a
  `createRenderEffect`. Reading a getter is not invoking the object. Not
  diagnosed further here.
- **`@solid-primitives/spring` `createDerivedSpring` is now honestly open one
  level down.** With the root's false non-callability withdrawn, the demand for
  the tuple item the IR claims (`[Tuple(0)]`) refuses because the declared result
  `Accessor<WidenSpringTarget<T>>` has no tuple index. The IR modelled
  `return springValue` (from `const [springValue] = createSpring(...)`) as the
  whole `createSpring` tuple. That is a shape defect, not a callability one, and
  it is unfixed.
- **`namePlugin`, `FaviconLink`, `SignalContextProvider`** still refuse on export
  *root* callability. Those demands are the grounded ones (`Plain`, `Component`),
  so the tri-state deliberately leaves them asserting; the divergence is between
  the closed `ExportKindProof` the proposal was built from and the export-value
  transcript's own `callability`, which is a producer/adapter question.

## The certify child's TypeScript-program retention, and the width it was costing (2026-09-01)

Not a precision movement: **no verdict, claim, digest, receipt, or report
semantic changes**, and this is recorded here only because it moved the resource
policy the previous entry installed. Six probes were measured end to end before
and after; every field of every probe result is equivalent (see below).

**What was retained.** `moduleDescription` in
`packages/cli/scripts/artifact-resolution.mjs` cached each module's description
with the `SourceFile` *and the `TypeChecker`* that produced it. The checker
retains its whole `ts.Program`, and `parseModule` builds a fresh standalone
program for every module needing symbol identity, so one
`ArtifactResolutionSession`'s description cache held one live program per such
module for the session's lifetime. A published dependency graph's preparation
walks dozens of packages under one session.

Phase-delta instrumentation on `corvu@0.7.2` (42 graph nodes), sampling
`process.memoryUsage()` at every stage boundary:

| phase | RSS | heapUsed |
| --- | --- | --- |
| artifact acquisition ends | 86 MB | 8 MB |
| **graph preparation ends** | **3179 MB** | **1997 MB** |
| graph acquisition ends | 3188 MB | 2192 MB |
| 42 in-process proposal generations end | 3446 MB | 2722 MB |
| witness acquisition ends | 3450 MB | 27 MB |

The entire cost is one phase, and it tracks the program count exactly: RSS rose
with `parseModule` calls at roughly 70 MB each. The proposal generations that
were the intuitive suspect add 258 MB across all 42. The heap collapses to
27 MB once preparation's caches are dropped, which is what says *retention*
rather than working set.

**Two repairs, both in `artifact-resolution.mjs`:**

1. `moduleDescription` extracts the syntax hazard census while the program is
   live and retains only that plain-data result; `closureForRoots` reads the
   cached rows (copying each, because `canonicalClosure` sorts in place) instead
   of recomputing from a retained AST. The census is a pure function of
   (relative path, file text), and the relative path is determined by the cache
   key, which already keys the specifier targets computed from the same package
   root. Alone this took `corvu` from 3622 MB to 1055 MB.
2. `PROGRAM_OPTIONS` gains `noLib`. These programs answer one question, in
   `syntaxHazards` and nowhere else: does this identifier's symbol have a
   declaration in *this same source file*? With `noResolve` already excluding
   every imported module, the default library is the only other declaration
   source a program has -- and a `lib.*.d.ts` declaration is never a declaration
   in an analyzed package file, so "resolves elsewhere" and "resolves to
   nothing" both read as *not locally bound*. Checked as well as argued: across
   6740 real installed package files (2507 needing a checker) the census is
   byte-identical with and without the library set. The library set is 86 source
   files re-parsed and re-bound per program, measured at ~63 MB resident each.

**Measured, per probe, whole process tree, `ps` sampled at 2 Hz.** The
watchdog was raised to 32 GiB for the measurement so the pre-repair peaks could
be observed rather than truncated at the kill.

| probe | before | after | verdict before/after |
| --- | --- | --- | --- |
| `@solidjs/start@2.0.3` | 30539 MB | 731 MB | certified / certified |
| `@solid-devtools/transform@0.10.4` | 25126 MB | 762 MB | certified / certified |
| `@kobalte/core@2.0.0-alpha.0` | 10385 MB | 644 MB | refused / refused |
| `@tanstack/solid-db@0.2.40` | 5994 MB | 589 MB | refused / refused |
| `corvu@0.7.2` | 3754 MB | 705 MB | certified / certified |
| `@solidjs/web@2.0.0-rc.3` | 3666 MB | 496 MB | refused / refused |

Five of these six are among the 38 probes the previous entry's 4 GiB ceiling
killed outright; all six now complete under the unchanged ceiling and produce a
real verdict.

**Equivalence.** Each probe's whole result document was diffed field by field,
ignoring only wall-clock timings, absolute temporary paths, and the identity
digests that hash those paths -- the dependency plan's node/leaf/cycle ids, its
`graphDigest`, and the published-graph digest quoted in a refusal reason. Those
are run-specific by construction: the harness installs each probe into a fresh
`mkdtemp` directory whose absolute path is hashed into `identity.runtime.path`.
The ignore set was validated by running the *same* code twice and confirming the
two runs differ in exactly those places and nowhere else. Under it, all six
probes are equivalent, contract content digests included.

**Width restored.** `CERTIFICATION_MEMORY_SHARE_BYTES` in
`scripts/ecosystem-benchmark/run.mjs` moves from 8 GiB to 2 GiB -- ~2.7x the
measured worst peak of 762 MB -- so a 48 GB host runs the full cores-bounded
width of 14 again instead of 6, while a 4 GB machine still floors at two slots.
The 4 GiB probe ceiling is deliberately **unchanged**: at ~5x the measured worst
peak it is a guard against pathology, and lowering it toward the measured peak
would start deciding rows. The certification row now carries a `memoryExceeded`
flag so a watchdog kill is machine-queryable rather than only prose in `reason`.

**What remains, honestly.** The peaks above are still hundreds of megabytes, and
that is mostly JavaScriptCore's allocator high-water rather than live data:
forcing a full GC at the phase boundary drops `corvu`'s live heap to 26 MB while
RSS stays at 490 MB. Reducing it further means creating fewer transient
programs, not releasing more references. Also unchanged, and still open from the
previous entry: `acquirePublishedArtifact` buffers each archive and packument
whole in memory with no per-dependency cap.

## Certification witness programs now carry their authenticated dependency typings (2026-08-31)

Ordinary root certification built its witness program from the target package's
snapshot alone. Every cross-package type reference in that package's own
declarations -- `Accessor` and `JSX` from `solid-js`, `Component`, `Context`, a
cross-package base class -- therefore resolved to `any`, and the producer's
`callabilityOfType` correctly fail-closed to Unknown on `any`. Contract
*generation* resolves those same names against the installed tree and records
definite facts, so the two halves disagreed and 75 ecosystem rows refused
`export root is not compiler-proved callable or constructable` at live
export-value verification.

The fix supplies evidence; it relaxes no determination. `callabilityOfType` and
`require_root_callability` are untouched. Ordinary root certification now names
its declaration-only closure through the *same* authenticated channel a
published graph node already uses -- integrity-verified published archives
replayed against exact Bun lock selections -- and Rust materializes those
snapshots into the private project so `moduleResolution: "bundler"` resolves
them. No installed `node_modules` byte is ever read as evidence, and the tsconfig
the witness program uses is unchanged.

### Drops are name-scoped, because a per-copy drop is substitution

The first version of this change dropped individual copies and claimed that
removing evidence is always the fail-closed direction. **That claim is false**,
and adversarial review caught it. `moduleResolution: "bundler"` walks up
`node_modules`, so withholding a *nested* copy while a *hoisted* copy of the same
name survives does not make the module unresolvable -- it hands the lookup to a
different version, whose bytes the source census accepts because they are
authentic under their own marker. The determination then comes from
authentic-but-wrong-version declarations. Measured against the published
typings, with `foo`'s `.d.ts` importing `Callback` from `bar`:

| private project | `IsCallable<typeof value>` | verdict |
| --- | --- | --- |
| nested `bar@2` (non-callable) shadows hoisted `bar@1` -- the install truth | `false` | non-callable |
| nested copy dropped, hoisted `bar@1` kept | **`true`** | **flipped** |
| whole `bar` name withheld | `TS2307 Cannot find module 'bar'` | `any`, demand open |

So both halves withhold whole package names, all-or-nothing:

- The CLI records every package name it could not name or acquire and excludes
  every copy of that name, globally across certification inputs (the private
  project is materialized once from their union). An unnameable grandchild
  poisons its own name only; the rest of the collected subtree still travels.
- Rust's `retain_authenticated_source_packages` partitions by package name and
  withholds the whole group if any member fails authentication, poisoning both
  the name the lock selection claims and the name the installed root occupies.
- A name whose authenticated copies cannot occupy distinct places in the private
  project is withheld too, rather than colliding on `write_immutable_project_file`
  -- which would be an `AlreadyExists` source-census failure, a hard failure class
  this path must not have.

Only then is a drop really "the module cannot be resolved". Published-graph
nodes keep the old fatal behavior throughout, because a node's canonical
identity binds its `source_dependencies_root`.

Everything else about an absence stays a non-event: a missing lockfile, a copy
the lock does not select exactly, an uninstalled package, unresolvable
declarations, an oversized registry document, an unreachable archive -- each
withholds a name and leaves the demand open, and none of them aborts a
certification. The source census is unchanged, so a producer-consulted file
outside an authenticated snapshot is still refused outright.

### The closure a receipt was proved against is now named by the receipt

The verdict depends on which declaration-only closure was materialized, so a
receipt has to say which one that was; otherwise an auditor cannot tell a
full-closure certification from a partial-closure one. `certification_sources_root`
composes the sorted canonical source identities -- mirroring the graph's
`source_dependencies_root` -- and every Type Facts witness binding folds it in.
An empty closure has its own well-defined root, so "nothing was supplied" is a
statement the receipt makes rather than an absence, and a withheld source is
indistinguishable from one never supplied.

Census site paths are now **project-relative** before hashing, for all three
site kinds (`typefacts-verifier-source`, `typefacts-source`,
`typefacts-source-snapshot`). They previously carried the producer-reported
absolute path inside a temporary directory keyed on pid and a counter, which
made every evidence root -- and every receipt witness root -- unique per run and
incomparable across certifications of the same bytes. The project-relative path
plus the content digest is what the site actually asserts. This is a deliberate
receipt movement: it affects the existing package-own and verifier-source sites,
not only the ones this change added.

Remaining fail-closed cases this does **not** close:

- **The TypeScript DOM library is not a package.** Solid 1.x types
  `JSX.Element` as `Node | ...`, and `Node` is a `lib.dom.d.ts` type. The witness
  program's tsconfig is deliberately unchanged, so `Node` remains an error type
  and any Solid 1.x export whose proof needs `JSX.Element` still fails closed --
  `@solid-primitives/keyed`'s `Entries` is the observed case.
- Packages whose type-providing dependency is outside the authenticated set for
  any of the reasons listed above.

Named follow-up, not fixed here: acquisition buffers each archive and packument
whole in memory, with no per-dependency cap. That is the pre-existing
`acquirePublishedArtifact` pattern, but the root path now runs it once per
declaration-only dependency instead of once per row, so the peak is multiplied
by closure size.

Pinned by, in `rust/crates/solid-facts-backend/src/contract_certification.rs`
unless noted:

- `root_certification_proves_a_cross_package_type_only_from_authenticated_sources`
  -- the same plan, claim, and demand graph refuses with no sources and
  certifies with the authenticated snapshot.
- `root_certification_withholds_every_copy_of_a_name_one_copy_could_not_authenticate`
  -- the substitution shape above. Deleting the name-scoping makes it certify
  from the surviving hoisted copy.
- `root_certification_drops_a_source_whose_lock_selection_claims_other_bytes`
  -- deleting the source authentication makes it certify.
- `root_certification_drops_a_source_installed_outside_an_exact_node_modules_coordinate`
  and `root_certification_withholds_a_name_whose_copies_collide_in_the_private_project`.
- `certification_sources_root_names_the_exact_closure_a_receipt_was_proved_against`
  -- a full closure differs from an empty one, a withheld source equals an
  absent one, and neither moves the demand graph.
- `graph_nodes_still_refuse_a_source_they_cannot_authenticate` -- the graph path
  keeps refusing on bad root, lock disagreement, and duplicate.
- `certification_source_request_tests` in `main.rs` -- a graph node refuses a
  planning that carries its own nested source set.
- In `packages/cli/test/contract-workflow.test.mjs`:
  `a package name one entrypoint could not authenticate is withheld from every
  entrypoint` pins the CLI half of the same rule, including that one unnameable
  grandchild does not discard its whole collected subtree.

This adds no finding and reports nothing, so it cannot duplicate a `tsc`
diagnostic; it only lets the checker see the types `tsc` would have seen.

## Two authenticated-demand regressions from the non-export proof policy (2026-08-31)

Landing "Authenticate non-export Type Facts demands" (`0fcaf82e`) introduced two
certification-time regressions that no `make verify` gate exercised (the
ecosystem certification runs only in the lead-owned benchmark). Both are now
fixed and pinned by regression tests that run inside `make verify`.

- **Value-only case sets refused as "multiple installation identities."**
  `export_implementation_location`
  (`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`)
  filtered every plan in the batch/graph by the runtime binding's
  `snapshot_root` and refused the moment more than one matched. But a value-only
  case set carries one plan per alternative artifact case of a single package,
  and the batch identity check requires all of them to share one `snapshot_root`
  -- so any package with two or more alternative cases always refused. Because
  `snapshot_root` is a content hash, every matching plan materializes
  byte-identical sources; the resolver now binds the first materialized owner
  (still failing closed when none is materialized). This restored certification
  for the corvu / corvu-next / `@solid-devtools/logger` clusters and every
  multi-case `@solid-primitives` package. Pinned by
  `implementation_location_binds_first_owner_for_shared_snapshot_root`.

- **Producer nil dereference in the returned-closure census.** The producer's
  new returned-closure resolution
  (`apps/solid-typefacts/internal/typefacts/tsgo/invocation_transcripts.go`)
  left the resolved closure node nil when a returned identifier's symbol had no
  value declaration and not exactly one declaration (e.g. a namespace import),
  then called `isCallableDeclaration(nil)` -> `ast.IsArrowFunction(nil)`, which
  panicked and took down the whole certification session (crashing
  `@solid-primitives/media`, `page-visibility`, and any package with that
  returned shape). A nil closure is a missing-evidence frontier: it now yields
  no proven captures (a fail-closed open premise) instead of crashing. Pinned by
  `TestExportImplementationTranscriptKeepsReturnedNilClosureOpen`.

Neither fix loosens subject binding or produces a finding, so neither duplicates
a `tsc` diagnostic: the first only removes a spurious multiplicity refusal
between byte-identical owners, and the second only replaces a crash with the
existing open-premise path.

## Symbol-less callback obligations no longer contaminate value siblings (2026-08-31)

"Normalize callback obligations by exact symbol" (`696834fc`) made
`contract_generation_obligation_target_names` select an obligation's target by
its exact owning-function symbol, but kept a `function_identity` fallback for
obligations whose owner is anonymous and so carries an empty symbol. When a
package republishes bindings through `export *`, one runtime identity is shared
across every republished binding, so a symbol-less callback obligation raised by
an anonymous forwarder in the same module could match a value sibling by
identity alone and open a callbacks domain the operation-graph invariant then
refuses.

The fallback now excludes value-kind exports. A callback obligation proves a
callback is forwarded, so its invocation subject is callable by construction;
without symbol provenance an identity-only match is not proof that any value
sibling is that subject, so it must never reach one. The exact-symbol path is
untouched, so the geolocation true positive (obligation symbol equal to its own
value-export symbol) stays marked and refused. This closes a latent
contamination path, pinned by a unit regression test (the shared identity only
arises from a real cross-package `export *` composition) and by the
`reexport-value-sibling-callback` fixture for the companion exact-symbol path.

## A non-callable value export's open call-path no longer manufactures function effects on composition (2026-08-31)

The five `@tanstack/solid-*` wrapper rows (`.:IR`, `.:initialServerFormState`,
`.:ALL_KEYS`, `.:dataTagErrorSymbol`, `.:PERSISTER_KEY_PREFIX`) refused with
`normalized operation graph is invalid: package contract value export … cannot
have function effects` when the wrapper's contract was generated with its
dependency composed. An earlier note hypothesised this was a symbol-resolution
defect in `contract_generation_obligation_target_names` ("mechanism (b)": the
callback obligation's `function_symbol` resolving to the value export through
re-export aliasing). Instrumenting the real composed refusal for
`@tanstack/solid-db@0.2.40 -> .:IR` disproved that: **no** callback obligation
reaches the value export through that function. The refusing `IR` summary is
`kind: "value"` with `open_claims: {Callbacks, Reads, Creates, Returns}` and
every corresponding claim `Open`, and it is built by
`project_accepted_export` (`rust/crates/solid-reactive-ir/src/contracts.rs`),
not by obligation attribution.

The mechanism is composition, not attribution. `IR` is
`import * as ir from "./query/ir.js"; export { ir as IR }` — a non-callable
namespace value. Its call-path effects are vacuous (a value is never invoked),
and standalone generation of `@tanstack/db@0.8.5` correctly certifies `IR` as a
plain value with an empty summary. But a *proposal* dependency keeps that
export's call-path domains **open** (unresolved — forwarders inside `ir.js`
reachable only via the namespace). When the wrapper's generation composes that
proposal, `project_accepted_export` opened the callbacks/reads/returns/creates
domains from the export's non-closed knowledge *regardless of callability*,
manufacturing function effects on a value export — the exact inconsistency the
operation-graph invariant then refuses.

`project_accepted_export` now closes those vacuous domains for a non-callable
(`kind == "value"`) export: an *open* call-path domain projects as
closed-empty instead of open, matching standalone generation. Only open domains
are closed; a genuinely *known* effect is left intact, so a real
value-with-effects defect still refuses, and certified value exports (already
closed-empty) are unchanged. The geolocation true positive is untouched: it is
a generation-time `reconcile_entry_export_kind` conflict on a genuinely callable
runtime whose declaration is a value, in a different code path, still pinned by
`an_exact_invocation_symbol_preserves_a_real_export_kind_conflict`.

The closing is gated by `shape_may_be_callable` (added after adversarial review
caught a false certification): only a *proven* non-callable shape has its open
domains closed. A `ValueShape::Unknown` (an `any`/error shape, reachable through
the untrusted wire-decode seam) or a `Choice` union whose membership is not
exhaustively non-callable stays open and continues to fail closed — closing them
would assert "invokes no callback" about something that may be callable, from
missing knowledge. Pinned by `shape_may_be_callable_keeps_unproven_callability_open`.

Residual, measured: clearing this false refusal moves all five wrapper rows off
the function-effects error, but four of them then refuse at the deeper
`export root is not compiler-proved callable or constructable` Type Facts stage
(the largest remaining ecosystem refusal class), so the net certified movement is
small. That callability class is a separate, open producer-evidence question.

Pinned by
`contract_document::tests::composing_a_value_export_with_open_call_path_never_manufactures_function_effects`,
which drives the `MINIMAL` golden's open-call-path value export through
`project_untrusted_proposal_for_generation` -> `AcceptedContractIndex` ->
`project_accepted_export` and reproduces the exact `{Callbacks, Reads, Creates,
Returns}` shape before the fix. It cannot be a self-contained package-contract
corpus fixture: the corpus runs single-package `generate` with no composed
dependency, and the open call-path on a value export only arises from composing
a proposal dependency.

## Dependency graph edges match a re-export from any module of the parent package (2026-08-31)

The native published-graph matcher assumed a package's external re-export lived
in its *entry* module: both `graph_request_edges` and the authoritative
planned-node matcher required the dependency edge's importer to equal the
parent's entry runtime path. But Node resolution is per-importing-module, and the
JS node builder correctly records the importer as the *source module* that issued
the import. They agree only when the re-export sits in the entry module, so
`@solid-primitives/form@1.0.0-next.2` (re-exports `@solid-primitives/a11y` from
`dist/form-control.js`) and `@tanstack/ai-solid@0.19.1` (via `@tanstack/ai`'s
`dist/esm/types.js`) refused with `has no exact dependency node`.

The request-ordering matcher now accepts an importer under the parent's
`package_root` (component-wise containment; a name-prefix sibling like `foo-bar`
vs `foo` does not match). The authoritative matcher is *tightened*, not relaxed:
it admits the importer only if it equals `<package_root>/<entry.path>` for a
Runtime- or Declaration-role entry of the parent's **verified** (replayed,
digest-pinned) closure, so every admitted importer is a proven byte-identity
module of the closure. The identity/artifact-case/semantic-digest checks and the
ambiguity/reachability censuses remain the fail-closed backstop, and the
transplanted-leaf regression (importer outside the package root) stays rejected.
Pinned by `native_published_graph_matches_a_non_entry_module_reexport`.

The constants are plain values (`Symbol()`, `Set`, string, object, namespace
object) `tsc` says nothing about, so certifying them duplicates no diagnostic.
Verified offline for `.:IR` (@tanstack/solid-db) and `.:ALL_KEYS`
(@tanstack/solid-hotkeys): both now pass the demand-planning stage where the
function-effects refusal lived. **Remaining, separate from this fix:** past that
stage the witness-acquisition / live Type Facts export-value verification raises
its own refusals (for `IR`, the resolved value declaration `query/ir` is not the
snapshot-selected `index.d.ts` suffix; for `@tanstack/hotkeys`'s
`createSequenceMatcher`, "operation value path has the wrong callability"). Those
are a distinct certification stage and are not addressed here; whether each of
the five rows fully certifies is the authoritative ecosystem re-measure's call.

## Two refusal classes became a recorded inapplicable disposition (2026-08-31)

The ecosystem benchmark counted every omitted artifact case as a refusal. Two
classes inside that count assert nothing about certifiable behavior, and
carrying them as refusals made a row look unproven where nothing was ever
provable.

An artifact case is now recorded as **inapplicable** -- with a class and a
reason, never certified, never counted as a refusal, never suppressing a sibling
case or the proposal -- in exactly two situations, decided from the export-map
selection alone before any analysis (`artifactCaseDisposition` in
`packages/cli/scripts/generate-package-contract.mjs`):

- **`unpublished-conditional-target`** -- the runtime target is absent from the
  artifact *and* the selection traversed at least one **private namespaced**
  export condition (a name containing `/` or starting with `@`).
  `@solid-devtools/debugger@0.28.1` is the shape: `"@solid-devtools/source":
  "./src/index.ts"` beside `"files": ["dist"]`, so the source tree is omitted
  from the tarball on purpose. The artifact itself proves the target
  unpublished, and namespacing is the published convention for a condition one
  tool opts into by name, so no consumer reaches it without naming it.
- **`non-module-target`** -- the selected runtime target's filename is one of an
  exact positive list of non-executable resources (`.map`, `.json`, `.css`,
  images, fonts, `.txt`/`.md`/`.html`). `@kobalte/solidbase`'s
  `"./default-theme/*"` enumerates sourcemaps, JSON, and CSS beside real
  modules; an entrypoint over one of those has no ESM runtime surface.

Boundaries that deliberately keep full refusal semantics: a missing target
reached through standard conditions only stays a refusal, because real
consumers do fail there and that is a defective publish; a missing target behind
a **bare-name** custom condition stays a refusal for the same reason, because
the ecosystem that owns the name activates it unconditionally (`bun`,
`workerd`, `edge-light`, `react-native`, `electron`, `svelte`, and `solid`
itself, which vite-plugin-solid and solid-start switch on for every Solid
consumer); the `.` entrypoint under the empty/default partition can only ever
reach standard conditions, so its refusal always stands; and `blocked`,
`conditions-unmatched`, `not-exported`, invalid target syntax, and traversal are
properties of the package and are untouched.

Both questions are decided against one shared vocabulary in
`packages/cli/scripts/artifact-resolution.mjs`. `isCustomCondition` answers
"this resolver does not define the name", reading
`RESOLVER_STANDARD_CONDITIONS`, `MUTUALLY_EXCLUSIVE_CONDITION_AXES`, and
`ECOSYSTEM_DEFAULT_CONDITIONS` (`solid`); `isPrivateNamespacedCondition` narrows
that to the namespaced names the disposition rule acts on. `solid` is
deliberately *not* in `RESOLVER_STANDARD_CONDITIONS`: that list is the
census-exclusion list of names active without a consumer naming them, and the
resolver does not activate `solid` implicitly (Rust's replay does not either,
and the two must select the same target), so the census still enumerates it as
an axis and really exercises the branch --
`fixtures/package-contracts/conditional-targets` pins the resulting refusal.

Non-module extensions are a **positive** list, not the complement of the
resolver's `RUNTIME_EXTENSIONS`. The complement swallowed `.node` and `.wasm`
and every unknown suffix, and an entrypoint of that kind is emphatically not
"nothing to assert" -- the closure already names those two as
native-code/opaque-wasm hazards -- so everything outside the positive list keeps
certify-or-refuse. A `.d.ts` runtime target still ends in a module extension and
keeps whatever disposition it has today; a target with no extension is not
answered by the rule at all.

The rule is about *entrypoints only*. An asset that an analyzed module imports
is still an ordinary closure member with the role the closure gives it; the
`asset-import` and `asset-query-import` fixtures pin that path unchanged.

Representation: the proposal refusal sidecar
(`solid-checker-contract-proposal-refusals`, `refusalVersion` 1) gained an
additive sibling array `inapplicable`, whose rows carry `entrypoint`,
`conditions`, `stage`, `class`, and `reason`. No version bump: every consumer
validates `format`, `refusalVersion`, and `Array.isArray(refusals)`, and every
one of them counts `refusals.length` as the refusal total, so a separate array
makes "never counted as a refusal" true by construction. A `disposition` field
*inside* `refusals` would have needed each counter edited and would have kept
the pollution wherever one was missed.

There is no Rust twin to add. Rust never enumerates the manifest census: it
replays the resolution of the cases a proposal *contains*
(`resolve_snapshot_export`), and the case-set completeness check in
`rust/crates/solid-facts-backend/src/main.rs` compares planning against the
proposal document's own cases. An inapplicable case is omitted from the
proposal exactly as a refused case already is, so both sides stay in agreement
without a new surface.

Regression pins: `fixtures/package-contracts/unpublished-conditional-target`
(namespaced-condition case inapplicable, sibling entrypoints certify, and a
`browser` arm whose missing target still refuses),
`fixtures/package-contracts/conditional-targets` (a missing target behind the
bare-name `solid` condition refuses, while the shipped `development`/`default`
branches still certify) and
`fixtures/package-contracts/wildcard-asset-entrypoints` (module entrypoint
certifies, `.map`/`.json`/`.css` entrypoints inapplicable, zero refusals). The
`.` default-partition boundary is pinned by the generator unit test *a fully
refused proposal writes every artifact-case refusal before throwing*, which now
also asserts an empty `inapplicable` census.

Remaining approximation: the classification is filename- and
existence-based, not a proof that the target has no ESM surface. A `.js` file
that is in fact a CJS bundle, or a `.ts` target that ships but cannot be parsed,
is untouched by this change and keeps its current refusal. The 1,280 `.ts`
"resolved target is not a file" cases are reduced only where a *namespaced*
condition selected them; a `.ts` target missing behind standard or bare-name
conditions is still a refusal. Namespacing is likewise a convention, not a
proof: a package that ships a private branch under a bare name, or a public one
under a namespaced name, is classified by the convention rather than by
evidence, and only the artifact's own absent target keeps that fail-closed.

Benchmark classification, same date: `export-kind-conflict` was promoted above
`unresolved-parameter-behavior` in `MARKERS` so a full-generation refusal
carrying earlier `UnknownCallbackExecution` attribution records cannot be
relabelled by diagnostic line order. That rank is deliberate and was chosen from
the one real sample (the `createGeolocation` geolocation refusal); the risk it
carries is that any future message embedding the exact phrase "package contract
value export ... cannot have function effects" inside a differently-caused
refusal would now be claimed by this class first. No such shape is known.

## Callback obligations do not create value-export function domains (2026-08-31)

Contract-generation `UnknownCallbackExecution` obligations previously carried
only a function's runtime identity. A re-exported module namespace can share
that identity with several exported bindings, so direct attribution set
`callbacks` to OPEN on value siblings that never had an invocation subject.
Those constants were then correctly refused by the invariant that a closed
value export cannot carry function effects.

The IR obligation now also carries the exact compiler symbol of its owning
function. Joined entrypoints select exports by that canonical symbol first and
never fall back to broader runtime identity when symbol provenance exists. The
negative regressions keep value siblings closed and forbid fallback after an
unmatched exact symbol. The real geolocation conflict remains symbol-matched,
so the pinned downstream refusal is unchanged.

## Package imports maps fail closed until Rust replays them (2026-08-31)

The generator's proposal closure followed a matched `#specifier` into the
package's own imports map and pulled the resolved module into the closure
(`closureForRoots` in `packages/cli/scripts/artifact-resolution.mjs`, via
`packageImportTargetOrUnknown`). The certifier has no imports-map support at
all: `SnapshotPackageManifest` carries no `imports` field, and `resolve_local`
in `rust/crates/solid-facts-backend/src/contract_certification/module_closure.rs`
treats every `#specifier` as External. The two therefore build different
closures for the same artifact, and the proposal could only ever be rejected on
replay with a closure mismatch -- a refusal reported at certification time, with
no reason naming the actual cause.

A matched `#specifier` resolving to a local module is now an explicit
artifact-case refusal at generation time, code `package-imports-unsupported`,
applicability `unsupported-artifact-shape`, reason "package imports-map target
`./x.mjs` resolves into the closure; certifier replay does not support imports
maps yet". Fail-closed and named, rather than emitted and rejected later.

An **unmatched** `#specifier` is unchanged and still certifies: the census row
that activates no environment condition cannot say which arm a consumer selects,
which is unknown rather than absent, so the specifier stays an opaque frontier
and every claim reachable through the binding stays open.
`fixtures/package-contracts/conditional-imports-side-effect` pins that.

Fixed while there: in `closureForRoots` an unmatched `#specifier` fell through
into the **external dependency census**, where `locateExternalFrom` derived a
package name from `#platform`. It now takes the same unaccepted-external
frontier the other two closure builders already gave it, and never a census row.

**Named follow-up: teach the Rust certifier the imports map.** That means an
`imports` field on `SnapshotPackageManifest`, `resolve_local` resolving a
`#specifier` through it with the case's own conditions, and the generator's
refusal above deleted in the same change. Until then every package that uses a
matched imports map is uncertifiable, which is a real coverage hole rather than
a proof.

Regression pins: the generator unit tests *a matched package imports target
refuses the proposal closure until Rust replays imports maps* and *an unmatched
package imports specifier stays an open frontier, never an external dependency*
in `packages/cli/test/artifact-resolution.test.mjs`. Graph **planning**
(`resolvePackageDependencyPlanClosure`) is a different operation with no
replayed closure and still follows the alias exactly; its own test is unchanged.

Package-scope ownership is also exact: a `#specifier` in a nested dependency is
selected against the nearest `package.json` owning the importing module, not
the certification root manifest, and the module cache binds that owner manifest
digest. A dependency-owned match reaches the existing
`package-imports-unsupported` refusal; a dependency cannot inherit a parent
package's imports map. Rust imports-map replay remains the named follow-up.

## Conditional targets backtrack the way Node resolves them (2026-08-31)

Node's PACKAGE_TARGET_RESOLVE continues to the next key when a matched key's own
target resolves to nothing. Both sides returned on the first *matching* key
instead, so `"./a": {"vendor": {"browser": "./missing.js"}, "default":
"./index.js"}` refused under conditions `["vendor"]` -- reporting a defect where
every real consumer resolves `./index.js`.

`selectTarget` in `packages/cli/scripts/artifact-resolution.mjs` and its twin
`select_target` in
`rust/crates/solid-facts-backend/src/contract_certification.rs` now backtrack
identically: a nested selection that matched no condition continues to the next
key, and only an object that yields nothing at all refuses. Rust needed a
`TargetSelectionError::ConditionsUnmatched` variant to tell that outcome apart
from the refusals that must still stop where they happen -- a `null` (blocked)
target, a target the snapshot does not contain, and an invalid target are
properties of the package, not unmatched conditions, and none of them
backtracks.

Trace semantics: an abandoned branch's steps and conditions-taken are
**discarded, never merged**. Both implementations copy the context per key
rather than extending a shared list, so the hashed resolution trace names
exactly the branch a consumer's resolution traverses; recording a condition the
selection walked away from would put a name in the receipt that no resolution
ever took.

Top-level unmatched conditions are untouched:
`fixtures/package-contracts/conditional-export-absence` and
`torture-conditional-semantics` pin those refusals and neither snapshot moved.
Focused pins: *a nested object that selects nothing backtracks to the next
sibling key* (`packages/cli/test/artifact-resolution.test.mjs`) and
*an_unmatched_nested_condition_backtracks_to_the_next_sibling_key*
(`contract_certification.rs`).

## Legacy runtime resolution reads `module` again (2026-08-31)

The v2 artifact-resolution boundary resolved a legacy (no-`exports`) manifest's
runtime axis through `main`, then `index.js`, and never consulted `module`. That
lost a distinction the pre-v2 generator already made (see "Legacy
`module`/`main` provenance was invisible"): a legacy dual package whose `main`
is the CJS transpile of the same source landed on the CJS sibling and was
refused with "no runtime ESM exports", even though the artifact ships a real
ESM build.

Legacy runtime resolution now prefers the `module` target when it is declared
and names a file inside the artifact, falling back to `main` and then
`index.js`. The trace branch is `legacy:module`, so the certifier replays the
same field and receipts stay attributable. The declarations axis is unchanged:
`module` never names a typing, so it is not consulted there.

A declared `module` target that names no file in the artifact -- or is a
traversal, an escaping path, or a non-string -- is not a refusal. Node consumers
never read `module` at all, so the `main` surface is the one every runtime
consumer loads and is still real; resolution falls back to it.

Both the Rust snapshot replay (`resolve_legacy` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`) and the
JavaScript generator (`resolvePackageExport` in
`packages/cli/scripts/artifact-resolution.mjs`) select the field by the same
predicate, so the generator and the certifier cannot disagree about which field
won. The two sides still validate the *selected* legacy target differently --
Rust rejects traversal, `node_modules`, and percent-encoded segments; the
JavaScript `main`/`index` path does not -- which predates this change and
remains a fail-closed mismatch at certification rather than a silent
divergence.

Regression pins: `fixtures/package-contracts/legacy-module-entry` (present
`module` wins, `legacy:module`), `legacy-module-absent` (absent `module` falls
back, `legacy:main`), and `legacy-dual-root`, whose refusal moved from the
runtime axis to the declaration axis -- it publishes no `types`/`typings`, so
declarations still fall back to the CJS `main`, which carries no declaration
binding for the ESM build's export. That remains exactly refused; a legacy dual
package with no published typings is still uncertifiable, and this change does
not let the generator assume the two builds agree. Unit pins:
`legacy_runtime_resolution_prefers_a_present_module_target_over_main` and
`legacy_runtime_resolution_falls_back_to_main_when_module_is_unusable` in
`contract_certification.rs`, plus the matching pair in
`packages/cli/test/artifact-resolution.test.mjs`.

The ecosystem benchmark was not re-run, so `benchmarks/ecosystem/report.json`
still records the old behavior. Its seven full-row `no-exported-surface`
refusals were spot-checked against the warm local package cache instead, with
no network access and no install. Six declare a present `module`; five of them
now generate a proposal through `legacy:module`
(`@solid-primitives/{until,countdown,date-difference,reducer}`,
`@solid-devtools/extension-adapter@0.12.1`). `@solid-devtools/ext-adapter@0.17.0`
still refuses, now truthfully: its ESM build is a side-effect-only entry that
exports nothing, so "no runtime ESM exports" is a fact about the artifact
rather than about the field that was resolved.
`@solid-devtools/babel-plugin@0.3.1` declares no `module` at all and is
genuinely CJS-only; it keeps refusing unchanged.

## Type Facts authenticates non-export proof subjects (2026-08-31)

Policy-2 certification formerly allowed an export-value transcript to answer
only `Export`-rooted recursive value and domain-closure demands. Selected-call,
callback-binding, operation, and operation-value demands for the same export
were refused before the in-repo Type Facts producer could inspect the runtime
implementation.

The verifier-owned declaration harness still authenticates the public export
value and signature. Each demand now additionally binds the snapshot-replayed
runtime identifier span, and protocol 6 returns an exhaustive implementation
transcript: parameter flows, exact resolved calls with reachability, returned
closure captures, and control-flow facts. Family adapters consume only the
local premise they need. A reachable callback call inside `try/finally` can
therefore certify while whole-function control flow remains open; a captured
call certifies only when a reachable return proves that exact parameter is
captured by the returned closure.

The transcript also preserves an exact direct-import type reference when the
private declaration harness cannot load a peer package. Only the dialect-owned
`solid-js` `Accessor` and `Setter` exports may close an otherwise unknown root
callability premise; a same-named local or third-party type stays open. Return
value provenance likewise authenticates direct closures and exact
`solid-js#createSignal` tuple items, and requires every reachable return branch
to establish the demanded callable path before it closes.

Missing runtime identifier bindings, ambiguous signatures, unresolved call
targets, unknown call reachability, retained-but-unreturned closures, tighter
than `0..many` cardinality, and unsupported operation kinds remain explicit
uncertifiable frontiers. A parameter used only inside a captured callback also
remains open unless exact callback-owner evidence proves that callback's
execution; a dialect fact for a nested call is not authority for its containing
helper. Constructable classes follow the existing runtime
function-shape rule; a value proof still requires both non-callable and
non-constructable negatives. The generated
`typefacts-implementation-transcript` fixture pins direct and returned-closure
positives plus the retained-closure negative without duplicating a TypeScript
diagnostic.

## Phase 21 published dependency graph transaction (2026-08-31)

Published dependency certification no longer treats recursive eager child
certification as proposal preparation. One run now deduplicates canonical nodes
by full artifact and resolution identity, generates dependency-first frontiers,
and submits one deduplicated case set to one native bottom-up certification
transaction. All reuse is bounded by its authority: immutable archive snapshots
and dependency-closure memo entries are transaction-local, closure hits rehash
current bytes through current realpaths, package and lock identity is reread,
and facts are shared only across independently canonicalized equal source
programs in one native emission request. Open dependency proposals and retained
refusal audits remain generation-only material. The final transaction replays
every archive, lock selection, graph root, source closure, proposal, and policy
input independently, and a fresh ordinary analyzer process must authenticate
and select the issued policy-2 receipt.

The final focused uncached Corvu graph certifies in 7.997 seconds (8.352 seconds
end to end) with 18 roots, 42 canonical nodes, 25 acquired published artifacts,
179 compiler-closure sources, one native transaction, and one Type Facts batch.
Proposal generation takes 5.762 seconds and native witness acquisition 2.015
seconds. The final focused Kobalte probe takes 21.144 seconds, including 19.564
seconds of proposal generation for 629 artifact-case candidates. Its exact
source programs form 621 batches and avoid only eight fact builds: no facts are
shared across unequal programs. The earlier condition-compatible batching
result remains rejected, and recursive eager child certification was not
reintroduced.

The final-code certified-release uncached 418-row corpus takes 112.595 seconds,
7.405 seconds beneath the hard two-minute gate. Its report SHA-256 is
`c4fdede40e69dcccada59749d9fec277db4a5c0a06956d4b5ed1649f41d33478`.
It contains 52 complete-contract generator successes, 324 partials, and 42
failures, with 41 certified and 61 exactly refused certification attempts. The
regenerated Phase 21 ledger
records every one of the 30 baseline fully refused rows: Corvu verifies, 15
dependency rows retain exact semantic or proof-policy refusals, Geolocation is
partial with an absent published target, Context is a confirmed upstream
declaration defect, and the five missing-byte plus seven CJS/no-ESM controls
remain fail-closed.

The 15 checker-addressable exact refusals are not one bucket disguised as a
resolution: one lacks the installed `@solidjs/router`, four lack the installed
`@solidjs/web` layout, three reach unresolved TanStack package-import targets,
five retain value-export/function-effect normalization contradictions, and two
hit unsupported non-export Type Facts transcript demands. The report preserves
six raw `unclassified` observations for provenance; the Phase 21 ledger gives
them exact terminal classes while retaining the observed value separately.

The JavaScript dependency plan digest still contains temporary absolute paths
and omits `rootIdentity`. It is diagnostic planning evidence only; native graph
reconstruction, authenticated inputs, receipt issuance, and fresh-process
selection do not trust it. This limitation is not a relaxation of archive,
lockfile, graph-root, source-closure, receipt, or policy authority.

## Uncertifiable baseline and evidence-owner matrix (2026-08-21)

The dirty-worktree baseline was coherent on the current source: the Reactive
IR library tests passed, all 76 armed backend process tests passed, and the
fresh-debug-binary coverage comparison passed for 72 fixture projects (517
findings). After the reviewed runtime-identity, environment-selector,
package-owner, closed-local-callback, dialect-selection,
rendering-premise, caller-witness, callback-extent, nested-transport,
object-graph, and program-boundary slices below, the snapshots contain 130
\`uncertifiable\` findings across 524 findings in 77 fixture projects. This is an inventory of the current proof obligations, not a
promise that every row is reducible; the last column records the only sound
owner that could discharge it.

The count moved 126 → 130 while precision improved, and the two are not in
tension. Every new proof path below needs its fail-closed controls pinned:
\`props-caller-witness\` contributes two honest uncertifiable results doing
exactly that, and the nested-transport slice two more. A negative control that
reports an obligation is the evidence that the new positive path did not
overreach; deleting one to lower the number would remove the only thing holding
the reduction honest. **The count is not a precision score, and the sections
below are the reason.**

### The corpus was the blind spot, not the facts (2026-08-21)

All four fact sources are saturated: every field `EntityFact` emits (15), every
`AstFacts` table (28), every schema-v1 contract property (39), and every
compiler execution-map table is consumed by a rule. "Extract more from the
producers" is largely finished as a strategy, and the two capabilities that
were genuinely missing are now supplied.

What is not finished is the corpus. Three real defects were found in one week
— a discarded caller witness, a false violation from a mis-attributed callback
read, and an undemanded nested library identity — and **none of them moved a
single fixture** across 76 projects. All three were found by writing a scratch
project by hand. The fixtures test the shapes their authors thought of: the
`interprocedural` fixture was two files and seventeen lines with one shape, and
the component fixtures are single files whose exported components have no
callers at all.

Two things came out of that:

- `scripts/obligation-audit.mjs` turns the manual probing into a gate. Every
  obligation states the evidence that would settle it and what the checker must
  say once it is present, so an over-conservatism can no longer pass as a
  missing fact. An obligation that closes on its own fails too — that is a real
  change, and it should be recorded rather than absorbed. Seven obligations,
  eleven closures, run against the audited published typings.
- `fixtures/reactive-ir/realistic-topology` is a project shaped like a project:
  components in their own files rendered by other components, a helper called
  from a component body, a module-scope source read across files. Under a closed
  boundary it produces six analyzed sites, three findings, all proven, and no
  obligations — which is what a well-analyzed application should look like, and
  is only reachable because the topology supplies the facts.

### Where the floor actually is (measured 2026-08-21)

The remaining obligations were probed rather than classified by eye: for each
large cluster, the closing evidence was supplied in a scratch project and the
checker was asked whether it closes.

| Cluster | Count | Closes when the evidence is supplied? | Why the fixture case cannot |
| --- | ---: | --- | --- |
| SC1001 / SC1003 props | 49 | Yes. A dynamic in-project caller proves the violation; a complete static caller set certifies it silent. | The engine and corpus fixtures have no in-project callers at all. |
| SC4001 owner | 27 | Yes. A \`createRoot\` call site certifies silent; a module-scope call site proves the violation. | Exported helpers whose caller set is incomplete. |
| SC9005 | 22 | Not applicable. | Wrong subpath, absent export, or unreviewed package — the fixtures exist to pin that. |
| SC9012 | 9 | No. | Divergent dispatch, globals, and opaque adapters by construction. |
| SC7005 | 5 | Never. | Per-request settlement race; no source fact decides it. |
| Type Facts–owned | 17 | Partly — and no longer fact-limited. | Broad numbers, non-exact tuples, dynamic serializer config. Object graphs now certify or report; what remains there is a non-finite `number` and three deliberate controls. |

Only **two** of the 130 were ever limited by the facts themselves, and both
were found by reading the produced facts rather than the rules. Both are now
closed, and the object-graph residue is at its floor — see the
nested-transport and object-graph entries below. What is left in that row is
irreducible in principle (a broad `number` may be non-finite) or a deliberate
control (a getter, a twice-referenced binding, a spread).

So the number is bounded by the corpus, not by the checker: the top two rows —
76 of 128 — are cases where a fixture deliberately withholds the closing
evidence, and the machinery demonstrably closes them the moment it is present.
Lowering those rows would mean adding callers to fixtures whose purpose is to
pin the open-world boundary, which destroys the case rather than improving
precision. Two rows are irreducible in principle.

One genuine fixture-hygiene item is recorded rather than taken: several SC4001
obligations in \`dialect-solid-1x\`/\`dialect-solid-2\` are *incidental* to
fixtures whose subject is \`createEffect\` argument shapes, and the technique
that removes them without weakening the claim is now demonstrated in
\`summary-callback-extent\` (render the host at an exact JSX call site so it is
a proven owner, or wrap the call site in \`createRoot\`). It was not applied
here: that pair is the pinned differential-dialect fixture and keeps message
wording, so restructuring it is a deliberate change on its own, not a
by-product of chasing a count.

| Finding | Count | Current contexts | Missing evidence and audit classification |
| --- | ---: | --- | --- |
| SC1001 | 35 | component props aliases/read sites in the engine and eslint corpora; Solid 1.x sources; \`solid2-precision\`; v1 reactivity; upstream component cases | Exact JSX callers, immutable/enumerable prop backing, or a component contract. Project IR can reduce closed-world/cross-file cases; exported/open-world props remain genuinely uncertain. |
| SC1002 | 1 | \`props-callers\` callback after \`await\` | Exact synchronous callback extent and caller-proven prop/accessor identity. Project IR/compiler facts are reducible; opaque callbacks remain fail-closed. |
| SC1003 | 14 | component parameter/body destructuring in engine/corpora and wrapped components | Proven component identity plus exact prop backing/caller set. Project/compiler facts can reduce exact JSX calls; ordinary/exported components remain uncertain. |
| SC1004 | 2 | conditional component returns in the engine corpus | Proven component execution identity and return control-flow shape. JSX/compiler evidence is reducible; unknown component calls remain uncertain. |
| SC1007 | 3 | reactive handler reads in shared Solid 2 and v1 reactivity fixtures | Exact runtime handler domain/tuple shape and reactive prop backing. Existing TypeFacts closes exact values; mixed/opaque prop sources remain uncertain. |
| SC3001 | 1 | exported \`onSettled\` helper in \`leaf-owner\` | Exact callback identity and synchronous dynamic extent. The exported helper's owner call sites remain open-world; closed local callback adapters are now followed. |
| SC4001 | 27 | exported/ambiguous component and helper owners across dialect, engine, corpus, and precision fixtures | Compiler owner regions, exact caller topology, and package callback owner behavior. Local/closed callers are reducible; exported library callers and conditional owners remain open-world obligations. |
| SC5001 | 1 | async boundary with opaque source options | Exact option-object initializer (\`loadingValue\`/\`seedLoadingValue\`) and selected runtime entry. TypeFacts/options facts and explicit runtime conditions are reducible; dynamic options remain uncertain. |
| SC5003 | 1 | unresolved CSR/SSR boundary fixture | An explicit rendering selector now discharges this outright: \`rendering: "csr"\` proves the premise false and SSR proves it true. The remaining case has no selector, and no visible server entry does not prove CSR-only, so it is reducible only by user configuration or a cross-project compiler fact. |
| SC7001 | 2 | spread-hidden Solid 1.x/2.0 effect callback and \`"use server"\` controls | Exact tuple-slot/expanded spread facts plus selected runtime/framework entry. TypeFacts tuple arity is reducible; framework directives without an explicit compiler contract remain uncertain. |
| SC7005 | 5 | HTTP response writes in CSR and SSR flush fixtures | Request-dependent settlement relative to shell flush. An explicit \`rendering: "csr"\` selector now discharges the claim entirely — no shell, no committed response head — but that removes the subject rather than deciding the timing. Where SSR is selected or unresolved, whether a boundary settles before or after the flush is a per-request race no source fact can decide, and all five remain irreducible. |
| SC7007 | 4 | server-function rich arguments and dynamic serializer configuration | Exact immutable serializer options and closed finite literal graphs. TypeFacts can reduce exact constants/primitive domains; arbitrary object graphs, casts, spreads, and dynamic configuration remain uncertain. |
| SC9005 | 22 | missing/partial Solid contracts, unknown package callbacks/exports, wrong subpaths, and stale fixture contracts | Exact reviewed package/entrypoint/export summaries, runtime identity, and selected variants. Contract schema/generator/consumer parity and bundled ecosystem coverage are reducible; unreviewed or absent packages remain correctly uncertain. |
| SC9011 | 1 | exported reactive source in v1 reactivity | Exact caller capture or package/source contract. Closed local callers are reducible; an exported source escaping to uncontracted code is genuinely open-world. |
| SC9012 | 9 | divergent method dispatch, opaque package/leaf callbacks, structured returns, and Solid 2 precision | Exhaustive equivalent target summaries, exact returned adapters, callback owner behavior, and contract propagation through aliases/re-exports. Indexed identity/contract fields are reducible; divergent/opaque targets remain fail-closed. |

The package-contract audit therefore starts with SC9005/SC9012 and the
contract-owned portions of SC1001, SC3001, SC4001, SC7001, and SC9011. The
environment-dependent SC5003/SC7001 paths and the TypeFacts-owned SC5001/
SC7007 paths are separate workstreams. SC7005 is intentionally retained in
the irreducible ledger even when SSR is explicitly selected.

- **2026-08-21 — an explicit program boundary is evidence, and it is the
  largest lever in the corpus.** Seventy-six of the obligations came from one
  assumption: an exported symbol may be imported by code this build cannot
  see, so its callers cannot be enumerated and neither its props' backing nor
  its owner can be settled. Nothing inside a tsconfig proves the opposite,
  which puts this in the same class as `rendering` — a premise only the user
  can supply. `RuntimeEnvironment::program_boundary` now carries it
  (`--program-boundary open|closed`, `"programBoundary"` in
  `.solid-checker/runtime.json` and in the ESLint adapter's runtime settings,
  where it joins the snapshot cache key).
  Selecting `closed` removes exactly one assumption: that an *additional,
  unseen* caller exists. It licenses nothing else. Two places consume it, and
  each drops one open-world artifact. `classify_one_component` stops treating
  exportedness as an escape, and stops treating an `export { Card }` specifier
  as a non-JSX reference — that specifier reaches an importer only if one
  exists, and under a closed program every importer's use is itself in the
  reference list. Aliasing and passing the component to a receiver still
  escape, because closing the program says nothing about what the receiver
  does. The owner graph stops *seeding* an exported non-component helper
  `UNOWNED`; that seed is the unseen caller, and with it gone the enumerated
  call sites decide the owner.
  Everything else is unchanged and deliberately so: a caller set must still be
  enumerated exactly, a reference that resolves to a use the analyzer does not
  understand still escapes, and a missing reference list is still the absence
  of a fact rather than proof of no callers. `program-boundary-closed` pins all
  five rows, including the two that prove the assertion cannot manufacture a
  finding (a dynamic witness and an unowned module-scope call are violations
  either way) and the one that proves it is not a blanket amnesty. A unit test
  pins that the boundary never reaches `selected_conditions`, so asserting a
  closed program cannot silently select a different package entrypoint. The
  open-world fixtures keep the default and keep their obligations: they exist
  to pin what is provable *without* this premise.
- **2026-08-21 — the object-graph floor: the binder resolves the reference,
  and a property kind closes the literal.** Two facts finished the job the
  nested-transport slice started, and each removed a different obstacle.
  `ArgumentFact::binding_declaration` records the declaration Oxc's scope tree
  resolved an identifier argument to — the same contract
  `ObjectPropertyFact::shorthand_binding` already carried. The demand plan now
  follows `save(payload)` to the literal `payload` was built from through the
  binder's own answer: one reference, one declaration, one literal. No
  spelling match, and none of the file-wide sweep that made this look
  unbounded. `ObjectPropertyFact::data` records whether a property is a plain
  data property (`kind: Init`, not a method), which is the fact a consumer
  needs to close a literal *against accessors* and so conclude something about
  every value in it. Without it `{ get when() { return new Date(); } }` is
  indistinguishable from `{ when: "2026-01-01" }`, and would read as JSON-safe
  when it is not; `exact_object_literal` carries the same guarantee but only
  for a literal written directly as an argument.
  Together they make the graph walk two-sided: witnessing a rich leaf needs
  only that nothing displaced it (no spread, no computed or duplicate key),
  while certifying the graph safe additionally needs every property to be a
  data property with a proven JSON-safe leaf. `savePlain(payload)` and a
  nested container now certify silent, `saveBoxed(boxed)` is a proven
  violation, and the getter, twice-referenced-binding, and spread cases are
  each pinned as obligations. Both new demands are cache-stable — a library
  identity and a primitive domain are the same for every inhabitant of a type,
  so unlike a type descriptor or a constant value neither makes `{ n: 0 }` →
  `{ n: 1 }` invalidate the table. Performance re-certified.
- **2026-08-21 — JSON reaches nested values, and so does the proof.**
  `JSON.stringify` flattens a Date sealed inside an object exactly as it
  flattens a top-level one, but SC7007 only ever checked the argument's own
  library identity, so `save({ title, when })` shrugged where `save(when)`
  reported. The demand plan now asks for the same library identity at each
  property value of an object-literal argument — spans taken from *inside* the
  argument, so the cost is bounded by the argument rather than the project, and
  a library identity is stable across a type's inhabitants, so unlike a type
  descriptor or constant value it cannot make `{ n: 0 }` → `{ n: 1 }` a
  table-invalidating edit. The consumer witnesses a rich leaf at any depth.
  This is the *presence* half of the proof and only that half: it never
  concludes a graph is JSON-safe. Every condition is a soundness requirement —
  no spread at any depth (a later spread overwrites an earlier explicit
  property), no computed key (it may collide with the witness's name), distinct
  static keys (a duplicate later key wins), and for the through-a-binding path
  an immutable binding referenced exactly once, so nothing can mutate a
  property or hand the object to something that does between construction and
  the call. Shorthand properties resolve through the binder's recorded
  `shorthandBinding`, because a symbol query at a shorthand span answers with
  the property's symbol, never the value binding's. The oracle ledger entry
  that documented the old limitation now records the proof; `tsc` is still
  silent there, so this remains transport behavior no signature expresses.
- **2026-08-21 — a caller witness survives the open world.** Caller-proven
  prop reactivity is two questions with opposite quantifiers. "Some caller
  passes a reactive expression" needs one witness and is *monotone*: a
  consumer outside the project can add a call site, never unwrite the one
  written here. "Every caller passes a static value" is falsified by a single
  unseen caller and needs the complete set. `PropsReactivity` had one state
  for both — an exported component collapsed to "nothing about its props is
  provable" — so an in-project `<C title={n()} />` was discarded and the
  untracked read of `title` reported an obligation where a violation was
  proven. The state is now `Escaping`, carrying the witness sets from the JSX
  call sites that *are* visible while refusing to conclude `Static` for
  anything. Witnesses are per prop name, so one dynamic prop does not make
  every prop on the same component report, and a spread anywhere on an element
  discards that element's witnesses entirely (a later spread wins over an
  earlier explicit attribute, so it can overwrite a dynamic value with a
  static one). `props-caller-witness` pins all four rows. No existing fixture
  moved: none has an exported component with an in-project dynamic caller,
  which is why the over-conservatism survived.
- **2026-08-21 — a read inside a tracked callback is not its caller's read.**
  The interprocedural read summary excluded exactly one shape — Solid 2.0's
  `createEffect` *apply* slot, matched by primitive name and `Deferred`. A
  Solid 1.x effect's callback is `Tracked`, so it fell through, and a helper
  whose only read sat inside `createEffect` exported that read to its callers.
  Calling it from a render scope produced a **proven SC1001 violation** for a
  read that never happens at the call site — while the identical read inside
  the helper was correctly silent. The two halves of the analyzer disagreed
  and the interprocedural half was wrong; a false violation is worse than a
  missing one. The filter is now `read_escapes_synchronous_extent`, keyed on
  the dialect's own callback vocabulary: `Inline` reads "subscribe whatever
  was tracking at the call site" and propagate, while `Tracked` and `Deferred`
  do not. It also requires a function literal between the read and the
  argument, so an eagerly evaluated argument — `onMount(compute(count()))`,
  where the slot is Deferred but nothing defers the read — still propagates.
  `summary-callback-extent` pins all five executions and reports three
  violations and nothing else. No existing fixture moved: the corpus's
  `interprocedural` fixture covers only the direct-read case, which is why
  this survived.
- **2026-08-21 — a silently mis-dialected parity corpus, and the gate that
  catches the next one.** `fixtures/reactive-ir/eslint-plugin-corpus-v1`
  shipped an *empty* `node_modules/solid-js/` directory. Git cannot record an
  empty directory, so the stub never existed, dialect selection fell back to
  the 2.0 default, and the fixture named `-v1` — with a `solid-js.d.ts`
  headed "Solid 1.x declarations … verified against solid-js 1.9.14" — pinned
  the 2.0 catalog. Its snapshot recorded that as if intended: no rule carried
  the `v1/` prefix. Adding the tracked stub moves 10 findings. Four SC9005
  obligations discharge against the reviewed bundled Solid 1.x contract, which
  exports `Index`, `mergeProps`, and `splitProps` from `.` — they were only
  missing because the *2.0* contract was being consulted. Seven SC1001/SC1003
  props obligations become proven violations, because 1.x props are always the
  compiler's reactive proxy while 2.0 needs caller-proven backing. Three
  findings on files named `*-valid.tsx` disappear: two SC7001
  `missing-effect-function` and one SC2001 were the 2.0 catalog misreading
  1.x `createEffect`, i.e. false positives on the corpus's own negative cases.
  Four SC1001 violations inside `Show`/`For` callbacks become uncertifiable,
  which is correct and not a regression: `direct_jsx_return_is_component()` is
  false for 1.x, where an exported PascalCase function returning JSX may be a
  *tracked render callback* rather than a component, and its callers cannot be
  enumerated. `control-flow-invalid.tsx` was a byte copy of the 2.0 corpus
  file, `keyed` prop and all; 1.x `For` has no `keyed`, so `keyed={false}` and
  `keyed={item => …}` are TS2322 against the fixture's own 1.x declarations.
  The accessor-item case is now spelled the way 1.x spells it, with `<Index>`;
  custom keying has no 1.x equivalent and stays covered only by the 2.0
  corpus. `fixtures/package-contracts/solid-reexport` had the second shape of
  the same trap — a stub present locally but with no `.gitignore` exception,
  so absent in CI. `scripts/coverage.mjs` now holds every fixture dialect stub
  to being present, parseable, versioned, and git-tracked, and names the
  `.gitignore` lines to add; both shapes fail it.
- **2026-08-21 — an explicit client-only rendering selector is evidence.**
  `project_server_renders` returned a bool, folding "the user selected CSR"
  into the same state as "no server entry is visible here". So selecting
  `--rendering csr` produced an SC5003 uncertifiable result whose own message
  read "the analyzed project cannot prove whether a server-rendering entry
  exists" — it could; the user had said so. The fact is now three-state
  (`ServerRenderingPremise::{Renders, ProvenClientOnly, Unresolved}`). A bare
  `ssrSource: "client"` source is a server-render hole only under `Renders`
  and a proof obligation only under `Unresolved`; under `ProvenClientOnly`
  the hole cannot exist and SC5003 makes neither claim. SC7005 follows the
  same premise for the same reason: its whole subject is the SSR shell flush
  committing the response head, and under proven CSR there is no shell, no
  committed head, and nothing to drop. `Unresolved` still reports for both,
  because a server entry in another tsconfig or package would make the defect
  real, and absence of a visible entry is not evidence of absence. The new
  `rendering-csr-selected` fixture pins the third state and carries a positive
  control — SC5003's *async* arm does not depend on the rendering premise and
  still fires — so the fixture cannot pass by containing nothing analyzable;
  dropping its selector turns the two quiet cases into three uncertifiable
  findings. This adds a proof path and its coverage; it lowers no count in the
  baseline above, because no pre-existing fixture selects a rendering mode of
  CSR, and neither `ssr-client-boundary-csr` nor `http-response-flush-csr` may
  gain one — pinning the unresolved state is exactly what they are for.
- **2026-08-21 — primitive domains, tuple arity, and runtime identity close
  three compiler-owned gaps.** Type Facts now exposes an alias-transparent
  primitive value domain, an all-numeric-constituents-are-finite guarantee,
  exact required-only tuple length, and exact runtime identity through the
  existing schema-v1 lifecycle. SC7007 certifies declared safe primitive
  aliases/unions and proves bigint/symbol/undefined-only arguments unsafe;
  broad numbers and object graphs remain uncertifiable. SC7001 proves an
  absent Solid 2 apply slot through an exact one-element spread tuple while
  hidden tuple contents and non-exact tuple shapes remain uncertifiable.
  Structured-return shorthands use compiler runtime identity and exact symbol
  declarations to close tsconfig paths and compiler-selected relative targets;
  external packages (including relative project re-exports) and globals still
  produce SC9012. The producer keeps compact bitsets, preserves the
  retained entity-row budget, and showed no material latency/allocation
  regression in the retained benchmark.
- **2026-08-21 — closed local leaf callback adapters.** `cleanup.rs` now
  follows a callback-producing call only when its exact in-project function has
  one unconditional return of a function literal or the exact callback
  parameter. The returned function is then scanned in its own synchronous
  extent, so the factory and identity-wrapper cases in `leaf-owner` become
  proven SC3001 violations instead of SC9012 obligations. Conditional returns,
  local aliases, package calls, and missing/invalid facts remain fail-closed.
- **2026-08-21 — reviewed contract joins and explicit runtime selection.**
  Exact Type Facts `runtimeIdentity` now joins direct package bindings to
  relative project re-export barrels in one indexed pass; conflicting exact
  summaries stay explicit SC9012/SC9005. The structured-return fixture lost
  its two external-barrel SC9012 obligations while its global remains
  uncertifiable. Native CLI, daemon, ESLint, and coverage metadata now carry
  target/build/rendering/condition/transform selectors in cache identity;
  conditional entrypoints and variants are consumed only when their selected
  evidence is exact. Explicit CSR/SSR selects the rendering premise but does
  not discharge request-dependent SC7005 timing.
- **2026-08-21 — schema-v1 package owner fields.** Additive callback `owner`
  rows and exported-call `ownerRequirements` preserve reviewed owner and leaf
  behavior across a package boundary. The incremental owner index consumes
  both fields; missing rows remain fail-closed and generated contracts put
  owner rows on the review checklist. The callback/owner consumer fixture now
  has one proven SC4001 owner violation and no false owner finding for its
  reviewed leaf callback.
- **2026-08-21 — source-vs-contract differential audit.**
  `scripts/contract-differential.mjs` now analyzes a source implementation,
  generates a contract, and compares the equivalent declaration/runtime
  consumer after an explicit reviewed promotion inside the harness. The
  generated-contract path now carries exact non-conditional owner requirements
  found inside direct exported functions; runtime-conditional and
  request-dependent owner paths remain review obligations. The generator
  checklist calls out both missing callback-owner rows and generated owner
  requirements.
- **2026-08-21 — contract proof boundary and exact conditional summaries.**
  Discovered inferred/generated contracts remain visible as SC9005
  `unverified` status but no longer enter Reactive IR, so an unreviewed claim
  cannot create a violation or suppress an obligation. Conditional generation
  collapses evidence-only and redundant development branches, merges their
  probe modes, retains genuinely disjoint target variants, and refuses
  overlapping semantic differences that schema v1 cannot express without
  negative predicates. Runtime configuration rejects contradictory target,
  build, and rendering selectors. Generated owner requirements now attach by
  canonical compiler symbol (including aliases and anonymous defaults) and by
  the immediate containing function, eliminating name and broad-span matches.
- **2026-08-21 — asserted server arguments use runtime-value facts.** SC7007
  now demands and consumes primitive, constant, and library facts at the peeled
  runtime expression behind transparent TypeScript wrappers. A bigint asserted
  as a safe scalar remains a proven violation; a finite number asserted as an
  unsafe scalar remains silent. The paired strict-`tsc` oracle is clean and the
  fixture adds one violation without increasing the uncertifiable baseline.

### Package-contract parity ledger

The source `ContractExport`/`ContractCallback` surface is now audited against
the consumer boundary. These claims are representable and consumed: reactive
reads, callback timing, callback owner context (including `leaf`), structured
returns, async behavior, exact conditional variants, exact runtime identity,
and direct exported owner requirements (`effect`, `cleanup`, `boundary`, and
`settled-cleanup`). Inferred rows still require review or attestation before
certification.

The following source/runtime behaviors remain explicit fail-closed obligations,
not silent omissions: argument-dependent computed callback maps; component
identity and reactive-prop obligations; reactive-write/action
constraints inside owned or leaf scopes; returned adapters whose callback
behavior appears only when the adapter is invoked; async/reactive-source
settlement through an uncontracted package; and conditional behavior whose
environment has not been selected. Each needs a stable contract field plus a
consumer proof path before it can be reduced. The generator currently refuses
CJS-only targets rather than inventing claims, and review output records
callback/owner gaps. No unreviewed inferred contract is used as certification.

The original exact reviewed `@solidjs/signals@2.0.0-rc.0` `isEqual` contract
closed the v2 oracle's inert-comparison gap. Phase 14 preserves that guarantee
against `@solidjs/signals@2.0.0-rc.3` through a new exact runtime/declaration
witness and proof-bound receipt. The RC.3 body is a strict identity comparison,
so all nine call-effect domains are complete-negative without saying anything
about sibling exports. Other exports remain intentionally unmodeled until each
runtime surface is audited; the v1 equivalent and arbitrary uncontracted
packages remain SC9005/SC9012 obligations.
- **2026-08-20 — cross-rule ownership follows effective enablement.** SC1004,
  SC5001, and Solid 2 SC1007 suppress an overlapping SC1001 only after both
  findings pass rule enablement. Disabling a more specific owner therefore
  restores the strict-read finding instead of silently deleting the whole
  defect. Retired JSX policies remain pinned by explicit negative ownership
  cases (including valid-jsx-nesting and no-implicit-draggable) as well as the
  permanent registry and migration tests.
- **2026-08-20 — control-flow preferences require reactive governing inputs.**
  SC8014 `prefer-for` now reports only when evaluating the rendered `.map`
  receiver performs a proven reactive read; SC8015 `prefer-show` applies the
  same requirement to the `&&` left operand or ternary test. Exact
  accessor/memo calls, store paths, interprocedural and package-contract read
  summaries, and Solid 2 caller-proven prop/accessor-prop reads are supported.
  SC8014 additionally requires an array/tuple Type Fact. Static values,
  once-captured snapshots, unknown calls, non-array members, and reads confined
  to callbacks or branches fail closed. Async callbacks are TypeScript-owned
  and skipped in Solid 1.x; Solid 2.0's published types accept them, so they can
  report but never receive the synchronous rewrite. Neither dialect promotes
  uncertain prop backing into proof for these preferences.
- **2026-08-21 — control-flow preferences use exact dispatch and scoped
  demands.** SC8014 now requires the compiler-selected declaration to be the
  standard-library `map` signature as well as an array/tuple receiver; a local
  or overridden same-name method fails closed. Its safe fix is limited to
  one-parameter arrows because a regular function can observe Array#map's
  three callback arguments through `arguments`. Array-shape Type Facts are
  requested only when `prefer-for` is effectively enabled. With the rule now
  default-on, default native and WASM analysis request them; an explicit native
  rule disable still removes the demand.

## 2026-08 preference defaults

`prefer-for` and `prefer-show` remain style preferences in both catalogs,
alongside `v1/prefer-classlist`, but all five external rule identities are now
enabled by default. Native callers opt out with `enabled: false` in rule
options; ESLint callers set the corresponding generated dialect rule to `off`.
The legacy preset and preference configs remain accepted but redundant. WASM
still lacks a rule-options transport, so it uses the new defaults and cannot
yet opt out.

The analyzer's known approximations, recorded so each is a decision with an
owner rather than a rediscovery. Items live here when a fix is a *design
change* — it would move findings broadly and needs its own fixture-gated
change — as opposed to the bounded corrections that land as ordinary fixes.

Direction legend: **FN** — misses real defects; **FP** — reports correct
code; **Both** — either, depending on the code.

## Rule-catalog reduction release notes

- **2026-08-20 — owner diagnostics merged.** `no-owner-effect`,
  `no-owner-cleanup`, `no-owner-boundary`, and the Solid 2-only
  `no-owner-settled-cleanup` now report as `missing-owner` / `SC4001` (with
  the `v1/` namespace in the Solid 1 catalog). Old rule-options keys are
  aliases to the merged family, so disabling any old member now disables all
  missing-owner variants. Explicit ESLint keys for the former members are a
  breaking removal. The `onSettled` cleanup message retains error severity;
  other proven variants retain warning severity.
- **2026-08-20 — leaf-owner diagnostics merged.**
  `cleanup-in-forbidden-scope`, `primitive-in-leaf-owner`, and
  `flush-in-forbidden-scope` now report as `leaf-owner-forbidden-call` /
  `SC3001`. This is a declared configuration break: old rule-options keys are
  accepted as retired no-ops, their disables do not transfer to the merged
  family, and the former explicit ESLint keys are removed.
- **2026-08-20 — unsuspendable pending reads merged.**
  `pending-async-untracked-read` and `pending-async-forbidden-scope` now report
  as `pending-async-unsuspendable-read` / `SC5001`. This is a declared break:
  old rule-options keys remain accepted as retired no-ops, disables do not
  transfer, and the old explicit ESLint keys are removed. Untracked variants
  retain error severity and leaf-owner variants retain warning severity.
- **2026-08-20 — loading-boundary diagnostics merged.**
  `ssr-client-source-outside-loading-boundary` now reports through the existing
  `async-outside-loading-boundary` / `SC5003` identity. The absorbed key is a
  retired no-op: its disable does not transfer and its explicit ESLint key is
  removed. Proven SSR client-source holes retain error severity; ordinary
  missing-fallback findings retain warning severity.
- **2026-08-20 — package-contract gaps merged.** Missing contracts, missing or
  environment-dependent exports, and unknown callback execution now report as
  `package-contract-incomplete` / `SC9005` (with `v1/` in the 1.x catalog).
  All six old rule-options keys alias the merged family, so disabling one now
  disables every contract-completeness variant. The old explicit ESLint keys
  are removed as a breaking change.
- **2026-08-20 — SC1003 names unified.** Solid 2.0's
  `component-props-destructure` is now `no-destructure`, matching the 1.x stem.
  The old rule-options key aliases the new identity; the explicit ESLint key
  remains temporarily as a deprecated delegate.
- **2026-08-20 — SC1004 names unified.** Solid 2.0's
  `component-returns-conditionally` is now `components-return-once`, matching
  the 1.x stem. The old rule-options key aliases the new identity; the explicit
  ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — SC1007 renamed.** `expected-function-got-expression` and
  `v1/expected-function-got-expression` are now `reactive-handler-frozen` and
  `v1/reactive-handler-frozen`. Both old rule-options keys alias their current
  identities; both explicit ESLint keys remain deprecated delegates.
- **2026-08-20 — SC2004 renamed.** `resolve-in-reactive-scope` is now
  `resolve-in-tracked-scope`, naming the precise execution fact the rule
  proves. The old rule-options key aliases the new identity; the explicit
  ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — SC7002 renamed.** `sync-node-received-async` is now
  `sync-computation-received-async`, naming the affected computation rather
  than an implementation node. The old rule-options key aliases the new
  identity; the explicit ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — three proven rule arms ported to Solid 2.0.** SC8014
  `prefer-for`, SC8015 `prefer-show`, and SC8003
  `jsx-no-duplicate-props`' intrinsic content-competition arm now run in the
  2.0 catalog. The 1.x DOM-slot folding arm stays dialect-restricted, and the
  2.0 list fix uses `<For keyed={false}>` because `Index` was removed.

### Reduction evidence retained with the release

The deletions were driven by runtime/compiler probes and real published
typings, not by catalog-size targets. Representative probe transcripts:

```text
createReaction callback: owner=PRESENT
createReaction callback cleanup: RAN
directive apply: owner=PRESENT
directive effect sees tick=0
directive effect sees tick=1
directive onCleanup RAN
1.9.14 spread onClick -> ATTACHED via delegated $$click
```

Those results removed the three false v1 ownership rules and the false
`warnOnSpread` premise. Runtime source also confirms handler/data arrays are a
supported dispatch form, array seeds are threaded into v1 effect callbacks,
and component props preserve their exact keys. Generic HTML, CSS, injection,
formatting, and naming policies were retired because they are outside the
checker's certification domain.

The real-typings probes also fixed the boundary with TypeScript. Representative
compiler output, in both strict and non-strict passes where applicable:

```text
TS2305: Module '"solid-js/web"' has no exported member 'createEffect'.
TS2540: Cannot assign to 'count' because it is a read-only property.
TS2322: Property 'dangerouslySetInnerHTML' does not exist on the intrinsic props type.
TS17001: JSX elements cannot have multiple attributes with the same name.
```

Accordingly `v1/imports` was removed entirely, readonly store-root writes stay
TypeScript-owned, and SC8003 retains only content-slot or compiler-folding
collisions that the type system does not already report. The executable
`fixtures/tsc-oracle/rule-cases.json` ledger remains the source of exact snippets,
diagnostic codes, and checker expectations.

## The `tsc` redundancy ledger (audited 2026-08-17)

AGENTS.md carries an absolute rule — never report what `tsc` reports, judged
against the library's *real published typings*. This section is that rule
applied to every rule in both catalogs, once, with evidence.

**How the evidence was produced.** `scripts/tsc-oracle.mjs` compiles a snippet
against packages installed from `fixtures/tsc-oracle/packages.json` at the
versions this repository audits — `solid-js@1.9.14` for the 1.x catalog (the
version `pkg/contracts/bundled/solid-v1/solid-js.json` was generated from) and
`solid-js`/`@solidjs/signals`/`@solidjs/web`@`2.0.0-rc.0` for the 2.0 catalog
(the versions in `pkg/contracts/bundled/runtime-lock.json`). It never reads a
fixture stub. Two passes run, `strict` and non-`strict`, because "the project
may not be `strict`" is a distinction a ledger entry has to state even though
the absolute rule refuses it as an exception. TypeScript 5.9.3.

**The product-owned ledger is held to its claims, by span.**
`scripts/ownership-gate.mjs` runs every case in
`fixtures/ownership-cases/cases.json` through the checker and through strict and
non-strict TypeScript over identical bytes. Each expected finding declares one
of `checker-only`, `typescript-owned`, or `distinct-claim`, with exact UTF-8
byte spans and any expected TS codes. Unlisted findings, overlapping claims,
missing fixes, and a TypeScript-owned finding emitted by the checker all fail.

The former 465-case upstream corpus is fully reconciled by
`migration-ledger.json`: 254 cases migrated into the product-owned manifest,
211 dropped with reasons, and zero pending. The old parity and deviation files
were deleted only after `make ownership-gate` enforced that completion.

### Duplicates the span comparison caught, both now narrowed

Found by the span comparison, each suppressed in `PENDING_NARROWING` with a
pointer here rather than left to fail:

**Landed 2026-08-17: `v1/jsx-no-duplicate-props`'s `children`-prop-plus-JSX-children
pair.** TS2710 is *"'children' are specified twice. The attribute named 'children'
will be overwritten."* — word for word the finding's claim, in **both** passes and
on components as well as intrinsic elements. (An earlier draft of this entry said
strict-only; that was a misreading — the strict-only diagnostic in this family is
TS2783, for the attribute-then-spread duplicate.) Only that exact pair is covered:
`innerHTML` with `textContent`, and `innerHTML` with JSX children, draw no
diagnostic at all, so a set including either still reports — the finding then
asserts more than TS2710 does even where TS2710 also fires. Pinned by
`eslint-compat`: the two surviving conflicts report, and the children pair is a
negative on both an intrinsic element and a component.
**Landed 2026-08-17: `v1/no-innerhtml`'s `dangerouslySetInnerHTML` arm** (upstream
cases 09, 10, 11). TS2322 *"Property 'dangerouslySetInnerHTML' does not exist"* and
the finding *"The dangerouslySetInnerHTML prop is not supported; use innerHTML
instead"* are the same claim. Narrowed to components, where props are whatever the
component declares and TypeScript is silent; the `innerHTML` arm is untouched
because `innerHTML` is a declared Solid prop and every claim about it is
independent. Pinned by `upstream-divergences`'s `ReactMarkupProp` — the silent
intrinsic, the reported component with its rewrite fix, and the reported component
whose extra object entry leaves no unambiguous rewrite.

Both are landed, and the ownership gate's confirmed-duplicate list is empty.

**The gate.** `scripts/tsc-oracle-gate.mjs` enforces
`fixtures/tsc-oracle/rule-cases.json` in `scripts/verify.sh` and as
`make tsc-oracle`. A rule whose positive case is also a `tsc` error fails CI, and
a removal justified by a diagnostic fails CI if that diagnostic ever disappears.
Verified in both directions.

It also enforces **completeness**: every rule in either catalog must have a case,
or an `EXEMPT` entry in the gate script saying why no snippet can express its
subject (the package-contract family, whose subject is a third-party artifact;
`execution-map-incomplete`, unreachable from real source by construction; the
server-surface and SSR rules, which need a rendering mode rather than a type; and
`v1/jsx-uses-vars`, which has no diagnostic of its own). That is what turns the
absolute rule from documentation into a mechanism: a new rule cannot be merged
without its positive spelling and the oracle's verdict on it. Verified by
deleting a case and watching the gate name the rule.

**Why this was invisible for a full cycle.** Every fixture stubs Solid with a
reduced `solid-js.d.ts`, and two of those stubs were *looser* than the real
package in exactly the place a rule's proof depended on:

| Stub said | Real package says |
| --- | --- |
| `apply: (value: T) => unknown` | `EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) \| void` |
| `onSettled(callback: () => unknown)` | `onSettled(callback: () => void \| (() => void))` |
| `createTrackedEffect(callback: () => unknown)` | `createTrackedEffect(compute: () => void \| (() => void), options?)` |
| `refresh(target: unknown)` | `refresh<T>(target: Refreshable<T>)`, where `Refreshable<T> = T & { readonly [$REFRESH]: any }` |
| `affects(target: unknown, key?: PropertyKey)` | `affects(target: Accessor<unknown> \| Store<object>)` / `affects<T extends object>(target: Store<T>, key: keyof T)` |

Each loosening manufactured a defect no real project can produce, and every
gate stayed green while the rule duplicated `tsc`. The proof-bearing
signatures are now byte-faithful in the fixtures that exercise them
(`solid2-precision`, `leaf-owner`, `execution-phases`, `eslint-plugin-corpus`,
`engine/eslint-reactivity-v2`, `package-callback-producer`); where a stub stays
deliberately loose (`static-api`, `static-api-unresolved`, whose subject *is*
the malformed call) the stub now says so in a comment naming the real signature
and asserting that no surviving rule's proof depends on the looseness.

### The general mechanism: TypeScript does not check hyphenated JSX attribute names

Found on 2026-08-17 by the predecessor span audit and now pinned in the
product-owned TypeScript cases; it is the boundary of every "this attribute is
TypeScript's" argument above.

TypeScript exempts a JSX attribute whose **name contains a hyphen** from the
excess-property check entirely — a deliberate allowance for HTML's own hyphenated
custom attributes. Verified against `solid-js@1.9.14`: `data-x`, `my-prop`,
`on-foo`, `html-For`, and the namespaced `class:mt-10` are all accepted on a
`<div>`, while `myProp` is TS2322. The *duplicate-name* check (TS17001) is
syntactic and is **not** exempt, so it still fires on `on-foo` written twice.

Three of the narrowings above were written per element rather than per name and
lost findings to this. All three now ask
`upstream_compat::jsx_name_is_type_checked` before staying silent:

- **SC8012** — `<div class:mt-10={true} />` and its shorthand are upstream's own
  cases 04 and 05. They were declared `status: "policy"` on the grounds that
  TypeScript reports them; it does not. Restored, and the two deviations removed.
- **SC8001** — `<div onFoo-bar="a" />` has an alphabetic third character, so the
  rule looks at it, and the name is never type-checked. Its static-value and
  ambiguous-name arms are restored for any hyphen-bearing name.
- **SC1005** — `<div data-count={count} />` is the one native-attribute value
  position that survives: the accessor is stringified into the attribute and no
  type objects.

All four shapes are pinned in `fixtures/reactive-ir/eslint-compat` and in the
oracle gate's `silent` cases.

### Removed: eight rules, 72 findings

Every one is a **violation the type system already reports on the same code**,
or an **obligation whose whole domain the type system closes**. The first seven
were 2.0-catalog rules; the eighth, `v1/imports`, was a 1.x rule. Their former
upstream cases are permanently reconciled in `migration-ledger.json`.

| Code | Rule | Findings | Why |
| --- | --- | --- | --- |
| SC3004 | `invalid-cleanup-return` | 29 | Every spelling is TS2345/TS2322 against `EffectFunction`'s `(() => void) \| void` return |
| SC9002 | `cleanup-return-unresolved` | 18 | Its whole domain was the *legality* of a returned value, which the same type closes |
| SC7003 | `invalid-refresh-target` | 6 | `Refreshable<T>` is the source brand as a type; every invalid target is TS2345 |
| SC7003 | `invalid-affects-target` | 2 | Same, against `Accessor<unknown> \| Store<object>` |
| SC7004 | `affects-keys-on-accessor` | 2 | A key on an accessor target selects the one-argument overload; the key is TS2345 |
| SC9003 | `refresh-target-unresolved` | 3 | Asked whether the target carries the brand — a question the type answers |
| SC9003 | `affects-target-unresolved` | 3 | Same |
| SC8002 | `v1/imports` | 9 | Its one condition — the named module does not export the name — is exactly TS2305's; audited later, see below |

#### SC3004 `invalid-cleanup-return`

`tsc --noEmit`, real `2.0.0-rc.0` typings, **both** passes (so no `strict`
argument is available):

~~~
sc3004.tsx(5,29) TS2345: Argument of type '(value: number) => number' is not assignable to
  parameter of type 'EffectFunction<number, number> | EffectBundle<number, number>'.
    Type 'number' is not assignable to type 'void | (() => void)'.
sc3004.tsx(6,29) TS2345: ... (explicit `return value + 1`)
sc3004.tsx(7,29) TS2345: ... (`() => makeCount()`, a returned call)
sc3004.tsx(8,29) TS2322: ... (`() => teardown.count`, a member return)
~~~

The legal spellings — `() => teardown.dispose`, `() => () => {}`,
`() => undefined`, `() => {}` — are **accepted**. The type does not merely
reject more than the rule did; it draws the same line. `(() => void) | void` is
a union, not bare `void`, so return-value-ignoring assignability does not apply.

#### SC9002 `cleanup-return-unresolved`

The obligation had four sources. Three are TypeScript's, and the fourth is not
a defect:

- **mixed union / `unknown`** — TS2345 and TS2322 in both passes.
- **a non-callback second argument** (`undefined`, `null`, `5`, `"apply"`, and
  1.x-style value threading) — TS2345 in both passes.
- **an unconstrained generic return** — TS2345.
- **`any`, and an unresolved wrapper callback** — `tsc` is silent, and that is
  not licence to report. Absence of a type error because the type is `any` is
  *missing evidence*, not proof (AGENTS.md's own trap list). More decisively:
  when the program type-checks, TypeScript has *proven* the returned value
  legal, so an obligation asserting uncertainty about its legality is noise
  about code the type system has cleared. The ownership consumer needs no
  finding for this — it simply does not get a "cleanup was handed over" fact,
  which is correctly modeled as an absent proof.

One of those unresolved-callback obligations was worse than noise: three of the
18 sat on `createEffect(compute, { effect, error })`, which is the **supported
`EffectBundle` form**. `tsc` accepts it; the checker raised an obligation on
idiomatic code. `rule_quality_process.rs` pins all of these at 0 so a
reintroduction fails there.

#### SC7003 / SC7004 / SC9003 — the refresh and affects target family

This family was hidden by the same mechanism, one layer deeper: the fixtures
type `refresh(target: unknown)`, while `@solidjs/signals` brands its
refreshable sources in the type system —

~~~ts
export type Refreshable<T> = T & { readonly [$REFRESH]: any };
export declare function refresh<T>(target: Refreshable<T>): void;
export declare function affects(target: Accessor<unknown> | Store<object>): void;
export declare function affects<T extends object>(target: Store<T>, key: keyof T): void;
~~~

so *every* shape the rules proved invalid is a type error, in both passes:

~~~
p3.tsx(11,11) TS2345: '() => number' is not assignable to 'Refreshable<() => number>'.
                        Type '() => number' is not assignable to '{ readonly [$REFRESH]: any; }'.
p3.tsx(12,11) TS2345: '{}' ... Property '[$REFRESH]' is missing
p3.tsx(13,11) TS2345: a plain accessor `target` ... '[$REFRESH]' is missing
p3.tsx(14,11) TS2345: `state.user`, a store child record ... '[$REFRESH]' is missing
p3.tsx(15,11) TS2345: `affects(signalGet())` — 'number' is not assignable to
                        'object | Accessor<unknown>'
p4.tsx(14,11) TS2345: `refresh(valueFormStore)` — only the derived forms are branded
p4.tsx(6,17)  TS2345: `affects(memo, "name")` — '"name"' is not assignable to 'unique symbol'
~~~

And the valid targets — `refresh(memo)`, `refresh(signalGet)`, `affects(memo)`,
`affects(state)`, `affects(state, "user")`, `affects(state.user, "name")` — all
type-check. Same line, both directions. Zero-argument and over-long calls are
TS2554 by arity.

**What was kept.** `static_api.rs` still records the `refresh(...)` *write* that
SC2001 `reactive-write-in-owned-scope` consumes; only the target diagnostics
went, and the control flow that skips a malformed call still skips it, so a call
`tsc` rejects records no write. SC2001 is unchanged at 36 findings.

**Do not amputate the runtime half.** The same applies to cleanup: SC3004's
consumer is gone, but `cleanup.rs::function_returns_cleanup` and the
`CleanupReturnStatus` classification behind it are load-bearing for SC4002 and
SC4004, which assert *ownership and disposal* — facts no type expresses. The
`callResultDomain` and member-return work still serves them; `diagnostics_process.rs`
pins that a returned call producing a `number` hands over no cleanup while one
producing a function does.

### Correction 2026-08-17: three rules were mis-classified as fully redundant

An earlier pass of this ledger listed `v1/event-handlers`, `v1/no-react-specific-props`,
and `v1/style-prop` as **proven redundant, removal specified**. That was wrong,
and the mistake is worth recording because it is the mirror image of the
fixture-stub trap: each was probed on *one* arm — an unknown attribute name on
an intrinsic element — and the verdict was generalised to the whole rule. Read
against each rule's complete former upstream domain (now reconciled in
`fixtures/ownership-cases/migration-ledger.json`),
all three have an arm TypeScript does not cover, so all three are **partially
redundant** and belong in the table below. None was deleted.

What the full probe found (real `1.9.14` typings, both passes):

| Spelling | `tsc` | Whose claim |
| --- | --- | --- |
| `<div onclick={fn} />` | silent — `onclick` *is* a declared prop | SC8001's canonical-casing advice |
| `<div ondblclick={fn} />`, `<div onDblClick={fn} />` | silent — both declared | SC8001's ambiguous-name advice |
| `<div {...{ onClick: fn }} />` | silent | SC8001's `warnOnSpread` arm |
| `<div onClIcK={fn} />`, `<div oncLICK={fn} />`, `<div onDoubleClick={fn} />`, `<div ondoubleclick={fn} />`, `<div only={fn} />`, `<div onLy="s" />` | TS2322 "Property does not exist" | TypeScript's |
| `<Pascal className="x" />`, `<Pascal htmlFor="x" />`, `<Pascal key={1} />` with permissive props | silent | SC8011 — and upstream's cases 4, 8, 9 are exactly these |
| `<Strict className="x" />` where the component declares `{ class?: string }` | TS2322 | TypeScript's |
| `<div className="x" />`, `<div htmlFor="x" />`, `<div key={1} />` | TS2322 | TypeScript's |
| `<div style="font-size: 10px; missing-value: ;" />` and every other string-valued `style` | silent — string styles are legal in 1.x | SC8017's string arms, including the malformed-CSS claim |
| `<div style={{ "-webkitAlignContent": "center" }} />` | silent — the `` [key: `-${string}`] `` index signature absorbs it | SC8017 |
| `<div style={{ fontSize: 10 }} />`, `{{ COLOR: "x" }}`, `{{ unknownStyleProp: "x" }}` | TS2561/TS2353 | TypeScript's |
| `<div style={{ "margin-top": -10 }} />` | TS2322 against `MarginTop<…>` | TypeScript's |
| `<div css={{ … }} />` (a configured extra style prop) | TS2322 | TypeScript's |

The narrowing each needs is the same question — *is this attribute name declared
on this element's attribute type* — which is a type fact the checker does not
have. The implementable approximation, per rule, is in the table below.

### Narrowed 2026-08-17: five rules, partially redundant, now scoped

Each keeps the arm no type answers and drops the arm TypeScript already
reports. Every one is pinned in **both** directions by
`fixtures/tsc-oracle/rule-cases.json` — a `removed-because-redundant` case for
the dropped arm and a `silent` case for the surviving one — so neither half can
move without failing CI. Each retained spelling is now a direct positive or
negative case in `fixtures/ownership-cases/cases.json`.

| Code | Rule | Dropped, and why | Kept, and why |
| --- | --- | --- | --- |
| SC8011 | `v1/no-react-specific-props` | `className`/`htmlFor`/`key` on an **intrinsic** element — TS2322 each. The `key` arm was intrinsic-only and went entirely. | The same spellings on a **component**, upstream's own cases 04 and 08. A component's props are whatever it declares, so the key is permitted on a permissive one and a type error on `{ class?: string }` — the answer genuinely depends on the component. |
| SC8017 | `v1/style-prop` | The object-key arms on an intrinsic element: camelCase (TS2561 with the kebab suggestion), an unknown key (TS2353), a unitless number for a length (TS2322 against `MarginTop<…>`), and a configured extra style prop (TS2322 on the attribute). | Every **string-valued** `style`, legal in 1.x, including the two claims no type can make — a declaration with a missing value, and a value that is not CSS. Plus any `-`-prefixed key on any element: `` [key: `-${string}`] `` absorbs it, so `-webkitAlignContent` is silent (upstream's case 02). Plus any key on a component. |
| SC8001 | `v1/event-handlers` | Every **type-checked unknown** `on*` name in every value form including the boolean shorthand (TS2322), and every mis-cased or non-standard spelling — `onClIcK`, `oncLICK`, `onDoubleClick`, `ondoubleclick` are not declared under any casing. Also the static-value arm on a standard declared handler: no static value is assignable to `EventHandlerUnion`. | The readability rename for a **declared** spelling: 1.x declares each handler as both `onClick` and `onclick`, so `onclick` and `ondblclick` type-check (upstream's cases 02 and 12). Every arm on a **hyphenated tag**: `<my-widget />` is TS2339 against stock typings, so a project using one declared it itself, commonly permissively. Hyphenated attribute names such as `on-foo`, which TypeScript deliberately skips but the compiler still lowers. And `warnOnSpread`, which type-checks while Solid does not attach the handler. |
| SC8003 | `v1/jsx-no-duplicate-props` | **Identically spelled** duplicates, by origin pair: two attributes are TS17001, an attribute then a spread is TS2783 (`strict` pass only, which the rule does not accept as an exception), two keys in one spread object are TS1117. | Two **differently spelled** props the compiler folds into one slot — `onClick`/`onclick` both become the delegated `el.$$click` write, `attr:title`/`title` share the template attribute slot. Plus the two identical-name orders TypeScript leaves alone: a spread then an attribute (upstream's case 02) and two different spread objects. Plus every child-content conflict — no type relates `children`, JSX children, `innerHTML`, and `textContent`. |
| SC8012 | `v1/no-unknown-namespaces` | Every namespaced prop on an **intrinsic** element — TS2322. Solid resolves namespaces through mapped types over user-augmentable interfaces plus individually declared `on:*` events, so an unrecognised prefix has nothing to land on. This covered the `style:`/`class:` steer too: **neither prefix is declared at all**, a real gap in Solid's published typings given the 1.x compiler supports both. | The same on a **component**, upstream's cases 06 and 07. Props are a plain object, TypeScript is silent, and the claim — the compiler special-cases namespaces only on DOM elements it lowers directly, so the prop arrives inert — is one no type makes. |
| SC1007 | `expected-function-got-expression` | The **call-result** arm on a normal declared handler. Both its triggers land on TS2322 at the same attribute: an expression *proven non-callable* is exactly what TypeScript rejects, and a *proven-accessor call* is rejected whenever the accessor's value is not callable (`onClick={count()}` with `count: Accessor<number>`). Deliberately **not** kept for the one spelling TypeScript permits — an accessor holding a function, `onClick={handler()}` — because there the finding would be wrong: a JSX attribute expression is a tracked read, so that handler does update. | The **reactive-handler-read** arm: a callable handler read out of reactive props or store state. TypeScript is silent, and the claim is a timing one — a native listener receives its function value once during DOM setup, so reading it through reactive props freezes the initial handler. Also the hyphenated native `on*` arm TypeScript deliberately declines to check: proven non-callable/non-array values are violations and mixed runtime shapes are uncertifiable. |
| SC1005 | `uncalled-accessor` (both catalogs) | Three of its six value positions, in both dialects: a native JSX attribute (TS2322 — an accessor is never assignable to a DOM attribute type), a class object value (TS2322 against 2.0's `Record<string, boolean>`), a computed property access (TS2538). This removed the last consumers of the dialect's `class_object_values_are_truthiness_coerced` and `native_children_attribute_invokes_functions` predicates, which went with them. | The positions TypeScript **permits**, and the most common real spellings of the bug: a string-concatenation binary operand (`"hello " + label` renders the accessor's source text), a unary operand (logical-not and the numeric coercions `-`/`+`/`~`, all clean against the published typings), and a template-literal interpolation. |

### Narrowed 2026-08-17: `no-direct-mutation`, in the 2.0 catalog only

2.0's `createStore` returns a shallowly `Readonly` proxy, so a write to a
**root** record property is already a type error against
`@solidjs/signals@2.0.0-rc.0`, for both spellings:

~~~
mut.tsx(4,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count = 1
mut.tsx(5,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count++
~~~

**Solid 1.x is the opposite**, and this is why the predicate is asked of the
dialect rather than assumed: its `createStore` returns a *mutable* store type,
and the same four writes produce **no diagnostic at all** against
`solid-js@1.9.14`. The 1.x rule is fully independent and untouched — carrying the
2.0 answer across the seam would have silenced it wrongly, which is exactly the
failure AGENTS.md warns about.

Three shapes survive in 2.0, each a write TypeScript accepts and the runtime
drops:

- **A nested record's property.** The readonly-ness stops at the top level, so
  `profile.user.name = "Grace"` type-checks.
- **A cast.** `(profile as { count: number }).count = 1` erases the readonly.
  This one constrains the implementation: `member_root` resolves *through* the
  cast, so comparing the written member's object against the resolved root span
  alone would have handed this case to a diagnostic that does not exist. The
  narrowing therefore requires the object to be a bare **identifier**.
- **A props member**, which is not readonly at all.

Coverage 526 → 524; the three root writes went and two of the surviving shapes
were added as fixture cases, because the fixture had none. The finding *count*
happens to land on four either way, so `diagnostics_process.rs` asserts each
surviving spelling and each dropped one **by span** — a count alone cannot tell
this narrowing from a regression that dropped the wrong three.

### Closed 2026-08-18: SC1005 no longer overlaps arithmetic diagnostics

The structural fact now retains binary `+` expressions with a string literal on
one side, unary logical-not, and the unary numeric coercions. Numeric and
bitwise **binary** operators reject a function operand in TypeScript — `count +
1` is TS2365, `count - 1`, `count * 2`, and `count | 0` are TS2362 — and are no
longer SC1005 positions, which removes the former `count + 1` duplicate.
Concatenations whose string behavior would require a resolved operator
signature remain outside the violation claim rather than being guessed from
source text.

The **unary** operators were dropped in the same pass on the assumption that
they behave like their binary counterparts. They do not: probed against
solid-js@1.9.14 through `scripts/tsc-oracle.mjs`, `-f`, `+f`, and `~f` on a
function value are clean in *both* the strict and loose passes, exactly like
`!f`. Dropping them removed a real, TypeScript-silent defect class (a coerced
accessor is silently `NaN`) and lost upstream parity case
`reactivity__invalid__21`, whose deviation could not be declared
TypeScript-owned because the span audit proved TypeScript reports nothing
there. They are restored under
`CoerciveOperandKind::NumericCoercion`, pinned by a `expect: "silent"` /
`checker: "reports"` oracle case and by
`fixtures/reactive-ir/uncalled-accessor-v2`, whose `TypeScriptOwnedOperators`
case now holds only the binary spellings.

### Independent — keep

Grouped by why no type can express the claim. `tsc` was confirmed silent on a
positive case for each entry marked ✓; the rest assert a runtime, timing, or
provenance fact with no type surface at all.

- **Reactivity and timing** — SC1001 `strict-read-untracked`, SC1002
  `reactive-read-after-await` ✓, SC1006 `untracked-derived-function`, SC9011
  `reactive-source-uncaptured`, SC5004 `v1/no-async-tracked-scope` ✓
  (`createMemo(async …)` type-checks: 1.x `EffectFunction` returns `Next`
  freely and 2.0's `ComputeFunction` admits `PromiseLike`), SC5001/SC5002/SC5003/SC5005
  (the pending-async and Loading-boundary family). *When* a read happens
  relative to a tracking scope is not a property of its type.
- **Ownership and disposal** — SC4001 `no-owner-effect`, SC4002
  `no-owner-cleanup` ✓ (`onCleanup(() => {})` is a well-typed `Disposable`),
  SC4003 `no-owner-boundary`, SC4004 `no-owner-settled-cleanup` ✓ (returning a
  real cleanup from an unowned `onSettled` is perfectly typed; the claim is that
  nothing will dispose it), SC3001/SC3002/SC3003 (the leaf-scope rules).
- **Write phase and transactions** — SC2001 `reactive-write-in-owned-scope`,
  SC2002 `action-called-in-owned-scope`, SC2003 `no-direct-mutation`, SC2004
  `resolve-in-reactive-scope`. Which scope is active at a call is not typed.
- **Compiler lowering** — SC8008 `v1/no-innerhtml` ✓ (`innerHTML` is a declared
  prop and a string is its declared type), SC8004 `v1/jsx-no-script-url` ✓
  (`href` is `string`; the claim is about the scheme the string carries), and
  SC8007 `v1/no-array-handlers` ✓ — the case that
  proves the JSX family is not uniformly redundant: `EventHandlerUnion` includes
  `BoundEventHandler`, so `onClick={[handler, 1]}` is **legal** per Solid's own
  types.
- **API shape that survives its own signature** — SC7001
  `missing-effect-function` ✓: the single-argument `createEffect(compute)`
  overload still exists in rc.0, deprecated and typed `never`, so the call
  type-checks and the claim "this effect never runs" is the checker's alone.
  Cast-hidden non-callable values survive in both dialects as well, including
  `.ts` angle-bracket assertions and a cast-hidden non-callable `effect` field
  in the required `{ effect, error }` bundle. Raw invalid arguments, including
  nullish values accepted only with `strictNullChecks` disabled, are excluded
  because the strict published-type pass reports them.
  Missing facts now remain explicit rather than becoming silent gaps. Exact
  required-only tuple length can prove that a spread-expanded Solid 2 call has
  no apply slot; the one-element case is therefore a violation and is proven
  not to allocate an owner-bound computation. A tuple slot hidden inside a
  spread still has no value fact, while optional/rest/array/unequal-union
  shapes have no exact length, so those paths remain uncertifiable. Unknown or
  `any` callback values are also uncertifiable, as is a nullable callback hidden by the
  runtime-transparent `!` wrapper, while compiler-proven callable identifiers retain a proven
  violation path. A `"use server"`
  directive is a framework and bundler convention that no core package reads;
  both published server entries neutralise client-runtime claims (1.9.14 uses a
  no-op; 2.0.0-rc.0 routes through `serverEffect`). Otherwise-reporting effect
  and ownership cases under the directive are therefore uncertifiable until a
  project/compiler fact proves which entry executes. Oracle cases pin both the
  uncertain directive path and undirected controls.
  SC7002 `sync-node-received-async`, SC7005 `http-response-after-flush`,
  SC7006/SC7007 (the server surface) likewise assert runtime behavior. SC5005
  distinguishes a visible server-render entry from an absent one. SC7005 is
  now always uncertifiable: even with SSR proven, source facts cannot decide
  whether a boundary settles before or after the shell flush. This
  2026-08-20 kind change means SC7005 now fails `--certify`.
- **Syntax and style, no type surface** — SC1003 `v1/no-destructure` ✓ /
  `component-props-destructure`, SC1004 `v1/components-return-once` /
  `component-returns-conditionally`, SC8002 `v1/imports`, SC8006
  `v1/jsx-uses-vars`, SC8009 `v1/no-proxy-apis` ✓ (a legal import; the claim is
  target-runtime Proxy support; explicit type imports are proven erased and
  runtime-referenced imports are proven executing, while unused value imports
  are uncertifiable because `verbatimModuleSyntax` changes their emit; direct
  Proxy calls require the exact standard-library declaration; and `mergeProps`
  reports a violation only for a proven function source, certifies only exact
  plain literals, and keeps every possible callable/`$PROXY` source
  uncertifiable without identifier-name heuristics), SC8010
  `v1/no-react-deps` ✓
  (`createEffect(fn, [dep])` type-checks — the array is 1.x's `Init` value),
  SC8013 `v1/prefer-classlist`, SC8014 `v1/prefer-for`, SC8015
  `v1/prefer-show`, SC8016 `v1/self-closing-comp` ✓, SC8018
  `prefer-component-syntax`, SC6001 `primitive-in-directive-application`.
- **Provenance and contracts** — SC9001, SC9005, SC9006 (the package-contract
  family). A missing contract is a statement about analyzability, not about a
  type.
- **SC8005 `v1/jsx-no-undef`** — kept, with a caveat worth recording. Its
  surviving domain is an unknown `use:` name (unresolved JSX tags are
  TypeScript-owned and remain checker-silent). Against the published typings *alone*,
  `use:autofocus` is TS2322, because `JSX.Directives` ships empty and is meant
  to be augmented. In a project that has augmented it — the documented, intended
  usage — `tsc` is silent, and the checker's claim (no lexical *value* binding
  exists for that name) is a different question from whether the *type* was
  declared. Independent, but a narrowing candidate if the two ever collapse.

### Fixed 2026-08-17: `solid-1x-sources` had been running the 2.0 dialect

The documented `.gitignore` trap, live in the repository.
`fixtures/reactive-ir/solid-1x-sources/node_modules/solid-js/` existed as an
**empty directory** — no `package.json`, no `.gitignore` exception, nothing
tracked — so dialect selection found no 1.x version and fell back to the 2.0
default. The fixture whose entire stated purpose is "the reactive-source
factories 1.x has and 2.0 does not" had never exercised the 1.x catalog.

What it was actually asserting: six `package-contract-export-missing`
obligations, because `createComputed`, `createDeferred`, `createSelector`, and
`createResource` are not in the 2.0 contract; plus a spurious
`missing-effect-function` and `no-owner-effect` on 1.x's single-argument
`createEffect`; plus one of the 18 SC9002 obligations the cleanup-return removal
dropped, which was this artifact rather than a real obligation.

What it asserts now: thirteen findings, every one a 1.x source factory's
untracked read — `createSignal`, `createMemo`, `createResource`,
`createDeferred`, `createSelector`, `createMutable`, `For`, `Index` — exactly the
"evidence that the source was discovered at all" its comment claims, plus
`v1/no-proxy-apis` on the store import, `v1/no-async-tracked-scope`, and
`v1/reactive-read-after-await`.

The stub and its `.gitignore` exception lines are now tracked together, which is
the only form of this fix that survives CI.

### Withdrawn: `import-location` is not a fixture defect

An earlier pass of this ledger recorded
`fixtures/reactive-ir/import-location`'s `import { createSignal, createMemo }
from "solid-js/store"` as a defect, on the grounds that it is TS2305 and no real
project compiles it. That reading was wrong: importing a name from the wrong
module **is** `v1/imports`'s subject, so the case is deliberate and correct.

It did raise a live question, and auditing it removed an eighth rule.
`v1/imports` (SC8002) fired on exactly one condition — the module named in the
import does not export the name — which is exactly TS2305's condition:

~~~
imp.tsx(1,10) TS2305: Module '"solid-js/web"' has no exported member 'createEffect'.
imp.tsx(2,10) TS2305: Module '"solid-js"' has no exported member 'render'.
imp.tsx(3,15) TS2305: Module '"solid-js/store"' has no exported member 'Component'.
~~~

Both passes, and value and type positions alike. The second arm I assumed it had
does not exist: a name exported by *both* modules returns early, so the style
preference upstream expresses for `import { Show } from "solid-js/web"` was never
reported here — verified silent, and pinned as such. Its module-rewrite autofix
was genuinely useful, and offering an autofix is explicitly not an exception.
**Removed**, 9 findings; `Dialect::export_modules` and the generated per-subpath
export index remain, still consumed by the contract layer.

## Compiler-faithful heuristics (verified against the 1.x compiler, do not "fix")

These were flagged as suspect eslint-plugin-solid ports and have now been
verified against the **pinned 1.x compiler**
(`solid-1x-compiler@79b9b637`, byte-faithful to
`babel-plugin-jsx-dom-expressions@0.40.7`) — the parity target is Solid's own
behavior, not upstream's quirks. Each entry below matches the compiler, which
is why it stays.

- **`on*` event-name detection** (`upstream_compat/shared_reactivity.rs`,
  `solid1x_attributes.rs`): the compiler's attribute lowering treats *every*
  `on`-prefixed DOM prop as an event (`plan.key.starts_with("on")`,
  `to_event_name` = the suffix lowercased), so `once`/`only` genuinely become
  listeners for events `ce`/`ly` when function-valued, and statically-valued
  ones are frozen into the template as plain attributes — exactly what
  `v1/event-handlers` reports. Upstream's `/^on[a-zA-Z]/` is *narrower* than
  the compiler; the checker deliberately uses the compiler's `startsWith`
  boundary instead, so a statically frozen `on-foo` receives SC8001 while a
  dynamic non-callable value is handled by SC1007's TypeScript-unchecked
  handler branch. A callable `on-foo` remains clean because it is a real
  listener for the distinct `-foo` event.
- **ASCII-only element-name case classification**
  (`upstream_compat/mod.rs::is_lowercase_led`): Babel's `isCompatTag` is
  `/^[a-z]/`, so a non-ASCII-led tag compiles as a component reference. The
  checker matches the compiler.
- **Static `innerHTML` without children is silent**
  (`no-innerhtml`, `allowStatic` default) and **single-line
  whitespace-only children block `self-closing-comp`** — configurable
  stylistic leniencies matching upstream's option defaults; neither can
  produce a false positive.

## Resolved: Solid 2.0 precision corrections 2026-08-17

**Read with the ledger above.** Where an entry below describes SC3004 or SC9002
as reporting something, that consumer is gone; the *classification* work it
describes survives because SC4002/SC4004 need it. The entries are kept as
written rather than rewritten, because they record how the runtime value domain
came to be trusted — which is still true — and rewriting them would erase the
reason the removal was safe.

- **SC7007 inline arguments and serializer identity** (`server_rules.rs`,
  `demand_plan.rs`): compiler library-type facts are now demanded for every
  non-spread server-function argument, so `save(new Date())` no longer falls
  through a variable-only gap. `configureServerFunctionsClient` must resolve
  to the exact `@solidjs/web/server-functions` declaration; a local same-named
  function with a `serializeArgs` property cannot silence the project. A valid
  exact configuration call with a dynamic options object produces an
  uncertifiable SC7007 until `serializeArgs` presence is closed. The remaining
  top-level fact boundary no longer creates silent nested false negatives.
  Alias-transparent primitive-domain facts now certify strings, booleans,
  null, and unions whose numeric constituents are all finite literals; domains
  containing only bigint, symbol, or undefined are proven violations. Broad
  numbers, mixed safe/unsafe primitive unions, object graphs, arrays, spreads,
  and missing facts remain uncertifiable. Invalid calls remain TypeScript-owned
  through an exact call-validity gate.

- **Synchronous standard callbacks after `await`** (`static_rules.rs` and
  `runtime_semantics.rs`): SC1002's accessor-call *and* member-read proofs now
  continue into a function written directly in an exact built-in
  `Array`/`ReadonlyArray.prototype.filter` call after a dominating await.
  Callability is sampled at the argument, not the callee, and the callback must
  be the literal argument for a proven SC1002 —
  `filter(makePredicate(fn))` and an `async` callback instead produce SC9012,
  preserving the unknown synchronous callback extent as an explicit proof
  obligation. Promise callbacks and project-defined or shadowed methods are
  outside this exact built-in behavior; unresolved package callbacks remain
  package-contract obligations.
- **Cleanup returns classified from the runtime value domain** (`cleanup.rs` and
  `demand_plan.rs`): identifier returns are demanded with TypeFacts'
  `runtimeValueDomain` and classified from it rather than from rendered type
  text, at exactly the peeled span the classifier resolves the entity at (so
  `return (value)` and `return value as Cleanup` work like the bare form).
  `CleanupReturnStatus` now separates "proven a function" from "proven legal but
  not a function", so a proven-`undefined` return can no longer make a callback
  look like one that returns a cleanup. Mixed legal domains, `unknown`, `any`,
  and generics are not legality findings (the removed SC9002 was TypeScript's
  job), but an unowned `onSettled` callback that may return a cleanup now keeps
  an uncertifiable SC4004 owner obligation instead of treating that cleanup as
  absent.
- **Static member cleanup returns** (`cleanup.rs` and `demand_plan.rs`): member
  return spans now receive the same exact `runtimeValueDomain` demand as
  identifier returns. A proven static function member is accepted as a
  cleanup and a proven primitive member was SC3004 (removed; now simply "not a
  cleanup"). A *mixed* union
  (`(() => void) | number`), `any`, and a computed member were SC9002 (removed),
  because their runtime property value is not closed by an exact dispatch
  proof. An **optional** member (`maybe?: () => void`) is legal but does not
  prove a cleanup on every execution; when owner safety depends on it, SC4004
  is uncertifiable. Verified against the pinned producer for all four
  spellings.
- **`runWithOwner` Owner identity** (`owners.rs` and `solid-dialect`): the
  supplied-owner proof now accepts only a compiler-resolved `Owner` type whose
  declaration and origin match the active dialect export table. Re-exported
  aliases are accepted; a user-local `Owner` lookalike and unresolved values
  remain conditional. This removes the rendered-type-name match without
  changing the nullable-owner fail-closed behavior.
- **Assignment target reads** (`solid-facts/src/ast` and
  `AstFacts::is_plain_assignment_target`): normalized facts distinguish plain
  assignment from compound/update reads, and only the member that *is* the
  written target is exempted, so a computed key or destructuring default inside
  a target stays an SC1001/SC1002 read.
- **Owner-backed settled cleanup** (`owners.rs`): the owner requirement pass now
  gates only the duplicate SC4002 for an inline owner-backed `onSettled` callback,
  and only when the callback is the literal argument; SC3001, genuinely unowned
  SC4002, and unowned returned-cleanup SC4004 remain distinct. Indirect,
  exported, and unresolved cases stay conservative.
- **The leaf pass requires an exact callback value and its synchronous extent**
  (`cleanup.rs`). The leaf-scope rules (SC3001/SC3002/SC3003) used to
  fire for a primitive written lexically *anywhere* inside the leaf-owner
  argument, so `onSettled(wrap(() => { onCleanup(fn) }))` reported SC3001 even
  though `wrap` may stash the callback and run it out-of-band, where no leaf
  scope exists and the call does not throw. The pass now demands the same two
  containment facts the dynamic-extent path already did: a literal or exact
  in-project callback exposes its body, and the call must sit in that
  callback's own synchronous extent (`direct_callback_contains`). An exact
  identifier callback and a closed local adapter with one unconditional
  function-literal or callback-parameter return are followed transitively, so
  forbidden operations keep their SC3xxx identity and an exact safe body is
  certified. Conditional, aliased, package, and otherwise opaque callbacks
  cannot support a specific violation claim and produce SC9012
  `uncertifiable` instead of silent failure. Known accessors, setters, actions,
  primitives, and exact standard-library calls discharge that walk, preventing
  false uncertainty on ordinary signal operations. The genuinely unowned
  SC4002 and the unowned returned-cleanup SC4004 are unaffected, as are the
  settled call-site gates. Pinned by `fixtures/reactive-ir/solid2-precision`
  and the closed/opaque pairs
  in `fixtures/reactive-ir/leaf-owner/`.

### Remaining approximations from this slice

- **Resolved 2026-08-17, returned calls are classified from the call result**
  (`cleanup.rs::returned_call_domain`, `demand_plan.rs`). The TypeFacts
  interface change this needed has landed: `callResultDomain`
  (`solid-ts-facts` `559c9031`, ADR 0013) matches a call-like node against the
  demand's exact start *and end* bytes and classifies the checker type there
  with the same runtime value-domain classifier, so the callee a call shares a
  start byte with can never be the subject. `cleanup_return_status` now feeds
  that domain to the existing `domain_cleanup_return_status`, which closes both
  directions the old callee probe produced: `return makeCount()` where
  `makeCount(): number` is SC3004 rather than silent (**FN** closed), and the
  unowned `onSettled(() => { return makeCount(); })` no longer reports SC4004
  as though a cleanup were handed over (**FP** closed). `handlers[i]()` is
  classified from its own signature rather than from a fact about `handlers`,
  which was the hazard that kept the value domain off call spans before.
  Both return spellings are covered: an expression-bodied arrow records its
  return on the function fact, so `returned_callees` now chains
  `functions[].expression_return` and `() => make()` is demanded exactly like
  `return make()`. Absent (no exact call-like node) and `unknown` (checker
  error or recovery type) remain fail-closed, as does a callee whose
  `resolvedCall` is not `Valid`. Pinned by
  `fixtures/reactive-ir/solid2-precision`'s `ReturnedCallCleanupReturns` and
  the two module-level `onSettled` returned-call cases.
  Across the corpus this discharged 30 SC9002 obligations and proved 13 SC3004
  returns — all of them a call producing a primitive where a cleanup is
  expected, such as `createEffect(() => 1, () => read())` and
  `createEffect(() => count(), () => untrack(() => count()))`.
  `callability` is no longer demanded at returned-call spans. Cleanup was its
  only consumer, and demanding it there is not merely dead: callability is read
  through `smallest_contained_callability`, which selects the smallest entity
  *contained* in the queried span, so a callability fact on an expression-bodied
  arrow's own returned call (`(post) => post.includes(id())`) sits inside the
  callback-argument span and outranks the arrow. That answered "is
  `post.includes(id())` callable" (no) where `inline_standard_callback` asked
  "is this argument a callable callback", which silently withdrew the
  `Array#filter` synchronous proof and with it SC1002 on the accessor read —
  visible only when the callback body *is* the returned call, since a binary
  body has no returned callee. The result domain is invisible to that lookup.
- **Evidence-backed divergence from upstream, `no_direct_mutation`**: with
  compound-assignment and update facts, the shared port now reports
  `store.count++` on a props/store member, which eslint-plugin-solid 0.14.5
  (commit `6d3bc311`) misses — its props branch tests for an ESTree
  `AssignmentExpression`, and `++` is an `UpdateExpression`. The compound form
  and an accessor binding's `++` are both parity-correct (upstream reports them
  via `AssignmentExpression` and `reference.isWrite()` respectively). No upstream
  case exercised any of these spellings; it is pinned by
  `fixtures/reactive-ir/v1-reactivity`'s `MutatesInPlace`.

## Resolved: false negatives closed 2026-08-16

- **Leaf-owner rules follow the dynamic extent through exact helpers**
  (`cleanup.rs::helper_forbidden_operations`). `onCleanup`/`flush`/primitive
  creation in a project function's *synchronous extent* (body minus nested
  function bodies) throws when the function is called from a leaf scope; the
  call site in the leaf callback is flagged, naming the helper
  (`LeafOwnerOperation::via`). Resolution is the exact TypeScript identity,
  transitive with a cycle guard. Remaining boundaries, deliberate: an
  unresolved/ambiguous/package callee contributes nothing (package behavior
  is the contract surface's), IIFEs inside a helper count as nested bodies
  (silent), and helper calls written inside nested functions within the leaf
  callback are not the leaf's synchronous extent (silent, correct).
  The leaf callback must be a **function literal, exact in-project function
  reference, or closed local callback return**. The last form is followed only
  when one exact local function unconditionally returns a function literal or
  its callback parameter; this proves the value received by the owner rather
  than treating the factory call itself as the callback. Conditional returns,
  local aliases, package calls, and opaque wrappers remain SC9012 obligations.
  `fixtures/reactive-ir/leaf-owner/` pins the `onCleanup`, `flush`, and
  primitive positives, the transitive hop, exact safe and defective
  references, both literal spellings, the nested-body and event-handler
  negatives, and the two closed local callback-return forms.
  Cost, accepted: the helper traversal is redone per call site rather than
  memoized by callee symbol. Depth is capped at 8 with a cycle guard and the
  walk only starts for a non-primitive call inside a leaf callback, so the
  fan-out is small; memoizing it is open work.
## Resolved: upstream quirks that contradicted the compiler

- **`on:`/`oncapture:` duplicate folding is gone** (2026-08-16,
  `solid1x_syntax.rs::duplicate_slot`). Upstream folds `onClick`/`onclick`/
  `on:click`/`oncapture:click` onto one name and reports runtime-legal pairs
  as duplicates. The compiler lowers `on:evt` to a bubble
  `addEventListener`, `oncapture:evt` to a capture `addEventListener`, and a
  non-delegated plain `on*` to one listener per occurrence — all attach, so
  none of those pairs is dead code. `v1/jsx-no-duplicate-props` now reports
  event-shaped names only for proven single-winner slots: the delegated
  `el.$$event = handler` property write (later-wins) and the statically
  valued template attribute (first-wins, shared with `attr:`). No upstream
  former upstream case pinned the folding; the product-owned cases now pin both
  directions directly;
  `fixtures/reactive-ir/eslint-compat` pins both directions.
  The slot model is **DOM lowering, so it applies to intrinsic elements
  only**. A component's props are a plain object the compiler never lowers:
  there the slot is the key as written, so `<MyComp onSave={a} onSave={b} />`
  and `<MyComp on:click={a} on:click={b} />` are real later-wins duplicates
  (the slot model would have silenced both), while `onClick`/`onclick` and
  `attr:title`/`title` are distinct keys.
  The static-value half is a *node-kind* test matching the compiler's inline
  branch (`StringLiteral`/`NumericLiteral`): `{0x10}` and `{1_000}` freeze,
  `{-1}`/`{+1}`/`{NaN}`/`{Infinity}` do not.

  `v1/event-handlers` (SC8001, `solid1x_attributes.rs`) now uses the same
  compiler node-kind predicate, so `{-1}`, `{+1}`, `{NaN}`, and `{Infinity}`
  are dynamic while radix and separator numeric literals remain static. The
  shared static-string resolver still covers upstream's proven string locals
  and literal concatenations. No former upstream case separated the two
  spellings; the focused fixture carries the regression instead.

  Adding the node-kind predicate was not sufficient on its own: a source-text
  arm (`text(span).parse::<f64>().is_ok() || static_string(..)`) survived in
  the same disjunction and decided the answer first, so `{-1}` and `{NaN}`
  still reported until it was removed. The diagnostic asserts Solid "will treat
  the value as an attribute", which is only true of the frozen forms, so the
  text arm was making a false claim rather than a conservative one. Pinned by
  `fixtures/reactive-ir/eslint-compat`'s `onClick={-1}`/`onClick={NaN}` pair
  (now clean) alongside the `onFoo="a"`/`onFoo="b"` static duplicates (still
  reported).

  **Closed 2026-08-18: non-frozen, non-callable handler values.** On a normal
  declared handler such as `onClick`, `onClick={-1}`, `onClick={NaN}`, and
  `onClick={someNumber}` are TS2322 and remain checker-silent. The uncovered
  case is a hyphenated attribute such as `on-event`, which TypeScript
  deliberately declines to check even though the compiler lowers every native
  `on` prefix as a listener. SC1007 now reads the exact runtime value domain and
  array shape there: a proven non-callable, non-array value is a violation; a
  callable/non-callable union, `any`, or an unresolved array/bound-pair shape is
  uncertifiable; a callable or absent handler is certified. Type assertions are
  peeled before classification. Real-typings oracle cases pin violation,
  uncertainty, and safe controls in both dialects.

## Audited remaining `TypeDescriptor.text` consumers 2026-08-17

**No consumer decides anything from `TypeDescriptor.text` any more** (verified
2026-08-18). Every remaining hit either labels a message or is a doc comment; the
two that made proof decisions were replaced by facts, below. The audit is kept
because it is what made the replacements findable:

- `interproc.rs` uses `text` only to label an unknown-callback diagnostic and
  generated contract stub; it does not make a proof decision.
- `solid1x_structure.rs` and the array branch of `solid1x_attributes.rs` asked
  a type-shape question (array/tuple versus a callable value) by matching
  descriptor text, because the Type Facts schema had no structural array-shape
  fact. **Resolved 2026-08-18** — see the `arrayShape` entry below.
- `shared_reactivity.rs` does not: its remaining `text` use is not a
  type-shape test.
- `server_rules.rs` asked whether a transport type has a rich serialization
  member (`Date`, `Map`, `Set`, typed arrays, and so on). **Resolved
  2026-08-18** — see the `libraryTypes` entry below. Its one remaining use of
  `text` quotes the author's type in the message; the decision never reads it.

Two consumers reintroduced a text decision while the SC1007 handler domain and
the SC7007 transport domain were being widened, and both are closed again:

- `shared_reactivity.rs::unchecked_handler_value_proof` certified an absent
  handler by matching `"null" | "false"`. `type Falsy = false` renders as
  `Falsy`, so the identical runtime value was a *proven violation* through an
  alias and silent as a literal. The runtime value domain cannot separate them
  — `null` and `false` both arrive as `may_be_other` — so the proof now comes
  from the AST: the literal written at the attribute, or the initializer of an
  immutable binding the reference resolves to.
- `server_rules.rs::argument_is_proven_json_safe` matched `"string" |
  "boolean" | "true" | "false" | "null"` for JSON safety, with the same alias
  asymmetry in the other direction — a spurious obligation on `type Name =
  string`. **Resolved 2026-08-21:** Type Facts' primitive value domain is
  structural and alias-transparent. Declared strings/booleans/null and
  safe unions now certify identically to literals; numeric domains certify
  only when every numeric constituent is a finite literal. No proof decision
  reads `TypeDescriptor.text`.

## Resolved: static attribute values are a fact, not a rendered type 2026-08-17

Bumping `typefacts` for the call-result domain also brought that revision's
node-selection change ("Classify complete demanded expressions"): a demand
resolves to the complete expression at its span rather than the deepest node at
its start byte. That is the correct subject, and it exposed a consumer
heuristic that had been right only by accident.

`upstream_compat::literal_string_type` recovered a static attribute string by
parsing `TypeDescriptor.text` for a rendered literal type, decoding JSON-style
escapes by hand. For `innerHTML={"a" + "b"}` the old selection typed the
leading `StringLiteral`, so a literal type appeared and the value read as
static; the complete `BinaryExpression` widens to `string`, so the same test
called a static value dynamic and `v1/no-innerhtml` reported it (upstream's own
case asserts it is valid under `allowStatic`).

The fix is the fact the migration guide asks for, not a repaired heuristic:
`constantValue` (`solid-ts-facts` `fc739a6c`, ADR 0014) is demanded at the
exact attribute-value span and accepted only as a present `kind: "string"`.
The producer folds literals, substitution-free templates, transparent
wrappers, unary signs, same-kind binary `+`, and compiler-resolved immutable
declarations (`const`, `readonly`, enum members), bounded by a depth limit and
a declaration cycle guard. Absence is "not proven constant", so a dynamic
value stays uncertifiable rather than guessed.

This is a precision *gain* in both directions, not only an FP fix:
`v1/jsx-no-script-url` now proves the scheme in
`href={"java" + "script:alert(1)"}`, which no literal type ever described, and
a `const`-referenced value is static wherever it was declared. Pinned by
`fixtures/reactive-ir/upstream-divergences`'s `FoldedMarkup` and `ScriptUrls`.
The later catalog reduction retired `jsx-no-script-url`; this paragraph records
the producer fact that remains useful to other semantic consumers.

Deliberately **not** folded into the producer: the *node-kind* tests. The 1.x
compiler inlines an attribute into the template on a `StringLiteral`/
`NumericLiteral` branch, so `jsx-no-duplicate-props` must keep asking what was
written rather than what it evaluates to — `{"a" + "b"}` is not inlined. The
`v1/event-handlers` inconsistency recorded above was that same syntactic
question and is now closed; see the note under the duplicate-props entry.

## Resolved: array/tuple shape is a fact, not a rendered type 2026-08-18

Two consumers decided "is this an array or a tuple?" by matching
`TypeDescriptor.text` against `[`, `readonly `, `Array<`, `ReadonlyArray<`, and a
trailing `[]`. Both were fail-closed, so both were false negatives, and text
could not settle the question in two independent ways:

- **An alias renders as its own name.** `type Handlers = [(data: number, event:
  MouseEvent) => void, number]` renders as `Handlers` and matched no prefix.
- **A trailing `[]` is ambiguous.** An array of functions (`((n) => void)[]`) and
  a function returning an array (`() => string[]`) render identically, which is
  why the screen also had to consult `callability` — and even then it could not
  see through the alias.

The fix is the fact, not a repaired heuristic: `arrayShape` (`solid-ts-facts`
`ce4c772`, ADR 0015) classifies the type at the exact demanded expression span
with the checker's own `isArrayOrTupleType` predicate over the real union
constituents. `array` requires every constituent to be an array or tuple;
`notArray` requires none to be, and is a positive claim so the negative is usable
as proof; `mixed` and `unknown` are proven states that prove neither side.
Absence stays fail-closed. `expression_descriptor` and `expression_callability`
had no other callers and were removed with the screen.

Closed false negatives, measured by A/B against the text screen on
`fixtures/reactive-ir/array-shape-v1`:

- `v1/no-array-handlers` now reports an aliased tuple, a doubly-aliased tuple,
  and a call returning one — all previously silent.
- `v1/prefer-for` now offers the `<For each>` autofix when the `.map` receiver is
  an *alias* for an array (`type Rows = string[]`). The alias hole had been
  withholding a correct rewrite, which is the same defect reaching a second rule.

### Narrowed in the same pass: `no-array-handlers`' `on:` arm

Writing the fixture against the real typings (`scripts/tsc-oracle.mjs`) showed
the fixture stub had been hiding a duplicate — the trap `fixtures/tsc-oracle`
exists for. `solid-js@1.9.14` types the two handler spellings differently:

~~~ts
type EventHandlerUnion<T, E, EHandler> = EHandler | BoundEventHandler<T, E, EHandler>;
interface BoundEventHandler<T, E, EHandler> { 0: (data: any, ...e: Parameters<EHandler>) => void; 1: any }

type EventHandlerWithOptionsUnion<T, E, EHandler> = EHandler | EventHandlerWithOptions<T, E, EHandler>;
interface EventHandlerWithOptions<T, E, EHandler> extends AddEventListenerOptions { handleEvent: EHandler }
~~~

`onEvent` accepts `BoundEventHandler`, an interface with members `0` and `1`, so
a `[handler, data]` tuple is legal and only this rule can object to it. `on:event`
has **no** bound arm, so every array and every tuple there is already `TS2322`:

~~~
Type '[(data: number, event: MouseEvent) => void, number]' is not assignable to
type 'EventHandlerWithOptionsUnion<HTMLDivElement, MouseEvent, ...>'
~~~

in both the strict and non-strict passes. The `on:` arm was removed under the
absolute rule; former upstream case `no-array-handlers__invalid__03` is recorded
as TypeScript-owned in the completed migration ledger.

### Closed 2026-08-18: the plain-array duplicate, via `tupleShape`

A **plain array** on `onEvent` has no `0`/`1` members either, so it was `TS2322`
too — confirmed for `((event: MouseEvent) => void)[]`, `ReadonlyArray<...>`,
`any[]`, and `unknown[]` — and the rule reported it anyway.

`arrayShape` could not settle it, by construction: it reports `array` for a plain
array and a tuple alike, because both of its consumers wanted the union of them.
The condition that is genuinely this rule's is not "array or tuple" but **"a
tuple with both numbered slots whose first can be called with `(data, event)`"**
— exactly what `BoundEventHandler` accepts and `tsc` therefore permits.

`tupleShape` (`solid-ts-facts` `b9d1a8e`, ADR 0016) supplies it: fixed slot count,
whether a rest tail follows, and the first slot's callability *and minimum
arity*, present only when the type at the exact span is itself a tuple. The rule
now fires on that and nothing else. `tsc` names each removed shape's reason
precisely, which is how the partition was checked:

~~~
((event: MouseEvent) => void)[]      missing the following properties: 0, 1
[number, number]                     Types of property '0' are incompatible
[(event: MouseEvent) => void]        Property '1' is missing
[1, 2, 3]                            Type 'number' is not assignable to (data: any, e) => void
[(a, b, c) => void, number]          Target signature provides too few arguments
~~~

The last row is the arity residual, closed by an amendment to ADR 0016.
`elementZero` says the slot is callable, which does not settle whether it can be
*invoked* with the two arguments Solid passes: `BoundEventHandler` types slot 0
as `(data: any, ...e: Parameters<EHandler>) => void`, and `EventHandler` takes
one parameter. A handler requiring three is callable, and not callable here.
Adding `elementZeroMinimumParameters` took the fixture's SC8007 count from 6 to 5.

Against the fixture, `handler-cases.tsx` now holds every SC8007 and produces
**zero** `tsc` diagnostics, while `clean-cases.tsx` produces eight and **zero**
findings. The rule and the type checker partition the space exactly.

**Contextual typing is what makes this work, and it is load-bearing.** An array
literal written where the expected type has numbered members acquires fixed
slots; the same literal in an unconstrained position stays a plain array. So
`tupleShape` absent *plus* an array literal written here means no bound-handler
type applies at this attribute — the project's JSX typings are not checking it,
and the rule is the only thing that can speak, which is the boundary
`jsx_name_is_type_checked` already draws. The syntactic fallback is kept for
exactly that case.

The consequence is that `fixtures/reactive-ir/array-shape-v1` had to stop using a
permissive `IntrinsicElements` index signature: its stub now carries the real
`EventHandlerUnion`/`BoundEventHandler` signatures, because a looser stub erases
the contextual typing the rule depends on and the fixture would stop exercising
its own path. The retired upstream corpus used a permissive harness, so three
former cases (`no-array-handlers__invalid__03`, `__05`, `__07`)
are recorded as TypeScript-owned in the completed migration ledger, each
verified against the real typings before the corpus was retired.

### Closed 2026-08-18: unions of tuples

A union had no `tupleShape` at all — the fact answered only for a type that was
itself a tuple — so `Handlers | OtherHandlers` and the very common optional
`Handlers | undefined` both failed closed. `arrayShape` reported `mixed` for the
optional form, which proves neither side, so the rule went silent on values that
are a bound pair whenever they are anything.

The fact now answers for a union with its constituents' **meet**: the slots they
all have, a rest tail only if all carry one, callable only if all are, and the
largest argument requirement among them. What it reports therefore holds
whichever constituent the value turns out to be. A single non-tuple constituent
voids the answer, and nullish constituents are skipped because they carry no
structure — presence stays `runtimeValueDomain`'s question. The payload and Wire
table schema are unchanged; this widened when the fact is emitted, not its shape.

`tsc` agrees on every boundary, which is how it was checked: `H1 | H2` and
`H1 | undefined` are silent (ours, now reported), while `H1 | number[]`,
`H1 | [number, number]`, and `H1 | [(a,b,c) => void, number]` are each TS2322
(the type checker's, still silent here).

### Closed 2026-08-18: mixed handler shapes and runtime presence

A union that mixes a bound pair with a **plain function** (`Handlers |
((e: MouseEvent) => void)`) has no `tupleShape`: one constituent is not a tuple,
so no violation holds for every runtime value. It no longer disappears. The
handler expression now also demands `runtimeValueDomain`; callable/non-callable
unions, `any`, and generic shapes produce an explicit **uncertifiable** SC8007.
An unresolved import remains silent because TS2307 owns that source.

The same audit found two adjacent proof errors:

- `Handlers | undefined` has a common tuple shape, but a tuple is not present on
  every execution. With `strictNullChecks` disabled TypeScript erases the
  undefined constituent before the runtime-domain fact is computed, and the IR
  does not receive that compiler option. A violation is therefore reserved for
  a structurally proven runtime array (an inline literal or immutable local
  array initializer); type-only pair evidence remains uncertifiable.
- A TypeScript assertion was treated as a safety voucher. JSX facts now record
  whether a wrapper is a runtime type escape, and the rule demands/classifies
  the peeled runtime expression. An asserted array is a violation, an asserted
  function is safe, and an unresolved runtime shape is uncertifiable.

The focused fixture now pins proven violations, proven-safe controls,
TypeScript-owned invalid tuples/arrays, and every uncertifiable branch. The
real-typings oracle carries strict and non-strict cases for a pair/function
union, an optional pair, an asserted array, and an asserted-function control.

### Closed 2026-08-18: `rich_transport_member`, via `libraryTypes`

`server_rules.rs` asked whether a server-function argument is one of a few
runtime types JSON cannot carry (`Date`, `Map`, `Set`, `RegExp`, a typed array).
It answered by splitting `TypeDescriptor.text` on top-level `|`/`&`, stripping a
`[]` suffix, and matching the head against a name list. It was the second
`TypeDescriptor.text` consumer named alongside `array_like_type`, and it was
deliberately left alone when `arrayShape` landed on the grounds that its question
was open-ended.

That framing was wrong, and worth recording. The open-ended question is "does
this object graph contain a non-JSON-safe member" — a recursive walk needing a
cycle guard, a depth bound, and a nesting policy. But that is not the question
this rule asks. It asks only about the **top level**, by deliberate design ("an
unproven rich type is not a proven throw"), and a bounded top-level question is
exactly the kind a fact can answer.

`libraryTypes` (`solid-ts-facts` `3d51c40`, ADR 0017) answers it: the sorted set
of standard-library type names the type at the exact span is built from at its
top level — itself, its union and intersection members, and one array-element
unwrap. A name is recorded only when the resolved symbol is declared in a
default-library file. The rule keeps its own list of which names matter, and that
a lone `Uint8Array` has a natural HTTP encoding; the fact carries no policy.

Three defects text could not avoid, all closed:

- **An alias renders as its own name.** `type Stamps = Date[]` matched nothing,
  declared locally or imported from another module. Measured on
  `fixtures/reactive-ir/server-function-rich-args`: the text walk found 6
  findings, the fact finds 8.
- **`Array<Date>` and `Date[]` are the same runtime value** and only the second
  matched.
- **A user-declared type could match a global's name.** `Map` from the project's
  own code is now excluded by its declaration file, not hoped away by spelling.

The fact's nesting boundary remains unchanged, but it no longer becomes a
checker blind spot: `Boxed = { title: string; when: Date }` produces an explicit
uncertifiable SC7007 because the complete object graph is not proven JSON-safe.
`tsc` cannot duplicate any of this — the argument's type matches its parameter's
type by construction, so no diagnostic is possible; the claim is entirely about
runtime transport.

## Closed 2026-08-18: unresolved generic member dispatch is explicit

Generic member calls now have three outcomes instead of a silent fail-closed
branch. One exact implementation contributes its summary; a finite set of
implementations contributes a summary only when every candidate has the same
reactive-read behavior; missing or divergent behavior produces SC9012
`reactive-dispatch-unresolved` as an `uncertifiable` finding.

The explicit obligation covers parameter-member substitution at each call
site, computed calls whose TypeScript call is valid, and direct member dispatch
on parameters of exported helpers with unseen callers. A compiler-proven
tracked JSX call is safe regardless of which implementation reads, and an
exact standard-library method is not open dispatch, so neither produces an
export obligation. Member calls nested in returned, assigned, or scheduler
callbacks are not mistaken for direct export behavior: the existing callback
execution contract owns those paths. These distinctions keep JSX collection
helpers and higher-order adapters certifiable while refusing a falsely empty
summary for a helper whose own execution directly depends on an unseen
implementation.

`fixtures/reactive-ir/interprocedural-methods-v2/` pins all three finite-set
outcomes: an exact reactive object is a proven SC1001 read, reactive candidates
with equivalent summaries remain a proven read, and reactive/inert candidates
produce SC9012. It also pins valid computed dispatch as SC9012 and a direct
export boundary. `fixtures/reactive-ir/v1-reactivity/` pins the same shared
obligation under the v1 identity. The real-typings oracle carries keystones for
both dialects and TS2349 negative controls; invalid calls remain TypeScript's
job and never receive SC9012.

## Closed 2026-08-22: package contracts preserve parameter-member reads

Schema version 1 now has the additive `reactiveReads` form
`{ "kind": "parameter-member", "parameter": N }`. The producer already knew
the exact parameter symbol behind a direct member receiver; it now exports that
provenance instead of refusing every JavaScript runtime artifact at the open
package boundary. Local and module-local receivers are unchanged and do not
become public effects.

Consumers instantiate the row per call site. Proven reactive store/path
arguments contribute a read, inline primitive/array literals are clean, and an
opaque argument is SC9012 rather than guessed plain. Local wrapper summaries preserve
the same parameter provenance. The package generator fixture
`fixtures/package-contracts/parameter-member-read/` pins the runtime-artifact
claim and its negative local-receiver controls;
`fixtures/reactive-ir/package-parameter-member-consumer/` pins the reactive,
plain, and uncertifiable consumer outcomes.

Argument-value/identity-dependent dispatch remains deliberately open. A
contract variant keyed by an arbitrary runtime argument would export the
callee's dispatch table, has no bounded exhaustiveness proof, and is not the
same thing as environment `variants` with ordered export-map conditions.
`solid-recharts`-style sentinel dispatch therefore remains correctly
uncertifiable. Parameter-attributed writes are also not claimed: their
operation and ownership semantics need a separate design rather than symmetry
by spelling.

## Closed 2026-08-22: legacy ESM roots reach contract generation

The package generator no longer requires `package.json#exports` when the
runtime artifact still has one exact legacy ESM root. It recognizes `module`,
an ESM-safe `main`, and an unambiguous ESM index fallback, all through the same
entrypoint-resolution module used before semantic analysis. The negative CJS
fixture pins that a conventional `main: index.js` without ESM package semantics
is refused rather than interpreted under the generator's TypeScript settings.
Missing, absolute, escaping, declaration-only, and CJS targets remain
unsupported. This changes package-shape reachability only; it does not add
trust or weaken any reactive proof obligation.

On the fixed 417-probe ecosystem manifest, 7 of the 11 former
`unsupported-package-shape` probes now generate reviewable drafts. Two more
reach semantic analysis and expose their real unresolved read/dispatch
obligations; one is correctly classified CJS-only, and one declares a missing
module artifact and remains no-ESM. Whole-corpus success moves from 336/417
(80.58%) to 343/417 (82.25%) with zero timeouts.

## Closed 2026-08-22: contracts distinguish proven none from unknown

Schema version 1 now accepts `{ "status": "unknown" }` in each existing
effect-claim field. Omission retains its previous reviewed meaning of proven
none. The marker occupies the existing field rather than a new sibling, so an
old loader rejects its incompatible type instead of ignoring a new property
and failing open. The Rust contract module normalizes both wire forms behind a
single `ContractClaim<T>` interface.

An exact exported callback obligation emits a partial reviewable draft with
`callbacks` unknown and keeps independently proven reads, returns, owner
requirements, and async behavior.
Consumers demand that uncertainty only when a call supplies a potentially
callable value; a no-argument call remains clean. Read, return, owner, and async
obligations now become unknown only in their affected domain. Exact containing
function identity keeps clean sibling exports intact; an import-level or
transitive obligation that cannot be joined to one export falls back to all
applicable function exports, which is conservative but permits a reviewable
entrypoint draft.

Callback rows also carry bounded `arguments` descriptors. A producer records a
fresh accessor passed to a callback parameter, and a consumer marks only the
matching callback-function parameter reactive. The
`callback-reactive-arguments` package fixture pins the producer behavior and
ensures the handoff itself is not reported as an uncaptured read.

`fixtures/reactive-ir/package-unknown-callback-consumer/` pins the demanded and
non-demanded consumer cases, and
`fixtures/reactive-ir/package-unknown-returns-consumer/` pins the other half:
a non-callback domain, which is opened where the claim enters the project
rather than where a call demands it, with a sibling export whose summary
withholds nothing staying clean.
`cli_reports_the_exact_unknown_claim_domain` holds the finding to naming the
one domain left unknown, since a summary that states four domains and
withholds one is not the same evidence as a summary that states nothing. The
existing unknown-callback producer process
fixtures pin partial emission, a known sibling callback summary, and cyclic
forwarding termination. Unresolved dispatch, unknown package identity, and
unreviewed evidence remain correctly uncertifiable at consumption even when
generation can write a partial draft.

The fixed 305-row/417-probe ecosystem manifest measures the result directly.
All 21 probes previously classified as `unresolved-parameter-behavior` now
generate contracts, so that class falls from 21 to zero. Across the complete
worktree (including the companion parameter-member and runtime-identity
slices and legacy ESM resolution), success rises from 291/417 (69.78%) to
343/417 (82.25%), a gain of 52 probes or 12.47 percentage points. The corrected full-run target uses the
documented 600-second budget and completed with zero timeouts; the generated
JSON and Markdown reports live under `benchmarks/ecosystem/`.

## Closed 2026-08-22: package generation reaches the artifact ceiling

The remaining semantic generation classes on the pinned ecosystem corpus are
now zero. The generator represents ordered conditional branches whose export
`kind` differs, recursively generates and caches exact installed dependency
contracts, scopes unresolved obligations to the affected export and claim
domain, carries accessor-valued callback arguments, and treats exact
standard-library declarations as platform behavior rather than an unresolved
package dispatch. None of these inferred drafts become reviewed evidence.

Generation projects now contain the exact static relative runtime-module
closure of an entrypoint instead of every JavaScript file below its distribution
directory. This keeps published `.js` barrels ahead of adjacent declarations
without repeatedly loading unrelated bundles. Return facts are also assigned to
their innermost summary and AST owners once, instead of rescanning every return
for every function. Returned-factory lookup likewise indexes exact binding,
factory, return-owner, symbol, and function-span relationships once per file,
instead of rescanning the bundle for every direct call. On the former
`@tanstack/ai-devtools-core` timeout, these changes reduce generation of both
entrypoints from 128.32 to 10.37 seconds in the debug-binary reproducer. Release
cold analysis falls from 4.03 seconds to 0.474 seconds; return-summary
attribution itself falls from 3.13 seconds to 34 milliseconds, and
interprocedural graph construction falls from 314 to 33 milliseconds after the
return-attribution optimization.

The measured result is **407/417 (97.60%)**, up from **343/417 (82.25%)**:
64 additional successful probes and 15.35 percentage points. The remaining ten
failures are six npm peer-resolution failures, three packages with no usable
ESM runtime artifact, and one CJS-only tsup bundle. There are zero timeouts and
zero semantic contract-generation failures. CJS stays fail-closed: the one
remaining bundle exposes generated `__export`/`__toCommonJS` machinery rather
than a statically auditable `module.exports` surface, and declarations are not
runtime proof. The exact report is `benchmarks/ecosystem/report.json` with the
human-readable companion `report.md`.

The full-corpus target now measures the optimized product binary rather than a
debug checker and schedules `min(available CPUs, 8)` probes concurrently. On
the same 417-probe manifest, wall time is **104.394 seconds**, down from
**238.502 seconds** for the four-worker debug run (56.23% less, 2.28x faster),
with the same 407 successes and ten artifact/install failures. Reports now
record installation and generation separately: this run spent 542.020 seconds
of aggregate worker time installing, 211.963 seconds generating, and 0.879
seconds in harness bookkeeping. The remaining full-run floor is therefore
primarily isolated npm resolution rather than semantic contract analysis.

**Superseded as the current figure, 2026-08-22.** The numbers above are the
measurement of that change on the then-current 417-probe manifest and stay as
history. The manifest is now 305 rows / 416 probes, and the regenerated
reports read **403 complete contracts, 6 partial, 7 failures** in 94.286 s —
not a regression from 407/417 but a stricter count, since `partial-success`
had not yet been split out when 407 was measured. See "The ecosystem benchmark
counted partial contracts as successes" below for the full old-versus-new
accounting.

## Closed 2026-08-22: contract review no longer certifies unobserved callbacks

Five defects in the package-contract slice shared one shape: generation
succeeded where it could only have said "unknown", and the review plan no
longer surfaced the negative claim that resulted. They are recorded together
because fixing any one of them alone leaves the same class open.

**Callbacks forwarded into an unresolvable callee.** A call whose callee had
neither a dispatch candidate nor a resolvable identity was dropped from the
graph entirely. That is `list.map(fn)` where `list` is one of the exported
function's own parameters -- `any` in every published JavaScript runtime
artifact, since the generation project deliberately keeps the runtime `.js`
ahead of its adjacent declarations. The forwarded callback escaped with no
recorded behavior, and an omitted `callbacks` field is a *negative* claim, so
silence certified "never invoked". `main` refused these packages outright; the
parameter-member slice replaced that refusal without covering the callback
path underneath it. Measured on the real registry:
`@solid-primitives/utils@6.3.2` claimed `map`, `filter`, and `sort` invoke
nothing and `tryOnCleanup` needs no owner, and
`@solid-primitives/event-listener@2.4.4` claimed `makeEventListener` never runs
its handler. A consumer of the promoted contract reported SC1001 on a signal
read inside a DOM click handler -- a proven violation asserted from a claim the
contract never had. Such calls now open the existing unknown-callback
obligation, scoped to arguments that are parameters of the enclosing exported
function and whose own syntax does not already prove them inert.

Two consumer-side halves follow from it. A literal argument is now proof of
non-callability in its own right (`slice(list, 0, 2)` demands nothing from an
unknown callback claim, where every argument previously did, because the type
system reports "potentially callable" whenever it has no type at all). And a
read inside a callback whose contract timing is unknown is no longer reported
as a proven untracked read: the call already carries an SC9005 obligation, and
claiming the timing on top of it asserts exactly what the contract says it does
not have.

**`default` branches were unmatchable.** Generation encodes an export map's
fallback as the literal condition `default`, but `selected_conditions()` never
produces that string, so `matches_conditions` could never satisfy it. Every
consumer with a real environment selected fell through to an
environment-dependent uncertifiable result -- including the one the fallback was
generated for. `default` is now satisfied by any selected environment and by no
selection at all, and `precedence` decides among several matching branches.
Handwritten contracts, which carry no `precedence`, resolve only the case that
needs no invented order: a named branch beats the unconditional fallback, while
two named branches stay fail-closed.

**`--conditions` erased the environment it was scoped to.** A contract
generated with an explicit selection recorded nothing about it, so a consumer
in any other environment applied it. The selection is an assertion about the
resolving environment rather than an observation of the export map, so a
branching entrypoint now carries it and a differing consumer fails closed. An
entrypoint with one unconditional target still records nothing.

**Conditional export-name absence was silently unconditional.** A name observed
in only some branches was published as a complete unconditional summary,
handing a consumer in the other environment a claim about an export that does
not exist there. The proven branches are now retained as `variants` even when
they agree, and normalization no longer collapses a variant set that fails to
cover its entrypoint's conditions.

**Legacy `module`/`main` provenance was invisible.** A legacy dual package's
contract describes only the analyzable ESM build. Refusing the package would
reject every legacy dual package, including the common case where `main` is the
CJS transpile of the same source, so the review plan now names the field the
root came from and says when `main` points elsewhere.

**The checklist section that would have caught all of this was removed.** The
"callbacks with no execution row" section is restored; `docs/package-contracts.md`
had continued to promise it.

Regression pins: `fixtures/package-contracts/unresolved-callee-callback`,
`conditional-export-absence`, and `legacy-dual-root` in the contract corpus;
`selected_variant` and `RuntimeEnvironment::matches_conditions` unit tests; a
review-plan test for legacy provenance; and
`fixtures/reactive-ir/package-variant-precedence-consumer`, which carries the
selection through to a consumer's proof. Its two exports declare the same two
overlapping branches and differ only in `precedence`: the unique lowest one
resolves the branch whose accessor return makes an untracked read provable,
and the tie leaves the import binding uncertifiable with the identical read
unreported. The unit tests alone could not distinguish a working selection
function from one whose answer never reached a consumer.

Remaining approximation, deliberately: an argument to a `parameter-member` read
whose origin the project cannot see -- a parameter, a prop, an import, a bare
`declare const` -- stays SC9012. A Solid store is a proxy typed as the object it
wraps, so no declared type proves the negative; only inline literal syntax or an
analyzed initializer with a standard-library origin does.

Generation reachability did not move, which is the expected result: a contract
carrying an explicit unknown marker is still a generated, reviewable draft. The
run that first measured these fixes read 407/417 on the manifest they were
reviewed against, matching the run before them exactly, with zero timeouts and
zero semantic contract-generation failures.

What moves is review surface, which is the point. On two pinned versions,
holding the package release constant so the comparison is not confounded by a
package update: `@solid-primitives/utils@6.3.2` goes from 23 to 73 checklist
items (43 of them the restored "callbacks with no execution row" section, and
unknown export claims rising from 2 to 9), and
`@solid-primitives/event-listener@2.4.4` from 12 to 14 (unknown export claims 3
to 7, including `makeEventListener`). A run that does not show that rise has
not applied these fixes.

The corpus denominator has since changed twice, so those two numbers are not
directly comparable to the current report -- see the entry below.

## Closed 2026-08-22: the ecosystem corpus measures a real environment

Three selection and reporting defects made benchmark numbers describe the
harness rather than the ecosystem. All three were found by reading the failure
list rather than the success rate, which is the general lesson: 407/417 was
stable across a run that shipped materially wrong contracts and a run that
fixed them.

**Solid 2 floors selected environments nobody supports.** The 2.x line spent a
long time in `experimental` and `beta`, so a package published this month can
still declare a range whose formal lower bound is an old beta while its own
dependencies have moved on. Flooring at that bound produced peer conflicts that
described nothing. The floor is now anchored at `2.0.0-rc.0`, and only ever
raised: a range accepting no `rc` keeps its own oldest accepted beta, the same
rule that keeps a beta-only package off a newer release candidate at the head.
`compatibleSolidVersions` still records the complete accepted set, so the range
fact is preserved and only the probe moves. Seventeen genuinely beta-only
probes remain in the corpus.

**Floor tuples were assembled per package and could not coexist.** Flooring
`solid-js`, `@solidjs/web`, and `@solidjs/signals` independently can synthesize
an environment that has never existed. `@tanstack/solid-router@2.0.0-rc.1` pins
`@solidjs/web@^2.0.0-rc.1`, and that release peers `solid-js ^2.0.0-rc.1`, so a
floor pairing `solid-js@2.0.0-rc.0` with web rc.1 was refused by npm before the
checker ran. The selector now raises a floor to a fixed point until the runtime
packages accept each other, collapsing the row to one `only` probe when floor
and head coincide. The catalog carries each runtime release's declared ranges on
its siblings to make that decidable.

**Two failure classes were conflated.** `no-esm-runtime-target` meant both "the
package declared a runtime target that does not exist" -- a publishing mistake --
and "the ESM target resolved, parsed, and exports nothing", which is a
well-formed side-effect-only module with no reactive surface to describe. The
second is now `no-exported-surface`. It is still a failure rather than a
success: promoting it would require the generator to emit an empty contract,
which is a semantics change and not a reporting one.

Two reporting defects travelled with them. A filtered run wrote the canonical
`report.json`, so a 23-probe sentinel silently replaced the full-corpus
artifact, and the report recorded no scope, so its header described the
manifest's 417 probes while its body held 23 results -- a clobbered report was
undetectable. Reports are now named for their scope, record it, and refuse a
`--baseline` from a different one. Separately, `diffManifests` compared version
and integrity but not probes, so a policy change printed "(no changes)" directly
above `--check`'s "file is out of date" verdict. `--check` itself was never
wrong: it compares the whole serialized document and correctly refused such a
manifest. The diff now reports probe changes, on the rule the same file already
states for exclusions and limitations.

The measured result is **409/416**. The denominator fell by one because the
incoherent `@tanstack/solid-router` floor collapsed into its head rather than
being probed as an environment that cannot install. All seven remaining
failures are outside this repository: two packages whose published manifest
names a file absent from the tarball (`@kobalte/themes`,
`@solid-primitives/composites`), two with self-contradictory peer ranges
(`@kobalte/solidbase`, and `@tanstack/solid-router-ssr-query` on both probes,
whose `@tanstack/solid-query >=5.90.0` peer cannot select any Solid 2 build
because every such release is a prerelease and a non-prerelease range never
matches one), one correct CJS refusal, and one side-effect-only module. Zero
timeouts, zero `type-facts-failure`, zero semantic contract-generation failures.

## Open: generation success is not contract correctness

The ecosystem benchmark counts whether `contract generate` produced a document,
not whether the document is true. That distinction is not academic: the run
immediately before the unknown-callback fixes measured 407/417 while
`@solid-primitives/utils` shipped a contract asserting that `map`, `filter`, and
`sort` never invoke their callbacks, and `@solid-primitives/event-listener`
asserting that `makeEventListener` never runs its handler. The metric was within
a percent of its cap and materially wrong at the same time.

The current run emits **14,309 checklist items across 409 invocations**, a mean
of 34. Every one of those contracts is `inferred` evidence held below the SC9005
trust ceiling, and none of those items has been reviewed against the packages'
published sources. So the corpus establishes that the generator reaches
essentially every installable package, and establishes nothing about whether
what it writes is correct.

Closing this needs a different measurement: review a sample of generated
contracts by hand against the real sources and count how many checklist items
resolve to "the generator was right" versus "the generator claimed something
false". Until that exists, a high success rate should be read as reachability
only, and the contract corpus under `scripts/contract-corpus.mjs` plus the
fixture snapshots remain the only checked-in evidence about correctness.

## Design-change candidates (open)

### `execution-map-incomplete` (SC9004) moved to producer integrity

Both dialect compilers emit every `jsx-expression` operation together with a
same-span region or callback role in every decision arm, and
`CompilerFacts::classifies` matches by span containment — so
`uncovered_jsx_expressions()` is empty by construction. The former SC9004
project rule could therefore describe only externally produced or partial
compiler facts, not a defect in analyzed source. It was removed from both
catalogs on 2026-08-20 and retained as a producer-integrity requirement. If a
third compiler adapter lands, its adapter tests must prove the same totality.

### Resolved: shorthand property values follow compiler runtime identity

TypeScript projects a shorthand property's *own* symbol at `{ pathname }` --
never the referenced value binding's -- so no TypeFacts entity, reference, or
declaration fact at that span identifies the value. The binder that builds the
Oxc AST facts does resolve that exact reference, and its answer is now carried
on `ObjectPropertyFact::shorthand_binding`; `interproc.rs`
(`binding_initializer`, `named_accessor`) reads the declaration from it instead
of matching the spelling within the enclosing function. That is scope-exact, so
the previous block-scoping hole is closed in both directions.

The cross-file gap is now closed from TypeScript's exact runtime identity and
symbol chain, not from a second module resolver. The imported binding must
carry a non-empty identity and its alias chain must end at a declaration in an
analyzed project source file. That directly incorporates the project's
selected module mode, extension priority, `baseUrl`/`paths`, package exports,
and re-export traversal without mistaking a project re-export of an external
value for project ownership. `interproc.rs::imported_accessor` joins the same
identity to the exact accessor/source export; the former textual relative
resolver remains only an accessor fallback for older or missing facts.

Two boundaries remain unavailable for an exact structured-property claim.
Each exported shorthand produces SC9012 instead of disappearing, and the
generated summary omits the unproven property rather than inventing a leaf:

- **external packages.** Their symbol chain has no declaration in an analyzed
  project source, even when a relative project module re-exports the value.
  Exact behavior belongs in an audited package contract.
- **globals/unresolved bindings.** A namespace import is an exact non-reactive
  namespace object and remains certified without SC9012. An unresolved export
  cycle is TypeScript-owned (TS2303), so it receives no checker finding.

What the fixture pins today is the same-file resolution set
(`scopedShorthand`, `unprovenShorthand`, `shadowedShorthand`,
`writtenShorthand`), the cross-file named-import join
(`importedAccessorShorthand`), compiler-selected ambiguous and path-mapped
joins (`ambiguousShorthand`, `pathMappedShorthand`), a nondeterministic import
set (`importedShorthand`, `namespaceShorthand`, `bareImportShorthand`,
`cyclicReexportShorthand`),
the default/named/export-all joins (`defaultReexportShorthand`,
`namedReexportShorthand`, `exportAllShorthand`), and a global
(`globalShorthand`).

The focused unresolved fixture now asserts three obligations explicitly: a
bare external import, the same value behind a relative project re-export, and
a global binding. Its path-mapped and ambiguous-relative controls certify
through runtime identity and project declarations. Exact local non-reactive
values and namespace objects likewise remain certified without SC9012.

The shared `solid_facts::resolve_relative_module_path` helper now answers
"which file does this relative specifier name" for both
`interproc.rs::relative_module_file` and the backend's
`resolve_relative_export`. It is lexical, project-local, and returns no
answer when extension/index candidates are ambiguous.

## Partially resolved design changes

- **`v1/jsx-no-undef` now fails closed on missing semantic facts.** It reports
  unresolved `use:` names only when the structural binder proves that no
  lexical binding exists. Unresolved JSX tags, including dotted roots, are
  TypeScript-owned (TS2304) and checker-silent rather than a second diagnostic.
  The old auto-import helpers remain test coverage for the upstream formatting
  logic, not a blanket semantic allowlist.
- **Unknown callback helpers remain contract obligations.** Exact TypeScript
  call facts now enrich the obligation and diagnostic with package,
  entrypoint, export/function, callback parameter index/type, required
  execution mode, and an editable schema-v1 contract stub. Standard-library
  behavior and project/package contracts can discharge it; unknown execution
  remains refused until an explicit contract proves it.

## Resolved design changes

- **Shorthand property values are resolved by the binder, not by spelling.**
  The value binding at `{ pathname }` is named by
  `ObjectPropertyFact::shorthand_binding` -- the declaration Oxc's scope tree
  chose for that exact reference -- so a same-spelled binding in a sibling
  block neither substitutes for the intended one nor makes it ambiguous. This
  replaced a spelling match scoped to the enclosing function, which both lost
  a provable structured return whenever any sibling block reused the spelling
  and, worse, could certify an accessor the shorthand never named. A shorthand
  the binder leaves unresolved carries no fact and proves nothing. The
  remaining cross-file gap is listed above.

- **Invoking a parameter's member is resolved per call site.** A function that
  calls `reader.read(value)` on its own parameter makes no claim about which
  implementation runs — that belongs to each caller. The owner records the
  obligation (`invoked_parameter_members`: parameter index and property), and
  `interprocedural_reads` resolves it against the argument actually passed at
  each site, the way `invoked_parameters` already substitutes a directly
  invoked parameter. One exact object contributes its reads; several exact
  objects contribute their common summary only when those summaries are
  equivalent. A missing or divergent implementation produces SC9012 rather
  than contributing no read. This replaces both the old pooled answer (which
  contaminated an exact site with an ambiguous sibling) and the later silent
  omission at the ambiguous site.

- **Callee resolution is exact and conservative.** Parenthesized, `as`,
  `satisfies`, and non-null wrappers are peeled through a shared AST fact
  helper. Resolved call declarations identify member callees when TypeScript
  provides them; static members can use their exact property entity, while a
  TypeScript-valid computed member such as `handlers[i]()` produces SC9012
  instead of inheriting `i`, inheriting `handlers`, or disappearing. An invalid
  computed call remains silent because TypeScript owns it.
- **Summary discovery covers method, alias, and returned-value branches.**
  Class/object methods, returned closures, conditional aliases, destructured
  function properties, and exact object spreads retain their canonical
  symbols. Direct generic calls and resolved structural member calls propagate
  summaries only through the dispatch proof described above; a finite
  unresolved dispatch remains explicitly uncertifiable through SC9012.
- **Transparent TypeScript wrappers are peeled at equality gates.** The
  shared helper is used by map/callback discovery, Solid 1.x structure gates,
  and shared reactivity function matching, with AST and fixture coverage for
  parentheses, `as`, `satisfies`, and non-null assertions.
- **Namespace-imported JSX primitives use dialect vocabulary.** `<Solid.For>`,
  `<Solid.Show>`, and `<Solid.Repeat>` resolve only when the namespace import
  is from a dialect-owned module and the member is in that dialect's export
  vocabulary. The namespace and named-import twins are pinned by
  `fixtures/reactive-ir/namespace-import-v2/`.
- **Component identity conventions are dialect-owned.** JSX call sites,
  direct JSX returns, and exact compiler-resolved Solid component aliases prove
  component identity. Solid 1 does not promote upstream's uppercase-name
  shortcut to proof: capitalization makes component identity **uncertifiable**
  until a JSX call site or exact component type selects the execution model.
  This uncertainty propagates through ownership, props, reads, destructuring,
  conditional returns, handler reads, and mutations. Solid 2's
  direct-JSX-return convention remains dialect-owned. The shared reactive core
  contains no hard-coded proven-component casing rule. Intrinsic-tag case
  checks remain syntax-only.
- **Dialect-owned type origin is no longer enough to register a source.** The
  dialect classifies exact exported aliases as component, accessor/resource,
  signal, store, setter, or store setter; user-local lookalike aliases and
  unrelated Solid types do not become accessors.
- **Unclassified function spans are `Unknown`.** Explicit compiler-untracked
  regions and other semantic proofs become `UntrackedRendering`; AST-proven
  module evaluation is its own one-shot role. Unknown reads/writes are not
  projected as violations.
- **Owner-shape recognition is AST-backed.** Binding immutability, array
  slots, call initializers, returned functions, and arrow kind now come from
  facts rather than scanning source bytes.
- **Compiler-established ownership is trace-backed.** Default compiler effect
  reruns emit typed owned regions without changing generated code. Custom
  wrappers make no claim; component and runtime-callback ownership still comes
  from exact TypeFacts identity and package contracts.

## Closed 2026-08-22: two vendored Solid 1.x compiler census gaps

Found by the ecosystem benchmark (`docs/ecosystem-benchmark.md`), which ran
`contract generate` against 417 real package/Solid-version probes. Two probes
failed with the `type-facts-failure` class, and both are **defects in the
vendored Solid 1.x JSX compiler, not in this repository's `rust/` tree**. Both
are recorded here rather than worked around: the checker is failing closed for
a reason that is real, but the reason is a bookkeeping disagreement inside the
compiler, not genuine ambiguity in the analyzed package.

Both live in `packages/compiler/src/semantic_trace.rs` of
`github.com/yumemi-thomas/solid-1x-compiler`, pinned at rev
`79b9b63721c59b0acfd72348438bbb6e090ec81c` (`rust/Cargo.toml`'s
`solid1-dom-expressions-compiler`). That file reconciles a **census** (every
JSX site the compiler owes an answer about) against the **trace** (the answer
lowering actually gave); `TraceRecorder::finish()` fails when the two disagree.
`rust/dialects/solid-v1/compiler` only consumes the finished trace and has no
seam to intervene, so neither is fixable from here. Fixing them means an
upstream change plus a `rev` bump per `docs/monorepo.md` — not a floated
branch.

Both are **Solid 1.x only**; the identical constructs compile cleanly under the
Solid 2.0 dialect, whose fork does not share either mechanism.

- **Static `style`/`classList` object before a later spread, on a native
  element.** Reported as `semantic trace has unresolved execution sites:
  NativeAttribute@<span>`, where the span is the whole object literal.
  Observed on `@kobalte/core@0.13.13`; reproduced in eight lines as
  `<input tabIndex={-1} style={{ "font-size": "16px" }} name={props.name} {...props} />`.
  The census decides whether to decompose the object using an element-wide
  "does this element have any spread" flag, while lowering uses a
  per-attribute, position-aware test (`!seen_spread && !dynamic`) that peels a
  static pre-spread attribute back into the ordinary template planner. The
  census records one opaque site that nothing then resolves. Removing the
  spread, or making the style value dynamic, both avoid it.
- **JSX fragment nested inside a callback passed as a control-flow built-in's
  prop.** Reported as `semantic decision targets an uncensused JsxChild site at
  <span>`. Observed on `@tanstack/ai-solid-ui@0.7.17`; reproduced with a
  `<Show fallback={(() => { ... return cond ? <>{expr}</> : null })()}>`. The
  census tracks "am I under a component" in a mutable `parent_component` flag
  that survives the recursive walk into attribute-value expressions, so a
  fragment created inside a separately scoped closure is censused as
  `ComponentChild` while lowering correctly decides `JsxChild` for the same
  span. Same span, different kind, so reconciliation fails. Solid 2.0 replaced
  the flag with an explicit set of component-child fragment spans, which is why
  it cannot occur there.

Status: **fixed upstream and pinned**. `rust/Cargo.toml` moved
`solid1-dom-expressions-compiler` from `79b9b63721c59b0acfd72348438bbb6e090ec81c`
to `ad2c9452041c757138bb972416d8abc4798ea6b9`, which carries both fixes:
`style`/`classList` decomposition now follows the same positional spread
carve-out lowering uses, and fragment children are classified from a span set
instead of a flag that leaked through attribute values. Both are census-only,
so emitted output is byte-identical across the two revisions.

The corpus confirms it: `@kobalte/core@0.13.13` and `@tanstack/ai-solid-ui`
both generate cleanly, and the whole 416-probe run contains zero
`type-facts-failure` results. Neither was a `tsc` concern in any form — this is
JSX-lowering execution-fact bookkeeping, so the absolute rule in AGENTS.md was
never implicated either way.

## Generated contracts are byte-bound only in the single-artifact case (2026-08-22)

`contract generate` used to write `artifacts: {}` unconditionally, so nothing
tied a generated contract to the bytes it describes beyond a version string —
and a version string is not a pin: republished or locally patched contents keep
the version, and the contract would still claim to describe them. The consumer
has always verified artifact hashes whenever present
(`validate_contract_artifacts` in
`rust/crates/solid-facts-backend/src/diagnostics.rs`), so the gap was entirely
on the producer side.

Generation now emits a real `artifacts.implementation` `{ path, hash }` pair —
the `hash` value carries the `sha256:` prefix — whenever schema v1 can carry it:
the contract's emitted entrypoints resolve to exactly one runtime artifact and
that file is inside the contract's own directory (the in-package default
output). Several residues remain, all recorded on the review plan as
`contract artifact binding` items rather than papered over:

- **Multi-artifact packages stay unbound.** Schema v1's `artifacts` object has
  exactly one `implementation` slot, and a package whose entrypoints resolve to
  several runtime files cannot be described by it. Hashing one of them would
  claim byte identity for a contract whose other entrypoints describe files
  nothing pins, so nothing is emitted. Closing this needs a per-entrypoint (or
  list-valued) artifact claim, which is a new schema shape rather than an
  additive field — an old schema-v1 reader must not be able to ignore a new
  sibling and read the omission as "no artifacts to check". Owner: a future
  schema revision, reviewed on its own.
- **Even a bound contract is bound at its entry artifact only.** The hash covers
  the export-map target file, while the analysis behind the summaries consumes
  that target's whole relative runtime-module closure (`runtimeModuleClosure` in
  `packages/cli/scripts/generate-package-contract.mjs`, seeded as the analysis
  roots in `analyzeTarget`). A barrel entry —
  `export { x } from "./internal.mjs"` — therefore gets a "bound" contract whose
  semantics come from a file no hash pins: patch `internal.mjs` and the entry
  bytes, and the hash with them, are unchanged. The hash is still real evidence
  about the entry file and keeps being emitted; the review plan now states the
  entry artifact is hashed and counts the closure modules that are not
  byte-bound. Closing this needs the same new schema shape as the multi-artifact
  residue — a list-valued or per-module artifact claim — so it is owned by that
  future schema revision, not by the generator.
- **Out-of-package outputs stay unbound.** A project-owned contract under
  `.solid-checker/contracts/<package>/` sits outside the package, so its
  artifact path could only be spelled with `..`, which the loader rejects by
  design. Nothing to fix here on the producer side; the review plan says the
  contract is not byte-bound, and the reviewer checks the artifact by hand.
- **No declaration artifact is ever generated.** The package generator analyzes
  runtime targets and never resolves the `types` condition, so it has read no
  declaration file whose bytes it could claim. `artifacts.declaration` remains
  available to the lower-level `--declaration-artifact` workflow, which is
  handed the exact file.

None of this is a `tsc` concern in any form: artifact identity is a trust-
boundary fact about bytes on disk, which the type system says nothing about.

## The runtime-module closure is walked, not attested (2026-08-22)

The per-entrypoint closure in a review plan's `generation.entrypoints` block is
what `contract review --transfer-from` treats as *the bytes this review was
recorded against*: an entrypoint transfers only when its recorded module paths
and sha256s are identical on both sides. That record is produced by
`packages/cli/scripts/runtime-module-closure.mjs` — a scanner and resolver in
the Node process, walking the same specifier forms TypeScript would. It is not
the file list the analyzing program actually opened.

The gap is real and was exploitable. Three shapes were silently omitted, each
producing a closure record that named fewer files than the analysis read while
claiming to name all of them:

- an ESM-spelled `./impl.js` whose checkout ships `impl.ts` (TypeScript's
  extension substitution);
- a `#`-prefixed specifier resolved through the manifest's `imports` map;
- every import below a string literal containing `/*`, because the comment
  stripper was a regular expression that knew nothing about strings.

A byte-identical barrel entry over a fully rewritten implementation therefore
transferred an entire review and promoted to `reviewed` evidence with zero
human decisions.

The walker now handles all three, and — more importantly — is fail-closed
instead of best-effort. Every static specifier form is resolved to a recorded
file, classified as carrying no runtime semantics (a declaration file), or
classified as external (a bare specifier, which the package-contract boundary
owns and no closure hash could pin). Anything else adds a `notes` entry to that
entrypoint's closure record, and a note makes the entrypoint non-transferable
and surfaces on the review plan's `contract artifact binding` section. A dynamic
`import()` with a non-literal specifier is noted for the same reason.

**The residue**: a syntax walk can still disagree with the compiler in ways
neither side reports — a `paths` mapping, a resolution the bundler condition
resolves differently, a specifier form the scanner classifies as external that
the analyzed program in fact opened. Nothing in this process can observe that,
because the process that resolved the modules is the other one. The exact fix is
a TypeFacts protocol addition: the analyzing program already knows its own file
list, and emitting it would turn the closure from a reconstruction into an
attestation. Until that exists, unresolvable specifiers fail closed via notes
and the record stays a generator-side claim.

### Closed: the record is attested, and the walk is now the thing being checked (2026-08-24)

The protocol addition exists and the checker consumes it. TypeFacts handshake
protocol `2` carries a `modules` operation reporting the program's own file list
and, for the importing files a request names, where each specifier resolved;
`--emit-module-inventory` asks for it on a generation run
(`write_module_inventory`, rust/crates/solid-facts-backend/src/main.rs), and the
generator builds `generation.entrypoints[*].modules` from that instead of from
its own walk (`attestedClosure`,
packages/cli/scripts/generate-package-contract.mjs).

The walk is **not** deleted. It seeds the analyzed program's `files` list, and it
has to: seeding only the entrypoint makes a published barrel's `.js` specifiers
resolve to the adjacent `.d.ts`, so the analysis would read declarations where it
now reads runtime bytes. What changed is that the walk's output is no longer a
claim about anything — it is a seed, and the attestation both replaces the record
and *verifies the seed*. A module the program opened that the walk never seeded,
or the reverse, is a named note in either direction.

The residue above was not theoretical, and the mechanism that produced it was not
one of the three shapes listed. `moduleSpecifiers` scans an import/export clause
for its `from` under a 300-token bound at depth 0, so a clause naming more than
~150 bindings hides its own specifier — and records **no problem**, because the
scanner never saw a specifier to fail on. The walk returned the entry alone with
an empty `notes`, which is exactly the false record this entry is about.
`fixtures/package-contracts/seed-attestation-discrepancy` pins it.

What was measured on real packages, reproduced locally against the pinned
producer (npm tarballs, no peer installs, so these are not the ecosystem
benchmark's own rows):

| Package | Before | After |
| --- | --- | --- |
| `@solidjs/vite-plugin@3.0.0-next.31` | 1 note (non-literal `import()`) | 0 notes, 1 `runtimeNotes` |
| `@solidjs/start@2.0.3` | 4 notes (2× `./styles.css`, `../../../package.json`, non-literal `import()`) | 1 note (restated), 1 `runtimeNotes`; 80 → 158 modules |
| `@tanstack/charts@0.14.0` | 1 note (`./Chart.svelte`) | 1 note (restated); 834 → 2423 modules |

Re-measured after the fixes below, against the same three tarballs and the same
producer: identical note counts, identical module counts, and
`@solidjs/vite-plugin`'s sole blocker — the non-literal `import()` — now
classifies as `attested-closure-note` rather than `unclassified-refusal`, both on
the full refusal line and on the 260-character head the corpus harness stores.

**Measured across the whole ecosystem corpus** (2026-08-24, 416 probe rows,
release binary `0356938638a2d6594452dd574dcff4a82332b2f0a4d58abed21b578a759a3588`;
the full account is in
[ecosystem-benchmark.md](ecosystem-benchmark.md#headline-numbers-2026-08-24-eighth-measurement-state-release-binary-416-probes)):

| Figure | Before attestation | After |
| --- | --- | --- |
| Closure notes | 31, across 7 probes | **5, across 2 probes** |
| Attested closure notes (`runtimeNotes`) | field did not exist | **17, across 5 probes** |
| Probes fully proven | 125 / 398 | **125 / 398** |
| Rows reaching `verified` | 275 / 416 | **275 / 416** |
| `closure-note` as root cause | 4 rows | **2 rows** |
| `attested-closure-note` as root cause | class did not exist | **2 rows** |
| `unclassified-refusal` rows | 0 | **0** |

The 31 notes split three ways and every one lands in exactly one bucket: **17
reclassified** to `runtimeNotes` (all of them the non-literal dynamic `import()`
shape — no corpus package exercises the unselected-conditional-branch shape that
`conditional-imports-side-effect` pins), **9 answered and dropped** (asset imports
— `./styles.css` on `@solidjs/start`, `./style.css` on
`@tanstack/form-devtools` — for which the compiler resolved nothing either and no
runtime target exists on disk), and **5 retained and restated** with the module
the analyzing program actually resolved the specifier to.

**Nothing was certified by a note disappearing.** No probe gained fully-proven
status, no row moved `refused → verified`, every claim figure in both harnesses is
unchanged to the claim, and the one row that lost its closure blocker outright
(`@tanstack/form-devtools@1.0.0-alpha.2`) is still refused on `kind-observed` and
`probe-failed`. Measured on real packages rather than argued: the module *record*
roughly doubled at the same time — `@solidjs/start@2.0.3`'s sum over its 12
entrypoints goes 215 → 428, distinct modules 96 → 184, with 209 of the 428 being
declaration files — so the record now names substantially more bytes while
certifying nothing extra.

**The `834 → 2423` figure is a sum over entrypoints, not a file count**, and it
should not be quoted as "what the analysis read under this package". The package
declares 110 entrypoints; their records name **337 distinct modules**, ranging
from 1 (`./export`) to 195 (`.`), and a file shared by twenty entrypoints is
counted twenty times in the sum. None of the three tarballs carries an installed
dependency, so the scope rule below does not move any of these numbers.

**Two of the design's predictions were wrong, and the direction matters.** The
`./Chart.svelte` note was classified as an asset import that would disappear; it
does not. The package ships `dist/svelte/Chart.svelte.d.ts` and the compiler
resolves the specifier to it, so the walk's note was *correct* and attestation
makes it more precise rather than removing it. The walk's declaration probe
appends `.d.ts` to the *stem* (`Chart.d.ts`) while TypeScript appends to the
whole specifier (`Chart.svelte.d.ts`) — a walk bug that only an attestation could
surface. Likewise `../../../package.json` resolves for real. So the asset-import
class is narrower than the 5-of-13 sample suggested: only a specifier with no
sibling declaration at all disappears.

**The named remaining approximations**, all deliberate:

1. **The declaration-sibling split is visible in the record and not otherwise
   reported.** An import that resolves to a `.d.ts` with an empty `includedPath`
   is a module the analysis read declarations for while runtime bytes sit beside
   it, and nothing joins the two — the producer says so explicitly and forbids
   pairing by file name. That is now *recorded* (the `.d.ts` is in the module set
   and its bytes are hashed) but it raises no closure note of its own, because
   the analyzer already reports the same fact as an incompleteness finding —
   `fixtures/package-contracts/declaration-sibling-reach` pins it — and a second
   report would fire on nearly every published package for one cause.
2. **The runtime is still unbounded.** Two shapes refuse promotion under
   `runtimeNotes`, and no module graph can close either: a non-literal dynamic
   `import()`, and a specifier the compiler resolved nothing for that names
   existing runtime modules inside the package a runtime *can* still select — an
   unselected conditional `imports` branch. The second was a **wrong
   certification** in the first cut of this change: "the compiler resolved
   nothing" was read as licence to say nothing, which is true about the record and
   silent about Node, which loads the `node` branch. The predicate is now a fact
   about files on disk (`runtimeTargets` in
   packages/cli/scripts/runtime-module-closure.mjs) rather than a judgement about
   a file suffix, so an asset import and a missing file still drop while a real
   branch does not.
   `fixtures/package-contracts/conditional-imports-side-effect` pins it, and its
   README records why the re-export form of the same package was never affected
   (the analyzer refuses the entrypoint on an `Unknown` runtime kind). A permanent
   limit, not a backlog item.
3. **`--emit-module-inventory` is a generation-run flag.** An ordinary analysis
   does not ask for the graph, so nothing on the diagnostic path is
   identity-bound to a resolved module yet — that is the separate open entry on
   contracts bound to a module *name*, below.
4. **The two processes spell paths differently and neither derives the other's.**
   TypeScript takes a realpath only where resolution walked a symlink under
   `node_modules`, so a package generated inside a symlinked temporary directory
   is reachable by two spellings at once. Every path is normalized through
   `realpathSync.native` before comparison and the record is written back in the
   generator's own spelling; a filter that assumed one spelling silently matched
   nothing and turned the scoped import request into an unscoped one. Two
   corollaries the first cut got wrong, both now pinned in
   `scripts/contract-closure-record.test.mjs`:
   - **The scope test accepts either spelling**, because the checker's own
     inventory filter does. Canonicalizing first and filtering second dropped an
     intra-package directory symlink (`src -> ../shared`) out of the record *and*
     out of both reconciliation sweeps — a record reading as a complete
     attestation while the file every summary came from went unnamed, which is
     the defect class this entry exists to close. The record and the sweeps now
     share one scoped view, and a module the analysis read that the scope excludes
     is itself a note.
   - **`realpathSync` does not canonicalize case; `realpathSync.native` does.** On
     a case-insensitive filesystem the walk accepts `./Impl.js` for `impl.js`, so
     one file arrived as two keys: the record named a spelling that exists on no
     case-sensitive filesystem, and the seed sweep reported the real file as
     seeded-but-never-opened. A record is transferred between machines, so the
     verdict may not depend on which one wrote it.

5. **A dependency's bytes are excluded from the record, and nothing here pins
   them.** The record is scoped to this package's own files, so a dependency the
   analysis read — nested under the package root or hoisted above it — is named by
   *that* package's contract and closure record, not by this one. That is
   deliberate: hashing it would bind the record to the install layout and to a
   dependency's version, so two generations over byte-identical package bytes
   would refuse to transfer a review, and the first cut of this change did exactly
   that. **The residue is a dependency with no contract of its own**: its bytes
   determined summaries here and no artifact in this repository pins them. The
   dependency-contract boundary is where that belongs (`dependencyContracts`, and
   the unresolved-dependency refusal that demands one), so this is a named
   approximation rather than a hole the closure record should paper over.
6. **A file the analysis read from outside the package is noted, not recorded.**
   The record cannot hash it and cannot claim it, so it says it excluded it. In a
   workspace install where a sibling package is reached through a `node_modules`
   symlink the note does not fire — the resolver's own `node_modules` spelling
   classifies it as a dependency — but a workspace dependency reached by a path
   with no `node_modules` segment at all would note. That direction is
   fail-closed and preferred to silence; if it proves noisy on a real monorepo the
   fix is a manifest-aware dependency test, not a wider silence.

**The fail-closed tier under all of this is defence, and calling it a tier would
overstate it.** An absent inventory or a `complete: false` graph leaves the record
labelled unattested and blocks everything, but against the pinned producer neither
shape can occur: a run that cannot write an inventory exits non-zero and aborts
the generation before a contract exists, and the producer builds its import
request out of the program's own inventory answer, so the request is always a
subset of the holdings. The two stub-driven tests
(`STUB_INVENTORY_ABSENT` / `STUB_INVENTORY_INCOMPLETE`) pin the contract a future
producer must be met with, not behavior this repository has observed, and they say
so. No generated contract here has ever carried the sentence.

**The blocker had to stay measurable, and did not at first.** `runtimeNotes`
raises its own refusal sentence, one word away from the closure-note one
("carries an **attested** closure note"), and the corpus classifier matched on
the shorter phrase — so every row whose only blocker was this one was counted as
an `unclassified-refusal`, the number amendment A9's stage 2 gate reads. It is now
its own blocker kind (`attested-closure-note`) in the verifier's `BLOCKERS`, its
own class in the classifier, and its own root cause ordered just after
`closure-note`. `verify-corpus.test.mjs` now holds *every* kind the verifier
declares to being nameable here, which is the assertion whose absence let the
first one through.

**Cost**: two extra round trips on a generation run, both reads of an
already-built program. Measured on the widest single program the corpus's widest
package produces (`@tanstack/charts@0.14.0`, 156 root files, 344 modules and 623
import facts answered): 4767 ms → 5201 ms mean over five runs, median 4779 ms →
5031 ms. Nothing on the analysis path pays it.

Corpus-wide the same cost is **+2.4% of the generation phase**: 197,685 ms →
202,504 ms of aggregate worker generation time over the ecosystem benchmark's 416
probes. Whole-run wall clock is noise-dominated at this scale and moved the other
way (100.820 s → 97.054 s), which is why the phase figure is the one quoted.

**One-time re-review.** The record's shape changed, so no review recorded against
a pre-attestation record transfers onto a regenerated plan: `contract review
--transfer-from` reports `its runtime module closure changed` and transfers
nothing. Verified end to end on `declaration-sibling-reach` (`transferred 0 of 9
review item(s)`). This is correct — the older record did not name declaration
bytes the summaries demonstrably depend on — and it is documented in
[package-contracts.md](package-contracts.md). No compatibility shim: one that
accepted the old record would be accepting a review of a file set nobody
enumerated.

Two adjacent facts belong with it. `contract generate --missing` writes
project-owned contracts under `.solid-checker/contracts/<package>/`, which are
outside the package by construction and therefore never byte-bound at the loader
(see the out-of-package residue above) — so the artifact-binding residue is the
*default* shape for project-owned contracts, not an edge case, and the review
plan's binding section is the only thing standing in for a hash there. And the
closure record is not evidence the loader reads: it lives in the review plan, and
nothing outside `contract review` consults it.

`tsc` has nothing to say about any of this. Which files a contract's summaries
were derived from is a provenance fact about a generation run, not a typing
question.

## The ecosystem benchmark counted partial contracts as successes (2026-08-22)

`scripts/ecosystem-benchmark/` classified any exit-0 generation as `success`,
including a contract that refused entrypoints and said so on stdout. The
checked-in `benchmarks/ecosystem/report.md` therefore reports the Official
Solid family under Solid 1.x as "Declared entrypoints: 44 / Generated
entrypoints: 28 / Success: 6/6 (100%)", with no field anywhere in the report
that could attribute the gap. Classification now has a `partial-success` class
and a matching probe outcome, so `success` means a complete contract, and the
refused entrypoints are counted, summed per family, and listed by package.

Adding the third outcome also left every *comparison* in the report still
written as a two-valued test. `buildBaselineComparison` and `buildFloorHeadDiffs`
in `scripts/ecosystem-benchmark/lib/report.mjs` asked "was it `success`, is it
`success`", so a probe going `partial-success` → `failure` — the run where the
contract disappeared entirely — matched neither regression nor fix, and the
symmetric `failure` → `partial-success` gain matched neither either. Both now
compare direction on the ordered scale `success > partial-success > failure`,
carry both outcomes on each entry, and render the transition rather than a
hardcoded destination. The floor/head headings are named for direction
("Worse/Better at head than at floor") for the same reason.

**Regenerated 2026-08-22.** The checked-in reports now carry the split. On the
305-row/416-probe manifest with the release binary, the full corpus is
**403 complete contracts, 6 partial, 7 failures**, against **409 successes and
7 failures** on the same manifest before the split. The failure set is
unchanged package-for-package and class-for-class, and all 6 partials are
former successes (`@kobalte/core`, `@tanstack/charts`,
`@tanstack/solid-pacer`, `@tanstack/solid-router`, and `solid-js@2.0.0-rc.1`
on both floor and head): `409 = 403 + 6`. The typed generation-refusal change
moved no probe into a failure class. The sentinel subset moves the same way —
23 probes, 20 complete, 2 partial, 1 failure — and now runs against the same
manifest as the full report instead of an older 417-probe one.

The measurement also refutes half of the prediction above. The Official Solid
"44 declared / 28 generated" gap is **not** refusals: that family records zero
refused entrypoints while still generating 11 of `solid-js`'s 23 declared
entrypoints and 2 of `@solidjs/image`'s 5, all classified `success`. Declared
counts include export-map branches for which the generator emits no contract
entrypoint at all, and no field attributes that. It is recorded as unmeasured
in docs/ecosystem-benchmark.md rather than as closed by this class.

## Closed 2026-08-22: schema-valid callback argument claims are never dropped

Contract callback rows may carry `arguments` descriptors — "this helper hands
your callback a reactive value at parameter N". Source discovery materializes
one shape only: an inline function literal whose span *is* the call-site
argument, carrying an `accessor` descriptor. Every other schema-valid shape —
a callback passed by name, or a `store-path`/`tuple`/`object`/`argument`
descriptor — was dropped in silence, so the callback body was analyzed as if
the contract had said nothing about its arguments. Reads through the callback's
parameters then looked like ordinary data: fail-open, and no gate could see it,
because no checked-in contract uses `arguments` yet.

The consumer now keeps those call sites demand-sensitive, through the same
per-export SC9005 path the unknown-callback domains use
(`rust/crates/solid-reactive-ir/src/interproc.rs`). Precision is unchanged
where the claim binds: an inline literal carrying only `accessor` descriptors
still materializes the accessor and reports nothing. A descriptor beyond the
literal's declared parameters is not a gap either — but only when the literal is
a *restless arrow*, the one shape that provably cannot name the argument. A
non-arrow function expression reads the slot as `arguments[N]`, and a rest
parameter — which `FunctionFact.parameters` deliberately excludes, because it
has no single argument index — absorbs every argument from its index onward;
`mapPath((...args) => args[0].value)` was silently clean before that fact was
carried (`FunctionFact.rest_parameter`, added in
`rust/crates/solid-facts/src/ast/mod.rs`).
`fixtures/reactive-ir/package-callback-arguments-consumer/` pins all six
outcomes.

**Remaining approximation.** The non-`accessor` descriptor kinds are *reported*
rather than *modeled*: a `store-path` argument handed to a callback is a real,
expressible claim that the consumer could materialize as a store source. Until
it does, such a contract makes the call uncertifiable instead of certified.
That is fail-closed and honest, but it is a claim shape the schema allows and
the consumer does not yet use.

## Closed 2026-08-22: contract and dispatch obligations no longer suppress each other

`PackageContractExportMissing` consumer obligations and genuine
`ReactiveDispatchUnresolved` findings travel in one vector — they are found by
the same interprocedural walk — and both were deduplicated under the single
identity `reactive-dispatch-unresolved`. The dedup key is
`(identity, path, start_byte)`, so an SC9005 and an SC9012 that merely started
at the same byte silently suppressed one another, and which one survived
depended on push order. The identity now follows the defect kind
(`rust/crates/solid-reactive-ir/src/reactive_analysis.rs`), pinned by
`reactive_analysis::tests::contract_and_dispatch_obligations_do_not_deduplicate_each_other`.
No checked-in snapshot moved: no current fixture produces the colliding pair.

## Closed 2026-08-22: explicit contracts cannot bypass version classification

An explicit `--contract` file was version-classified only when its package
appeared in the *import-derived* module set. Contract resolution also applies a
contract to `export … from "pkg"` re-exports, which never contribute to that
set, so a stale explicit contract could be applied to a package the project
only re-exports. Classification is now unconditional
(`rust/crates/solid-facts-backend/src/diagnostics.rs`); a package that is not
installed has no manifest to disagree with, so an explicit contract for it
still applies exactly as before. Pinned by
`diagnostics::tests::explicit_contracts_are_version_checked_without_an_import`.

**Remaining gap.** `package_contract_statuses_with` still enumerates
`imported_package_roots`, so a re-export-only package with a refused contract
is fail-closed in analysis but invisible in `--check-contracts`. Closing that
means broadening the module set the report walks, which changes what the
report claims about every tier, not just the explicit one.

## Entrypoint conditions are alternatives; only the host target is scope (2026-08-22)

`RuntimeEnvironment::matches_entrypoint_conditions` combines an entrypoint's
recorded conditions with membership, not containment, and that is correct for
how contracts are generated: the list is the union of the export-map branches
the entrypoint resolves through. The bundled `solid-js` root entrypoint records
`browser, deno, development, import, node, worker` — no environment satisfies
all of it at once, and requiring containment would make the contract this
checker ships unmatchable. Pinned by
`entrypoint_conditions_are_alternatives_except_for_the_host_target`.

`--conditions` generation writes the *asserted scope* into the same union
field, where the list is not alternatives. The host target is the one dimension
where the two are distinguishable — at most one of `browser`/`node`/`deno`/
`worker` describes any environment — so an entrypoint naming host targets and
not the consumer's now fails closed rather than matching through a shared
resolver condition such as `import`. Recording `default` keeps it open, since
the unconditional branch really is reachable everywhere.

**Remaining approximation.** The other exclusive dimensions (`development` vs
`production`, the rendering modes) cannot be tightened the same way: real
export maps record only `development` as a branch, and a production consumer
legitimately resolves such an entrypoint through its fallback. A
`--conditions production` contract therefore still records its build scope into
a field a development consumer can match. Closing this needs schema v1 to
distinguish "branches observed" from "environment asserted" at the entrypoint
level, which it cannot express today. A conservative alternative that also
fails closed for the non-target case would produce false uncertifiable results
against every checked-in contract, so it is not the smaller evil.

## Spread arguments to parameter-member reads are reported at the spread (2026-08-22)

`argument_proves_non_reactive` treats an array/object literal as proven plain
data, spread included, and that was audited rather than assumed:
`drop([...storeArray])` copies out of the proxy at the call site, so the callee
really does receive snapshot data and its `parameter-member` claim proves
nothing about reactivity. The read that exists is the spread, and the spread
pass in `local_access.rs` already reports it in its own execution role —
`fixtures/reactive-ir/package-parameter-member-consumer` `SpreadArgument`
produces exactly one `SC1001` for `"state spread"` and no `SC9012`. Adding the
obligation as well would report one dependency twice.

**Remaining gap.** The copy is shallow, so a nested proxy surviving it
(`drop({ ...store }).nested.value`) is a second dependency that neither the
spread read nor the parameter-member claim describes. Closing it needs the
consumer to track proxy identity through a literal's element/property
positions, which no fact table carries today.

## Closed 2026-08-22: contracts are enforced against the lockfile integrity, where one exists

`package.integrity` — the npm sha512 of the tarball a contract was audited
against — was format-validated on load
(`rust/crates/solid-reactive-ir/src/lib.rs`) and then compared to nothing. A
published or project-owned contract bound to nothing but a version string, and
a version string is not a pin: a republished tarball, an `npm overrides` entry,
and a locally patched install all keep the version while replacing the bytes
the summaries describe. Every bundled contract carries an integrity, so this
was the strongest available identity fact going unused.

Loading now recovers the installed copy's integrity from the project's Bun or
npm lockfile and refuses a disagreeing contract exactly as it refuses a stale one:
status `stale`, an uncertifiable `SC9005` at the import, the run continues. The
message and the report `detail` name **both integrities**, because the versions
agree and naming them would read as a contradiction. Bundled and project-owned
contracts get their existing, different remedies, reworded for the case where
the audited version is already the installed one.

The integrity comes from Bun's `bun.lock` package records, or from
`package-lock.json` / `node_modules/.package-lock.json` at `lockfileVersion` 2
or 3. Bun selects the record by its resolved package identifier and installed
manifest version; npm's `packages` map is keyed by *install path* and so names
the specific installed copy rather than a package name. Pinned by
`cli_refuses_a_contract_whose_lockfile_integrity_moved_under_the_same_version`
(process) and `lockfile_integrity_is_recovered_only_when_it_is_unambiguous`
(unit).

**Remaining approximation, deliberately fail-open on the fact and fail-closed
on the verdict.** Enforcement needs both halves — an integrity in the contract
and a recoverable one on disk — and every way the second half is unavailable
yields *no fact*, which means the previous behavior (version matching alone),
never a refusal:

- **No recognized Bun or npm lockfile.** pnpm and Yarn keep their own formats,
  and many projects have no lock at all. Reading them is tractable but each is
  a separate format with its own store layout and its own path-to-entry
  question; none of it can be guessed from the Bun or npm shape. Owner: one
  format at a time, each with its own fixture.
- **Unsupported lockfile versions.** npm `lockfileVersion` 1 is keyed by
  package name, so resolving an entry to *which* installed copy it describes
  under hoisting would be exactly the guess this must not make. Bun versions
  other than the currently supported v2 record are likewise ignored.
- **Link, workspace, `file:`, and git entries** have no registry tarball and so
  no integrity. A linked workspace package's bytes are unpinnable by
  construction; closing this needs a content hash of the linked directory,
  which is a different mechanism from npm integrity.
- **Two lockfiles disagreeing about the same installed directory.** Which one
  describes the bytes on disk is not answerable from the files, so no
  enforcement happens rather than a coin flip in either direction.
- **An unparseable lockfile** is the project's own file, not a malformed
  contract, so it never fails the run.

A contract with no `package.integrity` is unaffected in every case.

## Closed 2026-08-24: package contracts are bound to an installed package, not a module name

Contract discovery and contract application both keyed on the import
specifier's package root and nothing else. `discover_package_directory`
(`rust/crates/solid-facts-backend/src/diagnostics.rs`) walked ancestors for
`node_modules/<name>`, and `PackageContract::for_module` — the only gate in
`resolve_contract_imports` (`rust/crates/solid-reactive-ir/src/contracts.rs`) —
compared `contract.package.name` against `import.module`'s root. Neither asked
where the specifier actually resolved.

**The failing scenario, reproduced.** A tsconfig `paths` entry maps
`"reactive-package": ["./src/local-impl"]` while `node_modules/reactive-package`
is still installed (a local reimplementation, a fork under development, a test
double). Against a pre-change binary, the installed package's reviewed contract
was discovered by name, passed version classification, and raised its
`SC9005` callback-argument obligation *at a call whose callee is
`src/local-impl.ts`* — a file the contract's author never saw. Its summaries
were driving reactive-read, callback-timing, and owner-requirement conclusions
about project source: a false certification, not merely a missed one. The same
project against the fixed binary refuses the contract for that specifier and
raises nothing, while an identically shaped package with no `paths` entry in the
same file still binds and still raises its obligation
(`fixtures/reactive-ir/package-contract-paths-shadow`).

**What closed it.** The Type Facts producer's resolved module graph
(the Type Facts `modules` operation, introduced in handshake protocol 2 and
retained unchanged in protocol 3) forwards, per import specifier occurrence,
the file the resolver selected, the shape of the resolution, the owning
manifest's name and path, and the identity the resolver itself recorded. That
answer is carried as a fact table — `ProjectFacts.resolved_imports`
(`rust/crates/solid-facts/src/resolution.rs`) — and joined to the syntax facts by
the specifier's own span inside the declaration's, never by matching specifier
text. `PackageContract::for_import`
(`rust/crates/solid-reactive-ir/src/lib.rs`) then applies the name match as a
prefilter and the attested resolution as the confirmation. The full rule and its
five clauses are in
[package-contracts.md](package-contracts.md#which-imports-a-loaded-contract-describes).

Each of the three objections recorded when this was investigated on 2026-08-22
is answered by the fact rather than worked around:

- **A pnpm or workspace-symlinked install** reports the realpath while discovery
  returns the link path. Both spellings of the classified directory are
  accepted, and the comparison is component-wise containment rather than a
  string prefix, so `node_modules/pkg` cannot claim `node_modules/pkg-extra`.
  Pinned by `contracts_process::a_linked_install_binds_its_contract_through_the_realpath`,
  which builds the symlink in a temporary directory — the add-fixture skill
  forbids committing one.
- **An untyped JavaScript package**, which is precisely where a contract matters
  most, resolves to nothing at all. The compiler answering `unresolved` is an
  attested fact, and it is accepted: nothing resolved means nothing *else*
  claimed the specifier, so no shadowing package can be what the contract
  describes. The same clause covers a specifier typed by an ambient
  `declare module`, which is how an untyped package is normally typed.
- **An `@types`-typed package** resolves into `node_modules/@types/<name>`,
  which is a different installed package, and its contract is **refused**. This
  is a deliberate fail-closed outcome and the one named residue below.

**A refusal is silent in the findings and visible everywhere else.** The import
goes uncertifiable on the rules' own terms, which is the right answer for the
code under analysis — the alternative is a finding about the project's tsconfig,
which is not this checker's subject. But a refusal is also what a *defect* in
this machinery produces, so it is reported in the two places that describe the
run rather than the code. `SOLID_CHECKER_TIMINGS=1` carries
`contractBindingsBound` and `contractBindingsRefused`
(`Program::contract_binding`), and `solid-checker contract check` reports a
contract that binds no import as `unbound` and counts it as needing action —
that command answers whether contract coverage is complete, and it previously
answered `missing: 0` for a contract the analysis refused at every import. It
now performs the same identity attestation a diagnostic run does, which is what
makes that answer possible.

The earlier conclusion that "no narrow safe check exists with today's facts" was
correct about the facts it named — declaration paths and specifier text — and is
superseded by a fact that did not cross the seam then. What was rejected as
"half-implementing it" was containment on *declaration* paths; this is
containment on the *resolved module*, which is a different fact with none of the
three failure modes.

`tsc` has nothing to say about any of it. `tsc --noEmit` is silent on both new
fixtures: the local reimplementation type-checks, the `@types` package
type-checks, and the ambient declaration type-checks. Which installed package a
specifier resolves to, and which contract may therefore describe it, is a
resolution and provenance question the type system does not model.

### Named residues

- **With no classified install, a resolution outside every `node_modules` tree
  is refused even when the names agree.** Clause 5 compares package *names*,
  and names are not evidence about bytes: a monorepo package aliased to its own
  sources through `paths` has a root manifest declaring the very name its
  published contract carries, so name equality agrees while the file is source
  the contract's reviewer never saw. Requiring the resolution to have landed in
  an install tree closes that — it was a live false certification in the first
  version of this change, reproduced with a project-owned contract, no
  `node_modules/<name>` at all, and a root manifest sharing the contract's name
  — and the cost is the reverse case: a contract for a package that resolves
  outside every install tree can no longer apply at all. Nothing produces that
  shape except an alias, so the residue is theoretical, and the clause's
  remaining positive case is not: an explicit `--contract` for a package the
  ancestor walk never classified, resolving into
  `packages/app/node_modules/<name>` under a root-level tsconfig, was measured
  against the real producer and binds identically before and after this
  narrowing. That case is pinned by
  `with_no_classified_install_either_attested_package_identity_answers`, and
  both directions of the refusal are pinned end to end by
  `fixtures/reactive-ir/package-contract-uninstalled-name-match`.
- **Clause 3 is bounded by what TypeScript can see, not by what runs.** "The
  compiler resolved nothing, so nothing else claimed the specifier" is true of
  claims the compiler can make. An ambient `declare module` for a package that
  is installed nowhere reaches clause 3 and the contract applies, against
  whatever the runtime actually loads for that specifier — a bundler alias, an
  import map, a Node loader hook. `tsc --noEmit` is silent on such a project, so
  the absolute rule does not cover it either. This is the same limit as the
  runtime-alias residue below, reached from the other side: there the compiler
  resolves into the install and the runtime does not, here the compiler resolves
  nothing at all. The compiler's resolution is the only resolution this checker
  has.
- **An `@types`-typed package with a contract goes uncertifiable.** The
  resolution lands in `@types/<name>`, so neither identity is the contract's
  package and the contract is refused. Deriving "`@types/x` describes `x`" from
  the two names is the name-only reasoning the precision contract forbids, and a
  Solid-aware package that both ships a reactivity contract and is typed only
  through DefinitelyTyped is a shape nothing in the corpus has. The outcome is
  pinned in `fixtures/reactive-ir/package-contract-install-shapes` so it cannot
  be reversed silently.
- **A refusal does not fall back to a shorter name-matching contract.** The
  prefilter selects the longest matching package name, and a refusal ends the
  question for that specifier.
- **A runtime alias TypeScript cannot see is still invisible.** A bundler alias
  (`resolve.alias`, an import map) can make the runtime load something other
  than the installed package while the compiler resolves into it. The
  compiler's resolution is the only resolution this checker has, so that shape
  is out of reach of this rule and of every fact domain the checker holds.
- **The WASM adapter without `resolvedImports` binds by name.** That adapter has
  no session with which to ask, so a request omitting the field keeps the older
  behavior exactly. Documented in
  [packages/wasm/README.md](../packages/wasm/README.md) and pinned from both
  sides by `packages/wasm/test/resolved-imports.test.mjs`. Two properties of a
  *supplied* row are validated rather than trusted, because a wrong row refuses
  contracts exactly as a contract-less project does and would otherwise read as
  coverage varying by file: the span must be byte offsets naming the specifier
  in the source the request carries (a host forwarding TypeScript's UTF-16
  positions unconverted was silently correct for ASCII and silently wrong after
  the first non-ASCII character), and `resolvedPath` must be empty exactly when
  `resolution` is `unresolved` (an `unresolved` row is *accepted* by clause 3,
  making it the one host mistake here that failed open). Both are hard errors.
- **Contract emission still binds dependency contracts by module name.** Within
  one `--emit-contract` run the obligations of the package under generation are
  computed identity-bound (`resolve_contract_imports`), while
  `dependency_export_summary` and the export-all lookup in
  `solid-facts-backend/src/main.rs` call `for_module`. They can only disagree for
  a package whose own tsconfig shadows a declared dependency with a `paths`
  entry; no corpus package has that shape, and the generator produces these
  dependency contracts from the dependency's installed sources in the same run.
  Both sites say so in a comment. Routing them through `for_import` needs the
  declaration span threaded through two call chains and would move generation
  answers, so it is recorded rather than attempted.
- **The attestation is re-asked on every program generation, including every
  incremental edit.** Measured on this machine with
  `benchmarks/compare-performance.mjs` racing the pre-change binary: the
  one-file incremental edit on a 1,000-file corpus goes 35.3 ms → 39.2 ms
  (ratio 1.112, against the gate's 1.35 limit), first-IR ns/source is unchanged
  at 0.981, and a cold one-shot run costs 4.7 µs/source (+2.6 % of total wall
  time at 1,000 files, +1.2 % at 500). **That ratio is specific to 1,000 files,
  which is the only size the gate measures.** The module-graph operation answers
  the whole program's file inventory unconditionally, so the per-edit cost grows
  with program size rather than with the edit: an independent re-measurement at
  3,000 files put the incremental ratio at 1.152, consistent with ≈ 5.5 µs per
  program file per edit (+4.1 ms at 1,000 files, +16.7 ms at 3,000).
  `benchmarks/compare-performance.mjs` hardcodes 1,000 files, so the 1.35 gate
  will not see that trend; read the recorded figure as a point measurement, not
  as the shape of the cost. A sound reduction exists and is not attempted here:
  an edit that only *modifies* existing files cannot change another file's
  resolution, so those files' rows could be reused and only the edited files
  re-asked — and that is exactly the optimization that would flatten the trend.
  It is not attempted because a creation or deletion *can* change an unedited
  file's resolution, and reusing a row across that would fail open — the one
  direction this change exists to remove.
- **An attestation failure is fail-to-run, not fail-to-certify.**
  `attest_import_identities` propagates its error through
  `NativeIncrementalSession::attested` and the one-shot path with `?`, so a
  failing `modules` operation turns a previously analyzable project into a
  non-zero exit (exit 3 on a producer handshake mismatch) on *every* diagnostic
  run of *every* project with a bare specifier. The precision contract asks for
  an explicit uncertifiable result instead, and no degrade tier exists: there is
  deliberately no "attest what you can and name-match the rest", because that
  tier is name-only binding with extra steps. What is missing is the honest
  third option — refuse *every* contract for the run and say so once — which
  needs a place to say it that is not a finding about the user's code. Until
  then a producer or protocol failure is loud rather than silent, which is the
  safe direction but not the specified one.
- **`install_root` drops the canonical spelling when `canonicalize` fails.**
  `diagnostics.rs`: the spelled path can then be relative (`--project
  ./tsconfig.json` yields `./node_modules/pkg`), and containment against an
  absolute resolved path matches nothing, so the contract is refused everywhere
  for that run. Verified not to bite on the happy path — absolute,
  bare-relative, and `./`-relative invocations of the same fixture all bind
  identically — because the walk only returns a directory that exists, and
  `canonicalize` on an existing directory fails essentially only on permissions
  or a symlink loop. Absolutizing the spelled path without touching the
  filesystem would close it; it is left open because it has no reproduction
  outside those two conditions and the failure direction is a refusal.


## Open: contracts have no distribution mechanism beyond four local tiers

A contract reaches a project through exactly four channels: this checker's own
**bundled** artifacts, a **published** `solid-reactivity.json` inside the
installed package, a **local** file under `.solid-checker/contracts/`, and an
**explicit** `--contract` path. There is no fifth. There is no registry, no
fetch, no shared corpus of reviewed contracts, and no way for one team's review
work to reach another project.

The consequence is the whole many-packages user story. A project importing a
dozen Solid-aware packages that this checker does not bundle has one path
available: generate a draft for each with `contract generate` and review each by
hand. Generation never promotes `inferred` evidence, so until that review
happens every one of those packages reports `unverified` and certifies nothing.
The ecosystem benchmark measures the generator against real packages, but its
output is not a corpus anyone can install — the reviewed contract for a popular
package has nowhere to live except inside the package or inside one project.

**A design now exists**: [rfcs/0001-contract-registry.md](rfcs/0001-contract-registry.md)
specifies a signed, content-addressed registry of reviewed contracts and one new
explicit command, `contract fetch`, that resolves against the installed artifact
and materializes the contract into the existing local tier for the consumer to
commit. It adds no discovery tier, no precedence rule, and no analysis-time
network access.

The two candidate directions recorded earlier — a registry, and shipping a
reviewed ecosystem corpus as additional bundled contracts — turn out to
**compose rather than compete**. The registry is where reviews live and are
governed; bundling is a release-time snapshot of its most-imported entries, for
zero-configuration coverage. The RFC's §8 covers the one hazard that creates: a
fetched contract lands in the local tier and would otherwise shadow a bundled
audited artifact.

What remains open is the implementation — none of `contract fetch`,
`contracts-lock.json`, the entry/index specification, signatures, or the
revocation path exists — plus the RFC's own unresolved questions, of which the
load-bearing ones are the trust-set bootstrap, reviewer key rotation and
revocation, whether verifier identity can be recorded in a schema-v1 contract at
all (the loader's unknown-field failure is the outright-malformed path, so the
field would hard-fail older clients). The artifact-keyed review transfer the RFC
named as a hard dependency is no longer open: `contract review --transfer-from`
carries a previous review's resolutions onto a regenerated contract for every
entrypoint whose runtime-module closure is byte-identical, so an upstream release
costs a re-review of the diff rather than of the package.

## How much of a real ecosystem contract is actually a claim (measured 2026-08-22)

> **Superseded 2026-08-23 for the unknown-claim figures.** The all-five
> whole-summary shape this section identifies as the dominant cause was a defect
> in the emitter's attribution, not a limit of the analysis. It is fixed; the
> re-measured numbers are in "[Closed 2026-08-23: the whole-summary unknown
> collapse](#closed-2026-08-23-the-whole-summary-unknown-collapse)" below. The
> per-family reasoning and the closure-note conclusions here still hold.

The ecosystem benchmark measured generation *reachability* only — whether a
contract was emitted — and "[Open: generation success is not contract
correctness](#open-generation-success-is-not-contract-correctness)" already
records that a 98% success rate says nothing about what the emitted documents
contain. A machine-verification scheme asks the question in between those two:
under a scheme where an unknown stays uncertifiable, **how clean is a typical
package's generated draft before anyone reviews it?**

That is now measured. `scripts/ecosystem-benchmark/lib/contract-content.mjs`
opens every emitted `solid-reactivity.json` and its sibling `.review.json`
before the probe's temporary directory is removed, and counts unknown claims by
domain, refused entrypoints, closure notes, and positive behavioral rows. The
outcome classes are untouched: the same 305-row/416-probe manifest, the same
403 complete / 6 partial / 7 failures, class-for-class identical to the previous
run (release binary, 600 s budget, 95.413 s wall). Full method and caveats in
[ecosystem-benchmark.md](ecosystem-benchmark.md#contract-content-how-much-of-an-emitted-contract-is-actually-a-claim).

**Headline, over the 409 probes that produced a contract (207 packages):**

- **300 / 409 probes (73.35%) are fully proven** — no unknown claim, no refused
  entrypoint, no closure note.
- **126 / 207 packages (60.87%)** are fully proven across every one of their
  probes.
- **5,415 / 8,113 exports (66.74%) are proven.** 2,698 carry an unknown.
- 11,013 unknown claims in total, but **2,077 of the 2,698 unknown exports are
  unknown in all five domains at once** — most of each domain column is the same
  exports counted five times.
- 7 entrypoints refused across 6 probes; 32 closure notes across 7 probes.
- Positive behavioral rows available to a future probe step: 1,636 callback
  executions, 1,200 return trees, 990 reactive reads, 275 owner requirements,
  98 async behaviors.

**Per-family highlights:**

- **Solid Primitives is genuinely clean, and it is most of the corpus.** 288 of
  the 409 contracts; **230 fully proven (79.86%)**, 88.37% of exports proven,
  zero refusals, zero closure notes. Corvu is comparable on a smaller base
  (23/28, 82.14%). The small-single-purpose-package shape is what the generator
  handles well, and it is also the shape most of the ecosystem actually is.
- **The dominant unknown cause is one summary shape, not one claim domain.** A
  function export the generator reaches but cannot analyze is emitted with all
  five domains as `{"status":"unknown"}`, and that single summary is then shared
  by every export matching it. `@kobalte/core@0.13.13` emits exactly one such
  summary and attaches it to 452 of its 610 export names — 2,260 of the corpus's
  11,013 unknown claims from one summary. `solid-recharts` (305 of 327 exports),
  `motion-solidjs` (329), `@tanstack/solid-db`, `@tanstack/solid-table` and
  `@solidjs/router` are the same shape. The one large exception is
  `@solidjs/web@2.0.0-rc.1`: 188 unknowns, all `reactiveReads`, the other four
  domains fully claimed.
- **TanStack's unknowns are NOT its options-object callback pattern.** This was
  the expected answer and the data refuses it: 318 of TanStack's 322 unknown
  exports are the all-five whole-summary shape, and only 3 exports in the entire
  family carry a `callbacks`-only unknown. Hand-checked against two real
  contracts: `@tanstack/solid-query@5.101.4` on `solid-js@1.9.14` emits 57
  exports with exactly 3 unknowns (`useQuery`, `useInfiniteQuery`,
  `replaceEqualDeep`, all `callbacks`), while `@tanstack/solid-query@6.0.0-rc.0`
  on `solid-js@2.0.0-rc.1` emits 57 exports of which 37 are unknown in all five.
  Both declare the same non-standard `"@tanstack/custom-condition":
  "./src/index.ts"` branch pointing at TypeScript source; in 5.x that branch
  still yields real `reactiveReads` rows, in the 6.x prerelease it yields the
  whole-summary sentinel. The family's own numbers are unremarkable once that
  shape is set aside: 33/50 contracts fully proven, 84.84% of exports proven.
- **Official Solid is the worst-looking family (6/23 fully proven) for a reason
  that is not unknowns**: it owns 29 of the corpus's 32 closure notes. Its
  contracts largely make claims; they just cannot be bound to the bytes they
  describe.

**What this implies for machine-verified contracts**
([rfcs/0002-machine-verified-contracts.md](rfcs/0002-machine-verified-contracts.md),
forthcoming):

- A scheme that keeps unknowns uncertifiable does **not** start from a blank
  page. Three quarters of generated contracts already carry no unknown at all,
  and two thirds of all exports are claimed — the verification surface is real
  work, not an empty set.
- The work is extremely unevenly distributed. Roughly ten package/target pairs
  produce most of the corpus's unknown claims, and each of them concentrates in
  a single all-five summary. Closing that one shape — not five separate domain
  analyses — is what would move the number.
- **Closure notes, not unknowns, are the harder blocker.** An unknown is an
  honest uncertifiable result a consumer can route around; a closure note means
  the contract cannot be byte-attested at all, so no amount of verification
  binds it to an artifact. 7 probes and 32 notes when this section was written,
  29 of them in Official Solid. **Re-measured on the corpus after attestation
  (2026-08-24): 6 probes and 22 notes — 5 closure notes on 2 probes and 17
  attested closure notes on 5 — with 21 of the 22 still in Official Solid.** Both
  kinds still block promotion, so the blocker did not shrink by a third; what
  changed is that 9 of the notes were asset-import gaps that the analyzing
  program's own module list showed do not exist. The full account, including the
  two note classes that turned out not to disappear, is in "[The runtime-module
  closure is walked, not
  attested](#the-runtime-module-closure-is-walked-not-attested-2026-08-22)".
- These figures are the **demand-insensitive upper bound on the work**, and
  should never be quoted as a defect rate. An unknown becomes a finding only
  when a consumer touches that surface; a package with 452 unknown exports costs
  a project nothing if it imports two proven ones. The benchmark has no demand
  model, so the number of unknowns a real project would actually hit is
  unmeasured and is almost certainly far smaller.
- **"Proven" here means "claimed", not "verified".** Every claim counted as
  proven is still `inferred` evidence below the SC9005 trust ceiling. A contract
  asserting that `map` never invokes its callback is counted fully proven by
  this measurement and is false — which is exactly the gap RFC 0002 exists to
  close, and exactly why this measurement is a floor on the verification work
  rather than an estimate of it.
- **Probe drivability is not measured.** The 4,199 positive behavioral rows are
  what a probe step would have to drive; no attempt was made to drive any of
  them, so how many are actually executable is the next open question.

## Closed 2026-08-23: the whole-summary unknown collapse

An unresolved proof obligation used to erase claims the analysis had already
proven, on exports that could not reach it. Two independent defects in
`emit_package_contract`
(`rust/crates/solid-facts-backend/src/main.rs`) compounded:

- **Every domain.** `ReactiveDispatchUnresolved` fell through
  `unresolved_claim_domains`' catch-all to all five claim domains. The
  obligation proves that the possible runtime implementations of a dispatch do
  not share one *reactive-read* summary. It says nothing about the callbacks the
  export invokes, its owner requirements, or its async behavior.
- **Every export.** Attribution read only the *innermost* function containing
  the obligation. An obligation inside an anonymous arrow, a named local helper,
  or a private cross-file helper matched no export, and the fallback marked
  every export of the entrypoint — including exports with no path to it at all.
  A third rung scanned every call in the project whose callee *text* equalled a
  missing contract export's name, or ended in `.` plus that name, which is the
  name-only matching the precision contract forbids.

**What replaced them.** `ReactiveDispatchUnresolved` now marks
`reactiveReads` and `returns` only. Attribution is a ladder — `joined`,
`enclosing-chain`, `identity-widening`, `reachability`, `fallback-all` — spelled
out in
[package-contracts.md](package-contracts.md#which-exports-an-unresolved-obligation-belongs-to).
The name-text scan is gone; the reachability rung asks the call graph, in
`rust/crates/solid-reactive-ir/src/attribution.rs`, and is used only when the
enumeration is provably complete.

**Why `returns` and not `reactiveReads` alone.** The returns description is
derived from the same resolved callee summary the dispatch invalidates, and it
does **not** fail closed on its own: a value produced by an unresolved dispatch
and placed in a returned object is described from the local accessor index,
which knows nothing about it, so a possibly-reactive property is published as a
certified-negative omission. `StructuredReturnUnresolved` is not the guard — it
fires only for a shorthand property bound to an import with no project
declaration, an orthogonal condition. The
`unresolved-dispatch-domains-control` fixture is the proof: with the dispatch
resolved the generator claims `returns.properties.value` is an accessor, which
is exactly the claim the unresolved variant cannot make. `callbacks`,
`ownerRequirements` and `asyncBehavior` are proven by passes that never consult
the dispatch and are kept.

**A third defect the fix exposed.** With the collapse gone, two conditional
branches that each *prove* a different `returns` stopped being merge-compatible,
and `mergeSummaries` refused the whole entrypoint over it. solid-js 1.9.14's
`Show` is that shape — it returns its `props` argument in the server build and a
memo accessor in the client build — and refusing discarded the other 147 exports
of its `.` entrypoint. The base now carries the unknown sentinel for the
divergent domain and the exact per-branch behavior is emitted as `variants`,
which is the same discipline the function already applied when either side was
unknown.

**Measured recovery** (full ecosystem benchmark, release binary, 600 s budget,
same 305-row / 416-probe manifest; before = the 2026-08-22 run recorded above).

> **The `after` column was measured before the soundness fixes below, and has
> been re-measured.** The adversarial review recorded in "[Closed
> 2026-08-23: under-marking in the attribution
> ladder](#closed-2026-08-23-under-marking-in-the-attribution-ladder)" found
> six ways an export whose behavior depends on an unresolved obligation was
> published with the domain omitted — an arrow-bound export invisible to every
> rung, an escape test that never saw an escape, a name-text join, a blanket
> discharge, and two false callback rows. Every one of them *lowered* the
> unknown-claim counts and *raised* the "exports proven" figure by certifying
> something that was not proven. The improvement recorded here is real in
> direction; part of its magnitude was that inflation. The `corrected` column
> was the current state when this entry was written and is now four measurement
> states old — see
> [ecosystem-benchmark.md](ecosystem-benchmark.md#headline-numbers-2026-08-24-eighth-measurement-state-release-binary-416-probes)
> for the current figures, including the outcome classes, which have moved since
> ("moved once and have not moved since" below was true until 2026-08-24).

| | before | after (superseded) | corrected |
| --- | --- | --- | --- |
| Probes fully proven | 300/409 (73.35%) | 304/409 (74.33%) | **288/409 (70.42%)** |
| Packages fully proven | 126/207 (60.87%) | 128/207 (61.84%) | **111/207 (53.62%)** |
| Exports proven | 5,415/8,113 (66.74%) | 6,520/8,320 (78.37%) | **6,095/8,358 (72.92%)** |
| Exports unknown in ALL five domains | 2,077 | 492 | **527** |
| Unknown claims, total | 11,013 | 4,898 | **5,903** |
| Probes with at least one unknown claim | 102 | 99 | **116** |
| Entrypoints emitted / refused | 847 / 7 | 850 / 4 | **850 / 4** |
| Positive behavioral rows | 4,199 | 5,545 | **5,005** |

Outcome classes moved once and have not moved since: 403 success / 6 partial /
7 failure became **406 / 3 / 7**, and the corrected run is package-for-package
identical to that. The three probes that moved (`@kobalte/core@0.13.13`, both
`solid-js@2.0.0-rc.1` probes) are entrypoints the conditional-merge refusal
used to discard and now emits. No probe has regressed in any of the three runs.
The export total grew from 8,113 to 8,320 because those three entrypoints now
contribute exports, and to 8,358 when the declaration-sibling gate changed
which modules an entrypoint enumerates.

The `after` → `corrected` movement was attributed by re-running the full corpus
twice against the current binary, once with and once without the
conditional-merge one-sided fix: the engine soundness rounds account for 15
fully-proven probes and 316 proven exports across 48 probes, and the merge fix
for 1 probe and exactly 109 exports (108 `returns`, 1 `asyncBehavior`) across 8.
The per-cause table is in
[ecosystem-benchmark.md](ecosystem-benchmark.md#how-the-earlier-states-moved-history),
under the second → third transition; the heading it used to link to is renamed on
every measurement pass, so it points at the stable history section instead.

**Fixtures.** `fixtures/package-contracts/unresolved-dispatch-attribution` pins
the `joined` and `enclosing-chain` rungs and the surviving `callbacks` claim,
with a sibling export that must stay fully proven;
`unresolved-dispatch-domains-control` pins the claim the unknown replaces;
`unresolved-dispatch-reachability` pins the call-graph rung across files;
`unresolved-contract-export-attribution` pins that a missing contract export
still keeps all five domains, for that export only, through exact symbol
identity rather than a name scan. All four are in the
`scripts/contract-corpus.mjs` pin list.
`unknown_claim_attribution_markers_reach_the_review_plan` in
`rust/crates/solid-facts-backend/tests/contracts_process.rs` pins the stderr
marker seam on real bytes from both processes.

**Still fail-closed after this.**

- The `fallback-all` rung survives. An obligation whose containing function
  cannot be identified at all still marks every function export. It is now
  observable — the review plan says `fallback-all` — rather than silent.
- ~~The reachability rung is conservative in its escape test~~ — **false when
  written; corrected below.** The test accepted any reference inside an
  `ExportFact.span`, which for a declaration export covers the whole body, so
  none of the three escapes it claimed to catch were caught.
- ~~The `export_names_for_function` join reads `function.name` or
  `method_name`~~ — **the consequence stated here was wrong, and in the unsound
  direction.** An arrow export did not "reach a lower rung"; it made the
  reachability enumeration return an empty set, and nothing was marked.
  Corrected below.
- `ownerRequirements` is kept claimed across an unresolved dispatch. An
  implementation the analysis cannot select could call an owner-requiring
  primitive. That is the same gap every uncontracted external call already has,
  and narrowing it is a separate question from this one.

## Closed 2026-08-23: under-marking in the attribution ladder

An adversarial review of the entry above, driven by hand-written packages
rather than by the fixture corpus, found that the ladder's *fail-closed*
guarantees were not guarantees. Six shapes published an export whose behavior
depends on an unresolved obligation with the affected domain simply **omitted**
— a certified negative. Over-marking is imprecise; this direction is unsound,
and every fix below moves toward failing closed.

Each was reproduced against the debug binary before and after, and each has a
regression fixture in `scripts/contract-corpus.mjs`'s pin list.

| Shape | Before | After | Fixture |
| --- | --- | --- | --- |
| `export const X = props => Panel(props)` reaching a private helper | nothing marked | `X` marked | `arrow-export-attribution` |
| Private helper handed to a callee (`apply(Panel, …)`) | only the *caller* marked; the escaping export certified | every export marked (`fallback-all`) | `escaping-private-helper` (`./argument`) |
| Private helper returned (`return Panel`) | same | same | `escaping-private-helper` (`./returned`) |
| Private component rendered (`<Panel/>`) | nothing marked at all | every export marked | `escaping-private-helper` (`./rendered`) |
| Private `Render` beside an unrelated exported `Render` | the *unrelated* export marked; the reaching one certified | the reaching export marked, the unrelated one clean | `export-identity-join` |
| `export { Panel, Panel as Root }` | `Panel` marked, `Root` certified | both marked | `export-identity-join` |
| Export forwarding into an exported parameter-member helper | discharged wholesale; `reactiveReads` omitted | discharged only where the row is published | `parameter-member-forwarded` |
| Callback invoked behind a closure handed to a helper, or behind a returned closure | `execution: "inline"` | no row; `callbacks` sentinel | `callback-execution-boundary` |

**What replaced them.**

- `export_names_for_function` names a declaration through
  `solid_reactive_ir::function_binding_name`, the same helper the IR uses, so
  arrow bindings resolve. It now distinguishes *undecidable* (`None`) from
  *decided: private* (`Some(vec![])`), and the reachability rung propagates the
  first instead of reading it as the second. `Some(vec![])` is itself claimed
  only when every export of the entrypoint joined to an identity or a symbol.
- The escape test accepts an export **specifier** span, never containment in an
  `ExportNamedDeclaration`'s span.
- The name-text branch is deleted. It survives only in the whole-project mode
  with no entry file, where `exports` is keyed by the project-wide export name
  and no identity channel exists.
- The call graph answers for an obligation filed *at* a declaration span, not
  only for one inside a body. Without that, every exported-helper obligation
  went to `fallback-all`.
- The `exported-parameter-member-dispatch` string comparison is replaced by
  `parameter_member_row_covers`, which asks whether the exports the ladder
  resolved the obligation to actually publish the `parameter-member` row. The
  covering channel is real (`parameter-member-read` /
  `package-parameter-member-consumer` pin it) but does not survive a hop.
- A zero-export decision emits its marker, and
  `generate-package-contract.mjs` renders it as a review-plan note. Silence was
  how a truncated reach enumeration looked from the outside.

**`ReactiveSourceUncaptured` now invalidates `returns` as well** (R7). The
reads-only claim was never tested: every shape that reaches the arm during
generation also raises the package's missing-contract-export obligation, which
erases all five domains, so the narrower claim was masked rather than proven.
`fixtures/package-contracts/uncaptured-source-return` records that, and the arm
fails closed by construction rather than by proof. Reads-only can be restored
only by a shape that fires the arm *alone*, which nobody has constructed.

**Still fail-closed, or still wrong, after this.**

- **A sibling `.d.ts` for an internal module truncates the reach enumeration
  (under-marking).** *Closed as unsound; see the 2026-08-23 entry below — it is
  now a widening, not a certified negative.* With `channel.d.ts` beside
  `channel.js`, the caller edge from `index.js` into `channelFor` is lost: the
  graph reports `complete` while having enumerated only the helper itself, and
  the obligation attributed to no export. Repro:
  `fixtures/package-contracts/parameter-member-forwarded` with a `channel.d.ts`
  added — `forwarded` went from unknown to certified.
- **A provably unused callback parameter still opens the sentinel instead of
  emitting the honest negative** (the remainder of R6). In
  `callback-execution-boundary`, `schedule` never uses its second parameter, so
  the truthful answer for `Escaping` is *no callback row*. Proving it needs an
  interprocedural "this parameter is never invoked" summary the generator does
  not compute; the fail-closed sentinel is what is emitted instead.
- ~~**A re-exported helper called from the same entry file joins to nothing.**~~
  *Not reproducible against this entry's own code.* Re-tested 2026-08-23 in all
  four spellings — `export { x } from "./m.js"` before or after
  `import { x } from "./m.js"`, `import` then bare `export { x }`, and
  `export * from "./m.js"` — and every one resolves the obligation to both
  published names. The identity join in this entry closed it; the residual was
  recorded from a run that predated it. It is pinned now by
  `fixtures/package-contracts/entry-reexport-identity`, so it cannot silently
  come back. The shape that *did* still fail was the same source with a
  `channel.d.ts` sibling, which is the `.d.ts` class above and not a second one.
- `fallback-all` survives, and the three escape shapes above now reach it. That
  is deliberate: nothing in the package proves the escaped helper is
  unreachable from a sibling export's caller.
- The `runtime_execution` rung can still return `inline` for a call nested
  inside a proven-inline scheduler that is itself inside an unproven closure.
  The fold looks at the enclosing argument chain, not at the schedule of the
  outermost call in it. Not observed in a repro; not narrowed here.

## Open: nominal class-method dispatch could discharge these obligations (2026-08-23)

The obligations the entry above learned to *attribute* precisely are mostly
obligations that should not exist. The dominant real-world shape, from the
ecosystem benchmark's own samples, is a parameter typed as a class or interface
whose method is then invoked: `getQueryCache()` on a `QueryClient` parameter,
`.toLowerCase()` on a `string`. `member_value_symbols_at` finds no *value*
implementation for a nominal type's method, so the dispatch is unresolved and
the enclosing export loses its reactive-read claim — even though the callee's
own resolved declaration names exactly one method body.

Resolving the method through the callee's declaration would discharge the
obligation outright rather than re-attributing it, and it composes with the
attribution ladder: fewer obligations reach the ladder at all, and the ones that
do are the genuinely ambiguous ones.

It is **not** implemented, because it needs a soundness argument this change did
not attempt:

- **Subclass existence reopens it.** A parameter typed `Base` can receive a
  `Derived` that overrides the method. Selecting `Base`'s body then certifies an
  implementation that does not run. The argument has to be closed over the
  analyzed program — an override declared anywhere in the closure must reopen
  the obligation — and "closure" here is the package plus its consumers, which
  for a published package is open by construction. `--program-boundary closed`
  is the existing lever, and whether it is enough is exactly the open question.
- **The standard library is the easy half.** `.toLowerCase()` on a `string` has
  no user-declarable override, and Type Facts already marks standard-library
  declarations (`resolved_callee_call(..).declaration.standard_library`). That
  subset may be dischargeable without the subclass argument at all.
- **A Type Facts signal may be required.** Deciding "this method has exactly one
  implementation reachable at this call site" is a type-system question, not an
  AST one, and the current fact set does not answer it.

## Open: probe discovery contradicts bundled Solid negative claims (2026-08-23)

**`solid-v2/solid-js` was resolved on 2026-08-23 and
`solid-v2/@solidjs/web` on 2026-08-25; `solid-v1/solid-js` remains open.** The
resolutions and the remaining exact worklist are recorded below.

The Stage-1 probe driver (`contract probe`, RFC 0002), run with discovery
against the bundled `solid-js@2.0.0-rc.0` contract, reports 65 incompleteness
findings: exports whose summaries state no `callbacks` row — which schema v1
reads as the certified negative "never invokes a caller-supplied callback" —
while the installed release observably invokes a function argument. A sample
was verified by hand against the real package: `untrack`, `flush`,
`createSignal`, `merge`, `latest`, `isPending`, `flatten`, and `children` all
invoke a caller-supplied function. The declared behavioral probes themselves
all pass (89/95 driven, 0 failed); only the negatives-by-omission are
contradicted.

The same run against the other bundled artifacts reports 97 incompleteness
findings for `@solidjs/web@2.0.0-rc.0` (40 distinct
`(entrypoint, export, parameter, execution)` rows over 13 export names), 33 for
`solid-js@1.9.14` (13 rows over four entrypoints), and **none** for
`@solid-primitives/scheduled@1.5.3`, whose exact-version review holds.

**Determined (2026-08-23, by reading the consumption path; no experiment run).**
Consumers are exposed. The dialect shadows a contract for exactly the pairs
`(dialect-owned module, name in the dialect's primitive table)` and nothing
else, so every other export of a `solid-js` contract is consumed normally —
negatives included.

- Loading applies no dialect filter at all. `load_package_contracts_reporting`
  (`rust/crates/solid-facts-backend/src/diagnostics.rs:915`) fills one
  `HashMap<package_name, PackageContract>` from four tiers — bundled
  (`:938-976`, via the `include_bytes!` table at `:804-835`), package-published
  (`:977-987`), project-local `.solid-checker/contracts/<pkg>/` (`:988-998`),
  explicit `--contract` (`:999-1016`) — and hands the certifiable subset to the
  IR at `:292-297`. The bundled `solid-js` contract is preloaded on the hot path
  (`rust/crates/solid-facts-backend/src/main.rs:174`) and applies even with no
  `node_modules`, because `contract_matches_manifest` is `is_none_or`
  (`diagnostics.rs:1292`).
- The evidence gate does not help here. `contract_evidence_is_certifiable`
  (`diagnostics.rs:1116`) plus `claims_are_certifiable`
  (`rust/crates/solid-reactive-ir/src/lib.rs:1297`) admit `verified`, and both
  bundled `solid-js` documents declare `"kind": "verified"`.
- The shadow is `native_vocabulary_outranks_contract`
  (`rust/crates/solid-reactive-ir/src/contracts.rs:192`) —
  `dialect.owns_module(module) && dialect.declares_primitive(imported)` —
  applied at the namespace-member (`contracts.rs:413`), named/default
  (`:506`) and re-export (`:591`) binding sites, each of which `continue`s
  without creating a `ResolvedContractBinding` at all. `owns_module` and
  `declares_primitive` are `rust/crates/solid-dialect/src/lib.rs:1054` and
  `:1049`; `modules()` is four specifiers for v1
  (`rust/crates/solid-dialect/src/solid_1x.rs:105`) and thirteen for v2
  (`rust/crates/solid-dialect/src/solid_2.rs:110`), and the primitive tables are
  the `primitive()` matches at `solid_1x.rs:122` and its v2 counterpart.
- The suppression itself is `interproc.rs:1216-1218` — `if
  contracts.callbacks.contains_key(symbol) { continue; }` — which skips the
  `contract_generation_obligations` push at `:1230-1240`. The map is filled
  with no emptiness guard (`source_discovery.rs:1365-1367`), and an omitted
  `callbacks` field deserializes to `Known(vec![])`, not `Unknown`
  (`lib.rs:1108` `#[serde(default)]` + `lib.rs:1022` `Default for
  ContractClaim<T>`). An empty list is therefore `contains_key == true`, and the
  obligation is skipped. That *is* the negative claim taking effect.
- Reachable today: ~11 negative-callback exports in v1's `solid-js.json` that
  the v1 table does not name (`enableExternalSource`, `requestCallback`,
  `createComponent`, `observable`, `cancelCallback`, …), all 48 in
  `solid-js/web`, `createRenderer` in `solid-js/universal`, and every export
  under a subpath `modules()` omits — `solid-js/web/storage`
  (`provideRequestEvent`), `solid-js/jsx-runtime`, `./jsx-dev-runtime` — plus
  ~24 in v2's (`createComponent`, `flatten`, `runInServerComponentScope`,
  `ssrScope`, `isWrappable`, `storePath`, …). Several of these demonstrably do
  invoke a caller-supplied function.
- A second channel bypasses the shadow entirely: `bundled_returns`
  (`source_discovery.rs:1286-1305`, read at `:208`, `:706`, `:764`, `:907`) is
  keyed on the **export name**, not a resolved symbol, so a `solid-js`
  contract's `returns` claims reach dialect primitives. It reads only `returns`,
  so it does not widen the negative-callback exposure, but "the dialect fully
  shadows solid-js contracts" is false in general.

Consequence for RFC 0002 Stage 2: the incompleteness blocker in `contract
verify` is *not* the only impact. It does fail closed — a regenerated
`solid-js` contract cannot be mechanically promoted while discovery contradicts
it — but the already-shipped bundled artifacts are `verified` and live, so the
wrong negatives are consumed now, independently of Stage 2. Resolution is
unchanged in shape and now clearly not optional: add the missing callback rows
to the bundled contracts (and their probes), or state the negative honestly as
`{"status": "unknown"}` for the exports discovery contradicts. A carve-out for
"dialect-owned modules" is not available as an answer, because the contradicted
exports are precisely the ones the dialect does *not* own.

### Resolved for `solid-v2/solid-js` (2026-08-23)

Every one of the 65 findings is now either a row proven from the installed
release's own implementation or the unknown sentinel. `contract probe` with
discovery reports **0 incompleteness and 0 failures** against the artifact
(124 claims, 113 passed, 11 undriven), and `make contract-conformance` is green
with every new claim behaviourally probed in each mode it is stated for. The
per-export audit — source citation to row — is in the commit that carries this
change; the shape of the answer is:

- **Identical in both builds, `callbacks[0]=inline`:** `untrack`, `latest`,
  `isPending`, `flatten`, `createComponent`, `createRevealOrder`,
  `runInServerComponentScope`.
- **Browser-only callback:** `flush`. The server build is
  `function flush() {}` — no declared parameter, empty body — so its variants
  keep a *proven* negative rather than inheriting the browser row.
- **Client tracks, server runs it once:** `children`, `createSignal`,
  `createOptimistic` (`0=tracked` / `0=inline`), plus `callbacks[0]=inline`
  added to the server variants of `createMemo`, `createEffect` and
  `createRenderEffect`, whose browser rows were already there.
- **Two slots:** `repeat` (`0=tracked` on the browser, `1=inline` in both — a
  row callback runs with the listener cleared and a signal it reads never
  re-runs it), `createLoadingBoundary` (`0=tracked, 1=tracked` on the browser;
  `0=inline, 1=deferred` on the server), `createErrorBoundary`
  (`0=tracked, 1=tracked` on the browser; both deferred on the server, where
  neither argument is referenced outside the thunk the export returns).
- **Sentinel:** `merge`. It is variadic and wraps *every* function argument in
  a memo, so any finite `callbacks[]` certifies a false negative at the first
  parameter past it. `{"status": "unknown"}` is the only honest schema-v1
  encoding, and `scripts/check-bundled-contracts.mjs` now reads that value
  instead of throwing on it.

Two **stated** claims were falsified on the way, not merely incomplete, and are
corrected with them: the server variants of `solid-js`'s `createRenderEffect`
and of `@solidjs/web`'s `effect` said `callbacks[1]=deferred`, while
`serverEffect` invokes `effectFn` synchronously inside the call
(`solid-js` `dist/server.js:668-729`; `@solidjs/web`'s server `effect` is
literally `(fn, effectFn, options) => createRenderEffect(fn, effectFn, options)`).
Both now say `inline`. The old conformance body could not see the difference —
it asserted only that the apply did not *re-run* — so the bodies for those two
slots now assert that it ran inside the call.

`fixtures/reactive-ir/bundled-contract-callback-consumer` settles the consumer
half end to end, which reading the code could not: with the certified-negative
contract, a `doubled()` read inside a callback passed to `flatten` produced no
finding at the call site; with the row it produces `SC1001` there, and the same
call from compiler-tracked JSX stays clean. `createEffect` beside it is the
dialect-shadowed control and does not move. Only that fixture's snapshot
changed — no existing finding moved, because every other contradicted export a
fixture touches is in the dialect's primitive table.

### Resolved for `solid-v2/@solidjs/web` (2026-08-25)

The 40 discovered rows are no longer certified negatives. The exact RC.0
runtime was read per export condition and subpath, rather than treating the
root, JSX runtimes, frames server and storage entrypoints as one implementation:

- `applyRef`, `createComponent` and `untrack` are inline; the browser
  `getNextElement` template is inline while the root server binding is a
  throwing client-only stub.
- `effect` and `memo` now carry their missing server rows (`inline`) and their
  JSX-runtime re-exports use the same condition-aware summaries. Root-server
  `dynamic` is eager/inline; the JSX runtime's lazy memo makes its server
  source deferred; browser builds track it.
- Root `renderToString` invokes parameter 0 only on the server. Server
  `ssrElement` invokes function-valued props (parameter 1) and children
  (parameter 2) inline; recording parameter 2 goes beyond discovery's current
  two-parameter sampling bound.
- `frameTransformResult` parameter 1, `serverComponentResponse` parameter 0,
  and server `provideRequestEvent` parameter 1 are inline. The storage browser
  build throws before the callback and therefore retains a real negative.
- `mergeProps` is variadic and memoizes every function source, so schema v1's
  `{"status":"unknown"}` is used instead of a finite list that would certify
  a false negative at the next parameter.

The flat review contract also uses `unknown` wherever a single name-level row
cannot represent these condition splits. The bundled artifact retains exact
variants. Runtime verification uses three isolated workers because importing
the root, JSX and frames runtimes together changes renderer-wide trace hooks
and contaminates later observations. `contract probe` against the exact
package with `--no-environment-shim` reports 539 claims, 515 passed, 24
undriven, **0 failed and 0 incompleteness**; the declared bundled-contract
probe gate drives every new row without fake DOM globals.

`fixtures/reactive-ir/bundled-contract-callback-consumer` pins the consumer
effect with `@solidjs/web`'s exact browser `applyRef` declaration: an indirect
reactive read passed as its callback reaches the untracked component call,
while the same call in compiler-tracked JSX stays clean. TypeScript accepts
both against the published signature.

### Resolved for `solid-v1/solid-js` and `@solid-primitives/debounce` (2026-08-25)

The exact `solid-js@1.9.14` audit now closes the 1.x certified-negative callback
gap. `createComponent`, `requestCallback`, `getNextElement` and `use` have their
missing rows. `requestCallback` has a dedicated valid-function probe, so generic
discovery no longer schedules a non-function and later crashes the worker from
the `MessagePort` loop. The no-environment-shim probe now reports **374 claims,
322 passed, 52 undriven, 0 failed and 0 incompleteness**.

Two exports deliberately remain unknown rather than being forced into false
finite rows:

- `createResource` overloads parameter 0 as either a tracked source or the
  deferred fetcher, which schema v1 cannot select by overload.
- `mergeProps` is variadic and memoizes every function-valued source, so any
  finite callback list would certify the first omitted parameter incorrectly.

The 1.x composer now preserves unknown callback sentinels and the probe gate's
evidence shape. `scripts/check-bundled-contracts.mjs` also treats an unknown
return as the absence of a positive return-kind claim rather than generating
the meaningless string `returns=undefined`.

`@solid-primitives/debounce@1.3.0` is the next exact Solid 1.x package contract.
Both its named and default exports defer callback parameter 0 without an owner,
and creation requires a cleanup-capable caller owner. Those facts are fully
representable in schema v1 and are probed in all four runtime modes without DOM
shims. The consumer fixture uses the published signatures, is accepted by
TypeScript, and is fully certified with no findings.

### Resolved in the follow-up

- **Generic callback-result returns are now representable in schema v1.** A
  backward-compatible `callback-result` return names the callback parameter
  whose exact invocation result is returned. The consumer instantiates only an
  exact local callback result and fails closed otherwise. The reviewed
  `@solid-primitives/rootless@1.5.4` contract and its exact runtime dependency
  closure are now bundled; the consumer fixture proves that a memo returned
  through `createSubRoot` remains a reactive source.
- **Returned callback-result functions are now representable without a generic
  callable guess.** `callback-result-function` states only that invoking the
  returned function yields the named factory callback's result. The consumer
  follows an exact same-file binding to the contracted factory call and fails
  closed for aliases, assignments, cross-file values, and callbacks without a
  local body. The singleton-root and root-pool exports now preserve memos
  returned by their factories.
- **Solid 2 callback timing now has one condition-aware owner.** The browser
  fallback tables match the observed `repeat` and boundary behavior. For an
  exact installed Solid release, its selected package-contract variant may
  overlay callback timing on native vocabulary, while ownership, returns,
  async behavior, and all other primitive facts remain native. Node/server
  behavior therefore no longer has to be flattened into the browser table.

The honest residue is now the flat callback list: it cannot state Solid 1.x
`createResource`'s overload-dependent slots or `mergeProps`'s variadic callback
tail. The native Solid 1.x dialect remains call-shape exact for both, so this
is contract portability/audit duplication rather than a consumer correctness
gap.

## Closed 2026-08-26: declaration-bound imports join their exact runtime target

The `.d.ts` residual first closed fail-closed on 2026-08-23: when TypeScript
bound an import through a declaration sibling while the package runtime loaded
the adjacent implementation, the generator could no longer certify a negative
from the disconnected call graph. That safe repair widened the whole
entrypoint. The exact generator-owned runtime edge now restores the missing
identity join, so only exports proven to reach the implementation obligation
are made unknown.

**Mechanism.** `index.js` writes `import { channelFor } from "./channel.js"`.
TypeScript resolves that specifier to `channel.d.ts` whenever one exists beside
`channel.js` — a declaration file wins over an adjacent implementation in every
resolution mode `analyzeTarget` configures, and `closureOf` still seeds the
runtime `channel.js` as a root, so the program holds *both* files as unrelated
modules. `runtimeIdentity` is minted from the symbol's `ValueDeclaration`
(`durableRuntimeRefFor` → `runtimeID` in the pinned solid-ts-facts), so the
call in `forwarded` carries `channel.d.ts`'s identity and `channel.js`'s
`channelFor` has no reference outside its own file. Three lookups then fail in
the same direction, all downstream of that one split:

- `all_function_call_sites`
  (rust/crates/solid-reactive-ir/src/indexes.rs:2077) resolves the callee symbol
  to the declaration, `functions_by_symbol` has no function for it — a
  `declare function` has no body and so no `FunctionFact` — and the caller edge
  is dropped.
- `compute_entered_only_through_calls`
  (rust/crates/solid-reactive-ir/src/attribution.rs:190) walks the same symbol's
  references, finds only the declaration name and the export specifier, and
  reports the entry set fully enumerated.
- `CallGraph::reach` therefore returned `complete: true` with `reaching` holding
  the helper alone, and `export_names_from_reachability` mapped that to
  `Some(vec![])` — *decided: no export reaches this* — so `forwarded` and
  `Isolated` were both published certified with `complete=true`, no marker
  degradation, and (before this) a zero-export review-plan note as the only
  trace.

**The exact seam.** The TypeFacts module graph still answers the declaration
binding; it must not guess that two similarly named files describe one runtime
module. The package generator already has the other exact answer: its static
runtime resolver selected `channel.js` for the literal `./channel.js` edge, and
that same closure seeded the analyzing program. It now passes those successful
`importer` / `specifier` / runtime-`target` triples to the native generator.
The backend accepts a redirect only when all of the following hold:

- TypeFacts confirms that exact importer/specifier edge resolved through a
  declaration file, without a configured-project `includedPath`;
- importer and runtime target canonicalize inside the installed package root;
- the exact import is a runtime-referenced named or default binding; and
- both the local binding and the same named export of the exact runtime target
  join to compiler entities.

A missing join changes nothing. Conflicting targets remove the redirect. An
incomplete TypeFacts module graph rejects the whole map. There is no adjacent
filename pairing, name-only matching, namespace substitution, or guessed
member dispatch.

**The safety net remains.** `module_surface_is_unaccounted`
(rust/crates/solid-facts-backend/src/main.rs) gates the reachability rung. A
reaching function that is *decided: not an export of this entrypoint*, is
published by its own module's export surface, and has no reference anywhere
else in the project by exact runtime identity or canonical symbol, cannot have
had its entry set enumerated: either its importers are outside the analyzed
file set, or they are inside it and bound to a different declaration of the
same module. `export_names_from_reachability` returns `None` for it, the ladder
falls to `fallback-all`, and the marker records `mechanism: "fallback-all"`.

The gate is deliberately not asked of an entrypoint export (its consumers are
answered by marking its own name — this is what keeps `forwarded` exact in
`parameter-member-forwarded` and `channelFor` exact on `./direct`) nor of a
module-private function (its entries are exactly what the graph enumerates —
this is what keeps `unreached-private-obligation`'s zero-export answer).

**Before/after**, `fixtures/package-contracts/parameter-member-forwarded` with
a `channel.d.ts` added:

| | fail-closed intermediate | exact join |
| --- | --- | --- |
| mechanism | `fallback-all` | `reachability` |
| `.:forwarded` | `reactiveReads`, `returns` unknown | `reactiveReads`, `returns` unknown |
| `.:Isolated` | `reactiveReads`, `returns` unknown | independently proven |
| `./direct:channelFor` | exact `parameter-member` row | unchanged |

`Isolated` reaches nothing and is no longer charged for `channelFor`; the
redirect also restores the call graph for other exact consumers of the same
symbol. If any proof above is absent, the 2026-08-23 safety gate still falls to
`fallback-all`, so precision recovery never substitutes for refusal.

The full 416-row authority comparison found 393 rows with claim summaries in
both runs and 19 structural changes: claims **+47**, driven **+2**, passed
**+2**, undriven **+45**, failed **unchanged at 8**, and incompleteness
**unchanged at 589**. Generated unknown-bearing exports increased by 55 and
promoted unknown-bearing exports by 53 because restored call edges exposed
obligations the declaration split had hidden. This is mostly a soundness gain;
the exact narrowing is still visible where the graph supports it—for example,
`@solid-primitives/range@0.2.5` gained one driven passing claim and lost two
generated unknown-bearing exports. No common row's verified/refused outcome
changed. The raw headline moved from 286/110 to 284/109 only because three
previously measured TanStack rows failed npm installation in the later run.

Pinned by `fixtures/package-contracts/declaration-sibling-reach` (the split,
including the `./direct` control that must stay exact) and
`fixtures/package-contracts/entry-reexport-identity` (the same source with
identity intact, which must keep its three-way answer). Both are in
`scripts/contract-corpus.mjs`; the corpus is 24 packages.

**Still fail closed.** Open dynamic imports, unresolved or namespace bindings,
ambiguous runtime exports, incomplete module facts, package-external targets,
and conflicting declaration roots receive no redirect. They retain the
existing unknown or whole-entrypoint refusal. The bridge covers only static
runtime edges the generator actually used to seed this package analysis.

## Closed 2026-08-23: `contract verify` certified what no run had observed

A second adversarial review, this time of the RFC 0002 pipeline rather than of
the engine, found that `solid-checker contract verify` could reach
`evidence.kind: "verified"` on a contract **none of whose claims any probe had
observed**. Nine defects, in the Node commands under `packages/cli/scripts/`.
Each is closed; the design decisions the closures required are recorded in
[RFC 0002's Amendments section](rfcs/0002-machine-verified-contracts.md#amendments)
rather than silently applied.

**1. Stale `probed` markers (critical).** A `probed` row marker is a durable
property of the *document*. `writeProbeEvidence` never refreshed or removed
one, and `collectBlockers` never asked whether the *consumed* report witnessed
it. So probe-healthy → probe-observes-nothing → verify certified every marker
the healthy run had left behind. Closed on both sides: a `--write` now
supersedes the marker of any claim it re-drove that did not pass (reported as
`superseded`), and verification converts any marker its own report does not
witness — a passing claim of the same identity covering at least the marker's
modes — recording it under `staleProbedMarkers`. Conversion rather than a
blocker, because an unwitnessed marker and an absent one are the same state
from this run's point of view, and because blocking would make a legitimate
`--modes` narrowing unable to verify anything rather than able to verify less.

**2. `kind` certified from zero observations (critical).** `kind` is the one
claim schema v1 has no sentinel for, so `convertUnconfirmedClaims` exempted it
— relying on "a runtime kind that disagrees is a failed probe", which is
vacuous when the probe observed nothing. An import that threw, a missing
export, a crashed session, or a `--modes` narrowing all produced zero
observations and a verified contract. A `kind` claim not probed-passed in every
stated mode is now uncertifiable, with the deliberate consequence that a
package this checker cannot import cannot be machine-verified at all. It was a
document-level blocker when this was written; since
[RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md) it refuses the
*entrypoint*, and the document only when no entrypoint would certify anything —
see
"[`kind` has no unknown form, and 64 refusals turn on
it](#open-kind-has-no-unknown-form-and-64-refusals-turn-on-it-2026-08-23-re-measured-2026-08-24)" for
the measurement and the staged plan. Also in
this slice: discovery probes now run for `value` summaries, which are the
maximal negative claim and were exempt from their own falsifier; and the probe
report's family labels were realigned with what verification does — see 3.

**3. `because` destroyed by the plan rewrite.** A contract document carries no
generation-time attribution, so re-deriving the review plan from the verified
bytes threw away the only record of why each claim is unknown. Items now
inherit the prior plan's `because` by id, and every sentinel the verification
created gets a `because.conversion` mirrored from the sidecar.

**4. `--no-discovery` was invisible.** The probe report did not record it, so
`<contract>.verify.json` listed the incompleteness blocker as checked when
nothing had looked. The report records `discovery: {enabled, parameters}`, and
verification refuses a report with discovery disabled — or with no discovery
state at all.

**5. `returns=accessor` was transitively satisfiable.** The observation plants
its signal read inside the claimed callback, so `(cb) => () => cb()` passed.
The observation now also measures caching within one tracked read; a
forwarding closure is `undriven`, and a real `createMemo` accessor still passes
(proven against an installed `solid-js@1.9.x`). **An uncached derived accessor
— 1.x `mapArray`'s plain tracked function is the real example — now lands
undriven too**, and its `returns` domain converts. That is the safe direction
and it is a real precision loss: recovering it needs a distinguisher that
separates "recomputes per read because it is a plain tracked function" from
"recomputes per read because it is not an accessor at all", which no counter
available to a generic driver does.

**6. Fabricated call counts.** The worker stamped a per-probe-type constant, so
`evidence.calls` was a table lookup and a `deferred` claim recorded two calls
for one invocation. Counted now.

**7. Inherited-summary variants dodged conversion.** The walk converted an
inherited summary's five top-level domains and then descended into `variants`
on their own evidence, so the exact per-environment claims — the ones a
consumer selects — passed through certified. The inheritance travels with the
walk now.

**8. Summary-level markers outlived their claims.** An export summary's own
`probed` marker is computed from its `callbacks[]` rows and top-level
`returns`. Once those are converted (verify) or deleted (a review certifying
them absent), the marker asserted an observation of claims the document no
longer contained, and any row without evidence of its own inherited it. Both
paths recompute it.

**9. `mergeSummaries` one-sided divergence.** `left.returns ?? right.returns`
handed the environment-unaware base one branch's proven claim when the other
branch proved *none* — and in a proven summary an absence is a certified
negative, not an absence of knowledge. One-sided presence is a divergence now,
so the base is the sentinel and the exact per-branch claims stay in `variants`.
Merge-produced sentinels also carry a `because.divergences` on their review-plan
item, naming the branches and the shape of the disagreement; a merge was the
second emitter of the sentinel and the silent one.

**Fixtures.** `fixtures/package-contracts/conditional-returns-divergence` pins
the one-sided shape and `conditional-returns-divergence-both` the
both-present one, with a `Steady` negative control in each that must stay
unconditional. Both are in `scripts/contract-corpus.mjs`, which closes the
corpus-coverage gap the review flagged: `mergeSummaries` and `mergeClaimRows`
were not executed by the corpus at all. Everything else in this list is pinned
by unit cases in `scripts/contract-probe.test.mjs`,
`scripts/contract-verify.test.mjs`, and
`packages/cli/test/contract-attribution-notes.test.mjs`.

**Still fail-closed or unresolved after this.**

- **An inherited summary's *omitted* domains still pass through as certified
  negatives.** Conversion covers every domain the summary carries; a domain it
  omits is another package's proven negative, and schema v1 has no way to say
  "this negative is inherited". The reviewed tier is the only answer today.
- **An uncached derived accessor's `returns` claim is now unprovable** (5
  above).
- **A claim this run did not attempt keeps whatever marker it had** on
  `--write`. That is deliberate — the command reports what it drove — and it is
  safe only because verification independently refuses to certify an
  unwitnessed marker. The two checks are load-bearing together.
- **`--modes` narrowing can never verify.** Every stated mode must carry a
  passing `kind` observation, so a narrowed run blocks rather than converting.
  Deriving a package's genuinely applicable modes is RFC 0002 unresolved
  question 8.
- **`mergeClaimRows` still unions the multi-row domains** (`callbacks`,
  `reactiveReads`, `ownerRequirements`) across branches, so a row proven in one
  branch is published in the base even where the other branch proves the export
  invokes nothing. Unlike `returns`, a union there is not obviously the
  dangerous direction — it over-claims that a callback runs rather than that it
  does not — but it is the same one-sided shape and has not been argued
  through. Left open deliberately; fixing it without measuring the ecosystem
  cost would be the same guess in the other direction.

## Generated contracts contradicted by the runtime probe (2026-08-23)

The corpus-wide machine-verification measurement
(`benchmarks/ecosystem/verification-report.md`) attributed a root cause to each
of its 210 refusals. Two of them were defects in `contract generate` itself
rather than in what a probe could reach, and both are fixed here.

**1. An exported class was `kind: "value"`.** `Callability` is derived from
`GetSignaturesOfType(…, SignatureKindCall)`, and a class type has construct
signatures and *no* call signature, so every exported class answered
`nonCallable` and `promote_callable_export` /`promote_entry_callable` left it a
value. At runtime `typeof C === "function"`, which is what the probe's kind
probe measures, so each such export was a failed claim — 102 of them in
`@tanstack/solid-db@0.2.37` alone, all error classes, and `kind` is the one
claim schema v1 has no sentinel for, so one wrong answer blocks its whole
entrypoint. Class-ness now comes from the compiler's own declaration kind plus
the syntax facts' class-name spans, walked through alias and
`const Alias = SomeClass` hops by exact symbol identity.

**2. A retained callback parameter published the negative claim.** Local calls
are summarized transitively and the caller inherits the callee's callback
answer, but an *empty* answer is the claim "invokes no caller-supplied
function". `createComputation(fn, init) { const c = { fn, value: init, … }; }`
retains rather than calls, so solid-js 1.9.14's `createMemo`, `createEffect`,
`children`, `createSelector`, `createDeferred`, `createRenderEffect` and
`createComputed` each certified inertness the package contradicts on every use.
Retention is now tracked per parameter and opens
`callbacks: {"status":"unknown"}` on the declaring export, propagating along the
forwarding edges the callback rows already travel.

Measured against a `HEAD` baseline binary, per package, exports whose
`callbacks` domain moved to the sentinel: `@solidjs/web@2.0.0-rc.1` 38/388
(9.8%; 9 of its 48 exports with proven rows folded into the sentinel because a
sibling parameter of the same export escaped), and **zero** in
`@solid-primitives/analytics@2.0.0-next.2`,
`@solid-primitives/context@2.0.0-next.2`,
`@solid-primitives/connectivity@1.0.0-next.2` and `@corvu-next/dialog@0.1.5` —
the four corpus rows that reached `verified` before. On solid-js 1.9.14 the
probe's incompleteness findings fell 35 → 23 rows and 10 → 6 distinct exports.

**Still fail-closed or unresolved after this.**

- **A class's behavioral domains other than `callbacks` are still omitted.**
  The generator summarizes function declarations, not construct signatures, so
  a constructor that reads a signal or calls `onCleanup` publishes no
  `reactiveReads` or `ownerRequirements` row, and an omitted row is a certified
  negative. Only `callbacks` fails closed today, because only that domain has a
  demand-sensitive consumer and therefore a bounded cost. Resolving it properly
  means resolving a class export to its constructor's summary node — including
  the inherited constructor when the class declares none — which is a separate
  slice.
- **Retention is a closed list of positions**: an object-literal property
  value, an assignment value whose target is not rooted at a caller-supplied
  parameter, and a computed read of a rest parameter. A parameter that leaves
  through a conditional branch into a local binding does not open the sentinel,
  which is why solid-js 1.9.14 `createRoot` (`updateFn = unowned ? fn : () =>
  fn(…)`, then `runUpdates(updateFn, true)`) still publishes an empty callbacks
  claim in the client build and is still contradicted by the probe. Widening the
  list to conditional branches was measured and rejected: it converts a third of
  `@solidjs/web`'s exports while proving nothing.
- **Sub-entrypoint variants lag the root.** `./web:createComponent`,
  `./web:mergeProps` and `./jsx-dev-runtime:createEffect`/`createDeferred`
  remain contradicted for solid-js 1.9.14 where their root-entrypoint
  counterparts no longer are; those summaries are inherited through a
  dependency contract rather than analyzed in the sub-entrypoint's own runtime
  target.
- **Eight `callbacks[].execution` claims are simply wrong** and are now
  reachable because the probe no longer dies early: `.:onMount` states
  `tracked` where the runtime is `deferred`, `./jsx-dev-runtime`'s
  `createComputed`/`createMemo`/`createRenderEffect` state `inline` where the
  resolved artifact is `tracked`, `./jsx-dev-runtime:createSelector` states
  `deferred` where it is `tracked`, and `./web:use` states `deferred` where it
  is `inline`. These are a *different* generator defect — the wrong execution
  kind, not a missing row — and were present before this change; they were
  invisible only because the earlier contract crashed the probe worker first.

  **Re-measured 2026-08-23, after the execution-kind pass: four closed, four
  still fail.** The three `onMount` claims (`.`, `./jsx-dev-runtime`,
  `./jsx-runtime`) and `./web:use` no longer appear as failing claims at all —
  the fold over the enclosing callback chain answers `deferred` for
  `onMount(fn) { createEffect(() => untrack(fn)) }`, which is what the runtime
  does; `./web:use` stops failing in the same stage, and what the emitted row now
  says there was not separately read back. The four `./jsx-dev-runtime`
  sub-entrypoint variants —
  `./jsx-dev-runtime:createComputed`, `createMemo`, `createRenderEffect` and
  `createSelector` — still fail in `server` mode with the identical claim text and
  observed `tracked`, which is the "sub-entrypoint variants lag the root" bullet
  above: their root-entrypoint counterparts are fixed and these summaries are
  inherited through a dependency contract rather than analyzed in the
  sub-entrypoint's own runtime target. They are four of the ten `callbacks`
  failures left in the whole corpus. Note that these four are reached in a
  `server` session and are *not* withdrawn by the new inert-runtime rule, because
  `./jsx-dev-runtime` resolves unconditionally to `dist/solid.js` and re-runs
  normally — the withdrawal is per runtime, not per mode, and this is the shape
  that distinction exists for.

## Closed 2026-08-23: the probe environment was measuring itself

The corpus-wide machine-verification measurement attributed a root cause to each
of its 210 refusals, and the largest single one — `kind-observed`, 82 rows — was
not a claim anybody disagreed with. It was the absence of any observation at
all: roughly fifty of those rows had an entrypoint whose module **throws on
import** in at least one mode, so no `kind` reading existed, and `kind` is the
one claim schema v1 has no unknown sentinel for. A further 2,248 claims went
undriven because the throwaway install did not contain packages the probed code
imports, and three wide-surface rows exceeded a flat wall budget and produced no
result at all.

None of that is a fact about a package's reactivity. It is the probe's own
environment being reported as the package's behavior, and four things are fixed
here.

**1. A minimal, mode-scoped, recorded import environment.** The probe worker now
defines fifteen browser globals — `window`, `document`, `self`, `location`,
`screen`, `history`, `localStorage`, `sessionStorage`, `matchMedia`,
`requestAnimationFrame`, `cancelAnimationFrame`, `getComputedStyle`,
`MutationObserver`, `ResizeObserver`, `IntersectionObserver` — before it imports
anything, in the `client`, `development` and `production` sessions only. The
list, and the members of each fake object, are derived from what the corpus's
failing packages actually reach for, not from what a browser happens to have.

The premise is stated rather than assumed: **a claim observed under the shim is
a weaker observation than one observed in a browser.** So `<contract>.probe.json`
gains an `environment` block naming, per mode, the globals the process invented
and the ones Node already provided, and `<contract>.verify.json` carries it
forward. Four rules bound it: server modes are never shimmed (an import that
throws on `window` under `--conditions node` is a *truthful* observation);
generation is untouched, since `contract generate` imports nothing; every faked
value is stamped with a non-enumerable `__solidCheckerProbeShim` accessor and
the process carries `__solidCheckerProbeEnvironment`, so a probe body can ask;
and an import that still throws is unchanged — undriven, `import-failed`, with
the throw as its reason.

The sharpest reason the record exists: a `typeof window === "undefined"` guard
never threw, so for those modules the shim *redirects* rather than rescues. A
package that took its server path in every earlier measurement now takes its
browser path.

**2. Peer-complete installs.** The manifest's install environment was built for
static generation and installs what a row *pins*. For Solid 2 that runtime is
two packages, and rows whose package declares only `solid-js` as a peer got only
`solid-js` — 248 claims of the previous measurement were an
`ERR_MODULE_NOT_FOUND` for `@solidjs/web` attributed to the package. The harness
now completes the pinned runtime with the parallel `@solidjs/web` version, and
separately installs the non-optional peers the *installed artifact's own*
`package.json` declares, in a second npm invocation so no peer range can take
part in resolving a pinned version. If a peer install moves a pin anyway, the
pinned-only tree is restored and the row records that.

The line held: **a missing peer is the harness's gap; a missing undeclared
import is the package's.** `@solid-primitives/utils` (94 claims), `server-only`
(60) and the `react`/`vue`/`svelte`/`vite`/`@angular` group are imported by
packages that declare them nowhere, and completing those would mean the harness
choosing a version the package never named. They remain import throws.

**3. A probe budget that scales with the claim count.** 90 s + 500 ms per
planned claim, capped at 900 s, computed from the exact plan `contract probe` is
about to run rather than from an export count. A flat 120 s was a budget for the
median package and a guaranteed timeout for the wide-surface ones. All four rows
that previously timed out now complete in 83–208 s. A timeout remains its own
outcome class; this changes how many rows hit one, never what hitting one means.

**4. An asynchronous package throw no longer costs a whole mode.** Package code
the probe set running — a deferred callback, a promise left rejected — throws
outside every `try` the worker has. The process died with status 1 and an empty
stdout, so the parent had *no* results for that mode: every probe already
answered was discarded, and because a whole-process failure names no probe to
retry past, the mode ended there. The worker now answers with what it observed,
`completed: false`, and the abort reason, so the parent restarts for the
remainder exactly as it does after a synchronous throw. The reason is reported
and never attributed to a claim.

**Measured, by running the corpus four times against the same two snapshotted
binaries with one group of changes enabled at a time.** Each step is a full
416-row run and each attribution is a per-row set difference, not a
classification of deltas:

| State | Verified | Δ |
| --- | --- | --- |
| 2026-08-22 baseline | 194/416 (46.63%) | — |
| + engine fixes (class kind, retained-callback sentinels) | 214 | +20 / −0 |
| + the abort guard (4 above) | 217 | +3 / −0 |
| + shim, peer-complete install, scaled budget (1–3 above) | **222/416 (53.37%)** | +12 / −7 |

The environment half is a net **+1** on the headline, and that is the honest
result. What it bought is *observation*: claims driven 6,257 → 7,809, rows with
an entrypoint import throw 55 → 34, exports certified by a verified contract
672 → 752, probe timeouts 3 → 0. More observation surfaces more contradictions
as well as more confirmations, and one contradiction refuses a whole contract —
so `probe-failed` rises 65 → 75 as a root cause while `kind-observed` falls
82 → 71.

**Still fail-closed or unresolved after this.**

- **An inert fake can change an answer, and one row shows it.**
  `@solid-primitives/pagination@0.5.2` now fails `createInfiniteScroll
  callbacks[0]=deferred` with `observed inline`, because the fake
  `IntersectionObserver` never fires and a callback a browser would run on
  intersection ran only at setup. The driver already has the right precedent —
  a mismatch its own read scope could explain is recorded `undriven`, not
  `failed` — and the same reasoning applies to a mismatch a faked DOM API could
  explain. It is not implemented, because "which claims depend on which faked
  API" is not knowable from the contract, and the blunt version ("any failure in
  a shimmed mode is undriven") would discard the 99 genuine `tracked → inline`
  findings. The two `@solid-primitives/resize-observer` rows sit on the same
  line, one step less clearly.
- **The synthesized-argument boundary is now the binding limit for DOM
  primitives.** `@solid-primitives/interaction` reads `el.ownerDocument` on the
  element the *caller* passes, and the driver synthesizes `{}` there; the shim
  only let execution get far enough to reach the limit. RFC 0002 refuses a
  ladder of retries deliberately, so this stays undriven rather than being
  guessed at with a fake node.
- **Four globals were reached and deliberately not added.** `EventSource` (12
  claims), `Element` (4), `HTMLAudioElement` (2), `HTMLVideoElement` (2). Each
  needs constructor or `instanceof` identity rather than a value, and faking
  that invents behavior rather than removing an obstacle. 20 claims across the
  corpus.
- **93% of verified contracts still certify no observed behavior.** Verified
  rows carrying a probed behavioral row went 6 → 15 of 222, and the markers kept
  12 → 25. The rate roughly doubled on a base that is still almost nothing; the
  binding constraint is drivability, not the environment.
- **2,745 claims have no probe form at all** and never will —
  `reactiveReads` 1,354, `ownerRequirements` 565, parameter identity 421, nested
  return leaves 257, `asyncBehavior` 100, callback arguments 25, store paths 23.
  They are static claims, or claims schema v1 has no evidence slot for.
- **Wrong execution kind is now the dominant visible defect class**: **159** of
  the 218 failing claims, once the 53 `kind: value → function` failures the class
  fix removes are gone (the six `returns: accessor → array` failures are the
  remaining balance; an earlier revision of this entry said 155, which does not
  add up against the report's own shape table). `callbacks[n]: claimed tracked,
  observed inline` alone is 99. That is a generator *and* a probe defect and is
  tracked in "[Generated contracts contradicted by the runtime
  probe](#generated-contracts-contradicted-by-the-runtime-probe-2026-08-23)"
  above and in "[Closed 2026-08-23: execution-kind vocabularies, tracked-wrapper
  schedules, and one parameter with two
  executions](#closed-2026-08-23-execution-kind-vocabularies-tracked-wrapper-schedules-and-one-parameter-with-two-executions)"
  below, not here. **Re-measured 2026-08-23: 159 → 10 of 63. Re-measured
  2026-08-24: 11 of 24, the 53 `kind` failures having fallen to 13 in the same
  run, so neither class dominates any longer.**

## Closed 2026-08-23: `contract verify` refused without writing anything down

The refusal path built no sidecar at all — `buildVerifyReport` was reachable
only after the promotion succeeded, and its `blockers.raised` was always `[]` —
so the most common outcome of the command was the least legible one. The only
record of *why* a contract was not promoted was stderr: a CI run kept a log or
kept nothing, and the corpus measurement had to recover the RFC 0002 blocker
taxonomy by pattern-matching English sentences against lines carrying absolute
paths.

A refusal now writes `<contract>.verify.json` with `outcome: "refused"`,
`blockers.raised` carrying every line the command printed, `blockers.checked`
carrying the same taxonomy the success path lists, and the consumed probe
report's own figures. Every refusal path goes through it — the blocker list, the
stronger-existing-evidence refusal, and the document-does-not-validate refusal —
so the sidecar exists for the same set of outcomes the stderr lines describe.

The two shapes are told apart by `outcome`, never by which counts are zero:
every field that would imply a promotion is **absent** rather than zeroed — no
`evidence`, no `conversions`, no `probed`, no `summary`, and a `contract` block
with `before` and no `after`, because nothing was written. Success behavior is
unchanged, and the docs sentence that promised "the blockers checked" without
saying the file only existed on one path is corrected.

Two consequences had to be handled rather than discovered later:

- **A refusal never overwrites a record of a promotion.** A sidecar carrying
  `evidence` is the audit trail of a verification that actually happened — of
  some other bytes, if it survived a regeneration, and self-invalidating either
  way — and replacing history with the record of a failed attempt is a strictly
  worse artifact. A refusal record replaces a refusal record; that is the only
  overwrite.
- **`contract generate` read the file's *existence* as proof of a verification.**
  `snapshotPreviousReview` moved a contract to `.previous` and printed "the
  previous machine-verified contract … were kept" whenever the sidecar was on
  disk. With a refusal sidecar that message was false and the snapshot was
  pointless, so the check is now on the record's content: `outcome !== "refused"`
  and an `evidence` block present.

The corpus harness reads `blockers.raised` in preference to stderr, and keeps
the text classifier for journals written before this change.

**Still unresolved after this.** The refusal sidecar is not schema-validated —
nothing loads it, exactly as with the promotion sidecar — and `blockers.raised`
is free text rather than a taxonomy field, so a consumer still classifies by
matching the line. Emitting the blocker *class* alongside each line would remove
the last reason the corpus harness owns a text classifier at all; it is not done
here because the classes live in `contract-verification.mjs` as a flat list and
`collectBlockers` builds sentences rather than tagged records.

## Closed 2026-08-23: execution-kind vocabularies, tracked-wrapper schedules, and one parameter with two executions

Generator defects behind the largest visible class in the corpus verification
measurement — 159 of the 218 failing claims are a wrong
`callbacks[].execution` — the ones that are the generator's own fault rather
than the probe's.

**Measured 2026-08-23, staged.** The class is now **10 of 63** failing claims and
the corpus verifies at **261/416 (62.74%)**, from 222/416. The two halves of the
change set were measured separately, each a full 416-row run against a
snapshotted release binary, with stage 1 built from `origin/main` (95270bee) plus
only the three probe-side files: probe-side fixes **222 → 243** (+21 / −0,
failing claims 218 → 106, execution-kind 159 → 47), generator-side fixes
**243 → 261** (+18 / −0, failing claims 106 → 63, execution-kind 47 → 10). The
full account, including what it cost, is in
[ecosystem-benchmark.md](ecosystem-benchmark.md#the-staged-decomposition-2026-08-23).
The cost is stated there and here: 445 `callbackExecution` rows and 67 proven
exports withdrawn from the generated corpus, and 12 of the 15 verified rows that
carried probed behavioral evidence lost it, because 22 of those 25 markers had
been promoted from observations made in a runtime that re-runs nothing.

**Re-measured 2026-08-24.** The class is **11 of 24** failing claims and the
corpus verifies at **267/416 (64.18%)**. The one claim it gained is a genuine
finding the previous state could not see: `@solidjs/testing-library@0.8.10`'s
`testEffect callbacks[0]=deferred`, observed `inline`, reachable only once 48 of
that package's exports stopped being unobservable `kind: "value"` summaries. Its
ten predecessors are unchanged claim-for-claim.

### A clearing wrapper stays `inline`

`interproc.rs`'s `primitive_callback_execution` labelled `untrack` and 2.0's
`flush` `"deferred"`, and said so in a comment: a contract consumer reads
`"deferred"` as "not tracked here", which is the meaning the summaries needed.
But `"deferred"` also promises the callback does **not** run before the export
returns, and all four of `untrack`, `createRoot`, `runWithOwner` and `flush` run
it during the call. `docs/package-contracts.md` already stated the vocabulary
the other way round — these primitives stay `inline` while clearing the
listener, and the clearing travels separately through the dialect — so the two
halves of the tree disagreed, invisibly, until `contract probe` began measuring
timing.

The reconciliation is three pieces:

- `Dialect::runs_callback_synchronously`, a **derived** trait method rather than
  a per-dialect table: exactly the members of `runs_callback_deferred` whose own
  `callback_executions` rows are all `Execution::Inline`, so the two answers
  cannot drift. `the_synchronous_clearing_set_is_the_inline_half_of_the_deferred_set`
  pins the concrete sets — 1.x `{createRoot, runWithOwner, untrack}`, 2.0
  `{createRevealOrder, createRoot, flush, runWithOwner, untrack}`.
- `flush` earns its place on the rc runtime's bytes, not on its name:
  `@solidjs/signals` `flush(fn)` is `syncDepth++; try { return fn() } finally {
  … }`, so the callback is invoked and its value returned during the call
  (2.0.0-rc dev bundle). The reviewed bundled contract for `solid-js@2.0.0-rc.0`
  independently states `flush` `callbacks[0] = inline`.
- a composition over the chain of enclosing callback positions, innermost
  outward, replacing "the innermost classifiable wrapper" at both seams that
  needed it: the direct-invocation ladder and the local-callee forwarding
  ambient.

### The ambient tracking scope is not the export-relative schedule

The same composition fixes the opposite error. `onMount(fn) { createEffect(() =>
untrack(fn)) }` published `tracked` because the derivation read the enclosing
`createEffect` callback's lexical *tracking scope* and published it as the
callback's *schedule*. The clearing wrapper means the callback is not tracked;
the effect means it has not run when `onMount` returns. The fold answers
`deferred`, which is what the repo's own reviewed semantics map states for
`onMount` and what the runtime does.

Order is load-bearing and the fold keeps it: `untrack(() => createMemo(fn))`
stays `tracked`, because the memo subscribes what runs inside it and an outer
`untrack` cannot undo that. A rule phrased as "any clearing wrapper anywhere
means not tracked" answers `deferred` there and is wrong.

### `Tracked` does not mean "later", and the dialect now says which

The first version of the fold read a tracked wrapper above a clearing one as
`deferred`, on the assumption that a tracked computation has not run when the
creating call returns. **In 1.x that is false for four of the five tracked
wrappers the schedule table can produce.** Against `solid-js@1.9.14`
`dist/solid.js`: `createMemo` (`:244-256`), `createRenderEffect` (`:218-221`)
and `createComputed` (`:214-217`) all call `updateComputation(c)` on the
creating call; `mergeProps` (`:1329`) wraps every function-valued source in a
`createMemo`, so it is eager at every index; `createResource`'s tracked source
(`:283`) is a `createMemo` too. Only `createEffect` (`:222-229`) defers, via
`Effects ? Effects.push(c) : updateComputation(c)` — and it defers exactly
because a package export runs under an owner, where `createRoot`'s
`runUpdates(updateFn, true)` (`:192`) has installed `Effects = []` (`:820`).

Measured with the probe worker's own observation shape against the oracle
install (`rust/target/tsc-oracle/v1`, `--conditions browser`), five shapes
claimed `deferred` where the runtime and the probe answer `inline`:
`createMemo(() => untrack(cb))`, `createMemo(() => createRoot(() => cb()))`,
`createRenderEffect(() => untrack(cb))`,
`createRenderEffect(() => createRoot(() => cb()))` and
`mergeProps({a: 1}, () => untrack(cb))` — plus the same shape through the
local-callee forwarding seam that solid-js's own `dist` goes through. These were
not accidental leftovers: they were a *derived* `deferred`, which is worse.

The fix is a third dialect fact, `Dialect::tracked_callback_timing(primitive,
argument, argument_count) -> Option<TrackedCallbackTiming>`, established from
the audited runtimes rather than from names:

- 1.x eager (`DuringCall`): `createMemo`, `createRenderEffect`, `effect`
  (`solid-js/web`'s alias for it), `createComputed`, `createResource`'s
  two-argument source, `mergeProps`. Deferring (`AfterCall`): `createEffect`.
- 2.0 eager: `createEffect` and `createRenderEffect` — both go through
  `effect()`, which calls `recompute(node, true)` unconditionally
  (`@solidjs/signals@2.0.0-rc.0` `dist/dev.js:4107-4121`) — plus `createMemo`
  (`:4558-4560`), `createSignal(fn)` (`:4548-4552`), `createOptimistic`
  (`:4778-4790`) and `createProjection` (`:5634-5675`), all of which build a
  non-lazy `computed` and so hit `setupComputedNode`'s
  `!options?.lazy && recompute(self, true)` (`:2845`). Deferring:
  `createTrackedEffect`, which builds a `lazy` computed and only
  `enqueue`s it (`:4253-4309`). **The two dialects disagree on
  `createEffect`**, which is why this cannot be one shared table.
- Unestablished, and therefore the unknown sentinel: 1.x `createSignal`/
  `createStore` (the argument is stored, never invoked), 2.0 `createStore` and
  `createOptimisticStore` (their derived overloads did not accept the probe's
  call shape, so no measurement backs a claim), and every tracked primitive with
  no schedule row in `primitive_callback_execution` at all.

`the_tracked_callback_schedule_partitions_each_dialect` pins all three sets per
dialect. The fold composes a detached callback under an eager wrapper to
`inline`, under a deferring one to `deferred`, and under an unestablished one to
the unknown sentinel — it returns `Option<&str>` now, and both ladder seams plus
the forwarding seam treat its `None` as authoritative rather than falling back
to the lexical answer. Where tracking is *not* cleared the answer stays
`tracked` regardless of schedule: attribution is the claim there, and the
wrapper's timing is not asked for.

### The cross-target merge unioned contradictory callback rows

The per-export contradiction sentinel below runs inside Rust, **once per
analyzed target**. `mergeSummaries` in
`packages/cli/scripts/generate-package-contract.mjs` then unions the targets'
callback rows, with a comparator that broke ties on `execution` precisely
because two executions per parameter were expected there. So the sentinel was
bypassed for every conditional export, and `fixtures/package-contracts/
conditional-callback-conflict` shipped a base carrying `parameter: 0` as
`deferred` *and* `inline`. `returns` and `asyncBehavior` had been given the
sentinel for this exact shape (`claimDomainsDiverge`); `callbacks` had not, in
the same function.

`callbackRowsContradict` now applies the same rule to the merged callback rows,
reports the divergence through `onDiverge` so the review plan's
`unknown-sentinel` item names both branches, and leaves the exact per-branch
claims in `variants`. The fixture is registered in
`scripts/contract-corpus.mjs`, so a regression of the union specifically fails a
gate; before, no gate saw the base at all — the process test asserts only the
variants.

One-sided *presence* is deliberately not closed: a parameter with a row in one
branch and none in the other is a positive against a certified negative, the
same hole `claimDomainsDiverge` closed for `returns`. It needs its own
measurement and is listed as unresolved below.

### One parameter with two executions is one false claim

One row is pushed per invocation site and `push_contract_callback` dedups only
exactly-equal rows, so a parameter invoked twice with two schedules published
both — `@solid-primitives/range`'s `mapRange` carried `callbacks[2]` as
`deferred` *and* as `tracked`, and the report lists both as failing. Schema v1
has one execution axis per parameter and the runtime has one behavior, so at
least one row was false and a consumer picking either was guessing.
`contract_export_function` now opens the per-export `callbacks` sentinel for a
parameter carrying two different executions, in the same three lines the
retained-callback fix extended. Rows that agree on `execution` and differ
elsewhere are deliberately not contradictory.

The documents that defined the vocabulary moved with the code, because the
change makes contract emission a consumer that asks *when* a callback ran and
both of them said nothing downstream does. `docs/package-contracts.md` and the
`Execution` comment in `solid-dialect/src/lib.rs` now state the two axes
explicitly — `tracked` is attribution, `inline`/`deferred` are the schedule of a
callback the export does not subscribe — and name `startTransition` and
`createResource` as the two places the readings diverge, along with the reason
emission refuses them rather than restating their attribution as a schedule.

Fixtures: `callback-untracked-wrapper` (a clearing wrapper is `inline`, with the
tracked and deferred negatives), `callback-deferred-untracked-chain` (nesting
and its order-sensitivity, the eager/deferring/unestablished partition, and the
forwarding seam through a bootstrap-resolved local `untrack`),
`multi-role-callback-parameter` (the intra-target sentinel and its width, with
four negatives including two same-schedule sites),
`conditional-callback-conflict` (the cross-target union). All four are in
`scripts/contract-corpus.mjs`.

**Still fail-closed or unresolved after this.**

Two of these ship an *affirmative wrong claim* — not a lost fact, not a
sentinel. Both are pre-existing and both are rows `contract probe` will fail;
they are stated that way because "recorded" is not the same as "harmless".

- **A package-local transparent wrapper around the real `untrack` publishes
  `tracked`, and the truth is `deferred`.** A schema-v1 `callbacks` row carries
  the execution word and no clearing column, so once a local callee's summary
  crosses the forwarding edge an `untrack`-shaped wrapper and a transparent one
  are indistinguishable and the enclosing tracked wrapper wins. Measured shape:

  ```ts
  import { createEffect, untrack } from "solid-js";
  function runUntracked<T>(fn: () => T): T { return untrack(fn); }
  export function mountThroughWrapper(handle: () => void): void {
    createEffect(() => runUntracked(handle));   // published: "tracked"
  }
  ```

  Against solid-js@1.9.14 the callback does not run during the call and its
  reads subscribe nothing, so `classifyExecution` answers `deferred` and the
  `tracked` claim fails — which is what the reviewed bundled contract states for
  the identical `onMount` shape. Inside solid-js itself the wrapper *is* a
  primitive and the composition sees it; an arbitrary package's own detaching
  helper does not benefit. This is the one-line wrapper spelling most of the
  ecosystem uses. `trackedThroughLocalHelper` in the chain fixture is the
  *correct-answer* control for it (its `runNow` genuinely does not clear), so no
  fixture pins the wrong case; adding one is cheap and would make the gap
  visible instead of prose-only. Closing it needs a clearing column the schema
  does not have, or transitive propagation of the clearing fact along forwarding
  edges.
- **A wrapper the fold cannot classify at all falls back to the lexical answer,
  which can be a positive wrong claim.** `enclosing_callback_chain` refuses the
  whole chain on the first position `callback_wrapper_at` cannot classify, and
  the row then comes from `contract_callback_execution(semantic)` — the same
  lexical answer this fold exists to replace. Only
  `primitive_callback_execution`'s table classifies wrappers during generation
  (the bundled solid-js contract is not reachable through
  `contracts.callbacks` in a generation run), so the unclassifiable set is
  large: `batch`, `startTransition`, `catchError`, `createComputed`, `onMount`,
  `onError`, `createSelector`, `children`, `createDeferred`, `produce`, `from`,
  `render`, `hydrate`. Measured, with runtime truth:

  | export body | published | runtime |
  | --- | --- | --- |
  | `untrack(() => batch(() => cb()))` | `deferred` | **inline** |
  | `batch(() => untrack(() => cb()))` | `deferred` | **inline** |
  | `createComputed(() => createRoot(() => cb()))` | `deferred` | **inline** |
  | `createComputed(() => untrack(() => cb()))` | `deferred` | **inline** |

  All three are pre-existing RC3 residue and none is a regression. The honest
  behavior is the sentinel rather than the lexical fallback, and the forwarding
  seam shares it: `forwarded_callback_ambient_execution` now *names* the refusal
  instead of laundering it through `unwrap_or_default()`, but still lets the
  forwarding call's own position answer alone, which is deliberately
  best-effort. Note that `createComputed` reaches this residue and not the
  eagerness one — it has no schedule row, so the chain is refused a step earlier
  than the fold.
- **The contradiction sentinel carries no review-plan reason at the intra-target
  seam.** The `unknown-sentinel` item is derived from the contract's bytes, and
  the `because.attributions` block comes from an obligation marker whose label is
  hardcoded as `UnknownCallbackExecution` / `contract-generation-obligation` in
  `rust/crates/solid-facts-backend/src/main.rs`. A
  `contradictory-callback-execution` reason needs that label plumbed from the
  emitter; the sentinel itself is unconditional and does not depend on it. The
  *cross-target* twin does carry a reason, through `mergeDivergences`.
- **The contradiction sentinel is per export, which is wider than the
  contradiction.** One contradicted parameter discards the other parameters'
  undisputed rows (`contradictOnZeroOnly` in the multi-role fixture pins it).
  Schema v1 offers no narrower spelling: the only granularity below
  `{"status": "unknown"}` is a row's presence, and an absent row is a certified
  *negative*, so dropping only the contradicted parameter's rows would trade one
  contradiction for one affirmative false negative. Narrowing it needs a schema
  change, and the pre-existing `escaped_parameters` sentinel has the same width
  for the same reason.
- **One-sided callback-row presence across conditional targets is not a
  divergence yet.** `callbackRowsContradict` catches two executions for one
  parameter; a row proven in one branch against a *proven absence* in the other
  is the same class of hole `claimDomainsDiverge` closed for `returns`, and it
  still hands the proving branch's positive to the environment-unaware base. It
  needs its own measurement — the blast radius is every conditionally-exported
  callback-taking function — and its own fixture pair.
- **The wrong-execution-kind class is not closed, but it is now small and
  named.** Ten `callbacks[].execution` claims still fail across the whole corpus,
  measured 2026-08-23, and they are three groups:
  - **`@solid-primitives/pagination` `createInfiniteScroll`, three rows** (0.5.2
    `deferred → inline`; 1.0.0-next.6 floor and head `tracked → inline`). This is
    the row already flagged as **possibly the import shim's doing** under
    "[Closed 2026-08-23: the probe environment was measuring
    itself](#closed-2026-08-23-the-probe-environment-was-measuring-itself)": the
    fake `IntersectionObserver` never fires, so a callback a browser would run on
    intersection ran only at setup. Unchanged by this pass and still the leading
    candidate for a "a faked global could explain this" undriven rule.
  - **`solid-js@1.9.14` `./jsx-dev-runtime`, four claims** — `createComputed`,
    `createMemo`, `createRenderEffect`, `createSelector`. The sub-entrypoint
    variant lag described above; their root-entrypoint counterparts are fixed.
  - **Three single rows**: `@solid-primitives/memo@2.0.0-next.2`
    `createWritableMemo` (`deferred → tracked`, both Solid 2 probes) and
    `@solid-primitives/date-difference@1.0.2` `createDateNow`
    (`tracked → inline`). Neither has been investigated; both are new to the
    visible set only in the sense that they were previously buried under 149
    others.

  The `mergeProps` conservative-callable forwarding — a positive row for a
  parameter the export was never proven to invoke — is still untouched, and the
  probe-side noise it was waiting behind is now gone, so it is measurable.
- **A `callbacks` sentinel silences the `returns` probe of the same export.**
  Measured, and the one place this change set *lost* a finding instead of fixing
  it. `@solid-primitives/utils`'s `createHydratableSignal` and
  `createHydrateSignal` publish `returns: accessor` and really return a tuple;
  the probe caught that in all four modes and the corpus reported six such failing
  claims across three rows. After the contradiction sentinel opens `callbacks` on
  those exports, the returns probe reports *"no plantable reactive source: proving
  the returned value is an accessor needs a signal read inside a callback the
  contract states, and this export states none"* — the claim goes undriven,
  verification converts `returns` to unknown, and all three rows now verify.
  Nothing false reaches a consumer, because the wrong `accessor` claim is
  converted rather than promoted. But the generator defect is now invisible to the
  measurement, and three of the generator stage's eighteen gains rest on it. The
  driver plants a `returns: accessor` observation *through* a stated callback and
  has no other way in; giving it a second one (a synthesized reactive argument, or
  driving the accessor directly) is the fix, and it is not attempted here.
- **The forwarding seam's unknown arm is wider than it needs to be.** When the
  composed ambient execution is the sentinel, the emitter opens the
  unknown-callback obligation without knowing whether the callee publishes an
  `inline` row for the slot at all — so an export whose callee rows are all
  `deferred` loses its `callbacks` domain unnecessarily. Reaching it needs an
  unclassifiable-or-unestablished tracked wrapper above a clearing one above a
  local callee, and the cost is precision, never a wrong claim.

## Deferred: verification-suite speed work, robustness findings (2026-08-23)

An adversarial review of the `verify-speed-execution-kinds` speed work raised 26
findings. The stale-green and wrong-answer ones are fixed in that change set
(the worker pool's result attribution and death paths, the coverage key's
dialect-selection ancestor chain, the mid-run store guard, the registry memo's
input digest, `verify-delta`'s gitignored-input basis and its `pkg/contracts/`
row, the porcelain parse, the oracle base's symlink verification). These are the
remainder: none can produce a wrong verdict, each is a claim the code or a
document makes about itself that is narrower than it reads.

Ordered as the review ordered them, most severe first.

- **robustness — oracle case directories are a fixed path shared across
  processes.** `scripts/lib/tsc-oracle-case.mjs:29` — `rust/target/tsc-oracle-cases/<dialect>/case-<index>`
  has no per-process component, so two concurrent gate runs (`make tsc-oracle`
  in one shell and `make verify` in another, or a re-run started before the
  first finished) have worker threads rewriting the same `tsconfig.<pass>.json`
  and source file while the other run's checker reads them. The path was shared
  before the concurrency change too, but eight simultaneous writers make a bad
  interleaving far likelier. Failure shape is a JSON parse error or a bogus
  verdict, not a silent pass. A `process.pid` in `CASE_ROOT`, or a lock, closes
  it.
- **robustness — the provision short-circuit no longer heals a damaged
  install.** `scripts/tsc-oracle.mjs:105-135` — `assertProvisioned` checks only
  each top-level package's recorded `version`, so a missing transitive
  dependency (`csstype`, `seroval`), a deleted `.d.ts`, or a half-wiped tree
  passes and no `npm install` repairs it; the always-install path did. Direction
  of failure is loud (an incomplete install adds TS2307-class errors that land
  outside every case's `allow` set), and `--force` exists, but nothing tells a
  reader when to reach for it and the `already provisioned` line does not
  distinguish "verified complete" from "the two manifests I looked at agreed".
- **robustness — one unit crash now suppresses every drift report.**
  `scripts/coverage.mjs:286-334`, `scripts/contract-corpus.mjs:290-306` — compute
  and compare are two phases, so a crash in unit 40's `analyze`/`generate` makes
  `mapPool` throw before the comparison loop runs and drifts in units 0–39 are
  never printed. They were printed before the crash when the two were
  interleaved. Green/red is unaffected; the diagnostic value is lost on exactly
  the runs that need it. `mapPoolSettled` already exists, is unused, and is the
  right tool.
- **robustness — the gate cache has no eviction.**
  `scripts/lib/gate-cache.mjs:239, 291-298` — one `<key>.json` per (shared digest
  × unit), never pruned, and every checker rebuild invalidates all 83 coverage
  keys and writes 83 more files carrying full findings lists. `createdAt` is
  stored and nothing reads it. Only `make clean` reclaims it. The growth
  expectation is now stated in AGENTS.md; an age or count cap is not
  implemented.
- **robustness — no memory-aware cap on the oracle-gate fan-out.**
  `scripts/lib/pool.mjs:29-31`, `scripts/tsc-oracle-gate.mjs:312-316` — each
  worker thread carries its own `typescript` instance and runs two checker child
  processes, each spawning a TypeFacts producer; `min(cores, 8)` multiplies all
  of it. The cap bounds the process tree, nothing bounds resident memory. On a
  memory-tight runner the failure is an OOM-killed thread, which now surfaces as
  a `gate worker exited with code …` rejection rather than the hang it used to
  cause — loud, but it reads like a gate failure rather than a resource one.

The review's remaining test-coverage finding — the pool's death path being
untested, and one self-referential assertion — is **not** deferred: it is closed
by the four regression tests added to `scripts/pool.test.mjs` in the same change
set (unattributable message, idle death, queued-task settling on close, fatal
answer), and the self-referential `threadId` assertion is replaced. It is
recorded here only so the review's numbering has no silent gaps.

## Closed 2026-08-23: a rendered component is a call, not an escape

The row *Private component rendered (`<Panel/>`)* in **Closed 2026-08-23:
under-marking in the attribution ladder** ends at "every export marked
(`fallback-all`)". That was the fail-closed answer to an unsound one, not the
right answer: the checker already resolves a tag to its exact component
function — `SemanticLookup::function_called_at`, which is how
`jsx_call_sites` decides component identity and Loading placement — while
`all_function_call_sites` enumerated only `ast.calls`. A private helper whose
one caller was `<PanelView/>` therefore had a *known* caller and a call graph
that said it had escaped, so every export of the entrypoint was marked.

**The fix.** `all_function_call_sites`
(rust/crates/solid-reactive-ir/src/indexes.rs) emits a call edge for each JSX
element whose tag name resolves through `function_called_at` to exactly one
project function. The callee is the tag *name* span — the component's own
reference — so the escape test in
`compute_entered_only_through_calls`/`reference_is_accounted_for`
(rust/crates/solid-reactive-ir/src/attribution.rs) accepts that reference
through the branch it already had: `known_call_sites`, built from the same
edges. There is deliberately no branch that accepts a reference *because* it is
a tag. A syntactic short-circuit would also accept the tags that emit no edge —
unresolvable names, an escaped component rendered elsewhere — and each of those
is a case where the honest answer is that something the graph cannot enumerate
renders it.

Both halves are one commit for that reason: the edge without the acceptance
leaves the widening in place, and an acceptance not backed by the edge is the
unsound direction.

**Both spellings of a render, and only one edge for each.** `<Panel></Panel>`
writes the component's name twice, and TypeScript reports both occurrences as
references to the same symbol, so the edge alone accounted for the self-closing
form and nothing else: the paired form — the dominant real-world spelling for a
component with content — kept widening to every export. `solid-facts` records
the closing tag's name span (`JsxElementFact::closing_name`, `visit_jsx_element`
in rust/crates/solid-facts/src/ast/mod.rs), and `FunctionCallSite` carries it as
`also_referenced` beside the callee. `function_call_sites` is unchanged — one
entry per invocation, which is what the three consumers that count calls read —
and the escape test alone reads `function_call_site_references`, which is the
same sites plus that extra span. The closing span cannot mint an edge or account
for anything on its own: it is stored on the edge the *opening* tag's resolution
created, so a tag whose opening name resolves to nothing has no site to carry
it, and no consumer sees a caller that the runtime does not have. Pinned by
`escaping-private-helper` (`./closed`, `./children`, and the two
`indexes::tests` closing-tag cases, one of which is exactly the
opening-unresolved counterfactual).

**One authority for "which tags resolve".** `jsx_call_sites` (component
identity, Loading placement) and `all_function_call_sites` (the render edges)
both iterate `SemanticLookup::jsx_rendered_functions`. They had two literal
copies of the same resolution, and the argument that the new edges cannot move a
finding depends on their agreeing: a rendered function is
`jsx_call_site_loading(..).any`, which short-circuits
`compute_function_is_component` before `directly_called` ever sees a JSX edge. A
filter added to one copy would have silently broken that; one iterator cannot
drift from itself.

**Blast radius.** Three arms of `escaping-private-helper` are new (`./closed`,
`./children`, `./member-tag-children`) and one moved: `./rendered` became
shape-identical to its `./called` control (`reachability`, not `fallback-all`, in
the review plan). No findings snapshot moved — a JSX-rendered function already
short-circuits `compute_function_is_component` through `jsx_call_site_loading`
before `directly_called` is consulted, so the new edges cannot flip component
identity. The two other consumers of `function_call_sites` see a callee span
that is not a call expression: `member_parameter_symbols_at` finds no `CallFact`
there and marks the site unresolved, which clears the symbol set (fail closed),
and `semantic_write_execution_role_within` classifies the tag span's own
execution context, which is the render position the component actually runs at —
real evidence about a caller that was previously invisible, not a substitute for
it. Neither moved a finding across the 83 coverage projects; a write whose only
caller is a render site can now be classified where it previously stayed
`Unknown`, and that is the intended direction. For a function that is both
called and rendered, `all_function_call_sites` appends call expressions before
render sites, so `semantic_write_execution_role_within` — which takes the first
non-`Unknown` role — resolves the tie by an argued rule (a call expression is
the more direct evidence: its own syntax names the invocation) rather than by
which file happens to hold the call.

**Still fail-closed after this.**

- **A dotted tag stays an escape, in both spellings.** `<ns.Panel/>` *does*
  resolve — TypeScript reports the symbol at the whole `ns.Panel` name span, so
  the edge is emitted with that whole span as its callee, a member expression
  rather than an identifier. What fails closed is the span mismatch: the
  reference the escape test walks is the `Panel` property inside the name, and
  the test is byte-exact span membership, not containment. Adding the closing
  name changes nothing there, because both of a dotted tag's spans are the whole
  dotted name. Pinned by `escaping-private-helper` (`./member-tag`,
  `./member-tag-children`) and by
  `indexes::tests::a_resolvable_dotted_tag_is_an_edge_whose_callee_is_the_whole_name`.
  Closing it needs the member reference and the tag edge to name the same span,
  which is a resolution question, not a widening one.
- **A tag that resolves to nothing stays an escape**, which is the point: an
  unresolved import or an ambiguous computed name emits no edge, and a
  conservative caller set is the only sound one.
- **A component used as a value is an escape, and must stay one.**
  `<Wrap child={Panel}/>`, `return Panel`, `apply(Panel)` — the receiver decides
  whether and when to invoke it. Pinned by `escaping-private-helper`
  (`./prop-value`), beside `./argument` and `./returned`.

The two spellings that could have made the edge name-matched rather than
resolved are pinned in the same fixture: `./shadowed` renders a project function
named `Show`, a Solid 1.x built-in spelling in the dialect vocabulary, and gets
the edge because the symbol is the project's; `./intrinsic` renders `<div/>`
beside an unused project function named `div`, and gets none, because TypeScript
binds a lowercase tag name as an intrinsic element name and never against the
value scope.

**Measured on the ecosystem corpus (2026-08-23), and the honest headline is that
it moves two probes of 416.** Both full-corpus harnesses were re-run against the
release binary
`068b04bb1fe98268ccf37fb7a29780f5a194207149972bdbfdb1b73bf28a44b6` and the
checked-in reports under `benchmarks/ecosystem/` are that state (the account is in
[ecosystem-benchmark.md](ecosystem-benchmark.md)):

- **Content**: exports proven 5,410 → **5,417** of 8,358, exports carrying an
  unknown 2,948 → **2,941**, unknown claims 6,776 → **6,762** (`reactiveReads`
  −7, `returns` −7), reactive-read rows 1,198 → **1,202**. `callbacks`,
  `ownerRequirements` and `asyncBehavior` are unchanged to the claim, no probe or
  package gained or lost fully-proven status, and the outcome classes are
  identical probe-for-probe.
- **Verification**: **nothing moved** — the same 261 verified and 146 refused
  *rows*, zero gained and zero lost, the same 63 failing claims in the same five
  shapes, the same root causes, conversions, exports certified and session
  counts. The claim plan grows by the four `reactiveReads` rows the content
  measurement gained, and all four are `no probe form: reactiveReads`: static
  claims no probe can drive.
- **Where it lands.** `@tanstack/ai-solid-ui@0.7.18` is the whole content delta.
  Its `MessagePart` is a private component whose only caller is `<MessagePart …/>`
  inside `ChatMessage`, and it holds the `ReactiveDispatchUnresolved` obligation
  `props.toolsRenderer[props.part.name]?.(toolProps)`. That obligation's
  `mechanism` moves `fallback-all` → `reachability` and its reach enumerates
  `MessagePart` → `ChatMessage` → `ChatMessages` — two chained render edges, the
  second being `<ChatMessage message={message} />`. Eighteen unknown sentinels
  across nine exports become four across the two exports that reach it.
- **The second probe moved without moving a number.**
  `@solid-primitives/start@0.0.4` has three `solid-start` obligations, in
  `root/InlineStyles.tsx` and `root/Links.tsx`, whose components are each rendered
  exactly once (`<InlineStyles />` in `root/Scripts.tsx`, `<Links />` in
  `root/Document.tsx`). Their reach is now complete *and empty* — no export of the
  entrypoint reaches them — so they stop marking `createServerCookie` and
  `createUserTheme` and are disclosed as `artifact-binding` notes instead (5 → 8).
  Its four sentinels remain, raised by other obligations that still answer
  `fallback-all`.
- **Nothing certified without an edge behind it.** Every export that changed
  state, in both probes, traces to a render site the graph resolved; no probe in
  the corpus lost proven surface, and no unknown became proven anywhere else.

**Re-measured 2026-08-24 beside the export-kind proof, and it survives.** On the
merged engine (`ddb0ecd860d4c77f50d1d6c7a0af003bc3adb34ff46a0fcee81715c84ac574b1`)
the render edges are worth **exports proven +6, exports carrying an unknown −6,
unknown claims −14** (`reactiveReads` −7, `returns` −7) and reactive-read rows
+4 — the same two probes, the same mechanisms, and one proven export fewer than
above. The missing export is the two change sets' only overlap: of the seven
`@tanstack/ai-solid-ui@0.7.18` exports the render edge frees, one carries a
`callbacks` unknown the `kind` proof opens, so it is not fully proven either way.
Verification moves the same four undriven `reactiveReads` claims and nothing else,
against the `kind` proof measured alone
(`34e97be60c60291debbae66239082cd1e252ff53831f7f1eb977647207f31aec`).

Two probes is a small result and it is not evidence that the shape is rare in
real projects: the rendered-only private helper is a component-library idiom, and
this corpus is dominated by Solid Primitives, whose 288 contracts (281 after the
2026-08-24 refusals) are hooks and
primitives rather than components. The corpus measures published packages'
*contract generation*, which is the one place this shape is least represented.
The reports carry no attribution-mechanism field, so the `fallback-all` →
`reachability` counts above were read from the review plans' `because` blocks by
regenerating both probes against the previous and current binaries; a corpus-wide
count of the ladder's rungs is not available from `report.json` today.

## Open: the contract generator's tsconfig sets no `jsxImportSource`

`analyzeTarget` (packages/cli/scripts/generate-package-contract.mjs) writes
`jsx: "preserve"` with no `jsxImportSource`. solid-js declares its `JSX`
namespace as an `export namespace` inside the module — the published package
contributes no *global* `JSX`, which is why a real Solid project sets
`jsxImportSource: "solid-js"` — so during contract generation no `JSX` namespace
is in scope for any package, and TypeScript treats every JSX element as
implicitly `any`.

Surfaced while making `escaping-private-helper`'s `solid-js` stub faithful
(its README carries the measurements). Two observable effects:

- A built-in with a required `children` prop and a JSX child reports `TS2741`
  during generation — `escaping-private-helper`'s `builtin.jsx` does. The
  diagnostic is *identical with the real published package installed*, so it is
  a property of this tsconfig, not of any stub, and it changed no claim in the
  generated contract.
- Any future claim that depends on intrinsic-element *typing* would be untested
  during generation. Nothing depends on it today: `./intrinsic`'s claim is about
  TypeScript's tag-name **binding** rule, which holds with or without
  `JSX.IntrinsicElements`, and was verified against the published typings under
  `jsxImportSource: "solid-js"`.

Setting the option would need a dialect-aware value (`solid-js` for 1.x,
`@solidjs/web` for 2.0, and nothing for a non-Solid package) and would move
whatever pins currently depend on JSX expressions being `any`, so it is recorded
rather than done here.
## The `kind` claim a bundled artifact contradicts (2026-08-23)

The corpus measurement's 53 failing `kind: claimed value, observed function`
claims (`benchmarks/ecosystem/verification-report.json`) were two generator
defects, not a sentinel gap. Both are fixed here; the RFC 0002 amendment's
honesty check stands — no contradicted `kind` was absorbed into an unknown,
because `kind` still has none.

**1. A bundler's class expression (45 of the 53).** The exported-class fix read
declaration kinds and class-name spans, which is what *source* contains.
Rolldown, esbuild and tsdown all lower `export class C {}` to `var C = class
{ … }` and re-export it by specifier, so a published artifact has an
**anonymous** class expression and a variable declarator: no class-name span
covers the exported binding, and `nonCallable` is the truthful callability of a
class type. Every class-shaped failing row had exactly that shape —
`ReactiveMap`/`ReactiveWeakMap`, `ReactiveSet`/`ReactiveWeakSet`,
`TriggerCache`, `ResponseEnvelope`, `SelectionManager`, `ListCollection`,
`AsyncBatcher`/`Debouncer`/`Queuer`/`RateLimiter`/`Throttler`, and the
`*DevtoolsCore`/`*DevtoolsPanel` family. The initial diagnosis — a re-export
hop the alias walk could not follow — was wrong: `@tanstack/pacer@0.22.0`'s `.`
entrypoint was already correct (its barrel's imports resolve to `batcher.d.ts`,
whose `declare class` the alias walk does reach) while `./batcher`, the same
class entered through the `.js` artifact, was not. `BindingFact` now carries
`initializer_class`, and it decides class-ness for a plain-identifier
declarator.

**2. A `kind` no closed type answers (the other 8).** `@solid-devtools/locator@0.16.7`'s
`addClickInterceptor`, `addHighlightingSource`, `addLocatorModeSource`,
`highlightedComponent`, `highlightingEnabled`, `locatorModeEnabled`,
`setTarget` and `useLocator` are destructured from a value whose type comes
through an untyped dependency, so Type Facts answers `Callability::Unknown`.
`promote_callable_export`/`promote_entry_callable` treated that exactly like
`NonCallable` and published `kind: "value"` — and because `validate_export`
bars a `value` summary from carrying any claim domain, that is the **maximal
certified negative**: reads nothing reactive, returns nothing reactive, invokes
no caller-supplied callback, requires no owner. For functions whose whole
purpose is to take a callback. This was a live wrong-claim class in ordinary
`inferred` contracts, independent of verification. Generation now refuses the
entrypoint instead, through the existing refusal path.

Reproduced before and after against the pinned real packages:
`@tanstack/pacer@0.22.0` and `@tanstack/solid-pacer@0.22.0` (20 rows across 11
entrypoints, `value` → `function` with `callbacks: {"status":"unknown"}`),
`@solid-primitives/trigger@3.0.0-next.2` (`TriggerCache`), and
`@solid-devtools/locator@0.16.7` (now refused, with no wrong claim published).

Remaining fail-closed and residual cases:

- **A location with no callability fact at all keeps `value`.**
  `demand_plan.rs` requests callability exactly where it requests a type
  descriptor, so `entity.callability == None` is missing evidence about the
  span, not an answer about the type; refusing on it would refuse for a
  demand-coverage accident rather than for an unprovable kind. Closing it means
  widening the demand plan for export-specifier spans first, then measuring.

  **This hole is narrower than it was first described.** An adversarial review
  could construct no shape that publishes a `value` through it:
  `demand_plan.rs:130-135` demands `type_descriptor`, and therefore
  callability, for *every* export specifier and declaration, and
  `entry_export_entity` only ever looks at those spans — relative `export *`,
  `export * as ns`, renamed specifiers, string export names,
  `export default <expr>` and destructured `export const { … }` were all tried
  and all refused. Bare-specifier `export *` bottoms out in dependency-contract
  recursion, which is the carried path below rather than an undemanded publish.
  So the residual publish-without-proof risk is a carried claim, not the demand
  plan. `export_kind_proof`'s `Undemanded` arm is still the correct fail-open
  for an absent fact; it is simply not a reachable publisher today.
- **A locally declared type exported by specifier still refuses.**
  `export type { T }` and `export interface T {}` are dropped by their
  `type_only` marker, and an unmarked re-export is now dropped by following the
  relative import/re-export chain to a `type_only` export of the same name
  (`export_is_type_only` in the backend). `interface T {} export { T }` has no
  `type_only` fact anywhere: the interface is not an exported *declaration*, so
  nothing records that its name is type space. Reproduced: such a package still
  refuses its entrypoint. Closing it means recording type declarations in
  solid-facts (an `AstFacts` addition next to `classes`), which is a fact-domain
  change rather than a generator one.
- **`Callability::Mixed` refuses rather than describing the union.** A union
  with callable and non-callable constituents has no single `typeof`, and
  schema v1 has no way to say "either". Refusal is right for the document, but
  a consumer of such an entrypoint loses every other export's claim with it.
- **The project-wide analysis map keeps the `value` default.**
  `promote_callable_export` feeds `Program::contract_exports`, cannot refuse,
  and so still records `value` for an unprovable kind. Emission is what
  refuses, and every real `contract generate` invocation goes through it
  (`--contract-entry-file` is always passed); the `contract_entry_file`-empty
  emission mode does not, and would publish that default.
- **A carried summary is trusted on provenance, not on trust.** A dependency
  contract's `kind` is not re-decided locally *when* that contract was either
  generated by this same run from the dependency's own sources
  (`--generated-contract`) or carries evidence that a human or a verifier stood
  behind it. That is the right call for those two — the local facts are
  strictly worse — but it does mean a wrong `kind` in a *reviewed* dependency
  contract propagates into every package that re-exports it.

  **A contract with neither provenance was the laundering channel, and is
  closed.** `dependencyContracts()`
  (`packages/cli/scripts/generate-package-contract.mjs:766`) discovers
  `node_modules/<dep>/solid-reactivity.json` by walking upward with no
  `--contract` flag and no evidence check, so an `inferred` contract written by
  any earlier solid-checker — including one with the `Unknown ⇒ value` defect
  this pass fixes — carried its `kind` verbatim. Reproduced: an untyped
  dependency with a hand-written `inferred` contract calling
  `addClickInterceptor(fn)` a `value` republished that exact
  `@solid-devtools/locator` claim through the one door the refusal left open.
  Such a contract's `kind` now goes through `export_kind_proof` like a local
  claim (`fixtures/package-contracts/carried-value-kind`'s `./laundered`
  entrypoint, refused; `addTypedInterceptor` on `.`, corrected to `function`).
  Only `kind` is gated this way — every other claim in a discovered contract is
  used as before, because a contract is the only evidence there is about a
  package the project cannot see into.

**The measured residue of these two fixes.** First reproduced package by package
with the benchmark's own install shape (`buildSpecs`: the pinned package plus the
pinned Solid runtime), which put 36 of the 53 rows as resolved and 17 as still
wrong. **Superseded 2026-08-24 by the full-corpus run**, which is the authority
here because it exercises every row rather than the ones the reproduction chose:
**40 of the 53 are resolved — 25 corrected to `kind: "function"` and 15 withdrawn
because their entrypoint is now refused — and 13 remain wrong.** The
per-reproduction table below is corrected against
`benchmarks/ecosystem/verification-report.json`; three rows moved against the
reproduction's prediction. `@tanstack/form-devtools`'s `./production` entrypoint
refuses (the reproduction expected neither of its two to);
`@tanstack/ai-devtools-core`'s two rows were counted as remaining `value` when in
fact the whole package is refused; and `@tanstack/solid-table-devtools`'s one row
resolves by *refusal* rather than by correction — its `.` entrypoint is refused
and the row verifies on the other one.

| Package (probe pin) | Rows | After (measured 2026-08-24) |
| --- | --- | --- |
| `@kobalte/core@0.13.13` | 4 | `function` |
| `@solid-primitives/map@1.0.0-next.2` | 4 | `function` |
| `@solid-primitives/set@1.0.0-next.2` | 4 | `function` |
| `@solid-primitives/trigger@3.0.0-next.2` | 2 | `function` |
| `@tanstack/solid-pacer@0.22.0` | 10 | `function` |
| `@tanstack/devtools@0.14.2` | 1 | `function` |
| `@solid-devtools/locator@0.16.7` | 8 | refused (whole package) |
| `@tanstack/ai-devtools-core@0.5.6` | 2 | refused (whole package) |
| `@tanstack/solid-hotkeys-devtools@0.7.0` | 1 | refused (whole package) |
| `@tanstack/table-devtools@9.2.0` `.` | 1 | refused (1 of 2 entrypoints) |
| `@tanstack/hotkeys-devtools@0.9.0` `.` | 1 | refused (1 of 2 entrypoints) |
| `@tanstack/solid-table-devtools@9.2.0` `.` | 1 | refused (1 of 2 entrypoints) |
| `@tanstack/form-devtools@1.0.0-alpha.2` `./production` | 1 | refused (1 of 2 entrypoints) |
| `@solidjs/web@2.0.0-rc.1` `ResponseEnvelope` | 6 | **still `value`** |
| `@tanstack/*-devtools` `*DevtoolsCore` | 7 | **still `value`** |

The seven surviving `*DevtoolsCore` rows are `@tanstack/devtools-a11y@0.2.2`
(`./core` and `./core/production`), `@tanstack/pacer-devtools@1.4.0` (`.` and
`./production`), `@tanstack/form-devtools@1.0.0-alpha.2`'s `.`, and the
`./production` entrypoint of `@tanstack/hotkeys-devtools@0.9.0` and
`@tanstack/table-devtools@9.2.0`. In every one of those packages the *sibling*
entrypoint refuses and this one does not, which is the reason split measured
directly: the refused sibling reports *"which destructures a member of another
value"* or *"whose runtime kind no closed type answers (Unknown)"*, while the
survivor reaches the class only as a **tuple element type** and so gets a truthful
`NonCallable`. 25 + 15 + 13 = 53.

The residue is **one family**, not two shapes: a binding whose *type* is a
class but which the syntax reaches only through a value expression, so
`Callability::NonCallable` is the truthful call-signature answer and no
class-ness fact contradicts it. The measured instances:

- `const ResponseEnvelope = /* @__PURE__ */ (() => { class ResponseEnvelope
  {…}; ResponseEnvelope.prototype[ENVELOPE] = true; return ResponseEnvelope;
  })()` — the IIFE a bundler emits for a class with static prototype patches.
  The initializer is a *call*, so no syntactic hop reaches the class. **Still
  publishes `value`.**
- `const coreClasses = constructCoreClass(Component.preload); const
  TableDevtoolsCore = coreClasses[0]` — the class is a tuple *element type*
  declared in `@tanstack/devtools-utils`, reachable only through the type, and
  the binding is a member access on a plain-identifier declarator. **Still
  publishes `value`.**
- `const { Inner } = Container` (a static class member) and
  `const [Core] = pair` (a tuple element whose element type is a class) — the
  same family behind a *binding pattern*. An adversarial review found these
  after the first measurement, and one of them was a regression: pre-fix the
  ungated `binding_initializer_symbol` hopped `Inner → Container → class` and
  answered `function` by accident, while the shape gate — which is right, since
  `const { name } = class Named {}` is a string — made it a certified negative.
  **Refused now, not published:** for a destructuring pattern the class search
  cannot run at all, so `nonCallable` is not a `value` proof
  (`ExportKindProof::DestructuredMember`).

The cost of that last refusal, stated plainly: a destructured binding whose
type really is a primitive — `(class Named {}).name`, a string — is provably
not a function and is refused with the rest. Two facts would separate it, and
neither is demanded at an export-specifier span today: `primitive_value_domain`
(a demand-plan widening, measurable) or the constructability fact below.

No amount of further syntax chasing closes the first two: there is no class
expression anywhere in the analyzed package. The exact fix is a
**constructability fact** — `GetSignaturesOfType(…, SignatureKindConstruct)`
beside the existing `Callability` in the Type Facts producer. With it, `kind`
reduces to one rule (callable or constructable ⇒ `function`; neither ⇒
`value`; no closed answer ⇒ refuse), the whole syntactic class-ness search in
`binding_declares_class` becomes redundant, and the destructured-pattern
refusal above becomes a decision. That is a producer change at a pinned
revision (`typefacts` rev `e2f7ac5`), so it belongs to a deliberate pin move
per docs/monorepo.md, not to this pass.

Two consequences of the refusal path that the next corpus measurement will
see, and should not be read as regressions:

- **Newly refused entrypoints. Measured 2026-08-24, and four times wider than
  this entry first estimated.** The estimate was "five entrypoints across twelve
  real packages"; the full corpus reports **21 newly refused entrypoints across 19
  probe rows and 17 distinct packages**. Of those, **11 rows (9 packages, since
  `@solid-primitives/platform` contributes three) lose every entrypoint they had**
  and so fail generation outright, and **9 further entrypoints** are omitted from
  a contract that still emits. `@kobalte/core` (69 entrypoints), `@solidjs/web`
  (13) and `@tanstack/pacer` (14) gained no refusal at all, as predicted.

  The estimate was narrow because it was reproduced only against the packages
  that already had a *failing* `kind` claim. Most of the refusals are somewhere
  else entirely, and they are a distinct backlog item recorded below
  ("[The refusal path costs enums and untyped values, measured
  2026-08-24](#the-refusal-path-costs-enums-and-untyped-values-2026-08-24)").
- **The ecosystem classifier has no marker for these refusals yet.** Confirmed by
  measurement: all 11 all-refused packages landed in `unclassified`, with their
  full stderr retained, which is what made them diagnosable from the completed
  run. `scripts/ecosystem-benchmark/lib/classify.mjs` is the benchmark's own
  taxonomy and is left to the measurement pass; the two marker texts to key on
  are exactly `whose runtime kind no closed type answers` and
  `which destructures a member of another value`. Only an *all-refused* package
  reaches `unclassified` at all — a partial refusal is classified from the
  generator's own "N entrypoint(s) refused and omitted" note, independently of
  the reason text, and all 9 partials were classified that way — and
  `unclassified` is a report bucket, not a gate failure. Both phrases are pinned
  by assertions in rust/crates/solid-facts-backend/tests/contracts_process.rs, so
  the coupling cannot rot silently.

**Re-measured after the adversarial review's fixes.** Reproduced against the
same local installs: `@tanstack/pacer@0.22.0` (14 entrypoints, 75 exports) and
`@tanstack/solid-pacer@0.22.0` (13 entrypoints, 108 exports) move **no export
at all** from the numbers above, and `@solid-devtools/locator@0.16.7` still
refuses with the same message. `@tanstack/solid-pacer`'s `./types` entrypoint is
refused for an unrelated pre-existing reason (its dependency contract has no
entrypoint matching `@tanstack/pacer/types`), not by any `kind` decision.

**The corpus-level cost and gain, measured 2026-08-24.** The full ecosystem
benchmark was re-run against a release build of this change set as it sits beside
the render-edge change it merged with
(`ddb0ecd860d4c77f50d1d6c7a0af003bc3adb34ff46a0fcee81715c84ac574b1`), both
harnesses, all 416 probe rows, and the checked-in reports under
`benchmarks/ecosystem/` are that state. Machine verification: **261 → 267
verified** (+13 rows gained, −7 lost, none moved `verified → refused`), **146 →
129 refused**, failing claims **63 → 24**, and generation failures **4 → 15**.
Contract content: exports proven **5,417/8,358 → 4,450/8,082**, exports carrying
an unknown **2,941 → 3,632**, probes fully proven **205/409 → 125/398**. The
arithmetic closes exactly — proven −967 = 276 exports that stopped existing (the
export total falls 8,358 → 8,082) plus the 691 by which exports-carrying-an-unknown
rose, an identity because no export in either state lacks a summary — so nothing
in the movement is unattributed.

**Measured alone, this change set produces the same verdict on every row.** A
release build of it without the render edges
(`34e97be60c60291debbae66239082cd1e252ff53831f7f1eb977647207f31aec`) verifies the
same 267 rows and refuses the same 129 — *the same rows* — with the same 24
failing claims, root causes, conversions and session counts; the two change sets
differ only by 4 undriven `reactiveReads` claims and by 6 proven exports of
`@tanstack/ai-solid-ui@0.7.18`, where they overlap on one export. So none of the
movement above is an interaction between them. Both numbers are stated once, with
the family breakdowns, in
[ecosystem-benchmark.md](ecosystem-benchmark.md#headline-numbers-2026-08-24-eighth-measurement-state-release-binary-416-probes).

## The refusal path costs enums and untyped values (2026-08-24)

**Open.** The `kind` refusal above is the right call for the *document* and it is
much more expensive than the entry that introduced it estimated, because most of
what it refuses is not a class at all. Measured across the full corpus: 21 newly
refused entrypoints, 11 probe rows failing generation outright, 9 further
entrypoints omitted from a contract that still emits. Every refusal's reason was
read back from the review plan of a fresh generation against the pinned package,
not inferred: **17 report `whose runtime kind no closed type answers (Unknown)`
and 4 report `which destructures a member of another value`**. Only **6 of the 21
name a class-shaped export** — the `*DevtoolsCore` / `*DevtoolsPanel` family the
fix was written for. The other 15 are exports that are **provably not functions,
and whose own published `.d.ts` says so**, refused because the analyzed `.js`
artifact leaves `Callability::Unknown`.

Three shapes, each reproduced against the pinned real package:

- **A downleveled TypeScript enum** — five packages, and the single largest
  cause. `@kobalte/utils@0.9.2`'s `EventKey`,
  `@solid-primitives/audio@1.4.5`'s `AudioState`,
  `@solid-primitives/intersection-observer@2.2.5`'s `DirectionX`,
  `@solid-primitives/analytics@0.2.1`'s `EventType` and
  `@solid-primitives/cookies-store@1.1.11`'s `CookieSitePolicy`. Every one is
  published as `var E; (function (E) { E[E.A = 0] = "A"; … })(E || {});` with
  `export declare enum E` in the sibling `.d.ts`. The declarator has no
  initializer, so no syntactic hop reaches a type and no callability fact closes.
  Each of these five is a *single* export that costs its package its only
  entrypoint.
- **A value computed from an untyped global.**
  `@solid-primitives/platform@0.2.1` and `@1.0.0-next.2` (three probe rows)
  refuse on `isBrave`, which is
  `export const isBrave = !!n.brave && n.brave.isBrave && …` in the artifact and
  `export declare const isBrave: boolean` in the declaration.
- **`Object.assign(Object.create(null), …)`.** `solid-js@1.9.14`'s **`./web`**
  entrypoint — 76 exports, including `render`, `hydrate`, `Portal` and `Dynamic`
  — is refused because `web/dist/server.js` has
  `const Aliases = Object.assign(Object.create(null), { … })`. `Object.create(null)`
  is `any`, so the `Object.assign` result is `any`. The declaration says
  `Record<string, string>`. **This is the most consequential single refusal in the
  corpus**: `solid-js` is the anchor package, and a project generating its own
  `solid-js` contract now gets no `./web` claims at all. It does not affect the
  bundled 1.x contract, which is checked in rather than generated per project.
- **A destructured member that is not a class either.**
  `@solid-devtools/ui@0.10.3`'s `./theme` refuses on `color`, reported as
  *"which destructures a member of another value, so no fact here rules out a
  class"*. It is a theme colour table, and the refusal is the deliberate cost the
  `ExportKindProof::DestructuredMember` bullet above already states — measured
  here rather than hypothesised.

What all four have in common is that **the fact is present in the package and
the analysis does not reach it**. `export_kind_proof` asks the analyzed
implementation for a call signature and gets no closed answer; the `.d.ts` beside
it answers definitively. Two candidate closures, in order of how much they buy:

1. **A constructability fact** —
   `GetSignaturesOfType(…, SignatureKindConstruct)` beside the existing
   `Callability` in the Type Facts producer — is already recorded above as the
   fix for the *class* residue. It does **not** close this one: an enum object and
   `isBrave` are neither callable nor constructable, so the rule "callable or
   constructable ⇒ `function`; neither ⇒ `value`; no closed answer ⇒ refuse"
   still refuses them, because `Callability::Unknown` is the absence of an answer
   rather than a `NonCallable` one.
2. **A `primitive_value_domain` or object-literal-domain fact at an export
   specifier span**, which the demand plan does not request today. This is the one
   that would actually close it, and it is the same demand-plan widening the
   destructured-pattern refusal above needs. It is measurable: the 17
   `Callability::Unknown` refusals are a concrete before/after target.

Until then this is a stated, measured cost rather than a silent one. **The
direction is still correct** — a `kind: "value"` summary is barred from carrying
any claim domain, so publishing one for an unprovable kind certifies "invokes no
caller-supplied callback" about an export nobody analyzed — and losing a
whole package's contract is a strictly better failure than publishing a false
negative for it. But "refused because we could not tell an enum from a function"
is a *demand-coverage* limit dressed up as a soundness result, and the entry
above should not be read as though the seventeen packages were doing something
wrong.

## `export * as ns` loses its alias (2026-08-24)

Pre-existing, out of scope for the `kind` pass that found it, and real.
`rust/crates/solid-facts/src/ast/mod.rs:1986` records an `ExportAllDeclaration`
without `declaration.exported`, so `export * as ns from "./m"` is
indistinguishable from `export * from "./m"` everywhere downstream:
`ExportFact { kind: All, module: Some("./m") }` in both cases.

**Reproduced.** A package whose only entrypoint is

~~~ts
// index.ts
export * as ns from "./typed.js";
// typed.ts
export function f(cb: () => void): void { cb(); }
export const v = { a: 1 };
~~~

publishes `f` (with a `callbacks` row) and `v` — two names the package does not
export — and omits `ns`, the one it does. Both halves are wrong in the
dangerous direction: claims filed under names that do not exist, and no claim
at all for the name a consumer actually imports. A consumer of
`import { ns } from "pkg"` then reads `ns.f(cb)` against no contract row.

The fix is two-sided: record the alias on `ExportFact` (a new optional field,
so no snapshot moves for the unaliased form), and teach
`exported_names_for_file` and `external_export_summary_for_file` that an
aliased `export *` contributes exactly one name whose value is a namespace
object. Schema v1 has no namespace `kind`, so the honest first step is probably
to refuse such an entrypoint rather than publish the members under the wrong
names — which is what the `kind` refusal already does for the `./ns` case in
that experiment, but for the wrong reason.

## Open: `kind` has no unknown form, and 64 refusals turn on it (2026-08-23, re-measured 2026-08-24)

The blocker recorded above under "[`contract verify` certified what no run had
observed](#closed-2026-08-23-contract-verify-certified-what-no-run-had-observed)"
item 2 — a `kind` claim not probed-passed in every stated mode cannot be
certified, because schema v1 has no sentinel to convert it to — is the **largest
single reason a real package does not machine-verify**: `kind-observed` is the
root cause of **64 of 121 refusals** in the 2026-08-24 A9-stage-1 corpus run,
more than incompleteness (38) and probe failure (15) combined. It was 77 of 146
when this item was opened, and 74 of 129 in the state stage 1 was measured
against.

The decision on whether to relax it, the measurement behind it, and the three
rejected options are
[RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md). In short: a
schema-v1 sentinel for `kind` is **rejected** (`kind` is `required` with
`additionalProperties: false`, so a new spelling fails `validate_export` inside
`decode` — the malformed path that fails the whole analysis rather than refusing
one contract; and an unknown `kind` is only honest if every domain of that
summary becomes unknown too, at which point the summary is informationally
identical to omitting the export). Nothing in the plan may absorb a
*contradicted* `kind`: the failing claims stay failures, and the re-measurement
asserted it — **24 failing claims before and after, 13 of them `kind`, in the
same five shapes**.

**Staged, and the last stage is gated on data rather than on a decision.**

- **Stage 0 — done.** Every reason the probe pipeline can emit now lands in a
  named `undriven` bucket in `scripts/ecosystem-benchmark/verify-corpus.mjs`,
  and `verify-corpus.test.mjs` asserts that totality against the driver's own
  reason tables so the next new string fails the test instead of widening
  `other` (834 claims, the bucket that made stage 2 undecidable). Each row also
  carries a `kindGaps` breakdown — per (unobserved `kind` claim, mode), why it
  was unobserved — surfaced per refusal and aggregated as "Why a `kind`
  observation is missing", with a contradicted claim counted in a separate
  `contradictions` object under its own heading (a contradiction is never a gap,
  and the two sharing a number is what the amendment forbids), and with a mode
  the run never attempted and a mode where no unambiguous summary resolves each
  a labelled category rather than a silence. Label-only: nothing in the pipeline
  reaches a verdict by reading a reason string. One rule ordering is load-bearing
  rather than cosmetic: a session death forwards the child's stderr verbatim, so
  the `export-missing` rule sits below every session rule and is anchored to the
  end of the reason string — otherwise a crash quoting a bundler's `'x' is not
  exported by y` was counted as the one outcome stage 2 may narrow away.
  **Measured, 2026-08-24: `other` is 671 → 0, and all 671 are `probe session
  aborted by package code`** — one class, a session-death class, which is a gap
  and keeps blocking. `UNDRIVABLE.owner`, `export is not callable` and the two
  `returns`-distinguisher reasons account for **zero**, as predicted. Two things
  make that a measurement rather than a relabelling: the undriven total holds at
  5,005 with every other bucket unchanged to the claim, so nothing moved *between*
  buckets; and re-bucketing the previous state's own 416-row journal with the new
  rules reproduces the identical 671, so the classification is a property of the
  rules and not of the run. **One magnitude in the amendment needed correcting:**
  it recorded this bucket as **834** and predicted `834 → 0`. That 834 was
  measured against an earlier journal, before the export-kind proof shrank the
  claim plan; against the state stage 1 actually baselines on the bucket was
  already 671. The shape of the finding was right; the number was stale.
  And `export-missing` is *absent from that distribution by construction*: a claim
  observed in one mode and absent in another settles as `passed` and contributes
  no undriven reason at all, so stage 2's number had to come from the per-mode
  `kindGaps` figures. The undriven bucket was not where the addressable share was
  hiding — and, as stage 2 below now records, neither was anywhere else.
- **Stage 1 — done.** An unobserved `kind` claim refuses its **entrypoint**, not
  the document: the entrypoint is omitted from the promoted contract exactly as
  `contract generate` omits one it cannot certify, `<contract>.verify.json`
  records each refusal, the rewritten review plan carries a `refused-entrypoint`
  item naming the exports it dropped (so the reviewed tier is not silent about
  the omission either), and the document is refused only when no entrypoint
  would certify anything — an entrypoint with an empty export map is not a
  survivor. **Measured, 2026-08-24: +8 rows** (267 → **275** of 416 verified,
  129 → **121** refused), against a design-time bound of at most 10, with
  **nothing moving the other way** and no other outcome class moving at all. The
  30 entrypoints it refused inside a promoted document are the cost, made visible.
  Every prediction the amendment made about the *shape* of the payoff held: the
  two rows that had a survivor and still refused are the closure-note carve-out
  (`@solidjs/start@2.0.3` and `@tanstack/charts@0.14.0` — the two widest rows in
  the population, one of them the 91-blocker-line row the amendment named in
  advance), and the surviving half really is the less behavioral half — the eight
  newly verified rows contribute **zero** probed behavioral rows and only one
  contributes any conversion, six of them promoting fewer than nine exports. What
  stage 1 durably buys is consistency: generation and verification refuse on the
  same unit. The count is not the case for it, and the measurement says so.
- **Stage 2 — gate measured 2026-08-24, and it is a no-op. Do not build it.**
  Stage 2 would exclude from the modes a `kind` claim must be observed in exactly
  those whose probe outcome was `export-missing` — an observation that the export
  does not exist there rather than a gap. The amendment bounded it above at 43
  rows and could not say how much of the 43 was real. Now it can, and the
  addressable population is **empty**:
  - **45 of 6,962 gap (claim, mode) pairs (0.65%)** are observations of absence.
    The rest are import throws (3,878), sessions aborted by package code (2,629),
    sessions that wrote no report (328) and one unresolvable summary set (82) —
    every one a gap that must keep blocking.
  - **3 rows of 84 carry any `export-missing` gap**, and in none of them is it the
    only gap reason, so excluding the absences promotes nothing. All three are
    root-caused `probe-failed`, which is an independent blocker stage 2 may never
    absorb, so they are outside its population by definition.
  - Against the **64 rows root-caused `kind-observed`** — the population stage 2
    exists for — **zero** carry a single `export-missing` pair. Their 2,956 gap
    pairs are 2,226 import throws, 401 session aborts, 328 unreadable reports and
    1 unresolvable summary.

  The amendment's own gate was *"if stage 0 shows the server-only gaps are session
  deaths rather than absences, stage 2 buys nothing and must not be built."* That
  is what was measured. Building it would narrow on gaps, since there are
  essentially no absences to narrow on. **Stage 2 should be closed as
  measured-worthless rather than left open as pending work.**

**Remaining fail-closed cases after stages 0 and 1, re-measured 2026-08-24.**

- **The 64 rows root-caused `kind-observed` stay refused**, and stage 2 cannot
  help any of them. 8 of the previous state's 74 verified on a surviving
  entrypoint and 2 were reattributed to `closure-note`; what is left is
  single-entrypoint packages and packages every one of whose entrypoints is
  `kind`-blocked. A package the probe cannot import at all remains unverifiable,
  which is amendment A1's deliberate consequence; `contract review` is where an
  unimportable package belongs.
- **The 14 rows where `kind-observed` was a co-blocker rather than the root cause
  all still refuse** — every one has an independent blocker, and the
  re-measurement asserted it row by row rather than by eye.
- **A closure note still refuses the whole document.** Measured: it cost stage 1
  exactly 2 of its 10 candidate rows, and `closure-note` as a root cause rose
  2 → 4 for that reason and no other. Same for a failed probe or an
  incompleteness finding naming a claim of a refused entrypoint: a contradiction
  must be fixed, not dropped. `incompleteness` (38) and `probe-failed` (15) did
  not move. (Those 4 root-caused rows later split 2 `closure-note` / 2
  `attested-closure-note` when the record became compiler-attested, with no row
  leaving the refused set — see "[The runtime-module closure is walked, not
  attested](#the-runtime-module-closure-is-walked-not-attested-2026-08-22)".)
- **334 exports are now dropped from otherwise-verified documents** with their
  refused entrypoints — a state that did not exist before stage 1. They are
  counted as their own state inside the corpus composite's unchanged 8,696
  denominator, so nothing certifies more than it did; a consumer importing one
  gets the fail-closed pre-contract state, and the omission is named in the verify
  sidecar and the rewritten review plan.
- **Per-export omission is out** until it is established whether an export absent
  from a *present* entrypoint raises `SC9005` at the consumer or resolves
  silently to no summary.
- **The 13 remaining `kind: claimed value, observed function` failures are a
  generator defect**, not a sentinel gap, and they are unchanged by stages 0 and 1
  by design — 7 rows, 13 claims, contradicted in all four modes each, counted in
  their own `contradictions` object so no relaxation of the `kind` rule can
  quietly absorb one. They are the residue of the 53 the export-kind proof pass
  reconciled (25 corrected, 15 withdrawn with a refused entrypoint, 13 still
  wrong): a binding whose *type* is a class reached only through a value
  expression, needing a constructability fact from the Type Facts producer. Their
  own queue item, with their own fixtures.
- **The verified tier still rests on almost no observed behavior**, which is the
  number A9 asked to have published once: **3 of 275 verified rows** carry any
  probed behavioral row, and **0 of the 8** stage 1 added. Machine verification is
  certifying negatives and `typeof`; the human tier is still the useful one for
  positives, exactly as RFC 0002 unresolved question 1 warned.

## Compiler trace version 2: the producers converged, the 1.x consumer took the derivation (2026-08-24)

Both compiler pins moved to the merged semantic-trace producer work:

| pin | from | to |
| --- | --- | --- |
| `dom-expressions-compiler` (2.0) | `e6ab3469a94addd6f72c7e8347e871a1a0c7edf5` | `66004ab78fa10412208d1bc8cb301bfc028ea826` — **superseded later the same day** by `c6008f01df199ff0f0d072093e2393ed3d67f0c4`; see "Two lowering divergences the fork's own contract declares binding" below, and `rust/Cargo.toml` for the pin actually in force |
| `solid1-dom-expressions-compiler` (1.x) | `ad2c9452041c757138bb972416d8abc4798ea6b9` | `b66c3e34ba2a0b74238726eb2b83f767eacf94f2` — **superseded later the same day** by `d1e089581231b3028b7e8ce838ceed0f3c83e154`, which adds all-position `children` promotion and discarded-subtree retractions |

What the two forks ratified, from their own `docs/execution-contract.md`:

- `SemanticTrace` now carries `version: u32` (`SEMANTIC_TRACE_VERSION = 2`) and
  still refuses unknown fields, so a consumer must check the version before
  reading a single field.
- Three additive facts: `owner_establishments` (`{ span, wrapper, group_id? }`,
  one per wrapper call lowering emits), `component_render_sites` (`{ span }`),
  and `deferred_callback_sites` (`{ span, receiver_span }`).
- **The 1.x fork removed `ownership_sites` and `OwnershipDecision` entirely**;
  the 2.0 fork deliberately kept emitting them "for the currently pinned
  consumer". So the two producers now disagree about who owns the ownership
  rule, and this is the version where the consumer side had to answer.

What the consumer does about it:

- **Both adapters refuse a trace whose `version` is not the one they were
  written against**, rather than reading the fields they recognize and assuming
  the rest still mean what they used to. The refusal is unreachable through
  `analyze` — the pinned producer only ever emits 2 — so each dialect compiler
  crate pins it with a unit test instead
  (`an_unreadable_trace_version_is_refused_rather_than_projected`).

  **Corrected the same day, by review**: as first written the gate could not
  fire *at all*. It compared `trace.version` against `SEMANTIC_TRACE_VERSION`
  imported from the very producer that fills the field from that constant, so
  the two were equal by construction for every producer, a future version-3
  one included. The unit tests passed because they built their "wrong" version
  as `SEMANTIC_TRACE_VERSION + 1`, deriving the falsifier from the thing under
  test. Each adapter now owns a literal `READS_TRACE_VERSION = 2` used in the
  runtime check, plus a `const _: () = assert!(…)` holding the producer's
  constant equal to it so a schema bump fails the build; the tests build
  version `3` literally and name every `SemanticTrace` field rather than
  filling the rest from `..default()`. The producer's
  `#[serde(deny_unknown_fields)]` was never protection here either — nothing in
  this repository deserializes a `SemanticTrace`; `compile()` hands the struct
  over in-process. `docs/compiler-facts.md` carries the corrected story.
- **`rust/dialects/solid-v1/compiler` derives the owned ownership regions
  itself**: every `Value(ReactiveRerun)` site, and only when the compile ran
  under `Wrapper::Default`. That is the rule the 1.x producer applied at rev
  `ad2c9452` (`TraceRecorder::finish`) reproduced exactly, over the same
  `trace.sites` it used, including the "a configured effect wrapper is an
  unaudited runtime, so claim nothing" arm. Both existing unit tests still pin
  both arms.
- **`rust/dialects/solid-v2/compiler` still reads `ownership_sites`**, which the
  2.0 producer derives from the same rule at decision time. The projected
  `ExecutionMap` is identical either way; nothing downstream of the two adapters
  can tell which side derived it. Coverage confirms it: 86 fixture projects, 542
  findings, no finding moved.

Open, and deliberately not taken here — this slice was wiring, not new rules:

- **The three additive facts are produced and consumed by nothing.** They are
  the first compiler evidence about *where the wrappers went* rather than what
  each site decided: a memo boundary, a `createComponent` render site, and the
  component span that receives a deferred callback. Anything built on them needs
  its own fixtures and its own slice.
- **The join rules they require are not obvious, and getting one wrong is a
  false claim, so they are written down here before anyone consumes them.**
  A memo fact is spanned at the *booleanized test* it memoizes (`cond()` in
  `{cond() ? left() : right()}`), which is a strict sub-span of the consuming
  site: **join owner facts to sites by containment, not by equality.** A span is
  **not a unique key** — a `createComponent` and its child's `insert` can share
  one — so key on `(span, identity)`. A fact **need not join anything**: a
  literal-only hole emits a real `insert`, and literal-only leaves are
  deliberately not execution sites, so that fact joins to nothing and that is
  correct rather than a gap. `wrapper` is a string on purpose: an unaudited or
  custom identity must map to unknown, never be read as its name. The identity
  sets are also not identical across the forks — `scope` is 2.0-only, and 1.x
  emits `capture` for a captured listener where 2.0 reports only `direct` and
  `delegated` — so the mapping belongs to each dialect, not to shared code.
- **Whether the 2.0 adapter should migrate off `ownership_sites` too**, so the
  fork can delete the field it is keeping only for this consumer. The rule is
  the same, but the bytes are not guaranteed to be: the producer sorts and
  dedups its ownership sites, while a consumer-side pass over `trace.sites`
  emits one region per `(span, kind)`, so two same-span sites that are both
  `ReactiveRerun` would produce a duplicate region. `ExecutionMap::validate`
  permits that (it requires non-decreasing spans, not uniqueness), so the
  migration is safe — but it can change the map's bytes and therefore needs its
  own coverage run rather than riding along on a pin move.

One bookkeeping defect found while moving the pins: `THIRD_PARTY_NOTICES.md`
recorded the 1.x compiler at `79b9b63721c59b0acfd72348438bbb6e090ec81c`, two
pin moves behind `rust/Cargo.toml`. `docs/monorepo.md` requires the two to move
together; they now do.

## Census absence was read as proof of non-tracking; it is now uncertifiable (2026-08-24)

Found by adversarial review of the pin move above, and a precision-contract
violation rather than an imprecision: the checker was reporting a **proven**
SC1001 about code the compiler had declined to report on.

`ExecutionMap::uncovered_jsx_expressions` holds the census against itself — a
site the producer censused but left unclassified. It cannot see a source-level
JSX expression the census never listed, and the "the trace is total" comment in
both adapters is true only *within* the census. Each producer censuses the JSX
it lowers, and neither lowers everything. A reactive read inside an unlisted
expression matched no tracked region, no untracked region, no callback role and
no JSX operation, fell through `semantic_execution_role_within`'s
`inside_component` arm to `UntrackedRendering`, and SC1001 fired as a violation
whose evidence read "the read is outside every compiler-tracked JSX region and
deferred callback" — a completed search of facts that were never collected.

Two live shapes at the current pins, both verified against the fresh debug
binary:

| dialect | shape | before | after |
| --- | --- | --- | --- |
| 1.x | `{title()}` inside a nested non-hydratable `<head>` (census drops the whole head range) | SC1001 `v1/strict-read-untracked` **violation** | **uncertifiable** |
| 2.0 | `{count()}` as a root-level `<br>`'s child (no site emitted; generated code never reads `count`) | SC1001 `strict-read-untracked` **violation** | **uncertifiable** |

The fix is `missing_jsx_census` in
`rust/crates/solid-reactive-ir/src/execution_role.rs`, carried on
`ReactiveRead::missing_jsx_census` and flipped to `uncertifiable` in
`projection.rs`. It is dialect-free on purpose: the *rule* is identical in both
dialects and only the JSX each producer declines to census differs, which is
already a per-producer fact inside the producers. The JSX regions it consults —
attribute expression containers, spread containers, children — are read from
solid-facts' syntax, never from the census, because the question is exactly what
the source has that the census does not.

What it deliberately does not claim:

- **It does not certify the read safe.** "The compiler deleted this expression,
  so the stale read cannot happen" is a *second* claim, with no more evidence
  behind it than the first. Proving it needs the additive `owner_establishments`
  / lowering facts joined to source spans, i.e. its own slice.
- **It does not fire where a fact established the role.** The escalation is
  gated on `execution == UntrackedRendering`; a read the dialect proved runs in
  an untracked callback, or an event-handler census entry, inside an uncensused
  region keeps its proven violation, because its proof never came from the
  census. Verified at the current pins rather than reasoned about:
  `<br>{runWithOwner(owner, () => a())}</br>` — the 2.0 gap shape wrapping a
  callback `Solid2::reports_untracked_reads_at` claims (`RunWithOwner`
  argument 1), which `semantic_execution_role_within` turns into
  `UntrackedCallback` — reports an SC1001 **violation**, identical to the
  censused control. `untrack` is *not* such a callback in either dialect:
  neither catalog lists it in `reports_untracked_reads_at`, so a read inside an
  `untrack` callback produces no SC1001 at all and cannot demonstrate this gate
  either way.

Fixtures: `fixtures/reactive-ir/jsx-census-gap-solid-1x` (child and attribute
arms of the head gap) and `fixtures/reactive-ir/jsx-census-gap-solid-2` (the
void-element child). Each carries both negatives — a censused tracked read stays
silent, an untracked read outside all JSX stays a proven violation. Coverage: 88
projects, 547 findings; the 86 pre-existing projects and their 542 findings did
not move.

The wording is the claim here, so it is pinned rather than left to review. Both
projects are in coverage's `KEEPS_WORDING` set, so their snapshots carry the
message and hint: deleting `strict_read_message`'s census branch fails both,
which is how the pin was checked. The evidence chain appears in no snapshot, so
`untracked_evidence_sentence` — both its subjects, the direct read and the
propagated one — is pinned by unit tests in `findings.rs`. The spread arm of
`narrowest_jsx_region_containing` has no fixture (neither producer declines to
census a spread today) and is pinned by a unit test in `execution_role.rs`,
which places a census entry over the sibling child so it fails for the spread
specifically rather than for "nothing was censused".

Remaining, and known:

- ~~**Nested `<div><br>{count()}</br></div>` under the pinned 2.0 producer never
  reaches the checker at all.**~~ **Resolved** by the `c6008f01` pin move, but
  not into silence: see "Two lowering divergences the fork's own contract
  declares binding" below. The producer no longer errors — its census now
  includes the nested void child, because the transform really does lower it —
  and the shape is analyzable. It is *not* a census gap, though: the entry is
  present and claims `reactive-rerun`, which the compiler Solid ships would
  never emit. It gets its own mitigation and its own wording, and
  `fixtures/reactive-ir/jsx-void-child-divergence-solid-2` (with its 1.x
  sibling) pins it.
- **A census gap at module scope is not escalated, and under 2.0 it emits
  nothing at all.** `semantic_execution_role_within` classifies a read with no
  enclosing function body as `ModuleInitialization`, and `missing_jsx_census`
  returns `false` for every role but `UntrackedRendering`, so the escalation
  never considers it. What that leaves is dialect-dependent, and the earlier
  wording here ("still classified `ModuleInitialization`") implied a violation
  that the 2.0 catalog does not in fact emit: `CatalogCapabilities::SOLID_2` sets
  `module_scope_strict_reads: false` (the rc.0 runtime installs strict-read
  contexts only inside component and effect bodies), so `project_findings` drops
  the read and a module-scope census gap produces no finding under 2.0. Only the
  1.x catalog, which sets the flag `true` to keep upstream `reactivity`
  semantics, reports one — as an ordinary proven untracked read, with the
  pre-escalation message. The rationale for not escalating is unchanged: module
  initialization is an AST-proven one-shot context needing no census, since no
  owner or subscriber can be active before a containing function runs. If a
  producer is ever found to wrap module-scope JSX in a reactive effect, that
  rationale — and the 1.x finding resting on it — is where it would be wrong.
- ~~**The hint still says "Solid warns STRICT_READ_UNTRACKED here in dev"** on an
  uncertifiable finding, in the **2.0** catalog.~~ **Fixed (2026-08-24).** The
  hint asserted a runtime behavior none of its three uncertifiable carriers can
  prove: unenumerable callers (`ReactiveRead::uncertain`), a census hole
  (`ReactiveRead::missing_jsx_census`), and divergent lowering
  (`ReactiveRead::divergent_lowering`, where the read may not even execute).
  `strict_read_hint` in `rust/dialects/solid-v2/rules/src/lib.rs` now takes an
  `uncertifiable` flag — `ReactiveRead::is_uncertifiable`, the same predicate
  `projection.rs` uses to set `finding.kind = "uncertifiable"`, so the hint and
  the kind can never disagree — and picks between two trailing sentences. (The
  predicate was duplicated verbatim in both places when this landed and was
  hoisted to that one method on 2026-08-24; see the namespace-surface entry at
  the end of this file.) A proven violation keeps the original, unconditional
  sentence:

  > Solid warns STRICT_READ_UNTRACKED here in dev.

  An uncertifiable finding now gets a conditional one that stays true
  regardless of which of the three carriers applies — including divergence,
  where the antecedent may simply never fire:

  > If this read executes untracked in dev, Solid warns STRICT_READ_UNTRACKED
  > for it; this finding does not establish that it does, so confirm the read's
  > actual execution and tracking status before relying on that warning.

  Everything before that sentence (the actionable "move the read into a
  tracking scope" advice) is unchanged in both branches — it stays correct
  advice independent of whether the finding is proven. 1.x is untouched:
  `rust/dialects/solid-v1/rules` has no such hint to correct, confirmed by
  `rg "in dev"` still matching zero there.

  **Gates for this slice** (it recorded none when it landed): the v2 rules
  library (`cargo test -p solid-v2-rules --lib`), `ir-lib`, and coverage against
  a fresh debug binary — the hint text is snapshotted for the wording-under-test
  projects listed in `scripts/coverage.mjs`, so a hint change cannot move
  silently. All three were re-run on 2026-08-24 with the predicate hoist, with
  no finding moved.

## Two lowering divergences the fork's own contract declares binding (2026-08-24)

The `dom-expressions-compiler` pin moved
`66004ab78fa10412208d1bc8cb301bfc028ea826` →
`c6008f01df199ff0f0d072093e2393ed3d67f0c4` (fork PR #2 merged). The census now
covers a nested void element's children, the textarea `value` fold and the
inert `<noscript>` fast path retract the children they discard, and the fork's
`docs/execution-contract.md` gained a section — "The trace describes this
compiler, not the parity target" — that names every known divergence from the
Babel plugin Solid actually ships, with the emitted code of both compilers as
evidence, and states the rule as **binding on the consumer**:

> A consumer must not certify from facts an affected divergence touches; there
> the trace is accurate about this compiler's output and inaccurate about the
> parity target, and only the consumer knows which it is reasoning about.

That is a real hazard, not a formality. The trace is truthful, the facts are
present, and believing them anyway certifies behavior for a compiler that may
not build the user's project. Three of the four named divergences reach this
checker's rules; all three are now answered — two by a new mitigation
(divergences 1 and 3, which emit) and one by the pre-existing census-gap path
(divergence 2, which retracts). The fourth is a deliberate hard failure in the
producer.

### 1. Void-element children — the fork emits, Babel deletes

`<div><br>{count()}</br></div>`:

| compiler | emitted code |
| --- | --- |
| the fork (`lower_dynamic_native_child` walks into `lower_dom_children` unconditionally) | `_$insert(_el$2, count)` |
| Babel `babel-plugin-jsx-dom-expressions` (shipped) | nothing — the child list is discarded in every position |

The census follows the emission, so the site arrives as
`jsx-child` / `reactive-rerun` → `RegionReason::JsxChild` → `TrackedJsx`. Before
this slice that role made the read **silent**: a certification-by-silence built
on a fact about the wrong compiler.

The mitigation is `divergent_lowered_child` in
`rust/crates/solid-reactive-ir/src/execution_role.rs`, beside `missing_jsx_census`
because it is the same kind of thing — consumer policy over producer facts, with
one dialect-owned input (the parity-target-only tags; see the correction below).
It carries `ReactiveRead::divergent_lowering`, which `projection.rs` reports
(whatever the role) and marks `uncertifiable`, with its own message and evidence
sentence in `findings.rs`. Neither reading is
certifiable: *tracked* believes only the fork, *stale untracked read* believes
only Babel.

Detection is **positive, from solid-facts' own AST**, and that is load-bearing
in two ways. Detecting by census absence would be wrong because `census_touches`
overlap lets a wider censused region shadow a narrower hole — and, decisively,
because after this pin the divergent child *has* an entry claiming
`ReactiveRerun`. The one compiler fact consulted is also positive: a
`jsx-expression` operation inside the void element's child region. That is what
separates the divergence from an ordinary census gap, and it is what makes the
per-producer answer below come out right.

The void-element tag list lives in exactly one place, `VOID_ELEMENTS` in
`execution_role.rs`, byte-checked against `void_elements` in
`packages/compiler/src/shared/constants.rs` at **both** pinned producer
revisions (identical at each: `area base br col embed hr img input link meta
param source track wbr`). A tag this list missed would be a divergent child the
checker certified.

Three consumption points, all closed:

- **Rerun certification** — the read is reported `uncertifiable` instead of
  being silently certified tracked.
- **Ownership attribution** — `owners.rs` skips an `ownership_regions` entry
  inside a divergent void child, in `providing_regions` (so a leaf primitive
  there is not certified owned) and in `compiler_owner_context` (so a function
  body there is seeded neither owned nor proven-unowned). The fork wraps the
  insert it emits; Babel emits neither insert nor wrapper.
- **Reactive-reader satisfaction** — the props-destructure autofix in
  `owners.rs` requires every reference to sit in a tracked JSX position. A
  reference inside a divergent void child satisfied that only through the fork's
  own lowering, so it now refuses the fix. No fix beats a fix whose soundness
  rests on which compiler runs.

**Per-producer, probed with the fresh debug binary, not assumed:** 1.x lowers a
void element's children in the *template-root* position too, so
`<br>{total()}</br>` at a component root is the divergence under 1.x and an
ordinary census gap under 2.0 (whose `lower_dom_element` gates on
`!is_void_element` there, agreeing with Babel). Same source, same
`uncertifiable` verdict, different mechanism, different message. The pair
`fixtures/reactive-ir/jsx-void-child-divergence-solid-{2,1x}` holds
byte-identical sources (enforced through coverage's `IDENTICAL_SOURCES`) so that
this one-message snapshot difference *is* the claim.

### 1b. `<noscript>` children — the fork emits, Babel never lowers

Divergence 3, and the same hazard by a different route. `<noscript>` markup is
inert, so Babel drops its children in every position; this fork drops them only
on the static-template fast path. `<noscript>{a()}</noscript>`:

| compiler | emitted code |
| --- | --- |
| the fork, at template root or off the fast path | `_$insert(_el$, a)` |
| Babel (shipped) | nothing — `<noscript>` children are dropped in every position |

The contract states it as binding in the same words as divergence 1: *"a
consumer must treat a `jsx-child` site inside a `<noscript>` as
uncertifiable."* **Probed before implementing**, and the measurement is the
reason this arm exists: `<noscript>{a()}</noscript>` and
`<div><noscript id={tag()}>{c()}</noscript></div>` were **silently certified** in
*both* dialects — no finding at all — which is precisely the wrong-certification
class this slice removes.

It is the **same predicate, a second named condition** —
`INERT_MARKUP_ELEMENT` in `execution_role.rs`, not a fifteenth entry in
`VOID_ELEMENTS`. That list's entire value is that it matches the compiler's
`void_elements` byte for byte, and `<noscript>` is not in it: it has an ordinary
content model and diverges because its markup is inert, not because it is
childless. Merging them would destroy the one property that makes the void arm
auditable. `ReactiveRead` therefore carries `Option<DivergentLowering>` rather
than a bool, because the two conditions need **different sentences**: Babel
*deletes* a void element's child list, while it never *lowers* a `<noscript>`
subtree at all, and "deletes it" would name a compiler step that does not
happen. Where the two nest, the narrowest enclosing divergent element decides
the wording — both answers are uncertifiable, only the nearer is the reason.

Positions, probed at both pins:

| shape | 2.0 | 1.x |
| --- | --- | --- |
| `<noscript>{a()}</noscript>` (template root) | divergence | **identical** |
| `<div><noscript id={tag()}>{c()}</noscript></div>` (dynamic attribute forces it off the fast path) | divergence | **identical** |
| `<div><noscript>{b()}</noscript></div>` (static fast path) | retracted → census gap | **exit 2** (`unresolved execution sites`) |
| `<div><noscript id="d">{d()}</noscript></div>` (a *static* attribute does **not** force it off the fast path) | retracted → census gap | **exit 2** |

The first two are pinned in the divergence pair, which stays byte-identical
because both producers agree there. The retracting position is pinned 2.0-only,
as `RetractedInertNoscriptChild` in
`fixtures/reactive-ir/jsx-census-gap-solid-2` — where it doubles as the
mechanical guard that the mitigation keys on a *lowered site* and not on the tag:
key it on the tag and that arm flips to the divergence wording, and the fixture
is in `KEEPS_WORDING`, so it fails the gate. The 1.x rejection is recorded below
rather than pinned, for the same reason as the `textContent` arm.

### 2. Nested dynamic-`textContent` children — the fork retracts, Babel inserts

`<div><span textContent={x()}>{y()}</span></div>`:

| compiler | emitted code |
| --- | --- |
| the fork (missing Babel's `!hasChildren` gate on the nested placeholder branch) | template `` `<div><span> ` ``, no insert; the children's censused sites are **retracted** |
| Babel (shipped) | template `` `<div><span>` ``, `_$insert(_el$2, y)`, and an effect writing `_el$3.data` where `_el$3 = _el$2.firstChild` |

Here the fork's absence must not be read as no-execution — the fork's contract
says so explicitly. **No new mechanism was needed**: the retraction lands as a
hole in the narrowest JSX region, so the pre-existing `missing_jsx_census` path
already reports it `uncertifiable`, which is the honest answer for the same
reason. Verified with the fresh debug binary and pinned as the retraction arm of
`fixtures/reactive-ir/jsx-census-gap-solid-2`, whose README now states both ways
a hole arrives (never censused, and censused-then-retracted) and why the
mitigation cannot key on which.

SC8003 fires on the same element for an unrelated and legitimate reason (JSX
children and `textContent` at once). Two claims about one element, neither
duplicating the other, and neither duplicating `tsc`: the oracle reports **zero
diagnostics** for this source against the real `@solidjs/web@2.0.0-rc.0` and
`solid-js@2.0.0-rc.0` typings, under both `strict` and `loose`.

### Measurement

Coverage moved 88 projects / 547 findings → **90 projects / 558 findings**,
confirmed with the cache disabled (`SOLID_CHECKER_GATE_CACHE=0`) against the
fresh debug binary. Every one of the eleven new findings is in a fixture this
slice added or extended, and **nothing else moved**:

- +8 from the new pair (four `uncertifiable` SC1001s each: nested void,
  template-root void, `<noscript>` root, `<noscript>` off the fast path);
- +3 in `jsx-census-gap-solid-2`'s two retraction arms (two `uncertifiable`
  SC1001s and the SC8003 on the `textContent` element).

Two kinds of pre-existing snapshot line changed, both bookkeeping: byte offsets
in `jsx-census-gap-solid-2`, shifted by the components added above them, and one
apparent `violation` → `uncertifiable` at a fixed array index there, which is the
positional shift from inserting a finding ahead of it. Checked by enumerating the
fixture rather than trusting the diff: `ReadOutsideJsxStaysAViolation` is still an
SC1001 **violation** and `TrackedChildStaysCertified` is still silent, so both
negative controls hold. Ownership gate: 289 cases passed, ledger 465 rows,
0 pending.
`make verify` green end to end.

### Droppable when upstream fixes the transform

Every mitigation here is pinned to a producer defect, not to Solid semantics.
Each is a straight deletion once the fork's `transform()` output stops diverging
— divergence 1 by gating nested void-child lowering on `!is_void_element`,
divergence 3 by dropping `<noscript>` children off the fast path too, divergence
2 by restoring Babel's `!hasChildren` gate — and the census follows the emission
automatically. Until then removing any of them silently restores a certification
the facts do not support, so each carries that reasoning in its doc comment as
well as here. The two conditions are independent: divergence 1 and divergence 3
can be fixed upstream separately, and `divergent_lowered_child` is written so
that retiring one leaves the other intact.

### Remaining, and known

- **Other consumers of the same divergent `TrackedJsx` role still read it**: the
  destructure-freshness discharge in `static_rules.rs` (a tracked role means
  "fresh at call time", so it `continue`s), `resolve_tracked_scope` in
  `static_api.rs` (a `resolve()` inside a tracked JSX region reports), and the
  post-flush server rules in `server_rules.rs` (which match `TrackedJsx` and
  `UntrackedRendering` alike). Each needs its own uncertifiable path, one rule at
  a time.

  **SC1003 no-destructure belongs on this list too**, and its state is partial
  rather than untouched: on a divergent case it stays a **violation** while its
  autofix is refused (the props-destructure fix in `owners.rs` returns `None`
  whenever a reference sits in a divergently lowered child, since the tracked
  position that would make the rewrite sound exists only under the fork). That is
  deliberate for now — the destructure itself is a defect under either compiler,
  so the violation is not a divergence artifact — but the finding's *kind* has
  never been re-examined against the case where the whole subtree may not exist,
  and the fix refusal is silent, so a user sees a rule with no autofix and no
  reason given. Recorded here so the half that is done does not read as the whole. Forcing the *role* away from `TrackedJsx` would reach all three at once
  and is deliberately **not** done: `UntrackedRendering` is in
  `reports_disallowed_write`, so the role flip would manufacture proven
  disallowed-write findings inside a subtree that may not exist — a worse claim
  than the certification it removes. Today's behavior at all three is silence or
  a pre-existing report, never a new false positive.
- **Divergence 2's shape is fine here for a different reason.** The fork
  retracts, so the pre-existing `missing_jsx_census` path already answers
  uncertifiable; see section 2 above. No divergence-specific mechanism was
  needed, and none was added.
- ~~**Divergence 4 stays a hard reconciliation failure by design.** A `children`
  attribute on a nested native element with no source children
  (`<div><span children={x()}/></div>`) makes the census name a `jsx-child` site
  lowering never resolves, so the file is rejected and `solid-checker-rust`
  exits 2. The fork keeps that deliberately: the failure is the divergence's only
  detection signal. There is no fixture, for the same reason as before — a
  fixture would pin an exit code rather than a semantic claim.~~ **Resolved
  (2026-08-24)** by moving the pin to `fea62adb5d0332a4a3cb5088e97283673c40b540`
  — see "Divergence 4 is resolved, and it surfaced five more" below.
- **The 1.x producer used to fail the file on *both* retraction shapes.** Probed under
  `solid1-dom-expressions-compiler` at
  `b66c3e34ba2a0b74238726eb2b83f767eacf94f2`: both
  `<div><span textContent={x()}>{y()}</span></div>` and
  `<div><noscript>{b()}</noscript></div>` report `semantic trace has unresolved
  execution sites: JsxChild@<span>` and exit 2. The `d1e08958` pin closes the
  static-`<noscript>` half: `TraceRecorder::retract_within` now withdraws the
  sites under that fast path, plus both hydratable-`<head>` replacement paths,
  without changing emitted JavaScript or trace schema. The 1.x census-gap
  fixture now pins the `<noscript>` arm. The dynamic-`textContent` shadowed-child
  path is still not among the producer's three discarded-subtree retractions,
  so that half remains a file-level reconciliation failure and stays unpinned
  here rather than normalizing an exit code into a semantic expectation.
- ~~**The `uncertifiable` hint still says "Solid warns STRICT_READ_UNTRACKED here
  in dev"** under 2.0, on the divergence finding exactly as on the census-gap and
  `uncertain` ones.~~ **Fixed (2026-08-24)** by the same v2 wording slice as the
  census-gap entry above — see "The hint still says..." earlier in this
  document for the new conditional wording and why it holds for a divergence
  read that may not execute at all.

## The divergence mitigation manufactured one violation and missed one dialect (2026-08-24)

Two defects in the mitigation above, found in its own final review. Both are
fixed; both were the mitigation getting *more* wrong than the behavior it
replaced, in opposite directions.

### The ownership skip manufactured a proven SC4001

`providing_regions` (`owners.rs`) drops a divergent child's `Owned` ownership
region, correctly: the parity target emits neither the insert nor the wrapper
that region describes, so it is not evidence of an owner. But dropping it is only
half an answer. Where the ambient context is *proven unowned* — module scope —
the requirement then stands with nothing to satisfy it, and SC4001 reported a
**proven violation**. Reproduced with the fresh debug binary in both dialects:

~~~tsx
export const Divergent = <div><br>{(onCleanup(() => {}), null)}</br></div>;  // SC4001 violation — WRONG
export const Control   = <div><span>{(onCleanup(() => {}), null)}</span></div>;  // silent
~~~

`createEffect` reached it identically under 1.x (v1/missing-owner; under 2.0 the
one-argument call is an unrelated SC7001).

No fact supports that violation. Under the pinned fork the child *is* lowered and
the insert's wrapper owns the call; under the parity target the child is deleted
and the call never runs. Neither compiler produces the unowned live operation the
finding asserted — the finding existed only because the ownership evidence was
removed from one side of the proof while the other side kept asserting absence.

**Fix.** `OwnerRequirement` now carries `divergent_lowering:
Option<DivergentLowering>`, the same carrier `ReactiveRead` has, and the
requirement is marked uncertain when it is `Some`. `findings.rs` gives it its own
message and evidence — naming the disagreement, and *replacing* rather than
appending to the "no containing owner dominates this operation" sentence, which
would otherwise still assert a completed search for an owner.

The seam is `push_owner_requirement`: the one funnel both owner passes
(`find_missing_owners` in batch, `discover_owner_file` plus the incremental
emission) and all four operations go through. It takes `file` and the dialect and
derives the divergence itself, so the two passes cannot drift and a new candidate
kind cannot forget the escalation. Four call sites lost their `path` argument in
exchange.

This is the one consumption point of the divergence that *reports* where nothing
was reported before the mitigation existed. It reports a proof obligation, never
a defect; `docs/compiler-facts.md` said "all of them fail-closed rather than
newly-reporting" and now states this exactly.

Pinned by `CleanupInsideADivergentChild` (uncertifiable), its `<span>` twin
(silent — the escalation is positional and narrow), and a bare module-scope
`onCleanup` that stays a proven **violation**, all in the divergence pair; plus
`findings.rs`'s
`a_divergent_child_makes_an_owner_requirement_uncertifiable_not_a_violation`,
which pins the wording and the absence of the no-owner sentence, neither of which
a snapshot carries.

### The void tag set is dialect-dependent; 1.x silently certified `<keygen>`

`VOID_ELEMENTS` was byte-checked against the producers' `void_elements` and
described as "the only copy of the tag list". It is the only copy of the
*producers'* list, and a divergence is a producer disagreeing with **its own
parity target** — which is not the same list on both sides:

| | Rust producer | parity target |
| --- | --- | --- |
| 1.x | 14 — `void_elements`, `packages/compiler/src/shared/constants.rs` @ `b66c3e34` | **16** — `packages/babel-plugin-jsx-dom-expressions/src/VoidElements.ts` @ `b66c3e34`, adding `keygen` and `menuitem` |
| 2.0 | 14 — same file @ `c6008f01` | 14 — `VoidElements`, `packages/runtime/src/constants.js` @ `c6008f01`, imported by `babel-plugin-jsx` |

The 1.x plugin computes `voidTag = VoidElements.indexOf(tagName) > -1` and gates
its whole child pass on `if (!voidTag)`, so it deletes a `<keygen>`'s and a
`<menuitem>`'s children in every position. The 1.x producer lowers them, censuses
`ReactiveRerun`, and the checker **certified the read by silence** — the exact
failure class this mitigation exists to remove, surviving inside it. Probed
before and after with the fresh debug binary: five void-child arms, only the
`<br>` control reported before; all five report under 1.x now.

A union list was the wrong fix. 2.0's parity target dropped both tags
deliberately (`packages/babel-plugin-jsx/CHANGELOG.md`, `1cc342c`), so under 2.0
both compilers lower those children and the read is genuinely certifiable;
reporting it would withhold a certification the facts support.

**Fix.** The extras are dialect vocabulary and travel the dialect seam:
`Dialect::parity_target_only_void_elements` in `solid-dialect`, answered
`["keygen", "menuitem"]` by `Solid1x` and `[]` by `Solid2`, each naming its
parity target's file and revision at the implementation. It is **required**, not
defaulted: an empty answer is a claim about a specific parity target's list and a
new dialect must make it deliberately. `divergent_candidate_child` joins the two
lists at the one question that needs the union — "does the compiler this project
builds with drop this element's children?" — so neither list absorbs the other's
provenance, and `divergent_lowered_child` now takes the dialect (which is
available at every call site through `SemanticLookup::dialect`; the
props-destructure fix took a new parameter for it).

The extras take the shared `VoidElementChild` wording, since the reason is the
same, and the positive lowered-site fact still gates them.

Pinned by `NestedKeygenChild` and `RootMenuitemChild` in the divergence pair —
uncertifiable under 1.x, **silent under 2.0** — which is the first place that pair
has arms whose *verdict* differs rather than only their wording. The sources
stayed byte-identical, so `IDENTICAL_SOURCES` is unchanged. Unit coverage:
`a_parity_target_only_void_tag_diverges_under_that_dialect_alone` (both tags, both
positions, and the shared `<br>` still diverging under both dialects) and
`a_parity_target_only_void_tag_still_needs_a_lowered_site`.

### tsc oracle, checked rather than assumed

`<keygen>`/`<menuitem>` with children: **zero diagnostics** against both real
audited typings, `strict` and `loose`. Both published typings still declare them
as ordinary intrinsic elements, so the checker is not duplicating a type error
there and the fixture arms are legal shapes.

The ownership arm was *not* legal as first written. `{onCleanup(() => {})}` as a
JSX child is TS2322 against both packages — `Type '() => void' is not assignable
to type 'Element'` under `solid-js@1.9.14`, `Type 'Disposable' is not assignable
to type 'Element'` under `2.0.0-rc.0` — because neither return type is a
`JSX.Element`. Accepting it would have required a stub looser than the real
package, so the arm is `{(onCleanup(() => {}), null)}`, which both real typings
accept and which keeps the call inside the divergent child region. The whole
`App.tsx` compiles with zero diagnostics in both dialects, both modes.

### Remaining, and known

- **`<keygen>` makes both producers print to stderr.** `The HTML provided is
  malformed … Browser HTML: <keygen>` — their template round-trip validator
  follows the HTML standard's legacy void list while their lowering does not,
  which is arguably a fifth divergence, in the producers' *output* rather than
  their census. No gate reads stderr and no finding depends on it, so it is
  recorded rather than acted on. It is also mild evidence against 2.0's
  CHANGELOG rationale ("no longer parsed as void by modern browsers"): the
  producer's own HTML parser disagrees.
- **No oracle-ledger case was added.** `fixtures/tsc-oracle/rule-cases.json` has
  no void-element case at all, and neither fix adds or narrows a rule, so the
  gate's invariants are untouched. A `<keygen>` case under v1 would be a
  textbook keystone row (TypeScript silent, checker reports) and is the obvious
  next addition.
- **The parity-target lists are hand-transcribed, like the producers' list.**
  `VOID_ELEMENTS` has always been byte-checked by reading, not by a build-time
  assertion, and the new extras are the same: a comment naming a file and a
  revision. Nothing fails if a pin moves and the parity target's list changes
  underneath it — the same known weakness, now in two places instead of one.

## Divergence 4 is resolved, and it surfaced five more (2026-08-24)

`dom-expressions-compiler` moved from `c6008f01df199ff0f0d072093e2393ed3d67f0c4`
to `fea62adb5d0332a4a3cb5088e97283673c40b540` (upstream PR #3, "nested children
attribute promotion"), the fork's first deliberate transform change since the
census/lowering reconciliation work above. It resolves divergence 4 — the one
named "stays a hard reconciliation failure by design" two sections up — and its
own `docs/execution-contract.md` at the new revision documents five further
divergences (5-9), all pre-existing, found while fixing it.

### What changed in the producer

Before this pin, `<div><span children={x()}/></div>` — a `children` attribute
on a nested native element with no source children — made the producer's own
census name a `jsx-child` site that `lower_dynamic_native_child` never resolved:
`lower_dom_element` already promoted a template-root `children` attribute to a
child insert, but the nested path had no equivalent capture. The fork treated
that unresolved site as the divergence's only detection signal and kept it
deliberately: the file failed reconciliation and `solid-checker-rust` exited 2
before this checker's project analysis ever ran.

`lower_dynamic_native_child` now performs the same promotion, gated exactly as
the template-root path is (`!is_void_element`, `!has_spread`, an empty source
child list, and the value not resolving under `evaluate_confident`), so the
value joins the (empty) child list as an ordinary expression container and
lowers like any other nested child. The fix also corrects a latent dedup bug in
`children_attribute_container`, present in the template-root path too: Babel's
own attribute dedup selects the last attribute *named* `children` before
judging whether its value is literal, so a trailing literal duplicate
(`<span children={x()} children={"s"}/>`) must block promotion outright rather
than falling through to an earlier non-literal `children` attribute the dedup
already discarded — the old `rev().find_map(...)` selected by value shape
first and got this wrong.

### Why this checker needed no new mitigation

Divergences 1 and 3 needed `divergent_lowered_child` because the producer's
census carried a *present, positive* fact — `jsx-child` / `reactive-rerun` —
that was true about the fork and false about the compiler Solid ships, and this
checker had to actively distrust it. Divergence 4 never reached that point:
before this pin, a project containing the shape never got past the producer at
all, so there was no census entry to distrust and nothing for a mitigation to
intercept. After this pin the file compiles and the shape is an ordinary
`jsx-child` / `reactive-rerun` site indistinguishable from any other nested
child, so this checker's existing tracked-JSX and ownership machinery handles
it with no Rust change. Pinned in the new fixture
`fixtures/reactive-ir/jsx-nested-children-attribute-solid-2` (2.0-only — the
pin that moved is 2.0-only; Solid 1.x is built by its own fork,
`solid1-dom-expressions-compiler`, untouched by this move):

- the promoted value is read exactly like an ordinary tracked JSX child
  (silent — certified);
- a confidently-foldable `children` value is still never promoted (silent — no
  reactive site at all);
- the with-source-children shape is unaffected: `captured_child` is gated on
  `child.children.is_empty()`, a condition that shape never satisfies either
  side of this pin, so it falls through to the ordinary attribute-lowering path
  exactly as before, and ~~its `children`-attribute read is a genuine SC1001
  **proven violation** (an elided value read outside any tracked or deferred
  context, unchanged by the pin)~~ **that claim was wrong and is corrected
  below** ("`Value(Elided)` was projected as code that runs", 2026-08-24): the
  attribute is *deleted*, not evaluated once, so the read is silent. Its real
  JSX child stays certified either way;
- an ownership arm shows the promoted value's insert is owned by the fork's
  default effect wrapper exactly as any other nested child's is.

`<span children={ignored()}>{visible()}</span>` also draws a real TS2710 —
checked against the real `@solidjs/web@2.0.0-rc.0` / `solid-js@2.0.0-rc.0`
typings with `scripts/tsc-oracle.mjs`, not assumed — but that pair is already
narrowed out of `jsx-no-duplicate-props` (`only_the_children_pair` in
`upstream_compat/solid1x_syntax.rs`; see "Landed 2026-08-17" above), so this
checker's SC8003 stays silent there for an unrelated, already-audited reason.

### What did not move

The checked-in `fixtures/findings-snapshots/*.json` were last written under
the old pin. With the pin moved and a fresh debug binary built
(`SOLID_CHECKER_BIN` pointed at `rust/target/debug/solid-checker-rust`),
`node scripts/coverage.mjs` (no `--update`) recomputed all 91 fixture projects
against those snapshots and reported exactly **one** project differing: the new
fixture, which had no snapshot yet. No existing fixture contains the
divergence-4 shape — it could not have, since a project containing it never
reached this checker's analysis at all under the old pin — so nothing else
could move, and nothing did. `--update` then recorded only that one project
(565 findings total, cache: 90 hits / 1 miss on the immediately following run,
confirming the other 90 were untouched). Ownership gate: 289 cases passed,
ledger 465 rows, 0 pending, unaffected.

Divergences 1-3's mitigations and the census-gap path are unchanged by this
move: `divergent_lowered_child` and `missing_jsx_census` in
`rust/crates/solid-reactive-ir/src/execution_role.rs` still hold, and none of
divergence 1 (void-element children), 2 (nested dynamic `textContent`), or 3
(`<noscript>` children) is touched by PR #3's diff. `docs/compiler-facts.md`
records divergence 4 as resolved and divergences 5-9 as open in one place;
this entry is the detailed record.

### Divergences 5-9 were open at this pin, and reached no rule here

None of the five newly-documented divergences reached a rule in this checker at
this pin, and none was introduced by it — the fork's contract states all
five as pre-existing, surfaced only because resolving divergence 4 required
auditing the nested and template-root `children`-attribute paths against each
other:

5. **Template-root slot order** — a `children` attribute before a dynamic
   `textContent` loses the slot to Babel's captured-value overwrite, but this
   fork's template-root capture ignores attribute order and inserts the
   `children` value anyway. Nested lowering (this pin) follows Babel's order;
   the template root does not.
6. **JSX-valued holes** — `() => (() => {…})()` vs. `() => {…}` for a JSX
   element inside a hole, unrelated to `children`.
7. **`undefined`/`null` `children`** — Babel's literalness test accepts a
   string or number only, so it promotes `undefined`/`null` too; this fork's
   `evaluate_confident` filter is broader and keeps them as attributes.
8. **Nested custom-element owner context** — a template-root custom element
   gets the `contextToCustomElements` owner assignment; a nested one does not.
9. **Textarea `value` fold on a constant-but-non-literal-spelled expression**
   (`value={"a" + "b"}`) — Babel's literalness test is by AST node type before
   its own constant fold runs, so it stays a stateful property write; this
   fork's attribute planner folds first, so it collapses into the template
   exactly as a real literal would, and — the case that matters for
   `children` — a real `children` attribute alongside such a `value` is
   promoted by Babel but not by this fork.

Each required its own probe against the real fork output before a checker rule
could rest on it, the same discipline that produced divergences 1-4's
mitigations. Divergence 5 and divergence 2 are closed by the later producer-pin
entries below; divergences 6-9 remain open.

## `Value(Elided)` was projected as code that runs (2026-08-24)

**Closed for the projection, for the divergence route it exposed and that
route's boundary, and for the two rule funnels that read no execution facts;
four residues recorded open below** — divergence 5's certification residue, the
rules that never ask for an execution role (with one member measured), and the
producer's fail-closed refusal of a shadowed JSX-valued `children` attribute.

Both dialect adapters mapped the compiler's `Value(Elided)` decision onto an
**untracked region**, sharing the arm with `Value(EagerOnce)` under one comment:
*"`EagerOnce` and `Elided` settle at render and never re-run."* That sentence is
true of `EagerOnce` and false of `Elided`. An untracked region is a claim that
the code **executes** — once, at render, outside any tracking scope — so a
reactive read inside a deleted value became a **proven SC1001 violation**: *"the
read sees the current value once and never updates."* Every clause is false when
the read does not happen at all. The adversarial review that found this reported
five distinct shapes firing that false proven violation, with both compilers'
emitted code as evidence; one of them — the fixture arm re-pinned below — is
reproduced and pinned here, and the ownership variant further down was probed
directly.

The mapping predates the census-gap and divergence honesty work (commits
`1ddbac26` / `5fc3c60e` versus `85808728` / `24e7f86f`), which is why it
survived them: those slices asked what the *absence* of a fact licenses, and
this was a present fact whose meaning was misread.

### Nothing the producers mark `Elided` evaluates at runtime

Audited at the then-pinned revisions by reading the emitting path, not inferred
from the name. The 2.0 fork at `fea62adb` had nine emission sites and the 1.x
fork at `b66c3e34` had eight; every 2.0 site was read, and the 1.x sites were
matched to their 2.0 twin by emitting path, with 1.x's one site that has no 2.0
twin (`shared/component.rs`) read directly. Every one falls in one of two
classes:

- **A constant folded into the template.** `children.rs`, `static_template.rs`,
  and the `PlanDisposition::Skip`/`Inline` arms in `attrs.rs`. These reach
  `Elided` only for a value `evaluate_confident`/`static_jsx_expression_value`
  resolves, so they cannot contain an accessor call in the first place.
- **A value discarded unlowered.** A `children` attribute shadowed by real
  source children (`attrs.rs`), a promoted capture the slot's winner drops
  (`children.rs`, the nested `<noscript>` and dynamic-`textContent` cases), a
  spread's `$key` and its skipped `children` (`spread.rs`), and — 1.x only — a
  component's `children` prop shadowed by real children
  (`shared/component.rs`). Nothing is emitted, and the fork's own comments state
  Babel emits nothing either.

`resolve_lowered_attribute` never turns a callback site into an `Elided` value —
it *retracts* `on*` and `ref` sites — so `Elided` only ever arrives on a native
attribute, a spread member, a JSX child, or a component property.

The `eabc563d` port to `next` adds one more 2.0 emission route, reviewed with
the port rather than silently inherited from that count: a dynamic intrinsic
`$key={key()}` in DOM mode. `$key` is server-component markup identity; the DOM
lowering strips it completely, so the value is resolved as
`native-attribute` / `Elided`. SSR retains the expression under the emitted
`_key` attribute and resolves it through its live lowering instead. This is the
only `next`-specific semantic-trace adaptation in the port; the full upstream
census, regression, interface, and transform-baseline suites pass with trace
version 2 unchanged.

### The projection: a discarded region, distinct from an untracked one

`ExecutionMap` gained `discarded_regions` (`rust/crates/solid-facts/src/compiler.rs`)
and the IR gained `ExecutionRole::DiscardedRendering`. `Value(EagerOnce)` is
untouched: it evaluates once, which is exactly what an untracked region says.

Every consumer was decided rather than defaulted:

| consumer | decision |
| --- | --- |
| `ExecutionMap::classifies` / `uncovered_jsx_expressions` | **counts it.** Leaving it out would make every deleted value an unclassified `jsx-expression` and the adapters refuse the whole file. |
| `census_touches` → `missing_jsx_census` | **counts it, so it is not a hole.** The compiler reported on the JSX and said the code is gone; escalating that to an uncertifiable obligation would claim something is missing when nothing is. |
| `execution_role` (compiler facts) | **dominates**, ahead of the narrowest-region competition. |
| `semantic_execution_role` | **dominates**, ahead of every semantic path. Left below them, a read in a deleted value would take a proven-untracked role from an `untrack()` position, or be silently *certified* by a deferred one. |
| `ExecutionRole::reports_untracked_read` | **false.** No SC1001, proven or uncertifiable. |
| `ExecutionRole::reports_disallowed_write` | **false.** A write the compiler deleted runs in no phase, so it is neither a tracked-phase nor a render-phase write. Covers SC1002 and action invocations. |
| async-read selection (`projection.rs`) | **excluded**, so SC5001/SC5003/SC5005 stay silent: a pending read that never happens cannot throw. The exclusion is ordered *first*, ahead of the `read.leaf_owner.is_some()` short-circuit — that clause returns true before any role is tested, so a leaf-owned async read inside a deleted value would have been reported on the leaf owner alone, with reachability unproven. With the leaf-owner pass itself now gated (`cleanup.rs`), that route is not known to be constructible from a fixture; the ordering, not the reachability, is what was wrong, and it is pinned by `a_discarded_async_read_is_excluded_ahead_of_the_leaf_owner_short_circuit`. |
| `server_rules.rs` post-flush writes | **excluded** by the same gate (it names `UntrackedRendering`/`TrackedJsx` explicitly). |
| `contract_callback_execution` | **`None`.** `"inline"` would license a consumer to run the callback eagerly — a positive claim dead code cannot support. |
| `push_owner_requirement` (`owners.rs`) | **no requirement at all**, not an uncertain one. See below. |
| `cache.rs::same_compiler_semantics` | **compared**, so a changed discarded set invalidates a reused generation. |

Dominance is what keeps the class from becoming a certification channel, and it
is sound because of a producer property that was checked rather than assumed:
every `Elided` span is a single attribute or child *value* expression, never a
wider enclosing construct, so a discarded region cannot swallow a live sibling.
Both adapters carry a unit test for the projection
(`deleted_values_are_discarded_regions_rather_than_untracked_ones`,
`one_shot_values_stay_untracked_regions`).

**Silence over a discarded region is not a certification.** It means "both
compilers deleted this", never "this was proven safe" — no rerun, no owner, no
satisfied reactive reader, no settled value.

### The ownership half was the same false claim in another rule

Probed rather than assumed, with a scratch project outside `fixtures/`: an
`onCleanup` written inside a deleted `children` attribute at module scope
reported SC4001 as a **proven violation** — *"this cleanup function will never
run"* — about a call that never runs, because the surrounding context is proven
unowned and no `ownership_regions` entry exists inside deleted code to say
otherwise. `push_owner_requirement`, the single funnel both owner passes use,
now returns without recording a requirement when the span sits in a discarded
region. `uncertain` would have been wrong too: nothing is unproven there, the
two compilers agree the code is gone. The same scratch project is silent after
the change, and the ownership gate is unmoved (289 cases, ledger 465 rows, 0
pending).

### The exception: a divergence outranks the deletion

Where a deletion decision and a named producer/parity-target divergence touch
the same span, the divergence must win — the compilers *disagree* about whether
the code is deleted, so silence would certify one of them. For source child
regions this already held: `projection.rs` reports a read carrying
`divergent_lowering` whatever role the census gave it, so the mitigation is
role-independent by construction.

One route was open, and it is the review finding that made this section bigger
than a projection fix. `divergent_lowered_child`'s region set was an element's
source children only, deliberately excluding attribute spans ("attributes are
not children"). But `children={…}` **is** the child list — Babel promotes it to
a real child before `transformElement` runs — so a template-root
`<noscript children={c()}/>` was **silently certified**: the fork's
`lower_dom_element` promotes and lowers it (`_$insert(_el$, c)`), the census
reports an ordinary tracked child, and Babel's `transformElement` never visits a
`<noscript>`'s children and emits nothing. The identical divergence written
`<noscript>{c()}</noscript>` was correctly uncertifiable. The fork names this
route itself, in divergence 3 at `fea62adb`: *"The root-level
`children`-attribute-promoted variant is the same divergence by another route …
Still divergent."*

The predicate now also considers the `children` attribute's expression value on
the same elements — `children` only, expression containers only, resolved by
exact `local_name` against this checker's own AST — and its positive-lowering
test gained the discarded exclusion that keeps the arm position-aware without
naming positions: the **nested** `<noscript children={c()}/>` discards the
capture rather than promoting it (promoting would emit an insert Babel does
not), both compilers agree, and no divergence may be claimed there. Before
discarded regions existed as a category this exclusion was implicit — a
discarded child list was *retracted*, leaving no site at all — and it stops
being implicit as soon as deletion is expressed as a decision.

Both producer facts the arm rests on are pinned in the 2.0 adapter rather than
reasoned about: `a_template_root_noscript_children_attribute_is_lowered_not_deleted`
and `a_nested_noscript_children_attribute_is_deleted_not_lowered`.

#### The arm needed a boundary, and the census could not draw it

The first cut of the arm chained the `children` attribute's value onto the
candidate child regions unconditionally, and that over-claimed:
`<noscript {...props} children={c()}/>` and its nested spelling both reported
SC1001 **uncertifiable** with divergence-3 wording over a value both compilers
keep. Promotion has conditions — the fork gates its capture on
`!is_void_element && !has_spread && element.children.is_empty()` plus a
non-literal, non-confidently-foldable value, and Babel reaches its
`key === "children"` capture under the matching preprocessing — and only a
promoted value is in the child position at all.

`promoted_children_attribute_value` writes down exactly one of those
conditions, **no spread**, because it is the only one the census cannot express:

- With a spread the producer *still* censuses the `children` member as
  `ExecutionSiteKind::JsxChild` and decides it `ReactiveRerun`
  (`semantic_trace.rs` gates the child kind on
  `has_spread || element.children.is_empty()`; `spread.rs` records the
  decision), because at runtime `spread()` really does assign it as the
  element's children through a `mergeProps` getter. That is a truthful census
  entry, and it is indistinguishable from a promotion to the positive-lowering
  test. Babel's `processSpreads` consumes the attribute into the merged props
  before its capture runs, and the fork lists *"a spread keeps `children` in the
  merged props"* among the shapes the two compilers already agree on (divergence
  4, "the shapes that already agreed still do"). Both keep it, both execute it
  deferred: nothing to be uncertain about.
- Every other condition makes the producer census or resolve the value as
  something other than a lowered child, so the positive-lowering test already
  refuses it: a void element's `children` attribute is
  `native-attribute`/`elided` (the census gates the child kind on
  `!is_void_element` for the same reason lowering does), a shadowed one is too,
  a confidently foldable one is resolved `elided` by the attribute planner, and
  a name-first dedup loser is an elided value span.
- The **parity-target-only** void tags stay divergent through this arm, and
  correctly: the 1.x fork does not treat `<keygen>`/`<menuitem>` as void, so it
  promotes and lowers, while 1.x's Babel skips `transformChildren` for a void
  tag entirely (`if (!voidTag) { … transformChildren … }`). Gating on the shared
  `VOID_ELEMENTS` set instead would have withheld a real divergence.

Probed end-to-end with the debug binary over all four spellings, before and
after. Before: `<noscript children={c()}/>` uncertifiable (correct),
`<div><noscript children={c()}/></div>` silent (correct),
`<noscript {...p} children={c()}/>` uncertifiable (**wrong**),
`<div><noscript {...p} children={c()}/></div>` uncertifiable (**wrong**). After:
the first two unchanged, the two spread spellings silent. Pinned as fixture arms
(`SpreadKeepsChildrenInMergedProps`,
`NestedSpreadKeepsChildrenInMergedProps`) and as unit tests
(`a_spread_carrying_element_promotes_no_children_attribute_in_either_position`,
with `without_a_spread_the_same_two_positions_keep_their_verdicts` as the
control that the gate did not simply disable the arm).

### What moved

`fixtures/reactive-ir/jsx-nested-children-attribute-solid-2` is re-pinned:

| arm | before | after |
| --- | --- | --- |
| `SourceChildrenShadowChildrenAttribute`, `ignored()` | SC1001 **proven violation** | **silent** — a discarded region |
| `NoscriptPromotedChildrenAttribute`, `note()` (new arm) | — (was silently certified) | SC1001 **uncertifiable**, divergence 3 wording |
| `LiteralChildrenAttributeStaysSilent` | silent | silent (spelling changed, see below) |
| `SpreadKeepsChildrenInMergedProps`, `NestedSpreadKeepsChildrenInMergedProps` (new arms) | — (uncertifiable in the arm's first cut) | **silent** — a spread promotes nothing |
| `DestructureInsideADeletedChildrenValue` (new arm) | — (SC1003 proven violation, ungated) | **silent** |
| `DestructureAtComponentBodyScope` (new arm, control) | — | SC1003 **violation** |
| `LeafOwnerInsideADeletedChildrenValue` (new arm) | — (SC3001 proven violation, ungated) | **silent** |
| `LeafOwnerInsideAPromotedChildrenValue` (new arm, control) | — | SC3001 **violation** |
| every other arm | silent | silent |

The two new controls are why that fixture's status is now `violation` rather
than `uncertifiable`, and they are there so no silent arm can pass by its rule
being off: each deleted-value arm sits beside the same call in a live value.

The remaining SC1001 in that fixture is the real defect at the shadowed span's
`tsc` level being left where it belongs: `<span children={ignored()}>{visible()}</span>`
is **TS2710** against the real published typings (re-checked with
`scripts/tsc-oracle.mjs` after the edit — exactly one diagnostic, identical in
`strict` and `loose`), and the absolute rule makes the checker's silence there
mandatory rather than merely acceptable.

`node scripts/coverage.mjs` with a fresh debug binary and no `--update`
recomputed all 91 fixture projects and reported **exactly one** project
differing — that fixture — at every step: after the projection change, after the
fixture edit, and after the ownership funnel change (which moved nothing at all,
0 of 91). No other fixture pins a read inside an `Elided` region; the two that
looked like candidates (`dialect-solid-2` and `eslint-compat`) hold constant or
literal values, silent either way.

The literal arm's spelling moved from `children={"a literal string"}` to
`children={"a" + "b"}`. A *literal-spelled* nested `children` attribute is
reported to crash the parity-target Babel plugin outright in the fork's own
parity harness, so there is no parity-verified verdict to pin for that spelling.
**Provenance:** that observation comes from the adversarial review of this
change, working in the fork repository; it was **not** re-run here, because this
repository has no Babel harness. Recorded as a pre-existing fork-repo divergence
note, not as a claim about this checker. `"a" + "b"` is non-literal in spelling
and confidently foldable in value, which both compilers agree on.

### Closed later: divergence 5's certification residue

`<span children={c()} textContent={t()}/>` at a **template root**: the fork's
template-root capture ignores attribute order and inserts the `children` value
anyway, while Babel's captured-value overwrite gives the slot to the later
dynamic `textContent` and drops the `children` value (the fork's divergence 5;
its nested path follows Babel's order, which is why only the root shape is
affected). The census therefore reports a lowered, tracked child and this
checker **certifies** the read — a certification resting on a fact a named
divergence touches, which is precisely what the consumer rule forbids.

Not fixed in that discarded-region slice, deliberately: the mitigation is not the `children`-attribute arm
above but an attribute-*order* predicate (does a dynamic `textContent` follow a
`children` attribute on this element, at a template root only), which needs its
own probe of both compilers' emitted output for the ordering cases before a rule
rests on it. The producer fix merged as `bf437061`: the template-root capture
now follows Babel's source-order overwrite, so the later dynamic `textContent`
drops the earlier `children` value. Pin `b22de4ad` descends that merge. The
checker therefore consumes an `elided` loser rather than certifying a divergent
tracked child; no consumer mitigation was added or remains. Divergences 6-9
reach no rule here and remain open.

## Closed 2026-08-25: nested dynamic `textContent` keeps existing children

Producer `0ce01d7476367dab2f4d067f4771d5010e347c75` resolves divergence 2. Nested
lowering now uses the same `!hasChildren` gate as template-root lowering and as
the already-correct 1.x producer: source children, a promoted `children` value,
or a textarea `value` replacement block the synthesized placeholder. The
`firstChild` declaration and effect still emit, while ordinary child lowering
supplies the node the effect updates.

The fork's parity harness adds two probes — a real reactive child and a folded
textarea seed — and matches Babel in all ten modes. Its DOM transform baseline
adds exactly those two outputs and changes no prior row. In this checker,
`TextContentChildNowCertified` replaces the former retraction arm in
`fixtures/reactive-ir/jsx-census-gap-solid-2`: SC1001 drops because `body()` now
has a tracked execution site, while the independent SC8003 children-versus-
`textContent` authoring violation remains. Coverage moves only that project,
from 568 to 567 findings across 91 fixtures.

### Two more funnels were fixed, and the rest of the sweep is named

The table above covers the channels a read, write, action, async read, contract
callback or owner requirement flows through — every consumer that *asks* for an
execution role. Two rule funnels reached a verdict without asking, and both
reported **proven violations about deleted code**. Both are now fixed, gated the
same way `push_owner_requirement` is: a positive `discarded_region_contains`
lookup and an early return.

| rule | funnel | what it claimed about deleted code |
| --- | --- | --- |
| SC1003 `component-props-destructure` | `static_rules.rs`, both destructure loops | The freshness test is an allowlist of *fresh-at-call-time* roles, and `DiscardedRendering` is not one, so a destructure inside a deleted value fell through to a proven "the bindings are frozen, this component never updates". |
| SC3001 `leaf-owner-forbidden-call` | `cleanup.rs`, `leaf_owner_operations_for_file` | The pass is entirely lexical and reads no execution facts, so `onCleanup` inside an `onSettled` inside a deleted value was a proven "these nested primitives are never disposed". |

Neither gate adds `DiscardedRendering` to the SC1003 allowlist, deliberately:
that list means "legal because the context is fresh at call time", and a
deleted destructure is not legal-because-fresh, it is absent. Merging the two
would make the claims indistinguishable to the next reader and would silently
answer for any role added to the list later. SC3001's gate sits on the **owner
call**, this pass's single entry, because deletion travels down from there — a
producer `Elided` span is one attribute or child value expression, so every
nested call is contained with it, while a leaf callback resolved in another file
is reachable only through this call site.

Both are pinned end-to-end, with their live controls in the same file, by
`fixtures/reactive-ir/jsx-nested-children-attribute-solid-2`
(`DestructureInsideADeletedChildrenValue` / `DestructureAtComponentBodyScope`,
`LeafOwnerInsideADeletedChildrenValue` / `LeafOwnerInsideAPromotedChildrenValue`).
Each arm was verified load-bearing by disabling its gate and confirming the
finding returns: gates off, that fixture reports seven findings; gates on,
three. The destructure arms are passed a reactive prop rather than a literal
because SC1003's caller-proven gate answers `PropUse::Static` first and would
otherwise silence both arms for an unrelated reason.

### Open: the rules that never ask for an execution role

Rules that reason purely from syntax still report inside a deleted value,
because they consult no execution facts at all. This is **not** audited shape by
shape, and one member of the set is measured rather than hypothetical:

- **Observed.** SC7001 `missing-effect-function` fires as a **proven
  violation** on `<div><noscript children={(createEffect(() => {}), null)}/></div>`
  — a one-argument `createEffect` inside a discarded region (probed with a
  scratch project against a stub carrying both real overloads; the deprecated
  one-argument overload returns `never` in `@solidjs/signals@2.0.0-rc.0`, so
  this is the checker's own claim and not a `tsc` duplicate). Whether that is
  wrong is a real question rather than an obvious yes: the defect is in the
  call's *shape*, which is wrong in the source whether or not the emitter keeps
  it. It is recorded here as undecided, not as licensed.
- **Not audited.** The other `StaticDefect` kinds that can appear inside a JSX
  value expression — `ReactiveReadAfterAwait`, `ComponentReturnsConditionally`,
  `ReactiveHandlerRead`, `HandlerValueUnresolved`, `UncalledAccessor`,
  `DirectMutation`, `ReactiveCallbackUnresolved`, `ReactiveSourceUncaptured` —
  plus every `upstream_compat` static violation (the SC8xxx families) and
  `directive_creations`. None was probed inside a discarded region.

What the sweep does establish is the boundary: every consumer that reads an
execution role was decided, and the two funnels that reached a *reactive*
verdict without reading one are fixed. A rule that reports a syntactic defect
in deleted code is a separate decision, and this note exists so the next reader
does not mistake the first for the second.

### Open: a shadowed `children` attribute holding reactive JSX still fails the producer closed

Probed at `fea62adb` and re-probed after the `eabc563d` port to `next` with a
scratch project and the fresh debug binary:

```tsx
export function Shadowed() {
  const [x] = createSignal(0);
  const [y] = createSignal(0);
  return <span children={<b>{x()}</b>}>{y()}</span>;
}
```

`solid-checker-rust` still exits **2** with
`semantic trace has unresolved execution sites: JsxChild@<x-span>` — the span of
`x()`, inside the JSX-valued attribute. The census walks source, so it names the
`<b>`'s child as a `jsx-child` site; lowering drops the shadowed `children`
attribute *without visiting its value*, so that inner site is neither resolved
nor retracted, the completeness invariant fails, and the whole project is never
analyzed.

This is **producer-side and fail-closed** — the same failure class divergence 4
had before PR #3 resolved it, and the reason that divergence had no checker-side
mitigation to test. Nothing here is a wrong claim; the file is refused rather
than misjudged. The *promoted* variant of the same shape,
`<span children={<b>{x()}</b>}/>` (no source children), certifies cleanly with
zero findings, which locates the gap precisely: it is the shadowed path's
unvisited value, not JSX-valued `children` attributes in general. Related to the
fork's divergence 6 ("JSX-valued holes"), which is about the *emitted shape* of
a JSX-valued hole rather than about this reconciliation failure, and to
divergence 5's residue above; recorded as a residue of this entry because the
discarded-region work is what made the shadowed path interesting enough to
probe. No fixture pins it — a fixture containing it would make its whole project
exit 2 and pin nothing. The port therefore does **not** close this case; it
preserves the producer's fail-closed behavior.

## Closed 2026-08-24: the constructability fact decides `kind`, and the spans it must not be asked at

**Closed for the class residue and the destructuring refusal; the remaining
`Function` residue was closed by the schema-15 follow-up below.** The producer's span-exact `constructability` fact landed
(solid-ts-facts merge `3296ec8c`, wire table schema 14, handshake protocol
still 2, compact demand bit 16, producer ADR 0020). `rust/Cargo.toml`'s
`typefacts` pin moved `e2f7ac5` → `3296ec8c` and `bin/solid-typefacts` was
rebuilt from it.

`export_kind_proof` (`rust/crates/solid-reactive-ir/src/contracts.rs`) is now
the three-way rule the two entries above
([the `kind` claim a bundled artifact contradicts](#the-kind-claim-a-bundled-artifact-contradicts-2026-08-23),
[the refusal path costs enums and untyped values](#the-refusal-path-costs-enums-and-untyped-values-2026-08-24))
named as the fix. The current decision is the 6×5 product of callability's five
answers plus absence and constructability's four answers plus absence:

| | `Constructable` | `NonConstructable` | `Mixed` | `Unknown` | absent |
| --- | --- | --- | --- | --- | --- |
| **`Callable`** | function | function | function | function | function |
| **`UntypedCallable`** | function | function | function | function | function |
| **`NonCallable`** | function | **value** | unresolvable | unresolvable | unanswered |
| **`Mixed`** | function | unresolvable | unresolvable | unresolvable | unanswered |
| **`Unknown`** | function | unresolvable | unresolvable | unresolvable | unanswered |
| **absent** | function | unanswered | unanswered | unanswered | unanswered |

Fourteen cells prove a function, exactly one proves a value, and the other
fifteen refuse. `unresolvable` and `unanswered` both refuse at
`promote_entry_callable`; they are separate variants only so the refusal
message distinguishes "the facts closed nothing" from "a fact is missing". The
table is pinned cell by cell, against an independently restated rule and with
the four bucket counts asserted, by
`contracts::export_kind_proof_tests::every_fact_combination_decides_the_documented_way`.

Deleted with it: `ExportKindProof::DestructuredMember`,
`ExportKindProof::Class`, `ExportKindProof::Undemanded`, the public
`binding_declares_class` and its whole walk (`location_declares_class`,
`identifier_binding_at`, `binding_initializer_symbol`, `binding_is_reassigned`,
`destructured_binding_at`, `location_destructures`). `Callability::Mixed` is no
longer read on its own anywhere in the `kind` decision.

**Measured 2026-08-25 against the full 416-row ecosystem corpus.** The discharge
map below is now an observed result rather than a prediction. Against the
attested-closure state, verified rows moved **275 -> 281**, refused rows
**121 -> 116**, generation failures **15 -> 14**, failing claims **24 -> 11**,
and contradicted `kind` claims **13 -> 0**. Exactly six rows changed outcome and
none regressed; the complete account and binary identities are in
[ecosystem-benchmark.md](ecosystem-benchmark.md#measured-state-2026-08-25-phase-a-constructability-closure-full-corpus-416-probe-rows).

- The **13 wrong-kind claims** those entries left publishing `value` —
  `@solidjs/web@2.0.0-rc.1`'s `ResponseEnvelope` (6 rows, an IIFE-wrapped class
  whose initializer is a *call*) and `@tanstack/*-devtools`' `*DevtoolsCore`
  (7 rows, a class reached only as a tuple element type declared in another
  package) — are now `Constructable` and raise to `function`. Neither shape
  has a class expression in the analyzed artifact, which is why no amount of
  further syntax chasing could have closed them, and both shapes are reproduced
  as fixture rows here
  (`export_kind_proof_tests::a_class_is_proven_by_constructability_alone`). All
  seven TanStack claims now pass and their five rows move `refused -> verified`.
  All six `@solidjs/web` claims now pass too; those two rows remain refused only
  because their attested module-closure note independently blocks promotion.
- The **4 destructured-member refusals** should become decisions. The type
  answers a binding pattern directly: `(class Named {}).name` is a `string` and
  carries both closed negatives, so `@solid-devtools/ui@0.10.3`'s `./theme`
  `color` and its kin publish `value` and their entrypoints emit; a
  static class member (`const { Inner } = Container`) and a tuple element whose
  type is a class (`const [Core] = pair`) are `Constructable` and raise. The
  cost that entry recorded — a destructured primitive refused with the
  class-shaped ones — is discharged, and this one *is* measured locally:
  `fixtures/package-contracts/class-expression-kind`'s `./destructured`
  entrypoint flipped from **refused** to **two published `value` claims**,
  pinned in its `expected.json` and its closure record. Corpus-wide,
  `@solid-devtools/ui@0.10.3` remains verified while its emitted exports move
  **2 -> 13** and its driven/passing claims move **17 -> 28**.
- The **17 `Callability::Unknown` refusals** were unchanged by constructability
  alone, as
  [that entry predicted explicitly](#the-refusal-path-costs-enums-and-untyped-values-2026-08-24).
  The schema-15 follow-up below subsequently closes the signature-less
  `Function` subset. The remaining shapes — a downleveled enum, a value computed
  from an untyped global, and `Object.assign(Object.create(null), …)` — are
  `any`, and constructability reads the same type and fails closed on the same
  flags. That behavior is measured locally by `class-expression-kind`'s
  `./unresolvable` entrypoint, which still refuses with both facts `Unknown`.
  `solid-js@1.9.14`'s `./web` remains the most consequential single refusal in
  the corpus. Its open candidate is still `primitive_value_domain` or an
  object-literal domain demanded at export-specifier spans, and the full run
  confirms that the entrypoint remains refused.

### The refusal message changed shape

`whose runtime kind no closed type answers (Unknown)` became
`… (Unknown, Unknown)` — both facts, in that order. The
`(Unknown)` spelling in
[ecosystem-benchmark.md](ecosystem-benchmark.md) and in
[RFC 0002 amendment A9](rfcs/0002-a9-kind-has-no-unknown-form.md) is a record
of a past measurement and is deliberately not rewritten. A second phrase,
`whose runtime kind no fact covers at all`, is new (see below). Anything keying
on these texts — `scripts/ecosystem-benchmark/lib/classify.mjs` does not today,
though [the entry above](#the-refusal-path-costs-enums-and-untyped-values-2026-08-24)
proposed it — must key on all three.

### A demanded span that came back empty now refuses

`ExportKindProof::Undemanded` mapped an absent fact to "keep the summary", and
`promote_entry_callable` published `kind: "value"` on it — the maximal
certified negative, for a span whose facts simply did not arrive. An
adversarial review proved it reachable: the producer leaves both signature
facts absent when the demanded location has **no covering query node**
(`internal/typefacts/tsgo/semantic_runs.go`, the `queryNode != nil` guard), and
it emits the entity row regardless, so the consumer saw a present row with an
absent fact and read it as an answer.

**The two cases are not distinguishable from the fact table, so both fail
closed.** `ProjectFacts` carries no record of what was demanded, and
`export_kind_proof` is a fact-table function. What makes failing closed on both
honest rather than merely conservative is that at every call site the span *was*
demanded: `demand_plan` sets `callability` — and now `constructability` beside
it, unconditionally, the two never travel apart — at every export specifier and
every exported declaration name (`export_declaration_names` covers destructured
declarator names too), and both `export_kind_proof` call sites pass exactly one
of those spans. An assertion in `solid_facts_backend::semantic_demands`' own
tests pins that invariant, so a demand-plan narrowing that reintroduced a
genuinely undemanded span would fail there rather than silently publish a
`value`.

**The cost, stated plainly.** A future caller of `export_kind_proof` at a span
the demand plan does not cover gets a refusal, not a kept summary. There is no
such caller today, and no corpus row moved: the arm is unreached by any fixture
and is pinned by unit test
(`export_kind_proof_tests::absence_on_either_fact_is_unanswered_not_a_negative`)
instead. Half an answer refuses too — a present `NonCallable` beside an absent
constructability is `unanswered`, not `value`.

### A class declaration's span is not the export's value, and the review brief did not predict it

Wiring the two facts and deleting the syntactic search made
`fixtures/package-contracts/exported-class`'s `DirectError` publish
`kind: "value"`. **This was a real regression, found by the process gate, and
it is not in either entry's discharge map.**

`export class DirectError extends Error {}` has no export specifier — the
exported name *is* the class declaration's name — and the compiler's type at a
class declaration name is the class's **instance** type, which honestly answers
`nonCallable` *and* `nonConstructable`, because an instance is neither. ADR
0020 documents this under "What it does not answer" and the migration notes say
it outright ("Demand at the export-specifier span, never at a declaration
name"); what neither says is what a consumer should do about `export class C {}`,
which has no other span to ask at. The old code got this row right by accident,
through the class-name-span half of the search that was being deleted.

The fix is `class_declaration_name`, and it is deliberately *not* the search
that was retired. It is span identity against this file's own class spans and
nothing else: no symbol walk, no alias hop, no initializer-identifier hop, no
class-expression initializer fact, no assignment scan. `class C {}` binds the
constructor by language definition, and it cannot be defeated the way the
retired search was — a bundler that lowers the declaration away leaves a
*declarator* name, which types as the constructor and is decided by the facts.
`export_kind_proof_tests::a_class_declaration_is_decided_before_the_facts_are_read`
pins that this gate fires for all 30 fact combinations at a class declaration
and for none of the lowered shapes.

This subsection said "one span" when it landed, and that was wrong by one
spelling: `export default class {}` is anonymous, so the export records the
class *node's* span and the same instance-type problem applies there. It
published `kind: "value"` for a constructor until 2026-08-24 — see the
namespace-surface entry at the end of this file.

### Closed 2026-08-24: signature-less `Function` values have a positive kind answer

The Type Facts pin moved `3296ec8c` → `19671a88` (producer ADR 0021, wire table
schema 15, handshake protocol still 2). It adds
`Callability::UntypedCallable` for the signature-less `Function` family:
`Function`, `CallableFunction`, `NewableFunction`, aliases and interfaces based
on `Function`, and intersections containing it. The answer is deliberately
narrow: it proves the runtime value is a function while proving no readable
signature, parameters, arity, or callback behavior.

Both consumers moved with the pin. `export_kind_proof` treats
`UntypedCallable` as the same positive runtime-kind evidence as `Callable`, so
the generated contract raises to `kind: "function"` and keeps `callbacks`
unknown. `dynamic_key_form` accepts it for an identifier-valued custom key,
where runtime callability — not signature introspection — is the required
fact. No other signature-dependent inference was widened.

The previous entry overstated the family. `object`, `{}`, and
`Record<string, unknown>` are not `UntypedCallable`: those declared types admit
non-function values, so `NonCallable` + `NonConstructable` remains the honest
answer and `kind: "value"` remains correct. The updated
`fixtures/package-contracts/function-supertype-kind` pins six positive family
shapes and those three broad negative controls beside a `number`.

**Measured corpus effect:** `@tanstack/ai-devtools-core@0.5.6` moved from a
whole-package generation failure to verified. Both exports now generate, both
planned claims are driven, and both pass. It is the sixth and only
`generate-failure -> verified` outcome movement in the 2026-08-25 run; the other
five gains are the constructability-backed TanStack rows above.

**Remaining producer limits:** `UntypedCallable` does not imply a readable
signature; constructability remains `NonConstructable` for this family;
runtime-value-domain does not add a second positive; union constituent
aggregation remains coarser than a per-constituent proof; declaration lies are
still possible.

### Gates

Producer rebuilt from the new pin (`scripts/build-typefacts.sh`, which reads the
rev out of `rust/Cargo.toml`), then: `facts-lib` (53), `ir-lib` (152, including
the eight `export_kind_proof` tests), backend lib (29), every armed
`solid-facts-backend` process suite including `contracts_process` (53) and
`diagnostics_process` (15), coverage against a fresh debug binary (90 fixture
projects, 564 findings, **no finding moved**), the contract corpus (38
packages, one added), `node --test scripts/*.test.mjs` (281, which `make verify`
does not run and CI does), and `make verify`. `git diff --check`,
workspace-wide `cargo clippy -D warnings`, and `cargo fmt --check` all clean.

**Deferred at the implementation handoff, completed 2026-08-25:** the ecosystem
benchmark installs real packages from the registry, so it was not part of the
original gate pass. The later full 416-row run measured the discharge map above
and rewrote `benchmarks/ecosystem/verification-report.{json,md}`; its harness
tests passed 230/230, and the universal handoff set remained clean.

## Investigated: `./web`, `onSettled`, and undeclared-dependency corpus failures — all honest (2026-08-24)

A re-diagnosis of `benchmarks/ecosystem/verification-report.json` flagged five
rows failing with `Error [ERR_PACKAGE_PATH_NOT_EXPORTED]: Package subpath
'./web' is not defined`, two with `SyntaxError: ... does not provide an export
named 'onSettled'`, and a cluster of `ERR_MODULE_NOT_FOUND` rows, on the
hypothesis that the corpus manifest's floor selection (`scripts/ecosystem-
benchmark/lib/select.mjs`) might be pinning a Solid runtime version older than
a package actually needs. The verification-report aggregate only carries
message-frequency counts (`probeEnvironment.importThrows`), not row
attribution, so the actual failing probe IDs were recovered from the run's
surviving journal
(`.../scratchpad/state-d1/journal.jsonl`, the resumable per-row record
`verify-corpus.mjs` appends before aggregating) and independently confirmed by
real `npm install` of each package in an isolated temp directory. Verdict for
every row checked: **honest**. No manifest pin, floor computation, or install
policy changed.

**The five `./web` rows are not a floor problem, and the opening hypothesis was
wrong about the mechanism.** `solid-js`'s Solid-2 line never ships a `./web`
export subpath at *any* published version — checked directly against the
registry's `exports` map for `2.0.0-beta.0`, `2.0.0-beta.19`, `2.0.0-rc.0`, and
`2.0.0-rc.1`, all four `['.','./types/*','./package.json']` only. DOM rendering
moved to the separate `@solidjs/web` package for the entire 2.0 line, so there
is no "old floor" that would have had `./web` and a "new floor" that lost it.
The five packages —
`@solid-primitives/controlled-props@1.0.0-next.3`,
`@solid-primitives/drag-drop@0.1.0-next.0`,
`@solid-primitives/favicon@1.0.0-next.1`,
`@solid-primitives/upload@1.0.0-next.4` (via its `@solid-primitives/drag-drop`
dependency), and
`@solid-primitives/virtual@1.0.0-next.4` — each ship a compiled bundle whose
JSX output still does `import { ... } from "solid-js/web"` (confirmed by
grepping the installed `dist/*.js`), the Solid-1.x import path that was
retired in the 2.0 split. Reproduced with a bare `npm install
<package>@<version> solid-js@<pinned> @solidjs/web@<pinned>` in a fresh temp
project against **both** the floor (`2.0.0-rc.0`) and head (`2.0.0-rc.1`)
pins for `drag-drop` and `controlled-props`: identical failure at both ends,
which is itself the proof this is not floor-specific — the package's own
compiler output never targets the runtime split its `peerDependencies` claims
to support. Honest failure; the manifest's floor selection followed each
package's own declared `solid-js`/`@solidjs/web` range faithfully
(`^2.0.0-rc.0` for four of the five, an exact `2.0.0-beta.19` pin is unrelated
— that pin belongs to the separate `@corvu-next/*` family, which fails for a
different, unrelated runtime reason and was not one of these five rows).

**The two `onSettled` rows are a package-side dependency-graph/peer-range
inconsistency, not a corpus bug.**
`@solid-primitives/graphql@3.0.0-next.0` and
`@solid-primitives/immutable@2.0.0-next.0` both declare `peerDependencies:
{"solid-js": "^1.6.12"}` — correctly satisfied by the corpus's audited
`solid-js@1.9.14`, so `solid1` selection is faithful to the package's own
claim. But each also has a regular `dependencies` entry on a solid-primitives
package still in active "next" development — `@solid-primitives/keyed:
"^3.0.0-next.0"` for `immutable`, `@solid-primitives/utils: "^7.0.0-next.0"`
for `graphql` — and real npm semver resolves each of those ranges to the
newest matching prerelease under the *same* `major.minor.patch` prefix
(`keyed@3.0.0-next.2`, `utils@7.0.0-next.4`), which are Solid-2.0-only
releases peering `solid-js@^2.0.0-rc.0`. `onSettled` has existed on every
2.0.x prerelease checked (`beta.0` through `rc.1`) and is simply absent from
`1.9.14` — a 2.0-only API. Reproduced with a bare `npm install
@solid-primitives/immutable@2.0.0-next.0 solid-js@1.9.14`: npm resolves the
nested `keyed@3.0.0-next.2` with an `ERESOLVE overriding peer dependency`
warning (not a hard failure — the same npm behavior a real end user hits with
the identical command) and the runtime crashes on the exact reported
`SyntaxError`. The package's own declared floor for `solid-js` is honest by
itself; the package's *own* dependency tree contradicts it. Not a manifest or
selection defect.

**The `@solid-primitives/utils` / `server-only` `ERR_MODULE_NOT_FOUND` rows are
undeclared dependencies in the published artifact, not a peer-completeness
gap.** The "peer-complete installs" policy from
`Give probes an honest browser shim, peer-complete installs, and scaled
budgets` (d8d240a4) only completes peers the *tested* package itself declares
in `peerDependencies` (`peerSpecsFor` in
`scripts/ecosystem-benchmark/verify-corpus.mjs`), and deliberately skips peers
marked optional — matching real `npm install` behavior, which does not
auto-install optional peers either. Checked against the actual npm registry
per package:
  - `@solid-primitives/keyed@3.0.0-next.2` and
    `@solid-primitives/share@4.0.0-next.4` declare **no dependency of any
    kind** — not `dependencies`, not `peerDependencies`, not optional — on
    `@solid-primitives/utils`, yet their compiled `dist/index.js` /
    `dist/social-share.js` unconditionally `import` it. Reproduced with a
    bare, isolated `npm install @solid-primitives/keyed@3.0.0-next.2
    solid-js@2.0.0-rc.0 @solidjs/web@2.0.0-rc.0`: `@solid-primitives/utils` is
    never installed and the import throws exactly the reported
    `ERR_MODULE_NOT_FOUND`. (An earlier combined install of several
    `@solid-primitives/*` packages together made `keyed`/`favicon`/etc. import
    successfully by accident, because a *sibling* package's own declared
    dependency on `@solid-primitives/utils` hoisted it into the shared
    `node_modules` root — which is exactly why the isolated, single-package
    reproduction the corpus itself performs is the correct measurement, and a
    combined test is not.) This is a missing entry in the published
    `package.json`, indistinguishable from what any real consumer's
    `npm install @solid-primitives/keyed` alone would hit.
  - `@solidjs/start@2.0.3`'s `dist/http/index.js` and
    `dist/middleware/index.js` unconditionally `import "server-only"`, but
    `server-only` appears in none of `dependencies`, `peerDependencies`, or
    `optionalDependencies`. Same undeclared-dependency shape, confirmed the
    same way.
  - The `react`/`preact`/`svelte`/`vue`/`@angular/core` throws all trace to
    `@tanstack/devtools-a11y@0.2.2` and `@tanstack/devtools-utils@0.7.0`,
    which declare all five as `peerDependenciesMeta`-`optional` — a real `npm
    install` would not auto-install them either, so a Solid-only probe
    environment correctly lacks them.
  - The `vite` / `@rsbuild/core` throws on `@tanstack/solid-start@2.0.0-rc.1`
    trace to the same pattern: both are declared `peerDependenciesMeta`-
    `optional` on that package.
  - `@solid-primitives/start@0.0.4`'s `Cannot find module
    '.../solid-start/server/ServerContext.jsx'` is the deprecated legacy
    `solid-start` package (the registry itself reports `@solid-primitives/
    start@0.0.4: Package renamed to @solid-primitives/cookies`) reaching into
    a path that package no longer ships; a stale, deprecated release, not a
    corpus install gap.

**No code changed.** `scripts/ecosystem-benchmark/lib/select.mjs`,
`scripts/ecosystem-benchmark/verify-corpus.mjs`, and the manifest
(`scripts/ecosystem-benchmark/manifest.json`) are untouched — every row's
floor/head selection and every peer-completion decision already matches what
a real `npm install` of that exact package would produce. `benchmarks/
ecosystem/verification-report.json` and `report.json` were read but not
regenerated.

## Closed 2026-08-24: a `namespace` member is not a module export, and an anonymous class was published as a value

Two false claims in the same seam — what a module's **export surface** is, and
what the `kind` decision may read at a class declaration. Both were published
by the contract generator, so both reached consumers as certified statements
about a package.

### Phantom namespace-member exports

**Closed.** `export namespace Config { export const inner = 1 } export const
real = 2` generated an entrypoint whose exports were
`["Config", "inner", "real"]`. `inner` is a property of the `Config` namespace
object; `import { inner } from "pkg"` does not resolve, and no build of that
package ever exported it. A merged `class C {} namespace C { export const marker
= 1 }` leaked `marker` the same way, and so did a namespace that was not
exported at all — its own `export const` is one level further from the surface
and was picked up regardless.

Root cause, verified in the code and then against the reproduction
(`contract generate` over a two-line package):

- `AstFacts::exported_bindings` (`rust/crates/solid-facts/src/ast/mod.rs`)
  selected every non-array `BindingFact` whose declaration span is *contained*
  in the export statement's span, excluding only declarators inside a
  **function body**. A `TSModuleBlock` is not a function body, so a namespace's
  declarators passed. `export_declaration_names`, three hundred lines away, had
  it right the whole time: a `TSModuleDeclaration` yields the namespace's own
  name and nothing else.
- The nested `export` statement is an `ExportFact` like any other, and both
  surface enumerations walked every one of them. This is the path that fires
  even when the namespace itself is not exported, and it is the one the
  reproduction was actually hitting first.

The fix adds one fact and one filter. `AstFacts::module_blocks` records the body
of every `namespace`, `module`, and `declare global` declaration (one
`visit_ts_module_block` hook covers all three: `namespace A.B {}` nests
declarations and only the innermost carries a block). `exported_bindings` now
excludes declarators inside such a block, and `AstFacts::module_level_exports`
is the iterator the surface enumerations use. At the time of this fix that was
two call sites — `contract_export_fragment`
(`rust/crates/solid-reactive-ir/src/contracts.rs`) and `exported_names_for_file`
(`rust/crates/solid-facts-backend/src/main.rs`) — and a review pass afterward
found four more name-keyed enumerations still reading the unfiltered
`AstFacts::exports` table; see "Review follow-up" below for why those needed
the same filter and were not caught here. `AstFacts::exports` remains the
complete syntactic table; it is the *surface* that filters. `AST_FACTS_SCHEMA`
moved 36 → 37 for this addition — its own bump, distinct from the 35 → 36 bump
the `initializer_class` deletion below carries (the fact table changed; the
constant is compared only for incremental cache reuse,
`same_reachability_ast` in solid-reactive-ir's `cache.rs`, and is not a
published artifact).

`exported_bindings` had exactly two consumers, both of them export-surface
enumerations, so both wanted the fix; there is no consumer that wanted namespace
members.

**Deliberately left conservative.** The demand plan
(`rust/crates/solid-facts-backend/src/demand_plan.rs`) still demands both
signature facts at *every* export specifier and exported declaration name,
nested ones included. Nothing depended on the phantom rows — the surface no
longer asks about them — and demanding a fact that goes unread costs a walk,
while shrinking the plan would move an invariant `export_kind_proof`'s refusal
honesty rests on and is pinned by
`semantic_demand_plan_is_complete_for_downstream_consumers`. Two other sites
that iterate `ast.exports` (`module_surface_is_unaccounted` in the backend's
`main.rs`, the exported-function scan in its `lib.rs`) also still see nested
exports; both err toward "this is exported", which widens uncertainty rather
than certifying anything, so they were left alone.

**Audit of every checked-in contract: clean.** A throwaway script cross-checked
each `fixtures/package-contracts/*/expected.json` and each
`pkg/contracts/bundled/**` contract's export names against the namespace bodies
declared anywhere in that fixture's tree. **No phantom in any of them**, so no
`expected.json` needed correcting. The one namespace body in a package-contracts
fixture is the JSX namespace in `escaping-private-helper`'s `solid-js` stub, and
none of `Element`, `ArrayElement`, `ElementChildrenAttribute`, `HTMLAttributes`
or `IntrinsicElements` appears in that fixture's contract. The bundled contracts
are not exposed to this bug class at all: they describe compiled `dist`
artifacts (TypeScript namespaces do not survive to JS), their surfaces are
independently generated from and checked against the installed releases
(`scripts/generate-solid1-runtime-surface.mjs`,
`scripts/check-bundled-contracts.mjs`), and none of them lists a JSX-namespace
member. Nothing under `pkg/contracts/bundled/` was regenerated.

`fixtures/package-contracts/namespace-export-surface` is the pin: an exported
namespace whose three members and one nested namespace must not appear, a merged
class+namespace that must publish `Merged` alone and keep it `kind: "function"`,
an unexported namespace, and two ordinary exports as controls. `tsc --noEmit
--strict` is clean on its source, as it must be: this is a claim about a runtime
export surface, and no type error covers it.

### Review follow-up: four more enumeration sites, and a class static block

A review pass over this fix found the two closed defects above were not the
whole bug class, in the same seam.

**Four more name-keyed enumerations still matched a nested specifier.**
`contract_export_fragment` and `exported_names_for_file` were not the only
consumers that need to bind a name to *this module's* export, not to any
`ExportFact` reachable from the file regardless of nesting. Four more read
`file.ast.exports` (or `&file.ast.exports`) directly, unfiltered:
`entry_export_entity` and `external_export_summary_for_file`
(`rust/crates/solid-facts-backend/src/main.rs`), `export_is_type_only` (same
file), and `resolve_named_export`
(`rust/crates/solid-reactive-ir/src/contracts.rs`). `file.ast.exports` is
sorted by *span*, not by nesting depth, so a nested specifier can sort before
the module-level one it shares a name with — `entry_export_entity` is the one
proven wrong: `namespace internal { export function helper(v: number) { return
v } } export const helper = internal.helper(41)` bound the module-level
`helper` (a `number`) to `internal.helper`'s type facts and published `kind:
"function"`, a `tsc`-clean false claim. All four now iterate
`AstFacts::module_level_exports` instead, the same fix already applied to the
other two — there is no sixth call site of this shape left in either crate.

**A class static block is not a function body either.** `exported_bindings`'s
function-body exclusion covers a function's parameters and locals, but `static
{ const inside = 1 }` inside an exported class is neither a function body nor
a module block, so its declarator passed the same way a namespace member's
used to. `export class Holder { static { const insideStaticBlock = 1 } }`
published `insideStaticBlock` as a module export with zero facts behind it —
whatever type it turned out to have was whatever the demand plan happened to
find at that span, not a claim this analysis proved. `exported_bindings` now
also excludes a declarator whose span is *strictly inside* a `ClassFact::span`
that is itself inside the export span. Strictly inside, not merely contained,
is the exact boundary: a class **expression** initializer —
`export const boxed = class { static { const hiddenB = 2 } }` — gives
`boxed`'s own declarator span (identifier through initializer) a span that
*contains* the class expression, the reverse relationship from `hiddenB`'s,
so `boxed` must survive the same exclusion that removes `hiddenB`. A
module-level declarator can never itself sit inside a class body, so the
containment test cannot misfire on one; both directions are pinned in the
fixture below.

`fixtures/package-contracts/namespace-export-surface` grew three rows for
this: `helper` (a module-level `number` whose name collides with
`internal`'s nested `helper` function — the name-collision shape), `Holder`
(an ordinary exported class whose static block must not cost it its surface),
and `boxed` (the class-expression-initializer control for the strict-containment
direction). `tsc --noEmit --strict` stays clean — class static blocks are
ordinary ES2022 syntax, and this fixture needed no `tsconfig.json` change to
type-check them, since nothing here depends on downlevel emission.

Three unrelated documentation corrections rode along with this pass, none of
which changed behavior:

- `AstFacts::declares_class_at`'s doc undercounted its own domain.
  `export default (class {})` is not a case the function stays out of: the
  parser does not preserve the parentheses, so the export records the class
  expression's own span, which is exactly the span `visit_class` recorded for
  it, and `declares_class_at` matches it the same way it matches an anonymous
  class *declaration*'s span. The match is redundant there — ordinary
  `Constructability` already answers a class expression correctly on its own —
  but the fixture and package-contracts docs that described this control as
  decided "by the same [language] definition" alone, as if the span-addressing
  rule played no part, are corrected in
  `fixtures/package-contracts/exported-class/README.md` and
  `class-expression-default.ts`.
- `scripts/contract-corpus.mjs`'s summary line read `contract corpus: N
  packages`, which reads as "N correct contracts". A passing count is only "N
  pins matched their checked-in expectation"; at the time,
  `function-supertype-kind` deliberately pinned a known-wrong claim (since
  closed by schema 15). The line now reads `contract corpus: N pins, …`.
- The two shape changes to `AstFacts` in the fix above — the `module_blocks`
  addition and the unrelated `initializer_class` deletion recorded under "Dead
  field removed" below — shared a single `AST_FACTS_SCHEMA` bump (35 → 36).
  They are now two honest bumps: the deletion keeps 35 → 36, and
  `module_blocks` moves 36 → 37.

A direct re-probe of both repro shapes against a fresh debug binary and
`bin/solid-typefacts` (`solid-checker contract generate` over two throwaway
one-file packages in `/tmp`, outside this repository) confirms the fix:
`namespace internal { export function helper(v: number) { return v } } export
const helper = internal.helper(41)` now publishes `helper` as `kind: "value"`,
and `export class Holder { static { const insideStaticBlock = 1 } }` beside
`export const boxed = class { static { const hiddenB = 2 } }` publishes
`Holder` and `boxed` both as `kind: "function"` with neither
`insideStaticBlock` nor `hiddenB` anywhere in the surface. See "Gates" below
for the full suite this pass ran.

### `export default class {}` published `kind: "value"`

**Closed.** The `kind` decision reads `class_declaration_name` before any type
fact, because at a class declaration's name the compiler answers with the
*instance* type. That guard matched class **name** spans only. An anonymous
default-exported class has no name, so `visit_export_default_declaration`
records the `class …` node's own span, the guard missed, and the facts answered
about the instance — `NonCallable` + `NonConstructable`, the one cell that
publishes `value`. That is a **false maximal certified negative for a
constructor**: `value` asserts the export reads nothing reactive, returns
nothing, invokes no caller-supplied callback and requires no owner, about
something a consumer will call with `new`.

Measured before the fix: `export default class {}` and `export default class
extends Base {}` published `value`; `export default class Named {}`,
`export default (class {})` and every `export { C }` form were already correct.

`AstFacts::declares_class_at` now answers for a class's binding name **or** the
class node itself, which keeps it a span-addressing rule rather than a class-ness
proof: a class declaration *is* the constructor by language definition, named or
not, exactly as for the named case. `fixtures/package-contracts/exported-class`
grew four entrypoints — `./anonymous-default`, `./anonymous-extends` (the shape
a published package actually contains), and `./named-default` and
`./class-expression-default` as controls — each publishing `default` as
`kind: "function"`. The "one span" wording that this bug hid behind is corrected
in `contracts.rs`, `docs/package-contracts.md`, that fixture's README, and the
constructability entry above; there are two such spans, not one.

### Dead field removed, one predicate hoisted

- `BindingFact::initializer_class` had no reader left: its consumers
  (`location_declares_class`, `binding_is_reassigned`) were deleted when
  `export_kind_proof` moved to the constructability fact. The field, its
  extraction, and its `BindingMetadata` slot are gone, and its unit test is
  replaced by one that pins what remains true — the span-addressing rule, for a
  declaration name, a named class expression's name, and an anonymous class
  node. Nothing outside the crate serialized it (no snapshot, golden, or process
  fixture carries `initializerClass`), but the field's removal is still a shape
  change to a versioned fact table, and it does not share a bump with the
  unrelated `module_blocks` addition above: it carries its own `AST_FACTS_SCHEMA`
  move, 35 → 36, ahead of that addition's 36 → 37. The doc comment's dangling
  reference to `binding_is_reassigned` went with it.
- The uncertifiable predicate `read.uncertain || read.missing_jsx_census ||
  read.divergent_lowering.is_some()` was duplicated verbatim in
  `solid-v2/rules/src/lib.rs` (hint selection) and `projection.rs`
  (`finding.kind`). It is now `ReactiveRead::is_uncertifiable`, called from
  both: the two could not disagree by construction, which is the property the
  hint fix relied on and stated in prose.

### Gates

`facts-lib` (54), `ir-lib` (152), backend lib (29, with the demand-plan
completeness assertion widened to `export { local }`, `export class K {}`,
`export default class {}` and `export const { a } = o`), `solid-v2-rules --lib`
(10), the armed `contracts_process` (53) and `dialects_process` (37) suites,
coverage against a fresh debug binary (**90 fixture projects, 564 findings, no
finding moved**), the contract corpus (39 packages, one added), and
`node --test scripts/*.test.mjs` (281). `cargo fmt --check`, workspace-wide
`cargo clippy -D warnings`, `git diff --check`, `jq empty` on the findings
schema and `dialect-manifests.mjs validate` all clean. The reproduction now
generates `["Config", "real"]`.

**Re-run for the review follow-up above**, against the same fresh debug binary
and `bin/solid-typefacts` (`facts-lib` 54, `ir-lib` 152, backend lib 29, the
armed `contracts_process` 53 and `dialects_process` 37 suites, coverage 90
fixture projects / 564 findings / no finding moved, the contract corpus 39
pins (its 33 uncovered generator ranges are pre-existing `generatorCoverage`
bookkeeping over `packages/cli`'s generator, untouched by this pass and not a
gate failure — `generatorCoverage` only throws on a claim emitter with *zero*
coverage, which did not happen), `cargo fmt --check`, `git diff --check`,
`jq empty`, `dialect-manifests.mjs validate`, and
workspace-wide `cargo clippy -D warnings` — all identical or clean). Not
re-run: `node --test scripts/*.test.mjs`, `make verify`, and the ecosystem
benchmark, for the same unmapped-path and registry-install reasons as the
first pass.

**Not run: `make verify`.** `verify-delta` escalates to it because this tree
carries unmapped paths from the constructability work it sits on top of
(`rust/Cargo.toml`, `THIRD_PARTY_NOTICES.md`, `scripts/`), not because a check
above was skipped. The ecosystem benchmark and contract conformance (which
install from the registry) were not run either.

**Remaining approximation.** The analysis-side surface
(`contract_export_fragment`, the project-wide export map) takes the same
`module_level_exports` filter as the generator, but no fixture exercises a
*namespace* export through it — the pin is the generator's corpus row. A
reactive-ir fixture with a local package that exports a namespace would close
that.

## 2026-08-25 — merged compiler pins remeasured and obsolete divergence arms removed

Pinned `dom-expressions#next` at `ead46d12da34db2ae366e1c02183a87f7479f05c`
and `solid-1x-compiler` at
`98d265c38dbf63e363c9846048a93461e66f44c7`, then reran the compiler-backed
fixtures before changing expectations.

That measurement supersedes the earlier standard-void and `<noscript>`
mitigations. Neither is a current positive compiler disagreement. Surviving
source expressions without an execution site remain uncertifiable through
`missing_jsx_census`; operations in the same holes now fail closed for
ownership as well, preventing a deleted-or-uncensused cleanup from being
reported as a proven SC4001 violation. The tag-specific `<noscript>` arm and
the shared 14-tag void list were removed from `divergent_lowered_child`.

One transform divergence remains: Solid 1.x Babel calls `<keygen>` and
`<menuitem>` void while the 1.x Rust producer lowers their children. That list
stays in the Solid 1 dialect and continues to produce compiler-disagreement
wording. Solid 2 deliberately has no corresponding list and follows Ryan's
`next` transform semantics; in particular, the dynamic-`textContent` child
case is an ordinary census gap rather than a reason to force Babel output
parity into the fork.

The full handoff gate passed (`make verify`, 357.59 s). Its two optional
registry-install integration tests skipped when npm DNS was unavailable; the
five bundled registry pins were verified from the input-bound memo and every
non-network gate passed. The fresh full ecosystem run covered 416 probes:
390 complete contracts, 9 partial contracts and 17 failures. Solid 1.x moved
from 143/168 complete (85.12%) to 146/168 (86.9%), with 9 partial and 13 failed;
Solid 2 remained 244/248 complete (98.39%), with no partial and 4 failed. The
canonical JSON and Markdown reports were regenerated under
`benchmarks/ecosystem/`.

## 2026-08-25 — discarded syntax and shadowed JSX reconciliation closed

The remaining producer refusal from the discarded-region audit is closed at
`dom-expressions#next` `c7e83a1bb0fc8e8f7fad37a7523db9fcce568820`
and `solid-1x-compiler`
`a4566086a457a4f2ec2964350fd86f3ad5139ee7`. For
`<span children={<b>{hidden()}</b>}>{visible()}</span>`, each producer now
records the surviving outer attribute value as `Elided` before retracting the
nested JSX sites. The outer positive deletion fact therefore survives even at
a template root, the deleted `hidden()` read is absent, and the live
`visible()` child remains tracked. This closes the former file-level exit-2
failure without changing emitted JavaScript or semantic-trace version 2.

The producer handoff suites passed against the exact revisions: `next` 59
tests passed with one ignored baseline-regeneration test; Solid 1.x 99 passed
with one ignored. Both transform-output baselines remained byte-identical. The
Solid 1.x parity corpus also passed all 4,555 probes, covering the previously
documented divergences 6–9. Ryan's `next` retains its own documented Babel
differences by design; Solid 2 consumes that output truthfully rather than
turning the fork into a Babel-parity compiler.

The syntax-only policy left undecided in the earlier discarded-region entry is
also closed. After all IR producers run, one common pipeline gate removes any
structured static defect, upstream-compatible static violation, directive
creation, or contract-generation obligation whose primary location is inside
a producer-proven discarded region. This is a deletion fact, not ordinary
source dead-code inference. The fixture pairs a one-argument `createEffect`
inside deleted `<noscript children={...}>` with the identical live API defect:
the deleted call is absent and the live control remains SC7001. Reactive-read,
destructure, leaf-owner and ordinary owner controls continue to pin the other
projection funnels.

Fresh checker coverage against these exact heads completed all 91 fixture
projects with 564 findings. The only snapshot changes are the new live SC7001
control and source-offset shifts in the three edited fixtures; neither
shadowed JSX inner read produces a finding or a producer refusal.

The full 416-row ecosystem verification corpus was then rerun with stable
copies of the exact release checker and Type Facts binaries. Its semantic
result is unchanged: 281 rows verified, 116 refused, 14 generation failures,
3 install failures, and 2 rows with no probeable Solid runtime. Solid 1.x is
111/168 verified with 44 refused; Solid 2 is 170/248 verified with 72 refused.
All claim totals, failure shapes, blocker counts, and conversion counts match
the preceding report exactly. The regenerated reports record the new checker
hash and timing sample; one peer-complete install moved to peer-install failure,
an environment/install outcome that changed neither contract nor verification
classification.

One positive Solid 1.x transform disagreement still has a checker mitigation:
the Rust producer lowers children of `<keygen>` and `<menuitem>`, while
`babel-plugin-jsx-dom-expressions@0.40.10` treats those tags as void. Until that
producer output is aligned, the dialect's two-tag parity-target-only set stays
uncertifiable. This is the remaining transform item before calling the Solid
1.x Babel-faithfulness work complete; it is separate from the now-closed
discarded-region and divergences 6–9 work above.

## 2026-08-25 — final legacy void divergence closed; Phase B complete

This entry supersedes the remaining item above. Solid 1.x is pinned at
`ca3bbfae7d1e00e28ef73f9af58bdb46e248b512`: its compiler now treats `keygen`
and `menuitem` as void, matching `babel-plugin-jsx-dom-expressions@0.40.10`, and
records every discarded void or `<noscript>` child list as one positive
`Elided` range. The producer suite passed 100 tests with one intentional
baseline-regeneration test ignored, and all 4,665 Babel differential probes
passed.

Solid 2 is pinned at `26e744fb4feb973a3652bfc45a8c3938ece667f0`.
That change is semantic-trace-only: template-root void child lists now carry a
positive `Elided` range, while nested native void children remain live under
Ryan's authoritative `next` semantics. Generated JavaScript and the transform
baseline are unchanged; the producer's 59 Rust tests passed with one intentional
regeneration test ignored, and all 6,148 Jest assertions passed.

With those facts available, the checker deletes the obsolete consumer
mitigation rather than leaving it dormant: `DivergentLowering`, the dialect
parity-target-only void-list hook, rerun and ownership suppression, projection
wording, and autofix suppression are gone. Fresh compiler-backed coverage moved
only the four dedicated census/void snapshots, from 564 to 554 total findings:
ten obsolete uncertifiable findings disappeared, with no new violation and no
unrelated fixture movement. The paired fixtures retain byte-identical source
and certify each dialect's intentional result.

This completes the planned Phase B transform work. It does not make the compiler
fact boundary total over all source JSX: the Solid 1.x nested non-hydratable
`<head>` path and retraction shapes such as the inert `<noscript>` fast path can
still reach `missing_jsx_census` and remain explicitly uncertifiable. Those are
fact-coverage gaps, not known transform disagreements. The ecosystem corpus was
not rerun for this trace-only closure; the preceding 416-row measurement remains
the latest ecosystem result until a new run is recorded.

## 2026-08-25 — generic relational probes, entrypoint isolation, and finite dynamic imports

Three schema-v1 relational return claims now have honest generic probes:
`argument[N]`, `callback-result[N]`, and `callback-result-function[N]`. Each
claim identity includes its parameter index. The worker plants a fresh frozen
object and requires strict reference equality; the returned-function form also
requires a callable result and invokes it with no arguments. A completed
mismatch is a failure. A throw, including a returned function that needs
arguments the schema cannot describe, remains undriven.

The probes exposed false generator claims rather than evidence to relax. A
relational return is now emitted only when every return fact proves the same
relation, with no conditional/control-test path. Returned expressions are
peeled through transparent TypeScript wrappers but must then be an exact
identifier reference before compiler symbol identity can connect them to a
parameter. Predicates, conditional identities, and guarded fallthroughs remain
without a relational contract; the direct generic identity control remains
`argument[0]`.

The remaining `@solidjs/web` failures exposed a second ownership bug in the
Node generator. Two public entrypoints sharing one runtime target were treated
as semantic aliases, so the root entrypoint's server-only `ssrGroup` identity
variant was merged into `./jsx-runtime` and `./jsx-dev-runtime`, which always
select the void web/dev bodies. Cross-entrypoint merging is removed. Target
analysis is still cached by exact target, excluded siblings, and conditions,
but each public entrypoint retains only its own projection. The focused
`entrypoint-condition-isolation` fixture pins the same-name identity/void pair.

Finally, the closure seeder can close two finite dynamic-import forms without a
package-specific oracle: nested conditional expressions whose leaves are all
string literals, and inline (optionally `Object.freeze`d) literal tables indexed
by a finite literal selector. Both enumerate every target into the seed and the
attested module record. An identifier, template substitution, arbitrary lookup
key, or one open conditional branch keeps the existing fail-closed runtime
note. The note now says the specifier is not statically bounded to a finite
literal set, which is the actual missing proof.

### Full-corpus measurement

The final fresh run used stable checker SHA-256
`da63bebcad37215615392ca8f7ae03cefccc70ee2d9fb185470d62d3c85648e5`
and Type Facts SHA-256
`31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`
over all 416 probe rows. Raw counts moved as follows:

| Figure | Before | After |
| --- | ---: | ---: |
| Claims total | 12,470 | **11,941** |
| Driven / passed / failed | 7,827 / 7,815 / 12 | **7,959 / 7,951 / 8** |
| Undriven | 4,643 | **3,982** |
| Parameter identity without a probe | 400 | **0** |
| Conversions (return conversions) | 829 (450) | **699 (320)** |
| Probed rows kept / rows with evidence | 3 / 3 | **69 / 18** |

The raw outcome table is 281 verified, 115 refused, 15 generation failures, 3
install failures, and 2 rows with no runtime. Two no-contention reruns establish
the environment qualification: `@tanstack/ai-devtools-core` verified after its
full-run generation exceeded 120 seconds, and `@tanstack/solid-table` verified
after its full-run client children exceeded the 20-second per-mode timeout.
Replacing only those timing outcomes yields the semantic comparison: **283
verified, 114 refused, and 14 generation failures**, versus 282/115/14 before.

Finite dynamic imports change no ecosystem outcome or note count. The five rows
carrying all 17 such notes still use specifiers outside the accepted finite
syntax. This deliberately does not claim that `@solidjs/web` is unblocked: its
runtime `entryUrl` remains open.

### Remaining fail-closed surface

Generic probes still do not name nested return leaves, store paths, callback
arguments, reactive reads, owner requirements, or most async behavior. The raw
final report records 254, 23, 13, 1,107, 465, and 92 undriven claims in those
classes respectively. It also records 585 entrypoint-import throws, 379
synthesized throws, and 386 session aborts caused by package code. Those remain
uncertifiable; none was converted into negative evidence by this slice.

### Export-scoped open loads and entrypoint worker isolation

The former `@solidjs/web` qualification is now closed without pretending its
runtime `entryUrl` is finite. For flat entry modules, an open `import()` is
attributed to its exact containing named function and propagated through exact
local function references to explicit export bindings. Affected exports are
omitted; a top-level load, affected-function escape, duplicate/missing binding,
or cross-module attribution stays entrypoint-wide. The focused
`open-dynamic-import-attribution` fixture omits two transitively affected
loaders while retaining an independently proved `identity` summary.

On published `@solidjs/web@2.0.0-rc.1`, the only reachable public export is
`hydrate` in the web and development targets. The root, `./jsx-runtime`, and
`./jsx-dev-runtime` contracts omit that export and retain `ssrGroup`. Targeted
floor/head benchmark rows both moved from refused to verified. This is a
withdrawal, not invented evidence: callers of the omitted `hydrate` remain
uncertifiable.

Probe workers are also isolated by exact entrypoint specifier. The
`@solidjs/start@2.0.3` control remains refused with the same 5 passed and 90
undriven claims, but its four modes now finish all specifier batches instead of
ending with an entrypoint's partial module state: 60 worker starts, 12 restarts,
all four modes complete, versus 52 starts, 48 restarts, and four incomplete
modes before. Its `.jsx`, `solid-start:`, and missing `server-only` imports are
real published-runtime failures and stay undriven.

The fresh 416-row authority run moves from 281 verified / 115 refused / 15
generation failures to **286 / 110 / 15**. Claims move from 11,941 total,
7,959 driven, 7,951 passed, and 3,982 undriven to **11,935 total, 8,333 driven,
8,325 passed, and 3,602 undriven**. The eight failed claims are unchanged. One
raw generation timeout verified in a no-contention rerun, so the
environment-controlled interpretation is 287 verified / 109 refused / 14
generation failures; the checked-in report preserves the unmodified raw run.
The remaining broad static gaps are 254 nested return leaves, 23 store paths,
13 callback arguments, 1,107 reactive reads, 465 owner requirements, and 92
async claims. Published-runtime behavior still leaves 601 import throws and 41
session-abort withdrawals; those are not converted into negative evidence.

No callback-argument or nested-return probe was added merely on the strength of
`typeof`. The generated callback descriptors are reactive accessors, but schema
v1 names neither a source nor a mutation that a generic probe could drive;
common nested accessor/store leaves have the same missing observation path.
Those rows stay family C until the schema fully describes construction and
mutation. Relational top-level returns remain the only new generic forms in
this slice because strict sentinel identity proves exactly what they claim.

## 2026-08-26 — contradiction-free generation, recursive return leaves, and proven constructors

Eight runtime contradictions in the preceding authority report are closed at
their semantic owners. A call initializer no longer inherits its callee's
function summary when exact export Type Facts prove the exported value is not
callable (`@kobalte/core`'s namespace helper). Solid 1 `createResource` fetchers
are described by their observable initial scheduling rather than the dialect's
deferred attribution. Callback timing no longer crosses a nested helper from a
lexical role alone, and returned conditional adapters or after-call tracked
wrappers become `callbacks: unknown` when schema v1 cannot state their mixed
execution. The focused controls retain exact direct-inline and returned-
scheduler behavior; withdrawals are per export, not package-wide suppression.

Return tuples and objects are now traversed recursively. Claim identities carry
the exact property/element path, and relational leaves also retain their exact
parameter index. The worker selects that leaf before applying the same strict
accessor or reference-identity observation, evidence is written on that leaf,
and stale evidence for a sibling cannot corroborate it. Because schema v1 has
one sentinel for the whole `returns` domain, one unconfirmed leaf still converts
the domain. Store paths remain undrivable, and an accessor leaf without a
contract-named callback in which to plant a reactive read is recorded as `no
plantable reactive source`; neither is accepted on `typeof` alone.

Generation also emits a sibling `.probe-plan.json` bound to the exact contract
hash and package release. Exact Type Facts may supply only proven inhabitants:
`null`, `undefined`, an empty array, `Map`, or `Set`. The driver applies them
only to otherwise-undefined argument slots. Ambiguous public symbols,
conditional runtime targets, literal subtypes, and other open domains get no
recipe. The sidecar helps a call reach behavior but is never evidence; probing
refuses a stale hash.

The fresh 416-row authority run used checker SHA-256
`c1b606862f4ea98ac719c0d53c1db51d19a0f62e60ffb9f2899bb8b85d0f6cf8`
and Type Facts SHA-256
`31d6cc0daeb91d22d5ca16cfa8d28d4bb62157ccdf73b87cd4fddc533e37d889`.
Against the preceding checked-in raw report:

| Figure | Before | After |
| --- | ---: | ---: |
| Verified / refused | 284 / 109 | **297 / 97** |
| Generate / install / no-runtime | 15 / 6 / 2 | **17 / 3 / 2** |
| Claims total | 11,969 | **11,744** |
| Driven / passed / failed | 8,327 / 8,319 / 8 | **7,734 / 7,734 / 0** |
| Undriven / incompleteness | 3,642 / 589 | **4,010 / 518** |
| Conversions (callback / return / async) | 819 (424 / 365 / 30) | **802 (372 / 399 / 31)** |
| Probed rows kept / rows with evidence | 83 / 20 | **97 / 21** |

The lower driven count is not a coverage claim: false callback claims were
withdrawn, and recursively named accessor leaves that lack a plantable source
are now counted precisely instead of hidden under one `nested return leaf`
bucket. That bucket moved from 270 to zero while `no plantable reactive source`
moved from 235 to 859 and store-path leaves from 23 to 51. The result has no
failed claim and no contradicted kind claim. Remaining gaps include 1,079
reactive-read proofs with no runtime probe form, 463 owner requirements, 98
async claims, 51 store paths, 9 callback-argument descriptors, 601 import
throws, 368 synthesized throws, and 286 calls that did not reach the callback.

## 2026-08-26 — exact parameter-member reactive-read probes

`parameter-member` rows now retain the exact static property when every path
contributing to the row invokes the same member. The optional field is
backward-compatible: an older parameter-only row, a computed property, or
several distinct members remains valid and keeps its static family-(A) proof,
but gets no runtime probe.

For a path-qualified row the generic worker supplies an object whose named
method reads a fresh signal, invokes the export inside a memo, writes the
signal, and requires both the export and method to run again. A call that never
reaches the method is undriven rather than negative evidence. If the method ran
and the write produced no re-read, the observation contradicts the contract and
blocks verification. Passing modes can attach `probed` evidence to the exact
row; the compiler proof remains sufficient when a package-specific argument
shape prevents the probe from exercising it.

The focused `parameter-member-read` and `parameter-member-forwarded` fixtures
pin `slice` and `getThing` respectively. The fresh uninterrupted 416-row run
used checker SHA-256
`4fe1381a3f30f4f44efe8904b2a0adb5a8ac704e12840c28030b2c4fe67cf31b`
and Type Facts SHA-256
`983d0b702ace1476ecd7f5633e9e25b33003287b5319404851cdc5141d0d1844`.
It also corrects the corpus scope for multi-framework packages.
`@tanstack/charts` generates only `./solid`; `@tanstack/devtools-utils`
generates `./solid` and `./solid/class`; and `@tanstack/devtools-a11y`
generates its framework-neutral `./core` pair plus its `./solid` pair. Their
React, Preact, Svelte, Vue, Angular, Lit, and Octane adapters are no longer
presented as evidence about Solid contracts. Discovery owns these allowlists,
so a manifest refresh reproduces the scope automatically.

Against the preceding checked-in report:

| Figure | Before | After |
| --- | ---: | ---: |
| Verified / refused | 297 / 97 | **295 / 100** |
| Generate / install / no-runtime | 17 / 3 / 2 | **17 / 3 / 1** |
| Claims total | 11,744 | **10,274** |
| Driven / passed / failed | 7,734 / 7,734 / 0 | **6,719 / 6,719 / 0** |
| Undriven / incompleteness | 4,010 / 518 | **3,555 / 503** |
| Reactive reads with no probe form | 1,079 | **537** |
| Conversions (callback / return / async) | 802 (372 / 399 / 31) | **745 (346 / 377 / 22)** |
| Probed rows kept / rows with evidence | 97 / 21 | **90 / 20** |

The outcome and total-claim deltas are not attributable solely to the new
probe: registry/install outcomes moved between runs, and the three corrected
TanStack scopes deliberately remove 118 non-Solid entrypoints. The direct
coverage result is that the old broad `reactiveReads` no-probe bucket shrank by
542 claims; 45 exact-member attempts are now named separately because the
completed call did not reach the member. The remaining exact attempts either
pass or retain another explicit fail-closed reason. No claim failed, and the
kind-contradiction count is zero.

The two additional umbrella scopes remove nine foreign entrypoints and 19
claims from the preceding Solid-only-charts measurement. Their combined rows
move from 30 planned / 15 passed / 15 undriven claims to 11 planned and all 11
passed: `@tanstack/devtools-a11y` now emits four entrypoints and 6 claims;
`@tanstack/devtools-utils` emits two entrypoints and 5 claims. Fourteen foreign
adapter import-throw claims and one foreign member-not-invoked claim disappear;
the other four removed claims had passed but described foreign adapters.
Svelte, Vue, Preact, and Angular import failures fall to zero. The remaining
239 missing-React observations belong to Solid-facing entrypoints or ports and
are not treated as foreign adapters.

The larger 159-claim corpus delta also includes `solid-recharts@1.0.1` moving
from a 140-claim refused probe to a generation timeout in this raw run. That is
an environment-sensitive outcome, not a gain attributed to entrypoint scoping;
the checked-in report preserves the uninterrupted authority run as observed.

The current broad gaps are 537 unqualified reactive reads, 447 owner
requirements, 71 async claims, 47 store paths, and 9 callback-argument
descriptors. Published runtime behavior still leaves 588 entrypoint import
throws, 504 synthesized-call throws, 282 calls that did not invoke a callback,
and 45 calls that did not invoke the named parameter member. All remain
undriven rather than being accepted as negative evidence.

## 2026-08-26 — wide-package contract timing and isolated restart pools

The focused `solid-recharts@1.0.1|solid1|only` harness reconstructs the exact
audited artifact from Bun's local content cache, including its non-Solid D3
dependencies, and performs no registry install. It requires 109 expanded
exports, 140 claims, distinct browser/server closures (246/239 JavaScript
files in the current artifact), four modes, bounded initial chains plus many
restarts, no failed claim, and the existing refused verification with
server `kind` blockers for `Dot`, `LabelList`, and `Pie`.

Structured native timings identified probe-plan construction as the avoidable
generation hotspot: it repeatedly scanned every function and the full Type
Facts entity table for every public export. One exact-location entity index and
one canonical-symbol function index reduce that step from about 25.5 seconds
to about 0.3 seconds on the captured package. These indexes live only for the
immutable facts of one target analysis; no browser fact, closure, or condition
identity is reused as a server fact.

The probe experiment also rejected one-worker-per-observation: it increased
the row from 177 process sessions to 762 and made probing slower. The retained
model groups non-invoking `kind` reads once per exact specifier and runs
call-capable observations in a bounded pool of restart chains. A chain is discarded
after a synchronous throw, asynchronous abort, timeout, or unreadable result.
Stable output order is reconstructed by probe ID. A standalone row defaults to
eight call-capable lanes; the ecosystem runner divides host parallelism across
its concurrent rows (two lanes for this row on the 14-core authority host). With
one non-invoking import chain per mode, the full run used
182 processes across 12 chains (170 actual restarts) for this row while
preserving 124 passed and 16 explicitly undriven claims; no failed claim became
passed.

Three focused after samples on the same release binaries were 18,304 ms,
18,352 ms, and 18,652 ms (median 18,352 ms): generation median 12,720 ms,
probe median 5,581 ms, and verify median 41 ms. The exact full-corpus row was
19,217 ms (221 install, 10,551 generation, 8,423 probe, 20 verify), down from
the captured 84,262 ms authority row while retaining its 109 exports, 140
claims, and refusal attribution.

The full 416-probe corpus took 154,534 ms at row concurrency six, compared with
595,629 ms for the preceding checked-in report: 3.85x less wall time. Median
row time fell from 1,265 to 546 ms; generation p90 from 1,612 to 482 ms; and
probe p90 from 3,879 to 1,211 ms. The fresh report remains under `/tmp` rather
than replacing the checked-in semantic authority because concurrent schema and
entrypoint-scope work intentionally changed its outcome counts. It recorded
10,462 claims, 6,898 passed, no failed claim, and 3,564 explicitly undriven.

The follow-up slice canonicalizes a target analysis by the exact effective
native inputs rather than the export-map label that produced them. For
`solid-recharts`, `import` and fallback both select the browser artifact with
effective runtime conditions `{import}`, so the fourth native analysis was a
duplicate; the `{browser, import}` and `{node, import}` analyses remain
distinct. Three focused samples after removing it and sharing one bounded
probe queue across all four modes were 7,483, 7,503, and 7,502 ms (median
7,502 ms): about 5.03s generation, 2.46s probe, and 19ms verification. Claim
accounting and the `Dot`/`LabelList`/`Pie` refusal were unchanged.

Corpus scheduling now defaults to three outer rows on hosts with at least three
available CPUs. A deterministic 42-row slice improved from 50.08s at the old
six-row/two-lane allocation to 33.03s at three rows/four lanes with identical
verdict counts. The final two-tail check recorded 33.82s for
`@kobalte/core@0.13.13` and 35.33s for `@tanstack/solid-table@9.1.2`, down from
50.72s and 58.46s.

The final complete 416-probe authority took 153,305ms, compared with 154,534ms
for the preceding optimized six-row run and 595,629ms for the original
authority. Median / p90 / maximum row times moved from 546 / 2,274 / 61,296ms
to 332 / 1,540 / 37,592ms. Outcome counts were identical: 308 verified, 88
refused, 16 generation failures, 3 install failures, and 1 no-runtime. It
recorded 10,462 claims, 6,900 passed, zero failed, and 3,562 undriven. The two
additional passed observations are one claim in each Solid Focus 2.0 row that
an independent worker answered before another chain's asynchronous abort; the
aborted chain's unanswered claim remains undriven.

The retained follow-up moves all exact target analyses into one package-wide
four-lane pool. The analysis identity remains the runtime target, excluded
sibling targets, and effective native conditions; browser/server projects and
closure attestations are never equated. On the same debug binary,
`@kobalte/core@0.13.13` generation fell from 18.145s to 9.439s and emitted a
byte-identical contract. Its full authority row is now 18.192s, while
`@tanstack/solid-table@9.1.2` is the maximum row at 27.976s.

Probe orchestration now allocates the minimum restart chains needed to saturate
the row pool across exact mode/specifier groups, prefixes the first risky chain
with its non-invoking kind reads, sends requests over stdin, and records restart
causes plus worker/process subphase timings. It never reuses a process after a
throw or abort. Table retains 567 claims, 431 passed, zero failed, and 136
undriven with 2,342 sessions / 16 chains / 2,326 restarts; all restarts are
synchronous throws. Kobalte v1 retains 878 claims, 612 passed, zero failed, 266
undriven, and 21 incompleteness findings, while probe wall falls from 27.132s to
12.158s in the full run.

Three interleaved focused `solid-recharts@1.0.1` samples were 7,393, 7,375, and
7,448ms (median 7,393ms): generation 4,970 / 4,938 / 5,009ms, probe 2,403 /
2,417 / 2,419ms, and verify 20ms each. All retained 109 exports, 140 claims,
124 passed, zero failed, 16 undriven, 178 sessions / 8 chains / 170 restarts,
and the same `Dot`, `LabelList`, and `Pie` server-kind refusal. This is 11.4x
faster than the captured 84,262ms authority row.

The fresh full 416-row authority at standard outer concurrency three completed
in 128.178s wall versus 153.305s previously. Summed row work is 360.248s;
median / p90 / maximum row time is 255 / 1,340 / 27,976ms, with no row above
30 seconds. Outcomes remain 308 verified, 88 refused, 16 generation failures,
3 install failures, and 1 no-runtime. It records 10,462 claims, 6,897 passed,
zero failed, and 3,565 undriven. The three fewer passes are conservative
asynchronous-abort attribution in the two Solid Focus 2.0 rows and Solid
Promise 1.x; no failed or undriven claim became passed.

The required-object fact was added on 2026-08-26 (`solid-ts-facts` `cadb247b`,
ADR 0022, wire table schema 17) and is now consumed by declaration-only probe
planning. For `@tanstack/solid-table@9.1.2`, it proves
`{ columns: [], data: [], features: {} }` for `createTable`; the completed call
is checked before the recipe is emitted. The focused row keeps 567 claims,
zero failures and zero incompleteness findings, with the same 431 passed and
136 undriven. It reduces sessions only from 2,342/2,326 restarts to
2,338/2,322 and does not materially move the roughly 21.5s probe wall. The
final focused run was 4.15s generation and 21.50s probe wall; declaration
construction accounted for about 1.11s of
the generation phase across the root and recursively generated dependency
targets. Value-only and kind-only targets skip that query.

Conditional entrypoints no longer discard all construction facts merely for
having more than one target. Each target keeps its own Type Facts identity and
the published probe plan receives only the exact structural intersection of
their recipes. A missing or different browser/server recipe is therefore a
miss, while the `solid` and `import` Table branches retain `createTable` only
because both independently proved the same object.

A factory experiment supplied the root `createTable` result to exact
`./static-functions` parameters through a conditional-type assignability gate.
It reduced sessions to 1,993 and restarts to 1,977, but Table construction cost
offset the saved processes: aggregate process wall moved only 111.8s to 111.2s,
worker time increased 84.4s to 87.3s, and generation added about 1.1s. That
extension was not retained; even the same-entrypoint form admitted an
unconstrained generic in the armed process fixture and was removed rather than
weaken that boundary. The remaining tail is dominated by probes that
must plant a callback/member in the very Table/Row/Column slot, plus Row,
Column, Cell and stateful-table witnesses that the new leaf-only fact correctly
leaves unknown. Guessing those graphs or reusing a mutable Table instance across
observations remains unsound.

The subsequent retained-session experiment was also rejected. Reserving an
empty root in the runtime project and updating it after contract and inventory
emission produced the same sound Table recipes, but the update invalidated too
much of wide TypeScript programs under corpus concurrency: `@solidjs/meta` and
`@kobalte/core` reached the 120s generation ceiling. The retained design keeps
the strict declaration query in its own one-shot project. A release-only bug
found during that experiment is fixed independently: declaration-plan requests
are now ineligible for the diagnostics daemon, and the generator explicitly
sets `SOLID_CHECKER_DAEMON=0`, so success without the requested sidecar is no
longer possible.

The corrected full 416-row authority completed in 126.658s at standard outer
concurrency three. Outcomes and claim accounting are unchanged from the
128.178s authority: 308 verified, 88 refused, 16 generation failures, 3 install
failures, 1 no-runtime; 10,462 claims, 6,897 passed, zero failed, 3,565
undriven. Median / p90 / maximum row time is 275 / 1,374 / 29,362ms, with no
row above 30 seconds. The 1.52s corpus-wall movement is not attributed as a
generator win: summed generation rose from 86.922s to 105.206s while summed
install time fell from 52.039s to 20.430s, so host/cache variation dominates.
The exact `solid-recharts@1.0.1|solid1|only` row remained refused with 140
claims (124 passed, 16 undriven) and took 13.245s: 46ms install, 6.639s
generation, 3.523s probe, 22ms verify. Table verified in 29.362s with its exact
567 / 431 / 0 / 136 claim accounting and 2,338 sessions / 2,322 restarts.

## 2026-08-26 — Solid 2 RC.3 is one audited runtime tuple

The Solid 2 audit ceiling is now exact `2.0.0-rc.3`. Official runtime rows no
longer combine their RC.3 artifact with an independently selected RC.0 floor:
`solid-js`, `@solidjs/web`, and `@solidjs/signals` each receive one probe with
all three runtime packages pinned to RC.3. Selection fails closed when any tuple
member is absent from the audited catalog. This is deliberately narrower than
ecosystem compatibility selection, which continues to measure package-declared
floor/head environments.

The practical closure is that `@solidjs/signals` now has an honestly selected
`solid-js` above it and verifies instead of ending as `no-runtime`; all three
official runtime rows install and reach verification. `@solidjs/signals` and
`@solidjs/web` verify. `solid-js` remains refused on 15 incompleteness findings
after 91 of 123 claims were driven and passed, so the tuple fix removes an
environment-construction blocker without misreporting semantic completion.

Live discovery also added `@solidjs/diagnostics` and
`@solid-primitives/animation`, moving the corpus from 305 rows / 416 probes to
307 rows / 418 probes. The fresh full authority records 309 verified, 90
refused, 18 generation failures, no install failures, and one no-runtime. All
7,122 driven claims passed with zero failed; 3,692 remain undriven and 503
incompleteness findings remain. Official Solid is 14/21 verified; Solid
Primitives is 236/291 verified, with 44 refusals and 11 generation failures.
Those are the next concrete closure sets; RC.3 tuple construction is no longer
one of them.

## 2026-08-27 — Solid 2 compiler facts moved to the Solid compiler

The Solid 2 compiler-facts producer moved from
`yumemi-thomas/dom-expressions@26e744fb` to the semantic-only Solid fork at
`yumemi-thomas/solid@1d81e67fd393d12c74b13aa7d3fb492f3d85353b`, based exactly
on `solidjs/solid#next@a10cf1a1`. The fork carries trace code and facts tests
only and will not be proposed upstream. Its 358-entry transform baseline is
byte-identical to an independently generated baseline from the exact upstream
base; trace on/off is also identical for JavaScript, source maps, and
diagnostics across that corpus.

The migration removes one stale uncertifiable SC1001 from
`jsx-census-gap-solid-2`: current Solid positively lowers `body()` beside a
dynamic `textContent` attribute as a tracked child insert, whereas the former
producer omitted that site. The SC8003 children/`textContent` authoring
violation remains. The other 557 baseline findings are unchanged. No
compatibility shim was added because throwing away the new positive compiler
fact would reduce precision and contradict the pinned compiler's actual
lowering.

## 2026-08-27 — Compiler-facts protocol 2 keeps execution knowledge local

Solid 2 now emits semantic trace version 3 from the semantic-only compiler fork
at distribution revision `9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4`. The trace
separates disposition, trigger, schedule, tracking, cardinality, and owner;
relates source sites to exact generated operations; reconciles DOM and SSR
lowering; and binds source, full effective configuration, output, optional
source map, official upstream base, and semantic implementation identity.

The checker consumes it through compiler-facts protocol 2 and one deep
normalizer. Legacy region/role arrays are derived compatibility projections and
cannot disagree with semantic operations. Cache identity includes the producer
and protocol, and Reactive IR reuse compares normalized operations,
completeness, and producer semantic revision. This closes
the former risk that a new trace or compiler pin could reuse old execution
answers merely because source and dialect were unchanged.

Both adapters keep their generated-operation domains open: Solid 2 reports
exact positive generated identities but has no independent emission census,
while Solid 1 lacks that identity model entirely. Solid 1's full
producer/configuration identity also remains open. Solid 2 universal/dynamic
modes remain refused. Server-function directive facts now identify
compiler-created references and registrations, while transport/runtime behavior
and receipt integration remain open. None of these open domains is interpreted
as complete-negative knowledge.

## 2026-08-27 — Normalized package semantics keep uncertainty at the exact leaf

The Phase 5 package-contract model now lives in the deep
`solid-reactive-ir::contract_semantics` module. It separates the four local
knowledge states, recursive value leaves, operation causality, resources and
lifetimes, possible versus guaranteed cardinality, owner requirements/source/
production/capabilities, restricted guard partitions, exact artifact/export
identity, and local experimental status. Normalization canonicalizes unordered
semantic collections and number guards, validates cross-references and
contradictions, and hashes typed length-delimited meaning under semantic model
version 1. Wire summary names, aliases, `closed` arrays, omission rules, and
schema versions are not part of this model or digest.

All sixteen Solid 2 conformance rows have normalized representations. This
does not close their behavior. Runtime callback behavior, async emission and
cancellation, cleanup replacement/disposal, transition and optimistic
lifecycle, request/response commitment, root event disposal, renderer/runtime
transport, exact artifact selection, and mixed-framework closure still require
the later static-proof, probe, resolution, and receipt phases. Solid 2 generated
operation positives also retain Phase 4's open emission census; universal and
dynamic compiler modes remain refused. Experimental server-component protocol
leaves stay open even while their exact export and artifact can be known.

The backend wire decoder and all analyzer consumers remain unchanged and
fail-closed. A normalized proposal cannot create an `AcceptedContract`; the
later proof-and-receipt authority must authorize every proposed closed domain.
No Type Facts, compiler facts, public schema, generator, evidence sidecar,
receipt, bundled contract, or snapshot changed in this phase.

## 2026-08-27 — Temporary contract v2 normalizes without accepting closure

The Phase 6 decoder now terminates temporary wire mechanics inside
`solid-facts-backend::contract_document_v2`. It validates the required format,
exact package/artifact identities, direct summary references, local closure,
recursive values, operation/resource/owner/cardinality axes, finite guards,
experimental status, sidecar hashes, and bounded expansion before handing a
wire-independent proposal to the Phase 5 semantic normalizer. Three goldens
cover all four local knowledge states, and adversarial tests reject false
closure, contradictions, invalid graphs, path traversal, excluded trust/evidence
fields, unstable normalization, and resource-limit excess.

This closes the old `NormalizationUnavailable` boundary but does not make a
proposal trusted: the public loader ends at `AcceptanceUnavailable`, and
`AcceptedContract` still has no public constructor. Exact artifact selection,
independent runtime/declaration target binding, resolver closure, proof/probe
content, receipt acceptance, generator cutover, bundled-contract migration, and
analyzer consumption remain open in their scheduled phases. No open runtime or
compiler premise from the Phase 5 Solid 2 conformance matrix was converted into
negative proof.

## 2026-08-27 — Artifact resolution binds exact closure or refuses the case

Phase 7 adds one exact artifact-resolution boundary without moving legacy
analyzer consumers forward. Ordinary Type Facts and WASM-host import
attestations now preserve the compiler-included path, pre-realpath symlink
spelling, resolver extension, owning package version, and resolver package
version instead of discarding them. The replacement contract loader compares
the resolved package, manifest, runtime, declarations, closure, transform,
entrypoint, and ordered runtime/types branches against normalized artifact
cases and requires exactly one match. Runtime and declaration reexports bind to
their independently resolved module/export identities.

Closure identity is canonical across input order but changes with a file role,
package-relative path, bytes, accepted dependency-contract edge, generated
output, transform, or opaque frontier. Nonliteral dynamic loading, `eval`,
native code, opaque WASM, mutable unbound globals, unmaterialized transforms,
and unaccepted external dependencies cannot support closure. They weaken only
their named exports and immediate claim domains: complete negatives become
unknown, complete positives become partial, and known positives and unrelated
siblings survive.

This does not reduce the current SC9005/SC9012 corpus because Phase 12 still
owns analyzer consumer migration and Phase 11 still owns acceptance. It closes
the selection abstraction required for those later reductions. Nonliteral
dynamic loading, runtime-generated module names, `eval`, native addons, opaque
WASM, mutable globals with no bound declaration, transforms without stable
bytes and identity, and external dependencies without an accepted contract
digest remain exact fail-closed frontiers. The standalone resolver also refuses
missing or ambiguous exports, invalid targets, escaping symlinks, and stale
hashes; none is interpreted as negative package behavior.

## 2026-08-27 — Replacement generation emits only open Rust proposals

The Phase 8 replacement generator path now constructs semantics inside
`solid-facts-backend::proposal_generation`. Every analyzed artifact case must
first match an independently acquired Phase 7 `ResolvedImport`. Construction
then withdraws every local completeness candidate: complete positives retain
their items as partial positives, complete negatives become unknown, and the
exact recursive leaf becomes a proof obligation. A naturally unresolved leaf
adds its own local obligation without deleting independently provable sibling
closure candidates.

Positive operations remain in the open proposal with possible versus
guaranteed strength preserved. Probe planning includes only possible-positive
witness candidates; it cannot close a domain or establish absence. Proposal
emission is deterministic, declares `acceptance: "unaccepted"`, and contains
no accepted `closed` field, receipt, evidence sidecar, or main wire document.
Repeated analysis plans union monotonically to an order-independent fixed
point, while plans with different semantic digests are refused.

The matching Node module owns only package discovery, Phase 7 standalone
artifact acquisition, process-stage ordering, and final byte handoff. Analysis,
proposal construction, proof planning, and probe planning must return
Rust-owned products; no variant collapse or mutable summary merge exists on
this replacement JavaScript path. The legacy public generator still contains
its schema-v1 merge behavior because Phase 14 owns the atomic producer and
consumer migration and deletion. Consequently current SC9005/SC9012 findings,
bundled contracts, and corpus certification are unchanged. Claim IDs, evidence
sidecars, proof replay, receipts, accepted analyzer consumption, public
generator cutover, and contract regeneration remain open in Phases 9-14.

## 2026-08-27 — Evidence is claim-addressed and leaves the analysis hot path

Phase 9 assigns canonical semantic claim IDs inside
`solid-reactive-ir::contract_semantics`. The ID binds exact package identity,
artifact-case resolution/runtime/declaration/transform/closure identity, exact
export identity, and a validated domain, recursive value, operation-axis,
resource, guard-partition, or positive-operation path. It does not bind wire
summary names, JSON position, formatting, sidecar layout, or unrelated claim
values. Summary renaming and reordered formatting therefore retain evidence;
package, artifact, closure, export, or subject changes do not.

`solid-facts-backend::evidence_sidecars` now emits proof/fact and runtime-probe
documents as separate versioned families. Every proof row records artifact,
closure, fact transcript, proof input, limitation, producer, and tool identity;
every probe row records artifact, closure, recipe, runtime/environment,
sandbox, outcome, limitation, producer, and tool identity. The main temporary
contract names each complete sidecar hash, while each sidecar names normalized
contract/package identity. Validation refuses missing, content-mismatched,
unreferenced, stale, cross-package, cross-artifact, duplicate, noncanonical, or
unplanned evidence.

The validated evidence result contains claim IDs only, and contract
normalization still succeeds using main-document hash references after the raw
sidecars are removed. That Phase 9 slice did not accept any claim. Phase 10 now
owns semantic event evaluation, while proof replay and receipt issuance remain
Phase 11, analyzer consumption remains Phase 12, and public producer/consumer
cutover remains Phase 14. No Type Facts or compiler fact changed, no current
finding moved, and no open Solid 2 runtime premise became negative proof.

## 2026-08-27 — Runtime probes witness positives and falsify closure only

Phase 10 adds the replacement probe authority in
`solid-facts-backend::runtime_probes`. Exact artifact cases and runtime modes
expand into bounded repeat sessions. Every repeat must use fresh process,
realm, and module-instance identity, must remain within the recipe's timeout
and semantic microtask/macrotask drain limits, and must produce the identical
zero-based semantic event transcript. Call, render, flush, callback, cleanup,
settlement, emission, transition, request, response, and stream events replace
elapsed-time behavior guesses.

Only a planned possible operation can produce a possible-positive witness, and
only a planned closure domain can produce a contradiction record. Missing
markers, finite absence, timeouts, errors, isolation reuse, environment
mismatch, excess drain, malformed lifecycles, and repeat disagreement refuse
the exact mode. They never produce complete-negative knowledge, a guaranteed
minimum, a finite maximum, exhaustiveness, accepted closure, or a receipt.
Claim-local sidecar observations preserve this locality across modes.

The module validates ordered cleanup, repeated AsyncIterable emission,
transition, request/response, and root-lifetime scenarios. It deliberately
does not switch the legacy public probe plan, worker, driver, or harness;
implementation-plan item 161 owns that atomic Phase 14 migration. Phase 11
must still replay contradiction records as one proof input, prove every closure
family, compute proof roots, and issue acceptance receipts. Actual RC.3 corpus
recipes and end-to-end worker execution remain open until the Phase 13
conformance work and Phase 14 cutover. No current analyzer finding, Type Facts
protocol, compiler-facts protocol, bundled contract, fixture snapshot, or
public main-schema document changed.

## 2026-08-27 — Only replayed local proof can create accepted closure

Phase 11 adds the sole `AcceptedContract` constructor under
`solid-reactive-ir::contract_semantics::proof`. Every proposed closure leaf
must carry all eighteen semantic-model-v1 proof families. Each family is bound
to the exact semantic claim and artifact/export scope, a bounded raw transcript,
and a complete enumerated-versus-classified census with no unresolved premise.
Package artifact acquisition, Type Facts, compiler execution facts, accepted
dependency contracts, and runtime probes are distinct authorities and cannot
substitute for one another.

The backend derives candidate closure from the Phase 8 plan and converts Phase
10 contradiction records. A matching contradiction refuses that exact leaf.
Successful verification closes only that call, owner-production, resource,
guard, or recursive value leaf and reruns the semantic normalizer; an open
sibling is not contaminated. Canonical proof and closed-claim roots plus exact
artifact, closure, wire, final semantic, verifier-build, and policy identity
form receipt version 1. Bundled receipt reads and local content-addressed writes
rehash bytes.

This does not reduce a current finding. Phase 12 still owns receipt validation
and analyzer consumption, Phase 13 owns published-RC.3 proof corpus execution,
and Phase 14 owns the public producer/consumer cutover and bundled regeneration.
Consequently every current contract path remains on its prior authority until
those phases land. Missing or incomplete Type Facts censuses, unreconciled
compiler sites, opaque dynamic loading/native/WASM/global/transform frontiers,
unaccepted dependency edges, probe contradictions, and absent exact RC.3
transcripts remain local open claims rather than negative proof.

## 2026-08-28 — Accepted normalized semantics are queryable without global refusal

Phase 12 enables the private replacement analyzer boundary without cutting the
public schema over early. A temporary-v2 document is normalized, selected and
rebound against one actual `ResolvedImport`, then compared with its stored
receipt. Receipt version and proof policy, semantic model and digest, selected
artifact identity, dependency closure, and the recomputed closed-claim root
must all agree before `AcceptedContract` is exposed. The proof root remains the
opaque verifier authority; ordinary analysis does not reload raw evidence.

The normalized consumer resolves imports by exact importer/specifier and
exports by exact runtime plus declaration identity. Restricted guards consume
demand-shaped call-site facts. Unknown selection monotonically joins possible
operations, removes no known positive, creates no guaranteed operation, and
reports only the exact open guard or claim leaf. Complete empty knowledge still
proves absence, while empty open knowledge remains unknown. Native dialect
semantics take precedence only when compatible; proved disagreement refuses
the claim instead of choosing whichever answer yields more coverage. Cache
identity includes exact import mappings and every semantic, artifact, closure,
proof, closed-claim, verifier-build, and receipt-policy component.

This phase adds no diagnostic kind and moves no finding. The executable `tsc`
oracle already covers every finding kind currently produced from package
contracts: `strict-read-untracked` (both dialects), `missing-owner`,
`reactive-dispatch-unresolved` (both dialects), `prefer-for`, and
`no-destructure`; `package-contract-incomplete` remains explicitly exempt
because its subject is the presence and authority of an external artifact, not
a TypeScript expression. Phase 14 still owns public discovery, generator,
probe, verifier/tooling, CLI/WASM, fixture, bundled-contract, and legacy-decoder
cutover, so the current stable-schema analyzer path remains unchanged. Phase 13
still owns exact published-RC.3 contract rows and proof corpus execution. A
missing call-site literal/property/tuple fact, unresolved signature or spread,
open guard remainder, open claim leaf, artifact conflict, or receipt mismatch
therefore remains locally uncertifiable and never becomes negative proof.

## 2026-08-28 — RC.3 conformance is explicit without claiming false closure

Phase 13 adds the sixteen-row first-party Solid 2 RC.3 normalized corpus and a
machine-checkable evidence matrix. Every row has positive, clean negative,
partial, refusal, consumer, and real-published-typings oracle cases. Exact
manifest, entrypoint, runtime, declaration, and finite transitive package
closure identities are part of the semantic input. Runtime observation absence
is pinned as non-negative evidence, and artifact drift changes the canonical
semantic digest.

No diagnostic or finding snapshot changed. Fifteen oracle cases type-check
against the exact RC.3 declarations. The server-functions client declaration
is itself rejected by TypeScript because `ServerFunctionMetadata` and
`ServerFunction` are re-exported but used without a local import. The checker
must not diagnose that declaration defect; the declaration leaf remains
TypeScript-owned and open while exact runtime/reference facts stay usable.

The remaining uncertifiable leaves are recorded per row rather than widened:
dynamic effect result/error payloads; unowned `onSettled` cleanup lifetime;
dynamic keyed/control selection; async rejection/cancellation payloads; opaque
refresh/action/store targets; dynamic returned ref arrays; real-browser DOM,
delegation, and hydration observations; request-context and transport
integration observations; user serialization; all unstable frames protocol
details; and the incompatible `onMount`/`onCleanup` imports in
`@formkit/auto-animate@0.10.0`'s exact Solid adapter. Phase 14 still owns public
schema/tooling/bundle migration, so none of these corpus facts reaches ordinary
analysis early.

## 2026-08-28 — Temporary-v2 contracts are the only producer/consumer path

Phase 14 atomically switches package and missing-contract generation, first-
party bundle issuance, probes, review, proof verification, native discovery,
WASM input, differential/pin/conformance gates, and all process/fixture
contracts to the normalized temporary-v2 workflow. Analyzer consumers receive
only an `AcceptedContractIndex` assembled from exact document bytes, a
proof-issued receipt, and an independently acquired `ResolvedImport`. The old
public decoder, schema-1 generator, serialized unknown sentinel, public
variants/conditions, inline evidence, name-only dependency proposal trust, and
duplicate JavaScript normalization are deleted.

This intentionally removes several former package-name successes. A reduced
fixture declaration or implementation cannot borrow the receipted first-party
bundle because its artifact bytes and closure differ. Conversely, stale
`package-contract-incomplete` rows that existed only because a legacy bundle
omitted an export disappear when the exact accepted artifact has complete
negative knowledge. The ten changed finding snapshots record these semantic
differences; none is a weakened proof rule.

Proposal generation still refuses wildcard/unbounded public surfaces,
unresolvable callable kind, non-literal dynamic imports, unsupported classes or
namespaces, and external export-all boundaries without independently accepted
semantics. The corpus records each refusal instead of synthesizing a negative
claim. Missing registry integrity for linked/local packages, missing or
ambiguous exact runtime/declaration/export identity, closure hazards, stale or
absent receipts, unresolved guard selection, and open recursive leaves remain
locally uncertifiable. Runtime probes remain falsifiers only.

The Phase 13 RC.3 open leaves remain open: the server-functions client
declaration's TypeScript-owned self-error; real-browser DOM/delegation/hydration
observations; request-context and transport integration observations; user
serialization; dynamic payload/target/selection leaves; and unstable frames
protocol details. Solid 1 `jsx-runtime` and `jsx-dev-runtime` subpaths with no
common runtime/declaration value binding remain in the checked census without
an accepted semantic case. No Type Facts or compiler-facts producer, protocol,
pin, or generated binary changed in this phase.

## 2026-08-28 — Adversarial contract inputs stay bounded and fail closed

Phase 15 consolidates every untrusted package-contract JSON family behind one
structural limit owner. Main documents, workflow inputs, proof/probe sidecars,
runtime-probe documents and transcripts, receipts, and accepted catalogs have
explicit byte, depth, node, and string limits. Catalog, contract, and receipt
files are size-checked before allocation. Traversal spellings, noncanonical
entrypoints and closure paths, cross-platform absolute paths, mixed-package
specifier substitution, and noncanonical receipt digests are refused.

Normalized validation now treats operation triggers as causal arcs alongside
explicit edges and rejects their combined cycles. Indirect resource-lifetime
cycles are rejected while a typed resource may still name itself as its own
lifetime anchor. The seeded false-closure suite attacks sibling/misplaced
closure, empty open domains, dangling or cyclic identities, contradictory
capabilities, guard overlap and falsely covered remainders, and path or package
substitution. Deterministic mutation fuzzing accepts an input only when
decode/normalize/encode preserves identical semantics and canonical bytes.

This hardening creates no new diagnostic and closes no previously open claim.
Type Facts and compiler producer protocols did not change. Missing or stale
fact generations, incomplete Type Facts domains, unreconciled compiler sites,
unbounded exports, ambiguous artifact selection, closure hazards, stale
evidence, unresolved guards, and recursive open leaves remain locally
uncertifiable. The explicit resource limits may refuse an otherwise meaningful
oversized document; that is an intentional bounded failure, not negative proof.

## 2026-08-28 — Phase 16 measures generatability without promoting coverage to proof

The exact release-binary ecosystem run now covers 418 official probe rows. It
produced 40 structurally complete proposals, 318 partial proposals, and 60
fail-closed rows: **85.65% generatable**. The Solid Primitives subset produced
6 complete, 268 partial, and 17 refused rows across 291 probes: **94.16%
generatable**. “Complete” here means no artifact-case refusal in generator
orchestration; it does not mean semantically closed or accepted. Every emitted
proposal still requires checked proof and a receipt.

The partial rows preserve independently known cases and record 1,458 exact
artifact-case refusals. The remaining full-row refusals are 20 accepted-
dependency obligations, 14 unresolved export-kind censuses, 8 legitimately
empty runtime export surfaces, 2 missing exact package exports, 1 unresolved
parameter-behavior case, and 15 exact package/artifact shapes that remain
unsupported or unresolved (including wildcard-only censuses and missing or
non-file published targets). The proposal corpus keeps all nine call domains
open on 4,788 exports and keeps recursive uncertainty local to 13 exact leaves.
The 36 Phase 13 RC.3 open rows remain separately named in the Phase 16 refusal
artifact.

Compactness and performance are now checked by
`scripts/package-contract-v2-phase16.mjs`. Across 358 emitted ecosystem
proposals, canonical main bytes are 1,599 p50, 4,813 p95, and 43,055 max;
proposal-plan bytes are 49,172 p50, 303,349 p95, and 3,676,782 max. Across the
24 receipt-issued cases, canonical main bytes are 3,342 p50, 16,202 p95, and
21,538 max; raw proof evidence is 348,386 p50, 3,122,474 p95, and 3,209,546
max, while every receipt is 669 bytes. Ordinary analysis retains none of that
raw evidence and performs no package execution, network access, or query-time
file reads.

Generation cost over the 418 rows is recorded rather than hidden: 1,568 ms
p50, 25,439 ms p95, and 434,856 ms max. The current temporary-v2 fresh-process
probe driver costs 20.13 ms p50, 21.16 ms p95, and 21.73 ms max per isolated
session; it remains non-authoritative for acceptance. The accepted 24-case
corpus loads in about 15.9 ms p50/16.2 ms p95; normalized export lookup is 31
ns p50/32 ns p95 in the checked release run. The 31,232 KiB peak-RSS delta is
explicitly a whole-process upper bound including checked-corpus construction,
not a retained analyzer-heap claim. Phase 16 adds no compression transform
because the current main documents already pass the 8 KiB p50, 32 KiB p95, and
1 MiB maximum gates; compression remains allowed only when normalized semantics
and proof identity are unchanged.

The post-handoff CI audit closed two gate gaps without closing any semantic
premise. Peak RSS now calls `getrusage` only on Unix; Windows and WASI expose
that measurement as unavailable instead of failing to compile. The 39-fixture
generator corpus now pins refusal sidecars for partial proposals and is part of
`make verify`: 40 retained artifact cases coexist with 14 exact local refusals,
while 5 packages remain full fail-closed refusals. Eight fixtures that formerly
stored only the first whole-package error now retain their known cases and name
the unsupported condition, merge, or inference-entrypoint leaf separately.
Those local refusals remain uncertifiable; snapshot migration is not proof.

## 2026-08-29 — Phase 17 converges temporary-v2 mechanics without closing claims

The repository-wide convergence authority inventories 130 active temporary-v2
main documents, 73 exact byte-bound receipts, 69 independently versioned
documents, 15 producer/consumer source owners, and 579 active JSON files. The
retired legacy-v1 schema and dead Phase 0 decoder reconstruction are gone; the
only analyzer normalization boundary is the Rust temporary-v2 decoder. Rule,
dialect, receipt, catalog, sidecar, runtime-resolution, runtime-probe, cache,
and registry-memo versions remain independent rather than inheriting the main
document's temporary schema number.

Semantic-model version 1 now names and tests its SHA-256 algorithm, canonical
domain separator, typed length-delimited encoding, and golden digest. This is
an identity freeze, not new semantic evidence. Cache format versions moved to
2 so pre-convergence cached verdicts cannot cross the widened audit boundary.
No Type Facts, compiler facts, bundled main document, receipt, finding, or
closure claim changed.

The Phase 16 boundary remains exact: every generated ecosystem proposal stays
unaccepted without proof and receipt issuance; 60 full-row and 1,458 local
artifact-case refusals remain uncertifiable; all open call domains and 13
recursive leaves stay local; and the Phase 13 browser, request/transport,
serialization, dynamic-leaf, TypeScript-owned declaration, and unstable-
protocol domains remain open. Wildcard-only surfaces, missing or non-file
targets, unresolved callable kinds, external export-all without accepted
dependency semantics, closure hazards, stale receipts, and unsupported
artifact shapes continue to fail closed.

## 2026-08-29 — Phase 18 cuts stable-v1 without changing semantic closure

The sole main-document owner now emits and accepts stable public
`schemaVersion: 1` only at `schema/solid-reactivity.schema.json`. The required
`format: "solid-reactivity-contract"` discriminator separates it from the
retired legacy-v1 shape; temporary-v2 is rejected. All 130 mains were re-
emitted and all 73 acceptance receipts were reissued over their exact stable
bytes. First-party proof transcripts and their wire-only sidecar/proof roots
were regenerated where the stable encoding changed their inputs. Semantic,
artifact, closure, closed-claim, verifier, and semantic-model identities did
not change.

The Phase 18 convergence authority inventories those documents plus 69
independently versioned neighboring documents and 15 source owners. Runtime-
probe, review, dialect-manifest, runtime-lock, receipt, catalog, sidecar, Type
Facts, and compiler-facts versions remain in their own namespaces. Gate-cache
and registry-memo formats moved to 3 so no pre-cut verdict crosses the widened
input closure. No Type Facts producer/client/schema/protocol or Solid compiler
fact/pin changed.

The uncached 418-row ecosystem authority remains exactly 40 complete
proposals, 318 partial proposals, and 60 full-row refusals, with 1,458 local
artifact-case refusals. Every emitted proposal remains unaccepted without
proof and receipt issuance; 43,106 open domain/recursive observations remain
local across the measured proposal corpus. The Phase 13 browser,
request/transport, serialization, dynamic-leaf, TypeScript-owned declaration,
and unstable-protocol domains remain open. Wildcard-only surfaces, missing or
non-file targets, unresolved callable kinds, external export-all without
accepted dependency semantics, closure hazards, stale receipts, and
unsupported artifact shapes continue to fail closed.

## 2026-08-30 — Phase 19 authenticates policy 2 and remeasures every complete proposal

The active loader now requires receipt version 2 and authenticated built-in,
persistent-local, or portable issuer provenance. Caller proof-file issuance,
the checked-corpus acceptance shortcut, and policy-1 loading are removed. All
73 baseline policy-1 receipt documents have final retired/demoted dispositions
with zero pending; both first-party bundle indexes remain empty because no case
has yet reconstructed the complete policy-2 demand graph. This is a deliberate
loss of accepted coverage, not a demotion of proof into warnings or guesses.

The uncached 418-row generator remeasurement produced 44 structurally complete
proposals, 314 partial proposals, and 60 full refusals: 358/418 (85.65%) remain
generatable. Compatible condition axes and finite wildcard resolution moved
four formerly partial rows to structurally complete without accepting any
semantics. The same census removes four synthetic `browser,node` artifact-case
refusals from the focused contract corpus while retaining every real empty-case
or merge refusal. One `@kobalte/core` source wildcard would require 1,120
entrypoint/condition candidates and is refused against the Rust-owned 1,024
candidate budget; its independently bounded root surface remains generatable.
The five historical wildcard full-row failures were remeasured and now name
deeper export, dependency, no-surface, or artifact owners. They unlock zero
verified exports. Dynamic and opaque shapes remain open at their exact leaf.

All 44 structurally complete rows then underwent real policy-2 attempts against
exact registry bytes. Five stopped during snapshot/provenance or closure replay
(one duplicate archive member and four closure digest mismatches). Thirty-nine
reached witness acquisition and exposed 1,456 exact Type Facts-owned demand
instances; no attempt reached semantic certification or receipt issuance.
Across the attempted graphs, 71 instances of each artifact-wide family were
reconstructed from immutable snapshots, but serialized artifact evidence is
not treated as authority and no surrounding claim domain closed. The current
proposal corpus contains 4,562 exports and 41,072 open claim leaves; policy-2
verified exports, verified analyzer-visible positive facts, locally closed
domains, receipts, accepted-load samples, and accepted-query samples all remain
zero. Null cost/byte percentiles in the Phase 19 report mean “stage not
reached,” never zero-cost verification.

## 2026-08-30 — Context 0.3.2 is a confirmed published-declaration defect

The authenticated `@solid-primitives/context@0.3.2` archive imports
`../node_modules/solid-js/types/reactive/signal.js` from `dist/index.d.ts`.
That path requires a nested peer inside the package archive, but the archive
contains no `node_modules` tree and the real `solid-js` peer is hoisted. A
strict published-typing oracle reports TS2307 on that exact import. The
`@solid-primitives/source` export condition also names unpublished
`./src/index.ts` bytes. Phase 21 therefore retains both artifact-case refusals
and assigns the remaining owner to the upstream package; it does not create a
fixture-only nested layout or add a duplicate checker diagnostic. The exact
archive, layout, and oracle are recorded in
`docs/package-contract-v2/phase21/context-upstream-declaration-defect.md`.

## 2026-08-31 — A query-suffixed specifier is an asset import, not a missing module

A bundler resource query (`./read-preferred-language-cookie.js?raw`) was read as
part of the filename, so the artifact resolver reported a file the package ships
as a missing local closure module and refused the whole artifact case. That
shape produced 204 recorded `local closure module ... was not found` refusals in
the ecosystem census, all from `@kobalte/solidbase`.

The specifier is now opaque on both sides of the closure. Stripping the query
and walking into the module would be worse than the refusal, not better: the
binding's value is the loader's product -- a string for `?raw`, a URL string for
`?url`, a constructor for `?worker` -- and never the target module's exports, so
walking in would attribute the target's semantics to a binding that has none.
The specifier therefore contributes no closure edge and no resolved binding,
only the `unaccepted-external-dependency` frontier an unaccepted bare dependency
contributes, and every claim of the artifact case stays open.

This is deliberately conservative in three places. The frontier carries no
`affectedExports`, so one asset import leaves every export of that artifact case
unprovable rather than only the exports reachable through the binding; a query
target that is absent from the package is as opaque as one that is present,
because no loader-independent resolution exists for either; and a `#` fragment,
a `#imports` specifier carrying a query, and a bare specifier carrying one are
all opaque too, the last being kept out of the external dependency census
because no package entrypoint answers to a suffixed subpath. An unsuffixed
relative specifier with no file still refuses, and a literal on-disk filename
containing `?` or `#` stays unreachable from a specifier.

`fixtures/package-contracts/asset-query-import` pins the semantics: the same
shipped file imported as a module and as `?raw` yields a proposal with both
exports, twenty open claims, and no proof candidate. The published-graph
acquisition still refuses a node whose closure carries an opaque asset frontier,
exactly as it refuses `node:` and every other non-package external specifier;
that path is unchanged.

## 2026-09-01 — A re-exported runtime chunk is a witness-program member

The private Type Facts witness project listed only the harness plus the export
bindings' own runtime paths of a single representative plan in its
`tsconfig.json` `files`. Its harness imports declaration modules, and TypeScript
resolves every declaration re-export specifier to the sibling `.d.ts`, so a
runtime chunk that a re-export alone names never became a program member. The
producer's `sourceFileFor` then found nothing and returned an implementation
transcript open with `sourceUnavailable` — for a file the package ships.
`@tanstack/devtools-ui@0.7.1` (`ensureDevtoolsFonts`, implemented in
`dist/esm/styles/semantic-theme.js` and reached through `dist/esm/internal.js`)
and `@kobalte/core@2.0.0-alpha.0` (`ColorModeProvider`) were the two ecosystem
rows the census recorded under that reason.

The program's file census now also lists each plan's independently replayed
runtime module closure, and it covers every plan the one project serves rather
than only the plan whose package root was materialized first. `sourceUnavailable`
was never evidence that a package ships no source, and this is not new authority:
adding a source file to the program asserts no flow and no fact, the bytes are
the same authenticated snapshot materialization as before, and every premise is
still proved against the snapshot. Only the closure's runtime-axis entries are
listed — declarations remain TypeScript's to resolve and are pinned by the
declaration source census, resolution inputs are not modules — and a path the
materialized snapshot does not carry is dropped, so a demand whose module is
genuinely absent stays open exactly as it was. The closure is the package's own
module graph: `resolve_local` classifies every bare specifier as external and
records it as a dependency edge or an opaque frontier instead of visiting it, so
no dependency's internals enter the program beyond what materialization already
placed.

`@tanstack/devtools-ui@0.7.1|solid1|only` now certifies. `@kobalte/core@2.0.0-alpha.0|solid2|only`
clears the open premise and refuses one stage later, on `recursive-value-shape`
for `createDomCollection` ("operation value path has the wrong callability") —
an unrelated open claim that remains outstanding. The program grew from 2 files
to 47 for `@corvu/utils@0.4.2` and from 2 to 237 for `@kobalte/core`; witness
acquisition moved within run-to-run noise on a debug binary.

## 2026-09-01 — Twenty-four generator fixtures were asserted by nothing

The normalized-v2 migration (`474c101f`) deleted the Rust generator process
suite in `rust/crates/solid-facts-backend/tests/contracts_process.rs`, which
shrank from roughly 1738 lines to 121, and moved fixture registration to
`fixtures/package-contracts/corpus.json`. Thirty-three fixture directories were
named by the deleted tests; only some were carried into the new manifest.
Twenty-four `fixtures/package-contracts/` directories were left registered
nowhere and referenced by name nowhere, so their claims were asserted by no
gate at all for four weeks. They had no snapshots either, so nothing about them
was even observable.

Twenty-three are now registered in the corpus with reviewed snapshots. Nothing
about the analyzer changed; what changed is that these claims are now falsifiable.
Every one of the resolution branches `legacy:index` (runtime), `legacy:main`
(declarations) and the `legacy:module`/`legacy:index` pair was unpinned, as was
the CJS-only runtime refusal, the `accepted-dependency-binding` refusal on an
external re-export — the most common refusal class on real registry packages —
and the deferral of a callback that escapes through a returned callable whose
identity `Object.assign` preserves.

`carried-value-kind` is the exception and stays unregistered. Its claim is that
an installed dependency's own semantic document cannot launder a false value
kind, and it depends on `.solid-checker/accepted-contracts.json` reaching
analysis. `contract generate` — the only driver `scripts/contract-corpus.mjs`
runs — never loads that catalog; `acceptedDependencies` defaults to `{}` and is
supplied only by `contract certify`, which no repository gate runs over a
fixture. Registering it would pin an `accepted-dependency-binding` refusal that
is an artifact of the wrong driver while the laundering claim stayed untested.
It is not dead: it is one of the twenty-one tracked catalogs
`auditPhase19Cut().obsoletePolicy1Catalogs` counts, and that count already
describes those catalogs as policy-1. The claim needs a certification gate, or a
policy-2 reissue of the fixture's catalog, before it asserts anything again.

## 2026-09-01 — The invoking-position round re-measured: 331 verified, every movement owned

The full 418-probe re-measure after the invoking-position round (chain
composition, reach-carrying parameter uses, dialect-derived schedules,
full-chain read rooting) moved seventeen verdicts: 320/73/25 became
**331 verified / 62 exact refusals / 25 not attempted**. Fourteen gains, three
losses, all attributed:

- Twelve gains were the round's named acceptance targets (async ×2, autofocus,
  date-difference, flux-store ×3, meta 0.29.4, router 1.0.0, script-loader ×2,
  timer 1.4.4). flux-store and meta certify by *withdrawal* of the ungrounded
  arg-0 invoke claims — their callback domains stay open, nothing false is
  asserted.
- `solid-js@2.0.0-rc.3` was a bonus of the read-rooting repair: its old
  `$$component` demand pinned path `["set"]` — the same last-member-segment
  defect as hotkeys — and the corrected full path is witnessable by the
  signature census.
- `@kobalte/utils@0.9.2` left the open-root-observation class because the
  generation changes re-rooted/withdrew the `debugPolygon` demand whose root
  observation was open (`Polygon = Point[]`, the M1c array `openIndex`
  misattribution). It left by claim movement, not by the producer closing the
  observation; the M1c producer defect itself is still open and still owns the
  remaining rows of that class.
- **Three honest losses**, both premised on the unsound deep-containment rule
  the adversarial review killed (a call site credited through byte containment
  alone): `@solid-primitives/input-mask@1.0.0-next.2` floor+head — `replacer`
  runs inside an arrow handed to `String.prototype.replace`, which is
  deliberately not on the reviewed default-library invoker table; recover by
  adding `String.prototype.replace`/`replaceAll` (replacer at slot 1) as a
  reviewed table row with shadow/user-typed negative pins. And
  `@solid-primitives/jsx-parser@0.2.0` — `render` runs inside a closure
  returned by the *returned* closure; the chain premise models return-carry
  only at the export implementation's own return sites, so a second-order
  return needs a per-callable return-carry fact before it can compose. Both
  old certifications drew the right conclusion from a premise the review
  proved unsound in general; they stay refused until the sound premise exists.

## 2026-09-01 — Producer roots distinguish local answers from subtree enumeration

The producer-root round remeasured the complete 418-probe corpus at **340
verified / 53 exact refusals / 25 not attempted**, from 331/62/25. Every status
or first-refusal movement is attributed below. The canonical debug checker was
rebuilt before focused probes, the Type Facts producer was rebuilt after the
wire change, and the full report is bound by SHA-256
`c23caf2b0816ce759e473e66252dd1e841775eaaa9a1b255a01659d489f33519`.

The Type Facts protocol is now 9, with schema digest
`sha256:1a9cda6d1e2423bf9c07d42b56718c2b63f63584ca9db357f51772e09d59cf7b`.
`CallablePathFact.complete` once again means that the node itself was answered;
the new required `subtreeEnumerated` field says whether the producer exhausted
its descendants. Exact positive demands require a present, locally closed
node. Whole-census demands additionally require an enumerated subtree. Missing
wire data is rejected rather than defaulted, and an absent node cannot claim a
non-enumerated subtree.

The producer no longer stamps the root-shape observation with `openIndex` for
three exact cases where the index belongs to TypeScript's apparent member
surface: a fixed tuple without optional/rest elements, the intrinsic index of a
known primitive string domain, and an exact global `Array`/`ReadonlyArray`
numeric index whose value is the element type. Path facts retain `openIndex`, so
this closes no hidden member census. A separate latent defect stamped an
index-signature owner's openness on its last recursively visited descendant;
the owner now receives the reason. String/symbol author indexes, augmented
tuples/arrays, unions with an open constituent, optional/rest tuples, boxed
`String`, generic array-likes, and shadowed declarations remain open.

Adversarial review found three permissive defects while splitting the wire, and
all three were repaired before remeasurement: exact positive consumers had
accepted `Absent`; instantiable generic path nodes were emitted locally
complete; and synthetic union absence was invented below cycle/depth/open
prefixes. Required and optional present nodes can discharge exact positive
demands only when locally closed. Synthetic absence is emitted only below a
required, reasonless, locally complete, subtree-enumerated prefix. No
`require_verifiable_root_premise` guard was relaxed.

Eleven rows became verified, with their prior first demand digests:

- fixed tuple roots: reducer `d01d94212fb4701ac719bdd00cf8f9b5b92f5e0da5097bbac761cf6f512325b9`,
  selection `59c2bdc4e38a0f0ec9e36d23be21c6794327c1fbf57a3d414fd3d7f78f18f29b`,
  share `cca72a938c3337e5fe0cb39fcb576584c31d7d4df7164b47d1ca20acd02fd160`,
  controlled-props `2275b99fa1c2153803f4c41371bf94a033868f06423ec696ae170b2d86185c36`,
  and cookies floor+head
  `16f840b92d4ab22f01ab0be4fc1a1651450ffab2883354c8b7d065a490e47d48`;
- tuple root plus the local/subtree split: websocket floor+head
  `5108bf30341276093d839251f86ee093a0277bb39289f2deb10bfe8600c2044e`;
- primitive-string apparent indexing: TanStack solid-hotkeys
  `b378eced209d466d4fdd56c0da3ab5080667d1efb367cc1423a7692fab7bd923`;
- Corvu utils 1.x
  `6435189c4f5db8b6ffa1e800120cae4ff79e9a7014934e78dd712f22d4bcf88c`
  and Corvu-next utils
  `b381a04cd73ab6bbc28fd91a24cd5b077bef5335819861bcfe5062ff61481441`.
  These two were unexpected but are not over-proof: the demand belongs to
  exported subpath `./create/controllableSignal`, whose authenticated `.d.ts`,
  `.js`, and `.jsx` all explicitly default-export the callable. The diagnosis
  had inspected the unrelated package-root declaration.

Two previous certifications were deliberately removed: flux-store floor and
head now stop at
`a73e5975ad750e6a1856af4f0dd44840aab011b46e09f30bbe89a82241e5a84f`
because the required path is instantiable and carries `unresolvedGeneric`.
Those rows were false certifications produced by local completeness on a
caller-chosen generic. Recovery requires a real generic premise, not another
consumer exception.

Six refused rows advanced to later exact blockers without certifying:

- db-store: `d5d6bbc4…` open tuple root -> `3bb4a83e…` an actual descendant
  `openIndex`;
- i18n 1.x: `5d48e2da…` -> `8e2a7a25…`, absent `proxyTranslator.bind`;
- i18n 2.x floor+head: `2bd38ac7…` -> `bb9c0374…`, open
  `missingKeyAsPath` root;
- utils 2.x floor+head: `83483402…` -> `ecbf77c9…`, absent
  `wrapSetter.slice`;
- Kobalte core: `32c3e7f3…` depth truncation -> `1808f351…`, the requested
  alternative is locally `Absent`.

The producer transcript dump for favicon falsified the diagnosis's early-
refusal hypothesis: the transcript completes, but its exact export root is
still `openType` with unknown callability/constructability. Both
`1df2f037…` rows remain refused and need a narrower producer investigation.
The published-typing must-not-clear controls (`@solid-devtools/ui`,
`solid-devtools`, `@solidjs/web`, TanStack store/form, and the generic i18n
inputs) remain refused. The five certified control probes remain certified.
All 25 not-attempted rows are unchanged.

## 2026-09-01 — Type Facts identity refusals name stable evidence, not an opaque build-dependent subject

`TypeFactsCertificationError::IdentityMismatch` was a fieldless variant raised
from nineteen sites, so every plan, envelope, count, location, and
implementation disagreement produced the same sentence. The guards remain
fail-closed, but now report a literal site/field and oriented expected/actual
values. Path identities are package-relative or explicitly redacted; pair-aware
rendering distinguishes unequal private roots whose safe suffixes collide.
Tests pin all four expected/actual implementation-presence combinations, the
schedule root/count split, guard precedence, and path-redaction collisions.

Focused canonical reproducers show the underlying M9 failure: a scheduled
authenticated implementation location is `Some`, while the producer returns
`None`. The TanStack query and persist-client rows remain exact refusals; no
semantic authority changed. Case-set requests now sort by stable package
coordinates before digest, removing digest order as the source of the first
failing graph-node name. The identity/evidence pairing remains digest-keyed
after acquisition. Two repeated debug probes selected the same package
coordinate; release-profile confirmation is deferred to the Round 2 full
remeasurement. M9 is diagnosable, not yet semantically repaired.

## 2026-09-01 — Explicit re-exports take precedence over export stars during proposal emission

The package-contract emission walk now mirrors ECMA-262 export precedence per
module: one explicit runtime binding is followed without consulting that
module's bare export stars. Aliases follow their source name; type-only and
namespace exports cannot masquerade as runtime stars; duplicate explicit
specifier/default/namespace-export bindings refuse before identical candidates
can collapse; the reviewed TypeScript class+namespace merge remains one runtime
binding through an exact AST namespace-declaration-name fact, without collapsing
duplicate const/class/destructuring declarations; and distinct star identities
remain ambiguous. Ambient or string-named modules provide no runtime namespace
fact, and a nested namespace cannot reclassify its containing declaration. The same precedence check runs
before a local program summary can bypass re-export validation.

The canonical `motion-solidjs@0.6.0|solid1|only` artifact-stage reproducer has
no demand digest. Before the change it stopped while generating
`framer-motion` `./dom`, where public `delay` had two candidate identities.
After the change that dependency proposal emits and the resolver binds
`delay` to `motion-dom`'s `delayInSeconds`. The outer probe does not certify:
it advances to `motion-solidjs`'s local `addScaleCorrector` import/re-export,
whose generation pass lacks an accepted dependency contract. That later
refusal is retained; missing authenticated composition is not inferred from
matching bytes or a resolved path. The Round 2 full-corpus movement and report
digest remain deferred until all bounded artifact-mechanics slices land.

## 2026-09-01 — Declaration extension substitution follows the selected module format

The live package resolver and authenticated snapshot replay now substitute
declaration extensions from the selected module format: `.mjs`/`.mts` can use
only `.d.mts`, `.cjs`/`.cts` only `.d.cts`, and ordinary JavaScript/TypeScript
suffixes use `.d.ts` before retaining the pre-existing source fallback. Direct
`.d.*` targets and the six extensionless stem/directory candidates keep their
prior order. No identity or suffix comparison was relaxed, so a missing
format-matching declaration still refuses instead of borrowing a declaration
the compiler did not read.

Adversarial review found that the first Rust implementation disagreed with
Node's `extname` for legal leading-dot basenames such as `dist/.mjs`, and that
the extensionless test protected only the first candidate. Snapshot replay now
uses the same leading-dot and multi-dot extension classification as the
JavaScript resolver. Both implementations pin all six extensionless candidates
as successively first-present, and dotfile tests include the wrongly formatted
sibling that the rejected implementation would have selected. Re-review found
no remaining live/replay divergence or over-proof.

The canonical `@tanstack/ai-solid@0.19.1|solid1|only` reproducer advanced from
the `@ag-ui/core` exact-subject mismatch at demand
`2dd8afbd69ebe886683183c10f6e080712c0de1d0e21362540280f7fe8f4048f`
(`events-Bg2nO3O2.d.mts` live versus snapshot-selected
`events-JPFRVbr9.d.ts`) to demand
`54f2616d91b9d1f7f571f5b77919feccd0dd20c036afed72c214a9ee5180cf46`
on `@tanstack/ai`'s `parseWithStandardSchema`. That later root observation is
open and therefore remains an exact refusal. The diagnosis's prediction that
M2 alone would certify the row was falsified by the byte-reproducer; the M2
selection defect is gone, but the unrelated open producer premise is not
inferred. The parallel proposal refusal for `@tanstack/ai-client`'s missing
exact `StorageUnavailableError` runtime binding also remains unchanged.

The non-updating contract-corpus run exposed two expected fixture movements.
`json-import` had used its `.mjs` runtime file as its declaration; it now carries
an exact `index.d.mts`, remains a successful JSON-import fixture, and its
generated contract/proposal snapshots name that declaration and digest.
`legacy-dual-root` deliberately publishes no declarations, so its `.cjs` main
now refuses earlier at `declarations-not-found` instead of analyzing the CJS
runtime bytes and failing later on export identity. Both snapshots were
reviewed at their non-updating failures before being regenerated; no other
corpus snapshot moved.

All seven artifact-mechanics publisher-defect controls remain refused:
`@tanstack/solid-start-server` ×3 still lacks `#tanstack-router-entry`, and the
Solid 2 TanStack query/persist-client rows ×4 still lack the undeclared
`@solidjs/web` peer that `tsc` reports. The Round 2 full 418-probe remeasurement
and authoritative report digest remain deferred until the remaining bounded
artifact slices land; this slice changes the first refusal within one row, not
its verdict.

## 2026-09-01 — Byte-identical canonical archive duplicates are idempotent

Authenticated archive ingestion now accepts a repeated canonical regular-file
path only when the second member's bytes are exactly equal to the first. Both
payloads still count against the expanded-byte limit and both entries count
against the member limit; unequal bytes remain `DuplicateMember`. The logical
file map, member count, and snapshot root are therefore identical to a
hypothetical archive containing one copy. Repeated zero-sized directory entries
remain inert, while file/directory kind conflicts, unsupported links, unsafe
paths, and case-fold collisions remain fatal.

Adversarial review found three archive-topology/resource defects while removing
the blanket duplicate refusal. Explicit empty directories initially did not
participate in ancestor topology, case-different ancestors such as a `Dist`
file plus `dist/child` escaped that topology on case-insensitive extractors,
and nonzero directory payloads bypassed expanded-byte accounting. The final
ingestor validates file prefixes against both exact and folded explicit
directory/file topology and rejects nonzero directory payloads before
discarding the entry. Both archive orders, byte-identical case collisions,
symlink/hardlink collisions, conflicting duplicate bytes, and a duplicate that
crosses the expanded-byte limit are mutation-pinned. Final re-review found no
remaining bounded M7 over-permission.

The canonical `@solid-primitives/start@0.0.4|solid1|only` reproducer has no
demand digest. Before the change certification stopped at artifact provenance
with `duplicate archive member: dist/index.cjs`; after the change the same
authenticated tarball certifies through catalog publication. The focused
single-member/duplicate-member test pins the unchanged snapshot root. All eight
artifact-mechanics must-not-clear rows retain their exact defects: the three
Start Server rows lack `#tanstack-router-entry`, four Solid 2 TanStack
query/persist rows lack the undeclared `@solidjs/web` peer, and
`@solid-primitives/context@0.3.2` still refuses its absent published declaration
closure module. No protocol, schema, or generated artifact changed. The Round 2
full 418-probe remeasurement remains deferred until the remaining bounded
artifact slices land.

## 2026-09-01 — Default-export resolver and Type Facts identities are separate

M5's two default-export mismatches came from one overloaded replay field. Three
identities are now explicit: the module resolver's canonical target name, the
export selector that actually addresses that target from its terminal file,
and the producer's exact query name at the replayed span. They coincide for
ordinary exports but not for every default or aliased form. For
`export default createX`, the resolver target and terminal selector remain
`default` while the runtime query name is `createX`. For
`export default function createX`, the runtime query name is `createX`, the
declaration query name is `default`, and a same-file `export { createX }`
retains its resolver name and selector while following the binder-selected
default declaration span. Exact-name equality was not relaxed.

The necessary AST premise is exact binder identity, not spelling:
`ExportNamedDeclaration` local specifiers now contribute their Oxc
`reference_id -> symbol_id -> declaration span` to the existing reference table.
Replay recognizes a default function or class only when that exact declaration
span is the default declaration's binding identifier. Anonymous defaults retain
no synthetic identity. Forced-exact harnesses import with the terminal selector
and verify with the separate producer query name; resolution-variant and
evidence identities bind all distinct fields. Star-candidate identity also
includes the terminal selector, so paths to `default` and to a same-named live
binding cannot collapse.

The canonical before/after rows were:

- `@solid-primitives/local-store@1.1.4|solid1|only`: before, demand
  `sha256:174bb5aa2e0dcae2c5cfd3883b5358050541f3fd8a4e559de81951ea5efefaad`
  refused the internally inconsistent runtime identity. After, that demand is
  discharged, but the row honestly remains refused at demand
  `sha256:19c7d299065f90f653a83b32d2fcae879786e32da9c9942ab3a89d4dd8dcc645`:
  the required recursive value path is locally open with
  `unresolvedGeneric`. The diagnosis's predicted certification was therefore
  falsified; the root-closure gate remains unchanged.
- `@solid-primitives/tween@1.4.1|solid1|only`: before, demand
  `sha256:53bc385769aecf2468dce0903cc8795b2d832d96f3470697742bb283c1136d49`
  refused the declaration-name mismatch. After, the same authenticated row
  certifies through catalog publication; there is no refusing after-demand.

Adversarial review found four load-bearing gaps before final acceptance. The
resolver name was not mutation-pinned independently from the query name, a
forced-exact harness could incorrectly use a non-importable identity as named
import syntax, and star fan-in could collapse paths to distinct default versus
live bindings without a terminal-selector key. Finally, dependency declaration
authentication treated public/selector aliases as declaration identity, which
could accept a different same-file declaration; only exact replayed resolver or
query identities are now authoritative. All four are pinned, along with named
functions/classes, forward named exports,
`export { x as default }` versus `export default x`, local re-export/import
propagation, unplanned external defaults, exact mismatch, and anonymous-default
refusal. The three Start Server rows, four Solid 2 TanStack
query/persist rows, and the `@solid-primitives/context` declaration defect retain
their exact fail-closed causes. No Type Facts protocol, schema, snapshot, or
generated artifact changed. The Round 2 full 418-probe remeasurement remains
deferred until the remaining bounded artifact slices land.
