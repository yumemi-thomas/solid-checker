# Runtime audit: Solid 2 timing, tracking, ownership and guards, `2.0.0-rc.9` to `2.0.0-rc.13`

Scope: the redo of sections 2 (Method), 3.3 (Timing, tracking and ownership) and
3.4 (Guards and diagnostics the rules mirror) of
`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`,
with rc.9 as the baseline and rc.13 as the release under review. Read-only
review: nothing in the repository was changed; every file this review created is
under `rust/target/audit-rc13/runtime/`.

Every claim is tagged **[M]** (measured: a cited file and line, a digest, or
command output on the named bytes) or **[E]** (estimated: inferred, or read in
source but not executed). Where something could not be measured, it says so.

## 0. Bottom line

1. **Every timing, tracking, ownership and write-guard premise in the § 3.3 table
   still holds on rc.13.** Most cited slices are byte-identical; the ones that
   changed differ only in internals the rules do not read, except where section 1
   says otherwise. All 134 probes were run on rc.9 and rc.13, dev and prod; the
   only outcome differences are the ones listed in section 3.4.3 [M].
2. **All seven release-dependent answers in `releases.rs` (B1, B2, B3, B4, N3, N4,
   N5) are the same on rc.13 as `Solid2::RC9` states them** [M, section 3.5]. rc.13
   needs no new vocabulary axis for them. It does need to be named: the dialect
   today treats every release after rc.9 as "not compared" and answers
   conservatively (`releases.rs:39`) [M].
3. **There is one new runtime guard that a rule should mirror (row 1), a few
   runtime behaviour changes that rule pages or dialect premises should state
   (rows 2, 4, 7), and two new guards no rule mirrors (rows 5, 6).** See the table
   below.

## 1. Summary: items needing a dialect change, or a new guard the rules mirror

Classes follow the template: U unchanged, X export moved, S signature, R semantic
change affecting a rule, I irrelevant. "Guard" means a runtime diagnostic or throw.

| # | Item | Class | One-line evidence | What it needs |
| --- | --- | --- | --- | --- |
| 1 | **New dev guard `UNTRACKED_READ_AFTER_AWAIT`.** A read of a reactive value after the first `await` of an async computation warns in dev. It is the runtime twin of SC1002 `reactive-read-after-await` (`docs/rules/reactive-read-after-await.md`). | R (new guard the rule mirrors) | `dev-shared.js:1022` (check `:968-1037`), absent from rc.9 and from every `dist/prod/**` and `dist/observe/**` file [M]. Probes `PA_memo`, `PA_effect_compute`, `PA_store`, `PA_helper`, `PA_projection`, `PA_latest`, `PA_second_await`: no warning on rc.9, warning on rc.13 dev, nothing on prod [M]. | Cite it from the rule page. No vocabulary row. Check that SC1002's exemptions agree with the runtime's (section 3.4.2) [E]. |
| 2 | **`lazy`-loaded component bodies now run in a labelled strict-read window under a root owner.** rc.9 ran them as `untrack(() => Comp(props))` with no label. rc.13 calls `createComponent(Comp, props)`. | R (premise of SC1001 and of `component_body_runs_under_root`) | `solid.dev.js:1401-1432` (rc.9 `:1151-1187`) [M]. Probe `LAZY_component_body_read`: no `STRICT_READ_UNTRACKED` on rc.9, one on rc.13 dev. Probe `LAZY_component_body_owner`: owner is the wrapping memo (`_root:false`, name `computed`) on rc.9 and a transparent root (`_root:true`, name `<default>`) on rc.13 [M]. | Rule page `strict-read-untracked.md` says the runtime installs strict windows in component bodies. That is true for lazy-loaded components only from rc.13 [M]. No finding changes direction: the read is untracked either way [E]. If SC1001 should be release-aware for lazy components, it is a new axis. |
| 3 | **Dialect release gate.** rc.13 falls in the "anything else (rc.10+ ...)" row, which answers with the conservative vocabulary. | dialect change (no semantic change) | `releases.rs:39`; `AUDITED_INSTALLATION` `releases.rs:120-124` names rc.9 [M]. The seven answers all equal `Solid2::RC9` (`releases.rs:282-290`) on rc.13 bytes, section 3.5 [M]. | Move or extend `AUDITED_INSTALLATION` and the table. `AUDITED_ARCHIVES` (`solid_2.rs:239`) has no rc.13 tuple, so every archive-scoped negative row answers nothing for rc.13 until re-audited [E, not measured here]. rc.10 to rc.12 were not read by this review. |
| 4 | **`Errored` now calls any function fallback**, including a zero-parameter one (`typeof f === "function" ? f(err, reset) : f`). rc.9 returned a zero-parameter function as a value. | I today, X-watch | `solid.dev.js:1536-1542` and `solid.js:1472-1477` (rc.9 `:1290-1296`) [M]. Probe `ERRORED_fallback_arity0`: fallback called 0 times on rc.9, 1 time on rc.13, dev and prod [M]. | No dialect row models `Errored`'s fallback (`Primitive::Errored` is in no callback table; `boundary_kind` `solid_2.rs:5275` only) [M]. Nothing to change today. A future fallback row must be release-aware. |
| 5 | **New dev guard `LOADING_ON_OUTSIDE_HOLD`** (`createLoadingBoundary(..., { on })` re-arms while the same source is also awaited outside the boundary). | new guard, no rule mirrors it | `dev.js:9974`, trigger `:9795` [M]. Probe `LOADING_ON_OUTSIDE_HOLD`: warns on rc.13 dev only [M]. The `on` option itself is in rc.9 (`createLoadingBoundary` slice byte-identical) [M]. | None required. Rule opportunity only; the condition is structural, so a static proof is non-trivial [E]. |
| 6 | **Server `dynamic` throws `DYNAMIC_ASYNC_COMPONENT`** when an async source resolves to a client component function. | new throw, no rule mirrors it | `@solidjs/web` `server.dev.js:4757-4770`, `server.js:4341-4343`, `server.observe.js:4496-4498` (all tiers); absent from rc.9 [M by source]. Not executed (server runtime) [E]. | Rule opportunity, adjacent to SC2007 `static-dynamic-async-source`. TypeScript does not report it (the source type admits `Promise<T>`) [E]. |
| 7 | **Cleanup order reversed.** Disposal lists now run last-registered first. | I (no rule models order) | `runDisposal` `dev-shared.js:3251-3279` (rc.9 `:2754-2768`), server `disposeOwner` `server.js:169-210` [M]. Probes `CLEANUP_ORDER`, `CLEANUP_ORDER_MEMO`, `CLEANUP_PARENT_CHILD`, `CLEANUP_REENTRANT`: order flips, dev and prod [M]. A grep of `docs/rules`, `solid-reactive-ir/src`, `solid-dialect/src` and `rust/dialects` for ordering claims found none [M]. | Nothing. Record it: a contract or rule that ever states cleanup order has to be release-aware. |
| 8 | **Root typings no longer re-export `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`, `sharedConfig`, `$DEVCOMP`** (declared only in `solid-js/internal`), while the runtime root still exports them. | X (declaration) | `solid-js/types/index.d.ts` (rc.9 lines 3 and 8 listed all five, the N2 broken re-exports; rc.13 lists none); `types/internal.d.ts:65-144` [M]. Runtime root namespace still has all five [M]. The dialect's export table still maps them to `solid-js`: `exports/solid_v2_solid_js.rs:18,42,43,51,87` [M]. | Outside this review's runtime scope. A named import of these from the root is TS2305 on rc.13 [E, from the declarations; `tsc` not run], so the checker must stay silent there. |

Not a finding: no write guard, ownership rule, `STRICT_READ_UNTRACKED` window, effect
timing, `untrack`, `runWithOwner`, `resolve`, `flush`, `onSettled`, `createReaction`,
store setter or `refresh` answer moved (section 3.3 and the rows under it).

## 2. Method

1. **Bytes** [M]. rc.9 is `rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/{solid-js,@solidjs/signals,@solidjs/web}`. rc.13 is
   `rust/target/audit-rc13/node_modules/{solid-js,@solidjs/signals,@solidjs/web}`.
   - `package.json` sha256, rc.9: `solid-js` `c8d6224b...e11bc`, signals `c612461c...bb1c6`, web `5de9244e...773dc`. These equal the `manifest_sha256` values in `AUDITED_ARCHIVES` (`solid_2.rs:239`), so the rc.9 tree is the audited one [M].
   - `package.json` sha256, rc.13: `solid-js` `ce43a022f763e5995d1ac8a5f78f135804edd01b627a2bcbee8ff9530e79d284`, signals `6783c3c632cfebf3a0fc26e18925887462b5925c481c6535ae0186845efb95a6`, web `8b45ed71ed7a369883e7e00bb48b01fd7bda935bd5ba889cbecafc8eaa122bf5`. File counts 34, 119, 75 (rc.9: 34, 120, 68) [M].
   - rc.13 integrities are those in `audit-rc13/node_modules/.package-lock.json`: `solid-js` `sha512-62bYOI4J...BdoQ==`, signals `sha512-4+pRdrAH...MYeQ==`, web `sha512-vI/7v/XM...gOQ==`. No tarball was downloaded or re-derived [E: not re-derived].
   - rc.13 `solid-js` depends on `@solidjs/signals: ^2.0.0-rc.13` and `seroval ~1.6.7` (rc.9: `^2.0.0-rc.9`, same seroval); `@solidjs/web` peers `solid-js ^2.0.0-rc.13` [M].
   - Main dev bundles (sha256-12, bytes), rc.9 to rc.13: signals `dev.js` `f08c227c5c64` 418,037 to `8dc03b109003` 464,911; `dev-shared.js` `70b88ba97dcb` 295,301 to `c19016529420` 340,007; `solid.dev.js` `7baf8808f6bd` 45,011 to `0489d4c57801` 52,305; `web.dev.js` `bcbe02189e31` 86,356 to `bd417d367b53` 91,395 [M].
   - Signals `dist/` has the same 116 files in both versions; 79 differ, 37 are identical. 25 of the 34 `dist/prod/**` files differ [M].
2. **Function slices** [M]. The TypeScript 5.9.3 parser (`packages/cli/node_modules/typescript`) extracted every top-level function, class and variable statement of each bundle (`runtime/slices/slices.mjs`). Slices were compared by name, with sha256-16 of the exact statement text; the digest quoted below is that. A second normalised digest strips `$N` identifier suffixes; no slice differed by rename only. Results: signals dev (`dev.js` + `dev-shared.js`) 570 identical, 95 changed, 66 added, 8 removed; `solid.dev.js` 105 / 22 / 22 / 1; `web.dev.js` 202 / 11 / 13 / 0. Raw table: `runtime/slices/out/compare.json`. Diffs were read with `runtime/slices/show.mjs`. Prod bundles are minified, so prod was compared per file by sha256 and by probe behaviour, not by slice [M].
3. **Probes** [M]. 134 probes (appendix, `runtime/probes/probes.mjs`), each run in its own process by `runtime/probes/runall.mjs` on four configurations:
   - Node `the vite-plus-managed Node 24.21.0` (v24.21.0).
   - Dev: `--conditions=browser --conditions=development`. Prod: `--conditions=browser`. The `browser` condition is required: without it the `node` condition selects the server builds (`server.dev.js` / `server.js`) of `solid-js` and `@solidjs/web` [M].
   - Probe projects are `runtime/probes/rc9/` and `runtime/probes/rc13/`, each with a `node_modules` symlink to that version's `node_modules`. Signals resolves from `solid-js` in both [M].
   - Files actually loaded, printed by every probe with `import.meta.resolve` [M]: rc.9 dev `@solidjs/signals/dist/dev.js` + `solid-js/dist/solid.dev.js`; rc.9 prod `@solidjs/signals/dist/prod/index.js` + `solid-js/dist/solid.js`; rc.13 dev and prod the same relative paths under the rc.13 tree.
4. **Dialect line numbers** are the current lines of `rust/crates/solid-dialect/src/solid_2.rs` (read only), or `solid_2/releases.rs` where named. They differ from the rc.9 review's, because that file has grown to 10,017 lines.
5. **Not done**: see section 6.

## 3.3 Timing, tracking and ownership: rc.9 to rc.13

Slice digests are sha256-16 of the extracted statement. Locations are `file:line-range`; `sig` is `@solidjs/signals/dist`, `js` is `solid-js/dist`, `web` is `@solidjs/web/dist`. "same" means the digest is equal on both versions.

