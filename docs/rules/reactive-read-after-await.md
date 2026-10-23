# reactive-read-after-await

`SC1002` · **error** · violation

A reactive accessor is read after an `await` inside an async computation, where
dependency tracking has already ended.

## What it does

Flags reads of signal accessors, store paths, and props that occur after the first
`await` in an async function passed to a computation (`createMemo`, `createEffect`,
`createProjection`, and friends).

Accessor **calls** are proven by TypeScript-side dominance analysis, which handles
branches, loops, `switch`, and `try`/`finally` precisely (both-branch awaits
dominate; a conditional or looped await does not). Store-path and props **member
reads** are proven against the function's straight-line awaits: an await with no
conditional, logical, loop, switch, or try construct between the function entry
and the expression dominates every later read in the same function body. Props
member reads follow the component's caller classification (see
[strict-read-untracked](strict-read-untracked.md)): proven-static props are not
reactive and stay silent; unprovable ones are reported as **uncertifiable**.
A call after the await to a project function whose body reads an accessor
only on some paths (`await load(); return byUser(posts)`) is
**uncertifiable**: the read is past the await, but whether the callee reads on
this call is not proven.

Both proofs exclude nested closures by default. A read in a getter, setter,
constructor, class body or JSX child written after the await runs when it is
read, constructed or rendered, maybe not on the continuation: uncertifiable. The continuation reaches code
past the awaiting function's body only where the execution model proves it runs
there, and the reads in it are then reported with the same two proofs:

- **A callback the call invokes inline.** A function written directly in an
  argument slot an exact standard-library declaration invokes before it
  returns (`Array`/`ReadonlyArray` `map`, `forEach`, `filter`, `some`, `every`,
  `find`, `findIndex`, `flatMap`, `reduce`, `Set`/`Map`/`NodeList`
  `forEach`, `Array.from`'s mapper, the `Promise` executor), and an
  immediately invoked function expression, runs inside the region it is
  called from, nested ones too. It needs a call after a dominating await (a
  `try` around the await is enough). An `async` callback or IIFE runs its
  prefix the same way: the code up to its first own suspension (its operand
  included) is on the caller's stack, and its tail belongs to the
  promise-flight walk. An empty receiver never invokes its
  callback: the read is reported as where the callback would run. A `sort`
  comparator, a `reduce`/`reduceRight` reducer with no initial value (not
  called under two elements) and a `replace`/`replaceAll` replacer are the
  exception: whether they run depends on the contents or on a match, so a read
  in one is **uncertifiable**.
- **A promise handler.** The callback of `.then`/`.catch`/`.finally` on a
  proven native `Promise` runs from a microtask with no owner. It is in the
  continuation when the call's result is the promise the computation returns
  or awaits (`createMemo(() => pause().then(() => count()))`, `await
  pause().then(...)`, a handler chain). The receiver is proven native by
  `new Promise`, the statics of the global `Promise`, `fetch`, a call of an
  `async` project function or an `async` function literal invoked in place, a
  project function that returns one of these, or a handler call on one; a
  `PromiseLike`, a typed `Promise<T>` parameter or an unknown receiver is not
  proof, since a thenable may call back inline. The proof holds while no visible
  write reaches the Promise intrinsics (ADR 0349). With such a write the handler
  is a proof obligation, never a violation. Two limits keep a handler uncertifiable:
  - *An `async` function returns the chain without awaiting it* (the
    computation itself, or an `async` helper): the function adopts the promise
    in a later microtask. A receiver that may already be settled
    (`Promise.resolve(1)`, `Promise.reject(e)`, an `async` helper, a handler
    chain on one) queues the handler before that, so no frame of the
    computation is below it and the runtime raises nothing, even on a re-run.
    The handler is a violation only if `await` holds the call (`return await
    ...`), or the receiver is proven still pending: `new Promise` whose
    executor is one standard timer call, a combinator over at least one
    element, a project function returning one of these, or a handler call on
    one. A synchronous computation that returns the chain is not adopted: the
    handler is a violation there.
  - *A handler that runs only on one outcome.* A rejection handler (`.catch`'s
    callback, `.then`'s second argument) runs only if the receiver rejects,
    and a fulfillment handler (`.then`'s first argument) only if it fulfills:
    a read in a rejection handler is **uncertifiable**, and a fulfillment
    handler counts only when the receiver is proven to fulfill. Fulfillment is
    proven for `Promise.resolve` of a plain literal, `Promise.allSettled` and
    `Promise.all`/`race` of proven elements of an array literal, `Promise.any`
    with one proven element, a `new Promise` whose executor is one timer call
    passing its *first* parameter (`(done) => setTimeout(done, 20)`), a
    project function returning one of these, and `then`/`catch`/`finally` of
    one (a rejection recovered by `catch` fulfills; a handler is assumed to
    return normally). A receiver that rejects (`Promise.reject`, an executor
    passing its second parameter, `Promise.all` of one, `then`/`finally` of
    one), or whose outcome is unknown (any other executor, an `async` helper's
    promise), leaves the handler uncertifiable. `.finally` runs on both paths.
- **An `async` helper the computation awaits.** An exactly resolved `async`
  project function whose promise the computation returns or awaits, directly
  or through `Promise.all`/`allSettled` (of calls, or of `items.map(async ...)`
  or `items.map(helper)`), reads after its own first await in the computation's
  continuation, and it is reported with the computation's primitive. A helper
  whose promise is dropped, or called from an event handler, is not. A helper
  an `async` function returns without `await` is adopted one job after the
  call returns (as for a handler chain above), so it counts only when every
  `await` it makes is on a promise proven still pending (`await pause()`); a
  first `await 0`, `await Promise.resolve()` or an unknown value resumes it
  before the adoption, the runtime raises nothing, and its reads are
  **uncertifiable**, as are the reads of every helper it continues into.
  `return await load()`, `await Promise.all([load()])` and a synchronous
  computation returning `load()` are not adopted and stay violations.
- **A synchronous helper's straight-line read.** A call after the await to an
  exactly resolved synchronous project function runs its body on the
  continuation's stack: an accessor call the body makes on every normal
  completion (ADR 0276's completion fact) is a violation at that read. A read
  on some paths, or one reached through a further helper, stays the
  uncertifiable through-call obligation above.
- **An exact object-literal member.** `api.load()`, where `api` is a closed
  `const` object literal (no spread, computed key, accessor, `this`, export or
  escape, and every reference a plain member read that no one writes), names
  the literal's function. It is followed as the named helpers above are (ADR
  0351).

Each of these keeps it from applying:

- a method that does not resolve to the built-in or standard declaration: a
  project-defined or shadowed `map`, `then` or `Promise`, an unresolved or
  package callee;
- an argument that is not the literal function: `rows.map(makeRow(fn))` hands
  the callback to a wrapper; an opaque slot of an exact standard callback is
  an uncertifiable obligation;
- a generator callback, which runs nothing when it is called;
- a `.then` whose result is dropped, or a `setTimeout`/`queueMicrotask`
  callback (a deferred callback is not on the continuation's stack).
- a read inside or after `for await` (or after `await using`) is an
  uncertifiable obligation, not a violation: the loop suspends before its body
  and after it (its suspension ends with the iterable), but whether the body
  runs, and its position against an `await` the facts model, is not proven.
  Member reads, callbacks and helpers reached there are obligations too.

**The feasible-path premise.** A read is reported where the code is written to
run it, not where one input makes it dead: a `catch` that only an await
rejecting would reach (`try { await rejecting(); return value() } catch {}`), a
branch on a condition the analysis cannot decide (`if (tick() > 100)`), an empty
receiver of `map`. The runtime stays silent on a run that does not take the
path; the rule does not.

**A source already tracked is exempt** (ADR 0195). A read after the await of
an accessor that the same function already called *on every run, before its
first suspension* is not reported: that earlier call ran inside the tracking
window, so the source is a dependency and the computation re-runs when it
changes. The earlier call must be in the function's own straight-line flow
(not under a branch, logical operator, loop, switch, `try` or optional chain,
and not in a nested function) and must precede every suspension, a
conditional `await` included. Accessor calls are exempted this way, and so are
store-path member reads: a path read in the same straight-line flow before the
first suspension tracked every key along it, so a later read of that path, or of
a prefix of it, is silent (`store.a; await pause(); return store.a`), while
another key or a deeper key is not. Props member reads are not exempt.

**Possibly tracked is uncertifiable, not a violation.** Only a source the
analysis proves nothing before the first suspension can have read is reported.
The segment before the first suspension (the first `await`'s operand included)
may have read the source without a straight-line call of it: through a project
helper or a getter of that name, through an inline callback
(`[0].forEach(() => value())`), or through a call whose body is not visible
(a project function reached through a value, a method of a project object).
That read made the source a dependency, so the read after the await is an
uncertifiable obligation. Code outside the project is opaque too: a package
import, a global, a function declared without a body, and any standard-library
call outside the audited inert rows (`inert_standard_call` in `pre_await.rs`:
`Math.*`, timers, number and URI functions, `Date.now`, `fetch`, error,
collection and `RegExp` constructors, `Array.isArray`). A row is inert only for
the values it receives: its receiver is the built-in itself (a shadowed `Math`,
or a subclass or Proxy inheriting the signature, is not), no argument is a
spread, each slot that coerces or iterates (`Math.max(x)`, `parseInt(x)`,
`new Map(x)`, `fetch(url, init)`, `new Error(message, options)`) holds a
source-proven primitive, and each queued callback slot (`setTimeout`,
`queueMicrotask`, `new Promise(executor)`) holds a proven function. A type
annotation or assertion is not a value proof, and a primitive-valued operator
is not inert by its result (`delete proxy.key`). A call is primitive by its
result when its standard function returns a primitive (`Math.random()`), or when
it reads a signal the program only ever writes with numeric literals and the
compiler types as a number (ADR 0350); the call's own evaluation is judged on
its own. A standard call that is not an
inert row is opaque, never a project member of the same property name
(`stats.max` beside `Math.max`). A handler on a
native receiver is inert, and `Promise.all`, `allSettled`, `race` and `any` over an
array literal of native promises and primitives are inert while the
array-iteration premise holds (ADR 0349). Other handlers and combinators are
opaque. A coercion whose operand holds the function's own suspension
(`(await f()) + 1`) runs after the function resumed, so it is not a pre-await
read (ADR 0348). An object, spread or opaque slot stays
possibly tracked. A callee may invoke a callback registered earlier
(`window.dispatchEvent`, `fn.call()`, a package `invoke()`) or read a source a
package store holds, and nothing says it does not. A call whose declaration the
type facts did not resolve is judged the same way. A conditional read in
the computation's own flow does not hide the finding (the run that skips it
warns, the feasible-path premise). A callback of `untrack` or `runWithOwner`
runs with tracking off and is not part of the segment. A store key is a
dependency by its path only when the same path was read; a key another path
may share (`{ a: item, b: item }`, `store.a.n` before and `store.b.n` after) is
uncertifiable when a read before the await used a key of that name or anything
opaque, and a violation when none did (path text equality is not dependency
identity).

**The preceding chain decides settlement.** A fulfillment handler (and a
`.finally` handler) is on the flight only when the receiver is proven to
fulfill (settle): a handler that returns a rejected, never-settling or
unproven value, or contains `throw` or a call that never returns (a `never`
signature, an assertion function), leaves everything after it unproven; other
calls are assumed to return;
`setTimeout(done, ms, value)` resolves with `value`, so only a plain literal
fulfills (a promise there is adopted); `Promise.allSettled` fulfills only over
elements proven to settle. `undefined`, `NaN` and `Infinity` are plain values
only as the global: a binding of that name (`const undefined =
Promise.reject()`) is resolved as the binding it is. Anything else is
uncertifiable.

**A disposed owner is silent.** A computation inside a `createRoot` whose
`dispose` parameter the code mentions (called, passed to a timer, returned) may
be disposed before it resumes, and a read in a disposed computation does not
warn: uncertifiable. The root callback is a literal or the project function the
argument exactly resolves to (`createRoot(start)`); an argument that does not
resolve, written in the computation's own component, leaves the owner possibly
disposed. A root that never mentions its `dispose` keeps the violation.

**Owners.** `untrack(() => load())` with an `async` helper keeps the helper's
reads before its first await out of the window (tracking is off), so its read
after the await is a violation. `runWithOwner(null, () => read())` runs with no
owner (the check returns only for a context that is not `null`): a violation
with a literal `null`, silent for `undefined` (the global, not a shadowing binding),
uncertifiable for any owner the analysis cannot see through (a captured
`getOwner()` is live and the runtime is silent). `untrack(load)` and
`runWithOwner(null, async () => ...)` hand the computation that function's
promise: an `async` one is a member whose read after its own await is a
violation (a non-null owner: uncertifiable); a function the analysis cannot
resolve is an uncertifiable obligation.

**Exceptional continuations.** A catch/finally following a try-head await
is visited even without normal-flow dominance. It is a violation only when
the existing operand proof excludes synchronous throws; otherwise it is an
obligation. Code following the handler's join is an obligation too. A try
with statements before its first await still needs handler-region facts.

**The runtime agrees from rc.13.** `@solidjs/signals@2.0.0-rc.13` warns
`UNTRACKED_READ_AFTER_AWAIT` in dev for a read after the first `await` of an
async computation. It is silent inside `untrack`, before the first `await`,
for a source already a dependency, in a plain async function, in an async
effect apply or tracked effect, and in an action body; the rule is silent in
each of those cases too (the rc.13 review, runtime § 3.4.2). The warning
needs V8 async stack frames and is absent from production builds.

## Why is this bad?

Tracking is synchronous: a computation collects dependencies only until its first
`await`. A read after that point registers no dependency, so the computation never
re-runs when the value changes — the async result is permanently stale with respect
to that input.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const profile = createMemo(async () => {
  const posts = await fetchPosts();
  // Tracking ended at the await: changing userId() never re-runs this memo.
  return posts.filter((post) => post.author === userId());
});
```

Examples of **correct** code for this rule:

```tsx
const profile = createMemo(async () => {
  // Read every reactive input before the first await…
  const id = userId();
  const posts = await fetchPosts();
  // …and use the captured value afterwards.
  return posts.filter((post) => post.author === id);
});

// Or split the post-await dependency into its own synchronous computation:
const posts = createMemo(() => fetchPosts());
const profile = createMemo(() => posts().filter((post) => post.author === userId()));
```

## How to fix

Read reactive values before the first `await` and carry the results through the
async work. If a value must stay live after the `await`, split the read into its
own synchronous computation and compose the two.

## Limits

- **Each shape only adds reports.** A mistake in one shape can only add a false positive, so each has a runtime-oracle case: `after-await-settlement`, `after-await-tracked`, `after-await-disposed` and `after-await-owner` pin the reports. The other `after-await-*` cases (`after-await-quiet`, `after-await-possible`, `after-await-promises`, `after-await-adopted`) pin the silent shapes and the reads the checker leaves uncertifiable.
- **Not modeled:** `race` and `any` of helpers, a promise held in a variable and handled later, member helpers outside a closed literal (a class, a spread or exported object; ADR 0351), an async IIFE the computation awaits, a `Promise.all` over a `.map` result (not an array literal), an accessor alias, and props already tracked. The open items are in `docs/open-work.md`.

## Related

- [strict-read-untracked](strict-read-untracked.md) — the synchronous variant
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — consuming async computations
