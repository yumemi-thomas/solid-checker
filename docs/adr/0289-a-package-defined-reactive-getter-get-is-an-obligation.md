# ADR 0289: A Get of a package-defined reactive getter is an obligation

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `accessor_provenance::Resolver::dependency_getters` (derivation);
  - `interproc::dependency_getter_obligations` (the SC9012 obligation);
  - `SemanticLookup::is_context_reference` (the redirect-aware context
    census);
  - the `DEPENDENCY_SOURCE_GETTER` wording in `projection.rs`.
- Extends ADR 0284 (dependency source mode). Keeps ADR 0153, ADR 0272 and
  ADR 0273 unchanged.

## Context

ADR 0284 lists one loss against contract mode as "a context-provided value
read through a package hook (kui's location)". In kui's `shell` project,
contract mode reports

```
TopicsRoute.tsx:425:75 SC1001 reactive getter "listLocation.search" is read
through returned object Get in TopicsScreen
```

from `createSignal(queryFromAddress(listLocation.search))` with
`const listLocation = useLocation()`. With
`--dependency-source @solidjs/router` the site was silent: no violation and
no obligation.

The value comes from `@solidjs/router` 2.0.0-next.26's own source:

- `useLocation = () => useRouter().location`;
- `useRouter = () => invariant(useContext(RouterContextObj), …)`, where
  `invariant` returns its first parameter;
- the router component provides `<RouterContextObj value={routerState}>`
  (`routers/factory.jsx`), `routerState = createRouterContext(…)`;
- `createRouterContext` returns an object literal whose `location` member is
  `createLocation(…)`;
- `createLocation` returns `{ get pathname() { return pathname(); }, get
  search() { return search(); }, … }` over memos.

### Where it refused

Three things stood between the package's source and the Get, in order.

1. **The context census could not see the provider.** Every runtime file of
   the router has a `.d.ts` beside it, so the compiler binds
   `import { RouterContextObj } from "../routing.js"` in `factory.jsx` to
   `routing.d.ts`. ADR 0284 redirects that binding to `routing.js`, and the
   IR's entity table carries the redirected root. But
   `is_context_reference` matched the compiler's reference list of the
   context's runtime symbol, which does not hold a reference the compiler
   bound to the declaration file. So the `<RouterContextObj>` tag was not a
   provider: the census recorded no provider for the context, and counted the
   tag as an escape of it.
2. **No step carries a getter.** The accessor transport (ADR 0266, 0271,
   0273) projects only callable members, one level deep, of literals whose
   every property is a data property (`closed_properties`). `routerState`
   holds getters (`pendingTarget`, `submissions`), and the Get is two
   members deep: `.location`, then `.search`. Even in project code, a Get of
   a factory-made getter object (`makeLocation().search`) is never a read.
3. **The package's own open write vetoes its objects' members.**
   `staticIntegration` runs `Object.assign(obj, written)` and
   `mergeParams` runs `Object.assign(params, matches[i].params)`. Under ADR
   0284 decision 8 (ADR 0273's wildcard veto scoped to dependency internals),
   either one vetoes every member of every object the router creates,
   `location` and `search` included. `routerState` is also handed to
   `setupNativeEvents`, `setupLinkClaims`, `registerFlightRouter`,
   `restoration.create`, `<Root>` and `<Routes>`, so its receiver census does
   not close either.

Step 3 is a refusal by an accepted premise, not a missing fact: the name
index cannot tell that `obj` is not the location object, and the premise says
the package's open write vetoes the members of the objects it creates.
Carrying the getter would need a new premise as well as a new transport, and
would still refuse here. The refusal is right. The silence was not: in
contract mode the package's import is answered by its contract (here a
violation, elsewhere an ADR 0272 `returned getter` obligation), and without a
contract by SC9005. Dependency source mode removed both (ADR 0284 decision 6)
and put nothing at the Get.

## Decision

1. **The context census follows runtime redirects.** When the program
   carries runtime symbol redirects (ADR 0186, ADR 0284), a reference whose
   entity is a context's runtime binding is a context reference. Without
   redirects the census is unchanged. This only recognizes providers and
   context tags that exist; it never assumes one.
2. **A derivation finds the getter.** `dependency_getters` follows the value
   at a Get's receiver through exact bindings, a resolved function's every
   return (a returned parameter continues at this call's argument, so
   `invariant(value, message)` is the identity it is), the data members of
   object literals, and every recorded provider of an exact context. It
   collects the getters of that member name in object literals inside a
   package analyzed as source, when the getter's body calls an exact
   accessor. It ignores every closure, census and stability requirement, like
   `context_origin`, so it never proves anything.
3. **The Get is an obligation.** A project Get (`a.b`, a literal computed
   key `a["b"]`, or a destructuring pattern) in a rendering function whose
   execution role reports an untracked read, and whose receiver derives to
   such a getter, is an uncertifiable `SC9012` (`reactive-dispatch-unresolved`)
   with analysis context `dependency-source-getter:<package> defines the
   member as a getter that reads an accessor, line N of <file>`. A Get in
   JSX, a memo or an event handler, a write target, a callee, an inert getter
   and a data member get nothing. It is never a violation.
4. **Computed fresh.** The obligations are built on every analysis after the
   per-file interprocedural results and are never retained per file: the
   derivation reads other files' literals and the provider census.

## Consequences

- kui `shell`, `--dependency-source @solidjs/router`: the five Gets contract
  mode states through the router's getter rows are back, at the same
  locations. `TopicsRoute.tsx:425:75` (a violation in contract mode) and
  `TopicsRoute.tsx:802:41`, `ClustersRoute.tsx:99:26`, `:351:25`,
  `MessagesRoute.tsx:75:26` (`returned getter execution` obligations in
  contract mode) are now `SC9012`. Uncertifiable 137 → 142, violations 22 →
  22.
- Base mode is unchanged by construction (no package source, no redirects)
  and by measurement (every finding of the rc.13 corpus sweep identical).
- The context census fix also lets a package's own context reach the
  callable transport in dependency source mode where it was blocked as
  escaped. The transport's own requirements still apply.
- Remaining gaps:
  - a Get of a getter object the **project** creates
    (`makeLocation().search`, ADR 0283's literal handed to a helper aside)
    is still silent, in every mode;
  - a getter that reads only through a helper call, a parameter or a store
    member is not recognized as reactive (`createLocation`'s `get state()
    { return state(); }`, where `state` is a parameter);
  - spreads and `Object.assign` copies of such an object, Gets in a role the
    execution analysis cannot classify, and receivers that reach the object
    only through a parameter of a function called from several sites;
  - `export { name } from "./file.js"` inside a package, where `file.js` has
    a `.d.ts` beside it, is not redirected (ADR 0284 redirects import
    statements only), so a hook bound only that way is unbound. The router
    binds because its own modules import the same names.

## Evidence

- **Fixture** `reactive-ir/dependency-source-context-getter`: four `SC9012`
  (a local, an inline chain, a destructuring pattern, a literal computed
  key), no violation, and nothing for JSX, a memo, an event handler, inert
  getters, a data member, a same-shaped project hook and a project-made
  getter object. Without decision 1 the fixture reports nothing.
- **Coverage:** only the new fixture's snapshot is added; no other snapshot
  moved.
- **rc.13 corpus**, release binary, browser host:
  - base: 562 violations / 3,731 uncertifiable, every finding identical;
  - dependency mode with ADR 0284's 11 packages: violations 559 → 559
    (identical rows), uncertifiable 3,556 → 3,583. The 27 added are all
    `dependency-source-getter` obligations at `@solidjs/router` location
    Gets in 10 apps; none was removed.
