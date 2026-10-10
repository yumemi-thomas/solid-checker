# Audit: `merge`, `omit` and `solid-js`' own `createMemo` — `creates` on the rc.9 archives

Date: 2026-09-28. Status: **for the repository owner's review**, as the rc.3,
rc.6 and rc.9 audits it follows were.

**Why it exists.** The certification metric
([`phase22/2026-09-28-certification-metric-baseline.md`](../phase22/2026-09-28-certification-metric-baseline.md),
wall 3) found `creates` held open on exports of the top 30 Solid 2 packages
because no dialect negative row answers three callees:

- `merge` and `omit`, which `solid-js` re-exports from `@solidjs/signals` and
  for which no archive carries a row;
- `createMemo` imported from `solid-js`. `@solidjs/signals`' own `createMemo`
  has rows, but `solid-js@2.0.0-rc.9` re-*declares* `createMemo`
  (`types/index.d.ts:8`, from `./client/hydration.js`), so a `solid-js` import
  binds `solid-js`' archive and none of `@solidjs/signals`' rows.

It reads the three `creates` rows on the exact bytes of
`@solidjs/signals@2.0.0-rc.9` and `solid-js@2.0.0-rc.9`, in every runtime
build each package's `exports` map can select, and grants what those bytes
support.

**Result.**

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| 1 | `@solidjs/signals` `omit` `creates` | **GRANT** | The call builds an `OmitView` record and a `Proxy` over it (or, without `Proxy`, a descriptor copy). Its closure, the returned proxy's traps included, reaches only `store/utils` helpers, `ownEnumerableKeys`, built-ins (`Map`, `Object`, `Proxy`, `Reflect`, `Set`, `Symbol`) and caller-supplied receivers, predicates and accessors. `solid-js`' server builds re-export these bytes, so every host runs them. |
| 2 | `@solidjs/signals` `merge` `creates` | **WITHHOLD** | The archive's own bodies create nothing, but `solid-js@2.0.0-rc.9`'s `node`/`worker`/`deno` builds define their own `merge` (`server.js:1222-1229`), which wraps every function source in the *server* `createMemo`, whose `processResult` reaches `ctx.serialize` (`server.js:574`, `:715`). The declaration a `solid-js` import binds is `@solidjs/signals`' (`types/index.d.ts:1`), so a flat row would answer for that server body too. |
| 3 | `solid-js` `createMemo` `creates` (flat) | **WITHHOLD** | The server `createMemo` (`server.js:321-410`) runs its compute eagerly and hands a thenable result to `processResult`, which calls `ctx.serialize(id, deferred.promise, deferStream)`: a create under [Decision 2026-09-04]. |
| 4 | `solid-js` `createMemo` `creates`, **`browser` host target** | **GRANT, scoped** | The wrapper `(_createMemo \|\| createMemo$1)(...args)` and `hydratedCreateMemo` reach exactly the 27-definition closure the core-and-web audit's § 13 walked for `createSignal`, byte-identical in `dist/solid.js`, `dist/solid.dev.js` and `dist/solid.observe.js`. Delegates: `@solidjs/signals` `createMemo`, `createSignal` and `getOwner` `creates`, all granted on rc.9. |

### What the owner is asked to agree with beyond the table

1. **`merge` is withheld for a pairing, not for its own bytes.** Nothing in
   `@solidjs/signals@2.0.0-rc.9` creates on `merge`'s path: the archive holds no
   handle to a document or a server runtime (parity audit § 0.3), and `merge`'s
   one call out of `store/utils` is the archive's own `createMemo`, whose
   `creates` row is granted (parity audit B2). The withholding is the same
   `solid-js` server split that the rc.3 core-primitives audit § 1.4 names: the
   tier binds the declaration's archive, while a `solid-js` import under `node`
   runs `solid-js`' own body. A row that could say "the importing `solid-js`
   resolves to a browser build" would be sound; § 2.3 sketches it, and it needs
   a premise the row scope does not have today.
2. **The `createMemo` row rests on § 13's dispositions.** The walk here was
   re-run mechanically and reached the same definitions with the same bytes,
   so the per-callee dispositions (host reads of `_$HY.r`, the undone
   `fetch`/`Promise` swap, `queueMicrotask`) are § 13's and are not re-argued.
   If § 13 is redrawn, both scoped rows move together.
