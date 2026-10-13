# ADR 0100: A described `callbacks` enumeration the census confirms

- Status: accepted and implemented (2026-09-13); written with the
  implementation
- Date: 2026-09-13
- Owners: generator (`interproc.rs` direct-call rung,
  `ContractExport::direct_callback_parameters`, `inferred_contract.rs`
  `callbacks_enumeration_is_confirmable`), certifier (`type_facts.rs`
  `described_callbacks`, `confirm_described_callbacks`,
  `DirectInvocationSite`), veto synthesis (`synthesized_vetoes.rs`
  `Observation::DescribedCallbacks`)
- Relation: lifts the "empty enumeration only" restriction ADR 0008's shared
  call walk placed on the `callbacks` census when the domain became proposable
  (2026-09-12), for exactly the items that walk already derives. No wire
  change: every fact read here — `calleeParameter`, `captured`,
  `completionForm` — is on the transcript already. Handshake protocol stays 56.

## Context

`callbacks: [] closed` denies that one invocation of the export invokes any
callable it did not define (`semantic-model.md` § callbacks). Since 2026-09-12
the domain is proposable and the implementation census decides it by the same
call walk `creates` runs: a site dispositioned into the parameter-rooted family
is the export running its caller's code, and one such site refutes the empty
enumeration.

That left two populations the census could never close:

- **The refused empty proposals.** The 2026-09-13 pin carries 1,155 `callbacks`
  candidates refused as "enumerates no invocation, but the implementation
  census dispositioned N call(s) into the parameter-rooted family". Measured by
  member the family is 1,266 accessor sites, 710 direct calls, 270 iterations,
  270 elements, 127 coercions, 62 `hasInstance`; 154 entries are direct calls
  and nothing else. These are correct verdicts on false claims
  (`docs/precision-backlog.md`, "`callbacks` refusals measured by member"), and
  not this ADR's lever: the proposal described nothing, and the walk cannot
  supply timing for a getter or an iteration protocol.
- **The described enumerations that never became candidates.** The
  interprocedural pass already writes an `inline` row for `function f(cb) {
  cb() }` — `from` parameter 0, `at` the call event, `schedule: same-stack`,
  `tracking: untracked` — and the generator's proposal filter dropped every
  export with a non-empty enumeration on the ground that "a described
  invocation carries timing, tracking and owner the walk does not derive". On
  the hubs certified standalone these are `access`, `accessWith`, `pipe`,
  `safe`, `withAccess`, `evaluate`, `withCopy`, `withArrayCopy`,
  `withObjectCopy`: the domain stayed *partial* with its one item, and the
  consumer's `unknown_contract_callback_export` kept treating the export as
  one whose callbacks are unknown.

The second population is the one whose item the walk *does* derive. A call
dispositioned `parameter-rooted` in the export's own frame, outside any nested
callable, in a body that completes plainly, runs before the export returns, on
the caller's stack: that is `at: call, schedule: same-stack`, read off the
transcript rather than off the proposal's word.

## Decision

**A `callbacks` enumeration whose every item is an unguarded, untracked
`invoke` `from` a bare parameter `at` the call event on the same stack is
proposable, and the implementation census confirms it site for site: every
`parameter-rooted` site the walk reads must be an item, and every item must
have a site.** Anything else in either direction refuses by name.

### What the generator proposes

The interprocedural pass records, beside each `inline` row, whether the row
came from its last rung alone — a call of the parameter itself written directly
in the body of the function that declares it (`direct_callee &&
call_in_owner_body`). The wire does not carry this: `untrack(cb)` publishes
the same `inline` row as `cb()`, from the dialect's inline callback position,
and only the latter has a site the census walks to. So the fact travels as a
proposal input, `ContractExport::direct_callback_parameters`, in the same
family as `creates_walk_clean` and `returns_walk_clean`: never encoded, never
evidence, empty is "do not propose".

`callbacks_enumeration_is_confirmable` then admits the domain to
`propose_closures` when every item is `from` a bare parameter in that set and
its operation is `invoke`, `at: call`, `same-stack`, `untracked`, unguarded.
A `deferred` row, a `tracked` row (a dependency's invocation, which the walk
sees as the dependency call), a member-rooted row (`options.onChange()`), or an
`inline` row a primitive position produced keeps the enumeration partial and
yields no candidate — exactly what every non-empty enumeration did before.

### What the certifier confirms

`census_callbacks_domain` reads the proposal through `described_callbacks`,
which answers `None` for the empty enumeration (the 2026-09-12 census,
unchanged), the set of described parameter indices when every item has the
shape above, and refuses before any walk runs otherwise — naming the item and
the reason (a member path, a schedule, an execution point, tracking, a guard,
a non-parameter source).

The walk is the shared one. `CensusRun::record_call` keeps, for every
`ParameterRooted` disposition, a `DirectInvocationSite`: the parameter index
and whether the callee is a member of it (from the call's own
`calleeParameter`, not the rebound alias), the frame depth, `captured`, and
the location. `confirm_described_callbacks` then refuses, in this order, on:

1. a root that does not complete plainly (`async`, generator): a call in
   such a body may run after the export has returned;
