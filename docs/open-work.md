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
- `review-opus2-c18-field-iife-body-new` (App.tsx:12): an instance field initializer run by `new K()` in the body. Construction sites are not followed.
- `review-opus3-g06-yield-star-spread-body` (App.tsx:13): a read in a generator reached through `yield*` and spread in the body. Delegation is not followed.

Async read outside a loading boundary (`async-outside-loading-boundary`, SC5003):

- `r9-b3-childrenasync` (App.tsx:15): an async memo read flows through `children()` into JSX with no `Loading`.

Missing owner (`missing-owner`, SC4001):

- `review-r1-owner-mix` (App.tsx:5): one callback invoked through `runWithOwner` and as an event handler. The owner walk refuses the literal as a whole. Same shape: `review-r3-owner-double-call`.
- `review-r1-owner-mix` (App.tsx:13): a handler returned from `createRoot`, later dispatched with no owner.
- `review-r5-owner-class-control`, `review-r6-owner-default-control`, `review-r6-owner-jsx-getter-control`: the call sits inside a class field, a default parameter, or a JSX getter; the owner walk refuses those containers.
- `review-r7-event-array`: the typed `[handler, data]` event tuple is not followed by owner seeding.
- `review-opus5-t6-rest-timer-owner` (App.tsx:8): an effect created in a timer inside a `Show` callback.
- `review-opus2-o15-settled-class-field-new-in-timeout` (App.tsx:5): a class constructed in a timer. The field stays an uncertifiable SC3001.

Other:

- Synchronous DOM calls that dispatch (`focus`, `blur`, `click`, `dispatchEvent`, form `reset`/`requestSubmit`, popover and dialog toggles) are refused. Not modeled and still trusted: custom-element reactions on tree mutation and attribute writes, and focus loss when a focused element is removed (ADR 0329).
- A context-provided value read through a package hook (kui `TopicsRoute.tsx:425`) stays an uncertifiable SC9012. The router source proves its own provider only; a project provider or an `Object.defineProperty` replacement of the location getter both type-check (ADR 0289, ADR 0312).
- `typeof window` in solid-query source blocks the proxy proof unless a Window realm is declared; two spotify-desk-thing `prefer-show` rows were lost that way (ADR 0295, ADR 0312).

Not a miss: `r4c-leak` line 43 is listed open only because the runtime names the JSX read of the memo while the checker's uncertifiable SC5003 sits at the async read two lines above. The oracle has no frame linking them.

## 2. Decisions not to build

- Host-origin proof (DOM objects handed to Solid-package helpers, `makeEventListener(window, ...)`). Built as a prototype: 29 uncertifiable removed, 0 violations moved. It needs a premise that project code never installs members on host DOM objects, and app-game has 1,232 unknown-key writes. Reopen when a sweep shows a violation, not an uncertifiable, that hinges on it, and the owner states that premise.
- Anonymous arrows as their own summary owners. It detached captured parameter member calls from the enclosing function and silently dropped `execute(input) { return invoke(() => input.callback()); }`, which must keep SC9012 at an opaque caller (control in `fixtures/reactive-ir/accessor-provenance/Anonymous.tsx`). Reopen when captured-parameter behavior and unknown callback timing compose across separate source summaries; Relay's `() => props.environment` is the first user.
- A shared reference/identity layer (`Resolved<T> | Unknown`). Not started. Every false positive in the seven ADR 0326 review rounds was a proof re-deriving "is this still the same value" (props aliases, `arguments`, `delete`, redeclaration, reassigned tags, views of props). Reopen when the next proof needs that question answered.
- Recall fixes for the open oracle sites in section 1. The yield is zero on the 49-project corpus: `useParams()` destructuring occurs 0 times; every `children()` helper accessor there is read in JSX, never in a body. Reopen when a real project or report hits one.
- Inferring a `never` exit from a function body. A call completes normally unless its selected signature is `asserts` or returns `never`; a helper that throws without declaring it is outside the model. The alternative, treating every helper call inside a caller's `try` as unproven, costs recall the owner declined (ADR 0279, option b). Reopen if a producer fact records inferred throws.
- Showing users uncertifiable results. They are internal (sweeps, coverage, the oracle), never warnings or exit codes (ADR 0328).

## 3. Recorded precision trades

A violation withheld, or turned uncertifiable, to keep zero false positives. Each loses recall on purpose.

