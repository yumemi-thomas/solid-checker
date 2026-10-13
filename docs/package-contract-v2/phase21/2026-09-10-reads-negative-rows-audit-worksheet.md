# `reads` negative rows: the audit worksheet

- **Status:** partly executed. The fifteen mechanical rows shipped on
  2026-09-10; the five needing a judgement are in `WITHHELD` with their reason.
  `Solid1x`'s table is unchanged and needs the same pass.
- **Date:** 2026-09-10.
- **Unblocked by:** `semantic-model.md` § reads **[Decision 2026-09-10]** — a
  proxy property access is this export's read only when the export owns the
  proxy. That decision removed the convention conflict recorded in the
  [admission review](2026-09-10-reads-census-admission-review.md) § 2.1, so the
  audited documents now conform as written and their `reads` closures become
  *derivable* rows.
- **Why a worksheet and not a patch:** the table's completeness invariant.

## 1. The invariant that forbids a partial patch

`negative_rows_match_the_audited_documents` (solid_2.rs:2529) asserts

~~~rust
assert_eq!(shipped, expected, "the shipped table is not the derivable table minus the withholdings");
~~~

where `derivable` is recomputed from the audited documents for every domain in

~~~rust
assert_eq!(admitted, BTreeSet::from([CallClaimDomain::Creates]),
    "a new domain was added to the table without widening this comparison");
~~~

So admitting `reads` is not "append the rows we are sure of". Every export whose
audited document closes `reads` empty must be **either shipped or explicitly
withheld with a reason**, and the counts (`from_json.len()`, `shipped.len()`)
move with it. That is the table working as designed.

## 2. The derivable set: 20 canonical exports

Computed from the bundled documents — `reads` in `closed` with an empty
collection, agreeing across every artifact case, minus the five spellings that
are not canonical 2.0 primitives (`isEqual`, `applyRef`, `renderToString`,
`renderToStream`, `createServerReference`, which could not be rows either way).

| archive | exports |
| --- | --- |
| `@solidjs/signals` (8) | `action`, `createMemo`, `createOptimistic`, `createOptimisticStore`, `createTrackedEffect`, `flush`, `onSettled`, `reconcile` |
| `@solidjs/web` (5) | `clientOnly`, `httpHeader`, `httpStatus`, `hydrate`, `render` |
| `solid-js` (7) | `For`, `Match`, `Repeat`, `Show`, `affects`, `createEffect`, `refresh` |

Not derivable, and why: `createProjection` (1 item), `createStore` (3),
`snapshot` (1), `latest` (1) close `reads` **with** items — a positive claim, not
a denial. `Loading` and `isPending` leave `reads` open at one item, which is
partial positive.

## 3. What each row still needs a human to decide

A Summary citation makes the bytes the claim, so most of these are mechanical.
Four questions are not, and each is the `reads` restatement of a judgement the
`creates` audit already had to make.

- **Condition splits (`createEffect`).** Its `creates` row was *withdrawn*
  because `dist/server.js:868` routes to `serverEffect` → `processResult` →
  `ctx.serialize`, and a `(package, export, domain)` row carries no condition.
  The `reads` question is the same shape and unanswered: does the server body
  observe a source's current value on any reachable path? If yes, withhold for
  the same reason.
- **`hydrate` and `render`.** `hydrate`'s `creates` row is withheld because it
  reaches `render`, which registers a delegated root. That argument is about a
  `create`. For `reads`, `render` itself closes `reads: []`, so the reach does
  not carry a read — which means `hydrate` may be admissible for `reads` where
  it is withheld for `creates`. Needs stating rather than inferring.
- **`flush` and `action`.** `flush()` runs the scheduler, and effects that run
  under it read. Those reads are not the caller's callbacks and not `flush`'s
  own observation of a value; § reads' "including a read this call schedules to
  a later `at` event" clause has to be applied deliberately here. `action` has
  the transition analogue.
- **Components (`For`, `Match`, `Repeat`, `Show`, `clientOnly`).** Admissible
  under the 2026-09-10 decision — their only argument-path atoms are
  parameter-rooted (`arg0.keyed`, `cb:arg0.children`, `arg1.lazy`), which the
  decision assigns to the caller. Worth naming in the row comment, because a
  reader who has not seen the decision will read the guard as a contradiction.

Solid 1.x has not been enumerated here; its table needs the same pass.

