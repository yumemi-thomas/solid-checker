# Audit: `@solidjs/signals@2.0.0-rc.9` — the remaining negative rows, read to parity with rc.3

Date: 2026-09-27. Status: **for the repository owner's review**, as the rc.3,
rc.6 and 2026-09-26 rc.9 audits it follows were. It reads, on the exact bytes of
`@solidjs/signals@2.0.0-rc.9`, the 20 `@solidjs/signals` rows that
`rust/crates/solid-dialect/src/solid_2.rs` carries for rc.3 and that
[`2026-09-26-solid-2-rc9-signals-negative-rows.md`](2026-09-26-solid-2-rc9-signals-negative-rows.md)
(the "five-row audit") did not read. Together the two documents cover all 25 rc.3
rows.

**Why it exists.** The owner is making `2.0.0-rc.9` the audited release. The
five-row audit granted only the `creates` rows of `getOwner`, `onCleanup`,
`createRoot`, `untrack` and `runWithOwner`, so every other rc.9 row was
unanswered and certification of an rc.9 tree lost closures (the `KnownGap` in
`rust/crates/solid-dialect/src/solid_2/releases.rs`). It also left the scoped
`solid-js@2.0.0-rc.3` `createSignal` row refusing beside rc.9, because its
delegate `@solidjs/signals` `createSignal` `creates` had no rc.9 row
(`DELEGATE_GAPS` in the dialect test).

**Result.** 19 of the 20 rows **GRANT**. `createOptimisticStore` `reads` is
**WITHHELD**, for the reason the rc.6 audit withheld it: rc.9 still carries the
export's own landing router (`wrapCommit` → `stageLanding` → `stagedApply`),
which reads through the store proxy the call created. rc.9 therefore reaches
parity with rc.6 (24 rows), not with rc.3 (25). No row of rc.3, rc.6 or the
five-row audit changes.

**Method.** The rc.6 audit's, group for group:
[`2026-09-25-solid-2-rc6-signals-negative-rows.md`](2026-09-25-solid-2-rc6-signals-negative-rows.md)
groups A–D. That audit follows
[`2026-09-04-solid-2-rc3-core-primitives-creates.md`](2026-09-04-solid-2-rc3-core-primitives-creates.md)
§ 1.3 (dispositions `local`, `builtin`, `caller`, `hook`, `host`, and the
`drain` class of [Decision 2026-09-10]) and applies `semantic-model.md` § reads
and § creates. Each row below:

- reads the export's definition in all three builds rc.9's `exports` map can
  select (§ 0.2);
- walks the call-event closure with an over-approximate call graph (§ 0.4), and
  reads by hand every frame the verdict leans on;
- diffs, in the unmangled development build, the rc.6 and rc.9 bodies of the
  export and of every closure function whose behaviour decides the row, and
  states which changed and whether that matters; and
- reads the `solid-js@2.0.0-rc.9` server bodies of every name `solid-js`
  re-exports from this archive (§ 0.5).

It decides only what the two domains let it decide. A `creates` row denies that
one invocation **registers a version-1 resource into a runtime outside the
invocation** (a browser document or a server runtime). A `reads` row denies
that one invocation's own code observes a reactive source's current value,
excluding what a caller-supplied callable reads, a property access on a
caller-supplied receiver, and a read by a callable the export did not author
([Decision 2026-09-10], both clauses). The lines are exactly the ones the rc.6
audit drew; none is redrawn here.

**Performed** by one reader, in the foreground. Four further readers
independently read group B, group C (in two halves), and groups A and D
together; they reached the same verdicts, the same withholding
site, and the same two rc.9 changes (the removed patch channel, and
`snapshot`'s new transaction entry). Every citation in § 6 was
computed from the tarball bytes, and the slicing script reproduced the rc.6
slice digests that `solid_2.rs` already carries, under both slice conventions.

## 0. For the owner's sign-off

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| A1 | `createSignal` `creates` | **GRANT** | Plain overload: a signal literal and bound accessors (dev and observe also run `registerGraph`, which writes a field; dev also pushes onto the owner's `_signals` and calls `DEV.hooks.onGraph`). Derived overload: `computed` → `setupComputedNode` → `recompute(e, true)` runs the caller's compute unless `lazy`. The only host reaches are `queueMicrotask(flush)` and console output. |
| B1 | `createMemo` `reads` | **GRANT** | At the call event `createMemo`'s own code calls no read site in any build. The only read on the stack is `read(bridgeSignal)` in the wrapper that the `enableExternalSource` hook installs, which is dispositioned hook exactly as on rc.3 and rc.6. `flush` is reachable only from `handleAsync`'s async landing continuations. |
| B2 | `createMemo` `creates` | **GRANT** | The archive has no handle to a document or a server runtime (§ 0.3). The closure's host reaches are `queueMicrotask` and console output. |
| B3 | `createTrackedEffect` `reads` | **GRANT** | `trackedEffect` creates a lazy computed. rc.9 queues its first run on the heap (`enqueueSub` + `schedule()`); rc.6 used `Queue.enqueue`. No read site is reachable at the call event. |
| B4 | `createTrackedEffect` `creates` | **GRANT** | Same closure. The only host reaches are `queueMicrotask` and dev console output. The returned cleanup is a `cleanups` item. |
| B5 | `onSettled` `reads` | **GRANT** | `getOwner()`, then either `trackedEffect(() => untrack(e))` (lazy, queued) or `globalQueue.enqueue(EFFECT_USER, fire)`, where `fire` re-enqueues itself while the heap has work (new in rc.9). No read site is reachable. |
| B6 | `onSettled` `creates` | **GRANT** | Both arms reduce to B4's closure or to `Queue.enqueue` + `schedule()`. The solid-js rc.9 server body is `getOwner()` plus `getNextChildId`. |
| B7 | `flush` `reads` | **GRANT** | `flush`'s own code (the new `FLUSH_IN_ACTION` guard, the `fn` branch, the drain loop) has no read site. With the boundary-queue methods and the external-source hook cut, as on rc.6, `GlobalQueue.flush` reaches none. The drain's reads belong to the registrants ([Decision 2026-09-10]). |
| B8 | `flush` `creates` | **GRANT** | Every archive function the drain reaches is bounded by § 0.3. The solid-js rc.9 server `flush` is `function flush() {}`. |
| C1 | `createStore` `creates` | **GRANT** | `createStoreNext` (target, proxy, setter closure) or `createStoreDerivedNext` → `createProjectionNextInternal`. The only host reaches are `queueMicrotask(flush)` and console output. rc.9 has no patch channel (§ 0.6). |
| C2 | `createProjection` `creates` | **GRANT** | `createProjectionNextInternal` allocates a target, a proxy and a projection computed. The derive is the caller's; commits are adoption writes inside the archive. |
| C3 | `createOptimistic` `reads` | **GRANT** | Engine slot installs, node allocation, `accessor`'s `read.bind` (invokes nothing). No read site in the call closure or in the setter's (`setSignal`) closure. This still rests on the accessor line rc.3 and rc.6 drew. |
| C4 | `createOptimistic` `creates` | **GRANT** | Node allocation and module-state slot installs. The only host reaches are `queueMicrotask(flush)` and console output. |
| C5 | `createOptimisticStore` `reads` | **WITHHOLD** | Unchanged from rc.6: `wrapCommit` (prod `store/next/optimistic.js:194-199`, observe `:196-201`, dev `dist/dev.js:7690-7695`) → `stageLanding` → `storeSetterNext(fam.px, draft => stagedApply(draft, …))`. `stagedApply` runs string-key `get`, `length` and `ownKeys` traps on `fam.px`, the proxy this call created, when its own computation commits under a retaining transaction. |
| C6 | `createOptimisticStore` `creates` | **GRANT** | Targets, proxies, a family record, the derived computed, and the archive's own scheduler transitions. Nothing is registered outside the archive, including on the landing path. |
| C7 | `reconcile` `reads` | **GRANT** | At the call event: closure creation only. On application, `reconcileNextState` makes brand lookups on the store proxy passed in and otherwise works on raw backings and the caller's value. No read-path call anywhere in its closure; every proxy it can touch is parameter-rooted. |
| C8 | `reconcile` `creates` | **GRANT** | Application performs adoption writes, `setSignal` notifications and `schedule()`. The solid-js rc.9 server body only calls `setProperty` on the caller's `state`. |
| D1 | `action` `reads` | **GRANT** | The call event returns an arrow. The wrapper's frames match rc.6's apart from origin bookkeeping (`setOrigin`, `actionSeq`), the action-step depth counter, the `_acted` flag and attribution hooks. Its flushes are drains. |
| D2 | `action` `creates` | **GRANT** | Host reaches: the promise returned to the caller, `queueMicrotask(flush)`, and (dev) console output. The solid-js rc.9 server `action` is `return fn`. |
| D3 | `snapshot` `creates` | **GRANT** | `snapshotNext` → `snapshotWalk`. New in rc.9: under a reader context with a held fold, `pendingBackingVisible` → `holdVisible` → `enterStagedRead` may enter that transaction (`initTransition`, and so `schedule()`). The only host reach is therefore `queueMicrotask(flush)`; rc.6 had none at all. |

### What the owner is asked to agree with beyond the table

1. **Parity stops at rc.6's 24 rows.** `createOptimisticStore` `reads` was
   granted on rc.3 and withheld on rc.6. rc.9 keeps the rc.6 landing code
   essentially verbatim (C5), so it stays withheld. Nothing in rc.9's bytes
   supports the rc.3 claim.
2. **The lines are rc.6's.** B1 keeps the external-source hook line (flag 1 of
   the rc.6 audit). C3 keeps the accessor line (flag 2). C7 keeps the
   parameter-rooted receiver line. If any of those is redrawn, the rc.6 and rc.9
   rows move together.
3. **The `solid-js` pairing.** `solid-js@2.0.0-rc.9` re-exports `action`,
   `createTrackedEffect`, `flush`, `onSettled`, `reconcile` and `snapshot` from
   this archive (`types/index.d.ts:1`), and its `node`/`worker`/`deno` builds run
   `solid-js`'s own bodies for all of them except `snapshot`, which
   `server.js:2` re-exports from `@solidjs/signals`. All were read (§ 0.5) and
   reach the same verdicts. `createSignal`, `createMemo`, `createStore`,
   `createProjection`, `createOptimistic` and `createOptimisticStore` are
   `solid-js`'s own declarations in rc.9 (`types/index.d.ts:8`), so a `solid-js`
   import of those names does not bind these rows. `solid-js@2.0.0-rc.3`'s
   server bodies, read by the rc.6 audit, are `solid-js` bytes and reach the
   same verdicts beside rc.9. Where they re-export a name from
   `@solidjs/signals` (`snapshot`), the signals `default` build read here runs.
   A `solid-js` other than rc.3 or rc.9 is not covered.
4. **`reportError` is reached here, where it was not for the five.** The
   five-row audit ruled that `haltReactivity`'s `globalThis.reportError` (new in
   rc.9) registers no version-1 resource, and noted that none of its five rows
   reaches it. Some of these rows do: `flush`'s drain, and the queued runs that
   `createTrackedEffect` and owned `onSettled` schedule, reach `haltReactivity`
   when an effect's function throws uncaught (`dist/dev.js:1663`, `:1744`,
   `:1775`, from `runEffect` and the effect error paths). The `creates` grants
   B4, B6 and B8 therefore rest on that ruling as well. It hands the error to
   the host's uncaught-error channel and registers nothing that stays live.

