# async-outside-loading-boundary

`SC5003` · **warning** by default · violation

An async accessor is rendered with no `<Loading>` boundary above it.

## What it does

Flags tracked JSX reads that require a `<Loading>` boundary but have none. The
ordinary async variant is informational. A source that declares
`ssrSource: "client"` with no first-paint value is an **error** when the project
server-renders, or uncertifiable when no server entry is visible.

Only a *render* of the pending value is flagged. A read inside a `createMemo`,
a `createEffect` compute function or a derived signal propagates the pending
state to whichever render effect consumes it, and the boundary question belongs
to that consumer (a user-written `createRenderEffect` is a render). The
boundary may be any number of component call sites above the read. The finding
is a proven violation only when every render chain was followed to a
`render()`/`hydrate()` root without meeting a boundary; a chain that ends at a
function nothing in the project renders (an exported component, a page handed
to a router) is **uncertifiable**, because the boundary may be written where
the analysis cannot see. A read written in what a component tag receives is uncertifiable too:
a prop is a getter, so the child decides whether a render ever reads it (ADR 0641). A control-flow
tag runs it in a render, and stays proven.

A read in a named local function (`const wrapUse = () => { user(); }`) is
rendered where a call of it sits in rendered JSX, through up to four such
functions; the finding sits at that call, uncertifiable (ADR 0462). A call in a named function no named-callback role
places (a closure in a `<For>` child handed to a timer, or never called) is not
rendered by the JSX around it (ADR 0564). A function run in place in a tracked JSX expression of a flow row (`<For>{() => <p>{(() => use())()}</p>}</For>`) is rendered there, as outside a flow row (ADR 0834). A closure handed on from a child callback (`<Show when={1}>{(_u) => { h(() => use()); … }}</Show>`) is not rendered by the element around it: uncertifiable, never this rule's violation (ADR 0836). A read in a callback a
standard-library carrier may run zero times (`{[].map(() => user())}`, an
unknown-length receiver) is uncertifiable (ADR 0563). An exact `const` alias of the accessor (`const read =
user`, `const { read } = { read: user }`) is the accessor (ADR 0463).

A parent's or module-scope memo read by a component not proven to mount in
the pass that created it (a later `<Show>`, `<Switch>` or non-literal `<For>`)
may be settled at the read: uncertifiable (ADR 0570).

Where a project module awaits at its top level (or an unseen load may), the
finding is **uncertifiable**: the checkpoint may settle a module-scope source
before the read first runs (ADR 0518). Premise: no installed dependency awaits at its top level.

It is the static counterpart of Solid's dev-mode `ASYNC_OUTSIDE_LOADING_BOUNDARY`
warning, informational like it.

**Declared first paint exception.** A source that provably declares
`loadingValue` (or `seedLoadingValue: true` on a store-family source) is never
flagged: it is born committed, its first flight suspends no reader, and the
runtime warning does not fire (`solid-js@2.0.0-rc.0`, verified). If the options
argument cannot be read statically (a spread, a variable), the bare render is
**uncertifiable**, never a violation (ADR 0430; the same for the SSR variant
below and SC5001). Pass the options as an object literal.

A read of a source not proved synchronous that no producer places or rules out is uncertifiable, blocker `async-read-unaccounted` (ADR 0838), never silent; one an SC9012 row holds (a value no row follows to the read) has this rule's own row, blocker `pending-read-unresolved` (ADR 0926); a read in a `try` whose catch swallows its throw is uncertifiable (`async-read-caught`) wherever the function that holds it is run. A source whose `sync` option is not proved `false`, or that starts from an import nothing resolves, is never proved pending: blockers `async-options-opaque` (ADR 0889) and `source-module-unresolved` (ADR 0891), as in [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md).

## Why is this bad?

It isn't wrong, but it may not be what you intended. Without a boundary, the
runtime handles pending async by deferring the mount: the container stays empty (or
keeps its existing content) until every uncaught async value settles, then attaches
atomically. Users see nothing while data loads — no spinner, no skeleton — and a
slow endpoint reads as a hung app.

## Examples

Code this rule flags:

```tsx
const user = createMemo(() => fetchUser(id()));

// Nothing renders until user() settles.
render(() => <Profile user={user()} />, root);
```

Code with explicit fallback UI:

```tsx
render(
  () => (
    <Loading fallback={<Spinner />}>
      <Profile user={user()} />
    </Loading>
  ),
  root,
);
```

## How to fix

Wrap the reading subtree in `<Loading fallback={...}>` when you want visible
fallback UI or partial progressive mount. Leave it as is when an empty container
during load is intended (for example over a static shell) — the deferred atomic
mount is the permissive default, not an error.

For a "refreshing…" indicator during revalidation, `<Loading>` is the wrong tool —
once content has rendered, the boundary keeps it visible. Use
`isPending(() => expr)` under the same boundary, or `<Loading on={key}>` to re-show
the fallback on key changes.

## SSR client-source variant

The server never runs a computation marked `ssrSource: "client"`. Without a
declared `loadingValue` or store-family `seedLoadingValue`, the source is a hole
the server cannot fill. Under `<Loading>`, the server flushes the fallback and
hands the position to the client. Outside a boundary, Solid throws
`ssrSource: "client" read during SSR outside a <Loading> boundary` rather than
letting the stream hang.

The proof reads the option, not async provenance, so a synchronous client-only
compute is caught too. A named server-rendering or hydrate entry proves the
error; an entry that may sit outside the project leaves it uncertifiable. A
spread or variable options argument gives none: a first paint may be declared.

```tsx
const widget = createMemo(() => measureBrowserThing(), {
  ssrSource: "client",
});

function Dashboard() {
  return (
    <Loading fallback={<WidgetSkeleton />}>
      <div>{widget().label}</div>
    </Loading>
  );
}
```

Alternatively, provide `loadingValue` (or `seedLoadingValue: true`) so the
server has a provisional value to render.

## Related

- [pending-async-unsuspendable-read](pending-async-unsuspendable-read.md) — untracked pending reads, which do throw
- [missing-owner](missing-owner.md) — boundaries need owners

A violation needs two proofs: the source is pending, and the read's first run is inside
a `render()` call, which is the only place the runtime checks (`web.dev.js:536-557`). A
read a condition may delay, or a component mounted later, is uncertifiable, blocker
`root-mount-pass-unproven` (ADR 0661). A source really pending at a later mount is quiet.

A named function a carrier provably runs at a tracked position reads there, with the carrier's cover: `[1].map(hf)` with no boundary is proven, `[].map(hf)` reads nothing, and a carrier that may not run leaves it uncertifiable (ADR 0776). A read no fact places, a read at an anonymous function in a tracked position, and a call of a value that may hold a pending source (an alias, a literal member, a hook's value) are uncertifiable; a value that reaches only settled sources stays clean (ADRs 0774, 0775, 0779).

A source no fact proves async, a `lazy` tag, and a read in a method nothing places are uncertifiable, blocker `async-provenance-unproven` for the first two (ADRs 0665-0667). So is a source whose compute reads what it was handed (`createMemo(() => props.get())`) or names an async source in a function form: it is pending while that is, and a settled caller is not followed yet (ADR 0802). A hook's tuple slot is the accessor it names, so its read is proved or certified by that source (ADR 0801). A `Loading` covers what it holds, not what sits beside it (ADR 0668).

A flow element that is no JSX child and no component's return may never render, and rc.21 is silent then: its read is uncertifiable, blocker `carrier-run-unproven` (ADR 0892).
