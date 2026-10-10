# Audit: the rc.9 negative rows, re-read on the `2.0.0-rc.13` archives

Date: 2026-10-05. Status: **for the repository owner's review**, as the rc.3,
rc.6 and rc.9 audits it follows were. Its tools,
probe outputs, `rows.json` and `other-answers.json` are kept beside it in
`2026-10-05-solid-2-rc13-negative-rows/`; the rows bind through ADR 0197.

**Why it exists.** Rows are archive-scoped (`NegativeClaimRow::version`), and
ADR 0194 made `2.0.0-rc.13` the audited release without carrying any row to its
archives (`solid_2.rs`, `AUDITED_ARCHIVES` doc: "rc.13 is not listed: no row was
read on its bytes"). This document reads every one of the 48 rows the table
carries for `2.0.0-rc.9` on the exact bytes of the rc.13 archives, by the method
of the audit that granted the rc.9 row, and grants a row for rc.13 only where
its claim is established on rc.13's own bytes.

**Result.** 47 of the 48 rows **GRANT** for rc.13. 1 is
**WITHHELD**: `@solidjs/signals` `reconcile` `reads` (§ 24). rc.13's
`applyAdopt` now composes an optimistic store's view through
`readerOverride → nodeValue → serve`, an engine read-path call that the rc.9
grant said its closure did not contain. Every other rc.9-scoped answer in
`solid_2.rs` that this document checked holds on rc.13 (§ "Other rc.9-scoped
answers").

**What the rc.9 audits were, and what is reused.** The rows come from
`RC9_SIGNALS_AUDIT` (2026-09-26), `RC9_PARITY_AUDIT` (2026-09-27),
`RC9_CORE_WEB_AUDIT` (2026-09-27), `RC9_MERGE_OMIT_MEMO_AUDIT` (2026-09-28) and
`RC9_READS_AUDIT` (2026-09-30). Each row section below names the rc.9 section it
re-reads, keeps that section's reading rules, dispositions and owner flags, and
states what changed on rc.13's bytes and whether that matters. Where the export
slice and every helper it reaches are byte-identical to rc.9's, the verdict is
carried and the section says so. `docs/package-contract-v2/audits/2026-10-05-solid-2-rc13/runtime.md`
(the rc.9 → rc.13 runtime comparison) was used as a map and re-measured per row,
not trusted.

Every claim is tagged **[M]** (measured: a digest, a byte range, a tool run, or
a probe on the named bytes) or **[E]** (estimated: a reading or judgement not
mechanically checked).

## 0. For the owner's sign-off

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| 1 | `@solidjs/signals` `action` `reads` | **GRANTED** | Wrapper frames are rc.9's plus module counters; no read entry point reachable in any build with rc.9's cuts; drains are the registrants'. |
| 2 | `@solidjs/signals` `action` `creates` | **GRANTED** | Host reaches are the returned Promise, queueMicrotask, console, and dev/observe-only WeakRef/FinalizationRegistry bookkeeping; none registers a version-1 resource. |
| 3 | `@solidjs/signals` `createMemo` `reads` | **GRANTED** | Export slices byte-identical; changed frames (computed, setupComputedNode, recompute, handleAsync) add no read entry point; external-source hook line unchanged. |
| 4 | `@solidjs/signals` `createMemo` `creates` | **GRANTED** | Signals archive has no handle on a document or server runtime (§ 0.3); closure host reaches are microtasks, console and dev bookkeeping. |
| 5 | `@solidjs/signals` `createOptimistic` `reads` | **GRANTED** | dev/observe slices identical, prod differs by one mangled field; engine install and node constructors identical; neither the call nor the setter closure reaches a read entry point. |
| 6 | `@solidjs/signals` `createOptimistic` `creates` | **GRANTED** | Node allocation and engine slots; host reaches are microtasks and console. |
| 7 | `@solidjs/signals` `createOptimisticStore` `creates` | **GRANTED** | Only code change in the export is dev/observe nameStore (an attribution-only map write); closure stays inside the archive. |
| 8 | `@solidjs/signals` `createProjection` `creates` | **GRANTED** | Export slices byte-identical; changed projection internals stay inside the archive. |
| 9 | `@solidjs/signals` `createRoot` `creates` | **GRANTED** | dev/observe slices identical, prod locals renamed; createOwner adds a dev/observe-only weak root registry (module FinalizationRegistry), judged not a create (owner flag O1). |
| 10 | `@solidjs/signals` `createSignal` `reads` (argument scope) | **GRANTED** | dev/observe slices identical, prod one mangled field; plain path signal/accessor identical; function path is createMemo's clean closure; probes 16/16 per build. |
| 11 | `@solidjs/signals` `createSignal` `creates` | **GRANTED** | Same node constructors; host reaches are microtasks and console. |
| 12 | `@solidjs/signals` `createStore` `creates` | **GRANTED** | prod slice identical; dev/observe add nameStore (attribution map write); closure stays inside the archive. |
| 13 | `@solidjs/signals` `createTrackedEffect` `reads` | **GRANTED** | Export slices, trackedEffect and enqueueSub byte-identical; no read entry point reachable. |
| 14 | `@solidjs/signals` `createTrackedEffect` `creates` | **GRANTED** | Same closure; host reaches are microtasks and console; server body registers nothing. |
| 15 | `@solidjs/signals` `flush` `reads` | **GRANTED** | flush adds only an attribution hook; GlobalQueue.flush's new boundary re-arm/judge calls are boundary-queue methods (drain); no read entry point with rc.9's cuts. |
| 16 | `@solidjs/signals` `flush` `creates` | **GRANTED** | Drain stays inside the archive; host reaches are microtasks, console and haltReactivity's reportError. |
| 17 | `@solidjs/signals` `getOwner` `reads` | **GRANTED** | All three slices byte-identical (return context); no call. |
| 18 | `@solidjs/signals` `getOwner` `creates` | **GRANTED** | All three slices byte-identical; no helper. |
| 19 | `@solidjs/signals` `omit` `creates` | **GRANTED** | Closure is store/utils view code, built-ins and caller values only; the rc.13 changes (one-symbol record lookup, re-homed accessors on the copy path) call nothing new. |
| 20 | `@solidjs/signals` `onCleanup` `reads` | **GRANTED** | All three slices byte-identical; dev closure is the same 8 functions, two of them diagnostic code with no read. |
| 21 | `@solidjs/signals` `onCleanup` `creates` | **GRANTED** | Slices identical; dev diagnostics only. |
| 22 | `@solidjs/signals` `onSettled` `reads` | **GRANTED** | dev slice identical, prod/observe mangled fields only; no read entry point reachable. |
| 23 | `@solidjs/signals` `onSettled` `creates` | **GRANTED** | Both arms reduce to trackedEffect or Queue.enqueue + schedule. |
| 24 | `@solidjs/signals` `reconcile` `reads` | **WITHHELD** | rc.13 applyAdopt composes the optimistic view through readerOverride -> nodeValue -> serve, an engine read-path call the rc.9 grant said did not exist in the closure; reachable unless every application runs under authoritativeServe(), which was not established. |
| 25 | `@solidjs/signals` `reconcile` `creates` | **GRANTED** | Application performs adoption writes, setSignal and schedule; the new serve path can only enter a transaction (schedule -> queueMicrotask). |
| 26 | `@solidjs/signals` `runWithOwner` `reads` (argument scope) | **GRANTED** | dev/observe slices identical, prod locals renamed; the only invocation is fn; probes as rc.9. |
| 27 | `@solidjs/signals` `runWithOwner` `creates` | **GRANTED** | Saves/restores context and tracking around fn; dev diagnostic only. |
| 28 | `@solidjs/signals` `snapshot` `creates` | **GRANTED** | Slices identical; snapshotWalk's reach is still the archive's own flush microtask. |
| 29 | `@solidjs/signals` `untrack` `reads` (argument scope) | **GRANTED** | prod/observe differ only in the hook slot's mangled name; dev adds a counter and a fast-path condition; the only invocation is fn or the external-source hook. |
| 30 | `@solidjs/signals` `untrack` `creates` | **GRANTED** | Flag toggles and counters around fn; no helper call. |
| 31 | `@solidjs/web` `clientOnly` `reads` | **GRANTED** | All six slices identical; call-event closure (solid-js createSignal plain path, loadClientOnly) identical; returned component is F3. |
| 32 | `@solidjs/web` `clientOnly` `creates` | **GRANTED** | Nothing registers at the call in any build; server preload registration is inside the returned component (F3). |
| 33 | `@solidjs/web` `httpHeader` `reads` | **GRANTED** | Slices identical; getRequestEvent now also looks the request event up through a realm-global WeakMap of render roots; no reactive source. |
| 34 | `@solidjs/web` `httpHeader` `creates` | **GRANTED** | Writes to an existing response plus a retracting cleanup; owner flag carried from rc.9. |
| 35 | `@solidjs/web` `httpStatus` `reads` | **GRANTED** | As httpHeader. |
| 36 | `@solidjs/web` `httpStatus` `creates` | **GRANTED** | As httpHeader. |
| 37 | `solid-js` `For` `reads` | **GRANTED** | Six slices identical; signals mapArray identical and updateKeyedMap changes only route its row writes; server mapArray changes an id read. |
| 38 | `solid-js` `For` `creates` | **GRANTED** | Client stays inside the signals archive; server createSyncMemo never reaches processResult. |
| 39 | `solid-js` `Match` `reads` | **GRANTED** | function Match(props) { return props; } identical in all six builds. |
| 40 | `solid-js` `Match` `creates` | **GRANTED** | Same body. |
| 41 | `solid-js` `Repeat` `reads` | **GRANTED** | Six slices identical; signals repeat/updateRepeat identical in dev; server repeat changes an id read. |
| 42 | `solid-js` `Repeat` `creates` | **GRANTED** | As For. |
| 43 | `solid-js` `createContext` `creates` | **GRANTED** | Six slices identical; the call's only callee is Symbol(); provider is defined, not invoked. |
| 44 | `solid-js` `createMemo` `creates` (`browser`) | **GRANTED** | Wrapper identical; browser closure (40 definitions, identical across the three builds) reaches only the same five signals imports, module state, a restored fetch/Promise swap and microtasks; delegates granted on rc.13 signals. |
| 45 | `solid-js` `createSignal` `reads` (argument scope) | **GRANTED** | Wrapper, hydratedCreateSignal and all three server bodies identical; a primitive first argument reaches only the signals plain path (delegate granted). |
| 46 | `solid-js` `createSignal` `creates` (`browser`) | **GRANTED** | As createMemo browser: wrapper identical, closure 40 definitions identical across builds, delegates granted on rc.13 signals. |
| 47 | `solid-js` `useContext` `reads` | **GRANTED** | Six slices identical; signals getContext identical in dev and differs by one mangled field in prod/observe; server getContext identical. |
| 48 | `solid-js` `useContext` `creates` | **GRANTED** | Same bytes; a context-map read that may construct and throw an Error. |

### What the owner is asked to agree with beyond the table

1. **O1 — the rc.13 dev/observe root registry is not a `create`** [E]. **Accepted by the repository owner, 2026-10-05 (ADR 0197).** rc.13's
   `createOwner` calls `registerRoot(owner)` for an owner with no parent (dev
   `dist/dev-shared.js:775-783`, observe `dist/observe/core/dev.js:265-273`;
   prod folds it to nothing). It creates a `WeakRef`, inserts into a module
   `WeakMap` and `Set`, and calls `register` on a module-level
   `FinalizationRegistry` whose callback is `ref => liveRoots.delete(ref)`. The
   registry is the archive's own module object, and the host's only act is to
   call the archive's own deleter back after collection. So it registers nothing
   into a browser document or a server runtime that acts on it. Every
   `@solidjs/signals` `creates` row whose closure creates a parentless owner
   (`createRoot` above all) rests on this ruling. The same holds for dev's
   `watchAsyncTail` (a `WeakRef` pair in a module `Map`, and an
   archive-authored `async` function awaiting the caller's own promise).
2. **O2 — `reconcile` `reads` is withheld, and here is what would grant it.**
   Either a reading that proves every `applyAdopt` call on an optimistic family
   runs with `authoritativeServe()` true, which would make `readerOverride`
   unreachable. Or a ruling that an engine read of a node inside a store the
   caller handed in is the caller's read. That would extend Decision 2026-09-10's
   receiver clause, which today covers a property access, to the archive's own
   read path. Neither is established here (§ 24).
3. **The rc.9 lines are kept unchanged.** These are the external-source hook line
   (`wireExternalSource` is byte-identical), the accessor line
   (`createOptimistic`), the returned-value line F3 (`For`, `clientOnly`), the
   row-signal line F2, the drain line ([Decision 2026-09-10]: `flush`,
   `action`), the `reportError` ruling and the `httpHeader`/`httpStatus` owner
   flag. If any of them is redrawn, the rc.6, rc.9 and rc.13 rows move together.
4. **The spurious `write` edge is cut** (§ 0.4). The name-based call graph links
   `attrHooks.write(…)` (the attribution hook, in `setSignal`) and
   `updateKeyedMap`'s local `write(…)` to an unrelated top-level arrow
   `const write = () => storeSetterNext(…, reconcileNextState(…))`. That arrow
   lives inside `runProjectionComputedNext` (dev `dist/dev.js:8029`, prod
   `store/next/projection.js:248`). It is not reachable from any of the `reads`
   roots, which never reach `runProjectionComputedNext` [M]. Without the cut,
   every `reads` closure "reaches" `nodeValue → serve` through it.

### What this audit does not do

- It reads no domain or export beyond the 48 rows. In particular it does not
  re-read the rc.9 withholdings (`createOptimisticStore` `reads`, `merge`
  `creates`, `Show`, `Loading`, flat `solid-js` `createSignal`/`createMemo`,
  `affects`/`isPending`/`latest`/`refresh`, `hydrate`/`render` `reads`). They
  stay rowless on rc.13.
- It reads no release between rc.9 and rc.13, and it changes no code. § "What
  the lead must change" lists the code that has to move for the rows to bind.
- It did not download the tarballs again. Identity rests on the
  `benchmarks/package-contract-v2/phase0/rc13/*/tarball.json` records (`npm pack`
  of 2026-10-05, SRI equal to the registry's) and on the per-file manifests,
  which every file of the three installs matches (§ 0.1) [M].

---

## Shared inputs (§ 0.1–0.6; every row relies on these)

### 0.1 Identity [M]

| archive | integrity (`tarball.json`) | `package.json` sha256 | files |
| --- | --- | --- | --- |
| `@solidjs/signals@2.0.0-rc.13` | `sha512-4+pRdrAHtfyE9BUJWBup3TpzJojJjgW2mV1vm/Jik4tWa5epxXB/YrLkwqP1v8+S9XjyKKZu5BSLqmcpswMYeQ==` | `6783c3c632cfebf3a0fc26e18925887462b5925c481c6535ae0186845efb95a6` | 119 |
| `solid-js@2.0.0-rc.13` | `sha512-62bYOI4JZ15KOqL5eReKyWSwAXrGb0fbX8SDnHsbJk2UX+SzSyy1gobas2cxW/0BXcobGfQ1V6ffPyQkMIBdoQ==` | `ce43a022f763e5995d1ac8a5f78f135804edd01b627a2bcbee8ff9530e79d284` | 34 |
| `@solidjs/web@2.0.0-rc.13` | `sha512-vI/7v/XM8B/3U/oKAzCW2VjTLFkM5IArB2DFnrjxyPRYD3FX9KAXg9QuAnPIPQHC/xvg/LgxmvoVfZUfJ96gOQ==` | `8b45ed71ed7a369883e7e00bb48b01fd7bda935bd5ba889cbecafc8eaa122bf5` | 75 |

- Bytes read: `rust/target/audit-rc13/node_modules/{@solidjs/signals,solid-js,@solidjs/web}`.
  Every file of the three trees has the sha256 and length that
  `benchmarks/package-contract-v2/phase0/rc13/{solidjs-signals,solid-js,solidjs-web}/files.json`
  pins (34/34, 119/119, 75/75, no mismatch).
- `solid-js@2.0.0-rc.13` depends on `@solidjs/signals: ^2.0.0-rc.13`, and
  `@solidjs/web@2.0.0-rc.13` peers `solid-js: ^2.0.0-rc.13`. All three share
  gitHead `5efaf260becb32293f2bcb4d32f8be72be6de674`.
- The rc.9 comparand is
  `rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules`. Every rc.9
  citation the 48 rows carry was re-hashed there. Each file equals
  `phase0/rc9/*/files.json`, and each cited range hashes to its `slice_sha256`
  [M].

### 0.2 Builds, selection and bindings [M]

- **`exports` maps.** `@solidjs/signals` and `solid-js`: `exports.json` is
  identical between rc.9 and rc.13. `@solidjs/web` adds only a
  `./performance-tracks` subpath; `.` is identical. So every build a row cites
  on rc.9 is still the build the same condition selects on rc.13.
  - signals: `test`/`development` → `dist/dev.js` (with `dist/dev-shared.js`),
    `observe` → `dist/observe/index.js`, `default` → `dist/prod/index.js`.
  - `solid-js` and `@solidjs/web`: `browser` (or no host) →
    `dist/solid{,.dev,.observe}.js` / `dist/web{,.dev,.observe}.js`; `worker`,
    `deno`, `node` → `dist/server{,.dev,.observe}.js`.
- **Signals bindings.** The `from` lists of `dist/prod/index.js` and
  `dist/observe/index.js` are identical to rc.9's, and each row's export is
  re-exported from the same module (`core/core.js`, `core/owner.js`,
  `core/scheduler.js`, `core/action.js`, `signals.js`, `store/index.js`,
  `store/utils.js`, `store/next/optimistic.js` as `createOptimisticStore`,
  `store/next/projection.js` as `createProjection`). In dev, `flush`,
  `getOwner`, `createRoot`, `runWithOwner` and `untrack` are defined once in
  `dist/dev-shared.js` and imported into `dist/dev.js` (`aN`, `g`, `aY`, `aS`,
  `b2`). The other thirteen are defined once in `dist/dev.js`.
- **Every cited definition was found exactly once** in its rc.13 file, and the
  rc.9 slice convention of each citation was reproduced on rc.9 before it was
  applied to rc.13 (§ "Citations").
- **`solid-js` declarations.** `types/index.d.ts:1` still re-exports `getOwner`,
  `onCleanup`, `untrack`, `runWithOwner`, `action`, `createTrackedEffect`,
  `onSettled`, `flush`, `reconcile`, `snapshot`, `omit` and `merge` from
  `@solidjs/signals`. `:8` still re-declares `createRoot`, `createMemo`,
  `createSignal`, `createStore`, `createProjection`, `createOptimistic` and
  `createOptimisticStore` from `./client/hydration.js`. Only `sharedConfig`,
  `$DEVCOMP` and the three boundary creators were dropped from these lines, and
  `isHydrating`/`isHydratable` were added (runtime.md item 8). So the binding argument of
  every rc.9 `solid-js` pairing is unchanged.

### 0.3 Archive-wide host-boundary censuses [M]

The tool is `tools/hostcensus.mjs`, run on rc.9 and rc.13. It uses the acorn
tokenizer, so comments, strings, templates and regular expressions are skipped.
It lists every identifier-boundary occurrence of the rc.9 watch list, plus
`serialize`, `require`, `importScripts`, `location`, `history`, `crypto`,
`Headers`, `Response`, `Request`, `URL`, `ReadableStream`, `TextEncoder`,
`MutationObserver`, `AbortController`, `Buffer`, `AsyncLocalStorage`, `Node`
and dynamic `import()`/`import.meta`. Each occurrence that is not a property
name is mapped to its enclosing top-level statement, and every non-relative
import is listed. Raw output: `tools/host-*-rc{9,13}.txt`.

**`@solidjs/signals@2.0.0-rc.13`, every `.js` under `dist/`.**

- No import leaves the archive. There is no `document`, `sharedConfig`, `_$HY`,
  serializer, network or storage API, and no dynamic code.
- Every reference that leaves the archive on rc.9 is still there:
  `queueMicrotask` in `schedule`, `MicrotaskQueue`, `refresh` and dev
  `emitDiagnostic`; `Promise` in `action`, `resolve`, `refresh` and `until`;
  `setTimeout`/`clearTimeout` in `until`; `globalThis.reportError` and
  `console` in `haltReactivity`; the two `globalThis[Symbol.for(…)]` slots;
  `globalThis.process?.env?.COMPANION_CENSUS` at module load; `Node` in
  `isWrappable`; `console`.
- Every `self` hit is a parameter or local name (`disposeChildren`, `computed`,
  `setupComputedNode`, `createEffectNode`, `createOptimisticStoreNext`, …), as
  on rc.9; `Node` is `isWrappable`'s predicate read.
- **New in rc.13:**
  - `scheduleWithheld` → `queueMicrotask(flush)`, in all three builds.
  - Dev only: `checkPostAwaitRead` → `queueMicrotask(healDisposalDepth |
    resetWindowFlight)` and a diagnostic, which is the
    `UNTRACKED_READ_AFTER_AWAIT` guard.
  - Dev only: `watchAsyncTail`, which uses `Promise`, a `WeakRef` pair in a
    module `Map`, and a named `async` function that awaits the caller's native
    promise.
  - Dev and observe: `registerRoot`/`rootReaper`, which uses `WeakRef` and a
    module `FinalizationRegistry` (owner flag O1).
  - Inside the `./attribution` entry only: `Date`, `setTimeout`/`clearTimeout`,
    and local `history` bindings.
- **Consequence**, as on rc.3, rc.6 and rc.9: no call that stays inside the
  archive can register a version-1 resource into a browser document or a
  server runtime. For each signals `creates` row, the question therefore
  reduces to its `solid-js` server pairing (§ 0.5) and to the new
  host-touching helpers listed above, none of which registers a version-1
  resource kind (O1).

**`solid-js@2.0.0-rc.13`, the six `.` builds.**

- Each build imports only `@solidjs/signals`.
- New client host references: `Promise` (`adoptedAnswerStream`,
  `quietAnswer`); `queueMicrotask` (`hydrateSignalLike`, `hydrateStoreLikeFn`,
  `whenRevealed`); `document`/`globalThis` in `fragmentParked`, which replaces
  `fragmentPending`; `performance` in dev/observe `recover` and the hydrated
  loading boundary; `console` in dev `createConsoleTask`.
- New server host references: `Promise` (`createSharedSource`,
  `tappedCloser`) and `globalThis` (`callerRenderContext`). `CLIENT_HOLE`'s
  `Promise` and `reportServerError`'s `sharedConfig` are gone.
- None of the new references is in the closure of a granted `solid-js` row.
  The closures were walked per row (§§ 37–48).

**`@solidjs/web@2.0.0-rc.13`.** The web rows' verdicts rest on their own
closures (§§ 31–36), which were re-walked; the census output is kept for
reference (`tools/host-web-rc{9,13}.txt`).

### 0.4 How the walks were done [M]

- **Read-site call graph** (`tools/closure.mjs`). This is the rc.9 reads
  audit's `callgraph.mjs` (acorn, name-based, over-approximating) re-run over
  `dist/prod/**`, `dist/observe/**`, and `dist/dev.js` with
  `dist/dev-shared.js`, on both versions. Before use it was checked to
  reproduce the rc.9 audit's counts on rc.9: `createMemo` 83/164/171,
  `createSignal` 85/166/173, `onCleanup` 2/2/8, `runWithOwner` 1/1/6 (prod,
  observe, dev, `flush` cut). It reports calls to the read entry points
  `read`, `readNodeFast`, `serve`, `link`, `pendingCheckRead`, `latestRead` and
  `getLatestValueComputed`, plus value uses and the host identifiers of every
  reached function. For each root it also lists which reached definitions are
  new, gone or textually changed against rc.9.
- **Cuts**, exactly the rc.9 audits':
  - `flush` (and the `wireExternalSource` hook) for the call-event roots.
  - The boundary-queue methods `run` and `_evaluate`/`B`, plus
    `wireExternalSource`, for `flush` and `action`.
  - The spurious `write` edge, owner item 4.
- **Archive-wide read-site census** (`tools/readsites.mjs`). The set of
  functions that contain a read-entry call or value use is the same name set on
  rc.13 as on rc.9 in all three builds: `accessor`, `read`, `readNodeFast`,
  `nodeValue`, `serveDataKey`, `firewallGate`, `deepNext`, `pendingCheckRead`,
  `latestRead`, `getLatestValueComputed`, `isPending`, `refresh`,
  `wireExternalSource`, the boundary code, and so on.
  - The dependency-linking write `sub._depsTail = nextDep` is still only in
    `link` (`dev-shared.js:3602`).
  - **What is new is a caller:** rc.13's `readerOverride` (`dist/dev.js:6392`, prod
    `store/next/store.js:1577`) calls `nodeValue`, which calls `serve`. It is
    reached from `optimisticView`, so through `applyAdopt` (§ 24) and
    `snapshotWalk` (§ 28).
- **Hook slots** (`tools/slots.mjs`, dev). Every `GlobalQueue._x(…)`,
  `globalQueue.x(…)` or `optHooks.x(…)` call in a `reads` closure was resolved
  to the function assigned to the slot, and that function's own closure was
  walked with the same cuts.
  - Every assigned slot reaches no read entry point. These are:
    `_applyReask`, `_dispose`, `_endOptimism`, `_landOnOverride`,
    `_laneAsyncPending`/`Settled`, `_laneOverride`, `_markAffects`,
    `_optimisticWrite`, `_recomputeLane`, `_repollVerdicts`,
    `_runLaneEffects`, `_snapCompanions`, `_supersedeOverride`,
    `_syncCompanions`, `_trackOptimisticStore`, `_updateChildCompanions`,
    `_updatePendingSignal`, `_wakeSuppressedProbes`, and the inline
    `_clearOptimisticStores` (which only writes).
  - `_externalUntrack` and `_wireExternalSource` are the external-source
    hooks.
- **Per-file top-level walk** (`tools/topwalk.mjs`). This is the merge/omit and
  core-and-web audits' breadth-first walk over the top-level definitions of one
  file. It follows every identifier that names another definition and lists
  the imported bindings and well-known globals used. On rc.9 it reproduced the
  27-definition `createSignal`/`createMemo` browser closure and the 45-definition
  `omit` closure that those audits recorded.
- **Diffs.** Definitions were diffed by name between rc.9 and rc.13
  (`tools/show.mjs`, `tools/defdiff.mjs`, `tools/slicediff.mjs`), in the
  unmangled dev build where one exists. Prod and observe were diffed as cited
  slices. "Short identifiers only" in a citation table means the two slices are
  equal once comments and whitespace are removed and every identifier of one or
  two characters is masked. That is how mangled locals and fields were
  separated from code changes.

### 0.5 The `solid-js@2.0.0-rc.13` server bodies (the pairing) [M]

Under `node`/`worker`/`deno` a `solid-js` import runs `dist/server{,.dev,.observe}.js`.
For every name `solid-js` re-exports from the signals archive, the server body
was compared with rc.9's, by top-level definition, in all three server builds.

| name | rc.13 vs rc.9 (all three server builds) |
| --- | --- |
| `getOwner`, `onCleanup`, `untrack`, `runWithOwner`, `stampThrower` | byte-identical |
| `action` (`return fn`), `flush` (`function flush() {}`) | byte-identical |
| `createTrackedEffect`, `onSettled` (`getOwner()` + `getNextChildId`) | byte-identical. `nextChildIdFor` now prefixes a hole-scoped owner's id with `materializeId(counter)`, an id-string write on the owner |
| `reconcile`, `setProperty` | byte-identical |
| `snapshot`, `omit` | re-exported from `@solidjs/signals` in all three (`dist/server.js:2`, unchanged) |
| `getContext`, `useContext`, `serverComponentContextError`, `createContext`, `For`, `Repeat`, `Match`, `createSyncMemo`, `createOwner`, `formatChildId`, `createSignal` | byte-identical |
| `mapArray`, `repeat` | `rowOwner.id` → `ownerId(rowOwner)` (an id read; dev throws on an unmaterialized hole-scoped owner) |
| `createMemo` | the owner-disposal flag now also runs `comp.onDisposed` hooks, at disposal |

None of these bodies registers anything, reads a reactive source, or touches
`sharedConfig.context` on the paths the rows take.

### 0.6 Probes [M]

The rc.9 reads audit's probes
(`docs/package-contract-v2/audits/2026-09-30-solid-2-rc9-reads-rows-probes/probe.mjs`
and `probe-solid-js.mjs`, copied unchanged) were run on both versions. Each run
used a project under `2026-10-05-solid-2-rc13-negative-rows/probes/{rc9,rc13}/` whose
`node_modules` symlinks to that version's install, with Node 24.21.0. Raw
output is in `probes/output-rc{9,13}.txt`. `import.meta.resolve` confirms the
rc.13 files loaded: signals `dist/prod/index.js`, `dist/dev.js` and
`dist/observe/index.js`; `solid-js` `dist/solid.js` under `browser` and
`dist/server{,.dev,.observe}.js` under `node`.

- **Signals.** 16/16 as stated in prod, dev and observe, on rc.9 and on rc.13.
  This covers `getOwner`, `onCleanup`, the literal-callable forms of `untrack`
  and `runWithOwner`, the primitive forms of `createSignal` and its function
  form, and the four by-reference reads the argument scopes decline. The
  calibration that finds the build's dependency fields at run time (ADR 0163's
  veto) succeeded on every rc.13 build.
- **`solid-js`, client builds.** On rc.9 all 7 checks hold. On rc.13, 6 of 7
  hold. Every `createSignal(5)` and `useContext` check holds. The exception is
  the contrast check `createSignal(fn, { ssrSource: "client" })` under
  `sharedConfig.hydrating`, which now runs `fn` at the call (count 1; rc.9: 0).
  rc.13's `hydrateSignalLike` returns `coreFn(fn, options)` when the owner has
  no hydration id (`noHydrationId()`), and the probe's owner has none. That is
  the function path the `solid-js` `createSignal` `reads` scope excludes; it
  does not bear on any row.
- **`solid-js`, server builds.** 2/2 in all three, on both versions.


### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group D § 1 (call event returns an arrow; every run of the wrapper, its resumptions and the flushes it drives).

**rc.13 bytes [M].** prod and observe differ from rc.9 only in mangled property names on the transition record (prod `s.he` → `s.Te`; observe `s.dt`/`s.Tt` → `s.tt`/`s.et`); the byte ranges are unchanged (4429..7642, 4502..8183). dev adds `enterCallback()` before the generator step and `exitCallback()` on both exits: a module counter (`callbackDepth`) that the new dev `UNTRACKED_READ_AFTER_AWAIT` check consults (`dev-shared.js` `enterCallback`/`exitCallback`, two statements each).

**Closure [M].** With rc.9's cuts (boundary-queue methods `run`, `_evaluate`/`B`, the `wireExternalSource` hook) and the spurious `write` edge (§ 0.4), the call graph from `action` reaches no read entry point: prod 61→67 functions, read-entry calls none; observe 68→75 functions, read-entry calls none; dev 197→223 functions, read-entry calls none (cut: `run,_evaluate,B,wireExternalSource,write`). Hook slots followed in dev reach none (§ 0.4). `flush` and `GlobalQueue.flush` are § 15's.

**Pairing [M].** `solid-js@2.0.0-rc.13` server `action` is byte-identical to rc.9 (`return fn`) in all three server builds (§ 0.5).

**Verdict: GRANTED** (prod, dev, observe). Denies that one invocation of `action`, at its call event or on any later run of the wrapper it returns, observes a reactive source `action` authored.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/action.js | 94-165 | 4429 | 7642 | `e12ad95406313eadf1c98f2981feebd6d14a2ad8e568ca978b2492febb8ad2a6` | 4429..7642 `92babceb…` | short identifiers only |
| dist/dev.js | 2087-2197 | 94506 | 99299 | `dace43abb94b3bc3e1b147243818b3f67d8f7b402647e90b21a8bd7f7557afeb` | 87962..92525 `6b9bd50c…` | code changed |
| dist/observe/core/action.js | 98-175 | 4502 | 8183 | `e2ecc7ade034ed0e609d41dee4da36f44ee72c799fd121d22af9ba6aff029a9c` | 4502..8183 `a503cb66…` | short identifiers only |

---

### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group D § 2.

**rc.13 bytes [M].** As § 1.

**Closure [M].** Host references reached per build: host references reached, rc.13 — prod: `Promise` (action), `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `Promise` (action), `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `Promise` (action, resolve, watchAsyncTail), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `self` (createEffectNode, disposeChildren, setupComputedNode), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime. The dev `Promise`/`resolve` entries include the spurious executor-parameter edge rc.9 already named; `WeakRef`/`watchAsyncTail` and `registerRoot` are § 0.3's new dev/observe bookkeeping (owner flag O1).

**Pairing [M].** Server `action` identical (`return fn`).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/action.js | 94-165 | 4429 | 7642 | `e12ad95406313eadf1c98f2981feebd6d14a2ad8e568ca978b2492febb8ad2a6` | 4429..7642 `92babceb…` | short identifiers only |
| dist/dev.js | 2087-2197 | 94506 | 99299 | `dace43abb94b3bc3e1b147243818b3f67d8f7b402647e90b21a8bd7f7557afeb` | 87962..92525 `6b9bd50c…` | code changed |
| dist/observe/core/action.js | 98-175 | 4502 | 8183 | `e2ecc7ade034ed0e609d41dee4da36f44ee72c799fd121d22af9ba6aff029a9c` | 4502..8183 `a503cb66…` | short identifiers only |

---

### 3. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 1.

**rc.13 bytes [M].** All three slices are byte-identical to rc.9's (prod/observe `f0f1d1a8…`, dev `46130d23…`).

**What changed in the closure [M].** `computed` adds a `_plumbing` config bit and leaves plumbing nodes unnamed; `setupComputedNode` replaces its inline child link with `linkChild(context, self)` (four field writes) and keeps `!options?.lazy && recompute(self, true)`; `recompute` gains calls to `findLane`, `clearDeps`, `underFreshLoadingBoundary` (a queue-chain flag walk), `el._queue.notify` (the boundary queues' `notify`, which write `_disabled`/`_error` with `setSignal` and report errors) and `NotReadyError`; `handleAsync` attaches its continuation through dev's `watchAsyncTail` wrapper (§ 0.3). `accessor`, `signal`, `wireExternalSource` and `externalUntrack` are byte-identical.

**Closure [M].** With rc.9's cuts (`flush`, the `wireExternalSource` hook) and the spurious `write` edge: prod 83→91 functions, read-entry calls none; observe 91→100 functions, read-entry calls none; dev 154→194 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). Dev hook slots reach none.

**The guarded read on the stack.** Unchanged: the `wireExternalSource` wrapper (`dist/dev.js`, byte-identical to rc.9) reads the bridge signal the hook created; dispositioned **hook**, as on rc.3, rc.6 and rc.9.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 83-85 | 3341 | 3408 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | 3028..3095 `f0f1d1a8…` | identical |
| dist/dev.js | 2356-2358 | 105095 | 105184 | `46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0` | 98008..98097 `46130d23…` | identical |
| dist/observe/signals.js | 86-88 | 3423 | 3490 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | 3110..3177 `f0f1d1a8…` | identical |

---

### 4. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 2.

**rc.13 bytes [M].** Slices byte-identical (§ 3).

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime. `createMemo` is `solid-js`' own declaration in rc.13 (`types/index.d.ts:8`), so no server pairing applies.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 83-85 | 3341 | 3408 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | 3028..3095 `f0f1d1a8…` | identical |
| dist/dev.js | 2356-2358 | 105095 | 105184 | `46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0` | 98008..98097 `46130d23…` | identical |
| dist/observe/signals.js | 86-88 | 3423 | 3490 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | 3110..3177 `f0f1d1a8…` | identical |

---

### 5. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 3 (rests on the accessor line, rc.6 owner flag 2).

**rc.13 bytes [M].** dev (`9180aeea…`) and observe (`760ee92d…`) are byte-identical to rc.9. prod differs in one mangled field name (`n.T &= ~CONFIG_AUTO_DISPOSE` → `n.C &= …`). `installOptimisticEngine`, `optimisticComputed`, `optimisticSignal` and `accessor` are byte-identical in dev.

**Closure [M].** Call: prod 88→96 functions, read-entry calls none; observe 98→107 functions, read-entry calls none; dev 159→198 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). Returned setter (`setSignal`): prod 31→32 functions, read-entry calls none; observe 38→40 functions, read-entry calls none; dev 42→47 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). `setSignal`'s only code change is `captureWriteSnapshot` while a hydration snapshot capture is active (a write-side record).

**Verdict: GRANTED** (prod, dev, observe). Not denied, as on rc.9: a later, separate call of the returned accessor.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 599-611 | 26797 | 27344 | `e79b75e3d7d2700ab665f566f53fd92fa47d3286f059a3e117bef3edc31a7658` | 26484..27031 `09fd4865…` | short identifiers only |
| dist/dev.js | 2923-2936 | 129870 | 130478 | `9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3` | 122783..123391 `9180aeea…` | identical |
| dist/observe/signals.js | 602-615 | 26879 | 27460 | `760ee92d8393a3d2102625aadc0c31e9378285b46bc98c4af9b2e0fa85697fe1` | 26566..27147 `760ee92d…` | identical |

---

### 6. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 4.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 599-611 | 26797 | 27344 | `e79b75e3d7d2700ab665f566f53fd92fa47d3286f059a3e117bef3edc31a7658` | 26484..27031 `09fd4865…` | short identifiers only |
| dist/dev.js | 2923-2936 | 129870 | 130478 | `9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3` | 122783..123391 `9180aeea…` | identical |
| dist/observe/signals.js | 602-615 | 26879 | 27460 | `760ee92d8393a3d2102625aadc0c31e9378285b46bc98c4af9b2e0fa85697fe1` | 26566..27147 `760ee92d…` | identical |

---

### 7. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 6. (`reads` stays withheld; it is not an rc.9 row and was not re-read.)

**rc.13 bytes [M].** None of the three slices is byte-identical to rc.9's. prod differs in short (mangled) identifiers only. In dev and observe the only code line added (comments excluded) is `nameStore(store, options?.name)`, which writes `storeNames.set(proxy[$TARGET], name)` only when an attribution engine is installed (a module `WeakMap`; the `[$TARGET]` is a brand lookup).

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, createOptimisticStoreNext, declareFlight, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule, scheduleWithheld), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime. New in rc.13: `scheduleWithheld`'s `queueMicrotask(flush)` (the archive's own drain).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/next/optimistic.js | 178-310 | 8804 | 15708 | `cea4b77a869b73bcc2f8a32893b205e830ff4408c1fd2571b7dc0e647ccaa53d` | 6996..13900 `803fcb9c…` | short identifiers only |
| dist/dev.js | 8195-8344 | 375116 | 382179 | `c85e5378ff687e08f8c8ca1b1e1a8b0acd32406baae6327b8731a52edc6d3f23` | 342913..349941 `bc5b1564…` | code changed |
| dist/observe/store/next/optimistic.js | 180-317 | 8844 | 15856 | `7ace8b58aec6b4a9487bbc5d0b8be7873a77edf2bf89168baff222504ca82191` | 7025..14010 `ccd36eae…` | code changed |