### What this audit does not do

- It reads no domain other than the rc.3 row's for each export.
- It grants no `callbacks` row. The rc.6 audit found the rc.3 `createOptimistic`
  and `createOptimisticStore` summaries' `callbacks: []` contradicted by their
  function overloads. That is still true on rc.9 (`setupComputedNode`'s
  `!options?.lazy && recompute(self, true)`, dev `dist/dev-shared.js:5005`,
  prod `dist/prod/core/core.js:817`).
- It adds rc.9 to no install pin (`scripts/ecosystem-benchmark/lib/runtime-pins.mjs`).

---

## Shared inputs (§ 0.1–0.6; every group relies on these)

### 0.1 Identity and files

`@solidjs/signals@2.0.0-rc.9`,
`sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==`,
`package.json` sha256 `c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6`.
The tarball was downloaded again for this reading (`npm pack
@solidjs/signals@2.0.0-rc.9`, into an isolated temporary directory). Its
SHA-512 is the integrity above, and all 119 extracted files match
`benchmarks/package-contract-v2/phase0/rc9/solidjs-signals/files.json` byte for
byte and by length. The rc.6 comparand was downloaded the same way, and its
SHA-512 is the rc.6 tuple's `sha512-lPqwZNLP…`. `solid-js@2.0.0-rc.9` was
downloaded for the server pairing (SHA-512
`J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==`).

Files the citations rest on, as pinned in `files.json`:

| File | sha256 | bytes |
| --- | --- | --- |
| `dist/dev-shared.js` | `70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e` | 295,301 |
| `dist/dev.js` | `f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120` | 418,037 |
| `dist/observe/core/action.js` | `506c5f29d6344334f9eccece2c925dd0b94af4eaf3e0273aed825ed37ded4298` | 8,202 |
| `dist/observe/core/scheduler.js` | `3abec8dc70d0ed2834a5b7040ea0e28b0af867c4201ecb4c90b46581020327ca` | 71,016 |
| `dist/observe/signals.js` | `6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5` | 32,402 |
| `dist/observe/store/index.js` | `a58b551e6a02e139e89ac04e29b1ef08cfb086c5f55682039a31bb9b3350f324` | 936 |
| `dist/observe/store/next/optimistic.js` | `3213841e222127d05061bb483e5eb0d7e3b269683023a18525dd5c076a0a55cc` | 31,452 |
| `dist/observe/store/next/projection.js` | `8fdf22ff1154b5183d4e1c6fb84b40e5e4decea5c90e34ca1de50fe7930634fd` | 9,570 |
| `dist/prod/core/action.js` | `ade163f51610818458e29a7902c3b7042ce1f098ff6be252638857a848d44b77` | 7,661 |
| `dist/prod/core/scheduler.js` | `ac77e8c1c6b44a943310bb3976d41b2e66cdd58a8805dad70d7c5e7c91039cde` | 69,197 |
| `dist/prod/signals.js` | `d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6` | 32,262 |
| `dist/prod/store/index.js` | `4c0118833a2e8fb1455313398d87b6700aeb7f9a98731f28308ece04823350e8` | 910 |
| `dist/prod/store/next/optimistic.js` | `b3a3de0d67305b92c398eca0602f1a72f350e0d6915dc8d12cf486484b4e964c` | 31,342 |
| `dist/prod/store/next/projection.js` | `7e2175a020d7d9208211a122164a9c5e8bfe2c3df9aa2ec3441bcc8867982aec` | 9,476 |

### 0.2 Which build each condition selects, and the bindings

The five-row audit's § 0.2 is authoritative and was re-checked. `exports["."]`
selects `dist/dev.js` for `test`/`development`, `dist/observe/index.js` for
`observe`, and `dist/prod/index.js` for `default`. There is no `require` arm.

Bindings of the twelve exports read here:

- **`default`.** `dist/prod/index.js:27` re-exports `createMemo`,
  `createOptimistic`, `createSignal`, `createTrackedEffect` and `onSettled` from
  `./signals.js`. `:17` re-exports `flush` from `./core/scheduler.js`, `:23`
  `action` from `./core/action.js`, and `:33` `createStore`, `reconcile` and
  `snapshot` from `./store/index.js`. `:41` exports
  `createOptimisticStoreNext as createOptimisticStore` and `:43`
  `createProjectionNext as createProjection`.
- **`observe`.** `dist/observe/index.js` has the same shape over
  `dist/observe/**`; the two aliases are at `:43` and `:45`.
- **`test`/`development`.** `dist/dev.js` defines ten of the twelve itself. It
  imports `flush` from `./dev-shared.js` (`:103`, `aL as flush`; exported there
  at `dev-shared.js:6237`). Its final `export { … }` lists all twelve, with
  `createOptimisticStoreNext as createOptimisticStore` (`:9498`) and
  `createProjectionNext as createProjection` (`:9500`).

Each name has exactly one `function` definition per build.

### 0.3 Archive-wide host boundary

The five-row audit's § 0.3 census was **re-run independently** for this
reading. A lexer that skips comments, strings, template text and regular
expressions was run over every `.js` file under `dist/`, looking for the free
identifiers of that census plus `serialize`, `require`, `importScripts`,
`location`, `history` and `crypto`, and for every non-relative import. The
result is the same:

- No import leaves the archive. The only non-relative specifier is the
  `@solidjs/signals/attribution` diagnostic text.
- There is no `document`, `sharedConfig`, `_$HY`, serializer, network API,
  storage API or dynamic code. Every `window` and `history` token is inside
  `dist/dev.attribution.js` (local bindings), and every `self` token is a
  parameter or local name.
- The code references that leave the archive are the five-row audit's table:
  - `queueMicrotask` in `schedule()`, `MicrotaskQueue.enqueue`, `refresh` and
    dev `emitDiagnostic`;
  - `new Promise`/`Promise.resolve` in `action`, `resolve`, `refresh` and `until`;
  - `setTimeout`/`clearTimeout` in `until`;
  - `globalThis.reportError` in `haltReactivity`;
  - the two `globalThis[Symbol.for(…)]` slots (attribution hooks, records
    channel);
  - `globalThis.process?.env?.COMPANION_CENSUS` at flush;
  - `console.*`.

**Consequence**, as on rc.3 and rc.6: no call that stays inside
`@solidjs/signals@2.0.0-rc.9` can register a version-1 resource into a browser
document or a server runtime, because the archive holds no handle to either.
None of the references above registers a version-1 resource kind, so for every
`creates` row below the question reduces to two things: the export's own
`solid-js` server pairing, and whether anything the row would otherwise have
to own is in fact a caller-supplied or third-party callable.

### 0.4 How the walks were done

The rc.6 audit's call-graph aid (acorn 8, from `packages/cli/node_modules`)
was run over each build as an ES module tree: `dist/prod/**`,
`dist/observe/**`, and `dist/dev.js` together with `dist/dev-shared.js`. It
links calls by identifier across imports, by class-method name, and through
function-valued `GlobalQueue.X = fn` slots. It reports calls that resolve to the
engine's read entry points (`read`, `readNodeFast`, `serve`, `link`,
`pendingCheckRead`, `latestRead`), value uses of `read`, and free host
identifiers, per reachable function.