3. **No `node` row.** The per-host certification (ADR 0140) certifies
   `node` cases, and nothing here scopes a row to `node`: both `node`-reachable
   bodies read here (`merge`, `createMemo`) reach `ctx.serialize`.

### What this audit does not do

- It reads `creates` only. `merge` and `omit` `reads` are not read: the returned
  proxies' traps read their sources, including memos `merge` created, and that
  needs its own reading under [Decision 2026-09-10].
- It reads no rc.3 or rc.6 archive. `@kobalte/core@2.0.0-alpha.2`, the
  largest `merge`/`omit` consumer in the metric, is certified on rc.3 because
  its release admits nothing else, so these rows do not reach it.

---

## 0. Shared inputs

### 0.1 Identity

The three tarballs were downloaded with `npm pack` into an isolated `$TMPDIR`
directory on 2026-09-28. `npm pack --json` reported:

| archive | integrity |
| --- | --- |
| `@solidjs/signals@2.0.0-rc.9` | `sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==` |
| `solid-js@2.0.0-rc.9` | `sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==` |
| `@solidjs/web@2.0.0-rc.9` | `sha512-pfiWoLDnLc+QYWc7UyLqO+5QrPEf3oTiNmmRC+C+uM6AZ5VH0bZMNPtLM5rJ29LKPiTwQitKV843IQDf/oeyhQ==` |

These are the audited tuples in `audited-archives.json`. Every file cited
below has the sha256 and length that
`benchmarks/package-contract-v2/phase0/rc9/{solidjs-signals,solid-js}/files.json`
pins (checked by the slicing script, § 5).

### 0.2 Builds and bindings

- **`@solidjs/signals`.** `exports["."]` selects `dist/dev.js` for
  `test`/`development`, `dist/observe/index.js` for `observe`, and
  `dist/prod/index.js` for `default` (parity audit § 0.2). `merge` and `omit`
  are defined once per build: `dist/prod/store/utils.js`,
  `dist/observe/store/utils.js`, and `dist/dev.js`.
- **`solid-js`.** `exports["."]` orders `worker`, `browser`, `deno`, `node`,
  then `development`, `observe`, `default` (core-and-web audit § 0.2):
  `browser` or no host selects `dist/solid{,.dev,.observe}.js`; `worker`,
  `deno` or `node` selects `dist/server{,.dev,.observe}.js`.
  - Every browser build re-exports both `merge` and `omit` from
    `@solidjs/signals` (`dist/solid.js:2`), and defines its own `createMemo`
    (`:770-772`).
  - Every server build re-exports `omit` from `@solidjs/signals`
    (`dist/server.js:2`) but **defines its own `merge`** (`:1222-1229`) and
    its own `createMemo` (`:321-410`).
  - The declarations: `types/index.d.ts:1` re-exports `merge` and `omit` from
    `@solidjs/signals`; `:8` re-declares `createMemo` from
    `./client/hydration.js`.
- **`@solidjs/web`** re-exports `merge as mergeProps` from `solid-js` in every
  build (`dist/web.js:2`, `dist/server.js:2`) and exports neither `omit` nor
  `createMemo`.

### 0.3 Archive-wide bounds

- `@solidjs/signals@2.0.0-rc.9`: the parity audit's § 0.3 census (re-run
  independently there) holds: no import leaves the archive, and no reachable
  code holds a handle to a browser document or a server runtime.
- `solid-js@2.0.0-rc.9`: the core-and-web audit's § 0.3 census holds: the client
  builds' host reaches are the listed hydration helpers; the server builds hand
  values to `sharedConfig.context` through `processResult`, `serverEffect`,
  `lazy`, `createProjection`, `createErrorBoundary` and `ssrLoadingBoundary`.

### 0.4 How the walks were done

A breadth-first walk over top-level definitions (acorn 8, from
`packages/cli/node_modules`), from the export's definition, following every
free identifier that names another top-level definition of the same file, and
listing the imported bindings and free globals the reached definitions use.
It over-approximates (any same-named identifier links, including a shadowed
one), and it does not follow imports, which are dispositioned by hand below.
Each reached definition's text was hashed, so "byte-identical across builds"
is measured, not read. Every frame a verdict leans on was read by hand in the
unmangled dev build and checked against the minified ones.

---

## 1. `omit` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

### 1.1 Definition