---

### 8. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 2.

**rc.13 bytes [M].** All three slices (`return createProjectionNextInternal(e, t, r).store;`) are byte-identical to rc.9's. `createProjectionNextInternal` and `runProjectionComputedNext` changed (draft write withholding, `scheduleWithheld`).

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule, scheduleWithheld), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/next/projection.js | 180-182 | 7013 | 7111 | `bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7` | 7209..7307 `bcbe84ac…` | identical |
| dist/dev.js | 7958-7960 | 363258 | 363374 | `e1264ce5581a8fe3610fd0f676925fcb8f30fc2bc21983c0a5b712952b220e32` | 334649..334765 `e1264ce5…` | identical |
| dist/observe/store/next/projection.js | 189-191 | 7176 | 7274 | `bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7` | 7303..7401 `bcbe84ac…` | identical |

---

### 9. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_SIGNALS_AUDIT § 3.

**rc.13 bytes [M].** dev (`9a66667d…`) and observe (`d266035c…`) are byte-identical to rc.9. prod differs only in local names (`t`/`n` swapped).

**What changed in the closure [M].** `createOwner` now links a child through `linkChild` and, for an owner with **no parent**, calls `registerRoot(owner)` (dev `dev-shared.js:775-783`, observe `observe/core/dev.js:265-273`; prod folds it away, `prod/core/owner.js` has only `if (n) linkChild(n, i)`). `registerRoot` does `new WeakRef(owner)`, a module `WeakMap`/`Set` insert, and `rootReaper?.register(owner, ref, ref)` on a module-level `FinalizationRegistry` whose callback is `ref => liveRoots.delete(ref)`. Disposal calls `unregisterRoot` and runs disposal lists last-registered first (runtime.md item 7); `assertInvariant` only emits a dev diagnostic.