It over-approximates: it links nested closures into their enclosing function,
and it links any identifier to a same-named top-level function. Two spurious
edges on these paths are named where they matter:

- `emitDiagnostic`'s `capture.push(entry)` resolves `push` to an unrelated
  store function;
- `action`'s promise executor parameter `resolve` resolves to the module-level
  `resolve` export (D2).

It does not follow calls through a node's `_fn`, `_equals`, `_run`, queued
effect functions or user callables, which are `caller` or `drain` by
disposition. Every frame a verdict leans on was read by hand. The dev build
has unmangled names, so it is where the rc.6 → rc.9 diffs were taken, from
each function's code lines with comment-only lines dropped.

Archive-wide read-site census (rc.9 `default` build). The functions containing
a read entry-point call are:

- `read` itself;
- `readNodeFast`;
- `wireExternalSource` (`core/external.js:44-57`);
- `getLatestValueComputed`, `latestRead` and `pendingCheckRead`
  (`core/verdict.js`), reached only through `read`;
- `isPending` and `refresh` (public exports);
- `accessor` (`read.bind`, which invokes nothing);
- the boundary code `createBoundChildren`, `RevealController`'s evaluate
  method (`B` in prod and observe, `_evaluate` in dev), `CollectionQueue.run`
  and `createCollectionBoundary`;
- the store functions `nodeValue`, `serveDataKey`, `firewallGate` and
  `deepNext`, and the store trap objects in `store/next/store.js`'s module
  body.

This is the rc.6 set (rc.6 audit group B § 0.5), in rc.9's file layout, plus
one function: the store's `nodeValue` → `serve` (`store/next/store.js:1405-1418`,
`dist/dev.js:5996-6015`), a new untracked store read path. It is reachable from
none of the closures below that grant a `reads` row. The observe and dev
builds carry the same set.

### 0.5 The `solid-js@2.0.0-rc.9` server bodies

`solid-js@2.0.0-rc.9`'s `node`/`worker`/`deno` conditions run `dist/server.js`,
`dist/server.dev.js` or `dist/server.observe.js`. The bodies of the six names it
re-exports from this archive were extracted from all three builds. Each is
byte-identical across the three:

| Name | `server.js` | Body |
| --- | --- | --- |
| `action` | `:1596-1598` | `return fn;` |
| `createTrackedEffect` | `:890-893` | `const o = getOwner(); if (o?.id != null) getNextChildId(o);` |
| `onSettled` | `:1599-1602` | the same two statements |
| `flush` | `:1563` | `function flush() {}` |
| `reconcile` | `:1179-1193` | `state => { if (!isWrappable(state) \|\| !isWrappable(value)) return value; … setProperty(state, key, value[key]) … }` |
| `snapshot` | `:2` | re-exported from `@solidjs/signals`, so the signals `default` build runs (D3) |

`getOwner` (`:107-109`) returns `currentOwner`. `getNextChildId` (`:36-38`) →
`nextChildIdFor` (`:28-35`) increments `_childCount` on the owner or throws.
`setProperty` (`:905-910`) writes to or deletes from the caller's `state`.
`isWrappable` is imported from `@solidjs/signals` and is a predicate. None of
these bodies registers anything, reads a reactive source, or touches
`sharedConfig`. Every property access in `reconcile`'s returned function has the
caller-supplied `state` or `value` as its receiver.

### 0.6 What changed in rc.9's store tree, as it bears on these rows

- **The patch channel is gone.** rc.6's `store/next/patch.js` and
  `patch-hooks.js` do not exist in rc.9. No code in any build references
  `patchHooks`, `rowHooks`, `registerPatch*`, `registerRowOps`, `emitPatch*`,
  `emitRowOps*` or `demoteToEffects`. rc.6's § 0.4 hook dispositions for patch
  consumers, and its dev-only `applyAdopt` demotion, therefore have nothing to
  apply to on rc.9, and they are not carried.
- `store/next/target.js` is new. `createTarget` gains or renames bookkeeping
  fields (`wk`, `kc`, `ab`; `sc` becomes a counter). `wrapNext` resolves through
  `lookupTarget$1`, and `reconcileNextState` disowns an owned backing
  (`delete out[$OWNER]`). All are archive-internal.
- `storeSetterNext` adds `devGuardStoreSetterResult(result)` (dev) and
  `stageHeldAdoptions()`. `runProjectionComputedNext`'s loading-window shadow
  uses `cloneState` instead of `JSON.parse(JSON.stringify(…))`.
- `createStoreDerivedNext`, `createProjectionNextInternal` and `snapshotNext`
  are code-identical to rc.6 in dev.

---

## rc.9 parity, group A — `createSignal` (`creates`)

### 1. `createSignal` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

This is `@solidjs/signals`' own `createSignal`, the delegate of the scoped
`solid-js@2.0.0-rc.3` `createSignal` row. `solid-js@2.0.0-rc.9`'s `createSignal`
is its own declaration and is not this row.

#### 1.1 Implementation, all three builds

`default`, `dist/prod/signals.js:67-75`:

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

- **`test`/`development`, `dist/dev.js:2209-2218`.** Byte-identical to rc.6's dev
  slice (`d1292c1a…`). The plain overload adds `registerGraph(node, getOwner())`
  (`dist/dev-shared.js:649-658`), which sets `value._owner`, pushes onto
  `owner._signals`, and calls `DEV.hooks.onGraph?.(value, owner)` (**hook**).
- **`observe`, `dist/observe/signals.js:69-78`.** Prod's body plus the same
  `registerGraph` call. The observe `registerGraph` (`observe/core/dev.js:226-228`)
  is the single field write `e.nt = t`.

| Callee | Site (dev) | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `signal(v, options)` | `dev-shared.js:5014-5051` | cond: first argument not a function | local | builds the signal literal (rc.9 moves `_name` into the literal and adds `_prevChild`/`_owner` slots); stores the **caller**'s `unobserved` without invoking it; `linkFirewallChild` is unreachable (no firewall argument) |
| `accessor(node)` | `dev.js:2204-2208` | always | local / builtin | `read.bind(null, node)` (invokes nothing), `fn[$REFRESH] = node`; code-identical to rc.6 |
| `setSignal.bind` / `setMemo.bind` | — | always | builtin | bound setters handed back, not invoked |
| `computed(fn, options)` | `dev-shared.js:4804-4859` | cond: derived overload | local | the node literal (`inheritId`, `ownerInSnapshotScope`); `"loadingValue" in options` is on the caller's options object; stores the caller's `unobserved`; calls `setupComputedNode` |
| `setupComputedNode` | `dev-shared.js:4978-5013` (prod `core/core.js:802-825`) | cond | local | links under `context`; dev: `PRIMITIVE_IN_FORBIDDEN_SCOPE` `emitDiagnostic` + `throw`, and `DEV.hooks.onOwner?.(self)` (**hook**); `GlobalQueue._wireExternalSource` (**hook**, installed by `enableExternalSource`); `!options?.lazy && recompute(self, true)` (`:5005`, prod `:817`); snapshot bookkeeping into the module `snapshotSources` set |
| `recompute(el, true)` | `dev-shared.js:4188-4775` (prod `core/core.js:127-`) | cond: derived, not lazy | local | runs **`el._fn`**, the caller's compute; `el._equals` (caller option or `isEqual`); on an object result, `handleAsync` (§ group B, B1) attaches `then`/async-iterator handling to the **caller**'s value; hook slots `GlobalQueue._applyReask`, `_laneAsyncPending`/`Settled`, `_laneOverride`, `_recomputeLane`, `_repollVerdicts`, `_supersedeOverride`, `_syncCompanions` (archive functions installed at module load or by another export); dev `attrHooks.*` (**hook**, attribution engine) |
| `schedule()` | `dev-shared.js:1297-1305` (prod `scheduler.js:279-`) | later | local → **host** `queueMicrotask(flush)`, or `console.error` via `notifyHalted` when halted | the only host reach |

#### 1.2 Closure bound

The call graph (§ 0.4) of `createSignal` in each build reaches only the
following host references:

- `schedule`'s `queueMicrotask(flush)`;
- `MicrotaskQueue.enqueue`'s `queueMicrotask`;
- console output (`notifyHalted`, `reportClientError`, and dev's
  `emitDiagnostic`/`reportDiagnostic`).

`haltReactivity`'s new `reportError` is not reachable. Every other function is
archive code, and § 0.3 bounds it.

#### 1.3 What changed from rc.6

- The prod and dev slices are byte-identical to rc.6's. Observe is new: prod's
  body plus `registerGraph`.
- `signal` and `computed` differ only in literal layout (`_name` moved into the
  literal, new `_prevChild`/`_owner` slots) and in `linkFirewallChild`.
- `recompute` was rewritten (`dev-shared.js:4188-4775`). Its call-outs are the
  caller's compute and `equals`, `handleAsync`, archive graph and queue
  bookkeeping, and the hook slots listed above. None is a host reach beyond
  `schedule()`.
- Nothing that matters to `creates` changed.

#### 1.4 Verdict

**GRANT.**

Sign-off: `@solidjs/signals@2.0.0-rc.9`'s `createSignal` allocates a signal or
computed node, links it into the owner tree and, for the derived overload, runs
the caller's compute synchronously unless `options.lazy` is set. It returns
bound accessors. Every host reach on its paths is `queueMicrotask(flush)` or a
console write. The row denies that one invocation of `createSignal` registers a
version-1 resource into any runtime outside the invocation. It says nothing
about `solid-js`'s own `createSignal`.

