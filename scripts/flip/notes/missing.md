# Missing cases, ranked by likely defect yield

Job 2, 2026-11-05. Base commit 207b13142. Source of truth: the `missing: true` rows of `scripts/fuzz/surface.json` and `surface-state.json` (commit below). The rank is the census review's axis ranking (`adversarial.md` section 6). **Not one row below is a confirmed new defect unless it says so.** No checker was run for this list.

| rank | axis | rows | queue item | yield so far |
|---|---|---:|---|---|
| 1 | Value kind × path | 380 (246 product, 120 standard-library carriers, 14 others) | 8, 9, 10, 11, section 4 | 79 programs, 5 rules |
| 2 | Solid callback × closure layer | 74 | 3, section 4 | 19 false positives |
| 3 | Mount × claim | 40, plus 15 state sites | 2, 6, 11 (H-D5), section 4 | 1 false-positive class |
| 4 | Callee form | 7 | 11, 9 | 9 programs |
| 5 | Pending-source kind | 9 | 9, 11 | 5 programs |
| - | Sync DOM dispatch the checker's list lacks | 10 (inside rank 6's count) | **none** | not run; see below |
| 6 | Host callback kinds | 79 | none | 0 in 12 programs |
| 7 | JSX forms | 86 | none | 0 in 20 programs |
| 8 | Language and abstract operations | 62 | partly 4, 12 | only through rank 1 |
| 9 | Other public API, options, entries | 311 | none | none measured |

## Rank 1: value kind × path

**Generator:** `RECALL_VALUE_BINDINGS` and `CALLEE_FORMS` (`misuse.mjs`), `HANDOFF_FORMS` (`model.mjs`), as new bindings or hand-offs. Rows: `product/value/<kind>*<path>`, 6 kinds × 41 paths. Hold them to the review's rule: a path counts only when it crosses every kind.

| order | what to add | kinds | why first | queue |
|---|---|---|---|---|
| 1 | `map-get`, `nullish-tail`, `reassign`, `class-public-field`, `iife-return`, `weakref-deref` on **store-root and props** | store-root, props | member reads, not calls, so a call-side fix misses them (`v01`-`v24`); the one part with realistic corpus reach | 8 (held for the flip ruling), 10 |
| 2 | same paths on **handed-member** | handed-member | `v31`-`v38`; ADR 0509's net stops there | 9 (total `handed_value`) |
| 3 | class fields (5), containers (`map-*`, `set-values`, `weakmap-get`, `weakref-deref`, `proxy-wrap`, `globalthis`), `object-*`, `array-at`, `array-prototype-call` | accessor, setter, pending | D3's table: 51 of 51 reproduce | 8 |
| 4 | `reassign-*` (9 forms) | accessor, setter | block, `for`, `try`, `switch`, label: certified today. Do not reuse the control layers: those wrap the site, not the assignment | 8 |
| 5 | `or-tail`, `and-tail`, `nullish-tail`, `comma-tail`, `iife-return`, `generator-yield` | all | `?? x` 2,418 corpus sites; the largest uncertifiable cost | 8, 9 |
| 6 | `api-return-*` (`children`, `mapArray`, `repeat`, `createRoot`, `untrack`, `runWithOwner`) | accessor | F-D; `children`, `mapArray` and `repeat` have no generator at all | 9 |
| 7 | `await-promise-resolve` | accessor | `v49` | 9 |

Also rank 1 and not in any path list: the 120 `std/` rows (every `Array.prototype` callback and carrier, `Map`, `Set`, `WeakMap`, `Object.*`, `Promise.*`, `Reflect.*`, `Iterator` helpers). The list above came from the census's probes, so `flatMap`, `toSorted`, `Object.groupBy`, `Promise.try`, `Iterator.prototype.map` and the rest have never been asked. Add them as a value-carrier table in `model.mjs`, after 1-3 settle the mechanism.

## Rank 2: Solid callback × closure layer

**Generator:** a `nested-primitive` layer in `RECALL_LAYERS` (`misuse.mjs`), written in `model.mjs` beside the hand-off forms. Rows: `product/callback/<api>*<layer>`, 6 × 12. **Queue 3.** The `reconcile` export row and `callback/reconcile/key` go with it.

