# ADR 0162: A returned accessor is described by the dialect row of the call that created it

- Status: accepted and implemented (2026-09-30); written with the implementation.
  The owner decided on 2026-09-30 to design and implement the returned-accessor
  shape for the largest class of the `recursive-value-shape` wall.
- Date: 2026-09-30
- Owners: the semantic model (`DescribedRead::OwnedMemo`), the dialect
  (`Dialect::computed_accessor_read`, `unambiguous_computed_accessor_read`,
  `unambiguous_options_argument`; `solid-dialect/src/lib.rs`, `solid_2.rs`), the
  generator's described-callable walk (`returns_walk.rs`), the described-callable
  census (`type_facts.rs`: `owned_accessor_witness`,
  `computed_owned_accessor_witness`, `owned_returned_accessor_witness`,
  `inert_signal_arguments_discharged`), the Type Facts producer
  (`ImplementationValueSource.argumentsNotFunctionSyntax` and
  `.argumentsPlainOptionsSyntax`; handshake protocol 76), the wire schema, and
  the consumer's projection (`contracts.rs`, unchanged in code)
- Relation: extends ADR 0145 (described callables) and ADR 0146 (an owned signal's
  read) with the second accessor source, and moves ADR 0146's argument premise from
  "every argument a primitive by grammar" to what the audited bytes actually test.
  Reads ADR 0155 (composed dependency returns) and ADR 0157 (parked) for what it
  does not do. No new value shape: a returned accessor is still
  `ValueShape::DescribedCallable`.

## Context

The `recursive-value-shape` wall of the @solid-primitives checkpoint is the
`return` operation ADR 0113's plain proposal made and the census refused. Its
largest class by declared return type is an accessor `() => T`: **41 exports in 9
packages** (host free, base `bbde7700`). By how the accessor is built:

| how the returned accessor is built | exports | packages |
| --- | ---: | --- |
| the whole value is a `createMemo(literal)` call | 31 | `signal-builders` (30), `refs` `resolveFirst` |
| a conditional of two `createMemo` calls | 2 | `signal-builders` `filterInstance`, `filterOutInstance` |
| a memo bound, written to (`memo.toArray = …`) and returned by name | 1 | `refs` `resolveElements` |
| a call of a local helper or of a caller's callable | 4 | `range` `createNumericRange` (`mapRange`), `event-bus` `once` (the caller's `subscribe`), `raf` `createMs`, `cursor` `makeBodyCursor` |
| a cleanup function, not an accessor at all | 1 | `rootless` `createDisposable` |
| a tuple element of a composed dependency (ADR 0157) | 1 | `spring` `createDerivedSpring` |
| a parameter or a literal | 1 | `utils` `asAccessor` |

Two more families sit beside the class and are the other accessor-shaped
refusals of the wall: an accessor from a **`createSignal` the export made itself**
(12 exports under `browser`, from the ADR 0146 census: `createDevices`,
`createPointerList`, `createWSMessage`, `createPermission`, `createSensor`,
`createBattery`, …), refused for ADR 0146's argument premise ("every argument a
primitive by grammar") over an array literal, an options object or a binding; and
the accessors of **composed dependencies**, which ADR 0157 records and this ADR does
not change.

`createMemo` is not `createSignal`. ADR 0146 refused it in so many words:
"`createMemo`'s accessor recomputes its caller's function and is not stated". The
measured bytes are the reason. On `@solidjs/signals@2.0.0-rc.9`,

    createMemo(e, t)  ==  accessor(computed(e, t))       // dist/prod/signals.js
    accessor(n)       ==  read.bind(null, n)

and `read` of a node with a compute function calls `prepareComputed` /
`updateIfNecessary`, which re-run **that registered function** when the node is
stale, and may throw the node's own error or a not-ready signal
(`dist/prod/core/core.js`, `read`). So a memo's read is a tracked read that is not
inert, and the described-callable vocabulary as ADR 0145/0146 wrote it
("`callbacks: []`: invokes no callable it did not itself define") cannot say what it
does without a statement of *whose* code it runs.

## Decision

### 1. A returned accessor is described by the dialect's row for the call that created it

**A returned value is an accessor exactly when the producer traces the site's whole
value to a call result of a dialect primitive whose accessor slot the dialect states
a read for, the call is made in the export's own implementation and resolves into an
audited dialect archive that is a dependency of the certified package, and the
argument condition the row states is discharged at that call.** The export's
`returns` then states the accessor as one described callable: `reads` the row's
read, `returns: [read-value]`, `callbacks: []`. Nothing is inferred from the
callee's spelling anywhere the census decides.