- prod `dist/prod/store/utils.js:981-1019`, bytes 40808..42411;
- dev `dist/dev.js:4379-4428`, bytes 192223..194027;
- observe `dist/observe/store/utils.js:983-1021`, bytes 40834..42437.

The prod and observe slices are byte-identical (`8b9a50f9…`).

### 1.2 What the call does

With `Proxy` (`SUPPORTS_PROXY`), the call classifies its first argument:

- a function is `SOURCE_MEMO`;
- a store or foreign proxy is `SOURCE_PROXY`, and brand lookups (`$PROXY in`,
  `[$TARGET]`, `[$OMIT]`, `[$VIEW]`) fold an `omit()` over an `omit()` (one
  record, `combineHidden` of both filters) or take a `merge()` proxy's record
  as `SOURCE_MERGE`;
- anything else is `SOURCE_PLAIN`.

It then returns `new Proxy(new OmitView(source, kind, hidden), omitTraps)`.
`OmitView`'s constructor stores three fields. Without `Proxy`, it copies the
caller's own property descriptors onto a fresh object, filtering by the
caller's key list or predicate.

### 1.3 The walk [M]

From `omit`, the walk reaches 48 top-level names in `dist/dev.js`, and the
same set less three in both minified `store/utils.js` files (45 each), which
import those three (`$PROXY`, `$TARGET`, `ownEnumerableKeys`) from
`store/store.js`; `ownEnumerableKeys`' own walk there reaches nothing further
and uses only `Object` and `Reflect`. They are:

- the view records and kinds: `OmitView`, `SOURCE_*`, `$OMIT`, `$VIEW`,
  `$SOURCES`, `EMPTY`, `MISSING`, `READS_FOR_TABLE`;
- the traps `omitTraps` (`get`, `has`, `set`/`deleteProperty` = `trueFn`,
  `getOwnPropertyDescriptor`, `ownKeys`);
- the read helpers they call: `isHidden`, `combineHidden`, `hiddenByAny`,
  `viewSource`, `omitReadTable`, `omitTable`, `tableOf`, `tableSet`,
  `tableOwnKeys`, `tableDescriptor`, `collectTable`, `mergeTable`,
  `mergeLookup`, `mergeGet`, `mergeHas`, `mergeDescriptor`,
  `mergeEnumerableKeys`, `mergeKeysOf`, `mergeHasStaticKeys`,
  `entryHasStaticKeys`, `sourceDescriptor`, `sourceEnumerableKeys`,
  `sourceHas`, `leafOf`, `leafKeys`, `collectKeys`, `addKey`,
  `accessorDescriptor`, `isView`, `propertyIsEnumerable` (`$1` in dev),
  `ownEnumerableKeys`.

Imported bindings: `SUPPORTS_PROXY`, `$PROXY`, `$TARGET` (constants) and
`ownEnumerableKeys`. Free globals in the whole closure: `Map`, `Object`,
`Proxy`, `Reflect`, `Set`, `Symbol`, `undefined`. No scheduler, owner, memo,
signal or host function is reachable, including from the traps.

### 1.4 Dispositions

| callee | reach | disposition |
| --- | --- | --- |
| `new Proxy`, `new OmitView`, `Object.*`, `Reflect.*`, `new Set`, `new Map` | call event, traps | builtin / local |
| brand lookups on `props` (`$PROXY in`, `[$TARGET]`, `[$OMIT]`, `[$VIEW]`) | call event | **caller** (a caller-supplied receiver; a store's traps are the archive's own and bounded by § 0.3) |
| `hidden(key)` (a predicate) | traps, copy path | **caller** |
| `view.source()` for `SOURCE_MEMO` (`viewSource`) | traps | **caller** (a caller-supplied accessor) |
| getters on the caller's objects, and a folded `merge()` record's sources | traps | **caller** (the record and its sources arrived through the caller's argument; whatever `merge` created is `merge`'s act, § 2) |

### 1.5 The `solid-js` pairing

`solid-js@2.0.0-rc.9` re-exports `omit` from `@solidjs/signals` in all six
builds (`dist/solid.js:2`, `dist/server.js:2` and their `dev`/`observe`
siblings), so every host condition runs the bytes read here, through
whichever of the three builds signals' own `exports` map selects.

### 1.6 Verdict