1. `createRoot`, `runWithOwner` × `named-timer`, `named-then`, `named-listener`, `factory-scheduled`: ordinary shapes, and the fix's real test.
2. `reconcile-key` × `inline-timer`, `host-property`, `after-await-iife`, `project-helper`.
3. `mapArray-map`, `repeat-map`, `untrack` × all: ADR 0443's deliberate stale read, so they pin the boundary. Today's generators cannot place them.
4. `body-iife`: the control. Must stay a true violation.

## Rank 3: mount × claim

**Generator:** `RECALL_MOUNTS` and the mount printer in `mounts.mjs`; `BODY_LABEL` in `matrix.mjs`. Rows: `product/mount/<mount>*<claim>`, 7 × 3.

1. `Dynamic` × `pending-read`, and `clientOnly-first-mount` × `pending-read`: live false positives (`d17`, `d19`). **Queue 2.**
2. `clientOnly-later-mount` × all three: `d06`, `d07`. Needs a mount after the module resolved, so a variant of `show-later`. **Queue 2.**
3. `Dynamic` × `body-read`: the hint. **Queue 2.**
4. `dynamicComponent`, `lazy-resolved`, `lazy-import`, `mapArray-render` × all: agree today (`j13`-`j17`), never crossed.

**New from the state table, in no queue item:**

| state site | what it shows | add |
|---|---|---|
| `dev.js#mapArray`, `#repeat` (`setStrictRead`) | `name` arms the mapper's window. `<For>` and `<Repeat>` pass it, so a hand `mapArray(list, fn)` body has **no window**. `option/mapArray/name` | `mapArray-render` with and without `name`: two mounts, not one |
| `dev.js#updateKeyedMap/runWithOwner#1`, `#updateRepeat/runWithOwner#1` | the fallback runs in its own owner inside the computation (`s05` throws on a write). `mapArray/fallback`, `repeat/fallback`, `Repeat/fallback` | a `fallback` context in `misuse.mjs` |
| `solid.dev.js#Switch/untrack#0`, `#2` | `<Match keyed>` with a function child: labelled, never generated. `overload/Match#0`, `#1`, `Match/keyed` | one `flow-match-keyed` context beside `flow-show-keyed` |
| `web.dev.js#staticElement` | `dynamic(() => "div")` string tag | low |
| `overload/For#2`, `option/For/keyed` | `For keyed={fn}`: the key runs only on an update, inside the computation (`f04`). The table's `For keyed` yes is `{false}` only | a `for-key-fn` mount |
| `web.dev.js#memo` | compiled ternary children are sync memos | rank 7 |

## Rank 4: callee form

**Generator:** `CALLEE_FORMS` (`misuse.mjs`). Rows: `product/callee/*`. **Queue 11** for `f()[0]()`, **queue 9** for `createMemo(fn)()`. The rule when adding them: a held `const` form must stay beside each, because `r02`/`r04` against `r01`/`r03` is the invariant 3 bug.

## Rank 5: pending-source kind

**Generator:** async templates in `SPEC` (`misuse.mjs`, `setup`, `variantSources`). Rows: `product/pending/*`, `export/solid-js/NotReadyError`, the two thenable `spec/` rows.

- `NotReadyError-throw`, `NotReadyError-helper`: **queue 11** (C-D4). Pin that SC1001 is uncertifiable there.
- `async-generator`: **queue 9**.
- `promise-of-accessor`: **queue 9**.
- `until-in-sync-memo`, `resolve-in-sync-memo`: **in no queue item**. The report proposed them and `C-D4`'s fix names them ("directly or through `until`/`resolve`").

## Sync DOM dispatch the checker's list lacks: not in the queue

The measured DOM loop (`scripts/fuzz/surface-dom-probe.mjs`) rediscovers everything in `synchronously_dispatching_dom_member` (`solid-reactive-ir/src/cleanup.rs:1257`) and these on top. Each ran listeners before the call returned, in Chrome 148. **Measured in the browser only; I did not run them through the checker**, so these are candidates.