**Judgement [E], owner flag O1.** The registry is the archive's own module object; the host's only action is to call the archive's own deleter back after collection. Nothing is registered into a browser document or a server runtime, and no runtime outside the archive acts on the owner: semantic-model § creates' "a package's own private module variable is neither". So it is not a `create`, by the same reasoning rc.9 applied to the archive's `globalThis[Symbol.for(…)]` slots.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (schedule), `console` (notifyHalted); observe: `WeakRef` (registerRoot), `queueMicrotask` (schedule), `console` (notifyHalted); dev: `queueMicrotask` (emitDiagnostic, schedule), `console` (emitDiagnostic, notifyHalted, reportDiagnostic), `WeakRef` (registerRoot), `self` (disposeChildren).

**Pairing.** `createRoot` is `solid-js`' own declaration in rc.13 too, so this row does not answer a `solid-js` import.

**Verdict: GRANTED** (prod, dev, observe), resting on O1.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/owner.js | 344-347 | 14172 | 14292 | `d266035c22be8514767dbee4fe3aa0c947336ad4da585e24b09ee1722d916259` | 11782..11902 `eefc749b…` | short identifiers only |
| dist/dev-shared.js | 3478-3481 | 162084 | 162226 | `9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5` | 135728..135870 `9a66667d…` | identical |
| dist/observe/core/owner.js | 348-351 | 14337 | 14457 | `d266035c22be8514767dbee4fe3aa0c947336ad4da585e24b09ee1722d916259` | 11828..11948 `d266035c…` | identical |

