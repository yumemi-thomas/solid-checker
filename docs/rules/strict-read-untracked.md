# strict-read-untracked

`SC1001` · **warning** · violation

A reactive value (signal accessor, store path, or component prop) is read in a scope
that does not track dependencies.

Like every projected violation, this finding assumes
[standard runtime configuration](../decisions.md#adr-0266).
A positively identified visible configuration use makes it uncertifiable,
with the veto site related. Missing reference evidence does not veto this
conditional premise.

A whole store is read by more than its members (ADRs 0620, 0621, measured on
rc.21): `deep(s)`, `JSON.stringify(s)`, `Reflect.get(s, k)`, `Array.from(s)` and
`${s}` on any store; `for…of`, an array or call spread and array destructuring on
an array; `{...s}`, `Object.values`, `Object.entries` and `Object.assign` on an
object that holds a key. A store of unknown shape is uncertifiable there, and
`Object.keys`, `in`, `hasOwn`, `for…in` and `snapshot` read nothing on a settled store.
A `merge` view's Get lands on its last source that holds the key: a key a literal
serves reads nothing (ADR 0630). A store over a value no literal states may be its
raw value where a project uses `markRaw` or a shallow store (ADR 0634).

`untrack` scopes the wrapped read: a Get of a store proxy it returns warns
(`const o = untrack(() => s.o); o.x`), proven where the store's literal initial value
holds an object or array there (ADR 0629). A labelled `untrack(fn, "name")` opens a
window wherever it runs (ADR 0628).

A read through an escaped reactive value carries an SC9012 at the read site
(ADR 0860), with the blocker from its escape: a carrier, logical expression,
unresolved member, primitive callback return or forwarded value. A proven
tracked role discharges that read. A package-installed global function gets
`dependency-global-read` at its call (ADR 0862). These rows prove no violation.

A value a package the build does not read as source returned (ADR 0981) is read as possibly
reactive: the result of a call or `new` into such a package, a member of one, or a binding that holds
one, read by a member get, a call, a destructuring pattern, an iteration or an object spread where nothing
tracks or places the read, is **uncertifiable** (`package-value-read:untracked` or `:unplaced`), never a
violation. A package whose manifest names no Solid package is read as source nonetheless when its entry
closure imports one (a package that forgot its peer): the same read of its value is a violation or
silent by its source. `docs/open-work.md` names what would win the rows back. A call the reviewed reading of a package shows throws on every call returns no value, so no read of its result stands ([ADR 1063](../decisions.md#adr-1063): `@stylexjs/stylex` 0.19.0's `create`, `defineVars` and their kin).

Such a value is followed forward through the program as well ([ADR 0998](../decisions.md#adr-0998)): a binding, a pattern,
an assignment, a slot of a literal, a return, a parameter of a project function, a standard method's result, an export read in
another module. Each call or member get through it where nothing tracks or places the read is **uncertifiable**
(`package-value-read:flowed`), and so is a position inside the project the walk cannot follow (`package-value-read:value-in-class-field`,
`:value-through-generator`, `:value-thrown`, `:value-forwarded`, `:value-use-unclassified`). A value handed on as a named JSX
attribute is followed where the tag names its reader ([ADR 1002](../decisions.md#adr-1002)): a project component's reads of that prop (`props.view.search`, a
pattern's slot) and each `useContext` of a context provider. A value handed to a package, a constructor, a tag nobody resolved, a
spread or a child of a component, a Solid primitive or an `await` is not followed there (`docs/open-work.md`).

## What it does

Flags reads of reactive values that happen outside every tracking scope: at the top
level of a component body, inside an effect's apply phase, in a plain helper
function called from an untracked position, or through a chain of calls that ends
in one of those places, for a memo's accessor as for a signal's (ADR 0486). The
finding's evidence trail shows where the value was declared and how the read
reached the untracked scope.

An apply read remains untracked even when the same effect's compute reads the
same source. In that case the effect can re-run when the source changes, so the
message describes the untracked read and Solid's development warning without
claiming the effect never updates (ADR 0301). Finding kind and evidence stay the
same.

This is the static counterpart of Solid's dev-mode `STRICT_READ_UNTRACKED` warning,
and it mirrors where the runtime actually installs strict-read contexts (component
and effect bodies). Two consequences, both probed against `solid-js@2.0.0-rc.0`:

- **Every read form takes one role.** A member read and a spread take the role an
  accessor's call takes at the same place: an intrinsic `ref` the body applies
  (ADR 0520), a carrier that runs a named function (ADR 0531), a hand-off nothing
  follows (an obligation), a layer a summary climbs (ADR 0522; all ADR 0614).
  A store's member read in a helper the body calls is the body's read whatever
  makes the store (a `createStore` slot, a projection a primitive or a hook returns
  whole) and whatever TypeScript wraps the receiver (`as`, `!`, `satisfies`;
  ADR 1121).
  A tuple held whole (`const st = createStore(…)`) is no store: only `st[0].k` is
  its Get; `st[0]` alone, `st[1]` or a dynamic index is uncertifiable, since what the
  element is handed to is not followed (ADR 1138).
  A function a primitive names by its binding (`createMemo(cb)`, `onClick={cb}`,
  `onSettled(cb)`) takes that position's role only when every reference to it is
  a position, a resolved call or a `const` alias accounted the same way; any other
  (`[cb].forEach((g) => g())`, a global store, a returned object) leaves the read
  uncertifiable (ADR 1119).
- **Plain assignment targets are writes, not reads.** `store.name = next` is
  diagnosed by `no-store-mutation-outside-setter` without also reporting `store.name` here.
  Only the member that *is* the written target is exempt: read-modify-write
  forms such as `store.count += 1` and `store.count++` keep the read finding
  because the old value is evaluated, and so do reads nested inside a target —
  the computed key in `rows[props.index].done = true` and the default in
  `({ name: local = fallback.name } = incoming)` are both evaluated to perform
  the write.

- **Module scope is not reported.** A module-scope read runs before any component
  exists; the runtime opens no strict-read window there and never warns. A
  deliberate module-scope snapshot is legal, undiagnosed Solid, so this rule stays
  silent for it. (The `v1/` catalog keeps reporting module scope, matching
  eslint-plugin-solid.)
- **A host-scheduled callback is not the body.** A callback a component body
  hands to `setTimeout`, `setInterval`, `queueMicrotask`, `Promise.then`/
  `catch`/`finally`, `requestAnimationFrame`, `requestIdleCallback` or an
  observer constructor runs from a task or microtask queue after the body
  returned, with no strict-read window open, so a read there is not reported
  (probed on rc.3 and rc.9). The scheduler is matched by its standard-library
  declaration, not its name.
- **A function kept in a collection is uncertifiable** (`Set.add`, `push`,
  `Map.set`: project code may call it anywhere), and a `For` row or `Show` child
  created in a closed `untrack` reads in its own window (ADR 0927).
- **A host-retained callback is uncertifiable.** An `addEventListener`
  listener (on any DOM interface), a `Function.prototype.bind` bound argument, a
  `PromiseLike.then` callback and a Geolocation callback written in a component
  body run wherever the host invokes them: inside the body's strict-read window
  when the body dispatches synchronously (`dispatchEvent`, `el.click()`), calls
  the bound function, or hands the callback to a synchronous thenable, and after
  it otherwise (probed on rc.3 and rc.9: the first raises
  `STRICT_READ_UNTRACKED`, the second nothing). A read there is reported as
  **uncertifiable**, not as a violation, and a pending async read there the same
  way under `pending-async-unsuspendable-read`. The checker does not prove a
  synchronous dispatch in the body reaches the listener, so even that case stays
  uncertifiable. A JSX event handler (`onClick={...}`) is the compiler's event
  callback. Visible native synchronous dispatch withholds its execution proof
  (ADR 0356), a named handler's too (ADR 0934), so its reads are then
  uncertifiable; otherwise it is not reported.
  A dispatch on the global `window` (or `document` on web rc.17), or written in
  a scheduler callback or at module top level, does not withhold it (ADR 0435);
  one in a `runWithOwner(null, …)` callback does, because that leaves the
  enclosing strict-read window open. A dispatch in an effect apply (ADR 0639) or in a
  function only events invoke (ADR 0437) keeps the proof only while no listener of the
  program may sit in a shadow tree ([ADR 1115](../decisions.md#adr-1115)): a listener
  there runs with `window.event` unset, inside the window. Any `attachShadow`,
  `shadowRoot`, declarative shadow markup, `getRootNode`, `composedPath` or custom
  element, code evaluated from a string, or a load of a module the program does not
  analyse withholds it. A navigation delivers only the event types the host
  fact names ([ADR 1080](../decisions.md#adr-1080): `history.go` and `location.reload`
  at most `beforeunload`, `location.assign`/`replace` also `popstate` for a fragment):
  with no listener for them anywhere the program is read (an `on<type>` property, a
  `body`/`html` handler, an `addEventListener` whose type is unnamed or one of them),
  it delivers to none and withholds nothing. A dispatch in a callback a reviewed test
  runner registers (ADR 0438, an async one included, or a helper only such
  callbacks call, in a closed program, exported or not when every other
  module's reference to it is a call, and a call past an `await` in an async
  helper included; a literal handed to such a helper is one, ADR 0442), or
  written in a function only events invoke (ADR 0437), does not withhold it
  either, and a read in a registered test callback is placed outside every
  strict-read window, as in a scheduled callback. A helper handed a literal
  (`fireAndFlush(() => el.dispatchEvent(...))`) places the literal where the
  helper's call runs it; the same helper also called from a component body
  keeps the veto, and the literal's read is SC1001 there.
- **A map function runs untracked, and may run unwarned.** The map function of
  `mapArray(list, map)` and `repeat(count, map)` runs under `runWithOwner`, which
  sets `tracking = false`, so a read written directly in it (`item => item + tag()`)
  subscribes nothing and goes stale. Only a call that passes `name` opens a
  strict-read window around it; unnamed, Solid raises nothing, and the finding's
  hint says so instead of promising the dev warning (ADR 0443). JSX the map
  function returns tracks its own expressions, `untrack(() => tag())` is the
  explicit snapshot, and a function it only returns runs when its caller does.
  The list or count thunk is a tracked computation. The index accessor of a
  default `mapArray` is not a source here, so `index()` is not reported
  (`fixtures/reactive-ir/mapper-untracked-reads`).
- **A context provider reads `value` once.** In Solid 2 the context itself is the
  provider (`<Ctx value={count()}>`); its function reads `value` under the
  `<provider>` window and keeps a snapshot, so a child's `useContext` never sees
  a later write. A read written directly in `value` is a violation when the tag
  resolves to the exact `createContext` binding (also one imported from another
  file); a function value is stored, not run, and stays unplaced. A prop in
  `value` follows its component's callers like any prop read
  (`fixtures/reactive-ir/provider-value-untracked`).
- **A closure a tracked scope builds is not run by it.** A read in a closure a
  memo or effect compute, or a tracked JSX expression, returns, stores or names
  (`const f = () => count(); return f`) is tracked only where a call runs the
  closure in the scope's own pass; rc.17 warns when the component body calls it
  through the memo's value. Such a read is uncertifiable, and an IIFE or a name
  every reference of which is a tracked call keeps the tracked role (ADR 0452;
  `fixtures/reactive-ir/read-closure-in-tracked-scope`).
- **`runWithOwner` clears tracking inside a computation.** It sets `tracking =
  false` around its callback and keeps the window the call had, so in an effect
  or memo compute, or a JSX expression, a read in the callback subscribes nothing
  and raises nothing: reported, with the no-warning hint. In a component body it
  reports as the body does. Not reported: a callback that only creates a
  primitive (its own compute tracks), `untrack` around the call or the read, JSX
  the callback writes, a call after an await the compute reaches (that is
  [reactive-read-after-await](reactive-read-after-await.md)'s, also through an
  immediately invoked function), and a call `tsc` rejects. `createRoot` reports
  the same (ADR 0932).
- **A setter's updater runs on the setter's stack** unless the guard throws
  first; `ownedWrite: true` skips it (ADR 0932).
- **A read an await may precede is uncertifiable.** Past an unconditional
  `await` the window is closed and the read is silent. Past a conditional
  (`if (c) await p`, `x ??= await p`, `f?.(await p)`, a labelled `break`), a
  loop that awaits, or a `try` whose block awaits (its `catch` and `finally`),
  the path that takes the await runs the read after the suspension, so the
  role is unknown: uncertifiable, never reported (ADR 0354). A loop body that
  reads before its await is in that set too, since a later iteration follows
  the await. The other arm of the same `if` or `?:` does not follow it, and an
  await in a nested closure belongs to that closure. A destructuring default
  runs after the value it destructures, so it follows that value's await. A
  helper called at such a site is uncertifiable at the call. An in-place
  layer counts the awaits before its call (ADR 0938).
- **Named hand-offs (ADR 0531).** Resolved synchronous standard-library carriers,
  template tags and Function.call/apply place a named reader at the carrier's
  role. Other hand-offs are obligations; reviewed fresh-stack callbacks stay legal.
- **A Get that only leads to a write is uncertifiable in a dynamic-only body.**
  In `flush(fn)` in a body only `dynamic()` mounts, `s.a` in `s.a.v = 7` stands
  for no read of its own: the same write in the body is uncertifiable, so the
  path claims no more (ADR 0694). A real read there stays a violation.
- **A callback forwarded to a callee that calls it runs in place** (ADR 0931).
- **A read no fact places is uncertifiable.** Code in a component's element
  children, in a helper with no resolved caller, or in a closure a tracked
  scope only stores, or a call in a compute's returned function (ADR 0931)
  has no execution role; a read there is reported as
  **uncertifiable**, never dropped. A read past the strict-read window (after
  an unconditional `await`) or in several non-reporting roles (ADR 0323) is
  proven silent.
- **A rendered function's body is a component body, however else it is
  called.** `const P2 = Plain; <P2 />` renders `Plain` as `<Plain />` would,
  so a body read in `Plain` is a violation even beside a tracked `{Plain()}`
  or a handler position (`onClick={Plain}`). An alias of a binding that is
  ever written renders nothing proven.
  A visible call clears a function only when every other reference is
  followed (ADR 0331): one passed, stored, aliased by a written `let`, or
  exported from an open program leaves its body reads uncertifiable, beside
  whatever its calls prove.
- **A function nothing renders is no component body (ADR 0445).** A dead
  or lowercase JSX-returning function, and a callback or IIFE evaluated at
  module scope, leave body reads uncertifiable; an exported capitalized
  function is a component by module surface (ADR 0331).
- **A named local handler can forward a callback prop.** A project component
  may invoke a callback prop from a synchronous immutable local arrow bound
  only to intrinsic event attributes or exact component props that themselves
  reach only events. Exact local declaration identities and
  the complete reference census must establish those uses, including when the
  component is imported. Transparent TypeScript wrappers are preserved;
  eager calls, aliases, escapes, reassignment, direct eval, or missing reference
  evidence withhold the proof. This extends the existing inline JSX event
  forwarding rule and gives the callback the existing event execution role.
  It does not classify arbitrary functions by an `on…` name.
- **An accessor passed by reference is read where the callee calls it.**
  `run(count)` reads `count` during the call when `run` is a project function
  whose own synchronous body calls that parameter, so in a component body it is
  reported as `run(() => count())` is. A callee that calls the parameter only
  from a timer callback, a closure it returns, or an `async` body, or that only
  stores it, is not proven to read it during the call, and nothing is reported
  (`fixtures/reactive-ir/accessor-passed-by-reference`).
  A dependency-source summary's exact ambient inline callback invocation exposes
  the same read for `access(count)`. A call-scoped lower bound of at least one
  proves the read; an optional invocation is **uncertifiable**. Neither callback
  closure nor a possible invocation proves that this call invokes the accessor.
  When the argument is a `createSignal`, `createOptimistic` or `createMemo`
  getter, the walk knows it is a function of `length` 0 (ADR 0338), so the
  package's `access(v) = typeof v === "function" && !v.length ? v() : v` calls
  it on every path: `access(count)` in a component body is a violation, and
  the same call on an alias, a function literal, a props value or any value
  whose arity is unknown stays uncertifiable
  (`fixtures/reactive-ir/package-access-helper`).
  The invocation's execution event and trigger must both be the export call;
  a same-stack schedule at a later event is insufficient. Tracked JSX and event
  handlers remain clean; a retained callback, a spread slot, or a non-call protocol item does
  not establish a read during the package call.
- **Closed options callbacks can compose across source helpers.** An arrow
  forwarded in an exact options field is non-strict only when every consumer
  use, sibling field and enclosing callback carrier is accounted for. The
  bounded source proof follows fields into tracked computes, explicit untrack
  and audited fresh-stack scheduling. Mixed eager execution, object escape,
  receiver-capturing sibling methods, callback defaults, cycles and missing
  source keep the read uncertifiable. The proof runs again after cached read
  tables are merged; a consumer-only edit cannot retain an old exemption.
- **`flush(fn)` keeps the caller's read role.** It runs `fn` inline without
  touching the listener, so a read inside it subscribes a memo or effect compute
  that calls it, and in a component body it is reported exactly as a bare read
  there (probed on every published rc.0-rc.9 client build). `untrack` is the
  wrapper that clears the listener. A layer that runs in place inside `fn` (an IIFE)
  answers as `fn` does, and in a helper not proven to run in the body the read is
  uncertifiable (ADR 0859).
- **Props follow their callers.** `devComponent` wraps the body in
  `untrack(() => Comp(props), '<Name>')`, and the warning fires only when a prop
  *getter* reads reactive state in that window. A prop that every visible call
  site passes as a static value compiles to a plain property and can never warn —
  reads of it are not reported. A prop some call site passes a reactive expression
  for (a signal or memo call, a store or props member, a tracked expression) is
  proven signal-backed and reported as a violation. When the component's call
  sites cannot be enumerated — it is exported, spread into (`<Card {...rest}/>`),
  or referenced outside JSX — the finding is reported as **uncertifiable**: a
  proof obligation rather than a proven runtime warning. A `props.x` read keeps
  that classification when the body also passes `props` to an `omit`, to a
  `merge` with another never-falsy source, or to a spread onto a DOM element, or
  binds a `const` alias that does the same: none can write to the object
  (ADR 0337, `fixtures/reactive-ir/omit-view-props-root`). A read through the
  view or alias (`rest.x`, `local.x`), a destructured binding, a `merge` that may
  return its only source, and a spread onto a component stay uncertifiable.
- **A view's member call is the prop's call (ADR 1100).** `merged.r1()` and
  `rest.r1()` are judged as `props.r1()` when the view is a `const` whose rc.21
  Get is the parameter's own: `merge(literal…, props)` with the parameter last
  and each earlier source a closed object literal without the key (the key no
  member of `Object.prototype`), or `omit(props, "k"…)` with string-literal
  keys other than it. That holds only where the prop's own call is placed (in
  the body, or directly in a primitive's callback there); deeper (a closure, a
  helper, an IIFE) the call stays uncertifiable, as `props.r1()` does there. A
  parameter that is not last, an earlier source that is not such a literal,
  a predicate, a computed member, a view of a view, an alias and a shadowed
  `merge` stay uncertifiable (`fixtures/reactive-ir/merge-omit-view-props-call`).
- **An exact `const` alias is its source (ADR 0463).** `const read = count` and
  `const { read } = { read: count }` (a literal of static data properties only)
  hold the module-local `const` `count`, so `read()` is reported where
  `count()` is, for an accessor, a memo, an async memo and a store. A mention
  that may run before the alias's declarator throws there and is
  uncertifiable. A `let`, conditional, parameter, imported or exported alias is
  not followed.
- **A read in JSX a callback writes is tracked where the compiler tracks it (ADR 0470).**
  `createRoot(() => <div>{r()}</div>)`, an attribute, a component prop or
  child in a `createRoot`, `runWithOwner`, `untrack` or handler callback:
  the compiler wraps the expression in `insert`, `effect` or a getter, so the
  read is tracked and silent. A read in the callback's body beside the JSX is
  still reported.
- **A read through layers that run in place is placed where they run (ADR 0469).**
  A read in an IIFE, an inline `forEach` callback or a helper that calls its
  callback, inside a `createRoot` or `runWithOwner` body or an effect apply, is
  read as one written at the outermost layer's call: a violation where that
  is. An await on every path before it makes it legal, a possible await or a
  site in a layer's parameter list uncertifiable; a primitive's callback, a
  stored or handed-on function, a fresh stack or a layer called inside JSX
  ends the climb.
- **A read in such layers belongs to the enclosing function's summary (ADR 0522).**
  `const dir1 = () => { h1(() => { r1(); }); … }` reads at the `h1` call when
  `dir1` is called: a `ref` factory the JSX evaluates eagerly, a statement in
  the body or a hook called there is a violation; a factory in a tracked
  attribute, a timer's callback and a stored callback stay silent. A helper or
  a hook's accessor called in a class member's body is an obligation (ADR
  0516), unless the class is construct-only (ADR 1022: its one `new`, after
  it in the same non-async body and outside JSX, runs its constructor and the
  method called on that `new` there, so the call takes that body's role), as is a read in a generator stepped at its own call inside a function
  an intrinsic `ref` applies.
- **An effect apply reading an async source stays here (ADR 0467).** A run
  that meets the source pending throws `PENDING_ASYNC_UNTRACKED_READ` instead
  of the warning; when the apply first runs against the source's settling is
  not proven, so the untracked read is the claim and the message names the
  pending case (`fixtures/reactive-ir/strict-read-async-apply`). Where the
  first run is proven to meet the source pending (ADRs 0504, 0505), the read is
  [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md)'s
  instead and is not reported here. Where neither outcome is proven (ADR
  0509), both stay uncertifiable at the read, this rule's and that one's
  (ADR 0511): a `<Loading>` that shows its fallback holds a user effect to the
  reveal, where the warning fires instead.
- **An effect's returned cleanup and its error arm.** A read in the cleanup an
  apply returns is uncertifiable: a rerun runs it in the window, a disposal
  outside it (ADR 0539). A compute with no call, no reference and a literal
  return never reruns, and its cleanup is silent. A read in an exact bundle's
  `error` member is silent: rc.17 runs it before the window opens (ADR 0540),
  a `createReaction` bundle's too (ADR 0548).
  An action body's read before its first suspending `yield` runs on the
  stack of the action's call, in that caller's window: called from an effect
  apply or a `createReaction` callback it is reported; from a handler, an
  `error` arm or a leaf scope it is silent. An action whose calls are not all
  followed, or that is also called where nothing proves the run, leaves its
  body's reads obligations (ADR 0547).

- **A creation the effect-callback guard refuses first (ADR 1111).** A read
  written after a creation SC6002 proves refused, or holds as its
  `effect-owner-unproven` obligation, in the same function or in a function
  that function calls by name after the creation (`const f = () => s.v;
  createMemo(f); f();` in an apply), is uncertifiable
  `creation-throws-first`: PRIMITIVE_IN_EFFECT_CALLBACK throws before the
  read. A function handed by name to a primitive and also called by name
  where untracked reads are reported, or where nothing places the call, is
  unplaced (`function-run-unplaced`)
  (`fixtures/reactive-ir/competing-check-read-after-creation`).

- **A named function expression (ADR 1127).** `const f = function inner() {}`
  is entered through `f` as well as through `inner`: a call through either
  is a call of the function, and a read it makes is reported there as for an
  arrow (`fixtures/reactive-ir/named-fn-expr-calls`).

- **An effect compute also called by name (ADR 1130).** A store read in a
  function named as an effect's compute is silent there only when no direct
  call of the function runs where untracked reads are reported or where
  nothing places it; otherwise it is `function-run-unplaced`
  (`fixtures/reactive-ir/effect-compute-also-called`).

A read that no row, discard or named silence ends is uncertifiable: `strict-read-unaccounted` (ADR 0936).

## Why is this bad?

Solid's reactivity is dependency tracking: a read only subscribes when it happens
inside a tracking scope (JSX, a memo, or an effect's compute function). An untracked
read sees the current value exactly once and never re-runs — the UI silently shows
stale data after the first update.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Profile(props) {
  // Top-level read: `name` is captured once and never updates.
  const name = props.name;
  return <h1>{name}</h1>;
}

createEffect(
  () => user(),
  () => {
    // Apply phase runs untracked: this read never subscribes.
    console.log(store.settings.theme);
  },
);
```

Examples of **correct** code for this rule:

```tsx
function Profile(props) {
  // The property access inside JSX is what tracks.
  return <h1>{props.name}</h1>;
}

createEffect(
  // Extract what the apply phase needs in the compute phase.
  () => ({ user: user(), theme: store.settings.theme }),
  ({ user, theme }) => console.log(user, theme),
);

// An intentional one-time snapshot, made explicit:
const initial = untrack(() => count());
```

## How to fix

Move the read into a tracking scope: JSX, a `createMemo`, or the compute function
of `createEffect(compute, apply)`. To observe a whole store in an effect, read
`deep(store)` in the compute phase. If a one-time snapshot is intended, wrap the
read in `untrack()` so the intent is visible to both readers and the analyzer.

For component props specifically, read `props.name` directly in JSX or derive it
with `createMemo(() => props.name)`. `untrack(() => props.name)` is only an
explicit one-time snapshot; it does not make the prop reactive.

## How it is proven

- **Props views.** `omit(props, …)` always builds a view: `omitTraps` in `@solidjs/signals@2.0.0-rc.13` (`dist/dev.js`) returns true from `set` and `deleteProperty`. `merge(...)` returns its one non-falsy source, so it is a view only with no spread argument and an argument that is syntactically never falsy (an object or function literal, or a call that is itself a view). `@solidjs/web`'s `spread` and `ssrElement` only read the object. An element that spreads the props among several sources is a view only when another source is provably never falsy: a non-`ref` attribute, a rendering child, or a member spread (a bare identifier may be `undefined`). A `children` attribute beside a raw child, and a member spread carrying `/*@static*/`, count as no such source, in the native lowering and in `@solidjs/babel-plugin` alike. The view premise needs `Proxy`, since without it `merge` can return its last source.
- **Getter arity.** The rc.13 accessor is `read.bind(null, node)` over `function read(el)` (`dist/dev.js` `accessor`, `dist/prod/signals.js` `accessor`). `solid-js`'s client wrappers return it unchanged, and its server builds return arrows with no parameter. Probed on the dev and prod client builds: `length` 0 and `typeof` `"function"`. The walk folds `p.length` against a numeric literal (`===`, `!==`, `==`, `!=`, `<`, `<=`, `>`, `>=`), and `typeof p`, or a `p.length` that only feeds a test, is no escape, so a branch that cannot run for the arguments is proven absent. A hoisted function declaration's body runs whenever it is called, so an earlier `return` leaves it possible rather than unreachable. A binding declared in a file with a direct `eval` is refused, and a binding the module exports proves no length.
- **Not decided.** A read through another helper hop, an array of accessors (`accessArray`), and a read after an `await` (always an obligation).
- **A helper's call in a `try`** ([ADR 1109](../decisions.md#adr-1109)). A helper's call of its parameter after a statement that may throw, in a `try` whose `catch` completes normally, is not on every completion of the helper: the callback's read is uncertifiable. A store handed through a standard inline callback's parameter (`[store].map((s) => s.v)`) is an SC9012 `value-in-carrier` row, and a callback a carrier provably never runs (a `const` holding `[]` only iterations read, a one-element `sort`) is uncertifiable.

This rule is a member of the `reactivity` group key ([ADR 0428](../decisions.md#adr-0428)): `reactivity` in `rule-options.json` or a disable comment reaches it, and `strict-read-untracked` alone overrides the group.

## Related

- [no-destructure](no-destructure.md) — the destructuring special case
- [reactive-read-after-await](reactive-read-after-await.md) — reads that lose tracking at an `await`
- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — the async variant, which throws

A read in a callback named at its sole `createRoot`/`runWithOwner` position, or in a layer that callback runs in place, is placed at the call as the inline callback is; any other entry is an obligation (ADR 0517). So is a read through `r.call(…)` or `r.bind(…)()` in a closure the body calls, and a read written after a mention that may precede its declarator.

ADRs 0528–0530: eager IIFEs in handler expressions do not inherit the yielded handler's dispatch exemption. Binding-dispatched reads in proven in-place layers retain an undecided enclosing-summary row (an obligation). A closed dynamic-only component entry has the unwindowed role: a stale snapshot, with no promise of a runtime diagnostic; mixed entries stay unproven. Under `dynamicCore`'s unlabelled `untrack`, a read in `flush(fn)` (which keeps the label) is such a snapshot too, also through a closure it calls (ADR 0571).

Placement climbs that exhaust their budget retain an uncertifiable read (ADR 0532): 16 in-place layers, eight generator steps, or eight layers inspected for a pre-suspension window. Outer unread layers cannot supply a deferred or unwindowed role.

A handler's read is proven to run in no window only while no call can deliver it on another stack. `focus`, `blur`, `select`, `remove` and the other calls of ADR 0642 run a listener synchronously, like `click`, so an unplaced one in the program makes the read uncertifiable.

A read in a function only a hidden call runs (a getter, `toString`, an iterator, a tag, a class field, a `Map` slot), directly or through named
helpers, has no call node to place it: uncertifiable (ADR 0673). So is one in a helper handed on from such a member, or from another file
(`h.call(null)`, `[1].map(h)`, a copy, a tag), and a generator's body, a parameter default and a read past an await on some paths below one (ADR 0674). A
named function handed to a carrier that provably runs in a body takes the carrier's role: proved. A setter runs its callback on its stack: a read there, or in a `reconcile` key that is argument 0, takes the setter call's role, except a store's own Gets, which never warn
(ADR 0676). A key on a `reconcile` held elsewhere is uncertifiable, and so is a read in the callback of a setter called in a named helper no named-callback role places, which no summary carries (ADR 0833). Under a scheduler a window-inheriting call reports nothing (ADR 0675). In a named function it answers from where that function runs: its direct calls, or a scheduler that takes it (ADR 0679).

A function every reference of which lies in an `isServer` arm never runs on a client: silent. One only a build constant, a `clientOnly` or resumed `lazy` body by call, or a `<NoHydration>` subtree in a hydrating project reaches is uncertifiable (ADR 0677). `render(Local, el)` calls `Local` with no strict window, so a read there is silent where `<Local />` warns (ADR 0678). A callback handed to a standard method a project subclass overrides is not proven in place: uncertifiable (ADR 0683).

A member tag of a same-file `const` holder ADR 0342 accounts is one of the stored component's call sites (ADR 0823): `<Ui.Card title={label()} />` with `Card` reading `props.title` in its body is proved; an imported or escaping holder keeps the component's callers open.

A named `Show` or `Match` function child with runtime `length` 0 is a value. Its child position proves no untracked invocation; reads remain uncertifiable when their callers are unknown. An inline one runs in the insert's tracked effect only where the element is itself inserted (a child or child expression's value of an intrinsic element or of an inserted fragment, or a proven component's return); held and called elsewhere, its read is uncertifiable.

## Endings by code (ADR 0968)

ADR 1056 preserves tuple-backed read provenance when exact project calls supply every timer callback parameter through a composite callee or a closed literal member. Stored values, rewritten or defaulted callback parameters, receiver escapes and unread member uses retain the tuple-iteration obligation.

A read is accounted for, for this rule's Chrome code, only by an ending that answers that code: a row of its own table, an SC9012, the compiler's discard, a finding of a rule the dialect says claims the code, or a named silence that names it. A row of another table or a finding of another rule at the same place answers another code and ends nothing here, so this rule's own uncertifiable row (`strict-read-unaccounted`) stands beside it: never a violation, and it yields to any row of the rule that stands. A site whose rule claims no code here, or whose code has no policy in the dialect, is not ended by what stands at it.
