# ADR 0165: The reads census walks calls

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners: the implementation census
  (`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`:
  `census_reads_domain`, the shared walk's `CensusRun::domain`,
  `census_call_disposition`, `census_form_disposition`,
  `census_transcript_calls`, `census_dependency_claim`) and dependency receipt
  composition (`dependencies.rs`, `dependency_reads_claims`)
- Relation: amends `semantic-model.md` § reads (**[Decision 2026-09-29]**), the
  owner's decision of the same day. It corrects the census behind ADR 0101 and
  ADR 0107, which certified `reads: []` over forms alone, and reuses the
  `creates` census's walk (ADR 0008) and its dialect (ADR 0007), dependency
  (ADR 0036 § composition) and cycle (`LocalRecursionBackedge`) dispositions.
  ADR 0146 keeps what a returned callable reads.

## Context

`semantic-model.md` § reads said a read arises "from non-call syntax", and the
`reads` census did exactly that: it dispositioned the export's uncensused
invoking forms and no call ("this census dispositions no call and recurses into
no declaration"). A read made by calling something was therefore invisible to
it. That covers calling a signal accessor, a memo, or a helper or dependency
export that calls one.

ADR 0163's synthesized veto found the first case on its first run.
`@solid-primitives/date@3.0.0-next.3`'s `createCountdown(a, b)` builds
`difference = createTimeDifference(a, b)[0]` and then calls `difference()`, a
memo the call created, at the call. The census passed its `reads: []`; the veto
saw the dependency and the row refused. Every other `reads: []` of that shape
rested on a finite sample alone.

## Decision

1. **The domain.** `reads` covers every tracked read the call performs, call-made
   reads included. A read of a signal or memo the call itself creates is a read.
   Two things stay outside it:
   - the caller's own callable invoked: that is the `callbacks` domain's item,
     whose `tracking` and `owner` fields say how it runs;
   - the body of a returned callable nothing calls during the call. Whoever
     calls it performs those reads, under the returned value's described claim
     (ADR 0146).

2. **The walk.** After its forms, `census_reads_domain` runs the shared call
   walk with `CensusRun::domain = Reads`. Every call reachable in the export's
   synchronous execution, nested literals included, is refused unless it is one
   of these:
   - a call of the **censused export's own** parameter (`parameter-rooted`),
     the caller's own argument;
   - a standard-library member by identity (ADR 0149), as the `creates` walk
     admits it;
   - a dialect primitive whose audited negative row closes **`reads`** (the
     tier's `Reads` rows, e.g. rc.9 `createMemo`, `flush`, `onSettled`);
   - a dependency export whose own `reads` is closed and empty. The claim is
     named at the site under `census-dependency-reads:`, and composition
     discharges it against that dependency's receipt, exactly as `creates`
     does;
   - a same-package declaration, walked the same way with the same premises.
     A back edge to a frame already on the stack closes the cycle: it
     re-enters bodies whose every other edge is already dispositioned, and
     re-entering cannot add a read;
   - a call inside a literal that *is* a live return expression
     (`ReturnSite::callable`, or an arm's). Such a literal is referenced
     nowhere else, so nothing in this call can run it. The site is recorded
     as `census-reads-returned-literal-call:`.

3. **A nested frame's parameter is not the caller's.** In the `reads` walk, a
   call of a parameter of a frame at depth > 0 refuses. So does a form whose
   subject a caller-provenance derivation roots at one. A call site in this
   artifact filled that parameter, perhaps with an accessor or a store this
   very call created. The `creates` and `callbacks` walks are unchanged.

4. **What is unchanged.** The forms half, ADR 0101's described enumeration,
   ADR 0107's own-result demand, and ADR 0163's veto. No producer fact was
   needed, so the protocol is unchanged.

## Consequences

- `fixtures/package-contracts/reads-call-walk` pins the behaviour:
  - it closes `callsNothing`, `callsPureHelper`, `invokesArgument`, `callsStandardLibrary`,
    `returnsReader` and `mutualRecursion`;
  - it refuses `readsCreatedAccessor` (`createCountdown`'s shape),
    `callsReadingHelper` and `readingCycle` by the walk's name;
  - the graph test composes a dependency export whose `reads` is closed and
    refuses the same call when the dependency's `reads` is open.
- A dialect primitive with no audited `reads` row now refuses `reads: []` for
  every export that calls it, whatever the primitive does. rc.9 has no such row
  for `createSignal`, `createRoot`, `getOwner`, `onCleanup`, `untrack` or
  `runWithOwner`. The rows are the audits' to add, one at a time. This ADR does
  not infer them.
- Measured withdrawals are recorded below.

## Remaining approximations

- A standard-library call is admitted by identity alone, and its arguments are
  not traced. `Object.keys(store)` or `JSON.stringify(store)` on a store the
  export reached neither by parameter nor by a refused call is a whole-object
  observation this census does not see. A store the export imports from its
  own module's top level is such a store.
- A call inside a returned *object's* method, or inside a literal returned
  through a binding (`carriedCallables`), is walked, not skipped. That is
  conservative.

## Open finding: `onElementConnect`'s `returns` (not fixed here)

`@solid-primitives/lifecycle@1.0.0-next.2`'s `onElementConnect(el, fn)` is:

    if (isServer) return;
    if (el.isConnected) return fn();
    …; onCleanup(…)

Its proposed `returns` claims only "the result of invoking argument 1". It
leaves out the `undefined` completions: the early `return;`, and the fall-off
after the `ResizeObserver` setup. Under `node`, `@solidjs/web@2.0.0-rc.9`'s
server build sets `isServer = true`, the early return runs, and ADR 0115's
synthesized veto contradicts the claim. The row refuses under `node`.

The proposal is also host-blind: the reused-proposal lane certifies one
proposal on every host. Under `browser` the sample throws at `ResizeObserver`
before the fall-off, so the veto never sees that `undefined`. The generator's
`returns` proposal for a bare `return;` beside a value return is the defect.
Its fix belongs to the `returns` generator, not to this ADR.

## Measured (2026-09-29)

The census is `make contract-coverage-census` over the pinned 16-package
corpus, host-free, with the release binary, at the same base (`6c66c825`) with
and without this change. The pin (`benchmarks/ecosystem/coverage-census.json`)
was met by the base run and regresses by exactly the withdrawals below, so it is
re-pinned in this commit.

| Bucket (demanded sites) | Base | With the walk |
| --- | --- | --- |
| an operation is stated | 699 | 699 |
| determined: states nothing | 331 | 253 |
| degenerate: nothing determined | 122 | 200 |
| import finds open: some uses | 180 | 147 |
| import finds open: every import | 278 | 311 |

**Withdrawn claims: 47 export rows** (44 distinct exports across the corpus
packages that define them, and three re-exports through `@kobalte/core`). Each
was `reads` closed at the base and is open now, refused by this ADR's walk:

- `@solid-primitives/utils` (24 rows): `accessArray`, `contrastRatio`,
  `createCallbackStack`, `createHydratableSignal`, `createHydrateSignal`,
  `createMicrotask`, `darken`, `desaturate`, `fill`, `filterOut`,
  `isValidColor`, `lines`, `normalizeColor`, `parseColor`, `push`, `remove`,
  `removeItems`, `shallowCopy`, `sort`, `splice`, `tryParseColor`,
  `withAccess`, `withArrayCopy`, `withCopy`.
- `@solid-primitives/rootless` (5): `createBranch`, `createDisposable`,
  `createHydratableSingletonRoot`, `createSharedRoot`, `createSubRoot`.
- `@solid-primitives/scheduled` (5): `createScheduled`, `debounce`, `leading`,
  `scheduleIdle`, `throttle`.
- `@solid-primitives/memo` (4): `createLazyMemo`, `createPureReaction`,
  `createReducer`, `createWritableMemo`.
- `@solid-primitives/trigger` (3): `TriggerCache`, `createTrigger`,
  `createTriggerCache`.
- `@kobalte/core` (3, composed from `@solid-primitives/utils`): `isValidColor`,
  `normalizeColor`, `tryParseColor`.
- `@solid-primitives/storage` (2): `messageSync`, `multiplexSync`.
- `@solid-primitives/timer` (1): `createTimeoutLoop`.

The refusals, by the walk's own reason, are all fail-closed and none is a proven
read. Of the 84 new `reads` withheld-closure records (one per package and
export; a package that composes `@solid-primitives/utils` repeats its exports):

1. A resolved callee that is neither a standard-library member by identity, a
   dialect primitive with an audited `reads` row, nor a declaration this
   artifact owns (31). The named callees include `createSignal`, `onCleanup` and
   `createComponent`, which rc.9 has no `reads` row for.
2. An uncensused invoking form the walk reaches in a same-package callee (19),
   for example `property-access-unknown-accessor` in `accessArray`.
3. A same-package declaration whose own walk refuses (9: `createHydrateSignal`,
   `createSharedRoot`, `createBranch`, ...), an unresolved callee (8), and a call
   through a nested frame's parameter (8: `cb`, `mapConstructor`, `subscriber`).
4. A standard-library member the producer does not state by identity
   (`Set.has`, `Array.push`, `CallableFunction.bind`: 3) and a callee with no
   function-like declaration node (`parse`, `subscribe`: 3). Both are the
   `creates` walk's existing admissions, inherited unchanged.
5. The three `@kobalte/core` rows are the inherited closure: the parent's
   `reads` is no longer the projection of `@solid-primitives/utils`'s.

A refusal by (3) on a helper that merely forwards the caller's own callback, such
as `withCopy` and `withArrayCopy`, withdraws a claim that is probably true. That
is this ADR's price for not attributing a nested frame's parameter to the
caller, and it is the first place to spend a producer fact if the sites matter.

One row moved the other way, `@kobalte/core` `RTL_LANGS` (a non-callable value),
and it is noise: the same export name is published by several artifact cases and
the retained-tree read order differs between two runs.

The primitives checkpoint (`make primitives-checkpoint`, 97 packages, the same
base binary and the same tree, run before and after):

| Host | Clean, base | Clean, with the walk | partial -> degenerate |
| --- | --- | --- | --- |
| none | 90 | 99 | 42 |
| browser | 90 | 99 | 36 |
| node | 100 | 100 | 46 |

(721 exports; per-host clean counts from `measure*.json`.) No export left
`clean`. The nine that entered are `@solid-primitives/date`'s constants (`DAY`,
`DEFAULT_MESSAGES`, `HOUR`, `MINUTE`, `MONTH`, `SECOND`, `UNITS`, `WEEK`, `YEAR`)
under `none` and `browser`, and they are not a gain of the walk's own: at the base
that package was refused whole under those two hosts
(`witness-acquisition: probe contradiction`, ADR 0163's veto seeing the memo
`createCountdown` reads), while under `node` it already certified. The walk
refuses `reads: []` for `createCountdown`, `createDate` and
`createTimeDifference` at the census (`callee in solid-js`, the cause the
checkpoint records), so the veto never sees a closure it would contradict and the
package certifies under every host. The 42, 36 and 46 exports that went from
`partial` to `degenerate` are exports of the same corpus that lost their `reads`
closure to the walk; the checkpoint counts exports per package across all 97,
so the figure is not the census's 47 rows.