With this row, the scoped `solid-js@2.0.0-rc.3` `createSignal` row's two
delegates (`@solidjs/signals` `createSignal` and `getOwner` `creates`) are both
answered on rc.9, so the delegate gap the five-row audit recorded is closed.

---

## rc.9 parity, group B — `createMemo`, `createTrackedEffect`, `onSettled`, `flush` (`reads`, `creates`)

### 0. Group notes

- The `reads` rows decide what the export's own code observes at its call
  event, or schedules as its own. They exclude the reads of registered
  computations another invocation created ([Decision 2026-09-10]).
- As on rc.6, two nodes are cut from the call-only closure before the read-site
  check for `createMemo`, `createTrackedEffect` and `onSettled`:
  - the `_wireExternalSource` hook target `wireExternalSource`, a hook
    disposition (B1 § 1.3);
  - `flush`, which these exports reach only from `handleAsync`'s landing
    continuations: `asyncWrite` (`dev-shared.js:3662`, prod `core/async.js:447`)
    and the iterator's completion arm (`:3750`, prod `:532`). Both run when the
    caller's thenable or iterator settles.

  For `flush` itself, as on rc.6, the boundary-queue methods
  (`CollectionQueue.run` and `RevealController`'s evaluate method) and
  `wireExternalSource` are cut. What remains in every build contains no read
  site:

  | root | prod | observe | dev (with `dev-shared.js`) |
  | --- | --- | --- | --- |
  | `createMemo` | 112 functions | 119 | 123 |
  | `createTrackedEffect` | 113 | 120 | 125 |
  | `onSettled` | 114 | 121 | 126 |
  | `flush` | 111 | 117 | 162 |

  The only read-kind edge left in the first three is `accessor`'s `read.bind`
  (`createMemo`), which invokes nothing.
- **A synchronous thenable.** `handleAsync` attaches its continuation with the
  caller's value's own `then`. A caller whose thenable calls back synchronously
  therefore runs `asyncWrite` on `createMemo`'s stack. What that continuation
  does is `setSignal`, companion sync, `settlePendingSource`, `schedule()`,
  `flush()` and `then?.()`, none of which is a read site. `flush`'s drain is
  the registrants' (B7). This is the same code rc.6 had, and the verdict does
  not depend on the timing.

### 1. `createMemo` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

#### 1.1 Definitions

| Build | Definition | Body |
| --- | --- | --- |
| `default` | `dist/prod/signals.js:77-79`, bytes 3028..3095 | `function createMemo(e, t) { return accessor(computed(e, t)); }` |
| `test`/`development` | `dist/dev.js:2219-2221`, bytes 98008..98097 | `function createMemo(compute, options) { return accessor(computed(compute, options)); }` |
| `observe` | `dist/observe/signals.js:80-82`, bytes 3110..3177 | byte-identical to prod |

All three slices are byte-identical to rc.6's (`f0f1d1a8…` for prod and
observe, `46130d23…` for dev).

#### 1.2 Call-event walk (dev lines; prod in brackets)

| Callee | Site | Reach | Disposition | Reads? |
| --- | --- | --- | --- | --- |
| `computed` | `dev-shared.js:4804` [`core/core.js:631`] | always | local | Builds the node literal. The `in` test is on the caller's options object. No read. |
| `inheritId` → `getNextChildId` → `childId` → `formatId` | `dev-shared.js` owner helpers | cond: owner has an id | local / builtin | No |
| `ownerInSnapshotScope(context)` | — | cond | local | Field walk. No |
| `setupComputedNode` | `:4978` [`:802`] | always | local | Owner link; dev diagnostic + `throw`, `DEV.hooks.onOwner` (**hook**). No |
| `GlobalQueue._wireExternalSource(self)` | `:5004` [`:816`, slot `wt`] | cond: `enableExternalSource` was called | **hook** (`dist/dev.js:206-216`, prod `core/external.js:44-57`) | § 1.3 |
| `recompute(self, true)` | `:4188` [`:127`] | cond: not `lazy` | local | Runs the **caller**'s `el._fn`; `el._equals`; `handleAsync`; archive bookkeeping and the hook slots of group A § 1.1. None is a read site (§ 0). |
| `handleAsync(el, result)` | `:3374` [`core/async.js:229`] | cond: object result | local | `untrack(() => { iterator = result[Symbol.asyncIterator]; thenable = … isThenable(result) })`: members of the value the **caller**'s compute returned. The landing continuations run on settlement (§ 0). |
| `accessor(node)` | `dev.js:2204` [`signals.js:61-65`] | always | local / builtin | `read.bind` invokes nothing |

#### 1.3 The guarded read on the stack

When a third party has called `enableExternalSource`, the new node's initial
compute runs the wrapper `wireExternalSource` installed:
`self._fn = prev => { read(bridgeSignal); return source.track(prev); }`
(`dist/dev.js:212-215`; prod `core/external.js:53-56`). That tracked read of a
signal the hook created is dispositioned **hook**, exactly as on rc.3 and rc.6
(rc.6 audit group B § 1.3, and owner flag 1 there). It is the same code in
rc.9.

#### 1.4 What changed from rc.6

- The export is unchanged.
- `computed` moves `_name` into the literal.
- `setupComputedNode` is code-identical.
- `recompute` and `handleAsync` were rewritten (the rc.9 vocabulary review § 7).
  Their new call-outs are archive bookkeeping, hook slots, the caller's values,
  and the continuations above. The read-site census (§ 0.4) finds no new read
  entry point in either, and the closure check above finds none reachable.

**Per-build verdict.** prod, dev and observe: closed.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createMemo, Reads)` denies that one
invocation of `createMemo`, at its call event, observes a reactive source
through its own code. The caller's compute's reads are the caller's. The only
guarded read on the stack is inside the `_wireExternalSource` hook's wrapper.

### 2. `createMemo` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The walk is § 1.2. Every function in the closure is one of three things:

- archive code;
- a builtin;
- the caller's compute, `equals` or `unobserved`, or an installed hook
  (`DEV.hooks.onOwner`, `attrHooks`, the external-source
  `factory`/`track`/`untrack`).

The host reaches in the closure are `queueMicrotask` (`schedule`,
`MicrotaskQueue.enqueue`, dev `emitDiagnostic`) and console output. § 0.3 bounds
everything else. The node, its owner link and the snapshot bookkeeping are
reactive-graph or private-module structures, not version-1 registrations.
`createMemo` is `solid-js`'s own declaration in rc.9, so no server pairing
applies.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createMemo, Creates)` denies that
one invocation of `createMemo` registers a version-1 resource into a browser
document or a server runtime, now or at a later event it schedules.

### 3. `createTrackedEffect` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

#### 3.1 Definitions

| Build | Definition | Body |
| --- | --- | --- |
| `default` | `dist/prod/signals.js:231-233`, bytes 9691..9759 | `trackedEffect(e, t);` (byte-identical to rc.6) |
| `test`/`development` | `dist/dev.js:2387-2389`, bytes 105290..105376 | `trackedEffect(compute, options);`. rc.6 spread `{ ...options, name: … }` here; rc.9 labels the node inside `trackedEffect` instead. |
| `observe` | `dist/observe/signals.js:234-236`, bytes 9773..9841 | byte-identical to prod |

#### 3.2 Call-event walk

`trackedEffect` is `dist/dev.js:1802-1864`, `dist/prod/core/effect.js:144-178` and
`dist/observe/core/effect.js:151-188`.

| Callee | Reach | Disposition | Reads? |
| --- | --- | --- | --- |
| `computed(() => {…}, { ...options, lazy: true })` | always | local | `setupComputedNode` sees `lazy` and skips `recompute`. The archive arrow (`prevCleanup?.()`, `staleValues(fn)`) does not run at the call event. The spread of `options` is on the caller's object. |
| field writes (`_cleanup`, `_config`, `_modified`, `_type`, `_run`; dev and observe also `_name`) | always | local | No |
| `enqueueSub(node)` (`dev-shared.js:2508-2512`) → `queueFor`, `insertIntoHeap` | always | local | Heap insertion. **New in rc.9**; rc.6 called `node._queue.enqueue(EFFECT_USER, run)`. No read. |
| `schedule()` | always | local → host `queueMicrotask(flush)` | No |
| dev: `reportDiagnostic(emitDiagnostic({code: "NO_OWNER_EFFECT", …}))` | cond: no owner | local / host (console) | No |

The later run (`run` → `recompute(node)` → the archive arrow →
`staleValues(fn)`) executes the caller's `fn`; those reads are the caller's.

#### 3.3 What changed from rc.6

The first run now rides the heap (`enqueueSub` + `schedule()`) rather than the
user-effect queue, so it runs after that pass's staged writes commit. The dev
and observe nodes are labelled by kind inside `trackedEffect`. None of these adds a read site.

