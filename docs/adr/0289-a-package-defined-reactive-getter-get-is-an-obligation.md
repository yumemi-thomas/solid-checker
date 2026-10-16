# ADR 0289: A Get of a package-defined reactive getter is an obligation

> Status: contract-related decisions superseded by [ADR 0312](0312-package-contracts-are-removed.md). Source analysis and runtime premises remain subject to that ADR.

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `accessor_provenance::Resolver::dependency_getters` (derivation);
  - `interproc::dependency_getter_obligations` (the SC9012 obligation);
  - `SemanticLookup::is_context_reference` (the redirect-aware context
    census);
  - the `DEPENDENCY_SOURCE_GETTER` wording in `projection.rs`.
- Extends ADR 0284 (dependency source mode). Keeps ADR 0153, ADR 0272 and
  ADR 0273 unchanged.

ADR 0300 (2026-10-14) extends this decision with an explicit package integrity
premise and a separate getter proof. The derivation below remains the
uncertifiable fallback. The review and recovery investigation record the
boundary before the owner accepted that premise.

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

## Review round 1

One false violation, fixed. Decision 1 made a package's own provider tag
visible to the context census, so a package context with one visible provider
transported its members to the project. A package that is not analyzed can
import that context and provide another value: `StaticWrap` in
`fixtures/reactive-ir/dependency-source-context-reprovided` provides a
static object, and the project's read under it was reported as SC1001. A
context created by a package analyzed as source now has no closed provider
census (`Resolver::provider`). That was already the effect before this ADR,
when the package's tag counted as an escape. The obligations of decision 2
do not use the census and are unchanged.

Recorded, not fixed (contrived):
- an ADR 0287 trap that throws for every string key still yields a
  violation;
- a monkeypatched `Reflect.get` is not seen.

## Recovery investigation (2026-10-14)

No new proof is admitted. Kui's `TopicsRoute.tsx:425:75` remains SC9012 in
dependency source mode. The installed `@solidjs/router` 2.0.0-next.26 source
establishes the candidate getter, but not the value and descriptor at that
read:

- `routing.js` creates the default-less `RouterContextObj`; `index.jsx`
  publicly re-exports it as `RouterContext`. `useLocation()` reads
  `useRouter().location`, and `useRouter()` returns the context value through
  `invariant`. `routers/factory.jsx` provides `createRouterContext(...)`.
- `createRouterContext` builds `location` with `createLocation`. Its
  `search` getter unconditionally calls the `search` memo. The object is
  returned as an ordinary mutable literal, without a freeze or a sealed
  descriptor. `routers/components.jsx` passes the same location to root and
  route preload callbacks and to the supplied root component.

A single visible package provider is insufficient. ADR 0291 selects Solid
packages, not every runtime module: a non-Solid or refused package can import
the exported context and supply another value. Review round 1's
`dependency-source-context-reprovided` already demonstrates this failure.
The current facts contain no proof that all runtime users of this exported
context have entered the provider census. Nor do they connect this read's
owner to the router's provider through the route and component callbacks.

Joining all visible providers is insufficient too. A project provider may
use `{ ...router, location: { ...router.location, search: "" } }`, which
preserves the declared types but makes the eventual Get inert. An unknown
provider contributes no proven getter. Even when every provider initially
supplies the reactive object, a consumer can use
`Object.defineProperty(useLocation(), "search", { value: "" })` to replace
its getter. The package's open writes still veto `location` and `search`
under ADR 0284, and its shared receivers still escape. Ignoring those vetoes
would admit the package-writes-getter case rather than prove it impossible.

The project-provider and descriptor-replacement examples pass TypeScript
5.9.3 `--noEmit` against kui's installed, published router 2.0.0-next.26 and
Solid 2.0.0-rc.13 declarations, without fixture stubs. They establish that
types do not discharge either demand; they are not claims that kui contains
these replacements.

Recovery needs affirmative facts closing the context's runtime consumers
(or identifying the enclosing provider at this read), then preserving both
the `location` member and its `search` descriptor across aliases, callbacks
and writes. Alternatively, every possible provider and subsequent write
must preserve an accessor-reading getter. `dependency_getters` supplies
neither proof: it is a may-derivation that deliberately ignores these
requirements. Promoting its result, removing the dependency-root refusal,
or exempting the router's open writes would loosen the accepted boundary.
This investigation stops without any of those changes.

## Recovery under ADR 0300 (2026-10-14)

The owner now accepts the package integrity premise verbatim in ADR 0300.
Unseen providers and writes are excluded conditionally; every provider and
write in the analyzed program still participates. A separate proof follows
exact immutable bindings, one unconditional return, returned-parameter
identity guards and closed data members, joining every visible provider of a
default-less selected-package context. Open/spread provider values refuse.

The descriptor proof keeps global named vetoes and project wildcard scope. A
wildcard, with its existing project-wide or package scope, may be discharged when all of
its target's local identity inputs are other fresh literal allocation sites.
The router's `obj`, `params` and schema-merge result have those other origins;
an unknown parameter target does not. Dynamic-key writes whose receivers
derive to the location literal veto too. No package or API name is trusted.

The getter must read an exact runtime accessor at entry, and the existing
execution analysis must prove the Get's role. Failed proof stays SC9012;
helper-only getters, open values, project-created getter objects and
unsupported invocation channels retain their earlier limits. Reports state
the conditional premise explicitly.

The final rc.13 browser sweep adds two unique violations: kui
TopicsRoute.tsx:425:75 (observed in shell and feature-topics) and donegeon
WaitlistRoute.tsx:33:108. Both invoke the installed router's memo-backed
search getter in an untracked component body. Total violations 562 → 565,
uncertifiable 3,248 → 3,245; no errors or removed violation. The execution
holes at kui ClustersRoute.tsx:351:25 and TopicsRoute.tsx:802:41, and
donegeon WaitlistRoute.tsx:52:25 remain uncertifiable, now SC1001 about the
proven getter read's execution rather than SC9012 about its backing.
