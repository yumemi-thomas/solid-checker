# Open work

Open items only: known misses, decisions not to build, accepted precision trades, live approximations.
A rule `tsc` reports is removed and logged in `fixtures/tsc-oracle/`. Delete an entry here when fixed or reversed.
Case ids name directories under `fixtures/runtime-oracle/`; each `oracle.json` `open` list is section 1's executable half (a listed site that stops firing fails as `stale-open`).

## 1. Known open detections

Defects the runtime reports and the checker does not. Each is a missing violation, not a false positive; the named rule should claim it. Codes are the runtime's.

Strict read in a setup body (`strict-read-untracked`, SC1001):

- `c2d-method-tracked` (App.tsx:19, :32): a function registered from a method or helper called in tracked JSX, then run by a later component body, keeps its registrant's lexical role. Same family: c2, c2c, c3d.
- `r10-destructured` (App.tsx:12): destructuring `useParams()` in setup reads the reactive parameter proxy, which the checker does not take as a reactive read.
- `r6-b6-classmethodopaque` (App.tsx:13): an opaque consumer invokes a class method during setup.
- `r9-b3-childrenreadinbody` (App.tsx:14): the accessor returned by `children()` is called in the component body.
- `r9-d1-mixmemoslotreadinbody` (App.tsx:7): a helper builds a memo from a callback and reads `m()` in the helper body.
- `review-opus2-c04-ctor-body` (App.tsx:12): a constructor read, class instantiated in the body.
- `review-opus3-g06-yield-star-spread-body` (App.tsx:13): a read in a generator reached through `yield*` and spread in the body. Delegation is not followed.
- `precision-ref-callback-read` (App.tsx:4): a read in an intrinsic `ref` callback runs untracked in the apply phase; the read rule places nothing in the `DirectiveApply` role.

- `@solid-primitives/utils` helpers over a getter, beyond `access(getter)` and `accessWith(getter)` (ADR 0338): `accessArray([getter])`, `withAccess(getter, fn)`, and `access` reached through another project helper's call hop are undecided, so they stay silent or uncertifiable (an unproven `access` argument is an obligation). An alias (`const alias = count`), a function literal (`access(() => count())`) or a props value is arity-unknown and uncertifiable. A getter passed to a helper that reads its parameter in a hoisted function it calls is silent (fixture `package-access-helper`, `MissedHoistedCallee`).

Write in an owned scope (`reactive-write-in-owned-scope`, SC2001):

- A named handler both attached as `onClick` and called directly in the component body hides the write in a helper it calls; the runtime warns at that call.
- A setter call in a component's default parameter initializer runs in the owned component scope; the write rule places nothing there.
- SC2001 in `render`/`hydrate` `code` off web rc.13/17 and in rc.17's deferrable `hydrate`: uncertifiable (ADR 0353). Signals rc.17: a typed-only or `let`-aliased setter may be `createOptimistic`'s (unguarded): unclaimed.

`flush` in an effect apply (`flush-in-effect-callback`, SC2008). The runtime reports these; the rule does not claim the open shapes below (ADR 0341). Positions and premises: rule page `flush-in-effect-callback`.

- `createRenderEffect` re-runs and a returned cleanup report only on some runs (its first run is outside any flush); the rule cannot tell which run a call is in, so neither is claimed.
- A helper with a handler, timer, module-level or exported caller, an alias or an argument use is not claimed, though its call from the effect is a no-op (the claim is universal, ADR 0341). `runtime-oracle/flush-in-effect-callback` lists the exported-helper case as `open`.
- A `flush` imported through another package's `exports` conditions, an apply passed by name or in a `{ effect }` bundle, and a helper in a callback given to an unmodelled consumer are not claimed.

Async read outside a loading boundary (`async-outside-loading-boundary`, SC5003):

- `r9-b3-childrenasync` (App.tsx:15): an async memo read flows through `children()` into JSX with no `Loading`.

Missing owner (`missing-owner`, SC4001):