**Per-build verdict.** prod, dev and observe: closed.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createTrackedEffect, Reads)` denies
that one invocation of `createTrackedEffect`, at its call event, observes a
reactive source through its own code. It creates a lazy leaf computation and
queues its first run; what that computation later reads is its caller's `fn`.
On `solid-js@2.0.0-rc.9`'s server builds the body is `getOwner()` plus
`getNextChildId` (§ 0.5), which reads nothing.

### 4. `createTrackedEffect` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The walk is § 3.2. Its host reaches are `queueMicrotask` through `schedule()`
and dev's console diagnostic. The queued `run` is archive code. It runs the
caller's previous cleanup and `fn`, and stores the returned cleanup (a
`cleanups` item; dev throws on an invalid return). The leaf owner is owner
production. § 0.3 bounds everything else. The `solid-js@2.0.0-rc.9` server body
registers nothing (§ 0.5).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createTrackedEffect, Creates)`
denies that one invocation of `createTrackedEffect` registers a version-1
resource into a browser document or a server runtime, at the call or in the
queued run it schedules.

### 5. `onSettled` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

#### 5.1 Definitions

| Build | Definition |
| --- | --- |
| `default` | `dist/prod/signals.js:700-717`, bytes 30911..32079 |
| `test`/`development` | `dist/dev.js:2894-2924`, bytes 127274..129069 |
| `observe` | `dist/observe/signals.js:704-723`, bytes 31027..32219 |

prod, with its comments elided:

```js
function onSettled(e) {
    const t = getOwner();
    t && !(t.T & CONFIG_CHILDREN_FORBIDDEN) ? trackedEffect(() => untrack(e), undefined) : globalQueue.enqueue(EFFECT_USER, function fire() {
        if (dirtyQueue.EE >= dirtyQueue.et) return globalQueue.enqueue(EFFECT_USER, fire);
        e();
    });
}
```

Dev and observe pass `{ name: "onSettled" }`. Dev's unowned `fire` checks the
callback's return, and on a returned cleanup runs `emitDiagnostic` and throws
`SETTLED_CLEANUP_UNOWNED`, as on rc.6.

#### 5.2 Call-event walk

| Callee | Reach | Disposition | Reads? |
| --- | --- | --- | --- |
| `getOwner()` | always | local (`return context`) | No |
| `trackedEffect(() => untrack(e), …)` | cond: owned, not children-forbidden | local, § 3.2 | No. The arrow and `untrack` run only in the later execution. |
| `globalQueue.enqueue(EFFECT_USER, fire)` | cond: unowned or children-forbidden owner | local, `Queue.enqueue` + `schedule()` | No |

`fire` runs later. It compares two heap bounds of the module `dirtyQueue`
(record fields, not a reactive source), re-enqueues itself while the heap has
work, and then calls the caller's callback.

#### 5.3 What changed from rc.6

- The owned arm calls `trackedEffect` directly; rc.6 went through
  `createTrackedEffect`.
- The unowned arm's `fire` defers itself while the heap has work (#3411).

No read site is added.

**Per-build verdict.** prod, dev and observe: closed.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, onSettled, Reads)` denies that one
invocation of `onSettled`, at its call event, observes a reactive source through
its own code. What the callback later reads is its caller's. The
`solid-js@2.0.0-rc.9` server body reads nothing (§ 0.5).

### 6. `onSettled` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

Both arms reduce to § 3.2's closure, or to `Queue.enqueue` → `schedule()` →
`queueMicrotask(flush)`. The later run calls `untrack(e)`, which may reach the
**hook** `GlobalQueue._externalUntrack`, or the caller's callback directly.
Dev's unowned arm adds `emitDiagnostic` + `throw` (console only). § 0.3 bounds
the rest. The `solid-js@2.0.0-rc.9` server body registers nothing (§ 0.5).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, onSettled, Creates)` denies that one
invocation of `onSettled` registers a version-1 resource into a browser document
or a server runtime, at the call or when its callback later runs.

### 7. `flush` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

#### 7.1 Definitions

| Build | Definition |
| --- | --- |
| `default` | `dist/prod/core/scheduler.js:1174-1211`, bytes 57644..59114 |
| `test`/`development` | `dist/dev-shared.js:2202-2290`, bytes 103188..107202 |
| `observe` | `dist/observe/core/scheduler.js:1185-1232`, bytes 58268..60335 |

prod, with its comments elided:

```js
function flush(e) {
    if (actionStepDepth > 0) { return e ? e() : undefined; }
    if (e) { syncDepth++; try { return e(); } finally { try { flush(); } finally { syncDepth--; } } }
    if (globalQueue.Kt) { return; }
    if (halted) return;
    while (scheduled || activeTransition) { globalQueue.flush(); }
    origin = 0;
}
```

Dev throws `FLUSH_IN_ACTION` inside an action step (new in rc.8, the dialect's
SC2006). Otherwise it keeps rc.6's re-entrancy throw and
`FLUSH_IN_EFFECT_CALLBACK` diagnostic (now `reportDiagnostic(emitDiagnostic(…))`),
the `1e5` loop guard, and `attrHooks.flushEnd()` (**hook**) after a drain.
Observe matches prod plus the attribution hook.

#### 7.2 Call-stack walk

| Callee | Disposition | Reads? |
| --- | --- | --- |
| `e()` | **caller** | The caller's |
| `GlobalQueue.flush()` (`dev-shared.js:1592-`, prod `scheduler.js:567-`) | local | New in rc.9: `resyncUnflushedCompanions` (`dev-shared.js:5534-5554`: flag resets and `GlobalQueue._syncCompanions` over node fields), joining `batchJoins` (`initTransition`), `GlobalQueue._endOptimism` (`dist/dev.js:608-643`: transition and node-record predicates, `node._equals`, `supersedeOverride`), and the `wokenTransitions` wake in `finally`. The rest is rc.6's drain: heap runs, transition completion, commits, queue runs, dev `DEV.hooks.onUpdate` (**hook**). Node-record field inspection is graph bookkeeping, not an observation (rc.6 audit group D § 1.3, judgement 1). |
| `recompute` of dirty nodes → `node._fn`; queued effects | **drain**: registered by other invocations | Theirs ([Decision 2026-09-10]) |
| `CollectionQueue.run` (`read` of its own `_disabled`/`_collapsed`), `RevealController`'s evaluate method, and the new `CollectionQueue._readOn` (`dist/dev.js:9049-`, which calls the boundary creator's `on`) | **drain**: boundary queues that `createLoadingBoundary`/`createErrorBoundary`/`createRevealOrder` registered | The boundary's reads, as on rc.3 and rc.6 |

rc.6's patch-channel drain (`drainApplyQueue`/`applyEntries`) does not exist in
rc.9 (§ 0.6).

#### 7.3 What changed from rc.6

- The `FLUSH_IN_ACTION` guard, the `origin` reset and the attribution hook were
  added.
- `GlobalQueue.flush` adds the companion resync, batch joins, optimism end and
  parked-transaction wake. All of them are writes and bookkeeping over
  scheduler and node records.

Every read-capable path still hands control to a callable some other invocation
registered.

**Per-build verdict.** prod, dev and observe: closed.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, flush, Reads)` denies that `flush`'s
own code observes a reactive source during one invocation. The reads its drain
performs belong to the computations, effects and boundary queues that other
invocations registered, and to the caller's `fn`. The `solid-js@2.0.0-rc.9`
server `flush` is empty (§ 0.5).

### 8. `flush` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

`flush`'s own code, and every archive function its drain reaches, stay inside an
archive with no handle to a browser document or a server runtime (§ 0.3). Its
host touches are:

- `queueMicrotask(flush)` through `schedule()`;
- `MicrotaskQueue.enqueue`;
- console output (`notifyHalted`, dev diagnostics);
- on the effect error path of a drained effect, `haltReactivity`'s
  `globalThis.reportError`, which reports an error and registers no version-1
  resource.

Anything a drained computation or effect does is the body of a callable another
invocation registered.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, flush, Creates)` denies that one
invocation of `flush`, through its own code, registers a version-1 resource into
a browser document or a server runtime.

---

## rc.9 parity, group C — `@solidjs/signals` store and optimistic rows

### 0. Group notes

- The `reads` line is the rc.6 audit's group C § 0.6. A read is either an
  engine read-path call (`read`/`readNodeFast`), or a string-key, `ownKeys` or
  `has` trap on a store or projection proxy **the export created**, executed by
  code the export authored.
- Brand lookups (`[$TARGET]`, `[$PROXY]`, `[$REFRESH]`) and engine field access
  (`_value`, `_pendingValue`, raw backings `pb ?? v`) are not reads.
- A later, separate invocation of a returned accessor, setter or proxy is not
  this invocation's scheduled read.
- For `creates`, § 0.3 bounds every closure. The call graph of `createStore`,
  `createProjectionNext`, `createOptimisticStoreNext` and `reconcile` in every
  build reaches only these host references: `schedule`'s
  `queueMicrotask(flush)`, `MicrotaskQueue.enqueue`'s `queueMicrotask`, and
  console output.

### 1. `createStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.**

- prod `dist/prod/store/index.js:21-24`, bytes 525..676;
- dev `dist/dev.js:8286-8289`, bytes 370517..370703;
- observe `dist/observe/store/index.js:23-26`, bytes 551..702.

All three are byte-identical to rc.6's slices:

```js
function createStore(e, t, r) {
    if (typeof e === "function") return createStoreDerivedNext(e, t, r);
    return createStoreNext(e, !!t?.shallow);
}
```

**Walk.**

- `createStoreNext` (dev `dist/dev.js:6551-6580`) runs:
  - the dev shallow checks (`lookupTarget$1`, the `$TARGET` brand, `throw`);
  - `wrapNext` → `createTarget`, which allocates the target and
    `new Proxy(t, traps)` and invokes nothing;
  - for a shallow store, `markRawIngest` over the caller's value;
  - dev `registerGraph` (**hook** `onGraph`), and when attribution is installed,
    `storeOwners.set` (a module `WeakMap`);
  - the setter closure, which is not invoked.
