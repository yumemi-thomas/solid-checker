# Open work

Open items only: known misses, decisions not to build, accepted precision trades, live approximations. A finished slice goes in its commit message.
A rule `tsc` already reports is removed and recorded in `fixtures/tsc-oracle/README.md` and `rule-cases.json`. Delete an entry here when it is fixed or reversed.
Case ids name directories under `fixtures/runtime-oracle/`. The `open` list in each `oracle.json` is the executable half of section 1: a listed site that stops firing fails as `stale-open`.

## 1. Known open detections

Defects the runtime reports and the checker does not. Each is a missing violation, not a false positive; the named rule should claim it. Codes are runtime diagnostic codes.

Strict read in a setup body (`strict-read-untracked`, SC1001):

- `c2d-method-tracked` (App.tsx:19, :32): a function registered from a method or helper called in tracked JSX, then run by a later component body, keeps its registrant's lexical role. Same family: c2, c2c, c3d.
- `r10-destructured` (App.tsx:12): destructuring `useParams()` in setup reads the reactive parameter proxy; the checker does not take that destructuring as a reactive read.
- `r6-b6-classmethodopaque` (App.tsx:13): an opaque consumer invokes a class method during setup.
- `r9-b3-childrenreadinbody` (App.tsx:14): the accessor returned by `children()` is called in the component body.
- `r9-d1-mixmemoslotreadinbody` (App.tsx:7): a helper builds a memo from a callback and reads `m()` in the helper body.
- `review-opus2-c04-ctor-body` (App.tsx:12): a constructor read, class instantiated in the body.
- `review-opus3-g06-yield-star-spread-body` (App.tsx:13): a read in a generator reached through `yield*` and spread in the body. Delegation is not followed.
- `precision-ref-callback-read` (App.tsx:4): a read in an intrinsic `ref` callback runs untracked in the apply phase; the read rule places nothing in the `DirectiveApply` role.

- `@solid-primitives/utils` helpers over a getter, beyond `access(getter)` and `accessWith(getter)` (ADR 0338): `accessArray([getter])`, `withAccess(getter, fn)` and `access` reached through another project helper's call hop read the getter on every path but are not decided: they stay silent or uncertifiable, and an unproven `access` argument is an obligation. A getter held through an alias (`const alias = count`), a function literal (`access(() => count())`) or a props value is arity-unknown and uncertifiable. A getter passed to a helper that reads its parameter in a hoisted function it calls (`return g(); function g() { return p(); }`) is silent (fixture `package-access-helper`, `MissedHoistedCallee`).

Write in an owned scope (`reactive-write-in-owned-scope`, SC2001):

- A named handler both attached as `onClick` and called directly in the component body hides the write in a helper it calls; the runtime warns at that call.
- A setter call in a component's default parameter initializer runs in the owned component scope; the write rule places nothing there.

`flush` in an effect apply (`flush-in-effect-callback`, SC2008). The runtime reports these; the rule does not claim the open shapes below (ADR 0341). Positions and premises are in rule page `flush-in-effect-callback`:

- `createRenderEffect` re-runs and a returned cleanup report only on some runs (its first run is outside any flush). The rule cannot tell which run a call is in, so neither is claimed.
- A helper with a handler, timer, module-level or exported caller, an alias or an argument use is not claimed, though its call from the effect is still a no-op (accepted: the claim is universal, ADR 0341). `runtime-oracle/flush-in-effect-callback` lists the exported-helper case as `open`.
- A `flush` imported through another package's `exports` conditions, an apply passed by name or in a `{ effect }` bundle, and a helper in a callback given to an unmodelled consumer are not claimed.

Async read outside a loading boundary (`async-outside-loading-boundary`, SC5003):

- `r9-b3-childrenasync` (App.tsx:15): an async memo read flows through `children()` into JSX with no `Loading`.

Missing owner (`missing-owner`, SC4001):