---

### 10. `createSignal` — `reads`, argument scope — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 5, argument-scoped (`callable_or_primitive_slots: [0]`; scope unchanged).

**rc.13 bytes [M].** dev (`d1292c1a…`) and observe (`0366fccd…`) byte-identical; prod differs in one mangled field (`n.T` → `n.C`). `signal` and `accessor` byte-identical in dev.

**Closure [M].** prod 85→93 functions, read-entry calls none; observe 95→104 functions, read-entry calls none; dev 156→195 functions, read-entry calls none (cut: `flush,wireExternalSource,write`); setter `setMemo`: prod 33→36 functions, read-entry calls none; observe 40→44 functions, read-entry calls none; dev 44→51 functions, read-entry calls none (cut: `flush,wireExternalSource,write`) (`setMemo` adds `heldDerivation`/`rederiveHeld`: a flag test and a heap insert).

**Probes [M]** (§ 0.6): `createSignal(0)`, `createSignal(0, { equals: false })`, `createSignal(undefined)` and the function form leave the probe memo untracked; `createSignal(createdMemoAccessor)` by reference computes the created memo (the case the scope declines). 16/16 in prod, dev and observe, as on rc.9.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe). It is the delegate of § 45.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 73-81 | 3060 | 3340 | `fb6ef2e6f9d4932e0fe5d637daedf25207c1471b42357b528e783a2332b9d1e8` | 2747..3027 `ce6ade13…` | short identifiers only |
| dist/dev.js | 2346-2355 | 104746 | 105095 | `d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504` | 97659..98008 `d1292c1a…` | identical |
| dist/observe/signals.js | 75-84 | 3108 | 3422 | `0366fccdf03f70ca98cd1678d74800abeed9109c5499c65c626ee6868e6f565f` | 2795..3109 `0366fccd…` | identical |

---

### 11. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group A § 1.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Verdict: GRANTED** (prod, dev, observe). With it and § 18, the scoped `solid-js` rows' delegates are answered beside rc.13 signals.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 73-81 | 3060 | 3340 | `fb6ef2e6f9d4932e0fe5d637daedf25207c1471b42357b528e783a2332b9d1e8` | 2747..3027 `ce6ade13…` | short identifiers only |
| dist/dev.js | 2346-2355 | 104746 | 105095 | `d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504` | 97659..98008 `d1292c1a…` | identical |
| dist/observe/signals.js | 75-84 | 3108 | 3422 | `0366fccdf03f70ca98cd1678d74800abeed9109c5499c65c626ee6868e6f565f` | 2795..3109 `0366fccd…` | identical |

---

### 12. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 1.

**rc.13 bytes [M].** prod is byte-identical (`5ccb9272…`). dev and observe wrap the plain overload as `const store = createStoreNext(…); if (second?.name) nameStore(store[0], second.name); return store;` (`nameStore`: `attrHooks !== null && name` → `storeNames.set(proxy[$TARGET], name)`).

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule, scheduleWithheld), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule, scheduleWithheld), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/index.js | 21-24 | 525 | 676 | `5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4` | 525..676 `5ccb9272…` | identical |
| dist/dev.js | 8871-8878 | 403906 | 404183 | `0303d53fb85615e9c957a4d05e062951d7c3206792193c77477d67369a4c3b12` | 370517..370703 `5ab02487…` | code changed |
| dist/observe/store/index.js | 23-30 | 562 | 796 | `db153fc0bb96a41d329fe581a8ef762664921f79ac1ffcbcec2d37eb08084a44` | 551..702 `5ccb9272…` | code changed |

---

### 13. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 3.

**rc.13 bytes [M].** All three slices byte-identical (`eb5e0531…`, `59effb18…`). `trackedEffect` and `enqueueSub` byte-identical in dev; `schedule` adds the projection-draft withholding (`withheld = projectionWriteActive`), a flag.

**Closure [M].** prod 84→92 functions, read-entry calls none; observe 92→101 functions, read-entry calls none; dev 156→196 functions, read-entry calls none (cut: `flush,wireExternalSource,write`).

**Pairing [M].** Server body identical (`getOwner()` + `getNextChildId`); `nextChildIdFor` now prefixes with `materializeId(counter)` for a hole-scoped owner (an id string write).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 237-239 | 10004 | 10072 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | 9691..9759 `eb5e0531…` | identical |
| dist/dev.js | 2524-2526 | 112377 | 112463 | `59effb18726430b691223730bbeffeb8cb7c60f6b546261051389e4ee557154e` | 105290..105376 `59effb18…` | identical |
| dist/observe/signals.js | 240-242 | 10086 | 10154 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | 9773..9841 `eb5e0531…` | identical |

---

### 14. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 4.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 237-239 | 10004 | 10072 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | 9691..9759 `eb5e0531…` | identical |
| dist/dev.js | 2524-2526 | 112377 | 112463 | `59effb18726430b691223730bbeffeb8cb7c60f6b546261051389e4ee557154e` | 105290..105376 `59effb18…` | identical |
| dist/observe/signals.js | 240-242 | 10086 | 10154 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | 9773..9841 `eb5e0531…` | identical |

---

### 15. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 7.

**rc.13 bytes [M].** prod differs only in a mangled slot name (`globalQueue.Kt` → `.Jn`). observe and dev add `if (attrHooks !== null && (scheduled || activeTransition)) attrHooks.flushStart();` (**hook**, the attribution engine). The `FLUSH_IN_ACTION` guard and the `fn` branch are unchanged.

**GlobalQueue.flush, rc.13 additions [M].** `GlobalQueue._endOptimism` hoisted out of the `activeTransition` arm; `drainRearms()` calls `_rearm()` on boundaries an `on` option re-armed (`CollectionQueue` methods of boundaries `createLoadingBoundary` registered); `checkBoundaryChildren(this, true)` calls each child boundary's `_judgeHeld` → `_checkSources` (`setSignal(this._disabled, false)`, the reveal controller's `_evaluate`); `commitPendingNodes()` before the zombie heap; a `_batch._pendingNodes` test. All are boundary-queue registrants' methods (**drain**, as rc.9's `CollectionQueue._readOn`) or bookkeeping.

**Closure [M].** With rc.9's cuts and the spurious `write` edge: prod 53→59 functions, read-entry calls none; observe 60→67 functions, read-entry calls none; dev 166→218 functions, read-entry calls none (cut: `run,_evaluate,B,wireExternalSource,write`).

**Pairing [M].** Server `flush` is `function flush() {}`, identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/scheduler.js | 1295-1332 | 65269 | 66739 | `d9935f43d0d020b93711a1a2169d3938cf7f46012b47c7bbed205174d8708220` | 57644..59114 `06c46ce4…` | short identifiers only |
| dist/dev-shared.js | 2648-2740 | 125237 | 129517 | `ac0e1253703995d11781d18388bdf8c1296a8678c7ab4b944628d78c2802fa66` | 103188..107202 `b77e8fd0…` | code changed |
| dist/observe/core/scheduler.js | 1306-1357 | 65893 | 68238 | `15af13372a3ea04723f667a2664dd97d538363276f5a1b0ea67533090d165a56` | 58268..60335 `4c7dcc9d…` | code changed |

---

### 16. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 8.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `self` (createEffectNode, disposeChildren, setupComputedNode), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime. `haltReactivity`'s `globalThis.reportError` is rc.9's ruling (it reports an error and registers nothing).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/scheduler.js | 1295-1332 | 65269 | 66739 | `d9935f43d0d020b93711a1a2169d3938cf7f46012b47c7bbed205174d8708220` | 57644..59114 `06c46ce4…` | short identifiers only |
| dist/dev-shared.js | 2648-2740 | 125237 | 129517 | `ac0e1253703995d11781d18388bdf8c1296a8678c7ab4b944628d78c2802fa66` | 103188..107202 `b77e8fd0…` | code changed |
| dist/observe/core/scheduler.js | 1306-1357 | 65893 | 68238 | `15af13372a3ea04723f667a2664dd97d538363276f5a1b0ea67533090d165a56` | 58268..60335 `4c7dcc9d…` | code changed |

---

### 17. `getOwner` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 1. **Carried on byte identity [M]:** prod/observe `67fcbebd…`, dev `e8cb95b7…`; the closure is the function alone (no helper). Server `getOwner` identical. Probe: `getOwner()` leaves the probe memo untracked in all three builds (§ 0.6).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/owner.js | 239-241 | 10593 | 10641 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | 8615..8663 `67fcbebd…` | identical |
| dist/dev-shared.js | 3358-3360 | 157883 | 157925 | `e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb` | 131908..131950 `e8cb95b7…` | identical |
| dist/observe/core/owner.js | 242-244 | 10710 | 10758 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | 8635..8683 `67fcbebd…` | identical |

---

### 18. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_SIGNALS_AUDIT § 1. **Carried on byte identity [M]** of all three slices and of the server body; the function calls nothing.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/owner.js | 239-241 | 10593 | 10641 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | 8615..8663 `67fcbebd…` | identical |
| dist/dev-shared.js | 3358-3360 | 157883 | 157925 | `e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb` | 131908..131950 `e8cb95b7…` | identical |
| dist/observe/core/owner.js | 242-244 | 10710 | 10758 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | 8635..8683 `67fcbebd…` | identical |

---

### 19. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_MERGE_OMIT_MEMO_AUDIT § 1.

**rc.13 bytes [M].** All three slices changed. Two changes: the proxy classification reads one brand, `recordOf(props)` = `props[$RECORD]`, instead of `[$TARGET]`/`[$OMIT]`/`[$VIEW]`; and the non-`Proxy` copy path re-homes an accessor descriptor with `get: desc.get && desc.get.bind(props)` instead of copying it (the getter is bound, not invoked).

**Walk [M]** (§ 0.4, per-file top-level walk): rc.13 reaches 42 definitions in each minified `store/utils.js` and 48 in `dist/dev.js` (rc.9: 45 and 50 by the same tool). New: `recordOf`, `$RECORD`; gone: `$OMIT`, `$VIEW`, `$SOURCES`, `isView`; changed: `omitTraps`, `mergeLookup`, `sourceDescriptor` (brand tests and a plain-source fast path). Imports used: `SUPPORTS_PROXY`, `$PROXY`, `$RECORD`, `$TARGET`, `ownEnumerableKeys`. Free globals: `Map`, `Object`, `Proxy`, `Reflect`, `Set`, `Symbol`, `undefined` (the rc.9 set). No scheduler, owner, memo, signal or host function is reachable, traps included.

