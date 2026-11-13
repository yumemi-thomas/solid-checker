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
and the expression dominates every later read in the same function body. The
object of a written member (`s.a` in `s.a.v = 1`) is such a read (ADR 0612). Props
member reads follow the component's caller classification (see
[strict-read-untracked](strict-read-untracked.md)): proven-static props are not
reactive and stay silent; unprovable ones are reported as **uncertifiable**.
A read that may follow an await a branch can skip (`if (f) await p;`, `&&`,
`switch`, a destructuring default), directly or in a closure, IIFE or helper
run there, is **uncertifiable**: position cannot tell an arm that leaves from
one that falls through (ADR 0870). A helper is walked once per set of
windows, so a helper shared by two computations reports for each (ADR 0871),
and one called past the computation's first suspension adds no tracking
window (ADR 0872). An async function handed to a tracked primitive through a
project wrapper's parameter (`(f) => createMemo(f)`) is walked as that
primitive's compute; a hand-off the walk cannot follow is **uncertifiable**,
`function-run-unplaced`, except `f();` as a statement in a synchronous
wrapper, which runs in place (ADR 0873). A call past a suspension whose
callee names no symbol and resolves to no declaration (`fns[0]()`,
`(c ? a : b)()`) is **uncertifiable** (ADR 0879). On the flight, a handler the
walk cannot read (`then(api.fmt)`), an `async` or generator handler, an async
function a handler calls, and the handlers of a `.then` with no exact
standard declaration (an `any` receiver) are **uncertifiable** (ADRs
0880-0882).
A call after the await to a project function whose body reads an accessor
only on some paths (`await load(); return byUser(posts)`) is
**uncertifiable**: the read is past the await, but whether the callee reads on
this call is not proven.