- `review-r1-owner-mix` (App.tsx:5): one callback invoked through `runWithOwner` and as an event handler. The owner walk refuses the literal as a whole. Same shape: `review-r3-owner-double-call`.
- `review-r1-owner-mix` (App.tsx:13): a handler returned from `createRoot`, later dispatched with no owner.
- `review-r5-owner-class-control`, `review-r6-owner-default-control`, `review-r6-owner-jsx-getter-control`: the call sits inside a class field, a default parameter, or a JSX getter; the owner walk refuses those containers.
- `review-r7-event-array`: the typed `[handler, data]` event tuple is not followed by owner seeding.
- `review-opus5-t6-rest-timer-owner` (App.tsx:8): an effect created in a timer inside a `Show` callback.
- JSX in a `createEffect` apply (no `createRoot`), or in a helper creating a render effect that an apply calls, makes an unowned render effect: the runtime reports `NO_OWNER_EFFECT`, and the owner walk places nothing there (the first `flush-in-effect-callback` oracle case shows it).
- `review-opus2-o15-settled-class-field-new-in-timeout` (App.tsx:5): a class constructed in a timer. The field stays an uncertifiable SC3001.
- `oracle-behaviour-primitive-in-directive` (App.tsx:13): an effect in the apply a named factory returns (`ref={leakingTrack(count)}`) is reported as SC6001 only. The owner walk places only a ref expression's top-level functions (ADR 0345), so it makes no owner claim for a closure a call returns. An IIFE returning such a closure is uncertifiable for SC6001 and unclaimed.

Read after an await (`reactive-read-after-await`, SC1002), `after-await-open`: a handler on a promise held in a variable before it is returned; a member helper (`api.read()`); `fetch(...).then`; a `.then` on a `Promise<T>` not proven native; `Promise.race` of helpers; an async IIFE the computation awaits. Each fires the runtime warning without a proof (ADR 0340; rule page `reactive-read-after-await`).

Primitive used as a value (`reactive-dispatch-unresolved`, SC9012):

- Found only through a named or namespace import of the primitive. Not followed: a stored or aliased namespace object (`const NS = Solid; NS.createEffect(…)`), a namespace import of a project barrel, a computed member (`S["createEffect"]`), an `import S = require(…)` member. Fix: a census where every TypeFacts reference of a primitive's symbol that is not a primitive callee is an obligation. None of these shapes is in the corpus.

`runWithOwner` with a disposed owner (`RUN_WITH_DISPOSED_OWNER`, a dev warning, rc.13 `dist/dev-shared.js:6919-6932`) is claimed by no rule. Proof needs the owner variable written once, from `getOwner()` in a `createRoot` body; that body calling its `dispose` parameter unconditionally after the capture (no earlier exit, parameter never reassigned); and every `runWithOwner` reading the variable running after the body. A conditional disposal, or an owner captured in a store or a closure the body calls, is only sometimes disposed. Not built: no corpus site has a `runWithOwner` on a root owner it disposes. Reopen with a report or a corpus site.

Other:

- Synchronous DOM calls that dispatch (`focus`, `blur`, `click`, `dispatchEvent`, form `reset`/`requestSubmit`, popover and dialog toggles) are refused. Not modeled and still trusted: custom-element reactions on tree mutation and attribute writes, and focus loss when a focused element is removed (ADR 0329).
- An argument of unknown provenance handed to a callee that calls its parameter (`access(value)` over a spread, held alias, `Array.from`, `[get][0]`, `(() => get)()`, a helper's return or a store property) is an SC9012 obligation unless proven not a getter (ADR 0338, `Unproven.tsx`). A member call on a primitive result is uncertifiable even for `.toString()`. A project call's result is not proven a non-getter even when every return is a number (`access(give())`); its returns are not followed.
- Patching the array iterator's `next` (`Object.getPrototypeOf([][Symbol.iterator]()).next = …`) changes how tuples destructure (`const [value] = createSignal(1)`), which every tuple proof reads as slot order. ADR 0282's iteration veto covers project tuples only, so such a patch is not seen. The access proofs share the premise (ADR 0338).
- A context-provided value read through a package hook (kui `TopicsRoute.tsx:425`) stays an uncertifiable SC9012. The router source proves only its own provider; a project provider or an `Object.defineProperty` replacement of the location getter both type-check (ADR 0289, ADR 0312).
- `typeof window` in solid-query source blocks the proxy proof unless a Window realm is declared (ADR 0295, ADR 0312).

Not a miss: `r4c-leak` line 43 is listed open, but the runtime names the JSX read while the checker's uncertifiable SC5003 sits two lines above at the async read. The oracle has no frame linking them.

## 2. Decisions not to build

- A write is not proven legal because every invocation of its function is an event dispatch: a synchronous `click()`/`dispatchEvent` from an owned scope runs the handler under that owner. About 110 corpus SC2001 obligations stay open. Reopen with an owner-at-dispatch fact.
- Host-origin proof (DOM objects handed to Solid-package helpers, `makeEventListener(window, ...)`). Not built: it needs a premise that project code never installs members on host DOM objects (app-game has 1,232 unknown-key writes). Reopen when a sweep shows a violation that hinges on it and the owner states the premise.
- Anonymous arrows as their own summary owners. It detached captured parameter member calls from the enclosing function and dropped the SC9012 an opaque caller must keep (control: `fixtures/reactive-ir/accessor-provenance/Anonymous.tsx`). Reopen when captured-parameter behavior and unknown callback timing compose across separate source summaries; Relay's `() => props.environment` is the first user.
- A shared reference/identity layer (`Resolved<T> | Unknown`), not started. Every false positive so far, under ADR 0326, was a proof re-deriving "is this still the same value" (props aliases, `arguments`, `delete`, redeclaration, reassigned tags, views of props). Reopen when the next proof needs that question answered.
- Recall fixes for the open oracle sites in section 1. The corpus yield is zero: `useParams()` destructuring occurs 0 times, and every `children()` helper accessor is read in JSX, never in a body. Reopen when a real project or report hits one.
- Inferring a `never` exit from a function body. A call completes normally unless its signature is `asserts` or returns `never`; a helper that throws undeclared is outside the model. Treating every helper call in a `try` as unproven would cost recall the owner declined (ADR 0279, option b). Reopen if a producer fact records inferred throws.
- Uncertifiable results are internal (sweeps, coverage, the oracle), never warnings or exit codes (ADR 0328).

## 3. Recorded precision trades

A violation withheld, or turned uncertifiable, to keep zero false positives. Each loses recall on purpose.

- Instance field initializer: a violation whose primary location is in one is demoted to uncertifiable, whichever rule claimed it (`diagnostics.rs`, `site_in_instance_field`). Static fields and static blocks keep their placement.
- Code in a component's element children (`<Show>`, `<Loading>`, `<Errored>`, portals, wrappers rendering `props.children`) has no proven placement (`inside_component_element_children`). A read there is an uncertifiable `strict-read-untracked`, even where a wrapper evaluates its children eagerly (`return props.children`) and the runtime warns.
- `reactive-handler-frozen` (SC1007) on an element inside a tracked JSX expression or a component's children is uncertifiable: the listener is replaced when the scope re-runs.
- `async-outside-loading-boundary` (SC5003) through a forwarding wrapper that reads `children` with no `Loading` of its own is unresolved (`loading_cover_at`); a wrapper that resolves `children()` outside any `Loading` is uncertifiable.
- `no-direct-mutation` (SC2003): a write through a 2.0 props object or a `merge`/`omit` view is uncertifiable. The compiler's props object is plain, so the write may land; only a proxy drops it.
- `no-destructure` (SC1003) with an unknown or mixed role is uncertifiable, not a proven snapshot.
- Code in or after a `for await` loop is past the first suspension, but whether the body runs is not proven: a read there in an async computation is an uncertifiable SC1002 (`after-await-suspended`), never a violation, though the runtime warns.
- SC1002 (ADR 0340) trades true positives for certainty. A read before the first await through a call not seen into (any callee outside the audited inert rows, a value, method, getter or inline callback) makes a later read uncertifiable, not a violation. The cost: `await fetchPosts()` over a `declare function` or package import yields no violation. Never-returning calls are found by their `never` signature only; a function that always throws without one is assumed to return. Props are not covered: a prop read before the await may share its getter's signal with a later prop read (`props.a`, `props.b` over one signal). Owner disposal is proven only through a `createRoot` `dispose` the code mentions; a parent that re-runs and disposes the computation, `render` disposal and `Object.defineProperty` getters are not seen.
- A signal read inside a JSX element passed as a prop does not make that prop reactive; reads of it are uncertifiable.
- A generator body is never placed where it is written. A call to a generator runs none of its body.
- A compound-component holder (`function Screen() { const Tabs = { Sub }; … }`), a holder in a project with a direct `eval` anywhere, or an `Object.assign` the project may replace stays under ADR 0331 and uncertifiable, as do the other refusals of ADR 0342. An unrelated `Object.defineProperty(param, …)` or `xs[key] = v` on an unresolved object withholds every `Object.assign` holder in the project (an accepted cost). Residual premise: `Object.assign` is not patched by code built from strings or an engine hook.
- A function a ref expression writes at its top level that no proof places is uncertifiable, never a violation (ADR 0345): a generator, a callback handed to a callee no dialect slot describes, or a function the expression evaluates without yielding it.
- A function returning JSX with no rendering tag is a proven component only where every reference to it is one ADR 0331's census accepts; other references leave its body uncertifiable. Audited route components (ADR 0334, 0335) and compound members that only JSX member tags reach (`Object.assign(Root, { List })`, ADR 0342) are read. Not followed: `Object.assign(F, ...)` as a statement or `F.List = ...`, a holder named in a type (`typeof Split`) or reached through a namespace import, and an identifier-valued nested namespace. A named function expression as a route component stays under the census. Not built: definitions or components built by a project call (`protect(X)`, `lazyScreen(...)`), `children` thunks, `2.0.0-next.21` (no corpus use; audit on demand), file routes (`fileRoutes(pageRoutes)`), and an exposure veto for TanStack (no corpus project reads a component key). TanStack's key-read veto is project-wide (ADR 0334): one `props.component` read withholds it. Solid makes a function a component only where `createComponent` calls it (`precision-map-renderer`, `precision-method-renderer`).
- A read, write or action no fact places (`ExecutionRole::Unknown`) is an uncertifiable finding of its rule, never silent; `Unreported` (several non-reporting roles, or past the strict-read window or a dominating await) stays silent. A write in `catch`/`finally` after an await is uncertifiable unless the `try` starts by awaiting a call of an async function with inert arguments (a synchronous throw otherwise reaches it unsuspended). The same holds for `resolve`/`until` there (SC2004/SC2005), for a read a helper called after an `await` makes on some paths only (SC1002; a straight-line one is a violation, ADR 0340), and for options the syntax cannot read beside an async computation (SC7002). A primitive used as a value is SC9012 (open shapes in section 1). A coverage rule disabled by options still blocks certification.
- SC1002 (ADR 0340) reports a read in a callback an exact standard method invokes inline, or in a promise handler or async helper the computation's promise carries, even where the receiver may be empty or the handler never settles: the same feasible-path premise as a direct read. Uncertifiable instead: a handler an `async` function returns unawaited over a receiver that may be settled (the runtime is silent there), a fulfillment handler on a receiver not proven to fulfill, a rejection handler, an unawaited `async` helper whose first await is not proven pending, and a `sort` comparator, a `reduce` without an initial value or a `replace` replacer.
- Mixed invocation of one callback (owned setup and event dispatch) refuses the owner proof as a whole (ADR 0327). A callback run only by events is proven unowned.
- `missing-owner` (SC4001): a call written as an event handler or a primitive's callback (`onClick={makeHandler(count)}`, `onClick={untrack(() => …)}`, `onClick={(() => …)()}`, `createEffect(compute, makeApply(count))`) names no callback, so a function literal inside it is not unowned and the call's own effects are silent. By design a call names its callee (ADR 0346), except identity wrappers such as `wrap(() => …)`. Silent too: any wrapper ADR 0346 refuses, a member-valued callback (`onClick={obj.handler}`), and `onClick={obj.handler as any}`. Open: a call-valued callee's returned callable (`make(() => {})(() => s(1))`, `const g = make(…); g(…)`) stays uncertifiable, and the leading-token reads in `solid-reactive-ir` (about 150 `entities.at`/`get` sites outside ADR 0346's identity reads) are not audited.
- `missing-owner` (SC4001), recall lost: a closed program treats an exported function nothing in the program calls as context-free. A `runWithOwner(null)` callback in it, and a ref IIFE inside that callback, is uncertifiable rather than a violation, though a render harness outside the call graph runs it. The owner graph does not count a value handed out by a namespace import (`Object.entries(mod)`) as a caller. Reuse ADR 0203's `entered_only_through_call_expressions` for an exported function's open seed (`owner_graph.rs`).
- `missing-owner` (SC4001), recall lost: an IIFE is not an owner edge. A plain IIFE that creates an effect is silent wherever it runs: at module scope, inside a `runWithOwner(null)` callback, or inside a function no edge reaches. Modeling it as a named callee needs the guards `runs_at_module_init` uses first: JSX children and props, and instance fields.
- `missing-owner` (SC4001): a ref IIFE inside a component's children is unproven, because the owner walk does not place children that run through the child getter (ADR 0345). A ref IIFE in a method or constructor that no resolved call names is uncertifiable for the same reason (`settle_eager_owners`), unless an owner region the compiler records answers it.