**Pairing [M].** All six `solid-js@2.0.0-rc.13` builds re-export `omit` from `@solidjs/signals` (`dist/solid.js:2`, `dist/server.js:2`, siblings).

**Verdict: GRANTED** (prod, dev, observe, and every `solid-js` host).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/utils.js | 1019-1066 | 43043 | 45186 | `32992411ce661ee0c6948183ffcb32ff660c6d4796c5e6d789822a9b06ef6833` | 40808..42411 `8b9a50f9…` | code changed |
| dist/dev.js | 4549-4605 | 201577 | 203849 | `c30db00b1292f96c94704743f877848501dfee2ece0a6fda87f6f41ce0c4b326` | 192223..194027 `578fbc14…` | code changed |
| dist/observe/store/utils.js | 1021-1068 | 43069 | 45212 | `32992411ce661ee0c6948183ffcb32ff660c6d4796c5e6d789822a9b06ef6833` | 40834..42437 `8b9a50f9…` | code changed |

---

### 20. `onCleanup` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 2.

**rc.13 bytes [M].** All three slices byte-identical (`89ddda30…`, dev `5f8d40b1…`). The dev closure is the same eight functions as rc.9's; `emitDiagnostic` (listeners now receive the live subject as a second argument) and `isExcluded` (a `markedOwners` map instead of `excludedOwners`) changed, `cleanup`, `getOwner`, `reportDiagnostic`, `takeFooter`, `ownerPath` are identical. prod 2→2 functions, read-entry calls none; observe 2→2 functions, read-entry calls none; dev 8→8 functions, read-entry calls none (cut: `flush`). Server body identical. Probe: untracked in all builds.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 63-65 | 2911 | 2964 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | 2598..2651 `89ddda30…` | identical |
| dist/dev.js | 2311-2340 | 103785 | 104646 | `5f8d40b1c3165f17bc00846b5063eb55f8bdd0a6b49dceba4a60f5aeb6570e44` | 96698..97559 `5f8d40b1…` | identical |
| dist/observe/signals.js | 65-67 | 2959 | 3012 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | 2646..2699 `89ddda30…` | identical |

---

### 21. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_SIGNALS_AUDIT § 2. Slices byte-identical [M]; dev closure as § 20. host references reached, rc.13 — prod: none; observe: none; dev: `console` (emitDiagnostic, reportDiagnostic), `queueMicrotask` (emitDiagnostic). Server body identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 63-65 | 2911 | 2964 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | 2598..2651 `89ddda30…` | identical |
| dist/dev.js | 2311-2340 | 103785 | 104646 | `5f8d40b1c3165f17bc00846b5063eb55f8bdd0a6b49dceba4a60f5aeb6570e44` | 96698..97559 `5f8d40b1…` | identical |
| dist/observe/signals.js | 65-67 | 2959 | 3012 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | 2646..2699 `89ddda30…` | identical |

---

### 22. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 5.

**rc.13 bytes [M].** dev byte-identical (`add35758…`). prod and observe differ only in mangled field names (`t.T` → `t.C`; `dirtyQueue.et`/`.Ct` → `.ln`/`.st`).

**Closure [M].** prod 85→93 functions, read-entry calls none; observe 93→102 functions, read-entry calls none; dev 157→196 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). Server body identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 706-723 | 31224 | 32392 | `f1e430679468af832adf704b7f90a6e31fc0ea1ac6f007df0605e28cc4edce12` | 30911..32079 `107240ae…` | short identifiers only |
| dist/dev.js | 3031-3061 | 134361 | 136156 | `add35758c224c4e5339e7e89db7000bce0b2b92ba37a28df5042414a2719547e` | 127274..129069 `add35758…` | identical |
| dist/observe/signals.js | 710-729 | 31340 | 32532 | `8461deec634a0716375424e0b8bf2fca348f48f8ffabaf62bea5812907acd29f` | 31027..32219 `7535e81c…` | short identifiers only |

---

### 23. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group B § 6. host references reached, rc.13 — prod: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (enqueue, schedule), `console` (notifyHalted, reportClientError); dev: `queueMicrotask` (checkPostAwaitRead, emitDiagnostic, enqueue, schedule), `console` (emitDiagnostic, haltReactivity, notifyHalted, reportClientError, reportDiagnostic, runEffect), `self` (computed, createEffectNode, disposeChildren, setupComputedNode), `Promise` (resolve, watchAsyncTail), `WeakRef` (registerRoot, watchAsyncTail), `globalThis` (haltReactivity). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/signals.js | 706-723 | 31224 | 32392 | `f1e430679468af832adf704b7f90a6e31fc0ea1ac6f007df0605e28cc4edce12` | 30911..32079 `107240ae…` | short identifiers only |
| dist/dev.js | 3031-3061 | 134361 | 136156 | `add35758c224c4e5339e7e89db7000bce0b2b92ba37a28df5042414a2719547e` | 127274..129069 `add35758…` | identical |
| dist/observe/signals.js | 710-729 | 31340 | 32532 | `8461deec634a0716375424e0b8bf2fca348f48f8ffabaf62bea5812907acd29f` | 31027..32219 `7535e81c…` | short identifiers only |

---

### 24. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.13` — **WITHHOLD**

**rc.9 basis.** RC9_PARITY_AUDIT group C § 7. That grant rested on two findings: (a) "the call graph from `reconcileNextState` reaches no read-path call" in any build, and (b) every proxy it can touch is parameter-rooted. Group C § 0 draws the line: "A read is either an engine read-path call (`read`/`readNodeFast`), or a … trap on a store … the export created".

**rc.13 bytes [M].** The three `reconcile` slices are byte-identical (`d99b5e91…`, dev `572cd064…`), but the application closure changed. `applyAdopt` (prod `store/next/reconcile.js:102`, dev `dist/dev.js` `applyAdopt`) still computes `optHooks.optimisticView(t, prev)` for an optimistic family (`fam?.opt === true`), with `draft` defaulted to `false`. rc.13's `optimisticView` now sets `const reader = !draft && !authoritativeServe()` and, for each node with a visible override, takes `readerOverride(node, …)` instead of the raw `_overrideValue`. `readerOverride` returns `nodeValue(node, committed)` when the node carries `CONFIG_OVERRIDE_SUPERSEDED`, and `nodeValue` calls `serve(node, readerContext(), …)`: the selection core of `read` (it may `enterStagedRead`, `throw new NotReadyError`, and record into an `isPending` probe). rc.9's census already counts `nodeValue → serve` as a read path (parity audit § 0.4).

**Closure [M].** prod 93→120 functions, read-entry calls nodeValue->serve; observe 102→132 functions, read-entry calls nodeValue->serve; dev 106→139 functions, read-entry calls nodeValue->serve (cut: `flush,wireExternalSource,write`). The path is `reconcileNextState → applyAdopt → optimisticView → readerOverride → nodeValue → serve` in all three builds, with no spurious edge on it (`optHooks.optimisticView` is `optimisticView`: prod `store/next/optimistic.js:71`, dev `dist/dev.js:8084`).

**Why withheld.** The reach is guarded (optimistic store, visible superseded override, `!authoritativeServe()`, `!authoritativeRead()`), and a guarded reach still counts. The dev comment on `optimisticView` says applyAdopt's key-matching view "keeps the override itself", which would hold only if every application runs under `authoritativeServe()` (`projectionWriteActive || getWriteOverride() || authoritativeRead()`). This reading did not establish that for a caller's optimistic-store setter applying `reconcile(...)` [E]. Separately, the node read belongs to the store the caller handed in. rc.9's sign-off excluded "reads of the store handed to that function", but the model's receiver carve-out (Decision 2026-09-10) covers a *property access* on a caller-supplied receiver, not the archive's own read path observing a node of that receiver.

**What would grant it.** Either (1) a reading that proves every `applyAdopt` call on an optimistic family runs with `authoritativeServe()` true, so `readerOverride` is unreachable, or (2) an owner ruling that an engine read of a node inside a caller-supplied store is the caller's read. Owner flag O2.

**Verdict: WITHHELD** on rc.13. No citation is carried.

**rc.13 definition per cited build** (subject of the reading; not carried as citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/index.js | 26-28 | 678 | 758 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | 678..758 `d99b5e91…` | identical |
| dist/dev.js | 8879-8881 | 404184 | 404282 | `572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c` | 370704..370802 `572cd064…` | identical |
| dist/observe/store/index.js | 32-34 | 798 | 878 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | 704..784 `d99b5e91…` | identical |

---

### 25. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group C § 8.

**rc.13 bytes [M].** Slices byte-identical. The § 24 `serve` path can `enterStagedRead` → `globalQueue.initTransition` → `schedule()`: the archive's own flush microtask, the same class rc.9 D3 recorded for `snapshot`.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (schedule), `console` (notifyHalted, reportClientError); dev: `queueMicrotask` (emitDiagnostic, schedule), `console` (emitDiagnostic, notifyHalted, reportClientError, reportDiagnostic), `self` (disposeChildren). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Pairing [M].** Server `reconcile` identical (`setProperty` on the caller's `state`).

**Verdict: GRANTED** (prod, dev, observe, and the `solid-js` server path).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/index.js | 26-28 | 678 | 758 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | 678..758 `d99b5e91…` | identical |
| dist/dev.js | 8879-8881 | 404184 | 404282 | `572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c` | 370704..370802 `572cd064…` | identical |
| dist/observe/store/index.js | 32-34 | 798 | 878 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | 704..784 `d99b5e91…` | identical |

---

### 26. `runWithOwner` — `reads`, argument scope — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 3, argument-scoped (`invoked_slots: [1]`; scope unchanged).

**rc.13 bytes [M].** dev (`332a218c…`) and observe (`5c40363e…`) byte-identical; prod differs only in local names. Dev closure: the same six functions (`emitDiagnostic`, `isExcluded` changed as § 20). prod 1→1 functions, read-entry calls none; observe 1→1 functions, read-entry calls none; dev 6→6 functions, read-entry calls none (cut: `flush`). Server body identical.

**Probes [M].** `runWithOwner(owner, () => s())` and `runWithOwner(null, () => 1)` leave the memo untracked; `runWithOwner(owner, createdMemoAccessor)` by reference computes the created memo once (the case the scope declines). All three builds.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/core.js | 1902-1913 | 102384 | 102596 | `edcf5f33372c979731c0d5ce2f7bddf0aff822d92b11ae75af6178a000a482a0` | 87558..87770 `5c40363e…` | short identifiers only |
| dist/dev-shared.js | 6919-6947 | 331544 | 332252 | `332a218ceec024a1d0c3464213b7d043d4a902c529b5f02a7d6ae0467070a11c` | 287156..287864 `332a218c…` | identical |
| dist/observe/core/core.js | 1940-1951 | 104299 | 104511 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | 89058..89270 `5c40363e…` | identical |

---

### 27. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_SIGNALS_AUDIT § 5. Slices as § 26 [M]. host references reached, rc.13 — prod: none; observe: none; dev: `console` (emitDiagnostic, reportDiagnostic), `queueMicrotask` (emitDiagnostic). Server body identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/core.js | 1902-1913 | 102384 | 102596 | `edcf5f33372c979731c0d5ce2f7bddf0aff822d92b11ae75af6178a000a482a0` | 87558..87770 `5c40363e…` | short identifiers only |
| dist/dev-shared.js | 6919-6947 | 331544 | 332252 | `332a218ceec024a1d0c3464213b7d043d4a902c529b5f02a7d6ae0467070a11c` | 287156..287864 `332a218c…` | identical |
| dist/observe/core/core.js | 1940-1951 | 104299 | 104511 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | 89058..89270 `5c40363e…` | identical |

---

### 28. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_PARITY_AUDIT group D § 3.

**rc.13 bytes [M].** All three slices byte-identical (`3bb31686…`, dev `eebc4206…`). `snapshotWalk` and `pendingBackingVisible` changed. Through `optimisticView`'s new reader path (§ 24) a snapshot of an optimistic store can now reach `serve` → `enterStagedRead` → `initTransition` → `schedule()`, which is the rc.9 D3 class: the archive's own flush microtask.

**Closure [M].** host references reached, rc.13 — prod: `queueMicrotask` (schedule), `console` (notifyHalted, reportClientError); observe: `queueMicrotask` (schedule), `console` (notifyHalted, reportClientError); dev: `queueMicrotask` (emitDiagnostic, schedule), `console` (emitDiagnostic, notifyHalted, reportClientError, reportDiagnostic), `self` (disposeChildren). The closure reaches only archive code, builtins, the caller's callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.

**Pairing [M].** All three `solid-js` server builds re-export `snapshot` from `@solidjs/signals` (`dist/server.js:2`).

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/store/index.js | 30-32 | 760 | 813 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` | 760..813 `3bb31686…` | identical |
| dist/dev.js | 8882-8884 | 404283 | 404342 | `eebc4206bc3509952fd02519f13f9b58ed00bc63da5658c90322c97bd221d6be` | 370803..370862 `eebc4206…` | identical |
| dist/observe/store/index.js | 36-38 | 880 | 933 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` | 786..839 `3bb31686…` | identical |