2. any parameter-rooted member other than the direct call — an accessor, an
   iteration, an element, a coercion, a `hasInstance` — as an invocation the
   enumeration does not describe;
3. a direct site dispositioned through an alias whose parameter the call does
   not itself state;
4. a direct site whose callee is a *member* of the parameter;
5. a direct site read at depth ≥ 1 — a helper's parameter is the helper's,
   not the export's, until ADR 0092's provenance gate is lifted;
6. a direct site that is `captured` — inside a callable nested in the frame,
   whose execution point is not the call event;
7. a direct site whose parameter the enumeration does not name (the
   proposal understates);
8. a described parameter with no site (the proposal overstates: the walk did
   not derive that invocation).

What survives is exactly the enumeration, and the site is
`typefacts-implementation-census:callbacks:described-invocations:<sites>:parameters:<indices>`.
Tracking and owner are not confirmed: a same-stack call inherits both from
its caller, `untracked`/ambient is the generator's existing word for that, and
the census neither asserts nor contradicts it. The closure is a claim about the
*set* of invocations; the items' attributes were already published in the
partial enumeration.

### What the veto observes

`Observation::DescribedCallbacks(mask)` — the described indices as bits, a set
small enough to stay `Copy`; an index the mask cannot hold is not synthesized.
The module renders each callable slot as its own recording callable
(`callbackAt(slot)`) rather than ADR 0036's one shared `callback`, so it knows
which slot ran, and emits `callback-invocation` when a slot outside the set
runs at any time up to the end of the drain, or a slot inside it runs while no
sample call is on the stack. A described slot running inside its sample call is
the described item and observes nothing: the census, not the module, proves it.

The reads fixture's `invokesCallerAccessor` is the tracer, with a hand recipe
of the same shape (`probe-recipes/invokes-caller-accessor.mjs`), and
`a_described_callbacks_closure_reaches_a_receipt_through_its_mandatory_veto`
carries the closure — proposed from the generator's own `expected.json`,
confirmed, vetoed, bound — to a receipt whose document has `callbacks` closed
and non-empty.

## What refuses, and why each is a refusal rather than a skip

- **`plain(items, callback) { callback(0); return mapAll(items, callback) }`**
  (the creates fixture). Proposed — the direct call is there — and refused at
  rule 5: `mapAll` calls its own parameter at depth 1. Honest: the export does
  invoke the callback per element, and the enumeration's one item does not say
  so. Lifting this is ADR 0092's per-argument provenance, not a relaxation here.
- **`conditional-callback-conflict`'s `schedule`.** The development target
  calls `callback()` and proposes; the production target `queueMicrotask`s it,
  has no direct set, and stays partial. One export, two cases, two verdicts,
  each read off its own runtime file.
- **`untrackedWrapper(fn) { return untrack(fn) }`, `memoShape`,
  `renderEffectShape`.** Never proposed: the row is the primitive's, and the
  direct set is empty. Had they been proposed, rule 8 would refuse with "found
  no call of parameter 0", which is true of the walk and misleading about the
  runtime — which is why the generator does not propose them.
- **An `inline` item with a member path** (`options.onChange()`): refused at
  the generator and at rule 4. A member of the caller's object is the caller's
  callable, but a described item for it needs the accessor question answered
  first (does reading `.onChange` run a getter?), and § callbacks lists that
  getter as an invocation of its own.

## Consequences

- No protocol change. The producer already states every fact read here.
- `ContractExport` gains `direct_callback_parameters: BTreeSet<usize>`; the
  interprocedural summarizer threads it per node beside `invoked_parameters`,
  and composite dispatch requires equal sets across candidates as it already
  requires equal callbacks.
- Twenty-four generator fixtures now close `callbacks` with one item for the
  exports that call a parameter directly (`plain`, `direct`, `observe`,
  `mount`, `render`, `deepFactory`, `inert`, `invokesCallerAccessor`, …);
  snapshots regenerated, every changed export inspected to be of that shape.
  The `implementation-census-creates` partition test is unchanged: its five
  new `callbacks` candidates are served by synthesized vetoes and decided by
  the census (`plain`, `noRecipe`, `overloaded` refused at rule 5,
  `readCallerResult`, `readBoundCallerResult` confirmed).
- Corpus effect (release binary, 418 rows, `--timeout 1800`): closure
  candidates 24,912 → 25,468, certified closure entries 9,752 → 9,770,
  uncapped `callbacks` closures 339 → 365, no row below the pin, wall
  1,055 → 1,095 s. Ninety-three new candidates refuse at rule 2 (a getter on a
  parameter beside the direct call: `access`, `wrapSetter`, `getFirstChild`,
  `withArrayCopy`); the rest of the unclosed ones die in the shared walk's own
  refusals before confirmation. The split is in `docs/precision-backlog.md`
  (2026-09-13, ADR 0100 entry).
- **Left open, by size.** The tracked same-stack row (`createMemo`'s
  computation runs before the export returns) is confirmable in principle by
  composing the dependency's accepted `callbacks` item with the argument slot
  the walk already records (`argument_parameters`), and the member-rooted row
  by the accessor census; both are ADRs of their own. The 1,155 refused empty
  proposals stay refused — they are the definition working.