| assumption | dialect source (current line) | rc.13 evidence | class |
| --- | --- | --- | --- |
| Non-lazy `computed` runs its compute during the creating call: `createMemo`, `createSignal(fn)`, `createOptimistic(fn)`, `createProjection`, `createStore(fn)` | `tracked_callback_timing` `:5904`, `callback_executions` `:5723`, `contract_callback_execution_at` `:5848` | `!options?.lazy && recompute(self, true)` at `sig/dev-shared.js:5651` (rc.9 `:5005`); prod `sig/prod/core/core.js:914` [M]. Slices: `createMemo` `95be10c7`, `createSignal` `f315ef56`, `createOptimistic` `9180aeea`, `optimisticComputed` `6b903ed9` all same [M]. `setupComputedNode` `9e020cfa` to `fc647076`: the only difference is `linkChild(context, self)` replacing an inline child-link block [M diff]. `computed` `ac52b6ed` to `b2342585`: adds a `_plumbing` config bit and an unnamed node for plumbing [M diff]. `recompute` `e15bf2c8` to `261e9ef2`: lane posture hoisted before the frame is parked, a node disposed during its own pass publishes nothing (#3621), `REACTIVE_ZOMBIE` carried through the flag wipe, boundary collection for a born-held first value (#3540) [M diff]. `createProjectionNextInternal` `93498c18` to `e00464ae`, `createStoreDerivedNext` `4348c945` to `a89959f9` (setter is now `derivedStoreWrite`), `createStore` `5ab02487` to `0303d53f` (names the store) [M]. Probes `J`, `Y`, `AH`, `AI`, `X`: `ranDuringCall=true` on rc.9 and rc.13, dev and prod [M]. | U |
| `createEffect` / `createRenderEffect`: compute during the call, apply deferred (render effect: see section 5) | `callback_executions` `:5723`, `callback_owners` `:5302` (`InheritsFirstRun` for render effect), `contract_callback_execution_at` `:5848` | `effect` same (`19d6de3d`, `sig/dev.js:1723-1759`): `recompute(node, true)` `:1732`, apply gate `node._pendingValue === NOT_PENDING` and `EFFECT_USER \|\| schedule ? enqueue : runEffect` `:1734-1740` [M]. `createEffect` `ae0cc352`, `createRenderEffect` `9332f8df` same [M]. `runEffect` `5a2cf49c` to `c8de6ca3`: adds `enterCallback()`/`exitCallback()` and moves `attrHooks.effectRunStart` out of the dev-only block (observe tier) [M diff]. Probe `K`: `compute,returned,apply`; probe `L` (render effect): `compute,apply,returned`; both identical on all four configurations [M]. | U |
| `createTrackedEffect` runs after the call | `tracked_callback_timing` `:5922` (`AfterCall`) | `trackedEffect` same (`4c0027dd`, `sig/dev.js:1928-1991`), `createTrackedEffect` `417cc522` same, `enqueueSub` `7702c186` same (`sig/dev-shared.js:2968-2972`) [M]. `schedule` `3ccd6828` to `6958a1cc`: a projection draft write withholds the flush microtask until a landing or `scheduleWithheld` (`:1636-1652`) [M diff]. Probe `M`: `returned,run` on all four [M]. | U (`schedule` change: I) |
| `untrack` clears tracking, not `context`; a write inside `untrack` is still guarded | `callback_preserves_owner_write_context` `:5086`, `runs_callback_deferred` `:5025` | `untrack` `ee8333a5` to `81286bda` (`sig/dev-shared.js:5863-5889`): the fast path gains `asyncTailFlights === 0`, and the dev block increments and decrements `untrackDepth` [M diff]. Tracking, `context` and the strict-read label handling are unchanged [M]. Probe `AD` (no re-run): `runs 1 to 1`; probe `N` (write in `untrack` inside a memo): `REACTIVE_WRITE_IN_OWNED_SCOPE` dev, allowed prod; identical on both versions [M]. New role: inside `untrack`, `UNTRACKED_READ_AFTER_AWAIT` is suppressed (`untrackDepth === 0` in the check) [M, probe `PA_memo_untrack`]. | U |
| `runWithOwner` swaps owner, sets `tracking = false` | `runs_callback_deferred` `:5025`, `reports_untracked_reads_at` `:5948` | `runWithOwner` same (`332a218c`, `sig/dev-shared.js:6919-6947`) [M]. Probe `AE`: no re-run on both. Probe `SR_component_runwithowner`: `STRICT_READ_UNTRACKED` on both, which is the premise of `reports_untracked_reads_at(RunWithOwner, 1)` [M]. `RUN_WITH_DISPOSED_OWNER` still emitted (probe `RUNWITHOWNER_disposed`) [M]. | U |
| `createRoot` = `createOwner` + `runWithOwner` (Creates, Inline, untracked); `solid-js` re-exports its own `createRoot` | `callback_owners` `:5302`, `callback_runs_in_created_root` `:5110` | signals `createRoot` same (`683c23a6`, `sig/dev-shared.js:3478-3481`) [M]. `createOwner` `c7cd6632` to `8fbf144c`: `linkChild` replaces the inline link, a parentless owner calls `registerRoot` (a `WeakRef` registry of live roots for observe tooling, not a guard) [M diff]. `solid-js` `createRoot` same (`71ce1b1e`, `js/solid.dev.js:1036`), `hydratedCreateRoot` same (`1cc5a5d2`, `:900-905`) [M]. Probe `D` (signal setter in a `createRoot` body) throws `REACTIVE_WRITE_IN_OWNED_SCOPE` on both [M]. | U |
| `resolve(fn)` wraps `fn` in a root and throws in a reactive scope | `callback_owners` `:5302`, SC2004 | `resolve` same (`be9ad203`, `sig/dev.js:2620-2663`), throw text at `:2623` [M]. Probe `AA`: thunk owned on both. Probe `Q`: `Cannot call resolve inside a reactive scope` on both dev; prod allows [M]. | U |
| `flush(fn)` runs `fn` inline and returns its value | `callback_executions` `:5723` (`Inline`) | `flush` `96b9030e` to `67d30b37` (`sig/dev-shared.js:2648-2740`): the only change is `attrHooks.flushStart()` before the drain loop (observe tier) [M diff]. Probe `AG`: `ran=true ret=3` on all four [M]. `FLUSH_IN_ACTION` at `:2659` [M]. | U |
| `onSettled` under a children-capable owner is a tracked-effect leaf, otherwise out of band | `leaf_owner_requires_owned_call_site` `:5161`, `callback_owners` `:5302` (`Leaf`) | `onSettled` same (`8c77ff45`, `sig/dev.js:3031-3061`): `trackedEffect(() => untrack(callback), ...)` under an owner, `globalQueue.enqueue(EFFECT_USER, fire)` otherwise [M]. Probe `W`: `onCleanup` inside `onSettled` and inside `createTrackedEffect` throws `CLEANUP_IN_FORBIDDEN_SCOPE` (dev), allowed prod, both versions. Probe `P`: unowned `onSettled` returning a cleanup throws `SETTLED_CLEANUP_UNOWNED`, both. Probes `LEAF_*`: `createMemo`, `createRoot`, function `createSignal`, `createEffect` in a tracked effect throw `PRIMITIVE_IN_FORBIDDEN_SCOPE`; `flush()` throws `Cannot call flush() from inside onSettled or createTrackedEffect`; value `createSignal(0)` allowed; identical on both [M]. | U |
| `createReaction`'s invalidation callback has no owner | `callback_owners` `:5302` (`None`), `reports_untracked_reads_at` `:5948` | `createReaction` same (`1dcfbb77`, `sig/dev.js:2553-2592`) [M]. Probe `U`: owner `null` on all four. Probe `SR_reaction_cb`: strict-read warning inside the callback on both [M]. | U |
| `mapArray` / `repeat` rows are owned | `callback_owners` `:5302` (`Creates` at 1) | `mapArray` same (`377f168f`, `sig/dev.js:8889-8925`), `repeat` same (`0406a0b1`, `:9332-9361`), `For` same (`eabb480f`, `js/solid.dev.js:1439-1453`), `Repeat` same (`c9b26a45`, `:1454-1461`) [M]. Probe `V`: `[true,true,true,true]` on all four. Probe `SR_mapfn_named`: a named row callback keeps its strict window on both [M]. | U |
| `latest` / `isPending` do not clear tracking | `callback_executions` `:5723` (`Inline`) | `latest` same (`54850c24`, `sig/dev.js:1616-1624`) [M]. `isPending` `50238e52` to `90a02925`: a suppressed-probe host is enrolled for re-wake only when the probe is tracked (#3648) [M diff]. Probe `LATEST_tracks` (memo with `latest(...)` and memo with `isPending(...)`): `before 101, after 302` on all four [M]. Probe `ISPENDING_untracked_in_memo` (the #3648 shape): host runs `2 to 3`, fetches `1 to 2` on both; the source difference was not reproduced by this construction [M]. | U |
| Component bodies run in `untrack(() => Comp(props), label)` (strict-read window) under a transparent root | `props_require_caller_proof` `:5252`, `component_body_runs_under_root` `:5130` | `observedComponent` `59581715` to `5fd258e3` (`js/solid.dev.js:39-64`): the dev `_component` record gains a `task` (console task) field; still `createRoot(..., { transparent: true })` and `untrack(() => Comp(props), label)` at `:59` [M diff]. `createComponent` same (`c8072973`, `:1398-1400`) [M]. Probes `SR_component_*` (11 variants) and `CREATE_COMPONENT_owner`: identical warn/no-warn matrix and owner shape on both versions, apart from message text (section 3.4) [M]. **`lazy` is the exception**, see the next row. | U (component bodies) |
| `lazy`: the loader is called once from the wrapper body; the loaded component runs in the same strict window as any component | `reports_untracked_reads_at` `:5948` (`Lazy` at 0), `callback_owners` `:5302` (`Lazy` Inherits) | `lazy` `3b37ad89` to `53906b11` (`js/solid.dev.js:1401-1432`): `untrack(() => { ...; return Comp(props); })` became `createComponent(Comp, props)` [M diff]. Probe `LAZY_loader_read`: loader read warns `STRICT_READ_UNTRACKED` in `<wrap>` on both (the existing premise holds) [M]. Probe `LAZY_component_body_read`: the **loaded component's** direct read warns on rc.13 dev only. Probe `LAZY_component_body_owner`: `_root:false, name:"computed"` to `_root:true, name:"<default>"` [M]. Probe `LAZY_component_body_write`: setter in the loaded body throws on both (the wrapping memo or the root is an owned scope either way) [M]. | **R** |
| Store setter draft write-enables the original proxy only; writes outside a setter are dropped | `store_setter_callback_enables_proxy_writes` `:5241`, `store_root_properties_are_readonly` `:5204` | `storeSetterNext` same (`d5229367`, `sig/dev.js:6982-7022`) [M]. `wrapDraft` `6fe02aff` to `0d1cc1eb`: set, delete and define traps share one `mutate` bracket; a write to a superseded or disposed draft is dropped silently; an `afterWrite` hook schedules the flush [M diff]. Probe `T` (`store.a=7` in a setter commits): `a:7`; probe `T2` (another store's write dropped): `otherB:0`; probe `H` (direct `store.a = 99` dropped): `a:1`; identical on all four [M]. | U (`wrapDraft`: I) |
| `merge` keeps prop reactivity | `merges_props_reactivity` `:6122`, `wraps_function_arguments_in_memo` `:5719` | `merge` `dd154e09` to `ac3e2a27`: pre-sized arrays, `recordOf` replaces the `$SOURCES`/`$OMIT`/`$VIEW` symbols with one `$RECORD` [M diff]. Probe `MERGE_reactive`: `runs 2, a 2` on all four [M]. | I |
| `Loading` is the async boundary, `Errored` the error boundary | `boundary_kind` `:5275` | `Loading` same (`b2eb0ac4`, `js/solid.dev.js:1543-1548`), `Reveal`, `Show`, `Match`, `Repeat`, `For` same [M]. `Switch` `28fc1b90` to `15112342`: adds `name: "conditions"` [M diff]. `Errored` `a7abc64d` to `6501c73f`: fallback is now invoked for every function, see summary row 4 [M]. Signals `createErrorBoundary` changed: `reset` is `queue._retry()` [M diff]; `boundaryComputed` and `createBoundChildren` only add node names [M diff]. | U (`Loading`); I (`Errored`, no row) |
| `cleanup_rule`, `accepts_cleanup_return`, `creates_directive_owner`, `returns_store`, `returns_reactive_tuple`, `creates_reactive_source` | `:6023`, `:6052`, `:6344`, `:5447`, `:6334`, `:6080` | `cleanup` same (`7e987494`, `sig/dev-shared.js:3366-3372`), `onCleanup` same (`78f06b81`, `sig/dev.js:2311-2340`) [M]. Return shapes: `createSignal`, `createMemo`, `createOptimistic` slices same [M]. `runDisposal` `7d589ec2` to `59ac696d` and `disposeChildren` `bfce4c53` to `80e953bf`: list detached before it runs (#3601), **LIFO order** (#3572), depth bracket `enterDisposal`/`exitDisposal`, a zombie-frame drain on owner death (#3561) [M diff]. Probes `CLEANUP_*`: registration order `c1,c2,c3` runs `c1,c2,c3` on rc.9 and `c3,c2,c1` on rc.13, dev and prod; effect-returned cleanup and `onSettled`-returned cleanup run once as before [M]. | U (return shapes); I (order, summary row 7) |

Rows added to the template because the rules and dialect state them:

| assumption | dialect source (current line) | rc.13 evidence | class |
| --- | --- | --- | --- |
| `STRICT_READ_UNTRACKED` windows: component body (labelled `untrack`), effect apply (`"an effect callback"`), named `mapArray`/`repeat` callbacks; nothing else; a nested `untrack(fn)` without a label clears it | `props_require_caller_proof` `:5252`, `reports_untracked_reads_at` `:5948`, `apply_callback_argument` `:5940` | The set of `setStrictRead` / labelled `untrack` call sites is unchanged: `observedComponent`, `runEffect` (`sig/dev.js:1875`, rc.9 `:1755`), `mapArray` (`:8894`) and `repeat` (`:9335`) wrappers, each with the same label. `read()` `sig/dev-shared.js:6333-6549` changed in three ways: the warning moved ahead of the snapshot serve (#3675, matters only while hydrating), an armed derived override no longer throws for an uninitialized read with no reader, and the post-await check was added [M diff]. Probes `SR_*` (21): same warn/no-warn on all four. Component body, `runWithOwner`, `flush(fn)`, `latest(fn)`, `createRoot` inside a body, store read, effect apply, reaction callback, named map function and custom label **warn**; `untrack`, memo, `onSettled`, `onCleanup`, timer, effect compute, tracked effect, event handler, module scope and apply-inside-`untrack` do not [M]. Prod never warns [M]. | U |
| Write guards: `REACTIVE_WRITE_IN_OWNED_SCOPE` for signal setter, store setter (roots **not** exempt), `refresh`; `ACTION_CALLED_IN_OWNED_SCOPE`; leaf scopes exempt | `leaf_scopes_allow_writes` `:5058`, `store_setter_guard_exempts_roots` `:5141`, `optimistic_store_setter_guarded` `:5149` | `setSignal` guard unchanged (`sig/dev-shared.js:6738-6753`; the slice changed for held-derivation and snapshot-capture arms only) [M diff]. `devGuardStoreSetterWrite` same (`41b7ab1d`, `:6650-6665`: `if (context && !(context._config & CONFIG_CHILDREN_FORBIDDEN))`, no root exemption) [M]. `devGuardStoreSetterResult` same (`a1467132`) [M]. `refresh` same (`20d138d5`, `sig/dev.js:2705-2790`), guard throw `dev-shared.js:6980` [M]. `action` `d04f9a6f` to `5d0aa8cd`: adds `enterCallback`/`exitCallback` around the generator step [M diff]. Probes `G_*` (13), `N3_*` (8), `N5_*` (2), `E`, `D`, `B2_dynamic_static_write`: identical on all four. Allowed in tracked effect, effect apply and owner-backed `onSettled` (`G_write_onsettled`: signal setter, store setter and action all allowed); thrown in memo, component body, `createRoot` body, `untrack`/`runWithOwner`/`flush(fn)` inside those [M]. | U |
| Async: `ASYNC_OUTSIDE_LOADING_BOUNDARY`, `PENDING_ASYNC_UNTRACKED_READ`, `PENDING_ASYNC_FORBIDDEN_SCOPE`, `SYNC_NODE_RECEIVED_ASYNC`; **reads after `await` are untracked** | `computation_read_is_render` `:6106`, `reports_member_reads_after_await` `:5260`, `supports_sync_option` `:5650`, `fresh_stack_callback_owner` `:5191` | Same four diagnostics at the lines in section 3.4. `handleAsync` `3374-3891` to `3904-4424`: the value comparator is now called `equals(prev, next)` on an async landing (it was `(next, prev)`), the thenable path goes through `watchAsyncTail`, plus lane and override arms [M diff]. Probes `ASY_*` (4) and `NOOWNER_*_after_await` (2): identical. An `await` still ends tracking and leaves no owner: `getOwner()`-dependent diagnostics `NO_OWNER_CLEANUP` and `NO_OWNER_EFFECT` still fire after an `await` [M]. What is new is a runtime warning for the read itself (summary row 1). | U (existing guards); R (new guard) |
| `dynamic(source, { static: true })` is `staticDynamic(untrack(source))`: one call, untracked, no owner; a thenable throws in dev (B2) | `call_form` `:4927`, `callback_executions` `:5723` (`DynamicStatic` Inline), `callback_owners` `:5302` (`Inherits`), `callback_preserves_owner_write_context` `:5086` | `options?.static` still opens every client and server build: `web/web.dev.js:2297`, `web/web.js:2062`, `web/server.js:4281` (rc.9 `:2199`, `:2034`, `:3729`) [M]. `staticDynamic` same (`75b78e8b`, `web.dev.js:2380-2395`); the thenable throw is at `:2381` (rc.9 `:2249`) [M]. `dynamic` `1649686f` to `38ceff2c`: the default form gains a per-instance memo and a `FLIGHT` token for an async source and a `sameInstance` comparator; the address signal is `ownedWrite` [M diff]. Probes `B2_dynamic_static_once` (`callsAtCreate 1, callsAfterWrite 1`), `B2_dynamic_default` (`0, 1, 2`), `B2_dynamic_static_write`, `DYN_static_async_source`: identical on all four [M]. | U |
| `omit(props, hidden)` with one function argument is a predicate (B3) | `callback_runs_on_result_access` `:6171`, `splits_props` `:6130` | Selection test still `keys.length === 1 && typeof keys[0] === "function"` (`sig/dev.js:4550`, rc.9 `:4380`) [M]. `omit` `578fbc14` to `c30db00b`: record lookup via `recordOf`; the no-`Proxy` path re-homes accessors with `bind(props)` [M diff]. Probe `B3_omit_predicate`: `created, pred:a, pred:b, pred:b, keys:b` on all four, so the predicate runs on access, not at creation [M]. | U |
| `until(fn, options)` throws in a reactive scope and runs `fn` during the call (B4) | `callback_owners` `:5302` (`Until` Creates), `callback_executions` `:5723` | `until` same (`18b3461c`, `sig/dev.js:2854-2922`); the throw is at `:2857`, guarded by `getObserver()` [M]. Probes `B4_until_in_memo` (throws), `B4_until_untrack_memo` (allowed), `B4_until_in_trackedeffect` (throws), `B4_until_in_effect_apply` (allowed), `B4_until_runs` (`pred, returned, pred`): identical on all four [M]. | U |
| `FLUSH_IN_ACTION` (N4) | `throws_inside_action_step` `:6324` | `sig/dev-shared.js:2659`, step bracket `sig/dev.js:2141` (rc.9 `:2213`, `:2015`) [M]. Probes `R`, `N4_flush_in_action_after_yield`, `N4_flush_fn_in_action`: `FLUSH_IN_ACTION` dev, resolved prod, both versions [M]. | U |

## 3.4 Guards and diagnostics the rules mirror

### 3.4.1 Every diagnostic the catalog cites is still emitted by rc.13 [M]

Locations are the line of the `code:` or `throw` site. "probe" names the probe that exercised it on rc.13 dev; every one of them produced the same outcome as on rc.9.

| code | rc.9 | rc.13 | probe |
| --- | --- | --- | --- |
| `REACTIVE_WRITE_IN_OWNED_SCOPE`, signal setter | `sig/dev-shared.js:5954` | `sig/dev-shared.js:6746` | `G_setter_memo`, `G_setter_component`, `D` |
| `REACTIVE_WRITE_IN_OWNED_SCOPE`, store setter | `:5880` | `:6653` | `G_storesetter_memo`, `N3_*`, `E` |
| `REACTIVE_WRITE_IN_OWNED_SCOPE`, `refresh` | `:6135` (throw `:6144`) | `:6971` (throw `:6980`) | `G_refresh_memo` |
| `ACTION_CALLED_IN_OWNED_SCOPE` | `sig/dev.js:1975` | `sig/dev.js:2101` | `G_action_memo` |
| `MISSING_EFFECT_FN` | `:2298` | `:2435` | `G_effect_onearg` |
| `Cannot call resolve inside a reactive scope` | `:2486` | `:2623` | `Q` |
| `SETTLED_CLEANUP_UNOWNED` | `:2916` | `:3053` | `P` |
| `CLEANUP_IN_FORBIDDEN_SCOPE` | `:2192` | `:2329` | `W` |
| `PRIMITIVE_IN_FORBIDDEN_SCOPE` | `sig/dev-shared.js:2918`, `:4983` | `:3429`, `:5638` | `LEAF_*` |
| `STRICT_READ_UNTRACKED` | `sig/dev-shared.js:635` | `:734` | `SR_*` |
| `PENDING_ASYNC_UNTRACKED_READ` | `:620` | `:713` | `ASY_component_pending` |
| `PENDING_ASYNC_FORBIDDEN_SCOPE` | `:5699` | `:6449` | `ASY_trackedeffect_pending` |
| `SYNC_NODE_RECEIVED_ASYNC` | `:3402` | `:3932` | `ASY_sync_received_async` |
| `NO_OWNER_BOUNDARY` | `sig/dev.js:9194` | `:9991` | `NOOWNER_boundary` |
| `NO_OWNER_EFFECT` | `:1627`, `:1853` | `:1747`, `:1979` | `NOOWNER_effect`, `NOOWNER_effect_after_await` |
| `NO_OWNER_CLEANUP` | `:2182` | `:2319` | `NOOWNER_cleanup`, `NOOWNER_cleanup_after_await` |
| `ASYNC_OUTSIDE_LOADING_BOUNDARY` | `:1681` | `:1801` | `ASY_render_effect_pending` |
| `FLUSH_IN_ACTION` | `sig/dev-shared.js:2213` (+ `dev.js:2015`) | `sig/dev-shared.js:2659` (+ `dev.js:2141`) | `R`, `N4_*` |
| `FLUSH_IN_EFFECT_CALLBACK` | `:2247` | `:2693` | `N4_flush_in_effect_apply` |
| `Cannot call flush() from inside onSettled or createTrackedEffect` | `:2237` | `:2683` | `LEAF_flush_in_trackedeffect`, `LEAF_flush_in_onsettled` |
| `Cannot call until inside a reactive scope` | `sig/dev.js:2720` | `:2857` | `B4_until_in_memo` |
| `INVALID_REFRESH_TARGET` | `:2576` | `:2713` | `REFRESH_invalid_target` |
| `RUN_WITH_DISPOSED_OWNER` | `sig/dev-shared.js:6090` | `:6926` | `RUNWITHOWNER_disposed` |
| `ASYNC_STORE_SETTER` | `:5909` | `:6682` | not probed [E: slice `devGuardStoreSetterResult` is byte-identical] |
| static `dynamic` thenable throw | `web/web.dev.js:2249`, `server.dev.js:3979` | `web/web.dev.js:2381`, `server.dev.js:4698` | `DYN_static_async_source` |
| default-transport rich-argument throw | `@solidjs/web/server-functions/dist/client.js:570` | `.../client.js:721` | not executed [M for presence by line] |
| `LATE_HEADER_WRITE` | `web/server.dev.js:3745` | `web/server.dev.js:4463` | not executed (server) [M for presence] |
| `REACTIVITY_HALTED` | `sig/dev-shared.js:1355` | `:1739` | seen in `G_setter_renderapply` (text changed: no longer names `createErrorBoundary`) |

Outcome rules that hold on both versions, dev and prod [M]:

- **Dev throws or warns, prod does not.** In prod none of the dev guards fire. One-argument `createEffect` crashes with `Cannot read properties of undefined (reading 'effect')` on both (`G_effect_onearg`).
- **Identical throw or allow outcomes** for all 134 probes except the ones listed in section 3.4.3.

### 3.4.2 New in rc.13 (dev bundles) that rc.9 lacks

Method: a structural diff of every string literal of 30+ characters and every `code: "..."` in the signals, `solid-js` and `@solidjs/web` dev and server bundles, plus a diff of every `throw` and `reportDiagnostic` site (`runtime/slices/strings.mjs`, `throws.mjs`, `codes.mjs`). The same string diff over `dist/prod/**` (messages of 30+ characters containing whitespace) found **no** new or removed string, and the `throw` diff found only minifier renames of the same `read` throw sites [M].

| code | where | trigger | tier | warn or throw | probe |
| --- | --- | --- | --- | --- | --- |
| `UNTRACKED_READ_AFTER_AWAIT` | `sig/dev-shared.js:1022` (message `:1026-1029`); `checkPostAwaitRead` `:968-1037`; `watchAsyncTail` `:934-963`; registered from `handleAsync` `:4363`; called from `read()` `:6339-6347` and from the store `get` trap `sig/dev.js:6630-6650` | A reactive value is read after an `await` in the computation (memo, effect compute, projection derive, store derive) whose async function returned a native `Promise`. Conditions in code [M]: `context === null` (the continuation has no owner), `callbackDepth === 0` (not inside an effect callback or an action step), `disposalDepth === 0`, not `tracking`, `untrackDepth === 0` (not inside `untrack`), not inside an `isPending` probe, the computation's flight is still current, the value is not already a dependency of that computation, and it has not already warned for that computation, source and key. A read that would throw a pending error is not reported. Needs V8 async stack frames (`Error.captureStackTrace`, `attributedFlight` `:1061-1090`); other engines never warn [M, source comment and code]. | **dev only**: the three functions and the string exist only in `dist/dev.js` and `dist/dev-shared.js`; none in `dist/prod/**` or `dist/observe/**` [M] | **warn** (`kind: "async"`, `severity: "warn"`), never throws | Fires on rc.13 dev, silent on rc.9 dev and on both prod builds: `PA_memo`, `PA_effect_compute`, `PA_store`, `PA_helper`, `PA_projection`, `PA_latest` (names `latest(signal)`), `PA_second_await`, `PA_thenable` (the compute still returns a native promise). Silent on rc.13 for `PA_memo_untrack`, `PA_memo_before`, `PA_memo_already_tracked`, `PA_plain_async_fn`, `PA_effect_apply_async`, `PA_trackedeffect_async`, `PA_action_body` [M] |
| `LOADING_ON_OUTSIDE_HOLD` | `sig/dev.js:9974` (`reportUnseen` `:9970-9984`, trigger `_rearm` `:9795-9801`) | `createLoadingBoundary(fn, fallback, { on })` re-arms (a dependency read by `on` was written) while a live reporter outside the boundary awaits the same source, so the fallback can never show. Not reported for a display-ahead (`latest()`) re-arm [M source] | dev (`reportDiagnostic` is console output; the observe tier routes to the structured channel only [E]) | **warn** | `LOADING_ON_OUTSIDE_HOLD`: warns on rc.13 dev; message names the source (`computed`); nothing on rc.9, nothing on prod [M] |
| `INVARIANT_VIOLATION` | `sig/dev-shared.js:595` (`assertInvariant` `:591-602`); one call site, `disposeChildren` `:3223` (`owner-chain-head`) | The owner chain's head is not `parent._firstChild` when a node splices out (#3543). An internal consistency check | dev | `console.error` via `reportDiagnostic` (`severity: "error"`), does **not** throw despite the comment [M source] | Not reachable from user code, so no probe fires it [E] |
| `SETTLE_WALK_UNINITIALIZED_SOURCE` (existing code, new reach) | `sig/dev-shared.js:3823` | Was structured-channel only on rc.9; now also `reportDiagnostic` (console) [M diff] | dev | warn/error text unchanged | not probed (internal) |
| `UNSCOPED_HOLE_ALLOCATED_IDS` | `web/web.dev.js:226`, `web/server.dev.js:803` | A JSX hole receives a function that builds hydratable content, so ids are taken from the enclosing scope. The message says this is reached only from JavaScript or through a cast [M source] | dev (client and server) | warn | not executed (needs hydration) [E] |
| `SSR_UNDECLARED_LIVE_SOURCE` | `js/server.dev.js:1057` | An async iterable read in a server component is still producing 5 s into a document render | dev, server | warn | not executed (server) [E] |
| `SSR_BOUNDARY_WATERFALL` (rc.9 had `ASYNC_WATERFALL` with a near-identical message; rename [E]) | `js/server.dev.js:2334` | A `<Loading>` boundary took 3+ render passes (2 sequential waits is `info`, more is `warn`) | dev, server | warn or info | not executed [E] |
| `SERVER_ERROR_SANITIZED` (rc.9 had `SSR_ERROR_SANITIZED` with the same message; rename [E]) | `js/server.dev.js:1769` | A render error was replaced before reaching the client | dev, server | info | not executed [E] |
| `DYNAMIC_ASYNC_COMPONENT` | `web/server.dev.js:4761`; also `server.js:4341-4343`, `server.observe.js:4496-4498` | Server `dynamic` whose async source resolves to a client component function | **all server tiers** (dev message is longer) | **throw** (also recorded as a finding) | not executed (server) [E]; see summary row 6 |
| `ATTRIBUTE_SLOT_POSITION` / slot-value messages (rc.9 had `BEHAVIOR_CLAIM_DROPPED`, absent in rc.13; replacement is an inference [E]) | `web/server.dev.js:3527-3794` | Server-component attribute-slot misuse | dev, server | warn | not executed [E] |

Messages that changed text but not trigger [M diff]: `STRICT_READ_UNTRACKED` now names the value when it has a name (`Reactive value "a" read directly in <Anonymous> ...`; probes `SR_component_store`, `SR_component_latest`); `REACTIVITY_HALTED` no longer names `createErrorBoundary`; `DEV.setConsoleFooter` moved to a top-level `setConsoleFooter` export.

The internals the task named, and what they do [M]:

- `checkPostAwaitRead`, `untrackDepth`, `callbackDepth`, `enterCallback`/`exitCallback`: the post-await guard and its suppression state. `untrack` increments `untrackDepth` (`dev-shared.js:5876-5886`), `runEffect` and the `action` step bracket `callbackDepth` (`sig/dev.js:1877`, `:1907`, `:2145`, `:2150`, `:2155`).
- `enterDisposal`/`exitDisposal`, `disposalDepth`, `resetDisposalDepth`: bracket cleanup execution so a cleanup's reads are not mistaken for post-await reads; `runDisposal` and `disposeChildren` (`dev-shared.js:3129-3279`).
- `derivedStoreWrite`, `heldDerivation`, `rederiveHeld`, `derivedSetter`, `heldDerivationHit`: the setter of a derived store or memo, when its node is held by a transaction, re-derives under the hold instead of masking (`sig/dev.js:6963-6981`, `sig/dev-shared.js:6869-6883`, `setMemo` `:6893-6899`). No guard; transaction timing only [M diff, E on effect].
- `readerOverride`: derived-override read arm in the store `get` trap (`sig/dev.js:6392-6396`). Transaction/lane internals [M source].

### 3.4.3 Probe outcomes that differ between rc.9 and rc.13 [M]

Computed over all 134 probes, dev and prod, comparing result, error, event order, console text with the once-per-code footer ignored, and uncaught errors (`runtime/probes/results.json`).

| probe | difference | meaning |
| --- | --- | --- |
| `PA_memo`, `PA_effect_compute`, `PA_store`, `PA_helper`, `PA_projection`, `PA_latest`, `PA_second_await`, `PA_thenable` (dev) | `UNTRACKED_READ_AFTER_AWAIT` warns on rc.13 only | summary row 1 |
| `LOADING_ON_OUTSIDE_HOLD` (dev) | `LOADING_ON_OUTSIDE_HOLD` warns on rc.13 only | summary row 5 |
| `LAZY_component_body_read`, `LAZY_component_body_owner` (dev) | lazy-loaded component body gains a strict-read warning and a root owner | summary row 2 |
| `CLEANUP_ORDER`, `CLEANUP_ORDER_MEMO`, `CLEANUP_PARENT_CHILD`, `CLEANUP_REENTRANT` (dev and prod) | cleanup order reversed | summary row 7 |
| `ERRORED_fallback_arity0` (dev and prod) | zero-parameter fallback is called | summary row 4 |
| `SR_component_latest`, `SR_component_store`, `G_setter_renderapply` (dev) | message text only (`"latest(signal)"`, `"a"`, `REACTIVITY_HALTED` wording) | I |
| `EXPORTS` (dev and prod) | namespace key diff, section 4 | X |

## 4. Export surface and declarations (not part of 3.3 or 3.4; recorded because the rows depend on it)

- **None of the 53 `TABLE` names (`solid_2.rs:120-181`, which includes `until`) is missing from the rc.13 client namespaces of `solid-js` and `@solidjs/web`** [M, probe `EXPORTS`].
- Namespace key changes rc.9 to rc.13 [M, probe `EXPORTS`, dev and prod identical]:
  - `@solidjs/signals`: added `$RECORD`, `setConsoleFooter`; removed `ownerPath` (88 to 89 keys).
  - `solid-js` client: added `inLiveServerComponentScope`, `isHydratable`, `isHydrating`, `shareAsyncIterable` (81 to 85).
  - `@solidjs/web` client: added `getHydrationWriter`, `ssrElementAttribute`, `takeHydrationValue` (117 to 120).
  - None of these is a `TABLE` name or a vocabulary primitive.
- Declarations the rows restate [M diff of `dist/types`, comments stripped]: `Store<T> = T` unchanged (`signals/dist/types/store/store.d.ts:4`, B1 holds); `dynamic`'s source type gains `AsyncIterable<T>` (`web/types/index.d.ts:112`); `UntilOptions.signal` is typed `GlobalAbortSignal`; `createLoadingBoundary` takes `options?: { on?: () => any }` in `solid-js/internal`. `createEffect(compute, apply)` and the `Signal`, `Accessor`, `Setter`, `Refreshable` shapes are unchanged in the files diffed (`signals.d.ts`, `core/index.d.ts`, `store/*.d.ts`, `boundaries.d.ts`, `map.d.ts`, `index.d.ts`). Only the listed hunks differ [M]. `tsc` was not run [E for any TS2305 conclusion].
- Summary row 8 (root typings dropping five re-exports) is the one declaration change that touches a dialect table.

## 5. Open items carried from the rc.9 review

- **`createRenderEffect` applies during the creating call** (rc.9 section 9, the `(1, Deferred)` row): still true on rc.13 (`L`: `compute,apply,returned`), and still true that the dialect answers `InheritsFirstRun` for the apply's owner (`solid_2.rs:5351-5354`) and `Deferred` for its execution (`:5734-5736`) [M]. Not an rc.13 difference.
- **A signal setter directly in a `createRoot` body throws on both versions** (probe `D`), and so does a store setter (probe `E`) [M].

## 6. Not done, and where the measurement stops

- **Hydration and SSR paths were not executed.** The `hydrated*` slices of `solid.dev.js` (22 changed slices), the server bundles' new diagnostics, `@solidjs/web` server functions beyond the cited line, and the `observe` bundles were read only as source diffs or enumerated [M for what was read, nothing measured by execution].
- **Production slices were not compared by name** (minified); prod parity rests on per-file digests (25 of 34 signals prod files differ) and on the 134 probes [M].
- **The post-await guard was measured on V8 only** (Node 24.21.0). Its source says Firefox and Safari never warn [M source, E for those engines].
- **The #3648 `isPending` fix and the #3621 self-disposal fix were read in the diff but not reproduced** by the two constructions tried (`ISPENDING_untracked_in_memo`, `DISPOSE_in_own_memo`: identical on both) [M].
- **`tsc` was not run** against rc.13 typings, and no checker binary was run. Claims about what `tsc` or the checker does with rc.13 are [E].
- **rc.10 to rc.12 were not read.** `releases.rs` still treats them as "not compared"; nothing here extends rc.13's answers to them.
- **Registry integrity of rc.13 was not re-derived** (taken from `.package-lock.json`).
- **Not compared**: the rc.13 compiler interface (the rc.9 review's section 6), the `solid-js/internal` and `solid-js/attribution` subpaths' runtime, negative-row (`AUDITED_ARCHIVES`) eligibility for rc.13.

## 3.5 The seven release-dependent answers, checked on rc.13 bytes

These are the answers `releases.rs` derives per release (`releases.rs:9-17`, `Solid2::RC9` `:282-290`). Each was checked against rc.13 bytes and probes [M].

| answer | `Solid2::RC9` | rc.13 evidence | holds |
| --- | --- | --- | --- |
| B1 store root typing | `Mutable` | `Store<T> = T`, `signals/dist/types/store/store.d.ts:4`; probe `H`: root write dropped outside a setter, dev and prod | yes |
| B2 `dynamic` static form | `StaticForm` | `if (options?.static) return staticDynamic(untrack(source))` `web.dev.js:2297`, `web.js:2062`, `server.js:4281`; probes `B2_*`, `DYN_static_async_source` | yes |
| B3 `omit` predicate | `true` | `keys.length === 1 && typeof keys[0] === "function"` `sig/dev.js:4550`; probe `B3_omit_predicate` | yes |
| B4 `until` | `true` | exported from the `solid-js` root namespace and from signals; throw `sig/dev.js:2857`; probes `B4_*` | yes |
| N3 store setter under a root | `Guarded` | `devGuardStoreSetterWrite` byte-identical (`41b7ab1d`); probes `E`, `N3_optstore_root`, `N3_derived_store_root`, `N3_store_component`, `N3_store_flush_root`, `N3_store_untrack_root`, `N3_store_runwithowner_root`: all throw in dev | yes |
| N4 `FLUSH_IN_ACTION` | `true` | `sig/dev-shared.js:2659`; probes `R`, `N4_flush_in_action_after_yield`, `N4_flush_fn_in_action` | yes |
| N5 optimistic-store setter guarded | `Guarded` | probes `N5_optstore_memo`, `N5_optstore_effect_compute`: `REACTIVE_WRITE_IN_OWNED_SCOPE` in dev | yes |

Also unchanged: the effect-apply and tracked-effect exemptions (`N3_store_effect_apply`, `N3_store_trackedeffect`: allowed) [M].

## Appendix: probes

Every probe runs in its own Node process on four configurations. `rc.9` is
`rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules`; `rc.13` is
`rust/target/audit-rc13/node_modules`. `S` is the `@solidjs/signals` namespace,
`J` the `solid-js` client namespace, `W` the `@solidjs/web` client namespace;
`log(x)` appends to `events`. Helpers: `tick` (one `setTimeout(0)`), `settle` (flush,
tick, flush, tick), `attempt(f)` (returns `{ok,v}` or `{threw:<code>}`), `viaMemo`
(creates a memo from `fn` in a root and reads it inside `attempt`). Console
output of `warn`, `error`, `log` and `info` is captured; a line that is only the
once-per-code repair-guide footer of a diagnostic is shown as such. Files loaded, per
`import.meta.resolve`, are in section 2. Raw results: `runtime/probes/results.json`;
the probe source with comments: `runtime/probes/probes.mjs`; runner:
`runtime/probes/runall.mjs` and `run.mjs`.

Probe letters from the rc.9 review are kept: J, Y, AH, AI, X (creation timing), K,
L, M (effects), AD, N, AE, AA, Q, AG, W, P, U, V (tracking and ownership), T, T2, H
(store writes), E, D, R (guards). The review's original probe sources were not
available, so each is reconstructed from its description in the review's table; a
reconstruction is not a replay.


#### creation-time compute (row: non-lazy computed runs during the creating call)

**J**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, atReturn; S.createRoot(() => { S.createMemo(() => { ran = true; return 1; }); atReturn = ran; }); return { ranDuringCall: atReturn }; }
rc.9 dev   result={"ranDuringCall":true}
rc.9 prod  result={"ranDuringCall":true}
rc.13 dev  result={"ranDuringCall":true}
rc.13 prod result={"ranDuringCall":true}
```
**Y**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, atReturn; S.createRoot(() => { S.createSignal(() => { ran = true; return 1; }); atReturn = ran; }); return { ranDuringCall: atReturn }; }
rc.9 dev   result={"ranDuringCall":true}
rc.9 prod  result={"ranDuringCall":true}
rc.13 dev  result={"ranDuringCall":true}
rc.13 prod result={"ranDuringCall":true}
```
**AH**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, atReturn; S.createRoot(() => { S.createOptimistic(() => { ran = true; return 1; }); atReturn = ran; }); return { ranDuringCall: atReturn }; }
rc.9 dev   result={"ranDuringCall":true}
rc.9 prod  result={"ranDuringCall":true}
rc.13 dev  result={"ranDuringCall":true}
rc.13 prod result={"ranDuringCall":true}
```
**AI**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, atReturn; S.createRoot(() => { S.createProjection((d) => { ran = true; }, {}); atReturn = ran; }); return { ranDuringCall: atReturn }; }
rc.9 dev   result={"ranDuringCall":true}
rc.9 prod  result={"ranDuringCall":true}
rc.13 dev  result={"ranDuringCall":true}
rc.13 prod result={"ranDuringCall":true}
```
**X**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, atReturn; S.createRoot(() => { S.createStore((d) => { ran = true; }, {}); atReturn = ran; }); return { ranDuringCall: atReturn }; }
rc.9 dev   result={"ranDuringCall":true}
rc.9 prod  result={"ranDuringCall":true}
rc.13 dev  result={"ranDuringCall":true}
rc.13 prod result={"ranDuringCall":true}
```

#### effect timing

**K**  (dev =, prod =)
```
code: async ({ S, log }) => { S.createRoot(() => { S.createEffect(() => { log("compute"); return 1; }, () => { log("apply"); }); log("returned"); }); await settle(S); return {}; }
rc.9 dev   result={}  events=["compute","returned","apply"]
rc.9 prod  result={}  events=["compute","returned","apply"]
rc.13 dev  result={}  events=["compute","returned","apply"]
rc.13 prod result={}  events=["compute","returned","apply"]
```
**L**  (dev =, prod =)
```
code: async ({ S, log }) => { S.createRoot(() => { S.createRenderEffect(() => { log("compute"); return 1; }, () => { log("apply"); }); log("returned"); }); await settle(S); return {}; }
rc.9 dev   result={}  events=["compute","apply","returned"]
rc.9 prod  result={}  events=["compute","apply","returned"]
rc.13 dev  result={}  events=["compute","apply","returned"]
rc.13 prod result={}  events=["compute","apply","returned"]
```
**M**  (dev =, prod =)
```
code: async ({ S, log }) => { S.createRoot(() => { S.createTrackedEffect(() => { log("run"); }); log("returned"); }); await settle(S); return {}; }
rc.9 dev   result={}  events=["returned","run"]
rc.9 prod  result={}  events=["returned","run"]
rc.13 dev  result={}  events=["returned","run"]
rc.13 prod result={}  events=["returned","run"]
```

#### tracking / ownership

**AD**  (dev =, prod =)
```
code: async ({ S }) => { let runs = 0; const [s, set] = S.createSignal(0); S.createRoot(() => { S.createMemo(() => { runs++; return S.untrack(() => s()); }); }); const before = runs; set(1); await settle(S); return { runsBefore: before, runsAfterWrite: runs }; }
rc.9 dev   result={"runsBefore":1,"runsAfterWrite":1}
rc.9 prod  result={"runsBefore":1,"runsAfterWrite":1}
rc.13 dev  result={"runsBefore":1,"runsAfterWrite":1}
rc.13 prod result={"runsBefore":1,"runsAfterWrite":1}
```
**N**  (dev =, prod =)
```
code: async ({ S }) => { const [, set] = S.createSignal(0); const r = viaMemo(S, () => S.untrack(() => { set(1); return 1; })); await tick(); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**AE**  (dev =, prod =)
```
code: async ({ S }) => { let runs = 0; const [s, set] = S.createSignal(0); S.createRoot(() => { const o = S.getOwner(); S.createMemo(() => { runs++; return S.runWithOwner(o, () => s()); }); }); const before = runs; set(1); await settle(S); return { runsBefore: before, runsAfterWrite: runs }; }
rc.9 dev   result={"runsBefore":1,"runsAfterWrite":1}
rc.9 prod  result={"runsBefore":1,"runsAfterWrite":1}
rc.13 dev  result={"runsBefore":1,"runsAfterWrite":1}
rc.13 prod result={"runsBefore":1,"runsAfterWrite":1}
```
**AA**  (dev =, prod =)
```
code: async ({ S }) => { let owned; const p = S.createRoot(() => S.resolve(() => { owned = S.getOwner() !== null; return 1; })); await p; return { thunkOwned: owned }; }
rc.9 dev   result={"thunkOwned":true}
rc.9 prod  result={"thunkOwned":true}
rc.13 dev  result={"thunkOwned":true}
rc.13 prod result={"thunkOwned":true}
```
**Q**  (dev =, prod =)
```
code: async ({ S }) => { const r = viaMemo(S, () => { S.resolve(() => 1); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"Cannot call resolve inside a reactive scope; it only resolves the current value and does not track updates."}
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"Cannot call resolve inside a reactive scope; it only resolves the current value and does not track updates."}
rc.13 prod result={"ok":true,"v":1}
```
**AG**  (dev =, prod =)
```
code: async ({ S }) => { let ran = false, ret; ret = S.flush(() => { ran = true; return 3; }); return { ran, ret }; }
rc.9 dev   result={"ran":true,"ret":3}
rc.9 prod  result={"ran":true,"ret":3}
rc.13 dev  result={"ran":true,"ret":3}
rc.13 prod result={"ran":true,"ret":3}
```
**W**  (dev =, prod =)
```
code: async ({ S }) => { let r = {}; S.createRoot(() => { S.onSettled(() => { r.inSettled = attempt(() => S.onCleanup(() => {})); }); S.createTrackedEffect(() => { r.inTracked = attempt(() => S.onCleanup(() => {})); }); }); await settle(S); return r; }
rc.9 dev   result={"inSettled":{"threw":"CLEANUP_IN_FORBIDDEN_SCOPE"},"inTracked":{"threw":"CLEANUP_IN_FORBIDDEN_SCOPE"}}  console=["warn: [CLEANUP_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"inSettled":{"ok":true},"inTracked":{"ok":true}}
rc.13 dev  result={"inSettled":{"threw":"CLEANUP_IN_FORBIDDEN_SCOPE"},"inTracked":{"threw":"CLEANUP_IN_FORBIDDEN_SCOPE"}}  console=["warn: [CLEANUP_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"inSettled":{"ok":true},"inTracked":{"ok":true}}
```
**P**  (dev =, prod =)
```
code: async ({ S }) => { const errs = []; const h = (e) => errs.push(code(e)); process.on("uncaughtException", h); let sync = attempt(() => S.onSettled(() => () => {})); await settle(S); process.off("uncaughtException", h); return { sync, uncaught: errs }; }
rc.9 dev   error="[SETTLED_CLEANUP_UNOWNED] onSettled returned a cleanup in an unowned scope; a cleanup can only be honored under an owner. Call your setup helper from an owned s"  console=["warn: [SETTLED_CLEANUP_UNOWNED] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"sync":{"ok":true},"uncaught":[]}
rc.13 dev  error="[SETTLED_CLEANUP_UNOWNED] onSettled returned a cleanup in an unowned scope; a cleanup can only be honored under an owner. Call your setup helper from an owned s"  console=["warn: [SETTLED_CLEANUP_UNOWNED] (once-per-code repair-guide footer only)"]
rc.13 prod result={"sync":{"ok":true},"uncaught":[]}
```
**U**  (dev =, prod =)
```
code: async ({ S }) => { let owner = "unset"; const [s, set] = S.createSignal(0); S.createRoot(() => { const track = S.createReaction(() => { owner = S.getOwner() === null ? "null" : "owner"; }); track(() => s()); }); set(1); await settle(S); return { ownerInInvalidation: owner }; }
rc.9 dev   result={"ownerInInvalidation":"null"}
rc.9 prod  result={"ownerInInvalidation":"null"}
rc.13 dev  result={"ownerInInvalidation":"null"}
rc.13 prod result={"ownerInInvalidation":"null"}
```
**V**  (dev =, prod =)
```
code: async ({ S }) => { const o = []; S.createRoot(() => { const m = S.mapArray(() => [1, 2], (item) => { o.push(S.getOwner() !== null); return item; }); S.createMemo(() => m()); const r = S.repeat(() => 2, (i) => { o.push(S.getOwner() !== null); return i; }); S.createMemo(() => r()); }); await settle(S); return { owned: o }; }
rc.9 dev   result={"owned":[true,true,true,true]}
rc.9 prod  result={"owned":[true,true,true,true]}
rc.13 dev  result={"owned":[true,true,true,true]}
rc.13 prod result={"owned":[true,true,true,true]}
```

#### store write semantics

**T**  (dev =, prod =)
```
code: async ({ S }) => { const [store, setStore] = S.createStore({ a: 1 }); setStore((d) => { store.a = 7; }); await settle(S); return { a: store.a }; }
rc.9 dev   result={"a":7}
rc.9 prod  result={"a":7}
rc.13 dev  result={"a":7}
rc.13 prod result={"a":7}
```
**T2**  (dev =, prod =)
```
code: async ({ S }) => { const [store, setStore] = S.createStore({ a: 1 }); const [other] = S.createStore({ b: 0 }); setStore(() => { other.b = 1; }); await settle(S); return { otherB: other.b }; }
rc.9 dev   result={"otherB":0}
rc.9 prod  result={"otherB":0}
rc.13 dev  result={"otherB":0}
rc.13 prod result={"otherB":0}
```
**H**  (dev =, prod =)
```
code: async ({ S }) => { const [store] = S.createStore({ a: 1 }); try { store.a = 99; } catch (e) { return { threw: code(e) }; } await settle(S); return { a: store.a }; }
rc.9 dev   result={"a":1}
rc.9 prod  result={"a":1}
rc.13 dev  result={"a":1}
rc.13 prod result={"a":1}
```
**E**  (dev =, prod =)
```
code: async ({ S }) => { const [, setStore] = S.createStore({ a: 1 }); let r; S.createRoot(() => { r = attempt(() => setStore((d) => { d.a = 2; })); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**D**  (dev =, prod =)
```
code: async ({ S }) => { const [, set] = S.createSignal(0); let r; S.createRoot(() => { r = attempt(() => set(1)); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**R**  (dev =, prod =)
```
code: async ({ S }) => { const a = S.action(function* () { S.flush(); }); let r; try { await a(); r = "resolved"; } catch (e) { r = code(e); } return { actionWithFlush: r }; }
rc.9 dev   result={"actionWithFlush":"FLUSH_IN_ACTION"}
rc.9 prod  result={"actionWithFlush":"resolved"}
rc.13 dev  result={"actionWithFlush":"FLUSH_IN_ACTION"}
rc.13 prod result={"actionWithFlush":"resolved"}
```

#### write-guard matrix (§3.4 "identical outcomes")

**G_setter_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [, set] = S.createSignal(0); const r = viaMemo(S, () => { set(1); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_setter_component**  (dev =, prod =)
```
code: async ({ S, J }) => { const [, set] = S.createSignal(0); let r; S.createRoot(() => { r = attempt(() => J.createComponent(() => { set(1); return 1; }, {})); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_storesetter_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [, setS] = S.createStore({ a: 1 }); const r = viaMemo(S, () => { setS((d) => { d.a = 2; }); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_write_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { set(5); return "allowed"; }); }); }); await settle(S); r.value = s(); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"},"value":5}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"},"value":5}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"},"value":5}
rc.13 prod result={"t":{"ok":true,"v":"allowed"},"value":5}
```
**G_action_memo**  (dev =, prod =)
```
code: async ({ S }) => { const a = S.action(function* () {}); const r = viaMemo(S, () => { a(); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"ACTION_CALLED_IN_OWNED_SCOPE"}  console=["warn: [ACTION_CALLED_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"ACTION_CALLED_IN_OWNED_SCOPE"}  console=["warn: [ACTION_CALLED_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_action_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const a = S.action(function* () {}); const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { a(); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**G_refresh_memo**  (dev =, prod =)
```
code: async ({ S }) => { const m = S.createRoot(() => S.createMemo(() => 1)); const r = viaMemo(S, () => { S.refresh(m); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_effect_onearg**  (dev =, prod =)
```
code: async ({ S }) => { let r; S.createRoot(() => { r = attempt(() => S.createEffect(() => 1)); }); await settle(S); return r; }
rc.9 dev   result={"threw":"MISSING_EFFECT_FN"}  console=["warn: [MISSING_EFFECT_FN] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"threw":"Cannot read properties of undefined (reading 'effect')"}
rc.13 dev  result={"threw":"MISSING_EFFECT_FN"}  console=["warn: [MISSING_EFFECT_FN] (once-per-code repair-guide footer only)"]
rc.13 prod result={"threw":"Cannot read properties of undefined (reading 'effect')"}
```
**G_setter_renderapply**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [, set] = S.createSignal(0); let r; S.createRoot(() => { r = attempt(() => S.createRenderEffect(() => 1, () => { set(1); })); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["error: [REACTIVITY_HALTED] An uncaught error halted the reactive system. No further updates will be processed. Handle errors with createErrorBoundary/<Errored> or trea","warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)","warn: [REACTIVITY_HALTED] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["error: [REACTIVITY_HALTED] An uncaught error halted the reactive system. No further updates will be processed. Handle errors with <Errored> or treat this as a crash.","warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)","warn: [REACTIVITY_HALTED] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**G_setter_effectapply**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); const r = {}; S.createRoot(() => { S.createEffect(() => 1, () => { r.t = attempt(() => { set(9); return "allowed"; }); }); }); await settle(S); r.value = s(); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"},"value":9}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"},"value":9}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"},"value":9}
rc.13 prod result={"t":{"ok":true,"v":"allowed"},"value":9}
```
**G_setter_untrack_module**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); S.untrack(() => set(3)); return { value: s() }; }
rc.9 dev   result={"value":0}
rc.9 prod  result={"value":0}
rc.13 dev  result={"value":0}
rc.13 prod result={"value":0}
```
**G_setter_flush_component**  (dev =, prod =)
```
code: async ({ S, J }) => { const [, set] = S.createSignal(0); let r; S.createRoot(() => { r = attempt(() => J.createComponent(() => { S.flush(() => set(1)); return 1; }, {})); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**G_write_onsettled**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); const [, setS] = S.createStore({ a: 1 }); const a = S.action(function* () {}); const r = {}; S.createRoot(() => { S.onSettled(() => { r.signal = attempt(() => { set(5); return "allowed"; }); r.store = attempt(() => { setS((d) => { d.a = 2; }); return "allowed"; }); r.action = attempt(() => { a(); return "allowed"; }); }); }); await settle(S); r.value = s(); return r; }
rc.9 dev   result={"signal":{"ok":true,"v":"allowed"},"store":{"ok":true,"v":"allowed"},"action":{"ok":true,"v":"allowed"},"value":5}
rc.9 prod  result={"signal":{"ok":true,"v":"allowed"},"store":{"ok":true,"v":"allowed"},"action":{"ok":true,"v":"allowed"},"value":5}
rc.13 dev  result={"signal":{"ok":true,"v":"allowed"},"store":{"ok":true,"v":"allowed"},"action":{"ok":true,"v":"allowed"},"value":5}
rc.13 prod result={"signal":{"ok":true,"v":"allowed"},"store":{"ok":true,"v":"allowed"},"action":{"ok":true,"v":"allowed"},"value":5}
```

#### strict-read windows

**SR_component_body**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { s(); return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.13 prod result={}
```
**SR_component_untrack**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.untrack(() => s()); return 1; }, {}); }); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_component_memo**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.createMemo(() => s()); return 1; }, {}); }); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_component_onsettled**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.onSettled(() => { s(); }); return 1; }, {}); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_component_runwithowner**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { const o = S.getOwner(); S.runWithOwner(o, () => s()); return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.13 prod result={}
```
**SR_component_cleanup**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); let d; S.createRoot((dispose) => { d = dispose; J.createComponent(() => { S.onCleanup(() => { s(); }); return 1; }, {}); }); d(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_component_timer**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { setTimeout(() => s(), 0); return 1; }, {}); }); await tick(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_component_flush**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.flush(() => s()); return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.13 prod result={}
```
**SR_component_latest**  (dev DIFF, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.latest(() => s()); return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value \"latest(signal)\" read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's"]
rc.13 prod result={}
```
**SR_component_createRoot**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); S.createRoot(() => { J.createComponent(() => { S.createRoot(() => { s(); }); return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.13 prod result={}
```
**SR_component_store**  (dev DIFF, prod =)
```
code: async ({ S, J }) => { const [st] = S.createStore({ a: 1 }); S.createRoot(() => { J.createComponent(() => { st.a; return 1; }, {}); }); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value \"a\" read directly in <Anonymous> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute func"]
rc.13 prod result={}
```
**SR_effect_apply**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createEffect(() => 1, () => { s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.13 prod result={}
```
**SR_renderfx_apply**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createRenderEffect(() => 1, () => { s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.13 prod result={}
```
**SR_effect_apply_untrack**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createEffect(() => 1, () => { S.untrack(() => s()); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_effect_compute**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createEffect(() => s(), () => {}); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createTrackedEffect(() => { s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_reaction_cb**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); S.createRoot(() => { const t = S.createReaction(() => { s(); }); t(() => s()); }); set(1); await settle(S); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in an effect callback will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute f"]
rc.13 prod result={}
```
**SR_event_handler**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); const h = () => s(); h(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_module_scope**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); s(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**SR_mapfn_named**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { const m = S.mapArray(() => [1], (i) => { s(); return i; }, { name: "mapNamed" }); S.createMemo(() => m()); }); await settle(S); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in mapNamed will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in mapNamed will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.13 prod result={}
```
**SR_nested_untrack_in_effect_apply_label**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); S.createRoot(() => { S.createEffect(() => 1, () => { S.untrack(() => s(), "custom"); }); }); await settle(S); return {}; }
rc.9 dev   result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in custom will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in custom will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.13 prod result={}
```

#### NEW in rc.13: UNTRACKED_READ_AFTER_AWAIT

**PA_memo**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { await 0; return s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```
**PA_memo_untrack**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { await 0; return S.untrack(() => s()); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_memo_before**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { const v = s(); await 0; return v; }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_memo_already_tracked**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { s(); await 0; return s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_effect_compute**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createEffect(async () => { await 0; return s(); }, () => {}); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```
**PA_store**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [st] = S.createStore({ a: 1 }); S.createRoot(() => { S.createMemo(async () => { await 0; return st.a; }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"a\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when it cha"]
rc.13 prod result={}
```
**PA_helper**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const help = () => s(); S.createRoot(() => { S.createMemo(async () => { await 0; return help(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```
**PA_plain_async_fn**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const f = async () => { await 0; return s(); }; await f(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_effect_apply_async**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createEffect(() => 1, () => { (async () => { await 0; s(); })(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_thenable**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { await { then: (r) => r(1) }; return s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```
**PA_trackedeffect_async**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createTrackedEffect(() => { (async () => { await 0; s(); })(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_action_body**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const a = S.action(function* () { yield Promise.resolve(); s(); }); await a(); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}
rc.13 prod result={}
```
**PA_projection**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createProjection(async (d) => { await 0; d.v = s(); }, { v: 0 }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```
**PA_latest**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { await 0; return S.latest(() => s()); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"latest(signal)\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-ru"]
rc.13 prod result={}
```
**PA_second_await**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); S.createRoot(() => { S.createMemo(async () => { await 0; await 0; return s(); }); }); await settle(S); return {}; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [UNTRACKED_READ_AFTER_AWAIT] \"signal\" was first read after an `await` in an async computation, so it is not a dependency: the computation will not re-run when i"]
rc.13 prod result={}
```

#### async / boundary diagnostics

**ASY_render_effect_pending**  (dev =, prod =)
```
code: async ({ S }) => { S.enforceLoadingBoundary && S.enforceLoadingBoundary(true); let r; S.createRoot(() => { const m = S.createMemo(() => new Promise(() => {})); r = attempt(() => S.createRenderEffect(() => m(), () => {})); }); S.enforceLoadingBoundary && S.enforceLoadingBoundary(false); await settle(S); return r; }
rc.9 dev   result={"ok":true}  console=["warn: [ASYNC_OUTSIDE_LOADING_BOUNDARY] An async value was read outside a Loading boundary. The root mount will be deferred until all pending async settles."]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"ok":true}  console=["warn: [ASYNC_OUTSIDE_LOADING_BOUNDARY] An async value was read outside a Loading boundary. The root mount will be deferred until all pending async settles."]
rc.13 prod result={"ok":true}
```
**ASY_sync_received_async**  (dev =, prod =)
```
code: async ({ S }) => { let r; S.createRoot(() => { r = attempt(() => S.createMemo(() => Promise.resolve(1), { sync: true })); }); await settle(S); return r; }
rc.9 dev   result={"ok":true}  console=["warn: [SYNC_NODE_RECEIVED_ASYNC] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"ok":true}  console=["warn: [SYNC_NODE_RECEIVED_ASYNC] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**ASY_component_pending**  (dev =, prod =)
```
code: async ({ S, J }) => { let r; S.createRoot(() => { const m = S.createMemo(() => new Promise(() => {})); r = attempt(() => J.createComponent(() => { m(); return 1; }, {})); }); await settle(S); return r; }
rc.9 dev   result={"threw":"PENDING_ASYNC_UNTRACKED_READ"}  console=["warn: [PENDING_ASYNC_UNTRACKED_READ] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"threw":"Error"}
rc.13 dev  result={"threw":"PENDING_ASYNC_UNTRACKED_READ"}  console=["warn: [PENDING_ASYNC_UNTRACKED_READ] (once-per-code repair-guide footer only)"]
rc.13 prod result={"threw":"Error"}
```
**ASY_trackedeffect_pending**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { const m = S.createMemo(() => new Promise(() => {})); S.createTrackedEffect(() => { r.t = attempt(() => m()); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"Error"}}  console=["warn: [PENDING_ASYNC_FORBIDDEN_SCOPE] Reading a pending async value inside createTrackedEffect or onSettled will throw. Use createEffect instead which supports async-"]
rc.9 prod  result={"t":{"threw":"Error"}}
rc.13 dev  result={"t":{"threw":"Error"}}  console=["warn: [PENDING_ASYNC_FORBIDDEN_SCOPE] Reading a pending async value inside createTrackedEffect or onSettled will throw. Use createEffect instead which supports async-"]
rc.13 prod result={"t":{"threw":"Error"}}
```

#### cleanup/owner ordering

**CLEANUP_ORDER**  (dev DIFF, prod DIFF)
```
code: async ({ S, log }) => { S.createRoot((d) => { S.onCleanup(() => log("c1")); S.onCleanup(() => log("c2")); S.onCleanup(() => log("c3")); d(); }); return {}; }
rc.9 dev   result={}  events=["c1","c2","c3"]
rc.9 prod  result={}  events=["c1","c2","c3"]
rc.13 dev  result={}  events=["c3","c2","c1"]
rc.13 prod result={}  events=["c3","c2","c1"]
```
**CLEANUP_ORDER_MEMO**  (dev DIFF, prod DIFF)
```
code: async ({ S, log }) => { const [s, set] = S.createSignal(0); S.createRoot(() => { S.createMemo(() => { s(); S.onCleanup(() => log("m1")); S.onCleanup(() => log("m2")); }); }); set(1); await settle(S); return {}; }
rc.9 dev   result={}  events=["m1","m2"]
rc.9 prod  result={}  events=["m1","m2"]
rc.13 dev  result={}  events=["m2","m1"]
rc.13 prod result={}  events=["m2","m1"]
```
**CLEANUP_PARENT_CHILD**  (dev DIFF, prod DIFF)
```
code: async ({ S, log }) => { S.createRoot((d) => { S.onCleanup(() => log("parent-before-child")); S.createRoot(() => { S.onCleanup(() => log("child")); }); S.onCleanup(() => log("parent-after-child")); d(); }); return {}; }
rc.9 dev   result={}  events=["child","parent-before-child","parent-after-child"]
rc.9 prod  result={}  events=["child","parent-before-child","parent-after-child"]
rc.13 dev  result={}  events=["child","parent-after-child","parent-before-child"]
rc.13 prod result={}  events=["child","parent-after-child","parent-before-child"]
```
**CLEANUP_EFFECT_RETURN**  (dev =, prod =)
```
code: async ({ S, log }) => { const [s, set] = S.createSignal(0); S.createRoot(() => { S.createEffect(() => s(), () => { log("apply"); return () => log("effect-cleanup"); }); }); await settle(S); set(1); await settle(S); return {}; }
rc.9 dev   result={}  events=["apply","effect-cleanup","apply"]
rc.9 prod  result={}  events=["apply","effect-cleanup","apply"]
rc.13 dev  result={}  events=["apply","effect-cleanup","apply"]
rc.13 prod result={}  events=["apply","effect-cleanup","apply"]
```
**CLEANUP_ONSETTLED_RETURN**  (dev =, prod =)
```
code: async ({ S, log }) => { let d; S.createRoot((dispose) => { d = dispose; S.onSettled(() => () => log("settled-cleanup")); }); await settle(S); d(); return {}; }
rc.9 dev   result={}  events=["settled-cleanup"]
rc.9 prod  result={}  events=["settled-cleanup"]
rc.13 dev  result={}  events=["settled-cleanup"]
rc.13 prod result={}  events=["settled-cleanup"]
```
**CLEANUP_REENTRANT**  (dev DIFF, prod DIFF)
```
code: async ({ S, log }) => { let d; S.createRoot((dispose) => { d = dispose; S.onCleanup(() => { log("c-a"); dispose(); }); S.onCleanup(() => log("c-b")); }); d(); return {}; }
rc.9 dev   result={}  events=["c-a","c-b"]
rc.9 prod  result={}  events=["c-a","c-b"]
rc.13 dev  result={}  events=["c-b","c-a"]
rc.13 prod result={}  events=["c-b","c-a"]
```

#### B-items from rc9 review, re-measured

**B3_omit_predicate**  (dev =, prod =)
```
code: async ({ S, log }) => { const props = { a: 1, b: 2 }; const v = S.omit(props, (k) => { log("pred:" + String(k)); return k === "a"; }); log("created"); const keys = Object.keys(v); log("keys:" + keys.join(",")); return {}; }
rc.9 dev   result={}  events=["created","pred:a","pred:b","pred:b","keys:b"]
rc.9 prod  result={}  events=["created","pred:a","pred:b","pred:b","keys:b"]
rc.13 dev  result={}  events=["created","pred:a","pred:b","pred:b","keys:b"]
rc.13 prod result={}  events=["created","pred:a","pred:b","pred:b","keys:b"]
```
**B4_until_in_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(0); const r = viaMemo(S, () => { S.until(() => s()); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"Cannot call until inside a reactive scope; await it from an action or another imperative scope."}
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"Cannot call until inside a reactive scope; await it from an action or another imperative scope."}
rc.13 prod result={"ok":true,"v":1}
```
**B4_until_runs**  (dev =, prod =)
```
code: async ({ S, log }) => { const [s, set] = S.createSignal(0); const p = S.until(() => { log("pred"); return s(); }); log("returned"); set(1); await settle(S); const v = await p; return { resolved: v }; }
rc.9 dev   result={"resolved":1}  events=["pred","returned","pred"]
rc.9 prod  result={"resolved":1}  events=["pred","returned","pred"]
rc.13 dev  result={"resolved":1}  events=["pred","returned","pred"]
rc.13 prod result={"resolved":1}  events=["pred","returned","pred"]
```
**MERGE_reactive**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(1); const m = S.merge({ get a() { return s(); } }, { b: 2 }); let runs = 0; S.createRoot(() => { S.createMemo(() => { runs++; return m.a; }); }); set(2); await settle(S); return { runs, a: m.a }; }
rc.9 dev   result={"runs":2,"a":2}
rc.9 prod  result={"runs":2,"a":2}
rc.13 dev  result={"runs":2,"a":2}
rc.13 prod result={"runs":2,"a":2}
```
**LATEST_tracks**  (dev =, prod =)
```
code: async ({ S }) => { let runs = 0; const [s, set] = S.createSignal(0); S.createRoot(() => { S.createMemo(() => { runs++; return S.latest(() => s()); }); S.createMemo(() => { runs += 100; return S.isPending(() => s()); }); }); const b = runs; set(1); await settle(S); return { before: b, after: runs }; }
rc.9 dev   result={"before":101,"after":302}
rc.9 prod  result={"before":101,"after":302}
rc.13 dev  result={"before":101,"after":302}
rc.13 prod result={"before":101,"after":302}
```

#### release-dependent answers the dialect keys on (releases.rs): N3, N4, N5, B2, B3, B4

**N5_optstore_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [, setO] = S.createOptimisticStore({ a: 1 }); const r = viaMemo(S, () => { setO((d) => { d.a = 2; }); return 1; }); await tick(); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**N5_optstore_effect_compute**  (dev =, prod =)
```
code: async ({ S }) => { const [, setO] = S.createOptimisticStore({ a: 1 }); const r = {}; S.createRoot(() => { S.createEffect(() => { r.t = attempt(() => setO((d) => { d.a = 2; })); return 1; }, () => {}); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true}}
rc.13 dev  result={"t":{"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true}}
```
**N3_optstore_root**  (dev =, prod =)
```
code: async ({ S }) => { const [, setO] = S.createOptimisticStore({ a: 1 }); let r; S.createRoot(() => { r = attempt(() => setO((d) => { d.a = 2; })); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**N3_store_component**  (dev =, prod =)
```
code: async ({ S, J }) => { const [, setS] = S.createStore({ a: 1 }); let r; S.createRoot(() => { r = attempt(() => J.createComponent(() => { setS((d) => { d.a = 2; }); return 1; }, {})); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":1}
```
**N3_derived_store_root**  (dev =, prod =)
```
code: async ({ S }) => { const [, setS] = S.createStore((d) => { d.a = 1; }, { a: 0 }); let r; S.createRoot(() => { r = attempt(() => setS((d) => { d.a = 2; })); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**N3_store_flush_root**  (dev =, prod =)
```
code: async ({ S }) => { const [, setS] = S.createStore({ a: 1 }); let r; S.createRoot(() => { r = attempt(() => S.flush(() => setS((d) => { d.a = 2; }))); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**N3_store_effect_apply**  (dev =, prod =)
```
code: async ({ S }) => { const [st, setS] = S.createStore({ a: 1 }); const r = {}; S.createRoot(() => { S.createEffect(() => 1, () => { r.t = attempt(() => { setS((d) => { d.a = 2; }); return "allowed"; }); }); }); await settle(S); r.a = st.a; return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.13 prod result={"t":{"ok":true,"v":"allowed"},"a":2}
```
**N3_store_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const [st, setS] = S.createStore({ a: 1 }); const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { setS((d) => { d.a = 2; }); return "allowed"; }); }); }); await settle(S); r.a = st.a; return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"},"a":2}
rc.13 prod result={"t":{"ok":true,"v":"allowed"},"a":2}
```
**N3_store_untrack_root**  (dev =, prod =)
```
code: async ({ S }) => { const [, setS] = S.createStore({ a: 1 }); let r; S.createRoot(() => { r = attempt(() => S.untrack(() => setS((d) => { d.a = 2; }))); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**N3_store_runwithowner_root**  (dev =, prod =)
```
code: async ({ S }) => { const [, setS] = S.createStore({ a: 1 }); let r; S.createRoot(() => { const o = S.getOwner(); r = attempt(() => S.runWithOwner(o, () => setS((d) => { d.a = 2; }))); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**N4_flush_in_action_after_yield**  (dev =, prod =)
```
code: async ({ S }) => { const a = S.action(function* () { yield Promise.resolve(); S.flush(); }); let r; try { await a(); r = "resolved"; } catch (e) { r = code(e); } return { r }; }
rc.9 dev   result={"r":"FLUSH_IN_ACTION"}
rc.9 prod  result={"r":"resolved"}
rc.13 dev  result={"r":"FLUSH_IN_ACTION"}
rc.13 prod result={"r":"resolved"}
```
**N4_flush_fn_in_action**  (dev =, prod =)
```
code: async ({ S }) => { const a = S.action(function* () { S.flush(() => 1); }); let r; try { await a(); r = "resolved"; } catch (e) { r = code(e); } return { r }; }
rc.9 dev   result={"r":"FLUSH_IN_ACTION"}
rc.9 prod  result={"r":"resolved"}
rc.13 dev  result={"r":"FLUSH_IN_ACTION"}
rc.13 prod result={"r":"resolved"}
```
**N4_flush_in_effect_apply**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createEffect(() => 1, () => { r.t = attempt(() => S.flush()); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true}}  console=["warn: [FLUSH_IN_EFFECT_CALLBACK] flush() called from inside an effect callback is a no-op: the flush that runs effects is already in progress. Writes made here are pr"]
rc.9 prod  result={"t":{"ok":true}}
rc.13 dev  result={"t":{"ok":true}}  console=["warn: [FLUSH_IN_EFFECT_CALLBACK] flush() called from inside an effect callback is a no-op: the flush that runs effects is already in progress. Writes made here are pr"]
rc.13 prod result={"t":{"ok":true}}
```
**B2_dynamic_static_once**  (dev =, prod =)
```
code: async ({ S, W }) => { const [s, set] = S.createSignal(0); let calls = 0; let C; S.createRoot(() => { C = W.dynamic(() => { calls++; s(); return (p) => "x"; }, { static: true }); }); const atCreate = calls; set(1); await settle(S); return { callsAtCreate: atCreate, callsAfterWrite: calls }; }
rc.9 dev   result={"callsAtCreate":1,"callsAfterWrite":1}
rc.9 prod  result={"callsAtCreate":1,"callsAfterWrite":1}
rc.13 dev  result={"callsAtCreate":1,"callsAfterWrite":1}
rc.13 prod result={"callsAtCreate":1,"callsAfterWrite":1}
```
**B2_dynamic_default**  (dev =, prod =)
```
code: async ({ S, W }) => { const [s, set] = S.createSignal(0); let calls = 0; let out; S.createRoot(() => { const C = W.dynamic(() => { calls++; s(); return (p) => "x"; }); const atCreate = calls; const m = S.createMemo(() => C({})); out = { callsAtCreate: atCreate, m }; }); const afterRender = calls; set(1); await settle(S); return { callsAtCreate: out.callsAtCreate, callsAfterRender: afterRender, callsAfterWrite: calls }; }
rc.9 dev   result={"callsAtCreate":0,"callsAfterRender":1,"callsAfterWrite":2}
rc.9 prod  result={"callsAtCreate":0,"callsAfterRender":1,"callsAfterWrite":2}
rc.13 dev  result={"callsAtCreate":0,"callsAfterRender":1,"callsAfterWrite":2}
rc.13 prod result={"callsAtCreate":0,"callsAfterRender":1,"callsAfterWrite":2}
```
**B2_dynamic_static_write**  (dev =, prod =)
```
code: async ({ S, W }) => { const [, set] = S.createSignal(0); let r; S.createRoot(() => { r = attempt(() => W.dynamic(() => { set(1); return (p) => "x"; }, { static: true })); }); return r; }
rc.9 dev   result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true}
```
**LOADING_ON_OUTSIDE_HOLD**  (dev DIFF, prod =)
```
code: async ({ S }) => { const [key, setKey] = S.createSignal(0); let resolveFns = []; const data = S.createRoot(() => S.createMemo(() => { const k = key(); return new Promise((res) => resolveFns.push(() => res(k))); })); S.createRoot(() => { S.createRenderEffect(() => { try { data(); } catch {} return 1; }, () => {}); S.createLoadingBoundary(() => { data(); return "content"; }, () => "fallback", { on: () => key() }); }); resolveFns.shift()?.(); await settle(S); setKey(1); await settle(S); resolveFns.shift()?.(); await settle(S); return { keys: 1 }; }
rc.9 dev   result={"keys":1}
rc.9 prod  result={"keys":1}
rc.13 dev  result={"keys":1}  console=["warn: [LOADING_ON_OUTSIDE_HOLD] `on` re-armed a Loading boundary, but `computed` is also read outside it and holds the frame: the fallback can never be seen — the fra"]
rc.13 prod result={"keys":1}
```
**EXPORTS** (namespace-key diff only; full lists are in probes/results.json)

```js
async ({ S, J, W }) => ({ signals: Object.keys(S).sort(), solid: Object.keys(J).sort(), web: W ? Object.keys(W).sort() : null })
```

```
rc.9 -> rc.13 dev signals (namespace keys 88 -> 89): {"added":["$RECORD","setConsoleFooter"],"removed":["ownerPath"]}
rc.9 -> rc.13 dev solid (namespace keys 81 -> 85): {"added":["inLiveServerComponentScope","isHydratable","isHydrating","shareAsyncIterable"],"removed":[]}
rc.9 -> rc.13 dev web (namespace keys 117 -> 120): {"added":["getHydrationWriter","ssrElementAttribute","takeHydrationValue"],"removed":[]}
rc.9 -> rc.13 prod signals (namespace keys 88 -> 89): {"added":["$RECORD","setConsoleFooter"],"removed":["ownerPath"]}
rc.9 -> rc.13 prod solid (namespace keys 81 -> 85): {"added":["inLiveServerComponentScope","isHydratable","isHydrating","shareAsyncIterable"],"removed":[]}
rc.9 -> rc.13 prod web (namespace keys 117 -> 120): {"added":["getHydrationWriter","ssrElementAttribute","takeHydrationValue"],"removed":[]}
```


#### leaf-owner / ownerless scopes (rules SC3001, missing-owner), until, static dynamic

**LEAF_memo_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.createMemo(() => 1); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_root_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.createRoot(() => 1); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_signalfn_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.createSignal(() => 1); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_signalvalue_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.createSignal(0); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_effect_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.createEffect(() => 1, () => {}); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"PRIMITIVE_IN_FORBIDDEN_SCOPE"}}  console=["warn: [PRIMITIVE_IN_FORBIDDEN_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_flush_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.flush(); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"Cannot call flush() from inside onSettled or createTrackedEffect. flush() is not reentrant there. Writes made here are p"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"Cannot call flush() from inside onSettled or createTrackedEffect. flush() is not reentrant there. Writes made here are p"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_flush_in_onsettled**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { S.onSettled(() => { r.t = attempt(() => { S.flush(); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"Cannot call flush() from inside onSettled or createTrackedEffect. flush() is not reentrant there. Writes made here are p"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"Cannot call flush() from inside onSettled or createTrackedEffect. flush() is not reentrant there. Writes made here are p"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**LEAF_memo_in_onsettled_unowned**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.onSettled(() => { r.t = attempt(() => { S.createMemo(() => 1); return "allowed"; }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**NOOWNER_effect**  (dev =, prod =)
```
code: async ({ S }) => { const r = attempt(() => S.createEffect(() => 1, () => {})); await settle(S); return r; }
rc.9 dev   result={"ok":true}  console=["warn: [NO_OWNER_EFFECT] Effects created outside a reactive context will never be disposed"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"ok":true}  console=["warn: [NO_OWNER_EFFECT] Effects created outside a reactive context will never be disposed"]
rc.13 prod result={"ok":true}
```
**NOOWNER_cleanup**  (dev =, prod =)
```
code: async ({ S }) => { const r = attempt(() => S.onCleanup(() => {})); return r; }
rc.9 dev   result={"ok":true}  console=["warn: [NO_OWNER_CLEANUP] onCleanup called outside a reactive context will never be run"]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"ok":true}  console=["warn: [NO_OWNER_CLEANUP] onCleanup called outside a reactive context will never be run"]
rc.13 prod result={"ok":true}
```
**NOOWNER_boundary**  (dev =, prod =)
```
code: async ({ S }) => { const r = attempt(() => S.createLoadingBoundary(() => 1, () => 2)); return r; }
rc.9 dev   result={"ok":true}  console=["warn: [NO_OWNER_BOUNDARY] Boundaries created outside a reactive context will never be disposed."]
rc.9 prod  result={"ok":true}
rc.13 dev  result={"ok":true}  console=["warn: [NO_OWNER_BOUNDARY] Boundaries created outside a reactive context will never be disposed."]
rc.13 prod result={"ok":true}
```
**NOOWNER_cleanup_after_await**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { (async () => { await 0; r.t = attempt(() => S.onCleanup(() => {})); })(); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true}}  console=["warn: [NO_OWNER_CLEANUP] onCleanup called outside a reactive context will never be run"]
rc.9 prod  result={"t":{"ok":true}}
rc.13 dev  result={"t":{"ok":true}}  console=["warn: [NO_OWNER_CLEANUP] onCleanup called outside a reactive context will never be run"]
rc.13 prod result={"t":{"ok":true}}
```
**NOOWNER_effect_after_await**  (dev =, prod =)
```
code: async ({ S }) => { const r = {}; S.createRoot(() => { (async () => { await 0; r.t = attempt(() => S.createEffect(() => 1, () => {})); })(); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true}}  console=["warn: [NO_OWNER_EFFECT] Effects created outside a reactive context will never be disposed"]
rc.9 prod  result={"t":{"ok":true}}
rc.13 dev  result={"t":{"ok":true}}  console=["warn: [NO_OWNER_EFFECT] Effects created outside a reactive context will never be disposed"]
rc.13 prod result={"t":{"ok":true}}
```
**B4_until_untrack_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const r = viaMemo(S, () => { S.untrack(() => { S.until(() => s()).catch(() => {}); }); return 1; }); await tick(); return r; }
rc.9 dev   result={"ok":true,"v":1}
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"ok":true,"v":1}
rc.13 prod result={"ok":true,"v":1}
```
**B4_until_in_trackedeffect**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const r = {}; S.createRoot(() => { S.createTrackedEffect(() => { r.t = attempt(() => { S.until(() => s()); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"threw":"Cannot call until inside a reactive scope; await it from an action or another imperative scope."}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"threw":"Cannot call until inside a reactive scope; await it from an action or another imperative scope."}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**B4_until_in_effect_apply**  (dev =, prod =)
```
code: async ({ S }) => { const [s] = S.createSignal(1); const r = {}; S.createRoot(() => { S.createEffect(() => 1, () => { r.t = attempt(() => { S.until(() => s()); return "allowed"; }); }); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true,"v":"allowed"}}
rc.9 prod  result={"t":{"ok":true,"v":"allowed"}}
rc.13 dev  result={"t":{"ok":true,"v":"allowed"}}
rc.13 prod result={"t":{"ok":true,"v":"allowed"}}
```
**DYN_static_async_source**  (dev =, prod =)
```
code: async ({ S, W }) => { let r; S.createRoot(() => { r = attempt(() => { const C = W.dynamic(async () => (p) => "x", { static: true }); return typeof C; }); }); return r; }
rc.9 dev   result={"threw":"dynamic(): a static source must resolve synchronously, not to a promise"}
rc.9 prod  result={"ok":true,"v":"function"}
rc.13 dev  result={"threw":"dynamic(): a static source must resolve synchronously, not to a promise"}
rc.13 prod result={"ok":true,"v":"function"}
```
**REFRESH_invalid_target**  (dev =, prod =)
```
code: async ({ S }) => { const r = attempt(() => S.refresh({})); await tick(); return r; }
rc.9 dev   result={"threw":"INVALID_REFRESH_TARGET"}  console=["warn: [INVALID_REFRESH_TARGET] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"ok":true,"v":{}}
rc.13 dev  result={"threw":"INVALID_REFRESH_TARGET"}  console=["warn: [INVALID_REFRESH_TARGET] (once-per-code repair-guide footer only)"]
rc.13 prod result={"ok":true,"v":{}}
```
**RUNWITHOWNER_disposed**  (dev =, prod =)
```
code: async ({ S }) => { let o, d; S.createRoot((dispose) => { d = dispose; o = S.getOwner(); }); d(); const r = attempt(() => S.runWithOwner(o, () => 1)); return r; }
rc.9 dev   result={"ok":true,"v":1}  console=["warn: [RUN_WITH_DISPOSED_OWNER] runWithOwner called with a disposed owner. Children created inside will never be disposed."]
rc.9 prod  result={"ok":true,"v":1}
rc.13 dev  result={"ok":true,"v":1}  console=["warn: [RUN_WITH_DISPOSED_OWNER] runWithOwner called with a disposed owner. Children created inside will never be disposed."]
rc.13 prod result={"ok":true,"v":1}
```
**ISPENDING_untracked_in_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [k, setK] = S.createSignal(0); let hostRuns = 0, fetches = 0; const out = {}; S.createRoot(() => { const a = S.createMemo(() => { const v = k(); fetches++; return new Promise((r) => setTimeout(() => r(v), 5)); }); const host = S.createMemo(() => { hostRuns++; try { a(); } catch {} return S.untrack(() => S.isPending(() => a())); }); S.createEffect(() => host(), () => {}); }); await new Promise((r) => setTimeout(r, 30)); S.flush(); const h0 = hostRuns, f0 = fetches; setK(1); S.flush(); await new Promise((r) => setTimeout(r, 30)); S.flush(); return { hostRunsBefore: h0, fetchesBefore: f0, hostRunsAfter: hostRuns, fetchesAfter: fetches }; }
rc.9 dev   result={"hostRunsBefore":2,"fetchesBefore":1,"hostRunsAfter":3,"fetchesAfter":2}
rc.9 prod  result={"hostRunsBefore":2,"fetchesBefore":1,"hostRunsAfter":3,"fetchesAfter":2}
rc.13 dev  result={"hostRunsBefore":2,"fetchesBefore":1,"hostRunsAfter":3,"fetchesAfter":2}
rc.13 prod result={"hostRunsBefore":2,"fetchesBefore":1,"hostRunsAfter":3,"fetchesAfter":2}
```
**DISPOSE_in_own_memo**  (dev =, prod =)
```
code: async ({ S }) => { const [s, set] = S.createSignal(0); let runs = 0, disposed; S.createRoot((d) => { S.createMemo(() => { runs++; s(); if (runs === 2) d(); return runs; }); }); set(1); await settle(S); const afterDispose = runs; set(2); await settle(S); return { runsAfterDisposeWrite: afterDispose, runsAfterLaterWrite: runs }; }
rc.9 dev   result={"runsAfterDisposeWrite":2,"runsAfterLaterWrite":2}
rc.9 prod  result={"runsAfterDisposeWrite":2,"runsAfterLaterWrite":2}
rc.13 dev  result={"runsAfterDisposeWrite":2,"runsAfterLaterWrite":2}
rc.13 prod result={"runsAfterDisposeWrite":2,"runsAfterLaterWrite":2}
```
**LAZY_component_body_read**  (dev DIFF, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); const r = {}; S.createRoot(() => { const L = J.lazy(() => Promise.resolve({ default: () => { s(); return "c"; } })); S.createMemo(() => L({})()); }); await settle(S); await tick(); S.flush(); return r; }
rc.9 dev   result={}
rc.9 prod  result={}
rc.13 dev  result={}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <default> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.13 prod result={}
```
**LAZY_component_body_owner**  (dev DIFF, prod =)
```
code: async ({ S, J }) => { const r = {}; S.createRoot(() => { const L = J.lazy(() => Promise.resolve({ default: () => { const o = S.getOwner(); r.hasOwner = !!o; r.isRoot = !!o?._root; r.name = o?._name; return "c"; } })); S.createMemo(() => L({})()); }); await settle(S); await tick(); S.flush(); return r; }
rc.9 dev   result={"hasOwner":true,"isRoot":false,"name":"computed"}
rc.9 prod  result={"hasOwner":true,"isRoot":false}
rc.13 dev  result={"hasOwner":true,"isRoot":true,"name":"<default>"}
rc.13 prod result={"hasOwner":true,"isRoot":false}
```
**LAZY_component_body_write**  (dev =, prod =)
```
code: async ({ S, J }) => { const [, set] = S.createSignal(0); const r = {}; S.createRoot(() => { const L = J.lazy(() => Promise.resolve({ default: () => { r.t = attempt(() => set(1)); return "c"; } })); S.createMemo(() => L({})()); }); await settle(S); await tick(); S.flush(); return r; }
rc.9 dev   result={"t":{"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.9 prod  result={"t":{"ok":true,"v":1}}
rc.13 dev  result={"t":{"threw":"REACTIVE_WRITE_IN_OWNED_SCOPE"}}  console=["warn: [REACTIVE_WRITE_IN_OWNED_SCOPE] (once-per-code repair-guide footer only)"]
rc.13 prod result={"t":{"ok":true,"v":1}}
```

#### solid-js level

**ERRORED_fallback_arity0**  (dev DIFF, prod DIFF)
```
code: async ({ S, J }) => { let calls = 0; const out = {}; S.createRoot(() => { const r = J.Errored({ fallback: () => { calls++; return "fb"; }, get children() { throw new Error("boom"); } }); out.value = attempt(() => (typeof r === "function" ? r() : r)); }); await settle(S); out.fallbackCalls = calls; return out; }
rc.9 dev   result={"value":{"ok":true},"fallbackCalls":0}  console=["error: Error: boom"]
rc.9 prod  result={"value":{"ok":true},"fallbackCalls":0}
rc.13 dev  result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}  console=["error: Error: boom"]
rc.13 prod result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}
```
**ERRORED_fallback_arity1**  (dev =, prod =)
```
code: async ({ S, J }) => { let calls = 0; const out = {}; S.createRoot(() => { const r = J.Errored({ fallback: (err) => { calls++; return "fb"; }, get children() { throw new Error("boom"); } }); out.value = attempt(() => (typeof r === "function" ? r() : r)); }); await settle(S); out.fallbackCalls = calls; return out; }
rc.9 dev   result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}
rc.9 prod  result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}
rc.13 dev  result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}
rc.13 prod result={"value":{"ok":true,"v":"fb"},"fallbackCalls":1}
```
**CREATE_COMPONENT_owner**  (dev =, prod =)
```
code: async ({ S, J }) => { const r = {}; S.createRoot(() => { J.createComponent(() => { const o = S.getOwner(); r.owner = !!o; r.root = !!o?._root; return 1; }, {}); }); return r; }
rc.9 dev   result={"owner":true,"root":true}
rc.9 prod  result={"owner":true,"root":false}
rc.13 dev  result={"owner":true,"root":true}
rc.13 prod result={"owner":true,"root":false}
```
**LAZY_loader_read**  (dev =, prod =)
```
code: async ({ S, J }) => { const [s] = S.createSignal(0); const r = {}; S.createRoot(() => { const L = J.lazy(() => { s(); return Promise.resolve({ default: () => "x" }); }); r.t = attempt(() => J.createComponent(L, {})); }); await settle(S); return r; }
rc.9 dev   result={"t":{"ok":true}}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <wrap> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.9 prod  result={"t":{"ok":true}}
rc.13 dev  result={"t":{"ok":true}}  console=["warn: [STRICT_READ_UNTRACKED] Reactive value read directly in <wrap> will not update. Move it into a tracking scope (JSX, a memo, or an effect's compute function)."]
rc.13 prod result={"t":{"ok":true}}
```