- Instance field initializer: every violation whose primary location is in one is demoted to uncertifiable, whichever rule claimed it (`diagnostics.rs`, `site_in_instance_field`). Static fields and static blocks keep their placement.
- Code in a component's element children (`<Show>`, `<Loading>`, `<Errored>`, portals, wrappers rendering `props.children`) has no proven placement (`inside_component_element_children`); `strict-read-untracked` is silent there. A wrapper that evaluates its children eagerly in its own body (`return props.children`) makes the runtime warn at a read in those children, and that read is uncertifiable too.
- `reactive-handler-frozen` (SC1007) on an element inside a tracked JSX expression or a component's children is uncertifiable: the listener is replaced when the scope re-runs.
- `async-outside-loading-boundary` (SC5003) through a forwarding wrapper that reads `children` with no `Loading` of its own is unresolved (`loading_cover_at`); a wrapper that resolves `children()` outside any `Loading` is uncertifiable.
- `no-direct-mutation` (SC2003): a write through a 2.0 props object or a `merge`/`omit` view is uncertifiable. The compiler's props object is plain, so the write may land; only a proxy drops it.
- `no-destructure` (SC1003) with an unknown or mixed role is uncertifiable, not a proven snapshot.
- Code in or after a `for await` loop is past the first suspension; no strict read is proven.
- A signal read inside a JSX element passed as a prop does not make that prop reactive; reads of it are uncertifiable.
- A generator body is never placed where it is written. A call to a generator runs none of its body.
- Mixed invocation of one callback (owned setup and event dispatch) refuses the owner proof as a whole (ADR 0327). A callback run only by events is proven unowned.

Known false-positive shapes left open (fire today, wanted gone):

- `missing-owner` (SC4001): a synchronous `dispatchEvent` or `click()` from an owned scope runs the handler under that owner; every event handler is taken to run with no owner. Oracle: `review-r7-sync-scroll`, `review-r7-sync-child-scroll`. Fixing it needs an owner-at-dispatch fact.
- `strict-read-untracked` (SC1001): `try { await p } finally { read() }` reports the read. Only a synchronous throw while evaluating `p` skips the await. Oracle: `r9-d3-tryawait`; none in the corpus.
- A read after `if (c) await` or a loop that may skip the await is reported: the feasible-path premise every direct read rests on. Oracle: `r9-d3-ifawait`, `r9-d3-loopawait`.

## 4. Open questions and live approximations

- ESLint adapter: native analyzes the file on disk; the adapter maps byte offsets onto ESLint's in-memory text. An unsaved editor buffer, or a later `--fix` pass, places findings wrongly.
- WASM adapter: the request carries no package files, so every package import keeps an open requirement (SC9005, uncertifiable). Package source analysis exists only in the native check.
- Retained daemon: a tsconfig whose `extends` names a package with an `exports` map is never reused, because that resolution is TypeScript's and is not reproduced (`host_config.rs`). Performance only.
- Premises that are owner decisions, not defects: standard runtime configuration (ADR 0266), standard hydration host (ADR 0268), member stability (ADR 0273), package integrity (ADR 0300), the audited router bytes (ADR 0324; residual: a patch applied only at bundle time by a Vite plugin, or a `vite.config`-only alias without host resolution).
- Standard-library trust does not model getters, `toString`, Proxy on handed objects or thenables (ADR 0318).
- Route-parameter proofs stop at: `Match` accessors, `useSearchParams`, `useLocation`, `createMemo(() => params.k)`, and constants through barrels or `export *` (ADRs 0324, 0325). Each leaves an uncertifiable SC9012.
- `execution-map-incomplete` (SC9004) is retired because `uncovered_jsx_expressions()` is empty by construction. A third compiler adapter must prove the same totality in its own tests (`rust/dialects/solid-v2/compiler/src/lib.rs`).
- Recall instruments cover only direct shapes. `scripts/mutants.mjs` seeds fresh local bindings and inline handlers (459 sites, all proven). The next round should seed through existing bindings, imported handlers, wrappers and async. The runtime oracle skips cases it cannot represent: patched or aliased packages, CommonJS export objects, declaration-only modules (list in `fixtures/runtime-oracle/README.md`).
- Static-only oracle rows `r9-e2-storedirectwrite` and `r9-e2-storeviewwrite` are real defects (the store drops the write) with no dev diagnostic. They prove nothing about false positives.
- About 33 package-consumer fixtures went vacuous when contracts were removed: their snapshots hold only an SC9014 notice (about 18) or uniform SC9011/SC9012 findings (about 15), so they no longer pin the claim their name states. Some still carry dead contract files (`solid-reactivity.json`, `expected-claims.json`, `.solid-checker/accepted-contracts.json`). Each needs a real dependency-source case or deletion.