Both proofs exclude nested closures by default. The continuation reaches code
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
  promise-flight walk. The carrier must provably run the callback (ADR
  0566, ADR 0563's predicate): an array literal receiver with an element
  (`[1].forEach(cb)`), `Array.from` of one, `.call`/`.apply`, `Reflect.apply`,
  the `Promise` executor and a tagged template do; a receiver that may be
  empty (an awaited or prop array, `[]`, a `Set` or `Map`, a `NodeList`) may
  run it zero times, so a read in it is **uncertifiable**. A `sort`
  comparator, a `reduce`/`reduceRight` reducer with no initial value (not
  called under two elements) and a `replace`/`replaceAll` replacer are the
  exception: whether they run depends on the contents or on a match, so a read
  in one is **uncertifiable**.
- A promise chain past six links (ADR 0750) is **uncertifiable**, `search-depth-limit`.
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
  is a proof obligation, never a violation. So is a handler of a `.then` on a
  receiver nothing proves native and no premise claims (a call's result, an
  identity function's: batch 11), whether written in place, handed by name or
  taken from a closed object's member, and the receiver's own chain is walked. Two limits keep a handler uncertifiable:
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
- **A helper's callback (ADR 0485).** A function literal a project helper calls
  in its own synchronous body on every completion (`run(() => count())`) runs on
  the continuation's stack: its reads after the computation's `await` are
  reported. A helper that keeps or hands on the callback is not claimed.
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
  completion (ADR 0276's completion fact) is a violation at that read, and so
  is a store or prop member read in its body, at any depth of helpers, up to the
  helper's own first await (ADR 0614). An accessor read on some paths, or one reached
  through a further helper, stays the uncertifiable through-call obligation above.
- **A receiver holding a `const`.** `thing.useIt.a`, where `thing.useIt` holds
  exactly the store (ADR 0481), reads the store's key `a`, compared with the keys
  read before the await after the root (ADR 0614).
- **An async source.** Read pending it throws before any warning, read settled it
  warns; which one a read past an await meets is not proven, so the finding is
  **uncertifiable** (ADR 0614).
- **An exact object-literal member.** `api.load()`, where `api` is a closed
  `const` object literal (no spread, computed key, accessor, `this`, export or
  escape, and every reference a plain member read that no one writes), names
  the literal's function. It is followed as the named helpers above are (ADR
  0351).

**A hand-off the walk does not follow is an obligation (ADR 0565).** A
function the walk above does not place, handed whole to a project or
unresolved call (`run(hf)`), copied, or written as a member's value, an
object literal's slot or a method (`ho.m = () => …`, `{ m: hf }`), may run on the
continuation's stack where that code calls it: its reads are
**uncertifiable** when it is written after the await, handed on after it, or
(a member's value, a slot or a method) outlives a later await. A function the
computation returns, schedules on a fresh stack or never mentions stays
silent. A same-file wrapper called by its plain name that calls the function
(its parameter) on every completion of its own synchronous body runs it on
this stack (`run(hf)` over `const run = (g) => { g(); }`), so its reads are
reported as a literal's are (ADR 0485); a member wrapper (`api.run(hf)`) or
one in another module stays uncertifiable.

An uncertifiable finding says which proof is missing (ADR 0595): the hand-off
it runs through (`run`, `api.run`, a member or a copy), the carrier that may
run it zero times (`xs.forEach`), a handler that may not run, a possibly
tracked source, a disposable owner, an unplaced suspension. Only a prop read
whose backing is unproven mentions call sites that cannot be enumerated.

Each of these keeps it from applying:

- a method that does not resolve to the built-in or standard declaration: a
  project-defined or shadowed `map`, `then` or `Promise`, an unresolved or
  package callee;
- an argument that is not the literal function: `rows.map(makeRow(fn))` hands
  the callback to a wrapper; an opaque slot of an exact standard callback is
  an uncertifiable obligation;
- a generator callback, which runs nothing when it is called. A generator the
  computation steps runs on its flight: past an `await`, under a `for await`
  written in the computation, the read is proved; past a `yield`, or stepped any
  other way, it is an uncertifiable obligation (`generator-stepped`, ADR 0820),
  and so is a read in a function the body hands on past the `yield` (a slot, an
  assigned member, a forwarded callback, ADR 0850);
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
branch on a condition the analysis cannot decide (`if (tick() > 100)`). A
callback a carrier may run zero times is not on such a path: it is an
obligation (ADR 0566). The runtime stays silent on a run that does not take the
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
runs with tracking off and is not part of the segment; one of `latest`,
`isPending` or `flush(fn)` runs on the continuation under no owner, so a read in
it is (ADR 0944; `createRoot`'s callback has an owner and is silent). A store key is a
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
  const id = userId();
  return posts.filter((post) => post.author === id);
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
- **Not modeled:** `race` and `any` of helpers, a promise held in a variable and handled later, member helpers outside a closed literal (a class, a spread or exported object; ADR 0351), an async IIFE the computation awaits, a `Promise.all` over a `.map` result (not an array literal), and props already tracked. An exact `const` alias of an accessor or store (`const read = count`, `const { read } = { read: count }`) is its source, before the await and after it (ADR 0463). The open items are in `docs/open-work.md`.

This rule is a member of the `reactivity` group key ([ADR 0428](../decisions.md#adr-0428)): `reactivity` in `rule-options.json` or a disable comment reaches it, and `reactive-read-after-await` alone overrides the group.

## Parameter initializers (ADR 0961)

A call written in the async computation's own body after its await runs the parameter initializers of the function it calls,
or of the constructor it constructs, when it omits the argument: on the continuation, with no observer. The walk reads an
accessor call written directly in a default; where an IIFE, a closure, a destructuring default or a constructor's holds a call
or a member access (a function literal's own invocation reads nothing), the call is an uncertifiable row
(`parameter-initializer-unplaced`), never a violation. An argument the call passes keeps the default from running (no row);
`undefined`, no argument at its index and a spread that may hide it run it; a destructuring pattern's defaults run whatever
the argument is. A constructor of a class written in another file is not followed.

## Members run without a call (ADR 0963)

The continuation of an async computation runs a member without a call: a property access runs a getter, an assignment a
setter, a coercion `toString`, `valueOf` or `[Symbol.toPrimitive]`, a `for...of` or a spread `[Symbol.iterator]` and the
`next()` it returns, a Proxy access its trap, `new` a constructor and the class's field initializers, a tagged template its tag.
The walk of the continuation follows calls and runs none of these. A reactive read written where such a member runs it (a
class element, an object literal's accessor or method, a tag function, a function a `new` runs, a helper those call) is an
uncertifiable row of this rule (`function-run-unplaced`; `field-initializer-unplaced` directly in a class field's
initializer), never a violation, while the program holds a tracked async computation and a suspension (an `await`, an
implicit suspension or a `then`, `catch`, `finally` call). Only reads the role engine could not place are asked. A function
handed where nothing follows it is run by a call, and a parameter initializer is the call-site net's (above): not this net's.
The row is coarse: a read in a member stands whatever the continuation touches and whatever the computation tracked before
its first suspension.

A function stored under a key the host reads by itself runs the same way, written as a method, an arrow, a `function`, a name
bound to one or an assigned member (batch 10c): the keys of a coercion (`toString`, `toLocaleString`, `valueOf`, `toJSON`), the
promise protocol (`then`), the iterator protocol (`next`, `return`, `throw`), `handleEvent`, a computed key no literal decides,
the traps of a `new Proxy` handler and the `get` or `set` of a descriptor handed to `Object.defineProperty`,
`Reflect.defineProperty`, `Object.defineProperties` or `Object.create`. A function stored under any other key runs by a call of
its slot, below.

## Related

- [strict-read-untracked](strict-read-untracked.md) — the synchronous variant
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — consuming async computations

The async computation may be named exactly (a named function expression's binding, an object literal's member), and a comma, conditional, logical or assignment callee after an await (`(slot = r)()`) is a read when it always yields one accessor, else an obligation (ADR 0517). An object literal's arrow member names its function even when its text spells `this` (a nested class's `this.m = …`): an arrow binds no `this` of its own (ADR 0581).

A `.then` handler handed by name is followed as a literal is. A parameter called after an await is uncertifiable, blocker `handed-read-after-await` (ADR 0669).

An exact forwarding wrapper chain runs a callback on the continuation only when each whole-argument hand-on and the final parameter call occur directly in the wrapper’s synchronous prefix on every completion (ADR 0895). A deferred, conditional, reassigned or unresolved hand-on keeps `handed-read-after-await` at the reads. Closed literal members holding a function by name are followed too.

## Calls to a function nothing resolves (ADR 0964)

A function the program stored where nothing follows it (an array, a map, an object's list, a variable) and invoked through the
name it was stored under runs on the continuation when the call stands behind the `await`: `callbacks.forEach((c) => c())`,
`const f = list[0]; f()`, `registry.get(k)!()`, `Promise.all(tasks.map((t) => t()))`, `bus.emit()` over the listeners an
object's method iterates. The call resolves to no function the walk knows, so a source read in the stored function is untracked.
The call is an uncertifiable row of this rule (`unresolved-invocation`; `function-run-unplaced` in a handed async function), never
a violation: nothing here proves what was stored or that it reads a source. The callee is a parameter, a variable, or the result of
a call that is no primitive; a `function` declaration, a `const` bound to a function literal, an accessor, a setter, an action, a typed
candidate (ADR 0294, its own SC9012 row) and a primitive are not this. A method a member route runs holds the invocation
inside it, so the row stands at that invocation, under the same region premise as members (a tracked async computation and a
suspension). A parameter whose caller hands a function the walk follows stays a violation (ADR 0669), and the row at the parameter's
call stands beside it. The row is coarse: it stands whatever the stored value is (a clock a caller injects, a testing library's
destructured query). A call before the `await` is a dependency of the computation, and an event handler is no continuation: no row.

The same row stands where the call names a stored value another way (batch 10c): a member the project declares as a slot with no
function body (`bus.cb()`, `holder.cb?.()`, a function-typed field, an interface method an object implements by a function
value), `f.call(…)` and `f.apply(…)` on a stored value, a `let` the program writes again (its initializer is not the value), a
`new` of a stored value (the row starts at the `new` keyword, where V8's frame stands) and a tagged template whose tag is a
stored value. A class or an object method, a function declaration, a closed object literal's member and a global's or an import's
member (`Math.max.apply`) are not this. The rows only add: no row the rule filed before is dropped or replaced by one of them.

## Accounting (ADR 0973)

The producers above walk from a tracked async computation, so a read or a call written after an `await` in any other async function (an event handler, a helper no computation calls, a test callback) had no row and no record of why. `after_await_accounting.rs` lists those sites from facts alone and ends each one: every read of the checker's reactive-source universe (an accessor call, a store's or props' first `Get`) and every call that runs a project function (an identifier whose name still holds it, a module namespace's member, a method of a `const` object literal or of an exact class instance, and, with its row kept, one an override or a replacement may stand in for: `this.helper()`, a receiver typed as a class, a `let` the program reassigns) or of a function value no exact declaration resolves (a parameter, a stored callback), written directly in an `async` function after one of that function's own suspensions (an `await`, a `for await`, a loop that repeats one), outside dependency source. A function a reviewed test runner schedules is a function like any other (owner ruling 9b). A `.then` continuation, a callback a later microtask runs, a read in a nested function (that function's own site) and a tagged template's tag are not sites.

A site ends in a row of this rule that stands at exactly its span, an SC9012 there, the compiler's discard of the code, or one of two typed reasons; a row of another rule (SC1001's, at the same read) ends nothing. The reasons:

- **The callee reads nothing.** In a program that holds a tracked async computation (the member-route net above runs there), the call's callee is a project function the lookup resolves exactly and its whole transitive closure reads no reactive source: every call in it resolves exactly to a project function again, and it holds no read of the universe, no generator, no JSX, no tagged template, no construct and no primitive call. A call that reaches the standard library, a package, a function value, loops back (`cycle`) or leads through a chain of more than 64 functions (`blocked:depth`, never memoized) is not silenced. A run no call shows (a getter, a coercion, an iterator, a Proxy trap) is the member-route net's above: the read written in such code has a row of its own. Where no computation is recognized the net is off, so the call keeps its row (a start the producers' index does not find may be there).
- **The site never runs** (`SiteReach`: after an unconditional exit, in an arm a literal test rules out, in a function nothing enters).

A site no ending holds gets an **uncertifiable row of this rule, blocker `read-after-await-unaccounted`**: never a violation, never a silence. The mutation test of each reason is `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>` (`after_await_accounting.account.callee-reads-nothing`, `.never-runs`, or `all`): the sites it ended then each carry the row (`after_await_accounting_process`). Left out on purpose, with the walks' own exits unnamed: a role silence (a handler, a deferred callback), a read the walk's pre-await window already tracked, and a read inside an `untrack` callback each need a premise and a Chrome twin of their own (`docs/open-work.md`).

**There is no census reason.** "The program holds no tracked async computation" ends no site: the census is only as complete as the producers' index of computations, and Chrome shows starts the index does not find: a function declaration handed by identifier to a project wrapper that gives it to `createMemo` (ADR 0873 follows a literal and an arrow, not a declaration), `createMemo.call(undefined, work)`, `Reflect.apply(createMemo, undefined, [work])` and `[createMemo][0](work)`. In each, rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read after the await, and the accounting keeps the uncertifiable row (the oracle cases `after-await-universe-census-*`, the fixture `after-await-universe-census`).