Two rows exist, and they never overlap:

| row | dialect answer | read | condition at the creating call |
| --- | --- | --- | --- |
| `createSignal`, tuple slot 0 (ADR 0146) | `inert_accessor_read` | `owned-signal` | the first argument cannot be a function, the options argument carries no callback (§ 3) |
| `createMemo`, whole result (this ADR) | `computed_accessor_read` | `owned-memo` | no spread; callback-free options (§ 2) |

### 2. The memo row: `owned-memo`

`DescribedRead::OwnedMemo`, wire `"owned-memo"`, canonical byte 1 in the reads
sequence (appended; no earlier document carries it, so it needs no digest family).
One invocation of the described callable observes the current value of a memo the
export's own invocation created with the dialect's `createMemo` and handed back
unaltered, on the invoking caller's stack and in that caller's tracking context.

**What the read does, said outright.** It is not inert. When the memo is stale the
read re-runs the computation **the creating call registered**. That computation is
whatever `createMemo` was handed -- a literal the export defined, or a callable the
export was itself handed -- and the callables it invokes are the code of the export's
package or of its caller. The claim this ADR makes, and the premise the owner is
asked to hold, is:

> The executions of a memo's registered computation, and every callable they invoke,
> belong to the export's own `creates` and `callbacks` claims, where the computation
> is registered (the dialect's `createMemo` row is that registration). A read of the
> memo is an observation of the memo, not an invocation of that code. So the
> described callable's own `callbacks: []` is true in the sense ADR 0145 gives it --
> *this invocation* invokes no callable it did not define -- and says nothing about
> the registered computation, which the read may re-run.

**The row is archive-scoped.** The runtime evidence above is for
`@solidjs/signals@2.0.0-rc.9`, with the dialect's exact integrity and manifest
digest. Certification binds that tuple through
`computed_accessor_read_archive`; an archive merely appearing in the audited
archive list is insufficient. Older signals releases and `solid-js`'s own
hydration/server factories remain refused until their returned-value behavior
is audited. An import declared by `solid-js`'s own factory does not borrow the
signals factory's witness.

Why that is the right cut and not a loophole:

- The dialect already models the registered computation as a reactive invocation of
  the export's callback (`createMemo`'s `callback_positions`, cardinality many,
  scheduled), so a later re-run at a read is not a new kind of execution, only the
  engine choosing to pull the same one early.
- The consumer projects a described callable that reads to the `accessor` leaf and
  reads no more of it (`contracts.rs`, `project_return_shape`): what a caller's scope
  has to get right about a memo's accessor is that reading it is a tracked reactive
  read, which is what `accessor` says.
- The read ignores what the accessor is called with (`read` is bound over its
  node), and the computation's arguments are the creating call's, accounted there.
  The creating call's **options** are a separate premise: a read-time recompute
  also calls its `equals` comparator (`core.js`, `recompute`), and that is not
  the registered computation. Certification therefore requires every written
  argument to occupy an undisplaced non-spread slot, and every slot after the
  computation to be a primitive or, at the dialect's options position, a plain
  object literal of primitives. A comparator, accessor, shorthand, spread,
  options binding or missing non-spread fact refuses by name.

The read may **throw** (the memo's own error, or a not-ready signal while an async
computation is pending). The veto (ADR 0036) samples the export and calls each
returned accessor; a throwing sample observes nothing, and a run in which no nested
call completes normally is incomplete, so a claim whose only samples throw is
withheld by name (`veto did not complete`), never satisfied by silence.

### 3. The signal row: the argument condition is what the bytes test