- `createStoreDerivedNext` (code-identical to rc.6) is § 2's walk plus a setter
  closure.

**Later.** The setter (`storeSetterNext`) runs the caller's `fn(draft)`, write
notification and adoption. rc.9 adds dev `devGuardStoreSetterResult` and
`stageHeldAdoptions`. The projection computed recomputes, and flush-time fold
commits run. All of it stays inside the archive. There is no patch channel to
emit through (§ 0.6).

**What changed from rc.6.** The patch channel is gone (§ 0.6), and the store
tree changed as § 0.6 lists. None of it adds a host reach.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createStore, Creates)` denies that
one invocation of `createStore`, in either overload, registers a version-1
resource into a browser document or a server runtime, including through the
writes or recomputes it schedules.

### 2. `createProjection` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.** `createProjectionNext`, exported as `createProjection`
(`DEFINITION_ALIASES`):

- prod `dist/prod/store/next/projection.js:196-198`, bytes 7209..7307;
- dev `dist/dev.js:7454-7456`, bytes 334649..334765;
- observe `dist/observe/store/next/projection.js:202-204`, bytes 7303..7401.

All three are byte-identical to rc.6's:
`return createProjectionNextInternal(e, t, r).store;`

**Walk.** `createProjectionNextInternal` (code-identical to rc.6) runs:

- `wrapNext`;
- for a shallow projection, the brand write plus `markRawIngest`;
- `computed(…)`, whose first run (unless lazy) calls `runProjectionComputedNext`;
- field writes.

`runProjectionComputedNext` (`dist/dev.js:7477-7514`) builds the draft
(`wrapDraft`, a `new Proxy`), runs the **caller**'s derive through
`storeSetterNext`, attaches `handleAsync` to the caller's result, and commits
through `reconcileNextState`. rc.9 replaces the loading-window shadow's
`JSON.parse(JSON.stringify(…))` with `cloneState` (`:7474`). Everything else is
adoption writes and `schedule()`. § 0.3 bounds the rest.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createProjection, Creates)` denies
that one invocation of `createProjection` registers a version-1 resource into a
browser document or a server runtime, including through the recomputes and
async landings of the projection computation it creates.

### 3. `createOptimistic` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.**

- prod `dist/prod/signals.js:593-605`, bytes 26484..27031;
- dev `dist/dev.js:2786-2799`, bytes 122783..123391;
- observe `dist/observe/signals.js:596-609`, bytes 26566..27147.

The prod and dev slices are byte-identical to rc.6's. Observe is prod's body
plus `registerGraph(r, getOwner())` in the value overload (a field write,
§ group A).

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

**At the call event.**

| Call | Disposition | Read? |
| --- | --- | --- |
| `installOptimisticEngine()` (`dist/dev.js:913-933`) | local; writes `GlobalQueue` slots (rc.9 adds `_supersedeOverride`, `_endOptimism`, `_overrideRead`, `_laneOverride`, `_landOnOverride`, `_laneLive`) | no |
| `optimisticComputed` → `computed` → `setupComputedNode` → `recompute(self, true)` unless lazy | local engine. The compute is the **caller**'s; `_wireExternalSource` is a **hook** | the caller's reads are excluded; the engine code on this path makes no read-path call (closure check below) |
| `optimisticSignal` → `signal` | local; allocates a node | no |
| `accessor(n)` | `read.bind` invokes nothing | no |
| `setSignal.bind(null, n)` | builtin `bind` | no |

`optimisticComputed` and `optimisticSignal` are code-identical to rc.6. With
`wireExternalSource` and `flush` cut as in group B, the call graph reaches no
read site other than `accessor`'s `read.bind`, in prod (117 functions), observe
(126) and dev (130).

**Scheduled later.** The returned setter's `setSignal` closure (57/63/68
functions in prod/observe/dev) contains no read site either. rc.9's `setSignal`
adds batch joins, `_landOnOverride` for a projection-write landing,
`stashHeldRewrite`, `notePromotedWrite` and the unflushed-companion push: all
writes and bookkeeping.

**Not denied**, per the § 0 line: a later, separate call of the returned
accessor calls `read(node)`. This row still rests on the accessor line (owner
flag 2 of the rc.6 audit).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createOptimistic, Reads)` denies
that one invocation of `createOptimistic` performs or schedules any read of a
reactive source it authored. It does not deny the reads the caller's compute
performs, or the read a later invocation of the returned accessor performs.
`createOptimistic` is `solid-js`'s own declaration in rc.9, so no server pairing
applies.

### 4. `createOptimistic` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The definition and walk are § 3. The call allocates a signal or computed node,
links it under the owner, installs engine slots (module state, which § creates
excludes), and returns bound functions. The optimistic write path's transitions
(`globalQueue.initTransition`, lanes) are the archive's own scheduler objects.
The only host reaches are `queueMicrotask(flush)` and console output.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createOptimistic, Creates)` denies
that one invocation of `createOptimistic` registers a version-1 resource into a
browser document or a server runtime.

### 5. `createOptimisticStore` — `reads` — archive `@solidjs/signals@2.0.0-rc.9` — **WITHHOLD**

**Definition.** `createOptimisticStoreNext`:

- prod `dist/prod/store/next/optimistic.js:144-276`, bytes 6996..13900;
- dev `dist/dev.js:7635-7783`, bytes 342913..349941;
- observe `dist/observe/store/next/optimistic.js:146-282`, bytes 7025..14010.

None is byte-identical to rc.6's.

**The rc.6 landing router is still there, in all three builds.**

```js
// dist/dev.js:7690-7695 (prod store/next/optimistic.js:194-199, observe :196-201)
const wrapCommit = (write, value) => {
  const txn = retainingTransition(fam);
  if (txn !== null) return void stageLanding(fam, txn, value);
  if (getOwner() !== fam.node) enterFlightTransition();
  runAuthoritative(write);
};
```

```js
// dist/dev.js:7814-7826 (prod :306-310, observe :312-316)
function stageLanding(fam, txn, incoming) {
  runFolded(txn, () =>
    runAuthoritative(() =>
      storeSetterNext(fam.px, draft => { stagedApply(draft, unwrapValue(incoming), fam.key ?? null); }, false)
    )
  );
}
```

- `fam.px` is the store proxy this invocation created and returns (prod `:159`,
  `s.px = l`).
- `storeSetterNext(proxy, fn)` calls `fn` with that proxy.
- `stagedApply` (`dist/dev.js:7856-`, prod `:339-437`, observe `:345-`) reads
  through it: `cur.length` (`:7864`), `unwrapValue(cur[j])` (`:7866`),
  `unwrapValue(cur[i])` (`:7911`, `:7914`, `:7921`) and `Reflect.ownKeys(cur)`
  on the object arm.
- `wrapCommit` is handed to `runProjectionComputedNext` by the export's own
  computed (prod `:258`).
- The reach condition is rc.6's. `retainingTransition(fam)`
  (`dist/dev.js:7794-7804`) is non-null once the returned setter has recorded
  the ambient transaction (`(fam.rt ??= new Set()).add(txn)`, dev `:7780`, prod
  `:274`, observe `:280`) and it is still live. A later commit of the call's
  own derived computation then lands through `stageLanding`.

That is code the export authored, run by its own computation at a later `at`
event. It observes the current (staged) value of a reactive source the export
created. The read is untracked (the trap runs in draft mode), and § reads still
counts it. It is also guarded, and a flat row has nowhere to put the guard.

**What changed from rc.6.** `stageLanding` is code-identical, and
`retainingTransition` differs only in liveness bookkeeping. `stagedApply`'s only
change is that it skips the `$OWNER` key. The withholding reason carries over
unchanged. rc.3's `wrapCommit = write => { runAuthoritative(write); consume(); }`
had none of this code, which is why rc.3's row stands and rc.9's cannot.

**Sign-off (withholding).** On rc.9, as on rc.6,
`createOptimisticStore(fn, …)`'s own landing router observes the store it
created through that store's proxy, at a later event its own computation
causes, under a retaining-transaction guard. The denial cannot be established,
and the row is not carried for rc.9.

### 6. `createOptimisticStore` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The definition and walk are § 5. The call:

- installs engine slots (`installOptimisticEngine`, `installNextBlockedHalf`,
  prod `:63-`, and `GlobalQueue._clearOptimisticStores`/`_transitionBlocked`),
  all module state;
- allocates targets, proxies, a family record and, for the derived overload, a
  computed;
- creates the archive's own scheduler transitions in `enterFlightTransition`
  and `declareFlight` (prod `:179-`, `:228-`).