**GRANT** (prod, dev, observe, and every `solid-js` host).

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, omit, Creates)` denies that one
invocation of `omit`, or any later trap of the proxy it returns, registers a
version-1 resource into a browser document or a server runtime.

---

## 2. `merge` — `creates` — archive `@solidjs/signals@2.0.0-rc.9` — **WITHHOLD**

### 2.1 The archive's own bodies

- prod `dist/prod/store/utils.js:882-979`, bytes 36890..40806;
- dev `dist/dev.js:4277-4378`, bytes 188299..192222;
- observe `dist/observe/store/utils.js:884-981`, bytes 36916..40832.

`merge` flattens its sources into a `MergeView` and returns
`new Proxy(new MergeView(flattened, kinds), mergeTraps)`, or, without `Proxy`,
a descriptor copy. The one call out of `store/utils` is
`flattened.push(createMemo(s))` for a function source: the archive's own
`createMemo` (`../signals.js`), whose `creates` row is granted (parity audit
B2). Free globals: `Map`, `Object`, `Proxy`, `Reflect`, `Symbol`. **On its own
bytes the archive creates nothing on this path.**

### 2.2 The server body a `solid-js` import runs

`solid-js@2.0.0-rc.9`'s server builds define:

```js
function merge(...sources) {
  for (let i = 0; i < sources.length; i++) {
    if (typeof sources[i] === "function") {
      sources[i] = proxySource(createMemo(sources[i]));
    }
  }
  return merge$1(...sources);
}
```

at `dist/server.js:1222-1229` (bytes 38041..38256), `server.dev.js:1295-1302`
and `server.observe.js:1266-1273`, with one slice digest (`77ce089f…`).
`createMemo` there is the **server** `createMemo` (`server.js:321-410`,
`e72123d4…` in all three builds): unless `lazy` or `ssrSource: "client"` it
runs `update()` at creation, and a thenable result goes to `processResult`,
which calls `ctx.serialize(id, deferred.promise, deferStream)`
(`server.js:574`; `server.dev.js:645`; `server.observe.js:618`). The guard is:

- `node`/`worker`/`deno`;
- a `renderToStream` context (`ctx.async ∧ ctx.serialize`);
- a function source whose result is an unsettled thenable, and no recorded
  slot;
- an owner with an id;
- no `NoHydrate` (`serialize` is `undefined` here, never `false`).

It is reachable type-correctly: `merge<T extends unknown[]>(...sources: T)`
(`dist/types/store/utils.d.ts:215`) admits `() => Promise<…>`.

### 2.3 Why the row is withheld, and the form that would grant it

A `solid-js` import of `merge` binds the declaration in `@solidjs/signals`
(`types/index.d.ts:1`), so the census answers from `@solidjs/signals`' archive
while a `node` host runs `solid-js`' server body. A flat row cannot say
"except under that guard", so it is **withheld**. A `browser`-scoped row on
the `@solidjs/signals` archive would not fix it either: the scope's runtime
premise replays *signals'* `.`, which selects the same three builds under every
host.

The sound form would scope the row by the **re-exporting** archive: the closure
carries exactly one audited `solid-js@2.0.0-rc.9`, and its `.` resolves under
the requested set to `dist/solid{,.dev,.observe}.js`, each of which re-exports
`merge` from `@solidjs/signals`. That is a new premise kind for
`HostTargetScope`, not taken here. The metric shows no rc.9 package that the
row would reach today (the `merge` walls are `@kobalte/core`'s, on rc.3), so
the design is recorded rather than built.

**Verdict: WITHHOLD.**

---

## 3. `createMemo` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

This is the flat row. The server `createMemo` (`server.js:321-410`,
`server.dev.js:392-481`, `server.observe.js:365-454`, one slice `e72123d4…`)
creates an owner and a computation record, and, unless `lazy` or
`ssrSource === "client"`, runs `update()` at creation. `update()` passes the
compute's result to `processResult` (`server.js:516-`), which reaches
`ctx.serialize` on the sites of § 2.2 under the same guard, with
`options.deferStream`, `options.ssrSource` and `options.serialize` taken from
the caller. `createMemo(async () => …)` is type-correct.

**Verdict: WITHHOLD**, for the reason the rc.3 `createSignal` and
`createEffect` rows were withheld (2026-09-04 audit § 7.4): a guarded reach
still counts.

---

## 4. `createMemo` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`

### 4.1 Definition

```js
const createMemo = (...args) => {
  return (_createMemo || createMemo$1)(...args);
};
```

