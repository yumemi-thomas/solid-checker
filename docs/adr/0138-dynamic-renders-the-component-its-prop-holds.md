# ADR 0138: `Dynamic` renders the component its `component` prop holds

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the dialect facts (`rust/crates/solid-dialect/src/lib.rs`,
  `Dialect::component_prop_renderers` and
  `Dialect::accessor_yields_only_its_compute`, answered in `solid_2.rs`), the
  syntax fact (`rust/crates/solid-facts/src/ast/component_value_flow.rs`,
  `component_value_flows`), the render index
  (`rust/crates/solid-reactive-ir/src/indexes.rs`,
  `SemanticLookup::prop_render_sites_at`), and the call graph
  (`rust/crates/solid-reactive-ir/src/attribution.rs`, `reach_from`,
  `prop_render_sites` and `reference_is_accounted_for`)
- Relation: adds a third kind of edge to the call graph that ADR 0135's
  `reachability` rung walks, beside call sites and ADR 0136's render sites. It
  adds no rung.

## Context

After ADR 0137, three of the five obligations of
`@tanstack/solid-router@2.0.0-rc.8`'s root node that still mark every export
belong to `getNotFound` (`not-found.js` 105, 370, 408). Its callers are
`CatchNotFound` and `Match`, and both are entered as values that Solid's
`Dynamic` renders (`dist/esm/Match.js`):

```js
const ResolvedNotFoundBoundary = Solid.createMemo(() => routeNotFoundComponent() ? CatchNotFound : SafeFragment);
// …
createComponent(Dynamic, { get component() { return ResolvedNotFoundBoundary(); }, … })
```

and, at `:200`, `Outlet` (which renders `Match`) as the last operand of
`route().options.component ?? router.options.defaultComponent ?? Outlet` in a
memo that `Dynamic` renders the same way. The escape test read each of those
references as a function value escaping into a callee the graph cannot follow.

## The runtime fact

`@solidjs/web` exports `Dynamic` and `dynamic` in rc.3 and rc.9. `Dynamic` is
the same three lines in every build (rc.3 `web.js:1865`, `dev.js:1935`,
`server.js:3208`; rc.9 `web.js:2099`, `web.dev.js:2271`,
`web.observe.js:2121`, `server.js:3764`, `server.dev.js:4013`,
`server.observe.js:3852`):

```js
function Dynamic(props) {
  const Comp = dynamic(() => props.component);
  return createComponent(Comp, omit(props, "component"));
}
```

`dynamic(source)` keeps the value in a memo it creates. Rendering the
component it returns creates one more memo, which calls a function value as
`component(props)` (or its `Symbol.for("solid.component-binding")` target)
and builds an element for a string. Nothing else is done with the value:
`bindingOf` and the thenable test read properties of it, and the dev and
observe builds write `$DEVCOMP` on it. `createComponent` runs `Dynamic`, and
then `Comp`, in place (ADR 0136). So the value is invoked only inside
computations that the render creates, which is how the graph already treats
every callback written inside a function.

This was probed on the published triples, installed from npm. The probe
renders `createComponent(Dynamic, { get component() { return Selected(); } })`
inside a root, with `Selected = createMemo(() => flag() ? A : B)`, then
writes `flag` and flushes. rc.3's four builds and rc.9's six all behaved the
same way:

- `A` ran once, between the statements before and after the call, with no
  `component` in its props, under an owner below the render's;
- the client builds ran `B` once after the write, under the same owner depth;
  the server builds, which do not re-run, never did;
- a data property (`{ component: A }`) behaved the same.

A one-argument `createMemo` is the holder the router needs. Its value is its
compute's return, and program code reaches it only through the accessor
(`accessor(computed(compute))`, rc.9 `@solidjs/signals` `prod/signals.js:77`).
With no options there is no `loadingValue` or `equals`. A parameterless
compute cannot read its previous value. The client `createMemo` is
`solid-js`'s hydration wrapper, which may yield a serialized value instead of
calling the compute. That is another value, not a second path for this one.

## Decision

1. **The dialect states both facts.**
   - `Dialect::component_prop_renderers()` lists `(export, prop)` pairs whose
     render invokes the prop's value as a component only inside computations
     that the render creates. Solid 2 answers `[("Dynamic", "component")]`.
   - `Dialect::accessor_yields_only_its_compute(primitive)` answers `true`
     only for `CreateMemo`.

   The runtime model identity moves to `solid-v2/model-5`.
