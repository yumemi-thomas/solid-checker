# strict-read-untracked

`SC1001` · **warning** · violation

A reactive value (signal accessor, store path, or component prop) is read in a scope
that does not track dependencies.

## What it does

Flags reads of reactive values that happen outside every tracking scope: at the top
level of a component body, inside an effect's apply phase, in a plain helper
function called from an untracked position, or through a chain of calls that ends
in one of those places. The finding's evidence trail shows where the value was
declared and how the read reached the untracked scope.

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
  callback and is not reported.
- **An accessor passed by reference is read where the callee calls it.**
  `run(count)` reads `count` during the call when `run` is a project function
  whose own synchronous body calls that parameter, so in a component body it is
  reported as `run(() => count())` is. A callee that calls the parameter only
  from a timer callback, a closure it returns, or an `async` body, or that only
  stores it, is not proven to read it during the call, and nothing is reported
  (`fixtures/reactive-ir/accessor-passed-by-reference`).
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
  proof obligation rather than a proven runtime warning.

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

## Related

- [no-destructure](no-destructure.md) — the destructuring special case
- [reactive-read-after-await](reactive-read-after-await.md) — reads that lose tracking at an `await`
- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — the async variant, which throws