at `dist/solid.js:770-772` (bytes 23820..23905), `solid.dev.js:796-798`
(25516..25601) and `solid.observe.js:782-784` (24140..24225), with one slice
digest (`1528dcd8…`). `createMemo$1` is `@solidjs/signals`' `createMemo`
(`:1`). `_createMemo` is written only by `enableHydration` (`:704`), which
installs `hydratedCreateMemo`:

```js
function hydratedCreateMemo(compute, options) {
  if (!sharedConfig.hydrating || options?.transparent) {
    return createMemo$1(compute, options);
  }
  return hydrateSignalLike(createMemo$1, compute, options);
}
```

(`dist/solid.js:596-601`).

### 4.2 The walk [M]

From `createMemo` and `hydratedCreateMemo`, the walk reaches **27 definitions
in each of the three browser builds, every one byte-identical across them**.
The set is exactly the one the core-and-web audit § 13 walked from
`createSignal` and `hydratedCreateSignal`, with those three entries
(`createSignal`, `hydratedCreateSignal`, `_createSignal`) replaced by
`createMemo`, `hydratedCreateMemo` and `_createMemo`:

- `sharedConfig`;
- `hydrateSignalLike`, `withHydrationGate`, `markTopLevelSnapshotScope`;
- `readSerializedOrCompute`, `readHydratedValue`, `latchedOnce`, `subFetch`,
  `MockPromise`, `syncThenable`;
- `hydrateSignalFromAsyncIterable`, `normalizeIterator`,
  `forwardIteratorReturn`, `isAsyncIterable`, `hasLoadingWindow`;
- `armLiveTakeover`, `LIVE_SOURCE`, `liveGate`, `UNASKED`;
- `onHydrationEnd`, `_hydrationEndCallbacks`, `_pendingBoundaries`,
  `_hydrationDone`, `_snapshotRootOwner`.

Imported bindings reached, all from `@solidjs/signals`:

- `createMemo$1` (`createMemo`, `hydratedCreateMemo`, and as `coreFn` through
  `hydrateSignalLike`);
- `createSignal$1` (`withHydrationGate`, `armLiveTakeover`);
- `getOwner` (`hydrateSignalLike`, `markTopLevelSnapshotScope`,
  `readSerializedOrCompute`, `hydrateSignalFromAsyncIterable`);
- `peekNextChildId`, `markSnapshotScope`.

### 4.3 Dispositions

§ 13's table applies row for row, with `createMemo$1` in place of
`createSignal$1` as `coreFn`:

- `createMemo$1(compute, options)`, directly or as `coreFn(…)` at the end of
  every `hydrateSignalLike` branch: **archive (delegate)**, `@solidjs/signals`
  `createMemo`;
- `createSignal$1(false, …)` in `withHydrationGate` and `armLiveTakeover`:
  **archive (delegate)**, `@solidjs/signals` `createSignal`;
- `getOwner()`: **archive (delegate)**, `@solidjs/signals` `getOwner`;
- `sharedConfig.has`/`.load`: **hook**, installed by `@solidjs/web`'s `hydrate`
  as reads of `globalThis._$HY.r` (host reads only);
- `subFetch`'s `window.fetch`/`Promise` swap, restored in `finally`, and
  `onHydrationEnd`'s `queueMicrotask`: **host**, registering nothing that
  stays live;
- `peekNextChildId`, `markSnapshotScope`, and the bound `read`/`setSignal` of
  the gate and live signals: **archive**, non-primitive, bounded by the rc.9
  signals census (§ 0.3);
- the caller's `compute`: **caller**.

`enableHydration`'s `_$HY` writes, `watchTruncation`'s
`document.addEventListener` and `drainHydrationCallbacks`' `setTimeout` are
not reachable from `createMemo`, as they are not from `createSignal`.

### 4.4 The scope

- **Condition:** `browser`.
- **Runtime:** `dist/solid.js`, `dist/solid.dev.js`, `dist/solid.observe.js`.
  `browser` selects the first, `browser` ∧ `development` the second, `browser`
  ∧ `observe` the third; `worker` comes before `browser` in the map and selects
  `server.js`, which the row does not list, so it refuses.
- **Delegates:** `@solidjs/signals` `createMemo`, `createSignal` and `getOwner`
  `creates`: every canonical primitive the walk reaches in signals. All three
  are granted on `@solidjs/signals@2.0.0-rc.9` (parity audit B2 and A1, and the
  five-row audit for `getOwner`), and `solid-js@2.0.0-rc.9` depends on
  `@solidjs/signals@^2.0.0-rc.9`.