2. **The syntax fact follows the value, exactly or not at all.**
   `component_value_flows` reports an identifier reference only when its value
   reaches a rendering prop and nowhere else. A rendering prop is one of:
   - `renderer(Component, { prop: value })`;
   - `renderer(Component, { get prop() { return value; } })`;
   - `<Component prop={value}/>`.

   The value is followed through:
   - parentheses and TypeScript wrappers;
   - both branches of `?:`;
   - both operands of `??` and `||`, and the right operand of `&&`;
   - `const X = value`, read as `X`;
   - `const X = call(() => value)`, read only as `X()`. The call has that one
     argument, a non-async arrow with no parameters and an expression body.

   A holder counts only if every one of its resolved references is such a
   read, it is never written, never redeclared, and not exported. Anything
   else is not followed and stays the escape it was:
   - a parameter or a prop read;
   - a member or computed read;
   - `let`;
   - any other call, including one of a memo with options.
3. **Every span the syntax fact hands back must resolve exactly.**
   - Each site's component is declared in one of the dialect's
     primitive-defining packages under a name the dialect pairs with that
     prop.
   - A call form's callee renders its first argument (ADR 0136).
   - Each holder call is a primitive the dialect vouches for.

   Which function the value is comes from the compiler: the index is keyed
   by reference, and the call graph asks it only of the resolved references of
   a function's own symbols and their aliases. That is the walk the escape
   test already makes.
4. **Only the call graph reads the edge.** Such a reference is accounted for
   by its renders. `reach_from` walks each render site as an edge from its
   outermost function, exactly as ADR 0136's render sites. The IR change stays
   inside the attribution index: `function_call_sites`, component identity and
   execution roles are unchanged.

## Soundness

The edge claims that the render's function enters `F`, when the render runs
or in a computation it owns. The runtime fact proves that for every build.
The escape test accepts the reference only when the syntax fact proves that
its value goes nowhere but those renders. A holder read anywhere else, or
exported, removes the reference entirely. The value can also be reached as
`F.then` or a component binding, but either needs a property on `F`, and
writing one needs a reference to `F` that is not accounted for. Every
failure direction is the existing `fallback-all`.

## Consequences

- Pinned by `scripts/contract-dynamic-render-attribution.test.mjs`, over
  stubs byte-faithful to rc.9's `createComponent`, `createMemo` and `Dynamic`
  declarations:
  - `memo` (the router's shape), `coalesce` (`props.as ?? Panel`) and
    `direct` (`{ component: Panel }`): `local` keeps the control's closures,
    and only `Shell` is opened.
  - `escaped` (the accessor is also stored), `holder` (a project call in
    place of `createMemo`), `impostor` (a project `Dynamic`) and `parameter`
    (the value reaches `Dynamic` as a parameter) stay `fallback-all`.

  Each half was falsified separately. With the renderer table empty, `memo`'s
  `local` closes nothing. With every holder call accepted, `holder`'s `local`
  keeps `callbacks`.
- The syntax fact's own cases are in `component_value_flow.rs`, including
  JSX. The JSX form is pinned only there; no package in the suite ships JSX.
- Coverage (127 projects) and the contract corpus (107 fixtures) are
  unchanged. The scripts suite passes (46 files).
- On solid-router's root node, replayed exactly, the catch-all count falls
  from 5 to 2. `getNotFound`'s three obligations go `reachability`, to
  `Match`, `Outlet`, `Matches`, `RouterProvider` and `CatchNotFound`. A full
  run of the viviana environment agrees: only `routerStores.js`'s two remain.
- No export's call summary moves, and the same six exports propose closures
  as before (the re-exported history helpers and `rootRouteId`). The two left
  are `getStoreFactory` (ADR 0135): they mark all 97 exports with
  `reactiveReads`, `returns` and `ownerRequirements`. `createFileRoute`,
  `Link` and `Outlet` stay degenerate until `RouterCore`'s constructor
  contract states what it does with that parameter.
- Not covered:
  - `dynamic(() => X)`. `X` is invoked wherever the returned component is
    rendered, so the edge would belong to those renders, and following the
    returned component is a different flow.
  - A block-bodied memo compute, a memo with options, and `mergeProps(...)`
    props: all stay escapes.