Known false-positive shapes left open (fire today, wanted gone):

- `missing-owner` (SC4001): a synchronous `dispatchEvent` or `click()` from an owned scope runs the handler under that owner; every event handler is taken to run with no owner. Oracle: `review-r7-sync-scroll`, `review-r7-sync-child-scroll`. Fixing it needs an owner-at-dispatch fact.
- `strict-read-untracked` (SC1001): `try { await p } finally { read() }` reports the read. Only a synchronous throw while evaluating `p` skips the await. Oracle: `r9-d3-tryawait`; none in the corpus.
- `strict-read-untracked` (SC1001): a function rendered by a tag the program never reaches (`<Show when={false}><Row/></Show>`) and also called plainly (`items().map(Row)`) keeps its component proof. Every visible tag counts as rendered, as for a direct read.
- `primitive-in-directive-application` (SC6001): `createRoot` in an apply callback is reported even when its `dispose` is called; the rule's premise is that a per-element root is never disposed.
- A read after `if (c) await` or a loop that may skip the await is reported: the feasible-path premise every direct read rests on. Oracle: `r9-d3-ifawait`, `r9-d3-loopawait`.

## 4. Open questions and live approximations

- ESLint adapter: native analyzes the file on disk, while the adapter maps byte offsets onto ESLint's in-memory text. An unsaved buffer, or a later `--fix` pass, places findings wrongly.
- WASM adapter: the request carries no package files, so every package import keeps an open requirement (SC9005, uncertifiable). Package source analysis exists only in the native check.
- Retained daemon: a tsconfig whose `extends` names a package with an `exports` map is never reused, since TypeScript's resolution is not reproduced (`host_config.rs`). Performance only.
- Owner-decision premises, not defects: standard runtime configuration (ADR 0266), standard hydration host (ADR 0268), member stability (ADR 0273), package integrity (ADR 0300), and the audited router bytes (ADR 0324, 0334, 0335). Residuals: computed or reflective route-option keys (ADR 0334); a bundle-time patch by a Vite plugin, or a `vite.config`-only alias without host resolution (ADR 0324); callables installed on `Object.prototype` (ADR 0335); a caller `function` that deletes or redefines a getter of its `this` when invoked as `props.onX()` (ADR 0337).
- Standard-library trust does not model getters, `toString`, Proxy on handed objects or thenables (ADR 0318).
- Route-parameter proofs stop at: `Match` accessors, `useSearchParams`, `useLocation`, `createMemo(() => params.k)`, and constants through barrels or `export *` (ADRs 0324, 0325). Each leaves an uncertifiable SC9012.
- `execution-map-incomplete` (SC9004) is retired: `uncovered_jsx_expressions()` is empty by construction. A third compiler adapter must prove the same totality in its own tests (`rust/dialects/solid-v2/compiler/src/lib.rs`).
- A mention of a `const`/`let`-bound function (a `var` is never credited) not proven to follow its declarator supplies no call site or edge (ADR 0339), so its findings are uncertifiable or absent. Misses by design: a call from a hoisted declaration nested in a function or from a method; a module-level declaration that code above the declarator may call, or whose module is on or imports a cycle; any call across a cycle. Also withheld (ADR 0339): a direct `eval`, an unanalyzed JavaScript module, a non-literal default parameter, and a `require` mention other than a direct call. A function declaration whose name is written elsewhere (`go = other`) reaches nothing by that name; a component or export root whose binding is written is no root. Accepted trade: `(x = late())` withholds the helper even when `late` is declared first.
- Recall instruments: `scripts/mutants.mjs` seeds fresh locals and inline handlers (P1–P9), signal accessors (E1), reactive props (R1), setup helpers (H1, H8) and named or imported intrinsic handlers (N2). Open: a helper bound by `let` or `var` is unreachable (a known miss). Reads through an `omit(props)` view or `const` alias, destructuring and component spreads stay uncertifiable (ADR 0337). Unsampled: async continuations, package callbacks, component-prop handlers. The runtime oracle skips patched or aliased packages, CommonJS export objects and declaration-only modules (list in `fixtures/runtime-oracle/README.md`).
- Static-only oracle rows `r9-e2-storedirectwrite` and `r9-e2-storeviewwrite` are real defects (the store drops the write) with no dev diagnostic. They prove nothing about false positives.
- `inferred-host-*` fixtures do not pin admission through a finding: package contracts are gone (ADR 0312), so `startClosed()` is opaque. Snapshot status shows admission (`uncertifiable`) or a decline (`certified`), each README names its twin, and Rust tests pin the `*-cases.json` inputs. Admission through `@solidjs/vite-plugin` is unreachable from a fixture; `ssr` grammar is covered by `host_config/tests.rs`. A finding-bearing admission case needs a package declaring an owner requirement, which the checker does not read.