**GRANT, scoped.** The census terminates the row only for a certification that
requested `browser`, whose `.` resolves to one of the three files, beside an
audited rc.9 signals archive
(`census_host_target_row_binds_solid_js_rc9_create_memo_beside_signals_rc9` in
`contract_certification/type_facts.rs`).

**Sign-off.** `(solid-js@2.0.0-rc.9, createMemo, Creates)`, scoped to
`browser`, denies that one invocation of `createMemo` running
`dist/solid{,.dev,.observe}.js` registers a version-1 resource into a browser
document or a server runtime, provided the signals archive it delegates to
denies the same for `createMemo`, `createSignal` and `getOwner`.

---

## 5. Citations

Byte offsets are of the whole file, read in binary from the extracted tarball.
Each slice runs from the `function <export>(` token (for `createMemo`, the
`const createMemo = ` token) through the closing `}` inclusive (for
`createMemo`, the terminating `;`): the 2026-09-23 convention the core-and-web
audit uses. Each slice was checked to be the only definition of the name in
its file, and each file's sha256 and length to equal the `files.json` pin.

| row | `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` |
| --- | --- | --- | --- | --- | --- |
| `omit` | `dist/prod/store/utils.js` | 981-1019 | 40808 | 42411 | `8b9a50f9840bd11547765d1269cb10f5f3486b49679852d061f1906408ca9128` |
| `omit` | `dist/dev.js` | 4379-4428 | 192223 | 194027 | `578fbc1412c64da8a3fa7d5ae5c7a703eb2028cf86ee29c132a1847e91bb737a` |
| `omit` | `dist/observe/store/utils.js` | 983-1021 | 40834 | 42437 | `8b9a50f9840bd11547765d1269cb10f5f3486b49679852d061f1906408ca9128` |
| `createMemo` (`browser`) | `dist/solid.js` | 770-772 | 23820 | 23905 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` |
| `createMemo` (`browser`) | `dist/solid.dev.js` | 796-798 | 25516 | 25601 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` |
| `createMemo` (`browser`) | `dist/solid.observe.js` | 782-784 | 24140 | 24225 | `1528dcd8aeb175ac5dda5fa0477fdbe76d6a7498ef43ce7a0ab6ffc196f982f5` |

The withholdings' subjects, for reference (not row citations): signals `merge`
prod `36890..40806` (`0ba46ca4…`), dev `188299..192222` (`dd154e09…`), observe
`36916..40832` (`0ba46ca4…`); `solid-js` server `merge` `38041..38256`
(`77ce089f…`) and server `createMemo` `9388..12085` (`e72123d4…`) in
`dist/server.js`.

The six row slices are checked in under
`rust/crates/solid-dialect/audited-slices/solid-v2/rc9/{solidjs-signals,solid-js}/`.

## 6. What this changes

- **Rows.** Two: `(@solidjs/signals, 2.0.0-rc.9, omit, Creates)`
  every-condition, and `(solid-js, 2.0.0-rc.9, createMemo, Creates)` scoped to
  `browser`. Two withholdings are recorded: `@solidjs/signals` `merge`
  `creates` and the flat `solid-js` `createMemo` `creates`.
- **What they can move.** `omit` answers under every certification of an
  rc.9 tree, including the existing `["import"]` one (`@solidjs/meta` imports
  it). The `createMemo` row answers only in a certification that requested
  `browser`, which is what ADR 0140's per-host certification runs.
- **What they cannot move.** Anything certified on rc.3 or rc.6
  (`@kobalte/core`), and `merge` anywhere.

## 7. Residual approximations

- The walks are name-based over-approximations (§ 0.4); dispatch through view
  records, trap tables and caller values is dispositioned by hand.
- The `createMemo` row inherits § 13's residuals: the non-primitive signals
  reaches (`markSnapshotScope`, `peekNextChildId`, the bound `read` and
  `setSignal`) rest on the signals archive census, which no row names; and
  subscribing to a hydration payload's async iterator is read as a host read.
- The `omit` row's traps can read a `merge()` record whose memo sources a
  server `merge` created. That memo's create, if any, belongs to the `merge`
  invocation (§ 2), not to `omit`.
- The `solid-js` pairing for `omit` covers `solid-js@2.0.0-rc.9` only.
