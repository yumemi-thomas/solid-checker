# Audit: `@solidjs/signals@2.0.0-rc.6` — the negative table's `creates` and `reads` rows, re-read

Date: 2026-09-25. Status: **for the repository owner's review**, as the two rc.3
audits it follows were. It re-establishes, on the exact bytes of
`@solidjs/signals@2.0.0-rc.6`, every `@solidjs/signals` row that
`rust/crates/solid-dialect/src/solid_2.rs` carries for rc.3.

**Why it exists.** The corpus never installs rc.3: every `solid-js@2.0.0-rc.3`
declares `@solidjs/signals: ^2.0.0-rc.3` and resolves rc.6, and the floor rows
resolve rc.0 (`docs/precision-backlog.md`, 2026-09-25, "The dialect-callee
refusal is an archive-coverage question"). So no `@solidjs/signals` row bound
anywhere, and the `creates` census refused `getOwner` and `onCleanup` callees
on 59 demanded domain-sites.

**Why it is a new reading, not a carry-over.** rc.6 is a substantial rewrite
(`dist/dev.js` 420,677 -> 550,822 bytes, `dist/prod/signals.js` 15,277 ->
30,115, new `store/next/patch.js` and `patch-hooks.js`). A row names a package
and not a version, so adding the rc.6 archive tuple without re-reading would
have extended 25 rc.3 readings, 19 of them resting on the rc.3 bundled contract
document, to bytes nobody read. Every row below was read on rc.6's own bytes,
in every runtime bundle rc.6's `exports` can select (`dist/prod/**` for the
`import` default, `dist/dev.js` for `development`/`test`, `dist/node.cjs` for
`require`), by the method of
[`2026-09-04-solid-2-rc3-core-primitives-creates.md`](2026-09-04-solid-2-rc3-core-primitives-creates.md)
and
[`2026-09-23-solid-2-rc3-owner-and-context-creates.md`](2026-09-23-solid-2-rc3-owner-and-context-creates.md).
Where rc.3 rested on a summary of the rc.3 bundled contract, the rc.6 row rests
on an implementation reading instead and cites rc.6 bytes.

**Identity.** `@solidjs/signals@2.0.0-rc.6`,
`sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==`
(the integrity 85 ecosystem install trees' `bun.lock` record; no tarball was
downloaded to re-derive it), `package.json` sha256
`de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c` (re-derived).
The per-file digests are pinned at
`benchmarks/package-contract-v2/phase0/rc6/solidjs-signals/files.json`. The
census's archive gate compares the tuple against the certifier's own
authenticated snapshot, so a wrong integrity can only stop the rows from
binding, never bind them to other bytes. In the readings below `<rc6>/` is that
install.

**Performed** by four read-only readers in parallel (groups A to D below); every
one of the 72 citations was recomputed from the bytes by the lead before this
document was written.

