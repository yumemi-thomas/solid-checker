# Open work

Open items only: known misses, decisions not to build, accepted precision trades, live approximations. A finished slice is reported in its commit message, not here.
A rule removed because `tsc` already reports it is recorded in `fixtures/tsc-oracle/README.md` and `rule-cases.json`. Delete an entry here when it is fixed or reversed.
Case ids name directories under `fixtures/runtime-oracle/`; the `open` list in each `oracle.json` is the executable half of section 1 (a listed site that stops firing fails the oracle as `stale-open`).

## 1. Known open detections

Defects the runtime reports and the checker does not. Each is a missing violation, not a false positive. Codes are runtime diagnostic codes; the rule is the one that should claim it.

Strict read in a setup body (`strict-read-untracked`, SC1001):

- `c2d-method-tracked` (App.tsx:19, :32): a function registered from a method or helper called in tracked JSX, then run by a later component body, keeps its registrant's lexical role. Same family: c2, c2c, c3d.
- `r10-destructured` (App.tsx:12): destructuring `useParams()` in setup reads the reactive parameter proxy; the checker does not take that destructuring as a reactive read.
- `r6-b6-classmethodopaque` (App.tsx:13): an opaque consumer invokes a class method during setup.
- `r9-b3-childrenreadinbody` (App.tsx:14): the accessor returned by `children()` is called in the component body.
- `r9-d1-mixmemoslotreadinbody` (App.tsx:7): a helper builds a memo from a callback and reads `m()` in the helper body.
- `review-opus2-c04-ctor-body` (App.tsx:12): a constructor read, class instantiated in the body.
- `review-opus3-g06-yield-star-spread-body` (App.tsx:13): a read in a generator reached through `yield*` and spread in the body. Delegation is not followed.
- `precision-ref-callback-read` (App.tsx:4): a read in an intrinsic `ref` callback runs untracked in the apply phase; the read rule places nothing in the `DirectiveApply` role.

Write in an owned scope (`reactive-write-in-owned-scope`, SC2001):

- A named handler both attached as `onClick` and called directly in the component body hides the write in a helper it calls; the runtime warns at that call (review probe `r2-event-reuse`, step 4; present before it).
- A setter call in a component's default parameter initializer runs in the owned component scope; the write rule places nothing there (reviewer case `r4-param-write`).

`flush` in an effect apply (`flush-in-effect-callback`, SC2008). The runtime reports these; the rule claims none of them (ADR 0341):

- `createRenderEffect`'s re-runs report `FLUSH_IN_EFFECT_CALLBACK`, but its first run is synchronous at creation, outside any flush, where `flush()` drains. The rule cannot say which run a call is in.
- A cleanup returned from a `createEffect` apply reports when the next run replaces it (it runs inside the marker) and not at disposal.
- Accessors are matched by property name, not by receiver: a getter, setter or Proxy-bound variable that cannot return, anywhere in the file or its imports, withholds every same-named property operation on the path (a miss accepted to keep zero false positives). A registration inside a method, a generator or a function passed by name to a primitive is not claimed; neither is one in a function only reached through an unmodelled consumer.
- Constant expressions the folder does not model (`NaN`, BigInt, template literals, object comparisons) are not folded: `if (NaN) flush()` and similar are reported under the feasible-path premise.
- An apply passed by name (`createEffect(v, applyFn)`) or written as the `{ effect }` bundle: the position is not placed. A helper in a callback handed to an unmodelled consumer (a project function taking the callback, a package call) is not followed.

Async read outside a loading boundary (`async-outside-loading-boundary`, SC5003):

- `r9-b3-childrenasync` (App.tsx:15): an async memo read flows through `children()` into JSX with no `Loading`.

Missing owner (`missing-owner`, SC4001):