---

### 29. `untrack` — `reads`, argument scope — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 4, argument-scoped (`invoked_slots: [0]`; scope unchanged).

**rc.13 bytes [M].** prod and observe differ only in the mangled hook-slot name (`GlobalQueue.Yt`/`.In` → `.jn`/`.Jt`) and local names. dev adds `asyncTailFlights === 0` to the fast-path test and `untrackDepth++`/`--` around `fn` (a module counter the dev `UNTRACKED_READ_AFTER_AWAIT` check reads). The hook slot `GlobalQueue._externalUntrack` is still installed only by `enableExternalSource` (`dist/dev.js:236`; `externalUntrack` byte-identical). prod 1→1 functions, read-entry calls none; observe 1→1 functions, read-entry calls none; dev 1→1 functions, read-entry calls none (cut: `flush`). Server body identical (`return fn();`).

**Probes [M].** `untrack(() => s())` and `untrack(() => 1)` untracked; `untrack(createdMemoAccessor)` by reference and the literal `untrack(() => createdMemoAccessor())` compute the created memo, as on rc.9. All three builds.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/core.js | 1082-1092 | 56152 | 56432 | `0fd615f97eec22ed61c5e13caadfcffa01e1a9db357577116d899559201da786` | 49018..49298 `28c2d9f3…` | short identifiers only |
| dist/dev-shared.js | 5863-5889 | 278820 | 279412 | `a32fa71b267c22f271dfcb5ae86f7502dceca97942230649833183141d382b4d` | 242868..243344 `2a929c26…` | code changed |
| dist/observe/core/core.js | 1118-1128 | 57931 | 58211 | `2dadaf6243bbfde3d0ae1f249d570bf7bc8f60d3ab744db34483488acebbc76b` | 50382..50662 `01672a8c…` | short identifiers only |

---

### 30. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.13`

**rc.9 basis.** RC9_SIGNALS_AUDIT § 4. Slices as § 29 [M]; the closure is the function alone in every build. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/prod/core/core.js | 1082-1092 | 56152 | 56432 | `0fd615f97eec22ed61c5e13caadfcffa01e1a9db357577116d899559201da786` | 49018..49298 `28c2d9f3…` | short identifiers only |
| dist/dev-shared.js | 5863-5889 | 278820 | 279412 | `a32fa71b267c22f271dfcb5ae86f7502dceca97942230649833183141d382b4d` | 242868..243344 `2a929c26…` | code changed |
| dist/observe/core/core.js | 1118-1128 | 57931 | 58211 | `2dadaf6243bbfde3d0ae1f249d570bf7bc8f60d3ab744db34483488acebbc76b` | 50382..50662 `01672a8c…` | short identifiers only |

---

### 31. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 15 (returned-value line, flag F3).

**rc.13 bytes [M].** All six slices byte-identical (client `f6412c9a…`, server `31fd1a14…`). Client walk: `clientOnly` and `loadClientOnly`, both identical. At the call, `createSignal()` (no argument) is `solid-js@2.0.0-rc.13`'s wrapper (identical) → `hydratedCreateSignal` (identical; a non-function argument goes straight to signals) → signals `createSignal` plain overload (§ 10). Server body `return props => {…}` invokes nothing.

**Verdict: GRANTED**, under F3.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2175-2196 | 77253 | 78091 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 75603..76441 `f6412c9a…` | identical |
| dist/web.dev.js | 2417-2438 | 88866 | 89704 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 83888..84726 `f6412c9a…` | identical |
| dist/web.observe.js | 2223-2244 | 78976 | 79814 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 76567..77405 `f6412c9a…` | identical |
| dist/server.js | 4390-4395 | 147221 | 147402 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 128372..128553 `31fd1a14…` | identical |
| dist/server.dev.js | 4815-4820 | 166831 | 167012 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 139370..139551 `31fd1a14…` | identical |
| dist/server.observe.js | 4554-4559 | 152298 | 152479 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 131399..131580 `31fd1a14…` | identical |

---

### 32. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 16. Same bytes and closure as § 31 [M]; the signals plain path is bounded by § 0.3.

**Verdict: GRANTED**, under F3.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2175-2196 | 77253 | 78091 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 75603..76441 `f6412c9a…` | identical |
| dist/web.dev.js | 2417-2438 | 88866 | 89704 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 83888..84726 `f6412c9a…` | identical |
| dist/web.observe.js | 2223-2244 | 78976 | 79814 | `f6412c9af0fb9a131456f7aa9bb9e4e7b372a6ccd58038a058e30787fe8d572b` | 76567..77405 `f6412c9a…` | identical |
| dist/server.js | 4390-4395 | 147221 | 147402 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 128372..128553 `31fd1a14…` | identical |
| dist/server.dev.js | 4815-4820 | 166831 | 167012 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 139370..139551 `31fd1a14…` | identical |
| dist/server.observe.js | 4554-4559 | 152298 | 152479 | `31fd1a14ca31fa876e0af88c56d8b5c397fefd7f8ef84133d9484d5721ad1061` | 131399..131580 `31fd1a14…` | identical |

---

### 33. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 17.

**rc.13 bytes [M].** All six slices byte-identical (client `cd98272f…`, server `3d0445e2…`). Server walk: `headerLedgers`, `RequestContext` identical; `getRequestEvent` changed: after `globalThis[RequestContext].getStore()` it now falls back to `renderContextOf(getOwner())`, a lookup of `globalThis[Symbol.for("@solidjs/web/render-roots")]` (a `WeakMap` a renderer populates) along the owner chain, then `ctx.event`. `getOwner` and `onCleanup` are `solid-js` server bodies, identical. No reactive source is observed.

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2198-2198 | 78129 | 78176 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 76479..76526 `cd98272f…` | identical |
| dist/web.dev.js | 2440-2440 | 89742 | 89789 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 84764..84811 `cd98272f…` | identical |
| dist/web.observe.js | 2246-2246 | 79852 | 79899 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 77443..77490 `cd98272f…` | identical |
| dist/server.js | 4432-4472 | 148537 | 149937 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 129688..131088 `3d0445e2…` | identical |
| dist/server.dev.js | 4857-4897 | 168147 | 169547 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 140686..142086 `3d0445e2…` | identical |
| dist/server.observe.js | 4596-4636 | 153614 | 155014 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 132715..134115 `3d0445e2…` | identical |

---

### 34. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 18 and its owner flag ([Decision 2026-09-04]'s headline sentence vs its four terms). Bytes and closure as § 33 [M]; `renderContextOf` only reads.

**Verdict: GRANTED**, with the rc.9 owner flag.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2198-2198 | 78129 | 78176 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 76479..76526 `cd98272f…` | identical |
| dist/web.dev.js | 2440-2440 | 89742 | 89789 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 84764..84811 `cd98272f…` | identical |
| dist/web.observe.js | 2246-2246 | 79852 | 79899 | `cd98272f0f010dfe4990526fc6ba5a3508aea05b6f10233e5747ecd803b76e42` | 77443..77490 `cd98272f…` | identical |
| dist/server.js | 4432-4472 | 148537 | 149937 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 129688..131088 `3d0445e2…` | identical |
| dist/server.dev.js | 4857-4897 | 168147 | 169547 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 140686..142086 `3d0445e2…` | identical |
| dist/server.observe.js | 4596-4636 | 153614 | 155014 | `3d0445e29f9bdff71a0501f0a5fff30a15073ae3955a0519708d97324c004f20` | 132715..134115 `3d0445e2…` | identical |

---

### 35. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 19. All six slices byte-identical (client `9de7ebfe…`, server `56233798…`) [M]; closure as § 33 (`statusLedgers` identical, `getRequestEvent` as there).

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2197-2197 | 78092 | 78128 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 76442..76478 `9de7ebfe…` | identical |
| dist/web.dev.js | 2439-2439 | 89705 | 89741 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 84727..84763 `9de7ebfe…` | identical |
| dist/web.observe.js | 2245-2245 | 79815 | 79851 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 77406..77442 `9de7ebfe…` | identical |
| dist/server.js | 4398-4431 | 147507 | 148536 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 128658..129687 `56233798…` | identical |
| dist/server.dev.js | 4823-4856 | 167117 | 168146 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 139656..140685 `56233798…` | identical |
| dist/server.observe.js | 4562-4595 | 152584 | 153613 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 131685..132714 `56233798…` | identical |

---

### 36. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 20. As § 34.

**Verdict: GRANTED**, with the rc.9 owner flag.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/web.js | 2197-2197 | 78092 | 78128 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 76442..76478 `9de7ebfe…` | identical |
| dist/web.dev.js | 2439-2439 | 89705 | 89741 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 84727..84763 `9de7ebfe…` | identical |
| dist/web.observe.js | 2245-2245 | 79815 | 79851 | `9de7ebfe186dfaf587fcaf60460297678b12e25a81e7c1221a574094397c926b` | 77406..77442 `9de7ebfe…` | identical |
| dist/server.js | 4398-4431 | 147507 | 148536 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 128658..129687 `56233798…` | identical |
| dist/server.dev.js | 4823-4856 | 167117 | 168146 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 139656..140685 `56233798…` | identical |
| dist/server.observe.js | 4562-4595 | 152584 | 153613 | `56233798652c4cdbb935eecde9e930996c50b7a747a4da9b37e3ec4dc19183e7` | 131685..132714 `56233798…` | identical |

---

### 37. `For` — `reads` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 1 (flags F1–F3).

**rc.13 bytes [M].** All six slices byte-identical (client lazy-`list` body; server `36dc66a0…`). Client imports used: `getOwner`, `runWithOwner`, `mapArray` (the rc.9 set). Signals `mapArray` is byte-identical in dev; `updateKeyedMap` now writes its row and index signals through `write`, which is `setSignal` or, under a live optimistic lane, `GlobalQueue._landOnOverride` (writes; its dev closure reaches no read entry point). Closure from `mapArray`: prod 84→92 functions, read-entry calls none; observe 92→102 functions, read-entry calls none; dev 156→194 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). Server `mapArray` changes `rowOwner.id` to `ownerId(rowOwner)` (an id field; dev throws on an unmaterialized hole-scoped owner) and still calls `createMemo(…, { sync: true })` → `createSyncMemo` (identical).

**Verdict: GRANTED**, under F1–F3.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1386-1399 | 43257 | 43682 | `3f85ecafbe4729ce5c20deac5622cf43aed0153bf0154f5b1fa27513f3aeb065` | 37042..37467 `3f85ecaf…` | identical |
| dist/solid.dev.js | 1439-1453 | 46422 | 46873 | `eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5` | 39271..39722 `eabb480f…` | identical |
| dist/solid.observe.js | 1413-1427 | 44079 | 44530 | `eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5` | 37372..37823 `eabb480f…` | identical |
| dist/server.js | 2344-2352 | 72325 | 72547 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 65074..65296 `36dc66a0…` | identical |
| dist/server.dev.js | 2609-2617 | 83707 | 83929 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 74377..74599 `36dc66a0…` | identical |
| dist/server.observe.js | 2498-2506 | 77301 | 77523 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 69358..69580 `36dc66a0…` | identical |

---

### 38. `For` — `creates` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 2. Bytes as § 37 [M]. Client callees are signals code bounded by § 0.3. Server: `createSyncMemo`, `createOwner`, `runWithOwner`, `formatChildId`, `stampThrower` identical; `ownerId`/`materializeId` are id arithmetic; no `processResult` or `ctx.serialize` on the path [M].

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1386-1399 | 43257 | 43682 | `3f85ecafbe4729ce5c20deac5622cf43aed0153bf0154f5b1fa27513f3aeb065` | 37042..37467 `3f85ecaf…` | identical |
| dist/solid.dev.js | 1439-1453 | 46422 | 46873 | `eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5` | 39271..39722 `eabb480f…` | identical |
| dist/solid.observe.js | 1413-1427 | 44079 | 44530 | `eabb480fb722cb1bf1aec173113ad7c3b152476d54c436dd6b04f37ff59441b5` | 37372..37823 `eabb480f…` | identical |
| dist/server.js | 2344-2352 | 72325 | 72547 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 65074..65296 `36dc66a0…` | identical |
| dist/server.dev.js | 2609-2617 | 83707 | 83929 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 74377..74599 `36dc66a0…` | identical |
| dist/server.observe.js | 2498-2506 | 77301 | 77523 | `36dc66a015203248b9aa9eef6309f2386d8a3b74406d85a925cd123e0d2da223` | 69358..69580 `36dc66a0…` | identical |

---

### 39. `Match` — `reads` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 5. **Carried on byte identity [M]** (`54f4236b…` in all six); no call, no property access.

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1469-1471 | 45973 | 46014 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 39758..39799 `54f4236b…` | identical |
| dist/solid.dev.js | 1533-1535 | 49388 | 49429 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 42212..42253 `54f4236b…` | identical |
| dist/solid.observe.js | 1507-1509 | 47035 | 47076 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 40303..40344 `54f4236b…` | identical |
| dist/server.js | 2415-2417 | 74357 | 74398 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 67106..67147 `54f4236b…` | identical |
| dist/server.dev.js | 2680-2682 | 85739 | 85780 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 76409..76450 `54f4236b…` | identical |
| dist/server.observe.js | 2569-2571 | 79333 | 79374 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 71390..71431 `54f4236b…` | identical |

