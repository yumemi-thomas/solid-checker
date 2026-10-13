# ADR 0136: `createComponent(C, props)` is a call edge to `C`

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the dialect fact (`rust/crates/solid-dialect/src/lib.rs`,
  `Dialect::renders_component_argument`, answered in `solid_2.rs`), the render
  sites (`rust/crates/solid-reactive-ir/src/indexes.rs`,
  `SemanticLookup::function_render_call_sites`), and the call graph
  (`rust/crates/solid-reactive-ir/src/attribution.rs`, `reach_from` and
  `compute_entered_only_through_calls`)
- Relation: adds one kind of edge to the call graph that ADR 0135's
  `reachability` rung walks. It adds no rung.

## Context

After ADR 0135, seven of the nine obligations of
`@tanstack/solid-router@2.0.0-rc.8`'s root node that still mark every export
are reached only by rendering. `Matches.js` enters `Transitioner` and
`Rendered` only through `createComponent(Transitioner, {})` and
`createComponent(Rendered, {})`. That is what the compiler writes for
`<Transitioner/>`.

The call graph already treats a JSX tag as a call site of the component it
names. The compiled form is a call of a Solid export, and the graph read its
first argument as a function value escaping into a callee it cannot follow.
So `entered_only_through_calls` answered `false`, and attribution fell to
`fallback-all`.

## The runtime fact

`createComponent` is `solid-js`'s export. `@solidjs/web` re-exports it
unchanged in every build: `export { …, createComponent, … } from 'solid-js'`,
at line 2 of `dist/web.js`, `dist/server.js` and the dev and observe builds of
rc.3 and rc.9. Every build of `solid-js` `2.0.0-rc.0` through `rc.9` was read.
Each calls `Comp` exactly once, synchronously, before it returns:

| build | body | owner `Comp` runs under |
| --- | --- | --- |
| client prod `solid.js` (rc.9 `:1122-1124`, rc.3 `:1095-1097`) | `untrack(() => Comp(props \|\| {}))` | the caller's |
| client dev (rc.3 `dev.js` `devComponent` `:35-53`, rc.9 `solid.dev.js` `observedComponent` `:35-57`) | `createRoot(() => untrack(() => Comp(props)), { transparent: true })` | a transparent child root |
| client observe (rc.8, rc.9 `solid.observe.js:36-46`) | as dev, without the dev checks | as dev |
| server prod `server.js` (rc.9 `:1644-1646`, rc.3 `:1466-1468`), and rc.7/rc.8 server dev and observe | `Comp(props \|\| {})` | the caller's |
| server dev/observe (rc.9 `server.dev.js:1748-1755`, `server.observe.js:1721-1725`) | `runWithOwner(createComponentOwner(…), () => Comp(props \|\| {}))`, or plain with no owner | a child of the caller's |

`createRoot` runs its function in place, through `runWithOwner` on the
caller's stack. The dev builds also store `Comp` on the owner
(`owner._component = { fn: Comp, … }`) and tag it with `$DEVCOMP`. No build of
`solid-js`, `@solidjs/signals` or `@solidjs/web` at rc.3 or rc.9 reads
`_component` back, so neither is a second entry.

This was probed on the published triples, installed from npm. The probe calls
`createComponent` inside a memo, then writes a signal `Comp` read, then
flushes. rc.3's four builds and rc.9's six (prod, dev and observe, client and
server) all behaved the same way:

- `Comp` ran once, between the statements before and after the call;
- it never ran again;
- it ran under the caller's owner in prod, and one owner below it in dev and
  observe.

## Decision

1. **The dialect states the fact.** `Dialect::renders_component_argument(name)`
   returns the argument position that a call of the export declared as `name`
   renders. It defaults to `None`. Solid 2 answers `Some(0)` for
   `createComponent` in every vocabulary, because every release the reviews
   read behaves the same way. It is not a primitive: the call is not
   reactive, and no pass other than the call graph asks about it. The runtime
   model identity moves to `solid-v2/model-4`.
2. **An exact render is an edge, and nothing else is.** A call is a render
   site of project function `F` only when all of these hold:
   - its callee resolves to a symbol one of whose declarations lies inside one
     of the dialect's primitive-defining packages (exact path components:
     `solid-js`, `@solidjs/signals`, `@solidjs/web`), under a name the dialect
     says renders;
   - the argument at that position is a bare identifier, with no spread up to
     it;
   - that identifier's symbol is `F`.

   Anything else adds no edge, and the reference stays the escape it was:
   - a parameter;
   - `options.Wrap || Fallback`;
   - an unresolved import;
   - a project function that is merely named `createComponent`.
3. **Only the call graph reads the edge.** The render sites are kept apart
   from `function_call_sites`, which also feeds component identity (a
   directly called function is not a component) and execution-role
   inheritance. Neither of those questions has the same answer for an
   argument of a Solid call. `reach_from` walks render sites beside call
   sites. `compute_entered_only_through_calls` counts the rendered argument
   as a reference accounted for by its own edge, exactly as it counts a tag
   name.

## Soundness

The edge claims only that the caller enters `F` while the call runs. The table
above proves that for every build. The escape test now accepts the argument
reference, which is sound because the runtime invokes it in place and keeps
nothing it invokes later. An edge that did not fire, such as a dev build
throwing on a non-function, would only over-attribute. A render the graph
cannot name exactly changes nothing.

## Consequences

- Pinned by `scripts/contract-render-edge-attribution.test.mjs`, over stubs
  whose `createComponent` declaration is byte-faithful to rc.9's
  `types/client/component.d.ts:73`:
  - `rendered` imports the renderer from `@solidjs/web`, and `direct` imports
    it from `solid-js`. In both, `local` keeps the control's closures and
    only `Shell` is opened.
  - `impostor` (a project `createComponent` that retains `Comp`) and
    `parameter` (`createComponent(Comp, …)` with `Comp` a parameter) stay
    `fallback-all`.

  The pre-change binary fails the first test: `rendered`'s `local` closes
  nothing.
- Coverage (127 projects) and the contract corpus (107 fixtures) are
  unchanged.
- On solid-router's root node, replayed exactly, the catch-all count stays at
  9. Every render target there sits behind a `.d.ts` split: `Matches.js`'s
  `./Transitioner.js` binds `Transitioner.d.ts`, so the argument's symbol is
  the declaration's and names no project function. ADR 0137 joins that split.