- `review-r1-owner-mix` (App.tsx:5, :13): one callback invoked through `runWithOwner` and as an event handler (the owner walk refuses the literal as a whole; same shape `review-r3-owner-double-call`), and a handler returned from `createRoot`, later dispatched with no owner.
- `review-r5-owner-class-control`, `review-r6-owner-default-control`, `review-r6-owner-jsx-getter-control`: the call sits in a class field, a default parameter or a JSX getter, containers the owner walk refuses. `review-r7-event-array`: the typed `[handler, data]` event tuple is not followed by owner seeding.
- `review-opus5-t6-rest-timer-owner` (App.tsx:8): an effect created in a timer inside a `Show` callback. `review-opus2-o15-settled-class-field-new-in-timeout` (App.tsx:5): a class constructed in a timer; the field stays an uncertifiable SC3001.
- JSX in a `createEffect` apply (no `createRoot`), or in a helper creating a render effect that an apply calls, makes an unowned render effect: the runtime reports `NO_OWNER_EFFECT`, and the owner walk places nothing there (oracle `flush-in-effect-callback`).
- `oracle-behaviour-primitive-in-directive` (App.tsx:13): an effect in the apply a named factory returns (`ref={leakingTrack(count)}`) is SC6001 only. The owner walk places only a ref expression's top-level functions (ADR 0345), so a returned closure gets no owner claim; an IIFE returning one is uncertifiable for SC6001.
- Invoked literals the owner walk does not place: an IIFE in an instance field initializer (`ia-module`, App.tsx:17; a generator IIFE iterated by a spread, line 19); an IIFE in an object method or a constructor run by `new`, both reached from a timer (`ib-comp`, App.tsx:54, :58); an effect after an `await` in a loop or conditional, and an IIFE invoked after an `await` in its caller (`ic-await`, App.tsx:35–52, beyond the continuation rule of ADR 0333); and an IIFE called through `.bind(null)()` or a literal run by `Reflect.apply` (`ii1`, App.tsx:61, :65; ADR 0347 models only `.call`/`.apply`, so the runtime's `NO_OWNER_EFFECT` there is a missing finding).
- `Object.entries(ns)`/`Object.values(ns)` whose values the program calls: the functions reach it unnamed, so their detached callbacks are uncertifiable. Telling called from uncalled values needs invocation tracking (`owner-namespace-reachable`, `viaObject`, is the uncalled form).
- `missing-owner` (SC4001), recall lost: an unmodelled callback between leaves (`forEach`, `addEventListener`) misses `NO_OWNER_CLEANUP`; at probe k5 the wrongly coded SC3001 is gone.
- `missing-owner` (SC4001), recall lost: `onSettled(wrap(() => …))` (`solid2-precision`, `WrappedSettledCleanup`): a bodiless `wrap` is no identity wrapper (ADR 0346), so the cleanup in its literal goes unreported.
- rc.17 `PRIMITIVE_IN_EFFECT_CALLBACK` (a primitive made in an effect apply): no rule.

Possible owner suspension (ADR 0357): skippable-await cleanups retain SC4001 obligations. Call-argument-specific take/skip proofs are unimplemented; callback delivery outside the model remains open.

Missing provider (`missing-context-provider`, SC4002, ADR 0352): a context is claimed only when no provider exists in the program. Not claimed: a reader above no provider when another provider exists (needs the closed component tree), open programs, escaped contexts, package contexts (kobalte, router, query), `createContext` through an alias.

SC1002 (`after-await-open`): a handler on a promise held in a variable; a `.then` on a `Promise<T>` not proven native; `Promise.race` of helpers; an async IIFE the computation awaits; `Promise.all(x.map(async …))`; an accessor alias; a destructured or bound loader; router `query` flights; a catch join after statements; `Math.max(count())` after a functional setter. Each fires the runtime warning unproven (ADR 0340, 0349, 0350).


Primitive used as a value (`reactive-dispatch-unresolved`, SC9012):

- Found only through a named or namespace import. Not followed: a stored or aliased namespace object (`const NS = Solid; NS.createEffect(…)`), a namespace import of a project barrel, a computed member (`S["createEffect"]`), an `import S = require(…)` member. Fix: a census where every TypeFacts reference of a primitive's symbol that is not a primitive callee is an obligation. No corpus site has these shapes.

`runWithOwner` with a disposed owner (`RUN_WITH_DISPOSED_OWNER`, rc.13 `dist/dev-shared.js:6919-6932`) is claimed by no rule. Proof needs an owner written once from `getOwner()` in a `createRoot` body that disposes it unconditionally, and every `runWithOwner` using it running after that body. Not built: no corpus site has one.

Other:

- Synchronous DOM calls that dispatch (`focus`, `blur`, `click`, `dispatchEvent`, form `reset`/`requestSubmit`, popover and dialog toggles) are refused. Not modeled and still trusted: custom-element reactions on tree mutation and attribute writes, and focus loss when a focused element is removed (ADR 0329).
- An argument of unknown provenance handed to a callee that calls its parameter (`access(value)` over a spread, held alias, `Array.from`, `[get][0]`, `(() => get)()`, a helper's return or a store property) is an SC9012 obligation unless proven not a getter (ADR 0338, `Unproven.tsx`). A member call on a primitive result is uncertifiable even for `.toString()`; a project call's result is not proven a non-getter even when every return is a number (`access(give())`), since its returns are not followed.
- Patching the array iterator's `next` changes how tuples destructure (`const [value] = createSignal(1)`), which every tuple proof reads as slot order. ADR 0282's veto covers project tuples only, so such a patch is not seen (the access proofs share the premise, ADR 0338).
- A context-provided value read through a package hook (kui `TopicsRoute.tsx:425`) stays an uncertifiable SC9012. Only the router's own provider is proven; a project provider or an `Object.defineProperty` replacement of the location getter both type-check (ADR 0289, ADR 0312).
- `typeof window` in solid-query source blocks the proxy proof unless a Window realm is declared (ADR 0295, 0312).


Leaf body (`leaf-owner-forbidden-call`, SC3001):

- `leaf-owner-forbidden-call` (SC3001), recall lost: a primitive created under `untrack(() => …)` in a leaf body is not reported, though the runtime gives `PRIMITIVE_IN_FORBIDDEN_SCOPE`.

## 2. Decisions not to build

- A write is not proven legal because every invocation of its function is an event dispatch: a synchronous `click()`/`dispatchEvent` from an owned scope runs the handler under that owner. About 110 corpus SC2001 obligations stay open. Reopen with an owner-at-dispatch fact.
- Host-origin proof (DOM objects handed to Solid helpers, `makeEventListener(window, ...)`) needs the premise that project code never installs members on host DOM objects; app-game has 1,232 unknown-key writes. Reopen when a sweep shows a violation hinging on it and the owner states the premise.
- Anonymous arrows as their own summary owners: they detached captured parameter member calls from the enclosing function and dropped the SC9012 an opaque caller keeps (control: `fixtures/reactive-ir/accessor-provenance/Anonymous.tsx`). Reopen when captured-parameter behavior and callback timing compose across source summaries (Relay's `() => props.environment`).
- A shared reference/identity layer (`Resolved<T> | Unknown`) is not started. Under ADR 0326 every false positive so far was a proof re-deriving "is this still the same value" (props aliases, `arguments`, `delete`, redeclaration, reassigned tags, views of props). Reopen when a proof needs that answer.
- Recall fixes for the section 1 oracle sites: zero corpus yield (`useParams()` destructuring: 0 occurrences; every `children()` accessor is read in JSX). Reopen when a real project or report hits one.
- Inferring a `never` exit from a body: a call completes normally unless its signature is `asserts` or returns `never`, and a helper that throws undeclared is outside the model. Treating every helper call in a `try` as unproven would cost recall the owner declined (ADR 0279, option b). Reopen if a producer fact records inferred throws.
## 3. Recorded precision trades

A violation withheld, or turned uncertifiable, to keep zero false positives. Each loses recall on purpose.

- Instance field initializer: a violation whose primary location is in one is demoted to uncertifiable, whichever rule claimed it (`diagnostics.rs`, `site_in_instance_field`). Static fields and static blocks keep theirs.
- Code in a component's element children (`<Show>`, `<Loading>`, `<Errored>`, portals, wrappers rendering `props.children`) has no proven placement (`inside_component_element_children`); a read there is an uncertifiable `strict-read-untracked`, even where the runtime warns.
- `reactive-handler-frozen` (SC1007) on an element inside a tracked JSX expression or a component's children is uncertifiable: the listener is replaced when the scope re-runs.
- `async-outside-loading-boundary` (SC5003) through a forwarding wrapper that reads `children` with no `Loading` of its own is unresolved (`loading_cover_at`), as is a wrapper resolving `children()` outside any `Loading`.
- `no-direct-mutation` (SC2003): a write through a 2.0 props object or a `merge`/`omit` view is uncertifiable: the compiler's props object is plain, so only a proxy drops the write.
- `no-destructure` (SC1003) with an unknown or mixed role is uncertifiable, not a proven snapshot.
- Code in or after a `for await` loop: an uncertifiable SC1002 (`after-await-suspended`), never a violation, though the runtime warns.
- SC1002 (ADR 0340) trades true positives for certainty. A read before the first await, through a call not seen into (a callee outside the audited inert rows, a value, method, getter or inline callback), makes a later read uncertifiable: `await fetchPosts()` over a `declare function` or package import yields none. Coercive or iterating standard-library slots need proven primitives (ADR 0348). A function that always throws without a `never` signature is assumed to return. A prop read before the await may share its getter's signal with a later one. Owner disposal is proven only through a `createRoot` `dispose` the code mentions; a parent that re-runs and disposes the computation, `render` disposal and `Object.defineProperty` getters are not seen.
- A signal read inside a JSX element passed as a prop does not make that prop reactive; reads of it are uncertifiable.
- A generator body is never placed where written, and a call to a generator runs none of it.
- A compound-component holder (`function Screen() { const Tabs = { Sub }; … }`), a holder in a project with a direct `eval`, or an `Object.assign` the project may replace stays uncertifiable (ADR 0331, 0342). An unrelated `Object.defineProperty(param, …)` or `xs[key] = v` on an unresolved object withholds every `Object.assign` holder (an accepted cost). Residual premise: `Object.assign` is not patched by string-built code or an engine hook.
- A function a ref expression writes at its top level that no proof places is uncertifiable, never a violation (ADR 0345): a generator, a callback handed to a callee no dialect slot describes, or a function evaluated without yielding.
- A tagless JSX-returning function is a proven component only where every reference is one ADR 0331's census accepts; others leave its body uncertifiable. Read: audited route components (ADR 0334, 0335) and compound members only JSX member tags reach (`Object.assign(Root, { List })`, ADR 0342). Not followed: `Object.assign(F, ...)` as a statement or `F.List = ...`, a holder named in a type or reached through a namespace import, an identifier-valued nested namespace. Not built: components built by a project call (`protect(X)`, `lazyScreen(...)`), `children` thunks, `2.0.0-next.21`, file routes (`fileRoutes(pageRoutes)`), and a TanStack exposure veto (one `props.component` read withholds it, ADR 0334). A component is a function `createComponent` calls (`precision-map-renderer`, `precision-method-renderer`).
- A read, write or action no fact places (`ExecutionRole::Unknown`) is an uncertifiable finding of its rule, never silent; `Unreported` (several non-reporting roles, or past the strict-read window or a dominating await) stays silent. A write in `catch`/`finally` after an await is uncertifiable unless the `try` starts by awaiting an async call with inert arguments (a synchronous throw otherwise reaches it unsuspended). So are `resolve`/`until` there (SC2004/SC2005), a read a helper makes after an `await` on some paths only (SC1002; a straight-line one is a violation, ADR 0340), and options the syntax cannot read beside an async computation (SC7002). A coverage rule disabled by options still blocks certification.
- SC1002 (ADR 0340) reports a read in an inline callback, a promise handler or an async helper the computation's promise carries, even on an empty receiver. Uncertifiable: a handler an `async` function returns unawaited over a possibly settled receiver, a fulfillment handler on an unproven receiver, a rejection handler, an unawaited helper whose first await is unproven, a `sort` comparator, a `reduce` without an initial value, and a `replace` replacer.
- Mixed invocation of one callback (owned setup and event dispatch) refuses the owner proof as a whole (ADR 0327). A callback run only by events is proven unowned.
- `missing-owner` (SC4001): a call written as an event handler or a primitive's callback (`onClick={makeHandler(count)}`, `onClick={untrack(() => …)}`, `createEffect(compute, makeApply(count))`) names no callback, so a literal inside it is not unowned and the call's own effects are silent. A call names its callee (ADR 0346) except identity wrappers such as `wrap(() => …)`; wrappers ADR 0346 refuses stay unplaced. Exact member/tuple and intrinsic-spread literals are placed (ADR 0358); unresolved holders, getters, generators and unaudited web keys retain obligations. Joined creations (ADR 0359) exclude parameter-decided branches. Open: a call-valued callee's returned callable (`make(() => {})(() => s(1))`) stays uncertifiable, and ~150 leading-token `entities.at`/`get` sites are unaudited.
- `missing-owner` (SC4001), recall lost: in a closed program an exported function that nothing calls, and no namespace or dynamic-import read hands out, has no caller, so a `runWithOwner(null)` callback in it, and a ref IIFE inside that, is uncertifiable rather than a violation (`docs/rules/missing-owner.md`, closed-program paragraph). Other escapes leave exports dormant too: a named import passed as a value (`render(fn)`, `[fn]`), an anonymous default export, a barrel's re-export read through an unresolved namespace. Reuse ADR 0203's `entered_only_through_call_expressions` for the open seed (`owner_graph.rs`).
- `missing-owner` (SC4001): a ref IIFE inside a component's children is unproven, because the owner walk does not place children that run through the child getter (ADR 0345). A ref IIFE in a method or constructor that no resolved call names is uncertifiable for the same reason (`settle_eager_owners`), unless an owner region the compiler records answers it.
- `missing-owner` (SC4001), recall lost: an IIFE inside a function no owner edge reaches (a method or constructor no resolved call names, an unexported function nothing calls) is silent, since that function's context is zero. Module-scope and `runWithOwner(null)` IIFEs are reported (ADR 0347; oracle probes p3 N1, p4 V5).
- `missing-owner` (SC4001), uncertain reach (ADR 0347): a detached callback in an export that only an unnamed namespace escape reaches (`Object.keys(ns)`, passed along, spread, an alias, `export * as`, `require`, an unrecognized dynamic-import shape) is uncertifiable: whether the export runs is unproven. Recall is lost where the runtime warns and the program calls such an export (the `Object.entries` case in section 1).

Event delivery (ADR 0356): native `dispatchEvent`/`click` withholds event owner/execution proofs program-wide, including unrelated handlers. Target-specific delivery and other DOM emitters remain unmodelled.

SC6001 root lifetime (ADR 0355): explicit roots in ref apply remain uncertifiable, including disposed and empty roots. Proving live work and non-disposal is not implemented.

Recall trade (ADR 0354): first-iteration reads before a loop await and reads after an awaiting switch case remain uncertifiable.

Known false-positive shapes left open (fire today, wanted gone):

- `strict-read-untracked` (SC1001): a function rendered by a tag the program never reaches (`<Show when={false}><Row/></Show>`) and also called plainly (`items().map(Row)`) keeps its component proof.
- `leaf-owner-forbidden-call` (SC3001), precision (minor): a site in a dead tracked-effect body is claimed although the body never runs, when every path to its creation is an owned leaf body (probe m4, line 8).

## 4. Open questions and live approximations

- ESLint adapter: native analyzes the file on disk, while the adapter maps byte offsets onto ESLint's in-memory text. An unsaved buffer, or a later `--fix` pass, places findings wrongly.
- WASM adapter: the request carries no package files, so every package import keeps an open requirement (SC9005, uncertifiable). Package source analysis exists only in the native check.
- Retained daemon: a tsconfig whose `extends` names a package with an `exports` map is never reused, since TypeScript's resolution is not reproduced (`host_config.rs`). Performance only.
- Premises, not defects: standard runtime configuration (ADR 0266), standard hydration host (ADR 0268), member stability (ADR 0273), package integrity (ADR 0300), audited router bytes (ADR 0324, 0334, 0335). Residuals: computed or reflective route-option keys (ADR 0334); a bundle-time patch by a Vite plugin or a `vite.config`-only alias (ADR 0324); callables on `Object.prototype` (ADR 0335); a caller that deletes or redefines a getter of its `this` when invoked as `props.onX()` (ADR 0337).
- Standard-library trust does not model getters, `toString`, Proxy on handed objects or thenables (ADR 0318).
- Route-parameter proofs stop at: `Match` accessors, `useSearchParams`, `useLocation`, `createMemo(() => params.k)`, and constants through barrels or `export *` (ADRs 0324, 0325). Each leaves an uncertifiable SC9012.
- `execution-map-incomplete` (SC9004) is retired: `uncovered_jsx_expressions()` is empty by construction. A third compiler adapter must prove the same totality in its tests (`rust/dialects/solid-v2/compiler/src/lib.rs`).
- A mention of a `const`/`let`-bound function not proven to follow its declarator supplies no call site or edge (ADR 0339; a `var` is never credited), so its findings are uncertifiable or absent. Misses by design: a call from a hoisted declaration nested in a function or from a method; a module-level declaration that code above it may call, or that is on or imports a cycle; any call across a cycle. Also withheld: a direct `eval`, an unanalyzed JavaScript module, a non-literal default parameter, a `require` mention other than a direct call. A function declaration written elsewhere (`go = other`) reaches nothing by that name, and a written component or export root is no root. Accepted trade: `(x = late())` withholds the helper even when `late` is declared first.
- Recall instruments: `scripts/mutants.mjs` seeds fresh locals, inline and intrinsic handlers, signal accessors, reactive props and setup helpers. Known miss: a helper bound by `let` or `var`. Reads through an `omit(props)` view or `const` alias, destructuring and component spreads stay uncertifiable (ADR 0337); async continuations, package callbacks and component-prop handlers are unsampled. The oracle skips patched or aliased packages, CommonJS exports and declaration-only modules (`fixtures/runtime-oracle/README.md`).
- Static-only oracle rows `r9-e2-storedirectwrite` and `r9-e2-storeviewwrite` are real defects (the store drops the write) with no dev diagnostic; they prove nothing about false positives.
- `inferred-host-*` fixtures pin no finding: package contracts are gone (ADR 0312), so `startClosed()` is opaque. Snapshots show admission (`uncertifiable`) or a decline (`certified`); Rust tests pin the `*-cases.json` inputs. `@solidjs/vite-plugin` admission is unreachable from a fixture; `ssr` grammar is in `host_config/tests.rs`. A finding-bearing admission needs a package declaring an owner requirement, which the checker does not read.
- `lazy` identity: Solid's `lazy` is recognized from its import binding (`import { lazy } from "solid-js"`, named in the dialect's export table), since typefacts does not name the overloaded declaration. Any other module's `lazy` is not Solid's: its loader's `import()` is an unnamed escape, so that module's exports are uncertifiable.