---

### 40. `Match` — `creates` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 6. As § 39 [M].

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1469-1471 | 45973 | 46014 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 39758..39799 `54f4236b…` | identical |
| dist/solid.dev.js | 1533-1535 | 49388 | 49429 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 42212..42253 `54f4236b…` | identical |
| dist/solid.observe.js | 1507-1509 | 47035 | 47076 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 40303..40344 `54f4236b…` | identical |
| dist/server.js | 2415-2417 | 74357 | 74398 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 67106..67147 `54f4236b…` | identical |
| dist/server.dev.js | 2680-2682 | 85739 | 85780 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 76409..76450 `54f4236b…` | identical |
| dist/server.observe.js | 2569-2571 | 79333 | 79374 | `54f4236b8d126e70e15e5dea7b0afd6ed62ce73f4a95d4dd8fa9dd0da056920f` | 71390..71431 `54f4236b…` | identical |

---

### 41. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 3.

**rc.13 bytes [M].** All six slices byte-identical (`540fa67e…`, dev/observe `c9b26a45…`). Signals `repeat` and `updateRepeat` byte-identical in dev; closure from `repeat`: prod 84→92 functions, read-entry calls none; observe 92→102 functions, read-entry calls none; dev 156→194 functions, read-entry calls none (cut: `flush,wireExternalSource,write`). Server `repeat`: `ownerId(rowOwner)` as § 37.

**Verdict: GRANTED**, under F1.

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1400-1406 | 43683 | 43964 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 37468..37749 `540fa67e…` | identical |
| dist/solid.dev.js | 1454-1461 | 46874 | 47184 | `c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95` | 39723..40033 `c9b26a45…` | identical |
| dist/solid.observe.js | 1428-1435 | 44531 | 44841 | `c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95` | 37824..38134 `c9b26a45…` | identical |
| dist/server.js | 2353-2359 | 72548 | 72829 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 65297..65578 `540fa67e…` | identical |
| dist/server.dev.js | 2618-2624 | 83930 | 84211 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 74600..74881 `540fa67e…` | identical |
| dist/server.observe.js | 2507-2513 | 77524 | 77805 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 69581..69862 `540fa67e…` | identical |

---

### 42. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 4. As § 38 [M].

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 1400-1406 | 43683 | 43964 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 37468..37749 `540fa67e…` | identical |
| dist/solid.dev.js | 1454-1461 | 46874 | 47184 | `c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95` | 39723..40033 `c9b26a45…` | identical |
| dist/solid.observe.js | 1428-1435 | 44531 | 44841 | `c9b26a452e05bdb29c555e798b72a155d0eeee2c0d21d3df3025cd70647a1a95` | 37824..38134 `c9b26a45…` | identical |
| dist/server.js | 2353-2359 | 72548 | 72829 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 65297..65578 `540fa67e…` | identical |
| dist/server.dev.js | 2618-2624 | 83930 | 84211 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 74600..74881 `540fa67e…` | identical |
| dist/server.observe.js | 2507-2513 | 77524 | 77805 | `540fa67ec55df13ad8823dae14825bbf275c5c3697d4d3b8b3f516c8aa14d66a` | 69581..69862 `540fa67e…` | identical |

---

### 43. `createContext` — `creates` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 10. **Carried on byte identity [M]**: client `c3be410a…`, server `87cfe46a…` in all six builds. The transitive call table at the call event has one row, `Symbol(...)`. `provider` (whose body reaches `createRoot$1`, `setContext`, `children`) runs only when a consumer renders it.

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 6-17 | 1295 | 1634 | `c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f` | 1295..1634 `c3be410a…` | identical |
| dist/solid.dev.js | 5-16 | 1342 | 1681 | `c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f` | 1324..1663 `c3be410a…` | identical |
| dist/solid.observe.js | 6-17 | 1317 | 1656 | `c3be410a00e4ddc5fd5150138223a341e6280d8489dc94d2e6ac6560665d899f` | 1317..1656 `c3be410a…` | identical |
| dist/server.js | 1834-1845 | 56336 | 56673 | `87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829` | 49988..50325 `87cfe46a…` | identical |
| dist/server.dev.js | 1982-1993 | 62593 | 62930 | `87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829` | 54369..54706 `87cfe46a…` | identical |
| dist/server.observe.js | 1924-1935 | 58894 | 59231 | `87cfe46a740a3e2a7e4a35a9df518de40c49d5156a36c310f20a29f120722829` | 52107..52444 `87cfe46a…` | identical |

---

### 44. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_MERGE_OMIT_MEMO_AUDIT § 4 (§ 13 of the core-and-web audit's dispositions).

**rc.13 bytes [M].** The wrapper `const createMemo = (...args) => (_createMemo || createMemo$1)(...args);` is byte-identical in the three browser builds (`1528dcd8…`); `_createMemo` is still written only by `enableHydration` (`dist/solid.js:910`). `hydratedCreateMemo` dropped its `options?.transparent` test, which moved into `hydrateSignalLike`'s new first line `if (options?.transparent || noHydrationId()) return coreFn(fn, options);`.

**Walk [M].** From `createMemo`, `hydratedCreateMemo` and `_createMemo`: 40 definitions (rc.9: 27), every one byte-identical across `solid.js`, `solid.dev.js` and `solid.observe.js`. Changed: `hydrateSignalLike`, `hydratedCreateMemo`, `readSerializedOrCompute`, `armLiveTakeover` (per-owner gates in a `WeakMap`/`Map`), `subFetch` (iterator test), `markTopLevelSnapshotScope` (adds `openLiveScope`). New: `noHydrationId`, `adoptedAnswerStream`, `takeOver`, `wrapFirstYield`, `liveScopeOf`, `openLiveScope`, `isClaiming`, `_claimOwner`, `nodeGate`, `liveGates`, `openScopes`, `TAKEN`, `LIVE_LOCAL`, `LIVE_RESUME_FROM`. Gone: `liveGate`. Imports used are exactly rc.9's: `createMemo$1`, `createSignal$1`, `getOwner`, `peekNextChildId`, `markSnapshotScope`. Free globals: `window`/`fetch`/`Promise` (`subFetch`, the swap restored in `finally`), `Promise.resolve` (`adoptedAnswerStream`, `normalizeIterator`), `queueMicrotask` (`onHydrationEnd`, and new `queueMicrotask(flip)` in the hybrid branch, a write of the gate signal), `Symbol`, `WeakMap`, `WeakSet`, `Map`, `Set`. `sharedConfig.has`/`.load` are still `@solidjs/web`'s hooks over `globalThis._$HY.r` (`web.js:1371-1372`). No `document`, no `_$HY` write, no `addEventListener`, no `setTimeout` is reachable.

**No create.** Same dispositions as rc.9: delegate calls, host reads, a restored swap, microtasks, module state.

**Scope.** Condition `browser`; runtime `dist/solid.js`, `dist/solid.dev.js`, `dist/solid.observe.js` (rc.13's `exports["."]` is identical to rc.9's); delegates `@solidjs/signals` `createMemo`, `createSignal`, `getOwner` `creates`, all granted here on rc.13 (§ 4, § 11, § 18). `solid-js@2.0.0-rc.13` depends on `@solidjs/signals: ^2.0.0-rc.13`.

**Verdict: GRANTED, scoped to `browser`.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 977-979 | 29607 | 29692 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` | 23820..23905 `1528dcd8…` | identical |
| dist/solid.dev.js | 1014-1016 | 31810 | 31895 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` | 25516..25601 `1528dcd8…` | identical |
| dist/solid.observe.js | 989-991 | 29927 | 30012 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` | 24140..24225 `1528dcd8…` | identical |

---

### 45. `createSignal` — `reads`, argument scope — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 6 (`primitive_slots: [0]`, delegate signals `createSignal` `reads`; scope unchanged).

**rc.13 bytes [M].** Client wrapper slices byte-identical (`96473897…`); `hydratedCreateSignal` byte-identical (`if (typeof fn !== "function" || !sharedConfig.hydrating) return createSignal$1(fn, second);`). Server `createSignal` byte-identical in all three (`38028a6c…`, dev `6002fc5a…`): a non-function first argument returns two closures and calls nothing. The function path (server `createMemo`, client `hydrateSignalLike`) is excluded by the scope.

**Probes [M].** Client builds: `createSignal(5)` leaves the memo untracked with hydration off and on, and reads back `5` under `ssrSource: "client"` (5/5 primitive-path checks per build). The function-path contrast (`createSignal(fn, { ssrSource: "client" })` under `sharedConfig.hydrating`) now runs `fn` (1, rc.9: 0) because rc.13's `hydrateSignalLike` returns `coreFn(fn, options)` when the owner has no hydration id (`noHydrationId()`), which is the probe's situation. That is the excluded path; it does not bear on the row. Server builds 2/2.

**Delegate.** `(@solidjs/signals@2.0.0-rc.13, createSignal, Reads)`, § 10.

**Verdict: GRANTED, argument-scoped.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 980-982 | 29693 | 29784 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 23906..23997 `96473897…` | identical |
| dist/solid.dev.js | 1017-1019 | 31896 | 31987 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 25602..25693 `96473897…` | identical |
| dist/solid.observe.js | 992-994 | 30013 | 30104 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 24226..24317 `96473897…` | identical |
| dist/server.js | 469-487 | 13012 | 13637 | `38028a6c27dec146a1e4eb81e2e9bdfce1f1e90ce2d2701fe21c1d460148cf39` | 8763..9388 `38028a6c…` | identical |
| dist/server.dev.js | 565-585 | 17518 | 18207 | `6002fc5a126d938f9e23b7d284d707d2448bf56a65da3e428bb02167efe9c979` | 12317..13006 `6002fc5a…` | identical |
| dist/server.observe.js | 528-546 | 14665 | 15290 | `38028a6c27dec146a1e4eb81e2e9bdfce1f1e90ce2d2701fe21c1d460148cf39` | 10004..10629 `38028a6c…` | identical |

---

### 46. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 13.

**rc.13 bytes [M].** Wrapper slices byte-identical (`96473897…`); `hydratedCreateSignal` and `withHydrationGate` byte-identical; `_createSignal` still written only by `enableHydration` (`dist/solid.js:911`).

**Walk [M].** From `createSignal`, `hydratedCreateSignal`, `_createSignal`: 40 definitions (rc.9: 27), identical across the three browser builds; changed/new/gone exactly as § 44 minus `hydratedCreateMemo`; imports used `createSignal$1`, `getOwner`, `peekNextChildId`, `markSnapshotScope` (the rc.9 set); the same free globals. The hybrid branch now builds its own gate (`createSignal$1(false, { ownedWrite: true })`, a delegate call) instead of `withHydrationGate`, and may `queueMicrotask(flip)` (a reactive write).

**No create.** § 44's dispositions.

**Scope.** Condition `browser`; runtime the three browser files; delegates `@solidjs/signals` `createSignal` and `getOwner` `creates`, granted on rc.13 (§ 11, § 18).

**Verdict: GRANTED, scoped to `browser`.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 980-982 | 29693 | 29784 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 23906..23997 `96473897…` | identical |
| dist/solid.dev.js | 1017-1019 | 31896 | 31987 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 25602..25693 `96473897…` | identical |
| dist/solid.observe.js | 992-994 | 30013 | 30104 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` | 24226..24317 `96473897…` | identical |

---

### 47. `useContext` — `reads` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_READS_AUDIT § 7.

**rc.13 bytes [M].** All six slices byte-identical (client `03f0fc70…`, server `3cbc46d0…`). Signals `getContext`: dev byte-identical; prod and observe differ in one mangled owner field (`t.ze`/`t.wt` → `t.Je`/`t.rt`). `NoOwnerError`/`ContextNotFoundError` identical. Server walk (5 definitions: `getContext`, `serverComponentContextError`, …) all identical.

**Probes [M].** `useContext(ctx)` leaves the memo untracked and returns the default in every client build; the server builds return the default.

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 18-20 | 1635 | 1697 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1635..1697 `03f0fc70…` | identical |
| dist/solid.dev.js | 17-19 | 1682 | 1744 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1664..1726 `03f0fc70…` | identical |
| dist/solid.observe.js | 18-20 | 1657 | 1719 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1657..1719 `03f0fc70…` | identical |
| dist/server.js | 1846-1856 | 56674 | 56930 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 50326..50582 `3cbc46d0…` | identical |
| dist/server.dev.js | 1994-2004 | 62931 | 63187 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 54707..54963 `3cbc46d0…` | identical |
| dist/server.observe.js | 1936-1946 | 59232 | 59488 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 52445..52701 `3cbc46d0…` | identical |

---

### 48. `useContext` — `creates` — archive `solid-js@2.0.0-rc.13`

**rc.9 basis.** RC9_CORE_WEB_AUDIT § 11. As § 47 [M]; no path touches a host.

**Verdict: GRANTED.**