`runFolded` and the `stageLanding` writes are internal. The only host reaches
are `queueMicrotask(flush)` and console output (§ 0). There are no patch
emissions in rc.9 (§ 0.6).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, createOptimisticStore, Creates)`
denies that one invocation of `createOptimisticStore`, in either overload,
registers a version-1 resource into a browser document or a server runtime,
including through its later landings.

### 7. `reconcile` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.**

- prod `dist/prod/store/index.js:26-28`, bytes 678..758;
- dev `dist/dev.js:8290-8292`, bytes 370704..370802;
- observe `dist/observe/store/index.js:28-30`, bytes 704..784.

All three are byte-identical to rc.6's:
`function reconcile(e, t = "id") { return r => reconcileNextState(e, r, t); }`

**At the call event.** A default-parameter evaluation and a closure. No call and
no property access.

**On application.** `reconcileNextState` (`dist/dev.js:6917-6972`):

- `state?.[$TARGET]` is a brand lookup; `t.px !== state` is identity;
- `materializePB`;
- the key function applied to raw backings (`t.pb ?? t.v`) and to the caller's
  unwrapped value;
- `adoptPB`, `optHooks.applyTentative` for optimistic families, and
  `applyAdopt`, which work on raw rows through lookup maps and notify with
  `setSignal` writes.

rc.9 adds the `$OWNER` disown (`delete out[$OWNER]` on a raw backing).

With `wireExternalSource` and `flush` cut, the call graph from
`reconcileNextState` reaches no read-path call in prod (115 functions), observe
(123) or dev (128). The proxies it can touch are the `state` handed to the
returned function and any proxies inside the caller's `value`. Both are
parameter-rooted, and so the caller's reads ([Decision 2026-09-10]). That
includes proxies the passed store already holds in its raw backing, which are
reached only through the caller's store; rc.6 stated the carve-out for the
passed store and the caller's value, and this applies it, on the same argument,
one level into the store the caller handed over (§ 8 residuals). `reconcile`
creates no reactive source and imports none it reads.

On `solid-js@2.0.0-rc.9`'s server builds, every property access in the returned
function has the caller's `state` or `value` as its receiver (§ 0.5).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, reconcile, Reads)` denies that one
invocation of `reconcile`, or the later application of the function it returns,
reads any reactive source `reconcile` created or imported. It does not deny
reads of the store handed to that function or of the caller's `value`.

### 8. `reconcile` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The call creates a closure. Its application performs adoption writes,
`setSignal` notifications and `schedule()` → `queueMicrotask(flush)`. There is
no patch emission in rc.9 (§ 0.6). The `solid-js@2.0.0-rc.9` server body only
calls `setProperty` on the caller's `state` (§ 0.5).

**Verdict: GRANT** (prod, dev, observe, and the `solid-js` server path).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, reconcile, Creates)` denies that one
invocation of `reconcile`, or the later application of the function it returns,
registers a version-1 resource into a browser document or a server runtime.

---

## rc.9 parity, group D — `action` (`reads`, `creates`), `snapshot` (`creates`)

### 1. `action` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.**

- prod `dist/prod/core/action.js:94-165`, bytes 4429..7642;
- dev `dist/dev.js:1961-2066`, bytes 87962..92525;
- observe `dist/observe/core/action.js:98-175`, bytes 4502..8183.

The claim's scope is rc.6's: the call event, which only returns an arrow
(`return (...t) => new Promise(…)` in prod and observe, `return (...args) => {`
in dev), and, under the wider scope the rc.3 summary already uses, every run of
the returned wrapper, its resumptions and the flushes it drives.

**The wrapper's frames, rc.6 → rc.9 (dev diff).**

- `const seq = ++actionSeq; setOrigin(seq);` (`dev-shared.js:1918-1922`, a
  module variable swap).
- `ctx._acted = true` on the transition.
- `enterActionStep()`/`exitActionStep()` around the generator step
  (`dev-shared.js:1054-1059`, a module counter).
- `attrHooks.actionStepStart/End` (**hook**, attribution engine; observe and
  dev).
- `restoreTransition(seq, ctx, fn)` (`dist/dev.js:1879-1890`, prod
  `action.js:12-23`) saves and restores `origin`, and flushes only when
  `actionStepDepth === 0`.

Everything else is rc.6's:

- dev's `ACTION_CALLED_IN_OWNED_SCOPE` guard (`getOwner`, `emitDiagnostic`,
  `throw`);
- `new Promise` (builtin);
- the caller's generator `genFn(...args)`, `it.next`/`it.throw` (**caller**);
- `isThenable` and `.then` on the iterator result and yielded values
  (caller-supplied receivers);
- `globalQueue.initTransition`, `currentTransition`, the array
  `push`/`indexOf`/`splice` on the transition's actions, and `schedule()`.

**Scheduler frames.** `GlobalQueue.initTransition` (`dev-shared.js:1773-`)
restores the outgoing transition's stashed queues. On adoption, it commits a
pending node whose staged value `_equals` its committed one
(`commitPendingNode`). These are node-record comparisons and writes, the rc.6
judgement 1 kind. `mergeTransitionState` adds the attribution hook, the `_acted`
merge and `_contested`, and loses rc.6's held-patch merge. `createBatch` gains
`_contested`. `currentTransition` and `isThenable` are code-identical.

`flush` and `GlobalQueue.flush` are B7. With the boundary-queue methods and
`wireExternalSource` cut, the call graph from `action` reaches no read site in
prod (138 functions), observe (144) or dev (193). `action` creates no reactive
source: its only created thing is a transition batch.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, action, Reads)` denies that one
invocation of `action`, at its call event and on every later run of the wrapper
it returns (including the flushes it drives), performs any observation of a
reactive source that `action` authored. What the caller's generator and its
yielded values read, and what drained registrants read, is theirs. The
`solid-js@2.0.0-rc.9` server `action` is `return fn` (§ 0.5).

### 2. `action` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

The host touches reachable from the wrapper are:

- the `new Promise` handed back to the caller (prod `action.js:95`, observe
  `:99`, dev `:1985`);
- `queueMicrotask(flush)` in `schedule()`;
- `notifyHalted`'s `console.error`;
- in dev, the diagnostics' console output.

The call graph also lists, in dev only, `resolve`, `runEffect`,
`haltReactivity` and `reportClientError`. Those come through the spurious
`resolve` edge (§ 0.4): the promise executor's parameter is named `resolve`,
and the tool links it to the module export. None is reachable from `action`'s
own frames. None of these touches registers a version-1 resource, and § 0.3
bounds the rest. The `solid-js@2.0.0-rc.9` server `action` returns `fn`
(§ 0.5).

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, action, Creates)` denies that one
invocation of `action`, or any later run of the wrapper it returns, registers a
version-1 resource into a browser document or a server runtime.

### 3. `snapshot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

**Definition.**

- prod `dist/prod/store/index.js:30-32`, bytes 760..813;
- dev `dist/dev.js:8293-8295`, bytes 370803..370862;
- observe `dist/observe/store/index.js:32-34`, bytes 786..839.

All three are byte-identical to rc.6's:
`function snapshot(e) { return snapshotNext(e); }`

**Walk.** `snapshotNext` (code-identical to rc.6) → `snapshotWalk`
(`dist/dev.js:6784-`, 112 lines against rc.6's 101). It does:

- brand and map lookups;
- `pendingBackingVisible` (`:5905-5928`);
- `materializePB`/`cloneRaw` and `isWrappable`;
- `optHooks.optimisticView`;
- property-descriptor builtins.

Getters, traps and iterators on the caller's data are **caller**.

**New in rc.9.** When a reader context exists and a store's fold is held by a
live transaction, `pendingBackingVisible` → `holdVisible` (`:4731-4739`) →
`enterStagedRead(null, txn)` (`dev-shared.js:5382-5414`) can enter that
transaction with `globalQueue.initTransition(t)`, and so reach `schedule()` →
`queueMicrotask(flush)`. On the adoption path it can reach `commitPendingNode`
and `disposeChildren`. rc.6's `snapshot` touched no host API at all. rc.9's
touches exactly one, the archive's own flush microtask, and registers nothing
outside the archive.

**`solid-js` pairing.** `solid-js@2.0.0-rc.9`'s `server.js:2` (and its dev and
observe siblings) re-export `snapshot` from `@solidjs/signals`, so every
condition runs these bytes.