| member | dispatches | state needed |
|---|---|---|
| `ChildNode.replaceWith` | `blur`, `focusout` | focused element |
| `Node.replaceChild` | `blur`, `focusout` | focused child |
| `ParentNode.replaceChildren` | `blur`, `focusout` | focused descendant |
| `HTMLSelectElement.remove` | `blur`, `focusout` | focused; `lib.dom`'s owner differs from `("ChildNode", "remove")`, so check the name match |
| `HTMLFormElement.submit` | `formdata` | any form |
| `WindowOrWorkerGlobalScope.reportError` | `error` | none |
| `History.go`, `Location.assign`/`reload`/`replace` | `beforeunload` | navigation; lowest of these |

`.focus()`'s shape, from the same family, found a real defect. **Generator:** the dispatch cases in `misuse.mjs` near line 1167 (`submit`, `validity`, `modal`). This is the best cheap item here: one case each. Also 37 rows are marked unmeasured: the probe could not call them (`Element.setAttributeNode` family, `showPicker`, `stepUp`), or skipped them because they end the page (`Document.write`, `open`, `close`, `Window.close`).

## Rank 6: host callback kinds

**Generator:** a closure-host table in `misuse.mjs`. Rows: `host/<Name>`, `host/*.on*`. A cheap smoke, per the review: one program per kind. `requestAnimationFrame`, `setInterval`, `MutationObserver`, `ResizeObserver`, `IntersectionObserver`, `PerformanceObserver`, `requestIdleCallback`, `on*` property assignment, and `customElements.define` (the only callbacks with an owner: `attributeChangedCallback`). **In no queue item.** Do it once rank 1-3 are done.

## Rank 7: JSX forms

**Generator:** `print.mjs`. 86 rows. The ones with a runtime branch, not just a spelling: `{ handleEvent }` objects, non-delegated `on*`, tuple on a non-delegated event, `class` and `style` objects, DOM spread (`<p {...x}>`: only component spread is generated), `prop:`, component `ref` and `on*` props called in a body, conditional children (the compiled `memo` above). The other attribute-expression kinds are spellings and rank last. **In no queue item.**

## Rank 8: language and abstract operations

**Generator:** layers in `model.mjs`. `__defineGetter__`-family: **queue 4**. Class getters and setters, `super`, private methods, `using`, `Symbol.*` protocols, `toJSON`, `toString`, `valueOf`, Proxy traps: uncertifiable today (`h13`-`h21`, `h27`). Their value is pinning that they stay so once the flip lands: add them with queue 8, not before. `namespace`, `enum`, decorators need generator work and found nothing.

## Rank 9: the rest

311 rows, no defect evidence. Options: `equals`, `unobserved`, `transparent`, `schedule`, `deferStream`, `ssrSource`, `name`, `loc`, projection `key`, `seedLoadingValue`, `until` `timeout` and `signal`. Exports: `Reveal`, `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`, `flatten`, `from`, `affects`, `createUniqueId`, `TimeoutError`, `enableExternalSource`, the hydration family, `useHead`, direct `@solidjs/signals` imports. Worth a look first: `transparent` (changes the owner chain), `schedule`, `merge(fn)` (a thunk becomes a memo, `dev.js#merge`), `<Errored>` reset (`dev.js#reset`), `until`'s `signal`. The two unclaimed codes (`RUN_WITH_DISPOSED_OWNER`, `UNCOMMITTED_INPUT_OVERWRITTEN`) are here too: each is a defect code with no rule.

## What the queue plans that this list holds

Queue section 4 and items 2, 3, 8-11 cover ranks 1-5 except: `until-in-sync-memo`, `resolve-in-sync-memo`, the `mapArray` `name` split, `fallback` contexts, `<Match keyed>`, `For keyed={fn}`. **Not in the queue at all:** the sync dispatch table, and ranks 6-9. Rank 1's first row waits on the flip ruling for its fix, not for its fuzz shape: the shape can go in now and sit in `known-open.json`.

I read `queue.md` only, and did not open the other agent's worktree, so a row here may already be in progress there.
