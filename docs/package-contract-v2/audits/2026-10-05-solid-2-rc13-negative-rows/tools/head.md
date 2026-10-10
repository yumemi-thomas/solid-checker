# Audit: the rc.9 negative rows, re-read on the `2.0.0-rc.13` archives

Date: 2026-10-05. Status: **for the repository owner's review**, as the rc.3,
rc.6 and rc.9 audits it follows were. Staged under
`rust/target/audit-rc13/rows/`; nothing in the repository was changed.

**Why it exists.** Rows are archive-scoped (`NegativeClaimRow::version`), and
ADR 0194 made `2.0.0-rc.13` the audited release without carrying any row to its
archives (`solid_2.rs`, `AUDITED_ARCHIVES` doc: "rc.13 is not listed: no row was
read on its bytes"). This document reads every one of the 48 rows the table
carries for `2.0.0-rc.9` on the exact bytes of the rc.13 archives, by the method
of the audit that granted the rc.9 row, and grants a row for rc.13 only where
its claim is established on rc.13's own bytes.

**Result.** {{GRANTED}} of the 48 rows **GRANT** for rc.13. {{WITHHELD}} is
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
{{SIGNOFF}}

### What the owner is asked to agree with beyond the table

1. **O1 — the rc.13 dev/observe root registry is not a `create`** [E]. rc.13's
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
used a project under `rust/target/audit-rc13/rows/probes/{rc9,rc13}/` whose
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