- `review-r1-owner-mix` (App.tsx:5): one callback invoked through `runWithOwner` and as an event handler. The owner walk refuses the literal as a whole. Same shape: `review-r3-owner-double-call`.
- `review-r1-owner-mix` (App.tsx:13): a handler returned from `createRoot`, later dispatched with no owner.
- `review-r5-owner-class-control`, `review-r6-owner-default-control`, `review-r6-owner-jsx-getter-control`: the call sits inside a class field, a default parameter, or a JSX getter; the owner walk refuses those containers.
- `review-r7-event-array`: the typed `[handler, data]` event tuple is not followed by owner seeding.
- `review-opus5-t6-rest-timer-owner` (App.tsx:8): an effect created in a timer inside a `Show` callback.
- JSX written in a `createEffect` apply (`createRoot`-less) creates a render effect with no owner: the runtime reports `NO_OWNER_EFFECT` and the owner walk places nothing there; the same for a helper that creates a render effect and is called from an apply (found by the SC2008 review; probe: the first revision of `flush-in-effect-callback`'s oracle case).
- `review-opus2-o15-settled-class-field-new-in-timeout` (App.tsx:5): a class constructed in a timer. The field stays an uncertifiable SC3001.

Primitive used as a value (`reactive-dispatch-unresolved`, SC9012):

- Found only through a named or namespace import of the primitive. Not followed: a stored or aliased namespace object (`const NS = Solid; NS.createEffect(…)`), a namespace import of a project barrel, a computed member (`S["createEffect"]`), an `import S = require(…)` member (review round 3, probes `r3-ns-escape-defect`, `r3-barrel-escape-only`, `r3-computed`). The exact fix is a reference census: every TypeFacts reference of a primitive's symbol that is not a primitive callee is an obligation. None of these shapes is in the corpus.

`runWithOwner` with a disposed owner (`RUN_WITH_DISPOSED_OWNER`, a dev warning, rc.13 `dist/dev-shared.js:6919-6932`): no rule claims it. Proving it needs three facts the engine has no single query for: the owner variable is written once, from `getOwner()` directly in a `createRoot` body; that body calls its own `dispose` parameter unconditionally at statement level after the capture (no earlier exit, parameter never reassigned); and every `runWithOwner` that reads the variable runs after the root body, which holds for any handler or later callback. A shape that fails any of them (a disposal under a condition, a captured owner in a store or a closure that is itself called from the body) is only sometimes disposed. Not built in the SC2008 slice; the corpus has no `runWithOwner` on a root owner it disposes, so there is no measured yield. Reopen with a report or a corpus site.

Other:

- Synchronous DOM calls that dispatch (`focus`, `blur`, `click`, `dispatchEvent`, form `reset`/`requestSubmit`, popover and dialog toggles) are refused. Not modeled and still trusted: custom-element reactions on tree mutation and attribute writes, and focus loss when a focused element is removed (ADR 0329).
- A context-provided value read through a package hook (kui `TopicsRoute.tsx:425`) stays an uncertifiable SC9012. The router source proves its own provider only; a project provider or an `Object.defineProperty` replacement of the location getter both type-check (ADR 0289, ADR 0312).
- `typeof window` in solid-query source blocks the proxy proof unless a Window realm is declared; two spotify-desk-thing `prefer-show` rows were lost that way (ADR 0295, ADR 0312).

Not a miss: `r4c-leak` line 43 is listed open only because the runtime names the JSX read of the memo while the checker's uncertifiable SC5003 sits at the async read two lines above. The oracle has no frame linking them.

## 2. Decisions not to build

- A write is not proven legal because every invocation of its function is an event dispatch (built in step 4, dropped): a synchronous `click()`/`dispatchEvent` from an owned scope runs the handler under that owner (review round 4, probe `r4-sync-event`). About 110 corpus SC2001 obligations stay open. Reopen with an owner-at-dispatch fact.
- Host-origin proof (DOM objects handed to Solid-package helpers, `makeEventListener(window, ...)`). Built as a prototype: 29 uncertifiable removed, 0 violations moved. It needs a premise that project code never installs members on host DOM objects, and app-game has 1,232 unknown-key writes. Reopen when a sweep shows a violation, not an uncertifiable, that hinges on it, and the owner states that premise.
- Anonymous arrows as their own summary owners. It detached captured parameter member calls from the enclosing function and silently dropped `execute(input) { return invoke(() => input.callback()); }`, which must keep SC9012 at an opaque caller (control in `fixtures/reactive-ir/accessor-provenance/Anonymous.tsx`). Reopen when captured-parameter behavior and unknown callback timing compose across separate source summaries; Relay's `() => props.environment` is the first user.
- A shared reference/identity layer (`Resolved<T> | Unknown`). Not started. Every false positive in the seven ADR 0326 review rounds was a proof re-deriving "is this still the same value" (props aliases, `arguments`, `delete`, redeclaration, reassigned tags, views of props). Reopen when the next proof needs that question answered.
- Recall fixes for the open oracle sites in section 1. The yield is zero on the 49-project corpus: `useParams()` destructuring occurs 0 times; every `children()` helper accessor there is read in JSX, never in a body. Reopen when a real project or report hits one.
- Inferring a `never` exit from a function body. A call completes normally unless its selected signature is `asserts` or returns `never`; a helper that throws without declaring it is outside the model. The alternative, treating every helper call inside a caller's `try` as unproven, costs recall the owner declined (ADR 0279, option b). Reopen if a producer fact records inferred throws.
- Showing users uncertifiable results. They are internal (sweeps, coverage, the oracle), never warnings or exit codes (ADR 0328).

## 3. Recorded precision trades

A violation withheld, or turned uncertifiable, to keep zero false positives. Each loses recall on purpose.

- Instance field initializer: every violation whose primary location is in one is demoted to uncertifiable, whichever rule claimed it (`diagnostics.rs`, `site_in_instance_field`). Static fields and static blocks keep their placement.
- Code in a component's element children (`<Show>`, `<Loading>`, `<Errored>`, portals, wrappers rendering `props.children`) has no proven placement (`inside_component_element_children`); a read there is an uncertifiable `strict-read-untracked`, including where a wrapper evaluates its children eagerly in its own body (`return props.children`) and the runtime warns.
- `reactive-handler-frozen` (SC1007) on an element inside a tracked JSX expression or a component's children is uncertifiable: the listener is replaced when the scope re-runs.
- `async-outside-loading-boundary` (SC5003) through a forwarding wrapper that reads `children` with no `Loading` of its own is unresolved (`loading_cover_at`); a wrapper that resolves `children()` outside any `Loading` is uncertifiable.
- `no-direct-mutation` (SC2003): a write through a 2.0 props object or a `merge`/`omit` view is uncertifiable. The compiler's props object is plain, so the write may land; only a proxy drops it.
- `no-destructure` (SC1003) with an unknown or mixed role is uncertifiable, not a proven snapshot.
- Code in or after a `for await` loop is past the first suspension; no strict read is proven.
- A signal read inside a JSX element passed as a prop does not make that prop reactive; reads of it are uncertifiable.
- A generator body is never placed where it is written. A call to a generator runs none of its body.
- A function returning JSX with no rendering tag is a proven component only when every reference to it is its declaration, module surface, a dialect callback, a dialect renderer prop or a `lazy` default; otherwise (alias, object storage, a call argument, a router config not read from source, a method, an anonymous default export) its body reads and writes are uncertifiable. Audited routers' route components are read (ADR 0334, 0335). A named function expression written as a route component stays under ADR 0331's census (its own name can hand it on); only anonymous inline values count. Decisions not to build: definitions or components built by a project call (`protect(X)`, `lazyScreen(...)`: the project function may call them), `children` thunks and `2.0.0-next.21` (no corpus use; audit on demand), file routes (`fileRoutes(pageRoutes)`: the Vite plugin's virtual module is not readable, and a route file's default export is already accounted by its module surface), and an exposure veto for TanStack (no corpus project reads a component key, so the coarse veto costs nothing). TanStack's key-read veto (ADR 0334) is project-wide: one `props.component` read withholds it; the router-definitions premise instead closes the APIs that expose definitions. Solid makes a function a component only where `createComponent` calls it (`precision-map-renderer`, `precision-method-renderer`).
- A read, write or action no fact places (`ExecutionRole::Unknown`: a JSX attribute function, a closure stored in a tracked scope, a write after an `await` not proven to dominate it, a helper with no resolved caller) is an uncertifiable finding of its rule, never silent; `Unreported` (several non-reporting roles, or past the strict-read window or a dominating await) stays silent. A write in `catch`/`finally` after an await stays uncertifiable unless the `try` starts by awaiting a call of an async function with inert arguments: otherwise a synchronous throw reaches it unsuspended (about 480 corpus sites; most await a member call or a non-async function returning a promise). The same holds for `resolve`/`until` in such code (SC2004/SC2005), a read in a helper called after an `await` (SC1002), options the syntax cannot read beside an async computation (SC7002), and a primitive used as a value (SC9012; the shapes not yet followed are in section 1). A coverage rule disabled by options still keeps the result from certifying.
- Mixed invocation of one callback (owned setup and event dispatch) refuses the owner proof as a whole (ADR 0327). A callback run only by events is proven unowned.

Known false-positive shapes left open (fire today, wanted gone):

- `missing-owner` (SC4001): a synchronous `dispatchEvent` or `click()` from an owned scope runs the handler under that owner; every event handler is taken to run with no owner. Oracle: `review-r7-sync-scroll`, `review-r7-sync-child-scroll`. Fixing it needs an owner-at-dispatch fact.
- `strict-read-untracked` (SC1001): `try { await p } finally { read() }` reports the read. Only a synchronous throw while evaluating `p` skips the await. Oracle: `r9-d3-tryawait`; none in the corpus.
- `missing-owner` (SC4001): an effect in an IIFE or an eagerly called factory callback inside a `ref` expression runs while rendering, owned, and one in a generator `ref` never runs; the owner walk takes both as apply phase. SC6001 no longer fires there (`directive-owned-subscopes`).
- `strict-read-untracked` (SC1001): a function rendered by a tag the program never reaches (`<Show when={false}><Row/></Show>`) and also called plainly (`items().map(Row)`) keeps its component proof; every visible tag is taken as rendered, the same feasible-path premise as a direct read.
- `primitive-in-directive-application` (SC6001): `createRoot` in an apply callback is reported even when its `dispose` is called; the rule's premise is that a per-element root is never disposed.
- A read after `if (c) await` or a loop that may skip the await is reported: the feasible-path premise every direct read rests on. Oracle: `r9-d3-ifawait`, `r9-d3-loopawait`.

## 4. Open questions and live approximations

- ESLint adapter: native analyzes the file on disk; the adapter maps byte offsets onto ESLint's in-memory text. An unsaved editor buffer, or a later `--fix` pass, places findings wrongly.
- WASM adapter: the request carries no package files, so every package import keeps an open requirement (SC9005, uncertifiable). Package source analysis exists only in the native check.
- Retained daemon: a tsconfig whose `extends` names a package with an `exports` map is never reused, because that resolution is TypeScript's and is not reproduced (`host_config.rs`). Performance only.
- Premises that are owner decisions, not defects: standard runtime configuration (ADR 0266), standard hydration host (ADR 0268), member stability (ADR 0273), package integrity (ADR 0300), the audited router bytes (ADR 0324, and ADR 0334's TanStack route options and ADR 0335's router definitions with no callables installed on `Object.prototype`, residual: route options reached by a computed key or reflection; ADR 0324 residual: a patch applied only at bundle time by a Vite plugin, or a `vite.config`-only alias without host resolution).
- Standard-library trust does not model getters, `toString`, Proxy on handed objects or thenables (ADR 0318).
- Route-parameter proofs stop at: `Match` accessors, `useSearchParams`, `useLocation`, `createMemo(() => params.k)`, and constants through barrels or `export *` (ADRs 0324, 0325). Each leaves an uncertifiable SC9012.
- `execution-map-incomplete` (SC9004) is retired because `uncovered_jsx_expressions()` is empty by construction. A third compiler adapter must prove the same totality in its own tests (`rust/dialects/solid-v2/compiler/src/lib.rs`).
- Recall instruments: `scripts/mutants.mjs` seeds fresh locals and inline handlers (P1–P9) and, since round 2, existing signal accessors (E1), reactive props (R1), setup helpers (H1, H8) and named or imported intrinsic handlers (N2). Round 2 (1,047 sites): 0 missed except 2 H8 actions in arrow-bound helpers (since proven: reachability resolves a `const`-bound arrow or function expression by its binding; re-seeded by hand in sefer `useMenu` and app-game `createZoom`, not re-swept; a helper bound by `let` or `var` is still unreachable, a known miss); R1 loses 8 of 58 to `omit(props)` views refusing the exact props root. Unsampled: async continuations, package callbacks, component-prop handlers (timing set by the component). The runtime oracle skips cases it cannot represent: patched or aliased packages, CommonJS export objects, declaration-only modules (list in `fixtures/runtime-oracle/README.md`).
- Static-only oracle rows `r9-e2-storedirectwrite` and `r9-e2-storeviewwrite` are real defects (the store drops the write) with no dev diagnostic. They prove nothing about false positives.
- `inferred-host-*` fixtures no longer pin an admission outcome through a finding: package contracts are gone, so `startClosed()` is opaque. The admitted/declined outcome shows as snapshot status (`uncertifiable` for an admitted inferred host, `certified` for a declined one; each README names its twin), and the `*-cases.json` proof inputs are pinned by Rust tests. Admission through `@solidjs/vite-plugin` is unreachable from a fixture (the fixture-plugin authority was deleted with the contracts; the compiled pins name only published releases), so the plugin-config fixtures were deleted; `ssr` grammar stays covered by `host_config/tests.rs`. A finding-bearing admission case needs a package whose declaration alone states an owner requirement, which the checker does not read.