## 4. Then, and only then

Rows are gate 1. Two gates remain before a `reads` closure can certify, and
both are recorded in the [admission review](2026-09-10-reads-census-admission-review.md):

- **Gate 2**, a reviewed veto observation. Designable from public API alone —
  hand the export instrumented accessors as its arguments and observe whether it
  calls them — and partial in the way `creates`' globalThis observation is
  partial. Unreviewed, so `reviewed_observation("reads")` still returns `None`
  and every candidate withholds for want of a recipe.
- **Gate 3**, `ClaimDomain::Reads` in `PROPOSABLE` and in
  `require_census_decides_closure`. Small, and deliberately last: adding it
  before gates 1 and 2 produces proposals that can only withhold.

The census predicate itself needs no new machinery under the decision. ADR
0034/0040 already disposition the parameter-rooted receiver, and ADR 0044's
"a value this program built" already covers the owned side — an object
literal's members are data properties, and a proxy obtained by calling a Solid
primitive arrives as a *call* the existing walk enumerates.

## 5. Executed (2026-09-10): fifteen shipped, five withheld

`Solid2::NEGATIVE_ROWS` now carries **43 rows — 28 `creates` and 15 `reads`**.
Each `reads` row cites the *same* audited summary and byte range as its
`creates` twin, because the closure it restates is in the same bytes; nothing
new was read, and no Implementation-cited row was cloned (those cite a human
reading about `creates` specifically).

Shipped: `createMemo`, `createOptimistic`, `createOptimisticStore`,
`createTrackedEffect`, `onSettled`, `reconcile` (`@solidjs/signals`);
`clientOnly`, `httpHeader`, `httpStatus` (`@solidjs/web`); `For`, `Match`,
`Repeat`, `Show`, `affects`, `refresh` (`solid-js`).

Withheld, each with its reason in `NEGATIVE_ROWS`' doc comment:
`action`, `flush`, `hydrate`, `render`, `createEffect`.

The completeness invariant now reads `from_json = 45` (25 `creates` closures +
20 `reads`), `shipped = 43`, and `admitted = {Creates, Reads}`. Two other
assertions moved with it, both deliberately rather than to go green:
`the_negative_authority_answers_only_denials_and_silence` now asserts that
`createTrackedEffect` **does** answer `Reads`, that the other six domains still
answer silence, and that a *withheld* derivable row (`action`, `flush`) still
answers silence.

**Rows are the terminator half only.** `reads` is not in
`ClaimDomain::PROPOSABLE` and `reviewed_observation` has no `reads` entry, so
no proposal reaches a census and no consumer verdict changes: coverage compares
94 projects and 547 findings unchanged. `reads` is still closed for 0 of 8950
corpus exports.

## 6. The five read against the pinned bytes (2026-09-10)

Performed against `rust/target/tsc-oracle/v2/node_modules`, which holds
`solid-js@2.0.0-rc.3`, `@solidjs/signals@2.0.0-rc.3` and
`@solidjs/web@2.0.0-rc.3` — the exact audited versions. Verified byte-exact
before reading anything: `@solidjs/signals/dist/prod/core/owner.js` hashes to
`d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7`, the
`file_sha256` `RC3_CORE_PRIMITIVES_AUDIT` already pins.

**Result: one export resolved, four collapsed into a single model question.**

### 6.1 `createEffect` — withheld, and the counter-example is on the *browser* side

