# strict-read-untracked

`SC1001` · **warning** · violation

A reactive value (signal accessor, store path, or component prop) is read in a scope
that does not track dependencies.

Like every projected violation, this finding assumes
[standard runtime configuration](../decisions.md#adr-0266).
A positively identified visible configuration use makes it uncertifiable,
with the veto site related. Missing reference evidence does not veto this
conditional premise.

## What it does

Flags reads of reactive values that happen outside every tracking scope: at the top
level of a component body, inside an effect's apply phase, in a plain helper
function called from an untracked position, or through a chain of calls that ends
in one of those places. The finding's evidence trail shows where the value was
declared and how the read reached the untracked scope.

An apply read remains untracked even when the same effect's compute reads the
same source. In that case the effect can re-run when the source changes, so the
message describes the untracked read and Solid's development warning without
claiming the effect never updates (ADR 0301). Finding kind and evidence stay the
same.

This is the static counterpart of Solid's dev-mode `STRICT_READ_UNTRACKED` warning,
and it mirrors where the runtime actually installs strict-read contexts (component
and effect bodies). Two consequences, both probed against `solid-js@2.0.0-rc.0`:

- **Plain assignment targets are writes, not reads.** `store.name = next` is
  diagnosed by `no-direct-mutation` without also reporting `store.name` here.
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
  (ADR 0356), so its reads are then uncertifiable; otherwise it is not reported.
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
  helper called at such a site is uncertifiable at the call.
- **A read no fact places is uncertifiable.** Code in a component's element
  children, in a helper with no resolved caller, or in a closure a tracked
  scope only stores has no execution role; a read there is reported as
  **uncertifiable**, never dropped. A read past the strict-read window (after
  an unconditional `await`) or in several non-reporting roles (ADR 0323) is
  proven silent.
- **A component's body needs every reference accounted for.** A function a
  tag renders, or whose type is an exact `Component`, is a component body only
  when no other reference hands it on (`items().map(Row)`, an alias, a plain
  call; a type in `typeof Row` is no reference at runtime). A read in one that
  is also handed on is **uncertifiable**, even when the tag is reachable, since
  the plain call runs the body in the caller's scope (ADR 0331,
  `fixtures/reactive-ir/component-identity-escapes`). Beside a tag, a use that
  can only render stays proven: a closed object literal only
  `<Dynamic component={icons[k]}/>` reads, or a `const` alias only tags name
  (`fixtures/reactive-ir/component-identity-renders`).
- **`Dynamic` opens no strict-read window.** rc.17's `dynamicCore` runs the
  component under `untrack` with no strict-read label, so a read in a component
  that only `Dynamic` renders (directly, or as a TanStack route component)
  raises no `STRICT_READ_UNTRACKED` and is **uncertifiable** here; a tag that
  also renders it opens the window (ADR 0138, 0334).
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
  wrapper that clears the listener.
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
- **Props spread.** An object spread reads every prop only when its argument, or a value it may take (a conditional's branch, `&&`'s right, `||`/`??`'s operands), is exactly the props binding or an inline view (ADR 0375). `...(props.q === undefined ? {} : { r: props.q })` reads only `props.q`; a branch value or `{...props}` in an object literal stays uncertifiable (oracle `adjudication-props-spread-options`).

## Related

- [no-destructure](no-destructure.md) — the destructuring special case
- [reactive-read-after-await](reactive-read-after-await.md) — reads that lose tracking at an `await`
- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — the async variant, which throws