## 0. For the owner's sign-off

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| 1 | `getOwner` `creates` | **GRANT** | rc.6 getOwner is `return context` in dist/prod/core/owner.js:199-201, dist/dev.js:2749-2751 and dist/node.cjs:1802-1804. The body contains no invoking form, and all three slices are byte-identical to rc.3's. The solid-js@2.0.0-rc.3 server body (server.js:91-93, pinned bytes) is `return currentOwner`. No create. Caveat (not a verdict input): the file digests are from the on-disk install. The tarball integrity sha51... |
| 2 | `onCleanup` `creates` | **GRANT** | rc.6 onCleanup -> cleanup() stores the caller's function on the current owner's disposal field (prod owner.js:207-211, node.cjs:1810-1814, dev.js:2757-2763). The dev-only branches only emitDiagnostic (installed listeners, captures, the new consoleFooter hook and a console.warn microtask) and then console.warn or throw. That is a cleanups item on a reactive-graph owner, not a create. All three slices are byte-ident... |
| 3 | `createRoot` `creates` | **GRANT** | rc.6 createRoot = createOwner (in-memory owner literal and parent link) + runWithOwner around the caller's init. Its dispose path (disposeChildren/runDisposal/clearDeps) invokes only caller-registered callables and archive hooks, and its only host reach is schedule()->queueMicrotask(flush). The prod owner.js diff from rc.3 is only mangled-name renames, dev createOwner/disposeChildren are textually identical, and n... |
| 4 | `untrack` `creates` | **GRANT** | rc.6 untrack toggles the module `tracking` flag (dev: also strictRead) around the caller's fn, or the installed external-source hook GlobalQueue.ht/_externalUntrack/Ce -> externalSourceConfig.untrack. prod and node.cjs differ from rc.3 only in the hook slot's mangled name, and dev is byte-identical. The solid-js rc.3 server body (server.js:1392-1394) is `return fn()`. No create. Caveat (not a verdict input): the f... |
| 5 | `createSignal` `creates` | **GRANT** | @solidjs/signals' own createSignal. The plain overload builds a signal literal and bound accessors (dev: registerGraph + the DEV.hooks.onGraph hook). The derived overload runs computed -> setupComputedNode -> recompute(e, true), which calls the caller's compute synchronously unless lazy, attaches then/iterator handling to the caller's value, and does archive-internal graph and queue bookkeeping. Every host reach o... |
| 6 | `runWithOwner` `creates` | **GRANT** | rc.6 runWithOwner saves and sets the module context and tracking around the caller's fn and restores them in finally, in all three bundles (the slices are byte-identical to rc.3's). Dev adds only a disposed-owner emitDiagnostic + console.warn. The solid-js rc.3 server body (server.js:82-90) is unchanged. No create. Caveat (not a verdict input): the file digests are from the on-disk install. The tarball integrity s... |
| 7 | `createMemo` `reads` | **GRANT** | At the call event, createMemo's own code (computed, setupComputedNode, recompute(create), handleAsync, accessor) calls no read site in any of the three bundles. Its reads are the caller's compute and members of the caller's returned value. The one guarded read on the stack, read(bridgeSignal) in the wrapper that the _wireExternalSource hook installs after enableExternalSource, is dispositioned as installed-hook be... |
| 8 | `createMemo` `creates` | **GRANT** | The archive has no handle to a browser document or a server runtime. The only host reaches in the closure are queueMicrotask (the archive's own flush) and console. until's new timer and abort listener are unreachable. The node, owner link and snapshot bookkeeping are graph structures, not version-1 registrations. |
| 9 | `createTrackedEffect` `reads` | **GRANT** | At the call event trackedEffect creates a lazy computed, which skips recompute, sets fields, and enqueues run. No read site is reachable in any bundle. The queued computation's later reads are the caller's fn, through staleValues. |
| 10 | `createTrackedEffect` `creates` | **GRANT** | Its closure makes only queueMicrotask and console host reaches. The archive has no document or server-runtime handle. The returned cleanup is a cleanups item, and the leaf owner is owner production. |
| 11 | `onSettled` `reads` | **GRANT** | The call event runs getOwner, then either createTrackedEffect(() => untrack(e)), which is lazy and queued, or globalQueue.enqueue. No read site is reachable in any bundle. The callback's later reads are the caller's. |
| 12 | `onSettled` `creates` | **GRANT** | Both arms reduce to § 3.2's closure or Queue.enqueue plus schedule. Host reaches are queueMicrotask and console only. The archive has no document or server-runtime handle. |
| 13 | `flush` `reads` | **GRANT** | flush's own code (the fn guard, the GlobalQueue.flush drain loop, sweepDormant, commit, lanes) contains no read site. Its drain's reads belong to what other invocations registered, under § reads [Decision 2026-09-10]: recomputed nodes' _fn, queued effects, boundary queues (CollectionQueue.run and RevealController._evaluate reading their own _disabled/_collapsed, the same structure as rc.3), and the new patch consu... |
| 14 | `flush` `creates` | **GRANT** | Every archive function the drain reaches stays inside an archive with no document or server-runtime handle. Its host touches are queueMicrotask and console only. The bodies of callables other invocations registered, such as patch consumers and effects, are not flush's act. |
| 15 | `createStore` `creates` | **GRANT** | Both overloads (createStoreNext; createStoreDerivedNext -> createProjectionNextInternal) register no version-1 resource into any runtime outside the invocation in dist/prod/**, dist/dev.js, or dist/node.cjs. Archive-wide token-level host census (rc6-audit-C.md sec 0.2): the only host reach on any path is queueMicrotask(flush); no document/window/server-context/serializer/external import in any of the three bundles... |
| 16 | `createProjection` `creates` | **GRANT** | createProjectionNext -> createProjectionNextInternal allocates targets/proxies and a projection computed; its derive is the caller's; commits are archive-internal adoption writes. Archive-wide token-level host census (rc6-audit-C.md sec 0.2): the only host reach on any path is queueMicrotask(flush); no document/window/server-context/serializer/external import in any of the three bundles; hooks and caller-supplied ... |
| 17 | `createOptimistic` `reads` | **GRANT** | At the call event: engine-slot install, node allocation, and (fn overload) the caller's compute run inside the node; no export-authored read() call or owned-proxy access in any bundle. Scheduled later: recomputes run the caller's compute; setter/revert are writes. The returned accessor's read on a later separate invocation is expressed by the shape (same line rc.3 drew for createMemo/createOptimistic), not denied ... |
| 18 | `createOptimistic` `creates` | **GRANT** | Node allocation, engine-slot install, archive-internal transitions; no registration into a runtime outside the invocation. Archive-wide token-level host census (rc6-audit-C.md sec 0.2): the only host reach on any path is queueMicrotask(flush); no document/window/server-context/serializer/external import in any of the three bundles; hooks and caller-supplied callables are not this export's behavior (sec 0.3); the r... |
| 19 | `createOptimisticStore` `reads` | **WITHHOLD** | New in rc.6: the export's own landing router (wrapCommit, prod optimistic.js:202-207; dev 11205; cjs 9374) calls stageLanding (prod bytes 15230..15411) -> storeSetterNext(fam.px, draft => stagedApply(draft, ...)); stagedApply (prod bytes 17028..21326; dev 490579..494607; cjs 415134..419431) performs string-key get and ownKeys traps (cur[j], cur[k], Reflect.ownKeys(cur)) on fam.px, the store proxy this invocation c... |
| 20 | `createOptimisticStore` `creates` | **GRANT** | Targets, proxies, family, derived computed, archive-scheduler transitions (createTransition/initTransition), engine-slot installs; no registration into a runtime outside the invocation, including the new landing path. Archive-wide token-level host census (rc6-audit-C.md sec 0.2): the only host reach on any path is queueMicrotask(flush); no document/window/server-context/serializer/external import in any of the thr... |
| 21 | `reconcile` `reads` | **GRANT** | At the call event: closure creation only. On the returned function's application: reconcileNextState makes only brand ($TARGET) lookups on the store proxy and otherwise works on raw backings and the caller's value; no read()/readNodeFast() in reconcile.js or the store.js helpers it calls. Every proxy it can touch is parameter-rooted (the store passed to the returned function, or proxies inside the caller's value),... |
| 22 | `reconcile` `creates` | **GRANT** | Closure at call; adoption writes, setSignal notifications, flush microtask on application; patch/row/slot emission only via hooks installed by third-party registration. The solid-js server.js body only calls setProperty on the caller's state. Archive-wide token-level host census (rc6-audit-C.md sec 0.2): the only host reach on any path is queueMicrotask(flush); no document/window/server-context/serializer/external... |
| 23 | `action` `reads` | **GRANT** | At its call event, action(e) only evaluates and returns an arrow ('return (...r) => new Promise(' in prod and cjs, 'return (...args) => {' in dev), so nothing is invoked and nothing is read. Under the wider scope the rc.3 summary already uses (invoke-action, optimistic-write, settle-or-revert and yield-resume all run inside the returned wrapper), the wrapper's frames match rc.3 byte for byte except for a mangled f... |
| 24 | `action` `creates` | **GRANT** | rc.6's runtime bytes hold no handle on a document, a window, a server runtime or a serializer. Across the call event and every wrapper run, the only host touches reachable are four: the promise handed back to the caller, queueMicrotask(flush) in schedule(), MicrotaskQueue.enqueue's queueMicrotask of a registered apply, and console output (dev also emits diagnostics). None of them registers a version-1 resource. Th... |
| 25 | `snapshot` `creates` | **GRANT** | In every bundle, snapshot -> snapshotNext -> snapshotWalk reaches the same 23 frames: target and family map lookups, the new pendingBackingVisible(e, true) visibility predicates (#3147), materializePB/cloneRaw, isWrappable, optHooks.optimisticView, and property-descriptor builtins. Those 23 frames contain zero host references: no scheduling, no microtask, no timer, no console. Getters, traps and iterators on the c... |

### What the owner is asked to agree with beyond the table

1. **`createMemo` `reads` keeps the external-source line rc.3 drew.** When a
   third party has called `enableExternalSource`, `createMemo`'s first compute
   runs a wrapper the installed hook created and reads a signal that hook made
   during the same call. rc.3 carried this row over the same bytes as hook
   behavior (the 2026-09-04 audit, § 1.3). If a hook-created signal is instead
   "a source the export created", this row falls on rc.3 and on rc.6 alike.
2. **`createOptimistic` `reads` rests on the stated accessor line**: a later,
   separate call of the returned accessor reads the node and is not denied, as
   for `createMemo`.
3. **`createOptimisticStore` `reads` is withheld on rc.6 alone.** New rc.6
   landing code (`wrapCommit` -> `stageLanding` -> `stagedApply`) reads
   property values and keys through the store proxy the call created when its
   derived computation commits under a retained transaction. Those reads are
   untracked; § reads still counts them, and a flat row has no place for the
   condition. rc.3 has none of this code.
4. **The pairing with `solid-js`.** Where `solid-js@2.0.0-rc.3` re-exports a
   declaration from this archive (`createTrackedEffect`, `flush`, `onSettled`,
   `reconcile`, `snapshot`), its `node`/`worker`/`deno` condition runs
   `solid-js`'s own server bodies instead; those were read beside rc.6 and reach
   the same verdicts. A different `solid-js` version is not covered.

### Found about rc.3 while reading (no live row rests on either)

- The rc.3 summaries for `createOptimistic` and `createOptimisticStore` close
  `callbacks: []`, but both functions' overloads invoke the caller's function at
  the call event, in code unchanged since rc.3. No `callbacks` row exists, so no
  answer changes; those summaries are not authority for that domain.
- The rc.3 audit's line-prefix scan misses code written after `*/` on the same
  line (`node.cjs:291`, `prod/core/invariants.js:28` in rc.6); a tokenizer finds
  them. Both are harmless module-initialization reads here, but that scan should
  not be reused on these bundles.

---

## rc.6 re-audit, group A — `getOwner`, `onCleanup`, `createRoot`, `untrack`, `createSignal`, `runWithOwner` (`creates`)

Date: 2026-09-25. Read-only hand implementation census, following
`2026-09-04-solid-2-rc3-core-primitives-creates.md` § 1.3 and
`2026-09-23-solid-2-rc3-owner-and-context-creates.md` § 1 one-for-one, over the
bytes of `@solidjs/signals@2.0.0-rc.6`. It decides only what
`semantic-model.md` § creates lets it decide: whether one invocation performs a
published `create`, meaning it **registers a version-1 resource into a runtime
outside this invocation** (a browser document or a server runtime). Reactive
nodes, owners and computations coming into existence are not `create`
operations, and neither is anything a caller-supplied callable does. A guarded
reach still counts (`[Decision 2026-09-04]`).

Result: **all six rows GRANT.** The dispositions, bundle by bundle, are the same
as in rc.3.

---

### 0. Inputs, identity, and the archive-wide bound

#### 0.1 Identity

Install read:
`<rc6>/`.

- `package.json` sha256 is `de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c`,
  which equals the brief's value. `"version": "2.0.0-rc.6"`.
- Tarball integrity `sha512-lPqwZNLPq1Z9…VB2xw==` is what the tree's `bun.lock`
  records for `@solidjs/signals@2.0.0-rc.6`. **I did not verify it against the
  tarball**, because downloading was out of scope. Every per-file digest below
  was computed from the on-disk install. The repository has no
  `benchmarks/package-contract-v2/phase0/rc6/…/files.json`: only `rc3/` exists.
  So the wiring commit must pin an rc.6 per-file manifest from the tarball with
  that integrity, and confirm these digests match it (§ 7).

Files the citations and walks rest on (sha256 of each whole file):

| File | sha256 |
| --- | --- |
| `dist/prod/core/owner.js` | `39d08c35040e378f70b0751de74b3357711b9ec8d4cd786d32c502d0f75ac7bc` |
| `dist/prod/core/core.js` | `6b36cfb79c0b72f40e2b521901adbf23c90220aed4d5731602bb85c527338076` |
| `dist/prod/signals.js` | `b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729` |
| `dist/dev.js` | `f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73` |
| `dist/node.cjs` | `4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f` |
| `dist/prod/index.js` | `6b0fa739b3fc32060060a8f848040e60db17e851847dc2b93ba578b8742cea15` |
| `dist/prod/core/graph.js` | `881fadca3ebc338f6eb7c53095f2cbc462aa4944b612192b797f843591877256` |
| `dist/prod/core/heap.js` | `9a51b6a2ab02535c8a95886e01fe7087c15bd1282f0026ef9bbbe1979d44df74` |
| `dist/prod/core/scheduler.js` | `c66bec648e3f0b02333ce2283df9fcb9f77ec88038c0507b400c7529b44bf686` |
| `dist/prod/core/async.js` | `59a956fc24b1b3e20dbd5761fde15f70a6f1df2c9c3cd8a265e8aff30d13ca28` |
| `dist/prod/core/external.js` | `8da103ab1675bd0d326fde52d505a639facc02599ca1a7a08d114762132a8a85` |
| `dist/prod/core/verdict.js` | `048963a36425f906d1438c998a6198c6696478ccb64855910afb4a31b25fd929` |
| `dist/prod/core/optimistic.js` | `df689eed618c64169b012706b52eff4e5b58ddc18661a7c5f51453501aaf69f3` |
| `dist/prod/core/lanes.js` | `ace3de1fd73901040c06eac72deb77d99643316092e7a8364f5dd62b886ff2bf` |
| `dist/prod/core/effect.js` | `3c6db10524e9248a3fa04579a796d79dab3115b43cce69f31c179ebf2915b252` |
| `dist/prod/core/error.js` | `4e557cc95b95390f57639715a14b8a6d0b65d310ad52e9e568895480f6b0a1f3` |
| `dist/prod/core/constants.js` | `66d32bb98f73fcd468668844476d046f65bbc7fd55cf4fb3f7a7e9902a148bd2` |
| `dist/prod/core/invariants.js` | `74baa771edca26760a31b64e0f8f926d201aa0b76fdbd72dd3c9ce8dfaaeb89c` |

#### 0.2 Which bundle each condition selects

`package.json` `exports["."]` has these runtime targets:

| Condition | Runtime file |
| --- | --- |
| `import` + `test` / `development` | `dist/dev.js` |
| `import` default | `dist/prod/index.js`, which re-exports from `dist/prod/core/owner.js` (`createRoot`, `getOwner`, `index.js:7`), `dist/prod/core/core.js` (`runWithOwner`, `untrack`, `index.js:3`), and `dist/prod/signals.js` (`createSignal`, `onCleanup`, `index.js:23`) |
| `require` | `dist/node.cjs` |

`main` is `./dist/node.cjs`. `module`, `unpkg` and `jsdelivr` are
`./dist/prod/index.js`. No other condition exists. The bindings were checked:
`dev.js`'s final `export { … }` lists all six names un-aliased, and `node.cjs`
has `exports.createRoot = createRoot` (`:11033`), `createSignal` (`:11035`),
`getOwner` (`:11057`), `onCleanup` (`:11077`), `runWithOwner` (`:11103`) and
`untrack` (`:11121`). Each name has exactly one `function` definition per
bundle.

**Declarations** (the archive the row is keyed on): `dist/types/index.d.ts:1`
exports `createRoot`, `runWithOwner`, `getOwner` and `untrack` from
`./core/index.js`, and `:5` exports `createSignal` and `onCleanup` from
`./signals.js`. The definitions are:

- `core/owner.d.ts:61` `getOwner(): Owner | null`
- `core/owner.d.ts:121` `createRoot<T>(init…, options?)` (`owner.d.ts` has the
  same bytes as rc.3's, sha256 `40102fb1…`)
- `core/core.d.ts:88` `untrack<T>(fn, strictReadLabel?)`
- `core/core.d.ts:167` `runWithOwner<T>(owner, fn)`
- `signals.d.ts:41` `onCleanup(fn: Disposable)`
- `signals.d.ts:262-264` `createSignal`, the same three overloads as rc.3

`dist/types-cjs/index.d.cts:1,5` has the same shape.

**The `solid-js` side of the declaration/runtime split** (rc.3 audit § 1.4):

- In the ecosystem tree, `solid-js` is `2.0.0-rc.3`. Its `package.json` sha256
  is `e703e798…`. `dist/server.js` is `63269da7…` and `dist/server.cjs` is
  `2e2ed583…`. `types/index.d.ts` is `76b94bfb…`. All of these equal the rc.3
  pins (rc.3 audit § 9.3).
- `solid-js` rc.3 resolves `@solidjs/signals@^2.0.0-rc.3` to rc.6, and there is
  no nested copy.
- So a `solid-js` import of these names:
  - resolves its declaration into rc.6 (`types/index.d.ts:1` is a pure
    re-export);
  - runs rc.6 bytes on the browser conditions (`solid.js:2`, `dev.js:2`,
    getters in the `.cjs` files);
  - runs `solid-js/dist/server.js`'s own bodies on `node`/`worker`/`deno`.
    Those bodies are byte-identical to the ones rc.3 audited, and they are
    quoted again in each section.
- Every name `solid-js` rc.3's `solid.js`, `dev.js` and `server.js` import or
  re-export from `@solidjs/signals` is exported by rc.6's `prod/index.js`. None
  is missing.

#### 0.3 Archive-wide host-boundary census of `@solidjs/signals@2.0.0-rc.6`

This is the rc.3 audit's § 1.5, redone on rc.6 with a wider set of names.

**Scope.** I searched every `dist/prod/**/*.js`, `dist/dev.js` and
`dist/node.cjs`, skipping comment lines, for these names: `document`,
`window`, `navigator`, `globalThis`, `addEventListener`, `queueMicrotask`,
`setTimeout`, `setInterval`, `clearTimeout`, `requestAnimationFrame`,
`requestIdleCallback`, `MessageChannel`, `process`, `performance`,
`localStorage`, `sessionStorage`, `fetch`, `Promise`, `Date`, `self`,
`console`, `WeakRef`, `FinalizationRegistry`, `import(`, `require(`,
`XMLHttpRequest`, `WebSocket`, `Worker`, `BroadcastChannel`, `structuredClone`,
`postMessage` and `setImmediate`. I also searched for `import.meta`, `eval(`,
`new Function`, `sharedConfig`, `_$HY` and non-relative imports.

**What the archive does not contain:**

- No import leaves the archive. Every `import` in `dist/prod` is relative, and
  `dev.js` and `node.cjs` have no `import` or `require(`.
- There is no `document` or `window` object. The `window` and `self` matches in
  `dev.js` are local variable names (`checkHotRuns`, `disposeChildren`,
  `computed` and similar).
- There is no network API, no `sharedConfig`, no `_$HY`, and no dynamic code.

**Every code reference that does leave the archive:**

| Site (prod · node.cjs · dev.js) | Enclosing export or helper | What it registers |
| --- | --- | --- |
| `scheduler.js:276` · `node.cjs:703` · `dev.js:1608` `queueMicrotask(flush)` | `schedule()` | a one-shot microtask draining the archive's own queues; no version-1 resource |
| `scheduler.js:290,297` · `node.cjs:717,724` · `dev.js:1632,1638` `console.error` | `haltReactivity` / `notifyHalted` (the halted branch of `schedule()`) | console only |
| `effect.js:72` · `node.cjs:4741` · `dev.js:6218` `console.error` | `runEffect`, which runs at flush | console only |
| `signals.js:287` · `node.cjs:5211` · `dev.js:6797` `queueMicrotask(() => t(e))` | `createReaction`, and the dev `MicrotaskQueue` used by `until`/`resolve` | a microtask; not reached by the six |
| `signals.js:312` · `node.cjs:5236` · `dev.js:6827` `new Promise` | `resolve` | a promise handed to `resolve`'s caller; not reached by the six |
| `signals.js:394,414-415` · `node.cjs:5318,5338-5339` · `dev.js:6940-6941` `Promise`/`queueMicrotask` | `refresh` | same; not reached by the six |
| `signals.js:518,533,560,563` · `node.cjs:5442,5457,5484,5487` · `dev.js:7055,7070,7101,7104` `new Promise`, `setTimeout`/`clearTimeout`, `signal.addEventListener("abort", …)` | `until` (**new in rc.6**) | a timer and a listener on the **caller's** `AbortSignal`; not reached by the six |
| `action.js:77` · `node.cjs:4881` · `dev.js:6417` `new Promise` | `action` | not reached by the six |
| `dev.js:241` `performance.now` / `Date.now`; `dev.js:441` `Date.now()` | the module-init clock, and the tracer's `checkHotRuns` | clock reads |
| `dev.js:1185` `globalThis.process?.env?.COMPANION_CENSUS` | `devCheckFlushStart` (at flush) | an env read |
| `dev.js:1015` `queueMicrotask(() => console.warn(footer))` | `emitDiagnostic` (**new in rc.6**, only when an installed `consoleFooter` hook returns a footer) | a microtask that writes to the console |
| many `dev.js` `console.warn`/`console.log` | dev diagnostics and the attribution tracer | console only |

**Reachability of the export-specific reaches.** None of `resolve`, `refresh`,
`until`, `createReaction` or `action` has an internal caller in any bundle. I
grepped for call sites: the only textual hits are comments, plus `dev.js:6433`,
which is a local promise-resolve parameter inside `action`. `until`'s timer and
`AbortSignal` listener live inside `until`'s own closure (`signals.js:508-572`).
`until`'s one hook, `GlobalQueue.he = notifyAuthoritativeObservers`
(`core.js:727-731`), only does `enqueueSub` + `schedule()`
(`core.js:713-725`).

**`GlobalQueue.*` hook slots.** All of them are archive functions, installed
at module load or by another export:

- `affects.js:107-111`
- `core.js:17,19,730`
- `verdict.js:532-552`
- `effect.js:102`
- `optimistic.js:276-287` (`installOptimisticEngine`)
- `store/next/patch.js:369,494,547` and `store/next/optimistic.js:82,111`
- `external.js:66-69`: this one wraps the caller-configured
  `enableExternalSource` factory and `untrack`

**`node.cjs` versus prod.** In rc.3, `node.cjs`'s bodies were byte-identical to
prod's. In rc.6 `node.cjs` is mangled differently. So I compared every one of
the 266 top-level `function` statements in `dist/prod/**` with its `node.cjs`
counterpart, after normalizing 1-2-character identifiers (mangled properties
and locals):

- 257 are identical.
- The 9 that differ (`handleAsync`, `notifyStatus`, `resolveTransition`,
  `assignOrMergeLane`, `transitionBlocked`, `schedule`, `flush`,
  `computePendingState`, `getLatestValueComputed`) differ only by rollup's
  `$1` renames (`hasActiveOverride$1`, `scheduled$1`).
- So `node.cjs` is prod's code under different names, and the host table above
  holds identically there.

**Consequence**, the same as rc.3's: no call that stays inside
`@solidjs/signals@2.0.0-rc.6` can register a version-1 resource into a browser
document or a server runtime, because the archive has no handle to either. The
new rc.6 reaches (a timer and an `AbortSignal` listener in `until`, and the
diagnostic-footer microtask) register no version-1 resource kind, and none of
them is reachable at the call event of any export in this group.

Terms (`local`, `builtin`, `caller`, `hook`, `host`) and the reach notation
(`always`, `cond:`, `later:`) are the rc.3 audit's § 2.

---

### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 1.1 Implementation, all three bundles

| Bundle | Lines | Body |
| --- | --- | --- |
| `dist/prod/core/owner.js` | 199-201 | ` */ function getOwner() { return context; }` |
| `dist/dev.js` | 2749-2751 | `function getOwner() { return context; }` |
| `dist/node.cjs` | 1802-1804 | ` */ function getOwner() { return context; }` |

`context` is a module-level `let` (`core.js:41`, `dev.js:3847`,
`node.cjs:2715`).

Transitive call table: **0 rows.** The body contains no call, no `new`, and no
non-call invoking form.

#### 1.2 The `solid-js` server condition

`solid-js/dist/server.js:91-93` is `function getOwner() { return currentOwner; }`.
The bytes are the pinned rc.3 ones (§ 0.2). **Same verdict.**

#### 1.3 What changed from rc.3

- The slice digests equal rc.3's in all three bundles (`67fcbebd…` for prod
  and `node.cjs`, `e8cb95b7…` for dev).
- Only the offsets in `dev.js` and `node.cjs` moved.
- Nothing that matters changed.

#### 1.4 Verdict

**GRANT.**

Sign-off: `@solidjs/signals@2.0.0-rc.6`'s `getOwner` is `return context` in all
three bundles and contains no invoking form, so the row denies that one
invocation of `getOwner` performs any `create` (its `returns` is positive, so
this is not a claim that it does nothing).

---

### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 2.1 Implementation, `import` default — `dist/prod/signals.js:57-59`

```js
 */ function onCleanup(e) {
    return cleanup(e);
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `cleanup(e)` | `signals.js:58` → `owner.js:207-211` | always | local | `if (!context) return e;`. Otherwise stores `e` in `context.Ge` as a value, an array `push` (builtin, invokes nothing), or a two-element array. Returns `e`. |

**1 row.** The caller's function is stored but not invoked. It runs later in
`runDisposal` (§ 3), which is a `callbacks` item of whatever disposes the
owner. Registering it on a reactive-graph owner is a `cleanups` item, not a
`create`.

#### 2.2 The other two bundles

- **`dist/dev.js:6523-6551`.** This is byte-identical to rc.3's dev slice
  (`f2008034…`).
  - With no owner, it calls `emitDiagnostic` and then `console.warn` (host,
    console only).
  - When the owner forbids children, it calls `emitDiagnostic` and then
    `throw new Error`.
  - Otherwise it runs `return cleanup(fn)` (`:6550`). `cleanup` is at
    `:2757-2763`, the same shape as prod.
  - `emitDiagnostic` (`:1003-1018`) iterates `diagnosticListeners` and
    `diagnosticCaptures` (installed hooks and module sets). New in rc.6: it
    also invokes an installed `consoleFooter` hook (set via
    `DEV.diagnostics.setConsoleFooter`, `:967-970`) and may
    `queueMicrotask(() => console.warn(footer))` (host, console only).
  - Same verdict.
- **`dist/node.cjs:4981-4983`.** This calls `cleanup` at `:1810-1814`. Same
  verdict.

#### 2.3 The `solid-js` server condition

`solid-js/dist/server.js:97-102` reads `currentOwner`. If there is none it runs
`return fn`. Otherwise it stores `fn` in `o._disposal` exactly as prod does.
The bytes are the rc.3 pins. **Same verdict.**

#### 2.4 What changed from rc.3

- All three `onCleanup` slices are byte-identical to rc.3's (`89ddda30…` for
  prod and `node.cjs`, `f2008034…` for dev).
- The field `cleanup` writes was renamed from `he` to `Ge` in prod, and is
  `xt` in `node.cjs`.
- Dev's `emitDiagnostic` gained the footer hook and its console microtask.
- None of this is a registration into a document or a server runtime.

#### 2.5 Verdict

**GRANT.**

Sign-off: `onCleanup` appends the caller's function to the current owner's
disposal field (in dev it may also emit a diagnostic, `console.warn`, or throw).
The registration is onto a reactive-graph owner and is a `cleanups` item. The
row denies that one invocation of `onCleanup` registers a version-1 resource
into any runtime outside the invocation.

---

### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 3.1 Implementation, `import` default — `dist/prod/core/owner.js:298-301`

```js
 */ function createRoot(e, t) {
    const n = createOwner(t);
    return runWithOwner(n, () => e(() => n.dispose()));
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createOwner(t)` | `owner.js:299` → `:243-273` | always | local | builds an owner literal (`:246-261`) and, when a parent `context` exists, links it as the parent's first child (`:262-271`); field writes only |
| `inheritId(e, n, t)` | `:247` → `:145-147` | always | local | `options.id`, the parent's id, or the parent's next child id |
| `getNextChildId` → `childId` → `formatId` | `:137-139`, `:125-130`, `:158-161` | cond: the parent has an `id` | local | counter arithmetic (`toString(36)` and `String.fromCharCode` are builtins), or `throw new Error("")` (`:129`) |
| `runWithOwner(n, …)` | `:300` → `core.js:979-990` | always | local | § 6 |
| `e(() => n.dispose())` | `:300` | always | **caller** | the caller's `init` |
| `n.dispose()` = `disposeRootSelf` | `:232-234` | later: the caller invokes `dispose` | local | `disposeChildren(this, e)` |
| `disposeChildren` | `:44-107` | later | local, and **caller** at `:105` | sets `REACTIVE_DISPOSED`, recursively tears down the children (`deleteFromHeap`/`queueFor`, `clearDeps`), splices itself out of the parent chain (`:92-98`), calls `runDisposal` (`:99`), then invokes the effect-returned cleanup `e.Rt` (`:102-106`), which is **caller** |
| `GlobalQueue.un(t)` | `:55` | later, cond: companions exist | hook (`verdict.js:538`, `snapCompanionsToState`) | archive-internal companion snap |
| `clearDeps` → `unlinkSubs` → `unobserved` | `graph.js:48-56`, `:10-31`, `:58-62` | later | local, and **caller** at `graph.js:19` (`n.o?.Et?.()`, the caller's `unobserved` option) | unlinks graph edges |
| `deleteFromHeap`, `queueFor` | `heap.js:70-84`, `:7-9` | later | local | heap bookkeeping |
| `runDisposal` | `:109-123` | later | local → **caller** | `t.call(t)` on the functions `onCleanup` registered |
| `markDisposal`, `insertIntoHeap*` | `:14-31`, `heap.js` | later, cond: zombie path | local | flags and heap bookkeeping |

**12 rows.** The only outward reach is `schedule()` → `queueMicrotask(flush)`,
if a companion snap enqueues work (§ 0.3). **No path performs a `create`.** The
owner and its child link are reactive-graph facts.

#### 3.2 The other two bundles

- **`dist/dev.js:2861-2864`.** This is the same body, and its slice is
  byte-identical to rc.3's (`9a66667d…`).
  - `createOwner` (`:2794-2836`) is identical to rc.3's dev `createOwner`. On a
    forbidden parent it runs `emitDiagnostic` + `throw` (`:2813-2823`), and at
    `:2834` it calls `DEV$1.hooks.onOwner?.(owner)` (**hook**).
  - `runWithOwner` is § 6. Its disposed-owner warning cannot fire here, because
    the owner is fresh.
  - `disposeChildren` (`:2584-2655`) is identical to rc.3's dev body. It
    includes `clearSignals` (`:2597`) and `GlobalQueue._snapCompanions`
    (`:2595`, hook). Its `unlinkSubs` (`:2867-2895`) adds the dev
    `unnoteGraphLink` counter decrement.
  - Same verdict.
- **`dist/node.cjs:1901-1904`.** Its slice is byte-identical to prod's
  (`eefc749b…`). The helpers `createOwner` (`:1846-1876`), `disposeChildren`
  (`:1647-1710`), `runDisposal` (`:1712-1726`) and `runWithOwner`
  (`:3653-3664`) are prod's bodies modulo mangling (§ 0.3). Same verdict.

#### 3.3 The `solid-js` server condition

`solid-js/dist/server.js:175-178`:

```js
function createRoot(init, options) {
  const owner = createOwner(options);
  return runWithOwner(owner, () => init(() => disposeOwner(owner)));
}
```

The bytes are the rc.3 pins. `createOwner`, `runWithOwner` and `disposeOwner`
are exactly as rc.3 audit § 3.4 reads them, and there is no host reference.
**Same verdict.**

#### 3.4 What changed from rc.3

- All three `createRoot` slices are byte-identical to rc.3's.
- `diff` of rc.3 and rc.6 `prod/core/owner.js` shows only mangled-property
  renames (`ke`→`xe`, `Ve`→`Le`, `he`→`Ge`, `At`→`Rt`, and so on). Every call is
  unchanged.
- Dev `createOwner`, `disposeChildren` and `runDisposal` are textually
  identical to rc.3's.
- Nothing that matters changed.

#### 3.5 Verdict

**GRANT.**

Sign-off: `createRoot` allocates an owner, links it into the in-memory owner
tree, and runs the caller's `init` under it. Its only host reach is the
scheduler's one-shot `flush` microtask on the dispose path. The row denies that
one invocation of `createRoot` registers a version-1 resource into any runtime
outside the invocation.

---

### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 4.1 Implementation, `import` default — `dist/prod/core/core.js:659-669`

```js
 */ function untrack(e, t) {
    if (GlobalQueue.ht === null && !tracking && true) return e();
    const n = tracking;
    tracking = false;
    try {
        if (GlobalQueue.ht !== null) return GlobalQueue.ht(e);
        return e();
    } finally {
        tracking = n;
    }
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `e()` | `core.js:660`, `:665` | always, unless an external-source hook is installed | **caller** | the caller's thunk |
| `GlobalQueue.ht(e)` | `core.js:664` | cond: `enableExternalSource` installed it (`external.js:68`, `GlobalQueue.ht = externalSourceConfig ? externalUntrack : null`) | **hook** | `externalUntrack` (`external.js:59-61`) returns `externalSourceConfig.untrack(e)`, a callable the caller of `enableExternalSource` supplied |

**2 rows.** `tracking` is a module `let` (`core.js:21`), and toggling it is not
a reactive write.

#### 4.2 The other bundles

- **`dist/dev.js:4567-4581`.** This is byte-identical to rc.3's dev slice
  (`2a929c26…`): `strictRead` is saved, set and restored (a module `let` at
  `:4539`), and the hook is `GlobalQueue._externalUntrack` (installed at
  `:5171`). Same verdict.
- **`dist/node.cjs:3333-3343`.** This is the prod body with the slot spelled
  `GlobalQueue.Ce` (installed at `:3774`). Same verdict.

#### 4.3 The `solid-js` server condition

`solid-js/dist/server.js:1392-1394` is `function untrack(fn) { return fn(); }`.
The bytes are the rc.3 pins. **Same verdict.**

#### 4.4 What changed from rc.3

- The prod slice differs from rc.3's only in the hook slot's mangled name
  (`Gt`→`ht`), and the `node.cjs` slice only in `Ee`→`Ce`.
- Dev is byte-identical.
- The hook's installer and body (`externalUntrack`) are unchanged in shape.
- Nothing that matters changed.

#### 4.5 Verdict

**GRANT.**

Sign-off: `untrack` toggles the module-level `tracking` flag (and, in dev,
`strictRead`) around a call to the caller's `fn`, or to the installed
external-source hook. The row denies that one invocation of `untrack` itself
registers a version-1 resource into any runtime outside the invocation.

---

### 5. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

This is rc.3's R5, `@solidjs/signals`' own `createSignal`. `solid-js`'s
re-declaration (rc.3's R6) is a different archive and stays withheld. It is out
of scope for this group.

#### 5.1 Implementation, `import` default — `dist/prod/signals.js:67-75`

```js
function createSignal(e, t) {
    if (typeof e === "function") {
        const n = computed(e, t);
        n.T &= ~CONFIG_AUTO_DISPOSE;
        return [ accessor(n), setMemo.bind(null, n) ];
    }
    const n = signal(e, t);
    return [ accessor(n), setSignal.bind(null, n) ];
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `signal(e, t)` | `signals.js:73` → `core.js:587-618` | cond: the first argument is not a function | local | builds a signal literal; `ext(i).Et = t.unobserved` stores a **caller** callback without invoking it; the firewall link is `n === null` here; snapshot bookkeeping into the module `snapshotSources` set |
| `accessor(n)` | `:71`, `:74` → `signals.js:61-65` | always | local | `read.bind(null, e)` (builtin, invokes nothing); `t[$REFRESH] = e` |
| `setSignal.bind` / `setMemo.bind` | `:74` / `:71` | always | builtin | bound setters handed back, not invoked |
| `computed(e, t)` | `:69` → `core.js:408-453` | cond: the first argument is a function | local | builds a computed literal with `inheritId(t, n, context)` (§ 3) and `ownerInSnapshotScope`; stores the **caller**'s `unobserved`; calls `setupComputedNode(u, t)` |
| `setupComputedNode` | `core.js:562-585` | cond | local | links the node under `context` (`:565-573`); sets the height; calls `GlobalQueue.Pt(e)` if installed (`:576`, **hook**, `wireExternalSource`); `!t?.lazy && recompute(e, true)` (`:577`), so **the derived compute runs during this call** unless `lazy` is set; snapshot bookkeeping |
| `recompute(e, true)` | `core.js:108-380` | cond: the derived overload, not lazy | local | `bumpNotifyEpoch()` (`:110`); with `t = true` the disposal branch `:112-134` is skipped; sets `context = e` and `tracking = true` (`:158`, `:168`); calls **`e.oe(_)`**, the **caller**'s compute (`:202` under `CONFIG_SYNC`, otherwise `:212`); on an object result, `handleAsync(e, n)` (`:215`, `async.js:204`) attaches `then` and async-iterator handling to the **caller**'s thenable or iterable; then `clearStatus`, `notifyStatus`, `parkLoadingWindow`, `settlePendingSource` and `settleErroredDependents` (`async.js`), `trimStaleDeps` (`graph.js:33-42`), `insertSubs`, `queuePendingNode`, `runInTransition`, `insertIntoHeapHeight`, and `enqueueSub` + `schedule()` (`:376-379`) — all archive-internal graph and queue bookkeeping; `e.C.enqueue` (`:294`) is unreachable because `!t` is false and a computed has no `ge` |
| `GlobalQueue.Be`, `.$e`, `.Xe`, `.et`, `.k`, `.Ue`, `.he` | `core.js` inside `recompute` | cond: the optimistic, lane, verdict or `until` engines are installed | hook (archive functions, § 0.3) | lane, verdict and authoritative-read bookkeeping inside the archive; `.he` is `until`'s `notifyAuthoritativeObservers` (`enqueueSub` + `schedule`) |
| `schedule()` | `core.js:378`, and transitively | later: microtask | local → **host** `queueMicrotask(flush)` (`scheduler.js:276`), or `console.error` when halted | the only host reach; registers no version-1 resource |
| `ext(e)` | `core.js:458-…` | cond | local | lazily allocates the cold-field object |
| `isEqual` / `t.equals` | stored as `pe`, invoked at `core.js:277` | cond | local (`:634-636`) or **caller** | value comparison |
| `NotReadyError` `instanceof` | `core.js:234` | cond | builtin `instanceof` on an archive class | no invocation |

**11 rows.** Every helper lives in `dist/prod/core/*.js`, and § 0.3 establishes
that no file in the archive has a document or server-runtime handle. Here is
everything the derived overload does outside the archive:

- the caller's compute runs (excluded);
- `then` is attached to the caller's thenable (a `callbacks` item);
- a `flush` microtask may be scheduled.

**No path performs a `create`.**

#### 5.2 The other two bundles

- **`dist/dev.js:6557-6566`.** The slice is byte-identical to rc.3's
  (`d1292c1a…`).
  - The plain overload adds `registerGraph(node, getOwner())` (`:6564` →
    `:1052-1059`). That pushes the node into the owner's `_signals` dev array
    and calls `DEV$1.hooks.onGraph?.(value, owner)` (**hook**).
  - `computed` (`:4280-4332`) and `signal` (`:4478-4519`) differ from rc.3's
    dev only in `equals != null ? … :` → `?? isEqual`.
  - `setupComputedNode` (`:4442-4477`) is textually identical to rc.3's: a
    `PRIMITIVE_IN_FORBIDDEN_SCOPE` diagnostic + throw (`:4445-4455`),
    `DEV$1.hooks.onOwner?.(self)` (`:4466`), `GlobalQueue._wireExternalSource`
    (`:4468`), and `recompute` (`:4469`).
  - Dev's `recompute` (`:3904-4251`) calls the same helper set as prod, plus the
    `attrHooks.recomputeStart`, `derivedChanged` and `recomputeEnd` tracer
    hooks. `attrHooks` is `null` until `DEV.attribution.enable()` installs
    `engineHooks` (`:221-223`, `:905`), whose bodies reach only listeners,
    `console.*` and the clock.
  - Same verdict.
- **`dist/node.cjs:4991-4999`.** This is the prod body with `n.D` for `n.T`.
  `computed` is at `:3082-3127`, `setupComputedNode` at `:3236-3259`, `signal`
  at `:3261-3292`, `recompute` at `:2782-3054` and `handleAsync` at
  `:2233-2614`. All are prod's code modulo mangling (§ 0.3). Same verdict.

#### 5.3 What changed from rc.3

- The prod and dev slices are byte-identical to rc.3's (`ce6ade13…` and
  `d1292c1a…`). The `node.cjs` slice differs from rc.3's only by the mangled
  config field name.
- The callee closure grew substantially: `core.js` went from 40.7 KB to
  48.1 KB, `scheduler.js` from 32.0 KB to 45.7 KB, and `async.js` from
  27.8 KB to 30.9 KB.
  - `computed` gained the `loadingValue` and no-snapshot bits.
  - `recompute` gained the `latestReadActive`/`stale` save-restore, the verdict
    re-ask hooks (`GlobalQueue.et`/`.k`), and the `until` wake hook
    (`GlobalQueue.he`).
- Every addition is archive-internal, and § 0.3 bounds the closure.
- The only new host reaches in the archive (the `until` timer and
  `AbortSignal` listener) are unreachable from `createSignal`, and in any case
  they register no version-1 resource.
- **It does not change the rc.3 conclusion.**

#### 5.4 Verdict

**GRANT.**

Sign-off: `@solidjs/signals@2.0.0-rc.6`'s `createSignal` allocates a signal or
computed node and links it into the owner tree. For the derived overload it also
runs the caller's compute synchronously unless `options.lazy` is set
(`core.js:577`). It returns bound accessors. Every host reach on its paths is a
`queueMicrotask(flush)` or a console write. The row denies that one invocation
of `createSignal` registers a version-1 resource into any runtime outside the
invocation. It says nothing about `solid-js`'s own `createSignal`, which stays
withheld.

---

### 6. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 6.1 Implementation, all three bundles

| Bundle | Lines | Body |
| --- | --- | --- |
| `dist/prod/core/core.js` | 979-990 | saves `context` and `tracking`, sets `context = e` and `tracking = false`, runs `return t()` in a `try`, and restores both in `finally` |
| `dist/node.cjs` | 3653-3664 | the same bytes as prod's slice (`5c40363e…`) |
| `dist/dev.js` | 5024-5048 | the same body, preceded by one dev-only branch: when `owner && owner._flags & REACTIVE_DISPOSED`, it calls `emitDiagnostic({code: "RUN_WITH_DISPOSED_OWNER", …})` (`:1003-1018`) and then `console.warn(message)` (**host**, console only) |

`context` and `tracking` are module `let`s (`core.js:41`, `:21`;
`dev.js:3847`, `:3831`; `node.cjs:2715`, `:2695`).

| Callee | Reach | Disposition | What it does |
| --- | --- | --- | --- |
| `t()` / `fn()` | always | caller (parameter-rooted) | the caller's callable; whatever it registers is its own |
| `emitDiagnostic(...)` | dev, disposed owner only | local | pushes to installed listeners and captures; in rc.6 may invoke the installed `consoleFooter` hook and queue a `console.warn` microtask; registers nothing with a host |
| `console.warn(...)` | dev, disposed owner only | host | writes to the console; no resource |

**3 rows**, none of them a registration. Swapping the ambient owner is a change
to reactive-graph state, not a `create`.

#### 6.2 The `solid-js` server condition

`solid-js/dist/server.js:82-90` saves `currentOwner`, sets it, runs
`return fn()` in a `try`, and restores it in `finally`. The bytes are the rc.3
pins. **Same verdict.**

#### 6.3 What changed from rc.3

- All three slices are byte-identical to rc.3's (`5c40363e…` for prod and
  `node.cjs`, `87f9a231…` for dev).
- The only rc.6 difference that can be reached is inside dev's
  `emitDiagnostic`: the console-footer hook and microtask (§ 2.2).
- Nothing that matters changed.

#### 6.4 Verdict

**GRANT.**

Sign-off: `runWithOwner` swaps the module-level owner and tracking state around
a call to the caller's `fn` (in dev it may also emit a diagnostic and
`console.warn` for a disposed owner). The row denies that one invocation of
`runWithOwner` registers a version-1 resource into any runtime outside the
invocation.

---

### 7. Citations, and how they were computed

These are byte offsets of the whole file, read in binary (`python3`,
`open(…, 'rb')`). Each slice was re-read from a second, independent file read
and compared, and its sha256 was recomputed. Each slice was also checked to
begin with the export's own definition (`function <export>(`, after an
optional ` */ `).

**The same script reproduced every rc.3 citation.** On the rc.3 install it
reproduced all 15 core-primitive citations in `solid_2.rs` exactly, including
the ranges and the slice digests.

**Two conventions, mirroring the two rc.3 audits:**

- § 1–5 use the 2026-09-04 convention: from the start of the definition's first
  line (including a leading ` */ `) to the start of the line after the closing
  `}`.
- § 6 uses the 2026-09-23 convention: from the `function` token to the closing
  `}`, inclusive. That is what rc.3's `runWithOwner` row uses
  (`37382..37594`); I confirmed it by re-hashing the rc.3 bytes.

| Export | `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | = rc.3 slice? |
| --- | --- | --- | --- | --- | --- | --- |
| getOwner | dist/prod/core/owner.js | 199-201 | 7691 | 7739 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | yes |
| getOwner | dist/dev.js | 2749-2751 | 114268 | 114310 | `e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb` | yes |
| getOwner | dist/node.cjs | 1802-1804 | 73440 | 73488 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | yes |
| onCleanup | dist/prod/signals.js | 57-59 | 2586 | 2639 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | yes |
| onCleanup | dist/dev.js | 6523-6551 | 277753 | 278599 | `f20080340dc7598692247a9944e2f59a4d823a6b24df127a17b3658622521284` | yes |
| onCleanup | dist/node.cjs | 4981-4983 | 210048 | 210101 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | yes |
| createRoot | dist/prod/core/owner.js | 298-301 | 10535 | 10655 | `eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3` | yes |
| createRoot | dist/dev.js | 2861-2864 | 117762 | 117904 | `9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5` | yes |
| createRoot | dist/node.cjs | 1901-1904 | 76285 | 76405 | `eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3` | yes |
| untrack | dist/prod/core/core.js | 659-669 | 28285 | 28565 | `ac6c7ed9b0b05b086e8d17adb2546ddbf1a5af879a3447d12e3fe5a3b48831a6` | no (`Gt`→`ht`) |
| untrack | dist/dev.js | 4567-4581 | 193515 | 193991 | `2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852` | yes |
| untrack | dist/node.cjs | 3333-3343 | 138485 | 138765 | `7c6477036bc9e674dc99ea792fac64ac38fc9ad394f535163659a8e620bf6767` | no (`Ee`→`Ce`) |
| createSignal | dist/prod/signals.js | 67-75 | 2735 | 3015 | `ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388` | yes |
| createSignal | dist/dev.js | 6557-6566 | 278699 | 279048 | `d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504` | yes |
| createSignal | dist/node.cjs | 4991-4999 | 210197 | 210477 | `e8f670802281f73242b3a9e1d182fa6546a957587c92c93ff17a883108eadeed` | no (`T`→`D`) |
| runWithOwner | dist/prod/core/core.js | 979-990 | 45088 | 45300 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | yes |
| runWithOwner | dist/dev.js | 5024-5048 | 214431 | 215077 | `87f9a23193e8b2d00afedb19b7176d741338b3c996268408c46427cf4df0035f` | yes |
| runWithOwner | dist/node.cjs | 3653-3664 | 155261 | 155473 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | yes |

The `file_sha256` values are in § 0.1 (`dev.js` `f2cf81df…`, `node.cjs`
`4e509636…`, `owner.js` `39d08c35…`, `core.js` `6b36cfb7…`, `signals.js`
`b75f46de…`).

**Caution on the "= rc.3 slice" column.** A slice digest that matches rc.3's
pins only the *subject*: the definition is the same bytes. It does not pin the
callee closure, which did change (§ 5.3). The verdicts above rest on the rc.6
walks, not on the slice equality.

**Wiring prerequisites** (not verdict inputs):

1. The citation test resolves `(archive_path, file_sha256)` against
   `benchmarks/package-contract-v2/phase0/rc3/*/files.json`, and no rc.6
   manifest exists. One must be pinned from the tarball with integrity
   `sha512-lPqwZNLPq1Z9…`, and the five cited `file_sha256` values above must
   match it.
2. The archive arm reads `SOLID_CHECKER_RC3_ARCHIVE_ROOT`, and the tsc-oracle
   install is rc.3, so it needs an rc.6 root.
3. 15 of the 18 slices are byte-identical to already checked-in rc.3 slices under
   `audited-slices/`, but that directory's path scheme carries the
   archive-directory name and the offsets, so they need new paths.


---

## rc.6 re-audit, group B: `createMemo`, `createTrackedEffect`, `onSettled`, `flush` (`reads`, `creates`)

Read-only. The auditor edited nothing in the repository, ran no cargo, and installed nothing. It followed the method of
`docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md` § 1.3 (dispositions `local`,
`builtin`, `caller`, `hook`, `host`) and applied `semantic-model.md` § reads and § creates. The rc.3 rows being
re-audited are all `AuditedCitation::Summary` rows, and each rests on a hand-audited summary in
`pkg/contracts/bundled/solid-v2/solidjs-signals.json`:

| Export | Summary id (rc.3) | Bytes | rc.3 `reads` / `creates` |
| --- | --- | --- | --- |
| `createMemo` | `summary-6970e6d02d81c014fd7c2ef7aee46716c95cb9aac16a28e9f8adb95ece54eab1` | 12806..17314 | `[]` closed / `[]` closed |
| `createTrackedEffect` | `summary-aab0640db7c783a35e1c955cbf19c22197542f3eecb3f89b28433694eb07ff6a` | 29857..33425 | `[]` closed / `[]` closed |
| `onSettled` | `summary-5a08fc896d6f18c5378c69bc27d5fc1ddaeb013364aff1421330341801111663` | 10107..12724 | `[]` closed / `[]` closed |
| `flush` | `summary-00fc668bf5acaf07e617a9118eb0ef43a7dc1ba6359be1ea580793a91a76efbe` | 2598..6903 | `[]` closed / `[]` closed |

### 0. Shared inputs and archive-wide facts (every section relies on these)

#### 0.1 Identity

`@solidjs/signals/package.json` at the rc.6 install has sha256
`de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c`, which matches the brief. The `version` is
`2.0.0-rc.6`. The auditor had no tarball, so it did not re-derive the registry `integrity` and took it from the brief.
Whole-file digests of the cited files:

| File | bytes | sha256 |
| --- | --- | --- |
| `dist/prod/signals.js` | 30115 | `b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729` |
| `dist/prod/core/scheduler.js` | 45716 | `c66bec648e3f0b02333ce2283df9fcb9f77ec88038c0507b400c7529b44bf686` |
| `dist/dev.js` | 550822 | `f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73` |
| `dist/node.cjs` | 473305 | `4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f` |

#### 0.2 Bundles the `exports` map can select

`package.json` `exports["."]` has only `import` {`types`, `test` → `./dist/dev.js`, `development` → `./dist/dev.js`,
`default` → `./dist/prod/index.js`} and `require` {`types`, `default` → `./dist/node.cjs`}. There is no
`browser`/`node`/`worker` condition, so three runtime bundles exist:

- **prod (`import` default).** `dist/prod/index.js:23` re-exports `createMemo`, `createTrackedEffect` and `onSettled`
  from `./signals.js`, and `:15` re-exports `flush` from `./core/scheduler.js`. The ESM closure spans the
  `dist/prod/**` modules (`core/core.js`, `core/async.js`, `core/effect.js`, `core/external.js`, `core/scheduler.js`,
  `core/verdict.js`, `core/optimistic.js`, `boundaries.js`, `store/**`, …).
- **dev (`import` + `development`/`test`).** `dist/dev.js` is a single file.
- **cjs (`require`).** `dist/node.cjs` is a single file.

None of the three imports any non-relative specifier. The grep for `^import … from "[^.]`, `require("…")` and dynamic
`import(` finds nothing, so the archive's runtime closure is self-contained.

#### 0.3 How the walk was done

The auditor read the export bodies and every call-event callee by hand in `dist/dev.js`, which has unmangled names. It
cross-checked that walk with a scripted over-approximate call graph. The script
(`scratchpad/cg.cjs`, acorn 8.18.0 from `packages/cli/node_modules`) parses each bundle and resolves ESM imports for
prod. It links calls by identifier and by class-method name. It resolves function-valued slots
(`GlobalQueue.X = fn`, `let hook = fn`) to their assigned functions, and it reports every call it cannot resolve. The
same graph was built for prod and cjs. Top-level function names survive mangling, so the three closures can be
compared name by name, and they agree (§ 0.5). The script is a discovery and cross-check aid. The dispositions
below are the hand reading.

#### 0.4 Archive-wide host-boundary census (`creates`)

The auditor grepped `dist/prod/**/*.js`, `dist/dev.js` and `dist/node.cjs` for `document`, `window`, `navigator`,
`globalThis`, `addEventListener`, `queueMicrotask`, `setTimeout`, `setInterval`, `requestAnimationFrame`,
`MessageChannel`, `process`, `performance`, `localStorage`, `fetch`, `Promise`, `Date`, `sharedConfig`, `require`,
`XMLHttpRequest`, `WebSocket`, `Worker`, `setImmediate`, `FinalizationRegistry` and `WeakRef`, excluding comment lines.
The code references are:

| Site | Code | Registers |
| --- | --- | --- |
| `prod/core/scheduler.js:276`, `dev.js:1608`, `node.cjs:703` | `schedule()`: `queueMicrotask(flush)` | a one-shot microtask that runs the archive's own `flush`; no version-1 resource |
| `prod/signals.js:287`, `dev.js:6797`, `node.cjs:5211` | `MicrotaskQueue.enqueue`: `queueMicrotask(() => t(e))` | the same kind of microtask |
| `dev.js:1015` | `emitDiagnostic`: `queueMicrotask(() => console.warn(footer))` | console only |
| `prod/signals.js:312`, `:394`, `:414-415`, `:518`; `dev.js:6417`, `:6827`, `:6940-6941`, `:7055`; `node.cjs:5236`, `:5318`, `:5338-5339`, `:5442`; `prod/core/action.js:77`, `node.cjs:4881` | `new Promise`, `Promise.resolve`, `queueMicrotask` in `resolve`, `refresh`, `until` and the function `action` returns | promises handed back to that export's caller; not reached by any group-B export |
| `prod/signals.js:560`, `:563`; `dev.js:7101`, `:7104`; `node.cjs:5484`, `:5487` | `until`: `setTimeout(…, options.timeout)` and `signal.addEventListener("abort", …)` on the caller's `AbortSignal` | a timer and a listener on a caller value. Neither is a version-1 resource kind, and only `until` reaches them. **New since rc.3**, and not reached by any group-B export (§ 0.5) |
| `dev.js:241`, `:441` | `performance.now()` / `Date.now()` | clock reads; module init and dev diagnostics |
| `dev.js:1185`, `prod/core/invariants.js:28` | `globalThis.process?.env?.COMPANION_CENSUS` | a module-initialization read whose value is discarded |
| every bundle | `console.*` | console only |

The rc.6 archive has no reference to `document`, to a server render context (`sharedConfig`, `serialize`, a
request/response or stream), or to any network API. The `window` hits in `dev.js:455-498` are a local variable in the
hot-scope diagnostics. Consequently, as in the rc.3 audit's § 1.5, no code inside `@solidjs/signals@2.0.0-rc.6` can
register a version-1 resource into a browser document or a server runtime, because the archive holds no handle to
either. That bounds every registered computation, queue callback and installed archive hook the drain can reach. Only
caller-supplied or third-party callables could perform a `create`, and § creates excludes those.

#### 0.5 Archive-wide read-site census (`reads`)

The archive's observation entry points are `read(el)`, `readNodeFast(el)` (the store fast path) and the
`pendingCheckRead`/`latestRead` hooks that `read` dispatches to. Every dependency link outside `read` is in
`readNodeFast` or `pendingCheckRead` (`link(` call sites: `dev.js:4662`, `:4700`, `:4717`, `:4760`, `:4774`, `:5962`).

**Direct call sites of `read(`** in `dev.js`, by enclosing function:

- `read` itself (`:4791`, its error retry);
- `wireExternalSource` (`:5159`);
- `getLatestValueComputed` (`:5842`) and `latestRead` (`:5898`), both reached only through `read`;
- `isPending` (`:6057`) and `refresh` (`:6962`), both public exports;
- the store traps and helpers `serveDataKey`, `firewallGate`, the trap objects' `get`/`has`/`ownKeys`, and `deepNext`;
- the boundary code `createBoundChildren`, `createCollectionBoundary`, `RevealController._evaluate` and
  `CollectionQueue.run`.

**Value uses of `read`.** The only one is `read.bind(null, node)` in `accessor` (`dev.js:6553`, `node.cjs:4986`,
`prod/signals.js:62`). `bind` invokes nothing.

**Comparison with rc.3.** The rc.3 `dev.js` read-site set is the same set of functions (`:3922`…`:10079`). rc.6
**added no new direct read site**.

**Closure check.** For `createMemo`, `createTrackedEffect` and `onSettled`, the auditor removed two nodes from the
call-only closure and re-checked it in all three bundles:

- the `_wireExternalSource` hook target (`wireExternalSource`);
- `flush`, which these exports reach only from `handleAsync`'s `asyncWrite` and the iterator's post-drain continuation
  (§ 1.3). Both are async-emission events, never the call event.

After that removal, the closure contains **no** direct read site: dev 113/117/119 functions, cjs 107/110/112, prod
107/110/112.

For `flush`, removing `CollectionQueue.run`, `RevealController._evaluate`/`B`/`gn` and `wireExternalSource` likewise
leaves no read site: dev 187, cjs 177, prod 178 functions.

#### 0.6 The declaration/runtime split (context only; outside the verdicts)

The ecosystem installs `solid-js@2.0.0-rc.3`, whose dependency `"@solidjs/signals": "^2.0.0-rc.3"` resolves to this
rc.6 install. `solid-js/types/index.d.ts:1` re-exports `createTrackedEffect`, `flush` and `onSettled` from
`@solidjs/signals`, so a `solid-js` import of those three keys into these rows.

Under the `node` condition such an import runs `solid-js/dist/server.js`'s own bodies instead:

- `createTrackedEffect` (`:874-877`) and `onSettled` (`:1421-1424`) are `getOwner()` plus `getNextChildId(o)` when the
  owner has an id;
- `flush` (`:1395`) is `function flush() {}`, which does not call a passed `fn`.

None of the three reads or creates. `createMemo` is re-declared at `solid-js/types/index.d.ts:8` from
`./client/hydration.js`, so a `solid-js` `createMemo` does not key into the `@solidjs/signals` row. These are rc.3
`solid-js` bytes, not rc.6, and the rows are unaffected either way. The split is recorded here as the rc.3 audit's
§ 1.4 asks.

---

### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `reads: [] closed` denies for **one invocation at its call event**: that
`createMemo`'s own code observes a reactive source's value on that stack. The later re-executions of the memo's
registered computation are out of scope for this claim, and their reads are the caller's compute's reads.

#### 1.1 Definitions

| Bundle | Definition | Body |
| --- | --- | --- |
| prod | `dist/prod/signals.js:77-79`, bytes 3016..3083 | `function createMemo(e, t) { return accessor(computed(e, t)); }` |
| dev | `dist/dev.js:6567-6569`, bytes 279048..279137 | `function createMemo(compute, options) { return accessor(computed(compute, options)); }` |
| cjs | `dist/node.cjs:5001-5003`, bytes 210478..210545 | byte-identical to prod (same `slice_sha256`) |

#### 1.2 Call-event walk (dev line numbers; prod/cjs equivalents in brackets)

| Callee | Site | Reach | Disposition | Reads? |
| --- | --- | --- | --- | --- |
| `computed(fn, options)` | `dev.js:4280-4332` [`prod/core/core.js:408`, `node.cjs:3082`] | always | local | Builds the node literal. `options?.equals ?? isEqual`; `"loadingValue" in options` (an `in` on the caller's options object: a caller-supplied receiver, excluded by [Decision 2026-09-10]); `ext` (`:4337`, allocation). No read. |
| `inheritId` → `getNextChildId` → `childId` → `formatId` | `dev.js:2692`, `:2684`, `:2671`, `:2707` | cond: owner has id | local / builtin (`toString(36)`, `String.fromCharCode`) | No |
| `ownerInSnapshotScope(context)` | `dev.js:3851` | cond: `snapshotCaptureActive` | local | Field walk. No |
| `setupComputedNode(self, options)` | `dev.js:4442-4477` [`core.js:562`, `node.cjs:3236`] | always | local | Links into the owner. Dev only: the forbidden-scope `emitDiagnostic` + `throw`, and `DEV$1.hooks.onOwner?.(self)` (**hook**, devtools). |
| `GlobalQueue._wireExternalSource(self)` | `dev.js:4468` [`core.js:576` `GlobalQueue.Pt`, `node.cjs:3250` `GlobalQueue.Re`] | cond: `enableExternalSource` was called by someone | **hook**, installed by `enableExternalSource` → `syncExternalHooks` (`dev.js:5169-5172`). The 2026-09-04 audit § 1.3 names `GlobalQueue._wireExternalSource` explicitly as an installed hook. | Its body (`dev.js:5152-5162`, `prod/core/external.js:44-57`, `node.cjs:3750-3763`) creates `bridgeSignal`, calls the third party's `externalSourceConfig.factory`, and replaces `self._fn` with `prev => { read(bridgeSignal); return source.track(prev); }`. See § 1.3. |
| `recompute(self, true)` | `dev.js:3904-4251` [`core.js:108`, `node.cjs:2782`] | cond: `!options.lazy` | local | On `create`: skips the `!create` disposal block; `_recomputeLane` only when optimistic-dirty (never on a fresh node); invokes `el._fn(value)` (**caller** — or the § 1.3 wrapper); `handleAsync` (below); `clearStatus`, `notifyStatus`, `settlePendingSource`, `queuePendingNode`, `insertSubs` (no subs yet), `trimStaleDeps`; archive hooks installed at module init (`_updatePendingSignal`, `_updateChildCompanions`, `_syncCompanions`, `_applyReask`, `_repollVerdicts`, `_laneAsyncPending`/`Settled`, `_notifyAuthoritativeObservers`); `el._equals(…)` (**caller** option or `isEqual` = `a === b`); dev `attrHooks.*` (**hook**, attribution engine). None is a read site (§ 0.5). |
| `handleAsync(el, result)` | `dev.js:3258-3723` [`prod/core/async.js:204`, `node.cjs:2233`] | cond: the compute returned an object | local | `result[Symbol.asyncIterator]`, `isThenable(result)`, `result.then(…)`, `it.next()`: members of the value the **caller's** compute returned, a caller-supplied receiver, excluded by § reads. On a synchronous settle it only stashes `syncValue` (`isSync && initialRead`). `asyncWrite` and `handleError` run later, on async emission. |
| `notifyStatus` → `statusNotifierOf` | `dev.js:3745-3819`, `:4436` | cond: the compute threw | local | A fresh memo has no `_x._notifyStatus` and no `_type`, so no notifier is called; `forEachDependent` finds no subscribers. No read. |
| `accessor(node)` | `dev.js:6552-6556` [`prod/signals.js:61-65`, `node.cjs:4985`] | always | local / builtin | `read.bind(null, node)` creates a bound function and invokes nothing. `fn[$REFRESH] = node`. |

Host touches on the call stack: none except dev's `emitDiagnostic` (console) and `schedule()` → `queueMicrotask(flush)`
when work is queued (§ 0.4).

#### 1.3 The one guarded read on the stack, and why it is not `createMemo`'s

When a third party has installed an external source (`enableExternalSource(config)`), `createMemo`'s initial compute
runs the wrapper `wireExternalSource` installed on the new node. That wrapper performs a **tracked**
`read(bridgeSignal)` of a signal `wireExternalSource` created during this same call:

- `dev.js:5158-5161`:
  ```js
  self._fn = prev => {
      read(bridgeSignal);
      return source.track(prev);
  };
  ```
- `prod/core/external.js:53-56`: `e.oe = e => { read(n); return r.track(e); };`
- `node.cjs:3759-3762`: the same body, with mangled names.

It is dispositioned **hook**, not `local`, on exactly the rule the rc.3 audits apply. The rule is § 1.3's "installed
hook — a callable the export did not define and the caller did not supply, reached through module state
(`… GlobalQueue._externalUntrack`/`_wireExternalSource`)". Its body is therefore not this export's behavior. § reads
[Decision 2026-09-10] ("a read performed by a callable this export did not author is not this export's read") points
the same way.

The bytes are **unchanged from rc.3** (`rc.3 dev.js:4388-4396`, the same `GlobalQueue._wireExternalSource` call at
`:3748`), and the rc.3 summary closed `reads: []` over them. **Flag for the owner.** If this hook-created source were
instead read as "a source the export created" (the clause that withholds `solid-js`' `createEffect`), both the rc.3
row and this one would fall. This audit applies the method as written.

#### 1.4 Later events (context only)

The continuations `asyncWrite`/`handleError` that `createMemo` attaches to the caller's thenable run at async emission.
They call:

- `setSignal` (a write);
- `_syncCompanions` and `notifyStatus`;
- `settlePendingSource`, `schedule()` and `flush()`, whose drain belongs to its registrants (§ 7);
- `then?.()` → `it.next()` on the caller's iterator.

None of them is a read site.

#### 1.5 What changed from rc.3

The export body is byte-identical in dev (`diff`). The changes elsewhere are:

- `computed`: `equals != null ?` became `??`.
- `recompute`: adds `releaseFlightTeardown` (on the `!create` path), a `settlePendingSource` re-park release, the
  `CONFIG_DIRECT_COMMIT` branch, and `_notifyAuthoritativeObservers`, which is gated on
  `CONFIG_OPTIMISTIC`/`AUTHORITATIVE_OBSERVED` and so is off for a fresh memo.
- `handleAsync`: adds `attrHooks.flightStart`, `_reask` bookkeeping and `_flightTeardown`.

None of these adds a read site or a new call-event path to one.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `reads: [] closed` for `(@solidjs/signals@2.0.0-rc.6, createMemo)` denies that one invocation of
`createMemo`, at its call event, observes a reactive source through its own code. The reads of the caller's compute
are the caller's. The only guarded read on that stack is inside the `_wireExternalSource` hook that
`enableExternalSource` installs, and it is dispositioned as that hook's behavior, not `createMemo`'s.

---

### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `creates: [] closed` denies for one invocation, including anything the call
schedules to a later event: that `createMemo` registers a version-1 resource into a browser document or a server
runtime.

The definitions and the call-event walk are § 1.1 and § 1.2. Every callee there, and every function in the full
call-only closure, is one of three things:

- a module-local function of this archive;
- an engine builtin;
- the caller's compute, `equals` or `unobserved`, or an installed hook: `DEV.hooks.onOwner`, `attrHooks`, or the
  external-source `factory`/`track`/`untrack` of whoever called `enableExternalSource`.

Across the three bundles, the host touches in that closure are:

- `queueMicrotask` (`schedule`, `MicrotaskQueue.enqueue`, dev `emitDiagnostic`);
- `console` (`haltReactivity`, `notifyHalted`, and the dev diagnostics).

The one-shot `flush` microtask registers no version-1 resource. Everything it later does stays inside the archive,
which has no handle to a document or a server runtime (§ 0.4).

The later `asyncWrite` continuation attaches only to the caller's thenable or iterator. `async.js` constructs no
promise, and `new Promise` appears only in `resolve`, `refresh`, `until` and `action`, none of which is reachable here.

The node, its owner link, the `bridgeSignal` a hook may create, and the snapshot bookkeeping in `setupComputedNode`
(`snapshotSources.add(self)`) are reactive-graph or private-module structures. § creates excludes them: they are
neither a version-1 resource kind nor a runtime registry.

**What changed from rc.3.** § 1.5. The new rc.6 host references (`until`'s timer and abort listener) are unreachable
from `createMemo`.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `creates: [] closed` for `(@solidjs/signals@2.0.0-rc.6, createMemo)` denies that one invocation of
`createMemo` registers a version-1 resource into a browser document or a server runtime, now or at a later event it
schedules. It allocates a computed node, links it under the current owner, runs the caller's compute, and may queue the
archive's own `flush` microtask.

---

### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `reads: [] closed` denies for one invocation at its **call event**. The tracked
effect's computation does not run at the call event: it is created `lazy` and queued. What that registered computation
reads when a later `flush` executes it is out of scope for this claim. Those reads are the caller's `fn`, reached
through `staleValues(fn)`.

#### 3.1 Definitions

| Bundle | Definition | Body |
| --- | --- | --- |
| prod | `dist/prod/signals.js:221-223`, bytes 8995..9063 (begins ` */ function`) | `trackedEffect(e, t);` |
| dev | `dist/dev.js:6725-6727`, bytes 285726..285859 | `trackedEffect(compute, { ...options, name: options?.name ?? "trackedEffect" });` |
| cjs | `dist/node.cjs:5145-5147`, bytes 216457..216525 | byte-identical to prod |

#### 3.2 Call-event walk

| Callee | Site | Reach | Disposition | Reads? |
| --- | --- | --- | --- | --- |
| `{ ...options, name }` (dev) | `dev.js:6726` | always | builtin (own-property spread of the caller's `options`) | No. A spread of a caller-supplied receiver is the caller's, per [Decision 2026-09-10]. |
| `trackedEffect(fn, options)` | `dev.js:6266-6315` [`prod/core/effect.js:108-135`, `node.cjs:4777`] | always | local | See below. |
| ↳ `computed(() => {…}, { ...options, lazy: true })` | `dev.js:6276` [`effect.js:116-125`] | always | local | § 1.2 without the compute: `setupComputedNode` sees `lazy`, so `!options?.lazy && recompute(...)` is skipped (`dev.js:4469`, `core.js:577`). The archive arrow (`prevCleanup?.()`, `staleValues(fn)`) is **not run** at the call event. `_wireExternalSource`, when installed, only wraps `_fn` here. Its `read(bridgeSignal)` would run inside the later execution (§ 1.3's hook). |
| field writes (`_cleanup`, `_config`, `_modified`, `_type = EFFECT_TRACKED`, `_run`) | `dev.js:6292-6299` [`effect.js:126-133`] | always | local | No |
| `node._queue.enqueue(EFFECT_USER, run)` | `dev.js:6300` → `Queue.enqueue` `:1699-1710` [`scheduler.js:358`, `node.cjs:785`] or `MicrotaskQueue.enqueue` `:6796` | always | local | Pushes `run` (to a lane queue if `currentOptimisticLane`); `schedule()` → `queueMicrotask(flush)`. No read. |
| dev: `emitDiagnostic` + `console.warn` (`NO_OWNER_EFFECT`) | `dev.js:6301-6313` | cond: no owner | local / host (console) | No |

In the call-only closure, with `wireExternalSource` and `flush` removed, no direct read site is reachable (§ 0.5; dev 117,
cjs 110, prod 110).

#### 3.3 What changed from rc.3

The export body is byte-identical in dev. `trackedEffect` **dropped** its per-node `_x._notifyStatus` closure (rc.3,
which routed errors through `node._queue.notify`). rc.6 dispatches through the shared `notifyEffectStatus` installed at
module init (`dev.js:6316`, `setEffectStatusNotify`). That path is not on the call event, and it contains no read site.
The call-event closure also lost `CollectionQueue.notify`/`_onFn`, `GlobalQueue.notify` and `haltReactivity` (from
rc.3's closure). Nothing was added that reads.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `reads: [] closed` for `(@solidjs/signals@2.0.0-rc.6, createTrackedEffect)` denies that one invocation
of `createTrackedEffect`, at its call event, observes a reactive source through its own code. It creates a lazy leaf
computation and queues its first run. What that registered computation later reads is its caller's `fn`.

---

### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `creates: [] closed` denies for one invocation, including the queued run it
schedules: that `createTrackedEffect` registers a version-1 resource into a browser document or a server runtime.

The walk is § 3.2. Its host touches are:

- `queueMicrotask`, through `schedule()` or `MicrotaskQueue.enqueue`;
- dev's `console.warn` for an ownerless effect.

The queued `run` (`dev.js:6267-6275`, `effect.js:109-115`) is archive code. It calls `recompute(node)`, which runs the
archive arrow: the previous cleanup (the **caller's** returned function), then `staleValues(fn)` (the **caller's**
`fn`, `dev.js:5049-5056`). It then stores the returned cleanup as `node._cleanup`. That is a `cleanups` item, as the
rc.3 summary's `returned-cleanup` records, and not a `create`.

The archive has no document or server-runtime handle (§ 0.4). The leaf owner the effect creates is exactly the thing
the rc.3 summary declared as a `resources` entry beside `creates: []`. § creates says such a declaration and owner
production are not `creates` items.

**What changed from rc.3.** § 3.3. No new host reach is on this path.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `creates: [] closed` for `(@solidjs/signals@2.0.0-rc.6, createTrackedEffect)` denies that one invocation
of `createTrackedEffect` registers a version-1 resource into a browser document or a server runtime, at the call or in
the queued run it schedules. It creates a lazy, children-forbidden computation, queues it on the owner's queue, and
records the caller's returned cleanup.

---

### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `reads: [] closed` denies for one invocation at its **call event**. The
callback runs later, on the settle flush. What that queued or registered callback reads when it executes is out of
scope, and it is the caller's `callback`. It runs under `untrack` in the owned path, which clears no read's status as a
read, but the read is still the caller's.

#### 5.1 Definitions

| Bundle | Definition |
| --- | --- |
| prod | `dist/prod/signals.js:678-687`, bytes 29410..29932 (begins ` */ function`) |
| dev | `dist/dev.js:7217-7239`, bytes 306936..308089 |
| cjs | `dist/node.cjs:5602-5611`, bytes 236878..237400. The body equals prod with the owner-config field mangled `D` instead of `T`, so its `slice_sha256` differs. |

prod body:
```js
function onSettled(e) {
    const t = getOwner();
    t && !(t.T & CONFIG_CHILDREN_FORBIDDEN) ? createTrackedEffect(() => untrack(e), undefined) : globalQueue.enqueue(EFFECT_USER, () => { e(); });
}
```
(The comment is elided.) Dev passes `{ name: "onSettled" }`. Its unowned arrow checks the callback's return and, when
a cleanup comes back, calls `emitDiagnostic` and throws `SETTLED_CLEANUP_UNOWNED`.

#### 5.2 Call-event walk

| Callee | Reach | Disposition | Reads? |
| --- | --- | --- | --- |
| `getOwner()` (`dev.js:2749`, `return context`) | always | local | No. A module `let` is not a reactive source. |
| `createTrackedEffect(() => untrack(e), …)` | cond: owned and not children-forbidden | local, § 3.2 | No read at the call event. The arrow and `untrack` run only in the later execution. |
| `globalQueue.enqueue(EFFECT_USER, () => {…})` | cond: unowned or children-forbidden owner | local, `Queue.enqueue` | Push plus `schedule()`. No read. |

The call-only closure, with `wireExternalSource` and `flush` removed, reaches no read site (dev 119, cjs 112, prod 112).

#### 5.3 What changed from rc.3

The export body is byte-identical in dev. It inherits § 3.3.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `reads: [] closed` for `(@solidjs/signals@2.0.0-rc.6, onSettled)` denies that one invocation of
`onSettled`, at its call event, observes a reactive source through its own code. It reads the ambient owner variable
and either creates a tracked leaf effect or queues a user-effect callback. What the queued callback later reads is its
caller's.

---

### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `creates: [] closed` denies for one invocation, including the settle-time run
it schedules.

The walk is § 5.2. Both arms reach only § 3.2's closure or `Queue.enqueue` → `schedule()` → `queueMicrotask(flush)`.
The later run calls:

- `untrack(e)` (`dev.js:4567-4581`), which may call the **hook** `GlobalQueue._externalUntrack`;
- the caller's callback, or the caller's callback directly in the unowned arm.

Dev's unowned arm adds `emitDiagnostic` + `throw` (console only). There is no document or server-runtime handle
(§ 0.4). The leaf owner of the rc.3 summary's `onSettled-leaf-owner` declaration is owner production, not a `create`.

**What changed from rc.3.** § 5.3. There is no new host reach.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `creates: [] closed` for `(@solidjs/signals@2.0.0-rc.6, onSettled)` denies that one invocation of
`onSettled` registers a version-1 resource into a browser document or a server runtime, at the call or when its
callback later runs. It creates a tracked leaf effect under the current owner, or queues a user-effect callback, and
the callback's body is the caller's.

---

### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** `flush`'s whole act is on its call stack: an optional `fn()` and then a synchronous drain. The
question is what `flush`'s **own** code reads there. The drain executes computations, effects, boundary queues and
patch consumers that **other** invocations registered. § reads [Decision 2026-09-10] (authorship, not timing) assigns
their reads to whoever registered them, and they are what "later executions of a registered computation" means here.

#### 7.1 Definitions

| Bundle | Definition |
| --- | --- |
| prod | `dist/prod/core/scheduler.js:932-956`, bytes 41371..42076 |
| dev | `dist/dev.js:2264-2321`, bytes 95274..97566 |
| cjs | `dist/node.cjs:1359-1383`, bytes 57834..58541. Same as prod except the mangled `globalQueue.de` versus `.fn` and the name `scheduled$1`. |

prod body (comments elided):
```js
function flush(e) {
    if (e) { syncDepth++; try { return e(); } finally { try { flush(); } finally { syncDepth--; } } }
    if (globalQueue.fn) { return; }
    if (halted) return;
    while (scheduled || activeTransition) { globalQueue.flush(); }
}
```

Dev differs in three ways:

- the re-entrant branch throws inside tracked-queue callbacks, or `emitDiagnostic` + `console.warn` inside effect
  callbacks;
- the loop guard counts to `1e5` and throws an attributed `Potential Infinite Loop` error;
- the `globalQueue.flush()` loop is the same.

#### 7.2 Call-stack walk

| Callee | Disposition | Reads? |
| --- | --- | --- |
| `e()` | **caller** | The caller's. |
| `GlobalQueue.flush()` (`dev.js:1805-1917` [`scheduler.js:468`, `node.cjs:895`]) | local | Own code: `sweepDormant` (**new in rc.6**: disposes unobserved auto-dispose nodes via `unobserved` → `disposeChildren` → `runDisposal`, which invokes cleanups other exports registered); `runHeap(dirtyQueue, GlobalQueue._update = recompute)`; `transitionComplete`, `stashQueues`/`restoreQueues`, `reassignPendingTransition`, `commitPendingNodes`, `finalizePureQueue`, `_runLaneEffects`, `this.run(EFFECT_RENDER/USER)` → `runQueue(e[i](t))`; dev invariant checks and `DEV.hooks.onUpdate?.()` (**hook**). None of these is a direct read site. |
| `recompute(node)` of queued/dirty nodes → `node._fn(…)` | registered computations of **other** invocations (their callers' computes, or § 1.3's hook wrapper) | Theirs (Decision 2026-09-10). |
| queued effect functions (`runEffect.bind`, `trackedEffect`'s `run`, `drainApplyQueue`, …) | registered by other invocations | Theirs. |
| `CollectionQueue.run` (`dev.js:12553-12556`, `read(this._disabled)`, `read(this._collapsed)`) and `finalizePureQueue` → `checkBoundaryChildren` → `CollectionQueue._checkSources` → `RevealController._evaluate` (`:12474`, `read(this._disabled)`) | the boundary queues/controllers that `createLoadingBoundary`/`createErrorBoundary`/`createRevealOrder` registered as child queues | The boundary's reads of its own signals, attributed to the registering boundary export. The same structure existed in rc.3 (`rc.3 dev.js:9945`, `:9865`, `:1703`, `:1754`), under which the rc.3 summary closed `reads: []`. |
| **new in rc.6:** `_runLaneEffects` → `drainOptimistic` → `applyEntries` → `entry.fn(next, prev, force)` (`dev.js:10531-10566`, `:10693-10711`), and its error route `owner._queue.notify` → `CollectionQueue.notify` → `untrack(() => this._onFn())` | **hook** / registered: patch consumers `registerPatch`/`registerRowOps`/`registerSlotPatch` installed; `_onFn` is the boundary creator's `on` callable | Theirs. `applyEntries` itself reads only the store-internal `t.pb ?? t.v` target fields, not through `read`/a proxy. |

With the boundary queue methods and the external-source hook removed, the call-only closure contains no direct read
site in any bundle (§ 0.5: dev 187, cjs 177, prod 178).

#### 7.3 What changed from rc.3

The export body is unchanged apart from dev's loop-guard message. `GlobalQueue.flush` adds only `sweepDormant()`. The
drain now also reaches the store patch or optimistic apply pipeline. Every new read-capable path hands control to a
callable some other invocation registered, so the rc.3 disposition carries over. This rests on Decision 2026-09-10
exactly as the rc.3 row's comment says ("Its drain runs computations a third party registered").

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `reads: [] closed` for `(@solidjs/signals@2.0.0-rc.6, flush)` denies that `flush`'s own code observes a
reactive source during one invocation. The reads its drain performs belong to the computations, effects, boundary
queues and patch consumers that other invocations registered, and to the caller's `fn`.

---

### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**The claim decided.** The claim is what `creates: [] closed` denies for one invocation, including everything its
synchronous drain runs as `flush`'s own act.

The walk is § 7.2. `flush`'s own code and every archive function the drain reaches stay inside
`@solidjs/signals@2.0.0-rc.6`, which has no handle to a browser document or a server runtime (§ 0.4). That covers the
scheduler, recompute, commit, lanes, verdict companions, boundary queues, dormant sweep and patch drain. Their host
touches are:

- `queueMicrotask(flush)` through `schedule()`;
- `MicrotaskQueue.enqueue`;
- `console.error`/`console.warn` in `haltReactivity`, `notifyHalted` and the dev diagnostics.

`until`'s timer and abort listener, the only new host registrations in rc.6, are reached only from `until`'s own call.
The drain can run a settle continuation that `until` registered, but that continuation is `until`'s behavior. Anything
the drained computations, effects or patch consumers do, such as a DOM write by an `@solidjs/web` patch consumer, is
the body of a callable another invocation registered. It is not `flush`'s act (§ 1.3 "installed hook"; § creates
excludes caller-supplied callables).

**What changed from rc.3.** § 7.3. The patch drain is new, and it invokes registered consumers. It adds no `create` of
`flush`'s own.

**Per-bundle verdict.** prod: closed. dev: closed. cjs: closed.

**Sign-off.** `creates: [] closed` for `(@solidjs/signals@2.0.0-rc.6, flush)` denies that one invocation of `flush`,
through its own code, registers a version-1 resource into a browser document or a server runtime. It runs the caller's
`fn`, drains the archive's queues, and runs callables that other invocations registered, whose bodies are theirs.


---

## rc.6 re-audit, group C — `@solidjs/signals` store rows

Auditor: group C. Read-only. No repository file edited, no cargo, nothing installed.

Rows (rc.3 Summary-cited, `pkg/contracts/bundled/solid-v2/solidjs-signals.json`):
`createStore` creates; `createProjection` creates; `createOptimistic` reads + creates;
`createOptimisticStore` reads + creates; `reconcile` reads + creates.

| # | Row | Verdict |
| --- | --- | --- |
| 1 | `(createStore, Creates)` | **GRANT** |
| 2 | `(createProjection, Creates)` | **GRANT** |
| 3 | `(createOptimistic, Reads)` | **GRANT** (rests on the § 0.6 accessor line, same as rc.3) |
| 4 | `(createOptimistic, Creates)` | **GRANT** |
| 5 | `(createOptimisticStore, Reads)` | **WITHHOLD** |
| 6 | `(createOptimisticStore, Creates)` | **GRANT** |
| 7 | `(reconcile, Reads)` | **GRANT** (rests on the § reads [Decision 2026-09-10] parameter-rooted carve-out) |
| 8 | `(reconcile, Creates)` | **GRANT** |

---

### 0. Shared evidence (every section cites it)

#### 0.1 Archive and the bundles `exports` can select

rc.6 root: `<rc6>/`.
`package.json` sha256 on disk = `de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c`, which matches the brief. The
registry integrity was not re-derived, because no tarball was fetched.

`exports["."]`: `import.test` / `import.development` → `./dist/dev.js`; `import.default` →
`./dist/prod/index.js` (an ESM graph over `dist/prod/**`); `require.default` → `./dist/node.cjs`. There is no
`node`/`browser`/`worker` condition. So there are exactly **three runtime bundles**, and every row below is
read in all three.

| Bundle | File sha256 of the file holding the definitions |
| --- | --- |
| `dist/prod/store/index.js` (`createStore`, `reconcile`) | `4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8` |
| `dist/prod/store/next/projection.js` (`createProjectionNext`) | `9f8365fec3e8f7ba82b376301f2b766b4615bdeb3ad305a40d793471884940d0` |
| `dist/prod/store/next/optimistic.js` (`createOptimisticStoreNext`) | `ffc69f0689ff495b827f472b7d851de3c732d28ce68d7ada8f625e5917f0280c` |
| `dist/prod/signals.js` (`createOptimistic`) | `b75f46deb1ea7bae16d5d806d5e50582e68fd8b56d577cb0a1c4b17f1c351729` |
| `dist/dev.js` | `f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73` |
| `dist/node.cjs` | `4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f` |

Export bindings: `prod/index.js` re-exports `createStore, reconcile` from `./store/index.js`,
`createOptimisticStoreNext as createOptimisticStore` from `./store/next/optimistic.js`,
`createProjectionNext as createProjection` from `./store/next/projection.js`, and `createOptimistic`
from `./signals.js`. `dev.js`'s export list and `node.cjs:11019-11085` bind the same local names.

Byte offsets were computed by parsing each file with `acorn` (from `packages/cli/node_modules`),
converting UTF-16 indices to UTF-8 byte offsets, and slicing the top-level statement. The tool was
checked against the shipped rc.3 citation: it reproduces `runWithOwner` `37382..37594` /
`5c40363e…` exactly. Every rc.6 slice was then re-read independently in Python, checking that each
slice starts `function <name>(` and ends `}`.

**prod ↔ node.cjs equivalence.** I compared all 496 top-level declarations of `dist/prod/**` with
`node.cjs`, token by token, allowing only consistent renaming of names of three characters or fewer.
480 are alpha-equal. The other 16 differ only by bundler de-collision suffixes (`hasActiveOverride$1`,
`scheduled$1`, `runQueue$1`) or by object-literal property mangling (`computed`, `createEffectNode`;
I inspected `computed` by hand). Every definition and first-level callee named below is in the
alpha-equal set. `node.cjs` is therefore the prod reading with other mangled names, and each section
states its verdict for it explicitly.

**dev.js** is the unmangled development build. On these paths it differs from prod only by:
- dev diagnostics: `emitDiagnostic`, `console.warn`/`error`, `devGuardStoreSetterWrite`;
- `registerGraph(value, owner)` (`dev.js:1052-1059`), which pushes onto `owner._signals` and calls
  `DEV$1.hooks.onGraph?.(…)`;
- one dev-only branch in `applyAdopt` (`dev.js:9763-`): for a getter-bearing record that carries
  patches it `console.warn`s and calls `patchHooks.demoteToEffects(t)`. Prod does not demote.

#### 0.2 Archive-wide host-boundary census (rc.6, token level)

Method: the `acorn` tokenizer was run over every `dist/prod/**/*.js`, over `dist/dev.js`, and over
`dist/node.cjs`, so comments are excluded by the lexer rather than by line prefix. It looked for free
identifiers `document window navigator globalThis self addEventListener queueMicrotask setTimeout
setInterval setImmediate requestAnimationFrame MessageChannel postMessage process performance
localStorage fetch XMLHttpRequest WebSocket Worker Promise Date console sharedConfig
FinalizationRegistry WeakRef require importScripts eval Function structuredClone crypto location
history`. A separate property-form grep covered `.addEventListener` / `.removeEventListener`
/ `.appendChild` / `.insertBefore` / `.setAttribute` / `.createElement` / `.serialize` / `.write(`.

The line-prefix grep method used in rc.3 § 1.5 is not safe on these files. `node.cjs:291` and
`prod/core/invariants.js:28` put code after `*/` on the same line, and only the tokenizer found
them.

| Site | Code | Reached by a group-C export? |
| --- | --- | --- |
| `prod/core/scheduler.js:276`, `dev.js:1608`, `node.cjs:703` | `queueMicrotask(flush)` in `schedule()` | yes, through any write/fold (`queueFold` → `schedule()`); one-shot microtask draining the archive's own queues; registers no version-1 resource |
| `prod/signals.js:287`, `dev.js:6797`, `node.cjs:5211` | `queueMicrotask(() => t(e))` (`MicrotaskQueue`) | no (`onSettled` machinery) |
| `prod/signals.js:312/394/414-415/518`, `prod/core/action.js:77`; dev `6417/6827/6940-6941/7055`; cjs `4881/5236/5318/5338-5339/5442` | `new Promise` / `Promise.resolve` / microtask in `action`, `resolve`, `refresh`, `until` | no: no internal caller of `until/resolve/refresh/action/onSettled` exists anywhere in `store/**` or `core/**` |
| `prod/signals.js:560/563`, `dev.js:7101/7104`, `node.cjs:5484/5487` | `setTimeout(…)`, `signal.addEventListener("abort", …)` on the caller's `AbortSignal` | no (`until` only) |
| `console.*` sites (`prod/core/effect.js:72`, `scheduler.js:290,297`; many in dev) | diagnostics | dev diagnostics only; no resource |
| `dev.js:1015` | `queueMicrotask(() => console.warn(footer))` in `emitDiagnostic` | dev only, console only |
| `dev.js:241`, `:441` | `performance.now()` / `Date.now()` | module-init clock / dev rerun tracing; not an operation of any export |
| `prod/core/invariants.js:28`, `dev.js:1185`, `node.cjs:291` | `typeof globalThis !== "undefined" && !!globalThis.process?.env?.COMPANION_CENSUS;` | module-initialization read, value discarded; outside every domain (§ "one invocation") |

Also checked: there are no references to `document` or `sharedConfig`, no DOM method, and no
serializer. There is no import of any other package: every `prod` import is relative, and `node.cjs`
has no `require(`. The `window`/`history`/`self` tokens in `dev.js` are local bindings (`let window
= hotCauses.get(…)`, `let history = []`, `const self = {…}`).

**Consequence**, the rc.3 § 1.5 argument re-established on rc.6's bytes: no call that stays inside
`@solidjs/signals@2.0.0-rc.6` can register a version-1 resource into a browser document or a server
runtime, because the archive has no handle to either. The only reaches outside the archive are:
- `queueMicrotask(flush)`;
- protocol methods on the caller's own values: `.then`, `[Symbol.asyncIterator]`, `.next`, and the
  iterator teardown (`prod/core/async.js:209,403,412,434,438,509,532`), dispositioned **caller** as in
  rc.3;
- installed hooks (§ 0.3).

#### 0.3 Installed hooks and third-party callables on these paths

These are dispositioned **hook** (rc.3 method § 1.3): their invocation is recorded, and their body is
not the export's behavior.
- `DEV$1.hooks.onGraph` and `onOwner`, diagnostic listeners, and `consoleFooter`. Dev only.
- `GlobalQueue._wireExternalSource` (prod `.Rt` family), called from `setupComputedNode` when
  `enableExternalSource` installed a config (`dev.js:4468`). Its body (`prod/core/external.js:44-56`)
  creates its own signal and `read`s it on each recompute; the signal is the hook's, not the
  export's.
- `attrHooks.write` inside `setSignal` (`dev.js:4952`).
- `GlobalQueue._notifyAuthoritativeObservers` in `recompute`. It is new in rc.6 and is installed by
  `until()`.
- Verdict companions (`GlobalQueue._syncCompanions` and relatives, `prod/core/verdict.js:532-552`),
  which exist only after a third party called `latest`/`isPending`.
- **Patch channel consumers.** These are the `fn` bodies registered through `registerPatch`,
  `registerRowOps`, and `registerSlotPatch` (§ 0.4). `applyEntries` (`prod/store/next/patch.js:83-118`)
  calls `i.fn(t, n, l)`, and `demoteToEffects` (`:462-480`) re-drives them under
  `runWithOwner(l.owner, () => createRenderEffect(…))`.
- Engine-internal slots installed on first call by `installOptimisticEngine`
  (`prod/core/optimistic.js:274-`) and `installNextBlockedHalf` (`prod/store/next/optimistic.js:65-`),
  and at module load by `verdict.js`. Installing them writes the package's own module state and
  `GlobalQueue` statics. That is a "package's own private module variable" and not a `create`
  (§ creates, [Decision 2026-09-03]).

#### 0.4 The new rc.6 patch modules (`store/next/patch.js`, `store/next/patch-hooks.js`)

`patch-hooks.js` is 220 bytes: two `let` slots (`patchHooks`, `rowHooks`, initially `null`) and two
installers. `patch.js` is **never imported** by `store.js`, `reconcile.js`, `optimistic.js`,
`projection.js`, or `store/index.js`; those import only the slots. Its own header comment says "core
never imports this module". The slots are filled only by `armPatchHooks`/`armRowHooks`
(`patch.js:595-612`), which run only inside `registerPatch` (`:358-398`), `registerRowOps`
(`:483-514`), and `registerSlotPatchNext` (`:536-566`). Those are exported for the web runtime's
compiled output and list driver.

Every emission site is gated on the slot being non-null **and** on a consumer list on the target's
patch-channel extension. Only those three registrations populate a consumer list (`o.p ??= []` at
`:376`, `o.ro ??= []` at `:501`, `l.sp ??= []` at `:556`). The gated sites:
- `prod/store/next/reconcile.js:116,235,240,245,273,401,418`;
- `prod/store/next/store.js:625,654,658,799,1563`;
- `prod/store/next/optimistic.js:91,96,460,469`.

The draft write traps do allocate `pc` (for `wk`), but they leave `p`/`ro`/`sp` null. In addition:
- `store.js:799` is gated `e.fam === null && … patchHooks.hasPatches()`;
- `applyAdopt`'s emission needs `s = (e.fam === null)` (`reconcile.js:116`);
- `drainFolds`' emissions (`store.js:625-658`) run at the flush that follows, not on the call's stack.

**Answer for group C.** At the call event, **no function in `patch.js` or `patch-hooks.js` is invoked**
by any of the five exports, in any bundle. The most any of them does is read the null-valued slot
binding: `createStore(fn)`, `createProjection`, and `createOptimisticStore(fn)` do so through the
initial derive's commit → `reconcileNextState` → `applyAdopt`. Every target this call creates starts
with `pc = null` (`createTarget`, `store.js:104`) and has no registrant.

At later events (setter writes, the returned `reconcile` function's application, recomputes,
flush-time fold commits, optimistic reverts through `_clearOptimisticStores`), `emitPatch*`,
`emitRowOps*`, and `emitSlotPatch` can be reached. They are reached only when a third party has
registered a consumer, they only enqueue entries on `globalQueue` (`EFFECT_RENDER`), and what they
finally run is the registrant's `fn`. That is the hook disposition. In dev only, `demoteToEffects`
also creates render effects. Those are reactive computations coming into existence, so they are not
a `create` (§ creates, the "not a creates item" homes).

#### 0.5 The declaration/runtime split (rc.3 § 1.4), re-checked for this install

The ecosystem tree installs `solid-js@2.0.0-rc.3` beside `@solidjs/signals@2.0.0-rc.6`. solid-js
depends on `^2.0.0-rc.3`, which resolves rc.6 here.
- `solid-js/types/client/hydration.d.ts` **declares** its own `createStore`, `createProjection`,
  `createOptimistic`, and `createOptimisticStore` (`:312`, `:357`, `:419`, …). A `solid-js` import of
  those four therefore resolves into the `solid-js` archive, and these `@solidjs/signals` rows do not
  apply to it.
- `solid-js/types/index.d.ts:1` **re-exports `reconcile` from `@solidjs/signals`**. So a `solid-js`
  `reconcile` import binds the rc.6 declaration. Under the `node` condition it runs
  `solid-js/dist/server.js:1143-1157` / `server.cjs:1144-1158`, whose own body is
  `state => { … Object.keys(value); Object.keys(state); setProperty(state, key, value[key]) … }`.
  `setProperty` is `server.js:890-895`, and `isWrappable` is the one signals call.

  That body registers nothing into a runtime outside the invocation. Every property access it makes
  has the caller-supplied `state`/`value` as its receiver. So it reaches the same verdicts as § 7 and
  § 8 on that path. This is the § 1.4 approximation: it holds only because those server bytes were
  read too.

#### 0.6 What a `reads: [] closed` row denies, and the line applied here

Per `semantic-model.md` § reads, the row denies that **one invocation** gives rise to an observation of
a reactive source's current value. That includes:
- an untracked read;
- a read "this call schedules to a later `at` event", such as its own computation's recomputes and
  commits.

It excludes:
- reads a caller-supplied callable performs;
- property accesses whose receiver the caller supplied ([Decision 2026-09-10], "the proxy must be
  one the export owns");
- reads by callables this export did not author ([Decision 2026-09-10], authorship not timing).

The distinction each section states: **at the call event** is what runs on the call's own stack.
**Scheduled later** is what the invocation's own computation or its own closures do at later events.
Neither covers a *later, separate invocation* of a value the call returned, such as calling the
returned accessor or reading through the returned store proxy.

For accessors the rc.3 document is consistent on that last point. `createMemo` and `createOptimistic`
close `reads: []` while their shape carries `{kind: reactive, role: accessor, capabilities:
[readable]}`, so the read is expressed by the shape, not by `call.reads`. For returned **store
proxies** the rc.3 document is **not** consistent: `createStore` publishes `deep-read`/`shallow-read`
and `createProjection` publishes `read-store`, both `at: external-event`, while `createOptimisticStore`
closes `reads: []` over the same trap machinery. No verdict below rests on the store-proxy reading
(see § 5).

At the implementation level I treat a read as either:
- a call into the engine's read path (`read`/`readNodeFast`; in prod these are confined to the store
  traps, `serveDataKey`, `firewallGate`, `deepNext`, `external.js`, `verdict.js`, `boundaries.js`, and
  `signals.js:434` in `refresh`); or
- a string-key / `ownKeys` / `has` trap on a store or projection proxy the export created,

**executed by code the export authored**. Brand lookups (`[$TARGET]`, `[$PROXY]`, `[$REFRESH]`) return
before any observation (`prod/store/next/store.js:1286-1289`) and are not reads. Engine-internal field
access (`_value`, `_pendingValue`, raw backings `pb ?? v`) is not the read path. The same holds for
the `setSignal` updater, which hands the caller's function the current value. Both match the rc.3
closures of `createMemo`, `createOptimistic`, and `reconcile` over identical code.

---

### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**rc.3 basis:** summary `summary-7080e21f…` (dev.js browser-development case), `creates: []` closed.

**Definition.**
- prod `dist/prod/store/index.js:21-24`, bytes `525..676`.
- dev `dist/dev.js:11980-11983`, bytes `513397..513583`.
- cjs `dist/node.cjs:10107-10110`, bytes `437700..437851`.

```js
function createStore(e, t, r) {
    if (typeof e === "function") return createStoreDerivedNext(e, t, r);
    return createStoreNext(e, !!t?.shallow);
}
```

**Walk at the call event (prod; cjs is alpha-equal; dev noted).**

| Call | Where | Reach | Disposition |
| --- | --- | --- | --- |
| `createStoreNext(e, shallow)` | `store/next/store.js:1640-1648` (dev `9485-9500`, cjs `7696-`) | cond: `e` not a function | local, read |
| ↳ `wrapNext(e)` → `createTarget` | `store.js:127-137`, `:88-125` | always | local. It allocates a target and `new Proxy(i, traps)` (builtin: invokes nothing), sets a module/family `WeakMap`, and reads `e[$TARGET]` on the caller's value (a trap only if that value is itself someone's proxy: caller-rooted) |
| ↳ `n[$TARGET].s = true`, `markRawIngest(e)` | `store.js:1643-1644`, `store/store.js:74-80` | cond: `shallow` | brand lookup; walk over the caller's value (caller) |
| ↳ setter closure `e => storeSetterNext(n, e)` | `:1646` | always (creation only) | invokes nothing at the call |
| dev only: shallow checks `storeNextLookup.get`, `throw`; `registerGraph(proxy, getOwner())` → `DEV$1.hooks.onGraph?.()` | `dev.js:9486-9500`, `:1052-1059` | cond / always | local; **hook** |
| `createStoreDerivedNext(e, t, r)` | `store/next/projection.js:201-208` (dev `10423-`, cjs `8573-`) | cond: `e` is a function | local. It is `createProjectionNextInternal` (§ 2's walk) plus a setter closure calling `suppressComputedRecompute` + `storeSetterNext` later |

Everything past `computed(…)` is the reactive engine plus the caller's derive. That includes:
- the caller's `fn` (**caller**);
- `handleAsync`'s `.then`/async-iterator protocol on the caller's result (**caller**);
- `reconcileNextState` adoption writes, `setSignal`, and `queueFold` → `schedule()` →
  `queueMicrotask(flush)`.

That remainder is bounded by § 0.2: no host reach except the flush microtask. No patch-module function
runs (§ 0.4).

**Scheduled later by this call.** The setter (`storeSetterNext`, `store.js:1599-1639`) runs the
caller's `fn(draft)`, `notifyWrites`, and `adoptPB`/`optHooks.notifyOptimisticWrites` for a returned
replacement. The projection computed recomputes, and flush-time `drainFolds` runs. Patch emission
happens only through installed hooks (§ 0.4). All of it stays inside the archive, and § 0.2 applies.

**dev / node.cjs.** dev adds only the diagnostics and the `onGraph` hook. cjs is the prod reading.
Verdict in all three bundles: no `create`.

**rc.3 → rc.6.** The `createStore`, `createStoreNext`, `storeSetterNext`, `createStoreDerivedNext`,
`createProjectionNextInternal`, and `wrapNext` bodies are **byte-identical** in dev between rc.3 and
rc.6. Changes downstream:
- `createTarget` gains `pc`/`hv`/`ht` fields;
- `adoptPB` gains held views;
- `applyAdopt` gains patch/row-hook emissions and keyed prev-maps;
- the patch channel is new.

None of these adds a host reach. The new emissions are hook dispositions. The rc.3 conclusion is
unchanged.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, createStore, Creates)` denies that one invocation of
`createStore`, in any of its overloads and in any of `dist/prod/**`, `dist/dev.js`, or
`dist/node.cjs`, registers a version-1 resource into a browser document or a server runtime, including
through writes or recomputes it schedules. It does not deny that the call allocates store targets,
proxies, and a projection computation, or that later patch consumers registered by third parties run
on its writes.

---

### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

**rc.3 basis:** summary `summary-dc0413a1…`, `creates: []` closed.

**Definition.** `createProjectionNext`, exported as `createProjection`.
- prod `dist/prod/store/next/projection.js:195-197`, bytes `7048..7146`.
- dev `dist/dev.js:10417-10419`, bytes `450389..450505`.
- cjs `dist/node.cjs:8567-8569`, bytes `374809..374907`.

`return createProjectionNextInternal(e, t, r).store;`

**Walk at the call event.**

| Call | Where | Reach | Disposition |
| --- | --- | --- | --- |
| `createProjectionNextInternal(e, t, r)` | `projection.js:166-193` (dev `10393-`, cjs `8538-`) | always | local |
| ↳ `wrapNext(t, null, null, fam)` | `store.js:127` | always | local (§ 1) |
| ↳ `o[$TARGET].s = true; markRawIngest(t)` | `:176-177` | cond `shallow` | brand; caller value walk |
| ↳ `computed(() => {…}, n)` | `core/core.js:408` → `setupComputedNode` `:562` | always | local engine. It links under `context`, calls the `GlobalQueue._wireExternalSource` **hook** if installed, and runs `recompute(self, true)` unless `lazy` |
| ↳↳ computed body: `getOwner()`; `runProjectionComputedNext(o, e, key)` | `projection.js:183-186`, `:210-236` (dev `10434-`, cjs `8582-`) | always (unless lazy) | local |
| ↳↳↳ `JSON.parse(JSON.stringify(e[$TARGET][STORE_VALUE]))` | `:218` | cond: open loading window | builtin over the backing (caller data; any `toJSON` there is caller-supplied) |
| ↳↳↳ `wrapDraft(e, …, o)` | `:58-164` | always | local; `new Proxy` |
| ↳↳↳ `storeSetterNext(l, o => { s = t(u ?? o); … })` | `:220-234` | always | local. `t(…)` is the **caller**'s derive |
| ↳↳↳ `handleAsync(n, s, commit)` | `core/async.js:204-` | always | local. `.then` / `[Symbol.asyncIterator]` on the caller's result is **caller** |
| ↳↳↳ `commit` → `storeSetterNext(e, e => reconcileNextState(t, e, r, true), false)` | `:223-231` | cond: sync result | local writes (§ 7's adoption path); `queueFold` → `queueMicrotask(flush)` |
| ↳ `c.T &= ~CONFIG_AUTO_DISPOSE; i.node = c` | `:187-188` | always | field writes |

**Scheduled later.**
- Recomputes of the projection computed.
- `handleAsync` landings (`commit` from a continuation), with the same writes.
- Flush-time fold commits.
- Patch emission, only via installed hooks (§ 0.4).

All of it is bounded by § 0.2.

**dev / node.cjs.** Same shape. dev differs by diagnostics and hooks, and by the dev-only `applyAdopt`
demotion (§ 0.1), which is effect creation and not a `create`. cjs is alpha-equal to prod. Verdict in
all three: no `create`.

**rc.3 → rc.6.** The `createProjectionNext` and `createProjectionNextInternal` bodies are unchanged.
`runProjectionComputedNext` renames `onDraftWrite` → `aroundDraftWrite` and passes the value to
`wrapCommit`. `wrapDraft` routes writes through `aroundWrite`. `reconcileNextState` uses `sameKey`.
These are immaterial to `creates`.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, createProjection, Creates)` denies that one invocation of
`createProjection`, in each of the three bundles, registers a version-1 resource into a browser
document or a server runtime, including through the recomputes and async landings of the projection
computation it creates. The computation's coming into existence is not such a registration and is not
denied.

---

### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**rc.3 basis:** summary `summary-92071bb7…`, `reads: []` closed.

**Definition.**
- prod `dist/prod/signals.js:571-583`, bytes `24983..25530`.
- dev `dist/dev.js:7109-7122`, bytes `302445..303053`.
- cjs `dist/node.cjs:5495-5507`, bytes `232451..232998`.

```js
function createOptimistic(e, t) {
    installOptimisticEngine();
    if (typeof e === "function") {
        const n = optimisticComputed(e, t);
        n.T &= ~CONFIG_AUTO_DISPOSE;
        return [ accessor(n), setSignal.bind(null, n) ];
    }
    const n = optimisticSignal(e, t);
    return [ accessor(n), setSignal.bind(null, n) ];
}
```

dev additionally calls `registerGraph(node, getOwner())` in the value overload (hook).

**At the call event.**

| Call | Where | Disposition | Read? |
| --- | --- | --- | --- |
| `installOptimisticEngine()` | `core/optimistic.js:274-` (dev `5545-5560`) | local; writes `GlobalQueue` slots | no |
| `optimisticComputed(e, t)` → `computed` → `setupComputedNode` → `recompute(self, true)` unless `lazy` | `core/core.js:627`, `:408`, `:562` | local engine. The compute it runs is the **caller**'s `e`, with the node as observer. `_wireExternalSource` is a **hook** | the caller's reads are excluded. Engine code on this path makes no `read()` call: in prod `core.js` the only `read(` site outside `read` is its own recursion at `:829`, and `core/optimistic.js`, `async.js`, `lanes.js`, `heap.js`, `graph.js`, and `scheduler.js` have none |
| `optimisticSignal(e, t)` → `signal` | `core.js:620` | local; allocates a node | no |
| `accessor(n)` | `signals.js:61` (dev `6552-6556`) | `read.bind(null, node)` plus a `[$REFRESH]` field: creates a bound function and invokes nothing | no read at the call |
| `setSignal.bind(null, n)` | builtin `bind` | invokes nothing | no |

**Scheduled later by this call.** The computed's recomputes run the caller's `e` again, and async
landings go through `handleAsync` (writes). Optimistic reverts at transition settle (writes) and the
`setSignal` → `GlobalQueue._optimisticWrite` = `optimisticWrite` write path are covered by the
existing `optimistic-write` item. The updater form `v(currentValue)` hands the caller's function the
node's field value (`dev.js:4944-4945`); that is not the read path (§ 0.6).

**Not denied, per § 0.6.** A later, separate invocation of the returned accessor calls `read(node)`.
The rc.3 document states that through the shape's `readable` capability, and this row does not deny
it.

**dev / node.cjs.** dev adds `registerGraph` (hook). cjs is alpha-equal to prod. The verdict is the
same in all three.

**rc.3 → rc.6.** The `createOptimistic` body is byte-identical in dev. So are `accessor`,
`optimisticSignal`, `optimisticComputed`, `installOptimisticEngine`, `optimisticWrite`, and
`setupComputedNode`. `setSignal` changes one throw message. `signal` and `computed` change
`!= null ? :` to `??`. `recompute` adds three pieces of bookkeeping:
- `releaseFlightTeardown`, which invokes a caller iterator's teardown;
- `settlePendingSource`, a dependent-status walk with no `read`;
- `_notifyAuthoritativeObservers`, a hook installed by `until()`.

No read is added.

**Caveat, stated rather than hidden.** This GRANT rests on the § 0.6 line that a later call of the
returned accessor is not this invocation's scheduled read. That is exactly the line the rc.3 closure
drew for `createMemo` and `createOptimistic`. If that line is ever redrawn, this row moves with
`createMemo`'s.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, createOptimistic, Reads)` denies that one invocation of
`createOptimistic` performs or schedules any read of a reactive source it authored, in each of the
three bundles. It does not deny the reads the caller's compute performs inside the node the call
creates, or the read a later invocation of the returned accessor performs.

---

### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

The definition and citations are as in § 3, and so is the walk. Its creates-relevant facts:
- It allocates a signal or computed node, links it under the owner, installs engine slots (module
  state), and returns bound functions.
- The value overload in dev pushes onto `owner._signals` and calls `DEV.hooks.onGraph` (hook).
- The only host reach on any path, now or scheduled, is `queueMicrotask(flush)` through `schedule()`
  (§ 0.2).
- Transitions the optimistic write path touches (`globalQueue.initTransition`) are the archive's own
  scheduler objects. The rc.3 `action` summary likewise declares a `transition` resource beside
  `creates: []`.

The verdict is the same in all three bundles. **rc.3 → rc.6:** as § 3; nothing affects `creates`.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, createOptimistic, Creates)` denies that one invocation of
`createOptimistic`, in each of the three bundles, registers a version-1 resource into a browser
document or a server runtime. The node and owner-tree link it creates are not such a registration.

---

### 5. `createOptimisticStore` — `reads` — archive `@solidjs/signals@2.0.0-rc.6` — **WITHHOLD**

**rc.3 basis:** summary `summary-034586f3…`, `reads: []` closed. The same summary also closes
`callbacks: []`, although the function overload invokes the caller's derive. It was read for the
value overload only; see the final notes.

**Definition.** `createOptimisticStoreNext`.
- prod `dist/prod/store/next/optimistic.js:152-275`, bytes `7627..13764`.
- dev `dist/dev.js:11150-11289`, bytes `480810..487127`.
- cjs `dist/node.cjs:9324-9447`, bytes `405742..411875`.

**At the call event (nothing found).**
- `installOptimisticEngine()` and `installNextBlockedHalf()` (slot installs).
- `wrapNext(initialValue, null, null, fam)`.
- `fam.key` closure creation.
- `store[$TARGET].s` (brand) and `markRawIngest` if `shallow`.
- For a function first argument, `computed(…)`. Its first run does:
  - `runAuthoritative(() => runProjectionComputedNext(store, fn, key, wrapCommit, aroundDraftWrite))`,
    which is the § 2 path. The caller's derive reads the draft, and that is the caller's read;
  - `wrapCommit` → `retainingTransition(fam)`, which is `null` because `fam.rt` is undefined at
    creation;
  - `runAuthoritative(write)` → `reconcileNextState` on raw backings;
  - `declareFlight(self)`, which reads engine fields only.

At the call event I found no export-authored read of the store it creates.

**Scheduled later by this call: a read.** rc.6 adds a landing router in the export's own body:

```js
// prod/store/next/optimistic.js:202-207 (inside createOptimisticStoreNext; dev 11205-11210, cjs 9374-9379)
const wrapCommit = (e, t) => {
    const n = retainingTransition(r);
    if (n !== null) return void stageLanding(r, n, t);
    ...
```

```js
// prod/store/next/optimistic.js:305-309, bytes 15230..15411 (dev 11320-11331, bytes 488636..488903; cjs 9477-, bytes 413338..413519)
function stageLanding(e, t, n) {
    runFolded(t, () => runAuthoritative(() => storeSetterNext(e.px, t => {
        stagedApply(t, unwrapValue(n), e.key ?? null);
    }, false)));
}
```

`storeSetterNext(proxy, fn)` calls `fn(proxy)` (`store.js:1599-`), so `stagedApply`'s `cur` **is
`fam.px`, the store proxy this invocation created** (`fam.px = store`, prod `:167`). `stagedApply` is
at prod `:338-`, bytes `17028..21326`; dev `11362-`, bytes `490579..494607`; cjs `9510-`, bytes
`415134..419431`. It performs string-key `get` traps (`cur[j]`, `cur[i]`, `cur[k]`), `ownKeys`
(`Reflect.ownKeys(cur)`), and `cur.length` on that proxy. For example, from dev `11372` and `11442`:

```js
const raw = unwrapValue(cur[j]);
...
const pv = unwrapValue(cur[k]);
```

The reach condition:
- `fam.rt` is non-empty. The returned setter adds the ambient transaction:
  `if (txn !== null) (fam.rt ??= new Set()).add(txn)` at dev `11286`, prod `:273`, cjs `9445`.
- **and** a later commit of the derived computed runs `wrapCommit`. This is a recompute or an async
  landing of the computation this invocation created.

This is code the export authored, run by its own computation at a later `at` event. It observes the
current (staged) value of a reactive source the export created. Inside the setter bracket the trap is
`inDraft`, so no dependency links (untracked). § reads: "an `untracked` … read is still a read", and
the proxy is "one the export owns". It is also a guarded reach, and a flat `(package, export, domain)`
row has nowhere to put the guard. That is the § creates 2026-09-04 argument, which applies to this
table's shape for every domain.

Nothing in the audit record decides that an export's own draft access inside its own setter bracket
is *not* a `read`. The rc.3 corpus's only precedent points the other way: `createStore` publishes
`read-store` (untracked).

**Not the basis, recorded.** A separate weakness exists in the rc.3 closure, on both archives. The
returned store proxy uses the same `traps` as `createStore`'s, and rc.3 publishes those reads for
`createStore` (`deep-read`, `shallow-read`) and `createProjection` (`read-store`) but not here (§ 0.6).

**Bundles.** All three carry `stageLanding`/`stagedApply` with the same structure. prod and cjs are
alpha-equal. The withholding holds for each of prod, dev, and cjs.

**rc.3 → rc.6.** This matters. rc.3's `createOptimisticStoreNext` (`rc.3 dev.js`, bytes
`362722..364437`) had `wrapCommit = write => { runAuthoritative(write); consume(); }`. It had no
retention ledger, no `stageLanding`, and no `stagedApply`: `rg` finds none of the three names anywhere
in rc.3's `dist/`. The rc.3 derived commit only did brand lookups plus raw-level adoption on its own
proxy. The rc.6 landing path is new behavior.

**Sign-off (withholding).** On rc.6, `createOptimisticStore(fn, …)`'s own landing router observes the
store it created through that store's proxy. It does so at a later event its own computation causes,
under a retaining-transaction guard. So the denial that one invocation reads no reactive source it
authored cannot be established, and the row is not carried for rc.6.

---

### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

The definition and citations are as in § 5, and so is the walk. Its creates-relevant facts:
- It allocates targets, proxies, a family record, and (for the derived overload) a computed.
- It installs engine slots and `GlobalQueue._clearOptimisticStores` / `_transitionBlocked` (module
  state, § 0.3).
- `createTransition()` / `globalQueue.initTransition(…)` in `enterFlightTransition`/`declareFlight`
  (prod `:187-200`, `:236-250`; dev `11194`, `11247`) create the archive's own scheduler transitions (compare the rc.3
  `action` summary: a `transition` resource beside `creates: []`).
- `runFolded` → `runAsTransitionBatch` and the `stageLanding` writes are internal.
- Patch emissions (`optimistic.js:91,96,460,469`) happen only via installed hooks (§ 0.4).
- The only host reach is `queueMicrotask(flush)` (§ 0.2).

The verdict is the same in all three bundles. **rc.3 → rc.6:** the new landing/flight machinery is
transitions and writes inside the archive. None of it registers anything outside, so there is no
effect on `creates`.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, createOptimisticStore, Creates)` denies that one
invocation of `createOptimisticStore`, in either overload and in each of the three bundles, registers
a version-1 resource into a browser document or a server runtime, including through its later
landings. The store, the computation, and the scheduler transitions it creates are not such a
registration.

---

### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

**rc.3 basis:** summary `summary-97b25908…`, `reads: []` closed. Its only operation is
`reconcile-write` `at: {event: external-event, schedule: external}`, which is the application of the
returned function.

**Definition.**
- prod `dist/prod/store/index.js:26-28`, bytes `678..758`.
- dev `dist/dev.js:11984-11986`, bytes `513584..513682`.
- cjs `dist/node.cjs:10112-10114`, bytes `437853..437933`.

`function reconcile(e, t = "id") { return r => reconcileNextState(e, r, t); }`

**At the call event.** A default-parameter evaluation and closure creation. No call, no property
access, and so no read.

**Scheduled later: the returned function's application.** The store setter (`storeSetterNext` → `fn(proxy)`)
invokes the closure with the store's own proxy as `r`. That runs `reconcileNextState`
(`prod/store/next/reconcile.js:38-91`, dev `9710-`, cjs `7913-`):
- `n?.[$TARGET]` is a brand lookup that returns before any observation (`store.js:1286-1289`).
- `materializePB`, `unwrapValue`, and the key function `e => e?.[t]` (or the caller's key function)
  apply to raw backings `l.pb ?? l.v` and to the caller's `value`.
- `optHooks.applyTentative` / `optimisticView` handle optimistic families.
- `applyAdopt`/`descend` (`:93-380`, `:475-`) work on raw arrays and objects through the lookup maps,
  and on `notifyKeyValue`/`notifyKeyDiff`/`notifyFoldTail`/`bumpDeep` (`store.js:835-929`), which are
  `setSignal` writes, including the archive's own `e => e + 1` updater.
- Row and patch emission go through hooks (§ 0.4).

There is no `read(`/`readNodeFast(` call in `reconcile.js` or in any `store.js` helper it calls; the
read path is confined to the traps (§ 0.6). The proxies it can touch are:
- `r`, the receiver supplied by the returned function's caller, carrying the caller's store;
- proxies nested inside the caller's `value`, reached by a key function on a row.

All of them are parameter-rooted, so they are the caller's reads per [Decision 2026-09-10]. `reconcile`
creates no reactive source and imports none it reads.

**dev / node.cjs.** dev adds a `console.warn` and the `demoteToEffects` hook in `applyAdopt`
(§ 0.1). cjs is alpha-equal. The same verdict holds in all three. On the `solid-js` `node` path
(§ 0.5), the `server.js` body's reads (`Object.keys(state)`, `state[property]`, `value[key]`) are all
on caller-supplied receivers.

**rc.3 → rc.6.** The `reconcile` body is byte-identical. `reconcileNextState` changes only `sameKey`.
`applyAdopt` adds patch/row-hook emission and keyed prev-maps over raw rows. None of these reads an
owned source.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, reconcile, Reads)` denies that one invocation of
`reconcile` reads any reactive source it created or imported, in each of the three bundles. That
covers the call event and the later application of the function it returns. It does not deny reads
of the store handed to that function or of the caller's `value`; § reads assigns those to the caller.

---

### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

The definition and citations are as in § 7. The call creates a closure. Its application performs
adoption writes, `setSignal` notifications, and `queueFold` → `schedule()` → `queueMicrotask(flush)`.
When a third party has registered consumers, it enqueues patch, row, or slot entries whose bodies are
theirs (§ 0.4). Bounded by § 0.2, there is no registration of a version-1 resource into any runtime
outside the invocation. The verdict is the same in all three bundles and on the `solid-js`
`server.js` path (§ 0.5), which only calls `setProperty` on the caller's `state`.

**rc.3 → rc.6.** The new emission paths are hook dispositions and do not affect `creates`.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.6, reconcile, Creates)` denies that one invocation of
`reconcile`, or the later application of the function it returns, registers a version-1 resource into
a browser document or a server runtime, in each of the three bundles. It does not deny that third-party
patch consumers run on the writes it makes.

---

### Notes that bear on rc.3 conclusions (outside these eight verdicts)

1. **rc.3 `createOptimistic` and `createOptimisticStore` summaries close `callbacks: []`.** Both
   archives' function overloads invoke the caller's function at the call event:
   `computed` → `setupComputedNode` → `recompute(self, true)` unless `lazy`. That code is unchanged
   between rc.3 and rc.6 (`setupComputedNode` is byte-identical). Those two rc.3 closures are
   contradicted by rc.3's own bytes for the derived overload. There is no `callbacks` row in this
   table today, but the summaries are not safe authority for that domain.
2. **The store-proxy reads convention is inconsistent within the rc.3 document** (§ 0.6, § 5).
   `createStore` and `createProjection` publish reads through the returned store at `external-event`;
   `createOptimisticStore` does not. This was already true on rc.3.
3. **The rc.3 § 1.5 line-prefix grep would miss code after `*/`.** On rc.6 that pattern hides
   `node.cjs:291` / `prod/core/invariants.js:28`, which are harmless module-init reads. A token-level
   census is needed on these bundles.
4. **The § 1.4 split still exists for `reconcile`** (§ 0.5). The ecosystem pairs `solid-js@2.0.0-rc.3`
   with signals rc.6, so a `solid-js` `reconcile` import binds the rc.6 declaration and runs
   `solid-js/dist/server.js`'s own body on `node`. It was read and reaches the same verdicts.


---

## rc.6 re-audit, group D — `action` (`reads`, `creates`), `snapshot` (`creates`)

Read-only audit. Nothing in the repository was edited, nothing was built or installed.

### 0. Inputs, identity, method

**Archive.** `<rc6>/`.
`package.json` sha256 = `de11cde1dd28b678f380c865be674a1f1a18a198e399ad2f997fd83aef1c163c`, which matches the brief;
`"version": "2.0.0-rc.6"`. The sha512 integrity was **not recomputed**: there is no tarball on disk. The sibling
`bun.lock:17` records exactly `sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==`.
There is no `phase0/rc6/files.json`, so no per-file pin exists yet. The digests given below are of the bytes on disk.

**Runtime bundles a consumer can select** (`exports["."]`): `import`+`test`/`development` → `dist/dev.js`;
`import` default → `dist/prod/index.js`, which re-exports `action` from `./core/action.js` (`index.js:21`) and `snapshot` from
`./store/index.js` (`:29`); `require` → `dist/node.cjs`. `main`/`module` name the same two files. No other condition exists.
Every verdict below is given for all three.

**Declarations.** `dist/types/core/action.d.ts:64` and `dist/types/store/index.d.ts:17`, both reached from
`dist/types/index.d.ts`. `solid-js@2.0.0-rc.3`, the solid-js that sits beside rc.6 in this ecosystem (`bun.lock:27`),
re-exports both names from `"@solidjs/signals"` (`types/index.d.ts:1`), so a solid-js import also binds this archive's row.

**Method.** I followed the 2026-09-04 audit § 1.3 exactly. I opened each export's definition in each bundle and followed
every call to one of the dispositions `local`, `builtin`, `caller`, `hook`, `host`, and `drain`. `drain` is the
[Decision 2026-09-10] class: a computation, effect, boundary queue, cleanup or consumer that a third party registered and
that `flush` runs. I used scratchpad aids to enumerate reach: `cg.py` (a static call graph that over-approximates),
`calls.py`, `unresolved.py` (every call through a parameter, field or hook) and `hostcheck.py`. The reach sets are
`*-action-reach.txt` and `*-snapshot-reach.txt`. These aids list candidates. They prove nothing: every frame they list was
read, and every indirect call they list is given a disposition below.

**Archive-wide host census, rc.6.** I grepped every runtime file, excluding comment lines, for `document`, `window`,
`navigator`, `globalThis`, `addEventListener`, `queueMicrotask`, `setTimeout`, `setInterval`, `requestAnimationFrame`,
`MessageChannel`, `process`, `performance`, `fetch`, `Promise`, `Date`, `console`, `crypto`, `postMessage`,
`serializ*`, `sharedConfig` and `hydrat*`. The code references are:

- `queueMicrotask`: `schedule()` (prod `scheduler.js:276`, cjs `:703`, dev `:1608`). Also `MicrotaskQueue.enqueue`
  (prod `signals.js:287`), `refresh` (`signals.js:415`) and dev `emitDiagnostic` (`dev.js:1015`, which prints a console
  footer).
- `new Promise` / `Promise.resolve`: in `action`, `resolve`, `refresh` and `until`.
- **New in rc.6:** `until` calls `setTimeout` and `signal.addEventListener("abort")` on the caller's `AbortSignal`
  (prod `signals.js:560,563`, cjs `:5484,5487`, dev `:7101,7104`).
- `console.*`.
- Module-init clock and `globalThis.process?.env` reads (dev `:241`, `:1185`).

There is **no** reference to a document, a window, a server runtime, a render context, or a serializer anywhere in rc.6's
runtime bytes. That is unchanged from rc.3 § 1.5, except for `until`'s timer and abort listener, which neither export
below reaches.

---

### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.6`

#### 1.1 What `reads: [] closed` denies here, and the call-event distinction

§ reads: the claim denies that one invocation gives rise to any `kind: "read"`, meaning *an observation of a reactive
source's current value*. That includes untracked reads, reads this call schedules to a later `at` event, and non-call
forms such as a proxy property access. It excludes:

- a read a caller-supplied callable performs;
- a property access whose receiver the caller supplied ([Decision 2026-09-10]);
- a read performed by a callable this export did not author, such as computations a third party registered that `flush`
  or `action` drains ([Decision 2026-09-10], "authorship, not timing").

A read of a source the export itself *created* still counts.

**At the call event** of one invocation `action(e)`, all three bundles perform exactly one act. They evaluate an arrow
function expression and return it: `return (...r) => new Promise((t, n) => {` (prod `action.js:77`, bytes 3175–3215;
cjs `:4881`, bytes 205543–205583) and `return (...args) => {` (dev `:6394`, bytes 272295–272316). Nothing is invoked,
no property of `e` or of anything else is read, and no reactive source is touched. Under the narrow reading, "operations
at `action`'s own call event", the denial therefore holds trivially in every bundle.

The claim the rc.3 row rests on is wider. The cited summary `summary-c094d35a…` publishes `invoke-action` (`at: call`,
`from: arg 0`), `optimistic-write` (`at: transition`), `settle-or-revert` (`at: settle`) and `yield-resume`
(`at: settle, queued`). Every one of these is performed when the caller invokes the **returned** function. So the audited
document already counts the returned function's runs as this export's operations, under the "later execution is an
operation" rule. The walk below covers that wider scope: the wrapper and everything it reaches, at the wrapper's call, at
every resumption, and on every `flush` it runs. The verdict is the same under both readings.

#### 1.2 The export's own frames (identical in rc.3 and rc.6)

Against rc.3, the whole `action` body and `restoreTransition` differ **only in the mangled field name** of the
transition's action list: prod `oe` → `ue`, cjs `X` → `ee`. dev `action` and dev `restoreTransition` are
byte-identical to rc.3. Prod `action.js` has the same length in both releases (5898 bytes).

| Call | Site (prod / cjs / dev) | Disposition |
| --- | --- | --- |
| `getOwner()`, `emitDiagnostic(…)`, `throw new Error` | dev only, `:6404-6414`: `ACTION_CALLED_IN_OWNED_SCOPE` | local: `getOwner` returns the module `context`, which is not a reactive source. `emitDiagnostic` calls the diagnostic listeners (hook) |
| `new Promise(executor)` | `:77` / `:4881` / `:6417` | builtin, runs the executor synchronously; the promise goes back to the caller |
| `e(...r)` | executor | caller: the caller's generator function |
| `globalQueue.initTransition()` | executor, `done` | local, § 1.3 |
| `o.ue.push(i)`, `indexOf`, `splice` | executor, `done` | builtin, on a transition batch array |
| `currentTransition(o)` | `done` | local: walks `.sn` links |
| `schedule()` | `done` | local, § 1.3 |
| `t(e)` / `n(r)` | `done` | builtin: resolve/reject of the returned promise |
| `i.next(e)` / `i.throw(e)` | `step` | caller: the caller's iterator, so the generator body is the caller's |
| `isThenable(t)`, `t.then(run, …)` | `step` | local (`typeof e.then`, same body as rc.3) plus caller: the iterator result and its `.then` are caller-supplied receivers |
| `e.done`, `e.value`, `isThenable(e.value)`, `e.value.then(…)` | `run` | caller-supplied receivers. If the generator yields a store proxy, its `then` trap read is the caller's ([Decision 2026-09-10]) |
| `restoreTransition(o, () => step(…))` | `run` | local: `initTransition(o)`, the local arrow, then `flush()` |

#### 1.3 What the wrapper reaches in the scheduler (changed in rc.6)

The scheduler did change: prod `scheduler.js` grew from 32006 to 45716 bytes. I walked the reach from `action` with the
drains cut, in each bundle: 88 frames in prod and cjs, 102 in dev. Separately, I checked each reach set for every
read-kind or source-creating function in the archive: `read`, `readNodeFast`, `latestRead`, `pendingCheckRead`,
`recordFreshRead`, `gatedRead`, `laneReadsCommitted`, `laneSuspends`, `untrack`, `snapshot*`, `authoritativeServe`,
`nodeValue`, `serveShallow`, `readSource`, `serveDataKey`, `isPending`, `latest`, `staleValues`, `signal`, `computed`,
`optimisticSignal`, `optimisticComputed`, `getNode`, `getHasNode`, `getKeySetNode`, `getDeepNode`, `wrapNext` and
`createTarget`. **None of these is reachable from `action`'s own frames in any bundle.**

The frames themselves, with prod lines (cjs and dev in brackets). Every one was read:

- **`GlobalQueue.initTransition`** `scheduler.js:606` (cjs `:1033`, dev `:1945`), **`mergeTransitionState`** `:126`,
  **`createBatch`** `:108`, **`currentTransition`** `:1008`: these move array and Set entries between batches, re-stamp
  `node._transition` fields, and call `schedule()`. local.
- **`schedule`** `:269`: calls `notifyHalted` (host, console), then `queueMicrotask(flush)` (host, a one-shot microtask).
- **`flush`** `:932` (cjs `:1359`, dev `:2264`): action calls it with no argument, so the `fn` branch is not taken. It loops
  `globalQueue.flush()`. dev adds the `FLUSH_IN_EFFECT_CALLBACK` diagnostic and `console.warn`, which are hook and host.
- **`GlobalQueue.flush`** `:468` (cjs `:895`, dev `:1805`) reaches:
  - `sweepDormant` (`graph.js:85`, **new in rc.6**, #3078) → `unobserved` → `clearDeps`/`unlinkSubs` →
    `n.o?.Et?.()`. That callback is the node's `unobserved` option: either caller-supplied to that node's creator, or the
    store's own map-deletion callbacks (`store/next/store.js:167,222,244,264`), which only `delete` entries. Then
    `disposeChildren(e, true)`, whose `runDisposal` and effect-returned `t()` are drains, being cleanups a third party
    registered.
  - `commitPendingNodes` → `commitPendingNode`: pending-value field copies. `GlobalQueue.un = snapCompanionsToState`
    (`verdict.js:254`) → `computePendingState`, which inspects status flags and staged fields, calls `e.pe` (the
    node's `equals`, caller-supplied or `isEqual`), and writes companions. `GlobalQueue.He = disposeChildren`.
    `storeCommitHook = drainFolds` (`store/next/store.js:534`) commits store backings: `Reflect.ownKeys`, descriptor
    copies, `privatizeCommitted`/`cloneRaw`, `notifyFold` → `setSignal`. `patchCommitHook = releaseBatch`
    (`patch.js:129`, **new in rc.6**) → `pushLive` → `globalQueue.enqueue(EFFECT_RENDER, drainApplyQueue)`.
  - `runHeap(dirtyQueue, GlobalQueue.Fe)` / `(zombieQueue, …)`: **drain**, because `Fe = recompute` runs computations a
    third party registered. `cancelZombieRecompute` is local.
  - `transitionComplete` → `reporterBlocksSource` (field walks), `GlobalQueue.dn` (`transitionBlocked`, plus the
    `store/next/optimistic.js:111` wrapper; `n?.[$TARGET]` is a brand lookup, which the trap answers with
    `return e` at `store/next/store.js:1286`).
  - `stashQueues`/`restoreQueues`, `reassignPendingTransition`: arrays.
  - `finalizePureQueue` `:827`:
    - `checkBoundaryChildren` → child queue `.se()`: **drain**, a boundary queue a third party's boundary registered.
    - `GlobalQueue.Tn = resolveOptimisticNodes`, then `enqueueSub`.
    - `GlobalQueue.G = releaseAffectsMarks` → `GlobalQueue.k = repollDownstreamVerdicts(…, snap)` →
      `snapCompanionsToState`.
    - `GlobalQueue.qt`, the `store/next/optimistic.js:82` inline: emits the patch and row-op resync, then
      `runAuthoritative(() => setSignal(e.k, e => e + 1))`, which bumps the keyset node of an optimistic store a third
      party created.
    - `insertSubs` for held-truth reveals (**new**, #3164).
    - `sweepTransientStoreNodes`, a no-op: `transientStoreNodes` is never added to in rc.6.
    - `GlobalQueue.In = cleanupCompletedLanes` → `runQueue`, a **drain**.
  - `GlobalQueue.Nn = runLaneEffects` and `this.run(EFFECT_RENDER|EFFECT_USER)` (`:570`, bytes 24420–24444): **drain**.
    These run effect callbacks, child queues and the patch channel's `drainApplyQueue`. `drainApplyQueue` →
    `applyEntries` → `i.fn(…)` calls the patch consumers the registrants supplied.
  - dev also calls `DEV$1.hooks.onUpdate?.()` (hook, `:1913`) and the four `devCheck*` functions, which are no-ops
    (`:1168-1197`).

Where a `read` does occur on a flush path, it sits behind a drain edge. The examples are `CollectionQueue.run`'s
`read(this.D)` (`boundaries.js:258`, bytes 8858–8923) and `.se()`'s `untrack(() => this.re())` (`:321`). Both run on a
boundary queue that a third party's `Loading`/`Errored` registered, on signals that boundary created. Both are present at
rc.3 (`boundaries.js:258`, `:321`), so they are no change. The dev-only `drainFolds` → `patchHooks.demoteToEffects`
(`dev.js:8407`, bytes 358406–358459, **new in rc.6**) enqueues a render-phase closure. That closure calls
`createRenderEffect(() => fn(proxy, …))` under the registrant's owner. `fn` is the patch registrant's callable, so its
reads are the registrant's.

Two points rest on a judgement, and are consistent with the audited corpus rather than new:

1. The scheduler's field inspection of node records (`e.Re`, `e.S`, `_pendingValue`) is graph bookkeeping, not an
   observation of a reactive source. rc.3's § 3.2 table treats the same machinery (`GlobalQueue.un`) as "a reactive write
   inside the archive".
2. `setSignal`'s internal previous-value lookup for an updater (`core.js:905-906`) is part of a write. Every audited
   write in `solidjs-signals.json` (`stage-write`, `optimistic-write`, `reconcile-write`) sits beside `reads: []` closed.

**No source `action` created is read.** `action` creates no reactive source in any bundle: no signal, computed, store or
node constructor is reachable from its frames. Its only created thing is a transition batch (`createBatch`), which is not
a reactive source.

#### 1.4 What changed from rc.3, and whether it matters

- `action`'s own frames did not change (§ 1.2).
- The rc.6 additions on the flush path are the deferred dormancy sweep (`sweepDormant`), the patch channel
  (`releaseBatch`/`pushLive`/`drainApplyQueue`, `store/next/patch.js`, a file absent at rc.3), the reshaped
  store-optimistic hooks (`qt`/`dn`), the held-truth reveal wake, and in dev `demoteToEffects`.
- These add no read authored by `action`. They are commits, map deletions, companion writes, enqueues, or drains of
  callables that registrants supplied.

`solid-js` split: under the `node`/`worker`/`deno` conditions, `solid-js@2.0.0-rc.3/dist/server.js:1418-1420` is
`function action(fn) { return fn; }` (sha256 `63269da7…`, equal to the rc.3 oracle's copy). It invokes nothing.
`server.cjs:1419-1421` is the same. This is the only solid-js on disk beside rc.6. A consumer that pairs rc.6 with a
solid-js server build other than rc.3 is not covered by this reading. That is the § 1.4 approximation the tier already
carries, now with an unpinned pairing.

**Verdict: GRANT** for prod, cjs and dev.

Sign-off: in `@solidjs/signals@2.0.0-rc.6`, the claim denies that one invocation of `action` (at its call event, which
only returns a wrapper, and on every later run of that wrapper, including the flushes it drives) performs any
observation of a reactive source's current value that `action` authored. It excludes what the caller's generator and its
yielded values read, and what computations, effects, boundary queues, cleanups and patch consumers registered by third
parties read when `flush` drains them.

---

### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

§ creates: the claim denies that one invocation performs a `create`, meaning it registers a version-1 resource into a
runtime outside the invocation (a browser document or a server runtime) where the resource stays live after the call.
Reactive nodes, owners, computations and transitions coming into existence are not `create`s. The summary's `resources`
entry `action-transition` is a declaration, not an operation.

The call event and the wrapper reach are the same as in § 1. The host touches in those reach sets
(`hostcheck.py` over the reach sets) are:

| Bundle | Host touch reachable | What it registers |
| --- | --- | --- |
| prod | `new Promise` (`action.js:77`); `queueMicrotask(flush)` (`scheduler.js:276`, bytes 11693–11776); `console.error` (`notifyHalted`, `:297`); `queueMicrotask(() => t(e))` (`MicrotaskQueue.enqueue`, `signals.js:287`). The last is reached only through `enqueueSub` on a node whose queue an `onSettled` caller registered | a promise returned to the caller, and a one-shot microtask running the archive's own `flush` or a registered apply. No version-1 resource, nothing a host holds afterwards |
| cjs | the same four at `:4881`, `:703` (bytes 28229–28312), `:724`, `:5211` | same |
| dev | the same (`:6417`, `:1608` bytes 64806–64895, `:1638`, `:6797`), plus `emitDiagnostic`'s `queueMicrotask(() => console.warn(footer))` (`:1015`), `console.warn` (`:2296`) and `throw new Error` (`:6414`) | console output only |

The only new host APIs in rc.6 are `until`'s `setTimeout` and `addEventListener`. They are not in any `action` reach
set; a generator that yields `until(…)` makes that call itself, and the call is the caller's. The archive holds no handle
on a document or a server runtime (§ 0), and `action`'s own frames are unchanged from rc.3. `solid-js@rc.3`'s server
`action` returns `fn` (§ 1.4).

**Verdict: GRANT** for prod, cjs and dev.

Sign-off: in `@solidjs/signals@2.0.0-rc.6`, the claim denies that one invocation of `action`, or any later run of the
wrapper it returns, registers a version-1 resource into a browser document or a server runtime. Its only host reaches are
the promise it hands back, `queueMicrotask` of the archive's own `flush` or of a registered apply, and (dev only) console
output.

---

### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.6`

#### 3.1 Definitions

| Bundle | Export | Body |
| --- | --- | --- |
| prod | `store/index.js:30-32` (bytes 760–813) | `function snapshot(e) { return snapshotNext(e); }` |
| cjs | `node.cjs:10116-10118` (bytes 437935–437988) | byte-identical to prod (same slice digest) |
| dev | `dev.js:11987-11989` (bytes 513683–513742) | `function snapshot(value) { return snapshotNext(value); }` |

The callees are `snapshotNext` (prod `store/next/store.js:1732`, cjs `:7788`, dev `:9584`) and the recursive
`snapshotWalk` (prod `:1737`, cjs `:7793`, dev `:9588`).

#### 3.2 Walk (the same 23-frame reach set in all three bundles)

- `snapshotNext`: `e?.[$TARGET]` then `snapshotWalk(e, new Map, fam)`. If `e` is a store proxy, the `$TARGET` brand
  lookup is answered by `return e` (`store.js:1286`). Otherwise it is a caller-supplied receiver.
- `snapshotWalk`:
  - `r?.[$TARGET]?.v`, `n.map.get`, `storeNextLookup.get`, which are brand lookups and WeakMap reads.
  - `pendingBackingVisible(e, true)`, **new in rc.6**, #3147, bytes 84724–84765 in prod. It reaches `inDraft`/`scopeKey`,
    `getWriteOverride`, `inOwnerContext`/`inForbiddenScope`/`getOwner`, `heldTruthMasked` → `authoritativeServe`/
    `authoritativeRead` and `optHooks.retainsOptimism = transitionHoldsOptimism` → `currentTransition`, and `foldHeld`.
    All of these are predicates over module state and target fields.
  - `materializePB`, now conditional on that predicate, → `cloneRaw`.
  - `isWrappable` → `Object.isFrozen`.
  - `optHooks.optimisticView` (`store/next/optimistic.js:532`), which spreads, calls `hasActiveOverride`,
    `unwrapOverride` and `isEqual`.
  - The builtins `Reflect.ownKeys`, `Object.getOwnPropertyDescriptor(s)`, `Object.create`/`getPrototypeOf`,
    `Object.defineProperty` (copying accessor descriptors invokes nothing) and array spread.
- Every getter, setter, proxy trap or iterator reached through those builtins on the caller's data is **caller**.
- `hostcheck.py` over the reach sets: **zero** host references in any of the 23 frames, in any bundle. There is no
  scheduling, no microtask, no timer and no console.

Against rc.3: `snapshot` and `snapshotNext` are byte-identical. `snapshotWalk` gained the `pendingBackingVisible(e, true)`
visibility decision and a conditional `materializePB`; the rest differs only in local variable names. That changes which
backing a snapshot serves, which is `reads`/visibility territory and not this row. It adds no host reach and no
registration.

`solid-js` split: `solid-js@rc.3/dist/server.js:3` re-exports `snapshot` from `@solidjs/signals`, and so do
`solid.js`/`dev.js` and the `.cjs` getters. Every condition therefore runs rc.6's own bytes.

**Verdict: GRANT** for prod, cjs and dev.

Sign-off: in `@solidjs/signals@2.0.0-rc.6`, the claim denies that one invocation of `snapshot` registers a version-1
resource into a browser document or a server runtime. It resolves proxies through target and family maps, may
materialize a store's own pending backing, and returns a plain copy or the original objects. It touches no host API at
all.

---

### 4. Citations (computed over bytes; each slice re-read and re-hashed)

| Row | archive_path | file_sha256 | start_byte | end_byte | slice_sha256 |
| --- | --- | --- | --- | --- | --- |
| `action` | `dist/prod/core/action.js` | `67164bd6a93e34ba2309d4f2d16ada2005c70884e439a6bf14affbdf134b0506` | 3146 | 5879 | `d58c05845ac827bda46a9d6d061f30a5fe1804c955baa2ba855741ae6b7c81a8` |
| `action` | `dist/dev.js` | `f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73` | 272268 | 275948 | `17fac3b6886553d93d15963281b8a2accc36ff6a664622797b4ed1054ce70428` |
| `action` | `dist/node.cjs` | `4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f` | 205514 | 208247 | `3ac24c390918cb76da7303aefb948dad59301f320b06cc147e7347cc896dd73d` |
| `snapshot` | `dist/prod/store/index.js` | `4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8` | 760 | 813 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` |
| `snapshot` | `dist/dev.js` | `f2cf81dfe84a99e170afdb04dd75e762054e976c94a56ccafe84cea235ba6e73` | 513683 | 513742 | `eebc4206bc3509952fd02519f13f9b58ed00bc63da5658c90322c97bd221d6be` |
| `snapshot` | `dist/node.cjs` | `4e509636b109f8728f7ceb38fd52ca42090897ca05f4c978450be0c628d5a86f` | 437935 | 437988 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` |

The citations follow the rc.3 slice convention. Each range starts at the start of the definition's line (the ` */ `
prefix is kept where the minified doc comment ends on that line) and ends after the closing `}` and its newline. Each
slice was checked to begin with `function <export>(` once that prefix is stripped. The `action` rows cite the same
definition bytes for both domains.

Residual approximations, stated:

- **The static reach is an over-approximation.** It is regex-based and method-name-resolved, and I closed it by reading
  each frame. It is not a mechanical proof.
- **Two judgement points (§ 1.3).** Node-record bookkeeping and `setSignal`'s updater lookup are treated as not reads,
  consistent with the audited corpus.
- **The solid-js server pairing was read only for `solid-js@2.0.0-rc.3`.**
- **No rc.6 `files.json` pin exists.** The rc.3 test machinery (`SOLID_CHECKER_RC3_ARCHIVE_ROOT`, the phase0 `files.json`
  pins, `audited-slices/`) has no rc.6 counterpart.
- **These three rows were Summary-cited at rc.3.** Carrying them for rc.6 needs `Implementation` citations (above) and an
  audit section to name, because `solidjs-signals.json` describes rc.3.