**rc.13 definition per cited build** (these are the row's citations; status compares the rc.9 cited slice):

| `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | rc.9 range and digest | vs rc.9 |
| --- | --- | --- | --- | --- | --- | --- |
| dist/solid.js | 18-20 | 1635 | 1697 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1635..1697 `03f0fc70…` | identical |
| dist/solid.dev.js | 17-19 | 1682 | 1744 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1664..1726 `03f0fc70…` | identical |
| dist/solid.observe.js | 18-20 | 1657 | 1719 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` | 1657..1719 `03f0fc70…` | identical |
| dist/server.js | 1846-1856 | 56674 | 56930 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 50326..50582 `3cbc46d0…` | identical |
| dist/server.dev.js | 1994-2004 | 62931 | 63187 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 54707..54963 `3cbc46d0…` | identical |
| dist/server.observe.js | 1936-1946 | 59232 | 59488 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` | 52445..52701 `3cbc46d0…` | identical |

---

## Flat rows the scoped rows narrow

ADR 0197. A host-target row stands on a flat row of the same export that stays
withheld. On rc.9 the two `browser`-scoped rows (§ 44, § 46) narrowed flat
`creates` rows withheld for the server builds. Those reasons hold on rc.13's
bytes, so both flat rows stay withheld.

### W1. `createSignal` — `creates` — archive `solid-js@2.0.0-rc.13` — **WITHHOLD**

As on rc.9 (`2026-09-27-solid-2-rc9-core-and-web-negative-rows.md` § 12). Under
`node`/`worker`/`deno`, `dist/server.js:469` `createSignal`'s derived overload
calls `createMemo` (`:479`), whose run hands its result to `processResult`
(`:536`), which reaches `ctx.serialize` (`:744-747`, `:834-866`, `:927`, `:942`)
under `renderToStream` with an owner id and outside `NoHydration` [M]. That is a
create under [Decision 2026-09-04], and a flat row carries no guard. The
browser bodies create nothing (§ 46).

### W2. `createMemo` — `creates` — archive `solid-js@2.0.0-rc.13` — **WITHHOLD**

As on rc.9 (`2026-09-28-solid-2-rc9-merge-omit-creatememo-creates.md` § 3). The
server `createMemo` (`dist/server.js:488`) runs its compute at creation and
hands a thenable result to `processResult` (`:536`), which reaches
`ctx.serialize` (`:744-747`) under `node`/`worker`/`deno` ∧ `renderToStream` ∧
an owner id ∧ not `NoHydrate` [M]. The browser bodies create nothing, and the
scoped row states exactly that (§ 44).

## Other rc.9-scoped answers in `solid_2.rs`

These answers live outside `NEGATIVE_ROWS`. Each is either gated in code on an
rc.9 archive or sourced in its doc comment from rc.9 bytes. The ADR 0168
argument-scoped rows (§§ 10, 26, 29, 45) and the two `browser` host-target rows
(§§ 44, 46) are rows, and are decided above. `other-answers.json` carries the
same list.

| # | answer (`solid_2.rs`) | what it asserts | rc.13 verdict |
| --- | --- | --- | --- |
| A1 | `computed_accessor_read_archive` (ADR 0162, ADR 0175; gated on `audited.version == "2.0.0-rc.9"` for `@solidjs/signals` and `solid-js`) | Invoking `createMemo`'s whole result, or `createSignal`'s slot 0, reads the node the creating call made. When that node is stale it re-runs the computation the call registered, and it may throw. It invokes no other callable. | **Holds on rc.13** [M diffs, E reading]. In signals, `createMemo`, `accessor` (`read.bind(null, node)`, `$REFRESH`) and `signal` are byte-identical in dev. `read` adds only the dev `checkPostAwaitRead` diagnostic and a guard term; `prepareComputed` relinks an auto-disposed computed under its live parent before `recompute(comp, true)`; `updateIfNecessary` widens a flag mask. In `solid-js`, every exit of rc.13's client `hydrateSignalLike` still returns `coreFn(…)`'s result (the new `transparent \|\| noHydrationId()` exit, `withHydrationGate` (identical), the rebuilt hybrid branch, `hydrateSignalFromAsyncIterable` (identical), and the final `coreFn`). On the server, `createMemo`'s `read` is unchanged (only the disposal flag gained `onDisposed` hooks), and `createSyncMemo`, `clientHoleRead` and `createSignal` are identical. What changed is the registered computation, which is the creating call's. **The gate must be extended to the rc.13 tuples for the answer to bind**: a code change, not made here. |
| A2 | `inert_accessor_read` (ADR 0146) | With primitive arguments, `createSignal`'s slot 0 is a plain signal's `read`, which runs no code. | **Holds** [M]: `createSignal`'s plain path, `signal` (dev) and `accessor` are identical. `read`'s rc.13 additions are dev diagnostics (`checkPostAwaitRead`), of the same class as rc.9's `warnStrictReadUntracked`, so no caller code runs. The `solid-js` server `createSignal` non-function path (`() => first`) is identical. |
| A3 | `inert_read_ignores_options` (ADR 0180) | `read` consults neither `options.equals` nor `options.unobserved`. | **Holds** [M]: dev `signal` is byte-identical, prod differs in mangled names only, and rc.13 `read` does not reference `_equals` or the `unobserved` slot. The one `unobserved` mention is a comment above the `dormantNodes` deferral, which applies only to auto-disposed computeds and already existed in rc.9. Not re-probed. |
| A4 | `eager_owned_computation_slot` (ADR 0183) | A one-argument `createMemo` runs its compute during the call under the new node. A two-argument `createEffect` runs its compute during the call. | **Holds** [M]: `setupComputedNode` keeps `!options?.lazy && recompute(self, true)`. `effect` and `createEffect` are identical (runtime.md § 3.3). runtime.md probes J/K/N show `ranDuringCall=true`, and `REACTIVE_WRITE_IN_OWNED_SCOPE` still fires in dev on rc.13. |
| A5 | `callback_handles_pending_accessor_read` | `isPending`'s callback is exempt from the strict pending-read safeguard. | **Holds** [M]: rc.13 `read` still guards with `strictRead && !pendingCheckActive && owner._statusFlags & STATUS_PENDING`, and the new `checkPostAwaitRead` gate also excludes `pendingCheckActive`. |
| A6 | `tracking_runtime` (ADR 0163) | `@solidjs/signals` exports `createRoot`, `createMemo`, `createSignal` and `getObserver`, and a read inside a created memo links a dependency (`link`). | **Holds** [M]: all four are exported by `dist/prod/index.js`, `dist/observe/index.js` and `dist/dev.js`. `link` is identical in dev (prod and observe are mangled). The probe's run-time calibration of the dependency fields succeeded on all three rc.13 builds (§ 0.6). |
| A7 | `AUDITED_ARCHIVES` rc.9 tuples | The archives a row may name. | rc.13 tuples, all [M] from § 0.1: `@solidjs/signals` / `2.0.0-rc.13` / `sha512-4+pRdrAH…MYeQ==` / `6783c3c6…95a6`; `solid-js` / `2.0.0-rc.13` / `sha512-62bYOI4J…BdoQ==` / `ce43a022…d284`; `@solidjs/web` / `2.0.0-rc.13` / `sha512-vI/7v/XM…gOQ==` / `8b45ed71…2bf5`. The full strings are in § 0.1 and `other-answers.json`. |
| A8 | Withheld rc.9 entries (the dialect test's withheld list: `Show`, `Loading`, flat `createSignal`/`createMemo`, `merge`, `affects`/`isPending`/`latest`/`refresh`, `hydrate`/`render` `reads`; and `createOptimisticStore` `reads`) | No row. | **Not re-read.** They stay rowless for rc.13. `reconcile` `reads` joins them (§ 24). |
| A9 | `DEFINITION_ALIASES` (`createOptimisticStore` → `createOptimisticStoreNext`, `createProjection` → `createProjectionNext`, version-keyed) | The local name a citation's slice begins with. | Same aliases on rc.13 [M]: `dist/prod/index.js:41`, `:43` and `dist/observe/index.js:43`, `:45`. The table needs rc.13 entries. |
| A10 | The host-target scopes of §§ 44, 46: condition `browser`, runtime `dist/solid{,.dev,.observe}.js`, delegates | Which `.` resolutions the scoped rows answer for. | **Unchanged** [M]: `solid-js`' `exports["."]` is identical to rc.9's. The delegates are rc.13 signals rows granted here (§§ 4, 11, 18). |
| A11 | `releases.rs` release answers (B1–B4, N3–N5) | Per-release vocabulary. | Not re-measured here. runtime.md § 3.5 measured all seven equal to `Solid2::RC9` on rc.13. |

## Citations, and how they were computed

- **Bytes and offsets.** Byte offsets are of the whole file, read in binary from
  `rust/target/audit-rc13/node_modules`. Each cited file's sha256 and length
  equal its rc.13 `files.json` entry. Each slice is the rc.13 definition of the
  same name, cut by the convention the rc.9 citation of the same row and file
  used. There are two:
  - From the start of the definition's line (including a leading ` */ `) to
    the start of the next line.
  - From the `function`/`const` token to the closing `}` or `;`.
- **Conventions were reproduced first** [M]. The convention was inferred from
  the rc.9 slice, and re-cutting rc.9 by it reproduced every rc.9 citation's
  `start_byte`/`end_byte` exactly, all 192 citations of the 48 rows.
- **Uniqueness** [M]. Each rc.13 definition is the only `function <name>(` or
  `const <name> = ` in its file.
- **Slice files.** The slices are written verbatim to
  `audited-slices/solid-v2/rc13/{solidjs-signals,solid-js,solidjs-web}/<archive_path>.<start>-<end>.slice`.
  That is `audited_slice_path`'s scheme with `phase0_archive_directory` =
  `rc13/<archive>`.
- **Withheld row.** § 24's table records the subject of the reading only; the
  row carries no citation.
- **Self-check** [M]. For every citation in `rows.json`, the slice file and the
  cited range of the rc.13 file were re-hashed. Both equal `slice_sha256`, and
  `file_sha256` equals `files.json` (`tools/selfcheck.mjs`, output in
  `selfcheck.txt`).

## What the lead must change for these rows to bind (not done here)

1. `AUDITED_ARCHIVES`: add the three rc.13 tuples (A7), and add
   `audited-archives.json` if it mirrors them.
2. Add `const RC13: &str = "2.0.0-rc.13";` and an audit constant naming this
   document once it is moved under `docs/package-contract-v2/audits/`. The
   `section` strings in `rows.json` are this document's headings verbatim.
3. Add the 47 granted rows. Each keeps its rc.9 row's `scope` and argument and
   host-target premises verbatim; `rows.json` `scope` is the rc.9 Rust
   expression. Copy the slice files into
   `rust/crates/solid-dialect/audited-slices/solid-v2/rc13/`.
4. `DEFINITION_ALIASES`: add the two rc.13 entries (A9). In the dialect test's
   implementation-audit list, add the 47 rows and record `reconcile` `reads` as
   withheld for rc.13.
5. `computed_accessor_read_archive`: admit the rc.13 `@solidjs/signals` and
   `solid-js` tuples (A1), with its own test.
6. Update the counts in the `NEGATIVE_ROWS` and `AUDITED_ARCHIVES` doc comments
   and in `the_negative_table_is_derived_from_the_audited_documents`, and
   record the `reconcile` `reads` withholding in `docs/precision-backlog.md`.
7. `releases.rs:39` currently says "No negative row is carried to its archives
   (ADR 0194)". That sentence moves with the rows.

## Residual approximations

- The call graphs are name-based over-approximations (§ 0.4). Dispatch through
  node fields (`_fn`, `_equals`, `_run`), caller values and hook slots is
  dispositioned by hand. The hook-slot follow-up was run on the dev build only;
  prod and observe rely on the name-based graph and slice diffs.
- The spurious `write` cut (owner item 4) was verified by showing that no
  `reads` root reaches `runProjectionComputedNext`, the arrow's only legitimate
  caller. A root that did reach it would need the edge back.
- Owner flag O1 is a judgement, not a measurement; the owner accepted it on 2026-10-05.
- `reconcile` `reads` is withheld on reachability. Whether the
  `readerOverride` arm actually runs under a caller's optimistic-store setter
  was read, not executed [E].
- The `solid-js` pairing covers `solid-js@2.0.0-rc.13`'s server builds. No other
  `solid-js` release is covered beside rc.13 signals.
- The flat `For`, `Repeat`, `clientOnly` rows rest on rc.9's flags F1–F3 as
  before.

## Tools (all under `2026-10-05-solid-2-rc13-negative-rows/tools/`)

| file | does |
| --- | --- |
| `parse-rows.mjs` → `rc9-rows.json` | extracts the 48 rc.9 rows and their citations from `solid_2.rs` |
| `locate.mjs` → `located.json` | verifies each rc.9 citation, infers its convention, and locates and cuts the rc.13 definition |
| `closure.mjs`, `measure.mjs` → `measurements.json` | read-site call graph, host reach, rc.9/rc.13 closure comparison |
| `slots.mjs` | hook-slot follow-up (dev) |
| `readsites.mjs` | archive-wide read-site census |
| `hostcensus.mjs` → `host-*.txt` | archive-wide host-boundary census |
| `topwalk.mjs`, `defcmp.mjs`, `defdiff.mjs`, `show.mjs`, `slicediff.mjs`, `newcalls.mjs` | per-file walks and diffs |
| `texts.mjs`, `head.md`, `tail.md`, `build.mjs` | generate `rows.json`, the slices and this document |
| `selfcheck.mjs` | the citation self-check |
| `../probes/` | the rc.9 reads probes run on rc.9 and rc.13 |