The worksheet guessed the server path. The server path is **clean**:
`server.js:810` `serverEffect` and `processResult` touch only caller-supplied
values (`options`, and `compute`'s return, which ADR 0048 roots at the caller),
object literals this code built (`comp`), and SSR plumbing
(`sharedConfig.context`, the owner, `ctx[SLOTS]`). `ctx.serialize` — the reach
that withdrew the `creates` row — serializes a promise and observes no source.
No tracking primitive is reachable: `server.js:887`'s `untrack(read)` is inside
`createOptimistic`, not on this path.

The counter-example is in the **client** build. `solid.js` routes
`createEffect` → `hydratedCreateEffect` → `hydratedEffect`, and under
`sharedConfig.hydrating` with `options.ssrSource === "client"`:

~~~js
function withHydrationGate(create) {
  const [hydrated, setHydrated] = createSignal$1(false, { ownedWrite: true });
  const result = create(hydrated);
  setHydrated(true);
  return result;
}
// hydratedEffect:
withHydrationGate(hydrated => coreFn(prev => {
  if (!hydrated()) return prev;   // <- read of a signal this export created
  ...
}, ...));
~~~

`hydrated` is a signal **this export owns**, and the compute reads it. § reads
counts "a read this call schedules to a later `at` event", and ADR 0007's rule
stands: a guarded reach is still a reach, and a `(package, export, domain)` row
cannot say "except under hydration". **Withheld — now for a cited reason rather
than an open one.**

### 6.2 `render`, `hydrate`, `flush`, `action` — one question, not four

The bytes are unambiguous and identical in shape:

- `flush()` (`core/scheduler.js:722`) drains `globalQueue`, running queued
  computations, which read their sources. `flush(fn)` runs a caller-supplied
  callable, already excluded.
- `action(e)` wraps the caller's generator, opens a transition, and
  `schedule()`s the same drain.
- `render` (`web.js`) calls `code()` (caller-supplied), `flatten(tree)` and
  `insert(…, () => tree, …)` over the caller's tree (ADR 0048), reads
  `element.firstChild` and `options.*` — none of them a source — and then
  `flush()`.
- `hydrate` reaches `render`.

So all four reduce to: **does a scheduling primitive own the reads of
computations it drains?** That is a model question of exactly the class
§ reads' **[Decision 2026-09-10]** answered for props, and it decides all four
at once.

**Recommendation: no — attribute the read to whoever registered the
computation.** Three reasons:

1. **Consistency with the audit already relied on.** `createEffect`'s closure
   is `reads: []` while its `initial-compute` is `tracking: tracked`, which the
   [census plan](2026-09-03-implementation-census-plan.md) § 3.2 says "is
   coherent only under this rule" — the compute's reads are its caller's. A
   drained computation is a *third party's* callable, further from the draining
   export's own act than a caller's callback is.
2. **The alternative propagates.** If `flush` can never close `reads`, neither
   can `render`, `hydrate`, or any export that drains — and that reaches well
   past these four.
3. **No consumer can use it.** "This export reads something" with unknown
   sources belonging to other contracts discharges no proof obligation; the
   rules read `reads` for parameter and reactive-resource inputs, and a drained
   computation supplies neither.

The counter-argument, stated fairly: § reads already counts "a read this call
schedules to a later `at` event", and draining causes the read more directly
than scheduling does. The distinction being proposed is *authorship*, not
timing — the same distinction ADR 0034 draws — and it should be written as
such.

If that decision is taken, `flush`, `action`, `render` and `hydrate` ship and
`reads` reaches **19 of 20** rows, with `createEffect` withheld on § 6.1's
evidence.

## 7. Shipped (2026-09-10): nineteen of twenty

The § 6.2 recommendation was taken. `semantic-model.md` § reads carries a
second **[Decision 2026-09-10]** — *the exclusion is about authorship, not
timing* — and `flush`, `action`, `render` and `hydrate` shipped on it.

`Solid2::NEGATIVE_ROWS` now carries **47 rows: 28 `creates` and 19 `reads`**:

~~~
action createMemo createOptimistic createOptimisticStore createTrackedEffect
flush onSettled reconcile | clientOnly httpHeader httpStatus hydrate render |
For Match Repeat Show affects refresh
~~~

`flush` and `action` clone their `creates` twins' citations. `render` and
`hydrate` are the only rows authored from scratch — neither has a `creates`
row, so their byte ranges were computed fresh
(`solidjs-web.json` 16033..21942 and 13513..15951) with the offset method
first checked against an existing citation, which it reproduced exactly.

**Withheld: `createEffect` for `reads`**, on § 6.1's client-build
counter-example. That is the twentieth row, and it is the correct answer rather
than an open one.

The decision also changed a test's meaning rather than its outcome:
`the_negative_authority_answers_only_denials_and_silence` previously asserted
that `action` and `flush` answer *silence* for `Reads`; it now asserts they
**answer**, and says why.

### Still true after all of it

`reads` remains absent from `ClaimDomain::PROPOSABLE`, and
`reviewed_observation` still has no `reads` entry. No proposal reaches a
census, no consumer verdict changes — coverage compares 94 projects and 547
findings unchanged — and `reads` is still closed for **0 of 8950** corpus
exports. Gate 1 is done for Solid 2; gates 2 and 3 are not, and Solid 1.x's
table has not had this pass.