ADR 0146 required every argument of the creating `createSignal` call to be a primitive
by grammar. The audited bytes test less: `createSignal(e, t)` takes the memo path only
for `typeof e === "function"`, and `solid-js`' own `hydratedCreateSignal` and server
`createSignal` make no other test of the first argument (audit
`2026-09-30-solid-2-rc9-reads-rows-for-dialect-primitives.md` § 6: "a first argument
that is not a function reaches `createSignal$1` and nothing else"); `signal(e, t)`
reads `t.equals`, `t.ownedWrite` and `t.unobserved` off the options object, and only
`equals` and `unobserved` can be callbacks. So the condition is discharged at the
creating call by grammar the producer states per written argument (handshake protocol
76):

- `argumentsNotFunctionSyntax`: the argument is a primitive by grammar, an array
  literal or an object literal. It is a fact about the written expression, never about
  a binding: an identifier, a member, a call, a construction, an arrow or function
  expression and a spread are all false.
- `argumentsPlainOptionsSyntax`: the argument is an object literal whose every member
  is a plain `key: value` -- a non-computed identifier, string or numeric key -- of a
  primitive by grammar. No member can then be a function, a spread of an unknown
  object, a shorthand naming a binding, a method, an accessor or a computed key.
- `argumentsNonSpreadSyntax`: each written slot is not a spread and no earlier
  spread displaced it. Missing is unproved. This distinguishes a computation
  argument from a spread that can also supply memo options callbacks.

The census (`inert_signal_arguments_discharged`) accepts a written slot when it is a
primitive by grammar, or it is the first slot and not a function by grammar, or it is
the options slot the dialect names (`unambiguous_options_argument`, index 1 for
`createSignal`) and a plain options literal. Every other slot, a slot the producer
did not state, and every spread refuses by name. `createSignal([])`,
`createSignal(void 0, { ownedWrite: true })` and `createSignal("unknown", {
ownedWrite: true })` are discharged; `createSignal(config.value ?? D)`,
`createSignal(x, options)`, `createSignal(void 0, OWNED_WRITE)` (a module constant
another writer may mutate) and `createSignal(fn)` are not.

### 4. The generator proposes; the census decides

`returns_walk::described_callable_returns` proposes the memo read where a function's
own value-carrying completion is, whole and at depth 0, a call spelled `createMemo` or
`<namespace>.createMemo`, and every completion is one. The spelling only *proposes*: a
local function of that name, an alias and a shadowed import are proposed the same way
and the census withdraws the return by name (the callee does not trace to a dialect
call). A conditional of memos, a bound memo, an `await`, a call of the memo and an
`async` function propose nothing here; ADR 0113's plain proposal stands for them and
the census refuses it. The signal row needs no new proposal: the reading walk of ADR
0146 already proposes it wherever the reactive analysis describes the return as an
accessor.

### 5. The consumer

No consumer code changes. A described callable that reads projects to the `accessor`
leaf, so calling what `createMemo` handed back in a component body is the untracked
read `SC1001` reports, and the same read inside JSX is tracked and clean
(`fixtures/reactive-ir/package-memo-accessor-consumer`, `tsc --noEmit` clean; the
open control keeps its `SC9005`).

## Consequences

- Positive and negative pins, on synthesized transcripts (the native tracer harness
  cannot stand up an audited `solid-js` archive, as ADR 0146 recorded):
  `a_memo_is_witnessed_only_for_a_computed_audited_accessor` (the memo row and the
  signal row never answer for each other's slot; a local `createMemo`, a composed
  dependency's accessor, a literal callable, a creating call outside the export and a
  declaration outside an authenticated dependency each refuse),
  `a_returned_memo_accessor_is_a_whole_call_completion_only` (a conditional of memos,
  a bound-and-mutated memo, a member of a call's result and a mixed return each refuse
  the whole return by name), and the argument-condition table added to
  `an_owned_signal_is_witnessed_only_for_an_inert_audited_accessor`.
- Generation: `a_memo_accessor_is_proposed_only_for_a_whole_creatememo_completion`
  and the corpus fixture `implementation-census-memo-accessors`.
- The producer facts: `TestCallResultSourcesStateNotFunctionAndPlainOptionsSyntax` (24
  cases over the real checker).
- Consumer: `a_described_callable_projects_as_no_return_or_as_an_accessor` and
  `fixtures/reactive-ir/package-memo-accessor-consumer`.
- Handshake protocol 75 -> 76: a rebuilt `bin/solid-typefacts` and new certification
  pins. No existing document's digest moves.

## What still refuses, by name

- **A conditional of memos** (`signal-builders` `filterInstance`, `filterOutInstance`):
  the producer decomposes the conditional into arms and states a literal for none.
- **A memo bound and mutated** (`refs` `resolveElements`: `memo.toArray = …`): the
  producer traces a binding to nothing, and a written-to accessor is not the accessor
  the row describes.
- **A helper's result.** `return helper()` is described only if the helper's own
  certified claim says so, and no composition of a described callable through a
  dependency call exists (ADR 0155 composes a *plain* return; a dependency whose
  `returns` closes over a described callable would need the same obligation, and no
  export of the corpus forwards one: the census counts 0). `createNumericRange`,
  `createMs`, `makeBodyCursor` and `once` stay refused.
- **A tuple element of a composed dependency** (ADR 0157): `createFocusSignal`,
  `createConnectivitySignal`, `createPageVisibility`, `createMediaQuery` and
  `createActiveElement` all need the dependency's claim to be conditional on arguments
  only the dependent's call site fixes. The owner's three questions in ADR 0157 are
  unanswered and nothing here answers them: **5 exports counted, none decided**.
- **A signal created over a binding** (`createSignal(config.value ?? D)`,
  `createSignal(mq.matches)`, `createSignal(target())`), **options held in a module
  constant** (`OWNED_WRITE`) **or with a spread**: the grammar does not prove the
  condition, and a constant object is mutable by whoever holds it.

## Measured

The measurements below predate the lead's archive binding, options-callback and spread
restriction. They are historical branch measurements, not a checkpoint of the
integrated implementation. The stricter implementation must be measured again.

`make primitives-checkpoint`'s host procedure (release binary, the pinned 97
packages and 721 exports, misuse ledger run separately), the checker built at
`bbde7700` and at `bbde7700` plus this ADR, each with its own rebuilt
`bin/solid-typefacts`, three hosts, the same machine and flags:

| host | clean before / after | `recursive-value-shape` wall, exports before / after | packages |
| --- | --- | --- | --- |
| none | 100 / 100 | 221 / 189 | 56 / 56 |
| browser | 100 / 100 | 215 / 181 | 54 / 53 |
| node | 130 / 130 | 240 / 209 | 61 / 61 |

**Clean does not move, and could not.** Every export this ADR reaches has `callbacks`,
`reads` and `creates` open for their own reasons (three other causes each, in the
measurement), so `returns` was never the last cause of one; the sole-blocker count is
unchanged (10 / 10 / 40). What moves is where the `returns` cause sits:

- **19 exports certify `returns`** (host free; the same 19 under each host): `signal-builders`
  `add`, `ceil`, `clamp`, `concat`, `divide`, `float`, `floor`, `int`, `merge`,
  `multiply`, `omit`, `pick`, `power`, `round`, `string`, `substract`, `substring`,
  `template`, and `refs` `resolveFirst`. Each is the first callable certified whose
  return is a memo's accessor, decided against the real `@solidjs/signals` and
  `solid-js` rc.9 archives by the audited-dialect witness and the synthesized veto.
- **16 exports are proposed correctly and stop at the veto** (`veto did not complete`,
  by name): `signal-builders` `capitalize`, `drop`, `dropRight`, `filter`, `filterOut`,
  `get`, `join`, `lowercase`, `map`, `push`, `remove`, `removeItems`, `slice`, `splice`,
  `uppercase`, and `websocket` `createWSMessage`. The sampler's inputs make the memo's
  computation throw (a non-array list, a non-string, a `ws` with no `addEventListener`),
  every sample throws, and a run in which no nested call completes normally is
  incomplete. Under `browser`, `createDevices`, `createPermission` and
  `createPointerList` join them through the signal row (§ 3).
- **`createWSMessage`'s signal row is discharged** by `createSignal(void 0, {
  ownedWrite: true })` under every host; before this ADR it was refused for an options
  object.

Criterion 3 of the checkpoint (misuse reports) reads 4 of 97 packages before and after:
its ledger runs against the compiled-in accepted tier, which this change does not
touch, so it cannot move until a package with every domain closed is admitted. The new
ledger case `signal-builders-ceil-top-level-read` is `tsc`-clean on the misuse and its
correct twin against the published typings and, like every `signal-builders` case, stops
at `SC9005` (`callbacks`, `reads` and `creates` open); the consumer half is pinned by
`fixtures/reactive-ir/package-memo-accessor-consumer`, where the untracked read is
`SC1001`, the tracked twin is clean and the open control keeps `SC9005`.

Coverage: 141 projects, 729 findings before; the one project added is the consumer
fixture, no existing finding moves. Contract corpus: 120 fixtures (+1); four fixtures
that return a bare `createMemo` (`callback-deferred-untracked-chain`,
`callback-slot-derived-store-server`, `forwarded-local-untrack-wrapper`,
`multi-role-callback-parameter`) propose the memo read where they proposed `plain`.