**Verdict: GRANT** (prod, dev, observe).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, snapshot, Creates)` denies that one
invocation of `snapshot` registers a version-1 resource into a browser document
or a server runtime. It resolves proxies through target and family maps, may
materialize a store's own pending backing, may enter the transaction that holds
that backing (scheduling the archive's flush), and returns a plain copy or the
original objects.

---

## 6. Citations, and how they were computed

Byte offsets of the whole file, read in binary from the extracted tarball. Each
slice's sha256 was computed, and each slice was checked to begin with the
export's own definition (`function <export>(`, or the local name
`DEFINITION_ALIASES` names, after an optional ` */ `). The conventions are the
rc.6 rows', row for row:

- groups A, B and D and `snapshot` end at the start of the line after the
  closing `}`;
- group C (`createStore`, `createProjection`, `createOptimistic`,
  `createOptimisticStore`, `reconcile`) ends just after the closing `}`.

The script reproduced the rc.6 slice digests `solid_2.rs` carries under both
conventions (for example `createStore` prod `525..676` → `5ccb9272…`,
`createMemo` prod `3016..3083` → `f0f1d1a8…`).

| Export | `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | = rc.6 slice? |
| --- | --- | --- | --- | --- | --- | --- |
| createSignal | dist/prod/signals.js | 67-75 | 2747 | 3027 | `ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388` | yes |
| createSignal | dist/dev.js | 2209-2218 | 97659 | 98008 | `d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504` | yes |
| createSignal | dist/observe/signals.js | 69-78 | 2795 | 3109 | `0366fccdf03f70ca98cd1678d74800abeed9109c5499c65c626ee6868e6f565f` | no (`registerGraph`) |
| createMemo | dist/prod/signals.js | 77-79 | 3028 | 3095 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | yes |
| createMemo | dist/dev.js | 2219-2221 | 98008 | 98097 | `46130d23b4ce3e9a8bfbe79e6ea1920b95f064c446f61e65327e08305592e3f0` | yes |
| createMemo | dist/observe/signals.js | 80-82 | 3110 | 3177 | `f0f1d1a80dd61e1c62139a524f277d95e34957723fc71a02d9b9996e26cf5951` | yes (rc.6 prod) |
| createTrackedEffect | dist/prod/signals.js | 231-233 | 9691 | 9759 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | yes |
| createTrackedEffect | dist/dev.js | 2387-2389 | 105290 | 105376 | `59effb18726430b691223730bbeffeb8cb7c60f6b546261051389e4ee557154e` | no (no `name` spread) |
| createTrackedEffect | dist/observe/signals.js | 234-236 | 9773 | 9841 | `eb5e053187dd25e3e410d0c6755696fcbd64cf462b00940e7181f6925ee6da30` | yes (rc.6 prod) |
| onSettled | dist/prod/signals.js | 700-717 | 30911 | 32079 | `107240ae071701f70c6f27055f0a525fdcffcf0a4f98fc7b655f04758e8a43c9` | no |
| onSettled | dist/dev.js | 2894-2924 | 127274 | 129069 | `add35758c224c4e5339e7e89db7000bce0b2b92ba37a28df5042414a2719547e` | no |
| onSettled | dist/observe/signals.js | 704-723 | 31027 | 32219 | `7535e81cf25f0d998b1c69401f2225a460c6d4fd453855c905753c6a86471193` | no |
| flush | dist/prod/core/scheduler.js | 1174-1211 | 57644 | 59114 | `06c46ce4cc1dfc50fed5772ae6f737788defd6f59d26d9bc1ab81eba2d2e6536` | no |
| flush | dist/dev-shared.js | 2202-2290 | 103188 | 107202 | `b77e8fd032473ce4ab08aadd107ab2d9e56ef76452366d9f7e0151614c538cce` | no |
| flush | dist/observe/core/scheduler.js | 1185-1232 | 58268 | 60335 | `4c7dcc9dedee59eb7249051b3460fbc7a4a537fab9e489aab1293de9d4e387c0` | no |
| createStore | dist/prod/store/index.js | 21-24 | 525 | 676 | `5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4` | yes |
| createStore | dist/dev.js | 8286-8289 | 370517 | 370703 | `5ab024879afe7976504cb2b296f2e44c5f68823cd9cfc7c07c51d1dcba7d80ae` | yes |
| createStore | dist/observe/store/index.js | 23-26 | 551 | 702 | `5ccb92725a12c3fe4c8eacf07bc49e364f976718010a237a995e0794377c7ff4` | yes (rc.6 prod) |
| createProjection | dist/prod/store/next/projection.js | 196-198 | 7209 | 7307 | `bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7` | yes |
| createProjection | dist/dev.js | 7454-7456 | 334649 | 334765 | `e1264ce5581a8fe3610fd0f676925fcb8f30fc2bc21983c0a5b712952b220e32` | yes |
| createProjection | dist/observe/store/next/projection.js | 202-204 | 7303 | 7401 | `bcbe84ac16a3f08d0d7aca97559146257937d7387ca7a0d10af5096c18e045d7` | yes (rc.6 prod) |
| createOptimistic | dist/prod/signals.js | 593-605 | 26484 | 27031 | `09fd4865502a92243213a9a3162d7e6a99bed5d6456d0395e5d425d44161f189` | yes |
| createOptimistic | dist/dev.js | 2786-2799 | 122783 | 123391 | `9180aeea6599b0c64fdfb3859e8c1802ace64c405e0d05ff952bc44e63ad7be3` | yes |
| createOptimistic | dist/observe/signals.js | 596-609 | 26566 | 27147 | `760ee92d8393a3d2102625aadc0c31e9378285b46bc98c4af9b2e0fa85697fe1` | no (`registerGraph`) |
| createOptimisticStore | dist/prod/store/next/optimistic.js | 144-276 | 6996 | 13900 | `803fcb9cc11f9c2c503c6d6f86a6d82434f3655b8a1264fff1d3ed2d36b6a0a0` | no |
| createOptimisticStore | dist/dev.js | 7635-7783 | 342913 | 349941 | `bc5b1564c404e2ba7b688f6f44bb56fc3b6a211b0b055c935b1561d9f7474cc6` | no |
| createOptimisticStore | dist/observe/store/next/optimistic.js | 146-282 | 7025 | 14010 | `ccd36eaea185d28df29b219512185424d91c1688bd6dd7b67efae16123719c30` | no |
| reconcile | dist/prod/store/index.js | 26-28 | 678 | 758 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | yes |
| reconcile | dist/dev.js | 8290-8292 | 370704 | 370802 | `572cd0645c23a657a2116f3609d220d042405d28255c1fefb439573572dbfc9c` | yes |
| reconcile | dist/observe/store/index.js | 28-30 | 704 | 784 | `d99b5e9158dfd8f7042e5b5b8aa40be80ee52ab0c1e6c95d7f3ea18b0d5d9099` | yes (rc.6 prod) |
| action | dist/prod/core/action.js | 94-165 | 4429 | 7642 | `92babceb624fbc7aff9607a87d8bb7d36fd8be20ef96a92d4d3c6f32d594f422` | no |
| action | dist/dev.js | 1961-2066 | 87962 | 92525 | `6b9bd50c0f6bdfe85705563b2a76f7e146e2db559fce629be832289347893968` | no |
| action | dist/observe/core/action.js | 98-175 | 4502 | 8183 | `a503cb661462aed999de082d197ef9d229bd476655f176a73b1fce20603210ea` | no |
| snapshot | dist/prod/store/index.js | 30-32 | 760 | 813 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` | yes |
| snapshot | dist/dev.js | 8293-8295 | 370803 | 370862 | `eebc4206bc3509952fd02519f13f9b58ed00bc63da5658c90322c97bd221d6be` | yes |
| snapshot | dist/observe/store/index.js | 32-34 | 786 | 839 | `3bb31686d1eba591133042dd4c7c95515074c440f6d532da34f25cbf6c05bfd2` | yes (rc.6 prod) |

The `createOptimisticStore` citations serve its `creates` row only; the
withheld `reads` row carries none. The slices are checked in under
`rust/crates/solid-dialect/audited-slices/solid-v2/rc9/solidjs-signals/`.

**Caution on the "= rc.6 slice?" column.** A slice digest that matches rc.6's
pins only the *subject*, not the callee closure, which changed substantially.
The verdicts rest on the rc.9 walks above.

## 7. What this changes, and what it cannot move

- **Rows.** 19 new `@solidjs/signals@2.0.0-rc.9` rows (12 `creates`, 7 `reads`).
  The table goes from 80 to 99 rows (`creates` 54 → 66, `reads` 26 → 33); rc.9
  carries 24 (17 `creates`, 7 `reads`), exactly rc.6's set.
  `createOptimisticStore` `reads` is recorded as withheld in the dialect test's
  implementation-audit list.
- **The delegate gap closes.** With rc.9 `createSignal` `creates` granted, every
  audited `@solidjs/signals` archive answers both delegates of the scoped
  `solid-js@2.0.0-rc.3` `createSignal` row. The row can now bind beside rc.9,
  and `DELEGATE_GAPS` is empty.
- **The proposal side does not move.** `some_audit_denies_primitive` is
  version-blind, and every `(export, domain)` pair here was already denied on
  rc.3 and rc.6.
- **The pinned census does not move.** A row binds only through the census's
  four-field identity gate on an authenticated snapshot, and, as the five-row
  audit's § 7 records, no census row or checked-in environment installs
  `@solidjs/signals@2.0.0-rc.9`.

## 8. Residual approximations

- The call graphs are name-based over-approximations (§ 0.4). Dispatch through
  node fields and caller values is dispositioned by hand, not proved.
- C7 applies the parameter-rooted carve-out to proxies the caller's store
  already holds in its backing, not only to the store and the caller's value.
- The node-record judgement (field inspection and `_equals` comparisons inside
  the scheduler are bookkeeping, not observations) is rc.6's judgement 1,
  applied to rc.9's new `initTransition` commit, `endOptimism` and companion
  resync.
- The `solid-js` pairing covers `solid-js@2.0.0-rc.9` (read here) and
  `solid-js@2.0.0-rc.3` (read by the rc.6 audit). No other `solid-js` is
  covered.
- Found about the five-row audit while reading (no verdict moves): its § 0.3
  places the `globalThis.process?.env?.COMPANION_CENSUS` read in
  `devCheckFlushStart`/`devCheckActiveOverrides`. It is a top-level statement
  evaluated at module load (`prod/core/invariants.js:28`,
  `observe/core/invariants.js:30`, `dev-shared.js:801`), and those two functions
  return immediately. Its host list (and rc.6's) also omits the `Node` global
  that `isWrappable` reads (`prod/store/store.js:107`,
  `observe/store/store.js:109`, `dist/dev.js:3067`), a predicate read that
  registers nothing. rc.9's dev build also still declares and calls the slot
  `GlobalQueue._drainPatchOptimistic` (`dev-shared.js:1536`, `dist/dev.js:723`)
  without ever assigning it, a leftover of the removed patch channel that
  reaches nothing.
