# Audit: `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` — the negative table's `solid-js` and `@solidjs/web` rows, re-read

Date: 2026-09-27. Status: **for the repository owner's review**, as the rc.3,
rc.6 and rc.9 signals audits it follows were.

**What it covers.** It reads each of the 26 rows that
`rust/crates/solid-dialect/src/solid_2.rs` carried on 2026-09-26 for
`solid-js@2.0.0-rc.3` and `@solidjs/web@2.0.0-rc.3` (18 and 8). Each is read on
the exact bytes of `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9`, in
every runtime build each package's `exports` map can select. It grants exactly
the rows those bytes support. It lists both rc.9 archives as audited.

**Why it exists.** The owner is making `2.0.0-rc.9` the audited release. Rows
are archive-scoped (`NegativeClaimRow::version`), and no rc.9 `solid-js` or
`@solidjs/web` archive was listed, so no row answered for either. Every rc.3 row
of these two packages rests on a summary of an rc.3 bundled contract document,
and rc.9 has no bundled document. So, as in the rc.6 and rc.9 signals audits,
every rc.9 row below is an implementation reading of rc.9's own runtime bytes.
None is a carried-over rc.3 claim.

**What it found about rc.3.** Five rc.3 rows are contradicted by rc.3's own
bytes:

- `solid-js` `Show` `reads`;
- `Show` `creates`;
- `Loading` `creates`;
- `@solidjs/web` `render` `reads`;
- `hydrate` `reads`.

They were withdrawn in separate commits, one per row, in
[`2026-09-27-solid-2-rc3-show-loading-withdrawals.md`](2026-09-27-solid-2-rc3-show-loading-withdrawals.md).

**Method.** It follows
[`2026-09-26-solid-2-rc9-signals-negative-rows.md`](2026-09-26-solid-2-rc9-signals-negative-rows.md)
and [`2026-09-25-solid-2-rc6-signals-negative-rows.md`](2026-09-25-solid-2-rc6-signals-negative-rows.md),
which follow the two rc.3 audits.

For `creates` it decides only what `semantic-model.md` § creates lets it
decide: whether one invocation **registers a version-1 resource into a runtime
outside this invocation** (a browser document or a server runtime). **[Decision
2026-09-04]** makes a hand-off to the per-request server render context that
writes into the response such a registration.

For `reads` it decides whether one invocation gives rise to an observation of a
reactive source's current value. That includes a scheduled observation. It
excludes a read a caller-supplied callable performs, and a property access
whose receiver the caller supplied (**[Decision 2026-09-10]**). It includes a
read of a source the export itself created. A later, separate invocation of a
value the export returned is the caller's act: that is the rc.6 line for
`createMemo` and `createOptimistic`.

A guarded reach still counts, and a flat row must then withhold.

**Performed** by four read-only readers in parallel (A: `For`, `Repeat`,
`Match`; B: `Show`, `Loading`; C: `createContext`, `useContext`,
`createSignal`, the four re-exported names and the `solid-js` host census; D:
the eight `@solidjs/web` rows and the web host census). The lead recomputed
every citation from the tarball bytes and re-read each load-bearing body
before writing this document. Claims are tagged the way the rc.9 vocabulary
review tags them, where the distinction matters: **[M]** measured on the bytes,
**[E]** estimated.

## 0. For the owner's sign-off

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| 1 | `solid-js` `For` `reads` | **GRANT** | At the call event `For` reads only parameter-rooted `props`. When hydrating it runs `mapArray`, whose eager compute reads only `For`'s own thunks over `props` and the caller's list value. Its internal memo is read only when the caller invokes the returned `list`. Server: `mapArray` → `createSyncMemo` over the same thunks. |
| 2 | `solid-js` `For` `creates` | **GRANT** | Client: `getOwner`, `runWithOwner` and `mapArray` stay inside `@solidjs/signals`, which has no handle on a document or a server runtime (rc.9 signals audit § 0.3). Server: `mapArray` → `createSyncMemo`, which never reaches `processResult` or `ctx.serialize`. |
| 3 | `solid-js` `Repeat` `reads` | **GRANT** | `repeat`'s eager `updateRepeat` reads only `Repeat`'s thunks over `props.count` and `props.from`, and it invokes the caller's `props.children`. |
| 4 | `solid-js` `Repeat` `creates` | **GRANT** | Same closure bound as row 2. The server `repeat` → `createSyncMemo` path does not serialize. |
| 5 | `solid-js` `Match` `reads` | **GRANT** | `function Match(props) { return props; }`, byte-identical in all six builds. |
| 6 | `solid-js` `Match` `creates` | **GRANT** | The same body. |
| 7 | `solid-js` `Show` `reads` | **WITHHOLD** | Its memos compute on the call's stack. `condition()` and `conditionValue()` read memos `Show` created. Byte-identical to rc.3, which withdrew the row too. |
| 8 | `solid-js` `Show` `creates` | **WITHHOLD** | Server: its own `createMemo(() => props.when)` reaches `ctx.serialize(id, deferred.promise, …)` for a thenable `when`. |
| 9 | `solid-js` `Loading` `creates` | **WITHHOLD** | Server: `ssrLoadingBoundary` calls `ctx.serialize(id, "$$f")` and `ctx.registerFragment(id, …)` when its children are pending. |
| 10 | `solid-js` `createContext` `creates` | **GRANT** | A fresh `Symbol` and a provider function it defines but does not call, in all six builds. |
| 11 | `solid-js` `useContext` `creates` | **GRANT** | A context-map read that may construct and throw an `Error`. Byte-identical to rc.3 in all six builds. |
| 12 | `solid-js` `createSignal` `creates`, flat | **WITHHOLD** | Server: the derived overload's `createMemo` → `processResult` → `ctx.serialize`, as on rc.3. |
| 13 | `solid-js` `createSignal` `creates`, `browser`-scoped | **GRANT (scoped)** | § 7.3's walk, redone on `dist/solid.js`, `dist/solid.dev.js` and `dist/solid.observe.js`, whose closures are byte-identical: no create. Delegates are `@solidjs/signals` `createSignal` and `getOwner` `creates`, both granted on `@solidjs/signals@2.0.0-rc.9` (the rc.9 signals audits), so the row binds beside rc.9. |
| 14 | `solid-js` `affects` `reads` and `creates`; `isPending` `creates`; `latest` `creates`; `refresh` `reads` and `creates` | **WITHHOLD** (6 rows) | `solid-js@2.0.0-rc.9` declares none of the four, and its three client builds define none of them. Both the declaration and the browser body are `@solidjs/signals@2.0.0-rc.9`'s, so a `solid-js` row could neither cite the body it denies nor bind. The server bodies (the pairing) are clean and are recorded for whoever reads the signals rows. |
| 15 | `@solidjs/web` `clientOnly` `reads` | **GRANT** | The call allocates a signal and starts the caller's `fn()`, whose continuation only writes; the server invokes nothing. The returned component's reads are a later invocation (F3). |
| 16 | `@solidjs/web` `clientOnly` `creates` | **GRANT** | Nothing registers at the call in any build; the server `registerAsset` and a possible `ctx.serialize` run only inside the returned component (F3). |
| 17 | `@solidjs/web` `httpHeader` `reads` | **GRANT** | Client `{}`. Server: the ambient request event, the response object, `Headers` builtins and module-private ledgers; no reactive source. Byte-identical to rc.3. |
| 18 | `@solidjs/web` `httpHeader` `creates` | **GRANT** (owner flag) | A write to an existing response and a retracting cleanup, as rc.3's node-server summary models it (`declare-response` is a `write`); no version-1 resource kind is registered. |
| 19 | `@solidjs/web` `httpStatus` `reads` | **GRANT** | As § 17. |
| 20 | `@solidjs/web` `httpStatus` `creates` | **GRANT** (owner flag) | As § 18. |
| 21 | `@solidjs/web` `hydrate` `reads` | **WITHHOLD** | It turns hydration on and hands the caller's `options` to `render`, reaching § 22's gate read. |
| 22 | `@solidjs/web` `render` `reads` | **WITHHOLD** | Under hydration an undeclared but type-reachable `options.insertOptions` (`{scope: true, ssrSource: "client"}`) makes `solid-js`' `hydratedEffect` create a gate signal that a compute in `render`'s closure reads on its own stack. |

### What the owner is asked to agree with beyond the table

1. **`For` `reads` rests on the returned-value line (flag F3).** rc.9 client
   `For` creates its `mapArray` lazily, inside the returned `list`. Only when
   `sharedConfig.hydrating` does it create it at the call event. `list`'s body
   is `For`'s code: `(mapped ?? (mapped = create()))()` reads the `mapArray`
   memo. It runs only when the caller, or the renderer on the caller's behalf,
   invokes the returned value. This audit applies the rc.6 line: a later,
   separate invocation of a returned value is the caller's act. If the owner
   instead counts `list`'s body as `For`'s own later execution, `For` `reads`
   falls on rc.9. rc.3's eager client `For` does not have the question.
2. **The external-source hook line is kept (flag F1).** When a third party has
   called `enableExternalSource`, `setupComputedNode` hands every computed to
   the installed `wireExternalSource` hook. The hook's wrapper reads a bridge
   signal the hook created (`@solidjs/signals` `dist/prod/core/external.js:44-57`,
   read at `:54`). This is dispositioned as installed-hook behaviour, exactly as
   the rc.6 audit did for `createMemo` (its § 1.3). If a hook-created signal is
   instead a source the export created, `For` and `Repeat` `reads` fall with
   `createMemo`'s.
3. **Row signals are the caller's to read (flag F2).** `mapArray` creates row
   and index signals and hands their accessors to the caller's
   `props.children`. It only ever writes them. So the reads of those sources
   are the caller's callable's, excluded on authorship.
4. **The six `solid-js` rows for re-exported names are withheld, not
   granted.** See § 14 for why a `solid-js` row for them is unbindable on rc.9.
   The rc.3 rows for the same names were kept, not dropped: they are
   **unreachable along the natural import path** (the declaration resolves
   into `@solidjs/signals`) but not provably dead. rc.3's `exports` carries
   `./types/*`, and `types/server/signals.d.ts` declares all four inside the
   `solid-js` archive, so an import through that subpath resolves a `solid-js`
   declaration. `docs/precision-backlog.md` already records the rows as dead
   along the re-export path.
5. **The `createSignal` browser row binds through the signals rc.9 rows.**
   `solid-js@2.0.0-rc.9` depends on `@solidjs/signals: ^2.0.0-rc.9`, so its
   delegate archive is rc.9 signals, whose `createSignal` `creates` (the
   2026-09-27 signals parity audit) and `getOwner` `creates` (the 2026-09-26
   audit) are both granted. An rc.9 `solid-js` beside another signals
   prerelease binds only where that archive's own rows answer both, and
   refuses by name elsewhere.

### What this audit does not do

- It does not read `@solidjs/signals@2.0.0-rc.9` exports beyond the closures
  these rows reach. In particular it grants no signals row.
- It does not decide whether the client `Loading` path is a `create`. That
  path does a dynamic `import()` into the browser module map and calls
  `$dfr` to activate a streamed fragment. The server builds already withhold
  the row.
- It does not change `releases.rs` `KNOWN_GAPS`, the rc.9 declaration defect
  (`createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`,
  `sharedConfig` and `$DEVCOMP` are re-exported but not declared), or any tier
  admission (§ 23).

---

## 0.1 Identity

Both tarballs were downloaded with `npm pack` into an isolated `$TMPDIR`
directory on 2026-09-27. For each, the checks were:

1. Its SHA-512 (`openssl dgst -sha512 -binary | base64`) equals the registry
   `dist.integrity` (`npm view … dist`).
2. It equals the solid-primitives (`next`) `pnpm-lock.yaml` entry the rc.9
   vocabulary review copied.
3. Its SHA-1 equals the registry shasum.
4. `diff -r` of the extracted `package/` against that lockfile install reports
   no difference.

`gitHead` is `9a29b1a07aa3e06ee32afd1fc4c18414b4a558bb` for both, the same as
`@solidjs/signals@2.0.0-rc.9`.

| archive | integrity | `package.json` sha256 | SHA-1 | tarball sha256 | bytes | files |
| --- | --- | --- | --- | --- | --- | --- |
| `solid-js@2.0.0-rc.9` | `sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==` (lock line 9038) | `c8d6224bd4bd63ed38f0cec77eb4d30d3f78debec091338dc4e466a2618e11bc` | `f07125ce…88b9` | `157da22e…6080` | 130,124 | 33 |
| `@solidjs/web@2.0.0-rc.9` | `sha512-pfiWoLDnLc+QYWc7UyLqO+5QrPEf3oTiNmmRC+C+uM6AZ5VH0bZMNPtLM5rJ29LKPiTwQitKV843IQDf/oeyhQ==` (lock line 4909) | `5de9244eb8121c5efe10f3976a06b09ca09cb99de4caf5fcf56dc040ce9773dc` | `a056d251…400f` | `21840a39…c5ad` | 524,585 | 67 |

The per-file digests are pinned under
`benchmarks/package-contract-v2/phase0/rc9/{solid-js,solidjs-web}/files.json`.
Each directory also holds the verbatim `package.json`, its `exports.json`, and
`tarball.json` (the record of the checks above). The callee closures were
followed into `@solidjs/signals@2.0.0-rc.9`, whose tarball was re-downloaded,
matches its `phase0/rc9/solidjs-signals` pin, and is `solid-js@2.0.0-rc.9`'s
`^2.0.0-rc.9` dependency.

## 0.2 Which build each condition selects

`solid-js@2.0.0-rc.9` `exports["."]`, in order: `worker`, `browser`, `deno`,
`node` (each with `development` and `observe` sub-arms), then `development`,
`observe`, `default`. So every consumer runs one of six files:

| selected by | runtime file |
| --- | --- |
| `browser`, or no host condition | `dist/solid.js` |
| … ∧ `development` | `dist/solid.dev.js` |
| … ∧ `observe` | `dist/solid.observe.js` |
| `worker`, `deno` or `node` | `dist/server.js` |
| … ∧ `development` | `dist/server.dev.js` |
| … ∧ `observe` | `dist/server.observe.js` |

There is no `require` arm and no CommonJS build. `main` and `module` are
`dist/server.js`. `@solidjs/web@2.0.0-rc.9`'s `.` has the same shape over
`dist/web{,.dev,.observe}.js` and `dist/server{,.dev,.observe}.js`. Every
condition's `types` is `./types/index.d.ts`, so the declaration a consumer
binds is the same under every host.

**Declarations.** In `solid-js`'s `types/index.d.ts`:

- `For`, `Repeat`, `Show`, `Match` and `Loading` come from
  `./client/flow.js` (`:5`), which is byte-identical to rc.3's.
- `createContext` and `useContext` come from `./client/core.js` (`:3`).
- `createSignal` comes from `./client/hydration.js` (`:8`).

All eight are `solid-js`'s own declarations, so rows about them are keyed on
`solid-js`. `affects`, `isPending`, `latest` and `refresh` are re-exported from
`@solidjs/signals` (`:1`); see § 14.

## 0.3 Archive-wide host-boundary census of `solid-js@2.0.0-rc.9`

Reader C used a lexer that strips comments while respecting strings,
templates and regex literals. It listed every identifier-boundary occurrence in
all six `.` builds and mapped each to its enclosing top-level definition **[M]**.

- **Imports.** Every build imports only `'@solidjs/signals'` (lines 1 and 2).
  There is no dynamic `import()` and no `import.meta`.
- **Absent from all six as code.** `navigator`, `removeEventListener`,
  `setInterval`, the `clear*` timers, `requestAnimationFrame`,
  `requestIdleCallback`, `MessageChannel`, `localStorage`, `sessionStorage`,
  `Date`, `WeakRef`, `FinalizationRegistry`, `XMLHttpRequest`, `WebSocket`,
  `Worker`, `BroadcastChannel`, `structuredClone`, `postMessage`,
  `setImmediate`, `eval`, `Function`, `reportError`, `dispatchEvent`,
  `CustomEvent` and `EventTarget`.
- **Client builds** reach the host only in these places:
  - `subFetch`: a `window.fetch`/`Promise` swap, restored in `finally`.
  - `normalizeIterator` and hydration helpers: `Promise.resolve`.
  - `onHydrationEnd` and the hydrated loading boundary: `queueMicrotask`.
  - Hydration helpers: `globalThis._$HY`, which `enableHydration` writes and
    `replayHeldFragment` uses to call `$dfr`.
  - `watchTruncation`: `document` (`addEventListener("DOMContentLoaded")`).
  - `drainHydrationCallbacks` and `watchTruncation`: `setTimeout`.
  - `reportAssetFailure` and dev `Errored`: `console`.

  **None of these is in the closure of `For`, `Repeat`, `Match`,
  `createContext` or `useContext`.** `createSignal`'s browser closure reaches
  only `subFetch`, `normalizeIterator`, `onHydrationEnd`'s `queueMicrotask`,
  and `_$HY.r` reads through the `sharedConfig.has`/`load` hooks that
  `@solidjs/web`'s `hydrate` installs (§ 13).
- **Server builds** hand values to `sharedConfig.context`:
  - `processResult` calls `serialize`, `hold` and `commit`.
  - `serverEffect` and `lazy` call `block`.
  - `createProjection`, `createErrorBoundary` and `ssrLoadingBoundary` call
    `serialize`, and `ssrLoadingBoundary` also calls `registerFragment`.
  - Other host reaches are `globalThis` symbol slots (the server error hook,
    the request context, and in dev/observe the observe slot), `console`, and
    in dev/observe `performance`.

  Of the rows here, **only `Show`, `Loading` and `createSignal` reach a
  `ctx.serialize` or `ctx.registerFragment`**.

## 0.4 How the citations were computed

Byte offsets are of the whole file, read in binary from the extracted tarball.
Each slice runs from the `function <export>(` token (for `createSignal`, the
`const createSignal = ` token) through the closing `}` inclusive (for
`createSignal`, the terminating `;`). This is the 2026-09-23 convention. Each
slice was checked to begin with that token, and to be the only definition of
the name in its file.

Before computing anything for rc.9, Reader C's slicer was run on rc.3. It
reproduced every rc.3 `createContext` and `useContext` citation `solid_2.rs`
carries, in all six rc.3 bundles. The slices are checked in under
`rust/crates/solid-dialect/audited-slices/solid-v2/rc9/{solid-js,solidjs-web}/`.
Whole-file sha256 of the `solid-js` builds:

| file | sha256 | bytes |
| --- | --- | --- |
| `dist/solid.js` | `0238f90858359bc71da523cae1c38dc769f6d86502ae37bcf1c7d72cc977794d` | 41,418 |
| `dist/solid.dev.js` | `7baf8808f6bd87011707621ab9137a41416a585db3d12c89f7a7efcdec2411de` | 45,011 |
| `dist/solid.observe.js` | `c337cd4b1b90e5dda31dccd4373d7d72e3aec07a7d40e1a6fefd903b63b16ffb` | 41,964 |
| `dist/server.js` | `9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9` | 74,707 |
| `dist/server.dev.js` | `129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7` | 84,772 |
| `dist/server.observe.js` | `0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede` | 79,326 |
| `types/index.d.ts` | `3baf50a7fe13e5b23acfdb99faf58da630f3e5f5f14b2b42f53276839d29b6fe` | 3,363 |

---

## Part A — `solid-js@2.0.0-rc.9`

### 1. `For` — `reads` — archive `solid-js@2.0.0-rc.9`

#### 1.1 Implementation

Client, `dist/solid.js:1164-1177` (`solid.dev.js:1194-1208` and
`solid.observe.js:1176-1190` add only `options.name = "<For>"`):

```js
function For(props) {
  const options = "fallback" in props ? { keyed: props.keyed, fallback: () => props.fallback } : { keyed: props.keyed };
  const owner = getOwner();
  let mapped;
  const create = () => runWithOwner(owner, () => mapArray(() => props.each, props.children, options));
  if (sharedConfig.hydrating) mapped = create();
  const list = () => (mapped ?? (mapped = create()))();
  return list;
}
```

Server, `dist/server.js:2093-2101` (byte-identical in `server.dev.js` and
`server.observe.js`, and to rc.3's client and server `For`, `36dc66a0…`):
`return mapArray(() => props.each, props.children, options)` after the same
`options`.

#### 1.2 What runs, and when [M]

1. **At the call event, not hydrating.** It evaluates `"fallback" in props`
   and `props.keyed`, parameter-rooted receivers (the caller's). It calls
   `getOwner()`. It reads `sharedConfig.hydrating`: a field of the module's own
   `sharedConfig` object, or after `enableHydration` a getter over a module
   `let`. Neither is a reactive source. It allocates two closures and returns
   `list`. There is no other read.
2. **At the call event, hydrating.** `mapped = create()`, which is
   `runWithOwner(owner, () => mapArray(...))`. `@solidjs/signals`' `mapArray`
   (`dist/prod/map.js:23-47`; `dist/dev.js:8300-8336`;
   `dist/observe/map.js:25-51`) builds an owner and
   `computed(updateKeyedMap.bind(o))`. With no `lazy`, `setupComputedNode` runs
   `recompute(e, true)` on this stack (`prod/core/core.js:817`,
   `dev-shared.js:5005`, `observe/core/core.js:851`).
3. **`updateKeyedMap`** (`prod/map.js:193-379`) reads:
   - `this.ss()`: `For`'s thunk `() => props.each`, a parameter-rooted access;
   - `t[$TRACK]`, `t.length`, `t[e]` and `t.slice(0)` on the caller's list
     value, a caller-supplied receiver;
   - `this.qt(a)`, the caller's `keyed` function;
   - `this.us`, `For`'s thunk over `props.fallback`.

   It creates row and index signals with `signal(…)` and hands `accessor(…)`
   to the caller's `props.children` (flag F2). Its own code only writes them
   (`setSignal`). A reachability graph over each signals build (133, 149 and
   140 functions) finds **no read site** besides the installed
   external-source hook (flag F1).
4. **Later, on the caller's invocation of `list`.** It creates the `mapArray`
   if needed and calls `accessor(h)()`, which is `read(h)`. That is the
   returned-value line (flag F3).

**Server.** `mapArray` (`server.js:1230-1262`, byte-identical to rc.3) calls
`createMemo(…, { sync: true })`, which is `createSyncMemo` (`:411-455`). Its
eager `pull()` runs a compute that reads `list()` (`For`'s thunk over
`props.each`) and `items[i]` (the caller's value), and invokes `mapFn` (the
caller's `props.children`) and `options.fallback`. The memo's value is read
only through the returned `read`.

#### 1.3 What changed from rc.3

- rc.3's client `For` was the eager `return mapArray(…)`, byte-identical to
  the server body.
- rc.9 adds the lazy `list` wrapper, `getOwner`/`runWithOwner` and the
  hydrating eager path.
- The server `For`, `mapArray` and `repeat` are unchanged.

#### 1.4 Verdict

**GRANT**, under flags F1 to F3.

Sign-off: `For` observes no reactive source it created. Every value it reads at
its call event, and in the one computation it can create there, is a caller's
`props` property, the caller's list, or the caller's callables. Its memo is
read only when the caller invokes the returned value.

---

### 2. `For` — `creates` — archive `solid-js@2.0.0-rc.9`

| Callee | Reach | Disposition | Host reach |
| --- | --- | --- | --- |
| `getOwner`, `runWithOwner`, `mapArray`, and the `computed`/`signal`/`setSignal`/owner machinery | client, always or hydrating | archive `@solidjs/signals@2.0.0-rc.9` | `schedule()` → `queueMicrotask(flush)`; `console` in dev diagnostics |
| `sharedConfig.hydrating` | client | local | none |
| `mapArray` → `createOwner`, `createSyncMemo`, `runWithOwner`, `formatChildId`, `stampThrower` | server | local | `sharedConfig.context?.commitEpoch?.()`, a counter getter that `@solidjs/web/frames` installs |

- The rc.9 signals archive has no handle on a document or a server runtime
  (rc.9 signals audit § 0.3). On the client, nothing in `For`'s own code
  touches a host beyond `sharedConfig`.
- On the server, `createSyncMemo` never reaches `processResult`, so there is no
  `ctx.serialize` on this path **[M]**.

**GRANT.** Sign-off: `For` builds owners, memos and row signals, which are
reactive-graph facts, and registers no version-1 resource into any runtime.

---

### 3. `Repeat` — `reads` — archive `solid-js@2.0.0-rc.9`

`dist/solid.js:1178-1184` is byte-identical to `server.js:2102-2108` and to
rc.3's client and server `Repeat` (`540fa67e…`). `solid.dev.js:1209-1216` and
`solid.observe.js:1191-1198` add `options.name = "<Repeat>"` (`c9b26a45…`,
the rc.3 dev slice).

```js
function Repeat(props) {
  const options = "fallback" in props ? { fallback: () => props.fallback } : {};
  options.from = () => props.from;
  return repeat(() => props.count, index => typeof props.children === "function" ? props.children(index) : props.children, options);
}
```

- **Client.** `repeat` (`prod/map.js:395-415`) runs `computed(updateRepeat)`
  eagerly. `updateRepeat` (`:424-474`) reads `this.ds()` and `this.ws?.()`,
  which are `Repeat`'s thunks over `props.count` and `props.from`
  (parameter-rooted). It invokes `this.rs(t)`, which is `Repeat`'s arrow over
  the caller's `props.children`, under fresh owners, and `this.us` for the
  fallback. It creates no signal of its own to read.
- **Server.** `repeat` (`server.js:1263-1295`, byte-identical to rc.3) runs the
  same shape through `createSyncMemo`.
- Flag F1 applies as in § 1.

**GRANT.**

### 4. `Repeat` — `creates` — archive `solid-js@2.0.0-rc.9`

`For`'s closure bound applies unchanged (§ 2): `@solidjs/signals` on the
client, and `createSyncMemo` with no serialize on the server. **GRANT.**

---

### 5. `Match` — `reads` — archive `solid-js@2.0.0-rc.9`

`function Match(props) { return props; }` is byte-identical in all six builds
and to rc.3's (`54f4236b…`). It contains no call and no property access.
Returning the caller's props is not an observation. **GRANT.**

### 6. `Match` — `creates` — archive `solid-js@2.0.0-rc.9`

The same body invokes nothing. **GRANT.**

---

### 7. `Show` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

The client bodies are:

- `dist/solid.js:1185-1206`, byte-identical to rc.3 `solid.js` (`5e3245ae…`);
- `solid.dev.js:1217-1242`, byte-identical to rc.3 `dev.js` (`faf07392…`);
- `solid.observe.js:1199-1224`, which differs from dev only in passing
  `IS_DEV` instead of the `"<Show>"` label.

Each creates `conditionValue = createMemo$1(() => props.when)`, then (when
unkeyed) `condition = createMemo$1(conditionValue, {equals, sync: true})`, and
returns `createMemo$1(() => { const c = condition(); … })`. No memo is `lazy`,
and `setupComputedNode` computes each on the call's stack. So during the
invocation:

- the second memo reads `conditionValue()`;
- `Show`'s own returned-memo compute reads `condition()`.

Both are memos `Show` created, and a read of a source the export created still
counts. The server body (`server.js:2109-2128`, byte-identical in all three
server builds and to rc.3) reads `conditionValue()` inside its eager
`createSyncMemo`. The rc.3 withdrawal
(`2026-09-27-solid-2-rc3-show-loading-withdrawals.md` § 1) walks the same
bytes.

**WITHHOLD.**

### 8. `Show` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

The server `Show` calls the server `createMemo(() => props.when)`
(`server.js:321-410`, byte-identical to rc.3). It has no options, so
`update()` runs at once, and `processResult` (`:516-819`, byte-identical to
rc.3) sets `serializes = !!(ctx?.async && ctx.serialize && id && !noHydrate)`
(`:571`). For a thenable result it then calls
**`ctx.serialize(id, deferred.promise, deferStream)`** (`:574`); dev `:645` and
observe `:618` are the same. The async-iterable arm reaches it at `:715` and
`:776`.

- `@solidjs/web@2.0.0-rc.9`'s `renderToStream` context (`async: true`, web
  `server.js:1697-1698`) writes that promise into the response (`serialize`,
  `:1760-1776`). That is **[Decision 2026-09-04]**.
- `Show<T>`'s `when: T` is unconstrained. Reader B ran `tsc` 5.9.3 against the
  published typings, and `<Show when={promise}>` type-checks **[M]**.
- The client builds perform no create (§ 2's bound).

**WITHHOLD**, as on rc.3 (the rc.3 withdrawals audit § 2).

### 9. `Loading` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

The server `Loading` is `server.js:2173-2175`, byte-identical in all three
server builds and to rc.3 (`f92a3e9c…`). It calls `createLoadingBoundary`
(`:1858-1864`), which runs `ssrLoadingBoundary` (`:1865-2074`) whenever there
is an SSR context. When the children are pending, that calls:

- **`ctx.serialize(id, "$$f")`** at `:2008`, `:2023` and `:2071` (dev
  `:2224`, `:2239`, `:2287`; observe `:2142`, `:2157`, `:2205`);
- **`ctx.registerFragment(id, regOpts)`** under `renderToStream` at `:2032`
  (dev `:2248`, observe `:2166`);
- on commit, `ctx.serialize(id + "_assets", …)`.

Web's `registerFragment` (`server.js:1780-1854`) registers the key in the
per-request fragment registry and writes its `_fr` promise into the stream.
Its `done` callback later streams the `<template>` and a `$df` activation.
rc.3's server bytes reach the same sites (the rc.3 withdrawals audit § 3).

**WITHHOLD.**

---

### 10. `createContext` — `creates` — archive `solid-js@2.0.0-rc.9`

`dist/solid.js:6-17` (`solid.dev.js:5-16`, `solid.observe.js:6-17`, the same
slice `c3be410a…`):

```js
function createContext(defaultValue, options) {
  const id = Symbol(options && options.name || "");
  function provider(props) {
    return createRoot$1(() => {
      setContext(provider, props.value);
      return children(() => props.children);
    });
  }
  provider.id = id;
  provider.defaultValue = defaultValue;
  return provider;
}
```

The server builds, `server.js:1605-1616`, `server.dev.js:1706-1717` and
`server.observe.js:1679-1690`, are byte-identical to rc.3's six bundles
(`87cfe46a…`).

The transitive call table has **one row: `Symbol(...)`, a builtin**. `provider`
is defined, and two properties are written on it; it is not invoked. Its body
runs only when a consumer renders the returned provider, which is a later,
separate invocation of the returned value (the rc.3 audit's § 2.2 reading).
The client slice differs from rc.3's only in `createRoot` → `createRoot$1`.
rc.9 `solid-js` has its own `createRoot`, so the signals import is aliased.

**GRANT.**

### 11. `useContext` — `creates` — archive `solid-js@2.0.0-rc.9`

- **Client.** `dist/solid.js:18-20` (and the dev and observe twins,
  `03f0fc70…`, byte-identical to rc.3) is `return getContext(context);`. That
  is `@solidjs/signals`' `getContext`: `prod/core/context.js:28-39`,
  `observe/core/context.js:28-39`, `dev.js:272-283`. It calls `getOwner()`,
  reads the owner's context map, falls back to `context.defaultValue`, and may
  construct and throw `NoOwnerError` or `ContextNotFoundError`, whose
  constructors only call `super`.
- **Server.** `server.js:1617-1627` (and the dev and observe twins,
  `3cbc46d0…`, byte-identical to rc.3) wraps the server's own `getContext`
  (`:119-126`, byte-identical to rc.3). On `ContextNotFoundError` it calls
  `serverComponentContextError` (`:1550-1555`), which reads a context entry and
  may construct an `Error`.
- No path touches a host. No `Symbol.hasInstance` exists in any of the three
  archives, so `instanceof` invokes nothing.

**GRANT.**

---

### 12. `createSignal` — `creates` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

This is the flat row. The server `createSignal` is `server.js:302-320` and
`server.observe.js:346-364` (`d23963d5…`: rc.3's body minus two
`warnServerWrite` calls, which are empty in those builds) and
`server.dev.js:371-391` (`fc835fed…`, byte-identical to rc.3's
`server.js:285-305`).

Its derived overload calls `createMemo(prev => first(prev), opts)`. `opts`
carries only `deferStream`, `ssrSource` and `loadingValue`. That memo's
`processResult` reaches `ctx.serialize` on the same sites as § 8. The guard is:

- `node`/`worker`/`deno`;
- `renderToStream`;
- the derived overload;
- `ssrSource !== "client"`;
- a thenable or async-iterable result, or a served `loadingValue`;
- an owner with an id;
- no `NoHydrate`.

**WITHHOLD**, as on rc.3 (2026-09-04 audit § 7.4).

### 13. `createSignal` — `creates`, `browser` host target — archive `solid-js@2.0.0-rc.9`

This redoes the 2026-09-04 audit's § 7.3 walk on rc.9, in all three builds the
`browser` condition can select. The wrapper is:

```js
const createSignal = (...args) => {
  return (_createSignal || createSignal$1)(...args);
};
```

It is at `dist/solid.js:773-775` (`23906..23997`), `solid.dev.js:799-801` and
`solid.observe.js:785-787`, with the same slice (`96473897…`).

**The closure is byte-identical across the three builds [M].** A
breadth-first walk over top-level definitions, from `createSignal` and
`hydratedCreateSignal`, reached the same 27 definitions in each file, all
byte-identical. `enableHydration`, the only writer of `_createSignal`, is
byte-identical too. Line numbers below are `solid.js`:

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createSignal$1(...args)` | `:774`, imported at `:1` | cond: `enableHydration()` not called | **archive (delegate)** | `@solidjs/signals`' `createSignal`, both overloads |
| `hydratedCreateSignal` | `:602-605` | cond: `enableHydration()` was called (by `@solidjs/web`'s `hydrate`) | local | For a non-function argument, or when not hydrating: `createSignal$1(fn, second)`. Otherwise: `hydrateSignalLike(createSignal$1, fn, second)`. |
| `hydrateSignalLike` | `:571-595` | cond: hydrating, derived overload | local | Every branch ends in `coreFn(wrapper, options)`: the delegate, with an export-authored compute. |
| `markTopLevelSnapshotScope` | `:65-72` | cond | local | `getOwner()` (**delegate**), walks `_parent`, then signals `markSnapshotScope` (a field write). |
| `withHydrationGate` | `:563-570` | cond: `ssrSource` is `"client"`, or `"hybrid"` ∧ `has` | local | `createSignal$1(false, {ownedWrite: true})` (**delegate**), then `setHydrated(true)`: signals' bound `setSignal`, a reactive write. |
| `sharedConfig.has` / `.load` | `:173`, `:177`, `:332-333`, `:580` | cond | hook | Installed by `@solidjs/web` `hydrate` (`web.js:1341-1342`) as reads of `globalThis._$HY.r`. **Host reads only.** |
| signals `peekNextChildId` → `childId` → `formatId` | `prod/core/owner.js:166-168` | cond | archive | Id arithmetic. |
| `hydrateSignalFromAsyncIterable` | `:329-367` | cond | local | Wraps the payload's async iterator with `normalizeIterator` (`:209-244`, `Promise.resolve`) and ends in `coreFn(…)`. |
| `readSerializedOrCompute` / `readHydratedValue` | `:171-182`, `:156-169` | cond | local | Reads the serialized value. New in rc.9: the `latchedOnce` `WeakSet`. Runs `payload.then(…)` on the payload thenable. |
| `subFetch` | `:132-148` | cond | local → **host** | Temporarily sets `window.fetch = () => new MockPromise()` and `Promise = MockPromise`, runs the caller's compute, and restores both in `finally`. |
| `armLiveTakeover` | `:188-198` | cond: second compute on a latched owner, or a live source | local | `createSignal$1(false)` (**delegate**) into the module `liveGate`, and `onHydrationEnd(cb)` |
| `onHydrationEnd` | `:76-83` | cond | local → **host** | `queueMicrotask(callback)` or a module-array push |
| `fn(prev)` / `compute(prev)` | many sites | cond | **caller** | the caller's compute |

**No create.** Every reach outside the invocation is one of: a read of
`_$HY.r`, a method call on a payload value, the `fetch`/`Promise` swap that is
undone before return, or a microtask. `enableHydration`'s `_$HY` writes,
`watchTruncation`'s `document.addEventListener` and
`drainHydrationCallbacks`' `setTimeout` are **not** reachable from
`createSignal`. **[E]** Subscribing to the payload's async iterator (a seroval
stream deserialized on the page) is read as a host read or subscription, not a
create, consistent with the 2026-09-04 audit's § 7.3.

**The scope.**

- **Condition:** `browser`.
- **Runtime:** `dist/solid.js`, `dist/solid.dev.js` and `dist/solid.observe.js`.
  `browser` selects the first, `browser` ∧ `development` (with or without
  `observe`) the second, and `browser` ∧ `observe` the third. Adding `node` or
  `deno` still selects the `browser` arm, which comes first. `worker` comes
  before `browser` and selects `server.js`, which the row does not list, so it
  refuses.
- **Delegates:** `@solidjs/signals` `createSignal` and `getOwner` `creates`,
  the only canonical primitives the walk reaches in signals.
- **Non-primitive signals reaches** rest on the rc.9 signals archive-wide
  census (§ 0.3 there), which no row can name: `markSnapshotScope`,
  `peekNextChildId`, the bound accessor `read` (`hydrated()`, `liveGate()`), and
  the bound setter `setSignal` (`setHydrated(true)`, and `write(true)` later).
  The rc.3 row's comment names only the first two. The last two are its gap
  too.

**What changed from rc.3.** The wrapper slice is identical to rc.3's.
`hydrateSignalLike`'s last branch now goes through `readSerializedOrCompute`,
which gains the `latchedOnce` latch and a second-compute `armLiveTakeover`.
Every other helper is byte-identical to rc.3's `solid.js`.

**GRANT, scoped.** Beside `@solidjs/signals@2.0.0-rc.9`, both delegates are
granted, so the census terminates the row under `browser`
(`census_host_target_row_binds_solid_js_rc9_beside_signals_rc9` in
`contract_certification/type_facts.rs`).

---

### 14. `affects`, `isPending`, `latest`, `refresh` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

**Where each name comes from [M].**

- `types/index.d.ts:1` re-exports all four from `@solidjs/signals`.
- `dist/solid.js:2`, `solid.dev.js:2` and `solid.observe.js:2` re-export them
  from `'@solidjs/signals'`. None of the three defines a body.
- The three server builds define their own bodies and list them in their final
  `export { … }`.

A consumer's declaration therefore binds `@solidjs/signals@2.0.0-rc.9`, not
`solid-js`. The census binds the archive the declaration lives in
(`census_dialect_axiom`), and so does the proposal side
(`some_audit_denies_primitive`, keyed on the declaring package). A
`solid-js@2.0.0-rc.9` row for these names is not reached along the natural
path.

A `solid-js` row could also cite only half of what it denies. Its archive holds
no browser body for these names, and an `Implementation` citation may cite only
the row's own archive's files. So the six rows are **withheld**:

- `affects` `reads`;
- `affects` `creates`;
- `isPending` `creates`;
- `latest` `creates`;
- `refresh` `reads`;
- `refresh` `creates`.

A row for these names belongs to the signals archive. There it must cite
signals' three builds, and be paired with the `solid-js` server bodies below.

**The pairing: what a `solid-js` import runs under `node`/`worker`/`deno`
[M].** These bodies are byte-identical in all three server builds:

| export | `server.js` | body | vs rc.3 | `creates` | `reads` |
| --- | --- | --- | --- | --- | --- |
| `isPending` | `:1573-1581` | `try { fn(); return false } catch (err) { if (err instanceof NotReadyError) throw err; return false }` | identical (`cf2f2b80…`) | none | none (`fn` is the caller's) |
| `latest` | `:1582-1584` | `return fn();` | identical (`78986572…`) | none | none |
| `refresh` | `:1585-1592` | for a non-function, `Promise.resolve(target)`; otherwise `Promise.resolve(target())`, or `Promise.resolve(undefined)` on a throw | **changed**: rc.3 was `return undefined` | none (the builtin `Promise`) | none (`target()` is the caller's) |
| `affects` | `:1593-1595` | `return undefined;` | identical (`1dc4d6d0…`) | none | none |

For the audited documents, `latest` publishes a `read` and `isPending` leaves
`reads` open, so on the client side no `reads` row exists for either.

**The rc.3 rows for these names were kept, not dropped.** They are unreachable
along the natural import path, because the declaration resolves into
`@solidjs/signals`. But they are not provably dead: rc.3's `exports` has
`./types/*`, and `types/server/signals.d.ts` declares all four inside
`solid-js`. `docs/precision-backlog.md` already records them as dead along the
re-export path.

---

## Part B — `@solidjs/web@2.0.0-rc.9`

### B.0 Builds, pairing and the archive-wide bound

Whole-file sha256 of the six `.` builds (all equal to `files.json`):

| file | sha256 | bytes |
| --- | --- | --- |
| `dist/web.js` | `32083d72a93231d8919bc78c57dea48d2c13bac8b571f07900c1b15fe95edfbf` | 78,071 |
| `dist/web.dev.js` | `bcbe02189e31a7ce29852ef74b9a12e7ad135d92a269cb89057d112e8075129d` | 86,356 |
| `dist/web.observe.js` | `b5b2950b906244f10ed54fc11a45076bcabd341f77e86249cb2ce462e72d8d74` | 79,035 |
| `dist/server.js` | `b96fc8399d4c9909fb8a275e76abde586c843d006dcb427fe4a56620bfee2c40` | 132,975 |
| `dist/server.dev.js` | `4636ed7864cad6f1b541d70872f51d1d98c35bb21bb30549a2b0c091570465c6` | 143,973 |
| `dist/server.observe.js` | `9919deaaa0c6b73d1f760247581e351c2d0778fc18784653b11579c4e11fe3fa` | 136,002 |

**Pairing.** `@solidjs/web`'s and `solid-js`'s `exports["."]` have the same
shape, so one condition set selects matching builds of both: `web.js` with
`solid.js`, `server.dev.js` with `server.dev.js`, and so on. `@solidjs/signals`
is paired underneath by its own map. Every signals check below ran on all
three signals builds, so any pairing is covered **[M]**.

**Where the five exports are defined.**

- The client builds define each of the five once.
- The server builds define `clientOnly`, `httpHeader` and `httpStatus`, and
  bind `notSup as hydrate` and `notSup as render` in their final `export`
  (`server.js:3892`). `notSup` (`:3721-3723`, byte-identical in all three
  server builds and to rc.3) is `throw new Error("Client-only API called on the
  server side. …")`. So the server `render` and `hydrate` read nothing and
  create nothing.
- (`const hydrate = sharedConfig.context` at `server.js:3148` is a local of
  `getHydrationKey`, not the export.)

**Host census [M].** Reader D ran acorn's tokenizer over the six builds. The
watch list was the rc.9 signals audit's, plus `Headers`, `Response`,
`Request`, `URL`, `ReadableStream`, `TextEncoder`, `crypto`, `location`,
`history`, `MutationObserver`, `AbortController`, `Buffer`, `require`,
`AsyncLocalStorage` and dynamic `import()`. Every hit was mapped to its
top-level definition. On the calls of the five exports:

- The only reach that registers a version-1 resource into an outside runtime is
  `render`'s `registerDelegatedRoot(element)` → `registerDelegatedContainer` →
  `attachDelegatedEvent` → `container.addEventListener` (`web.js:370`, `:420-454`).
  `hydrate` inherits it.
- Other client reaches, which register no resource kind:
  - `document` (`render`'s `element === document` branch; `insert`'s
    `createTextNode`);
  - `hydrate`'s `globalThis._$HY` reads and writes, its
    `document.getElementById`, the `loadModuleAssets` `import(new URL(…,
    document.baseURI))`, and the new rejected-preload
    `(globalThis.reportError || console.error)(err)`;
  - `solid-js`' `enableHydration` → `watchTruncation`
    (`document.addEventListener("DOMContentLoaded", …, {once: true})`) and
    `drainHydrationCallbacks` (`setTimeout`);
  - `queueMicrotask(flush)`.
- Server reaches: `getRequestEvent` (`globalThis[Symbol.for("solid.RequestContext")].getStore()`
  or `sharedConfig.context.event`), and `notSup`'s throw. `clientOnly`'s returned
  component reaches `registerClientOnlyPreload` → `ctx.registerAsset`, but that
  is not the call (§ 15).
- No `window`, `fetch`, `navigator`, storage, `XMLHttpRequest`, `WebSocket`,
  `Worker`, `eval` or `Function` code reference exists in any build.

Citations below use § 0.4's convention. `render` and `hydrate` get no rc.9
row (§§ 21-22), so their server `notSup` bodies are described but not cited.

---

### 15. `clientOnly` — `reads` — archive `@solidjs/web@2.0.0-rc.9`

- **Client.** `web.js:2113-2134` (and the dev and observe twins, `f6412c9a…`,
  byte-identical to rc.3's `web.js:1877-1898`) opens with:

  ```js
  const [comp, setComp] = createSignal();
  let started = !options.lazy;
  started && loadClientOnly(fn, setComp, options.export);
  return props => { … };
  ```

  `createSignal` is `solid-js`' wrapper → `hydratedCreateSignal` or signals'
  `createSignal`. With no argument it takes the plain overload, which invokes
  nothing. `loadClientOnly` (`:2110-2112`) is `fn().then(m => setComp(() =>
  exportName ? m[exportName] : m.default))`:
  - `fn()` is the caller's;
  - `.then` is on the caller's returned value;
  - the continuation is a **write**. The updater form passes the current value
    as an argument and is not the read path (rc.6 audit § 0.6).
- **Server.** `server.js:3808-3813` (all three builds, `31fd1a14…`,
  byte-identical to rc.3) is `return props => {…}`. It has no row.
- **The returned component.** It calls `untrack(comp)`, which reads the
  signal this call created. It creates a gate memo reading `comp()` and
  `mounted()`. On the server it calls `registerClientOnlyPreload` →
  `ctx.registerAsset` and a server `createMemo(() => props.fallback)`. All of
  this runs **only when the caller invokes the returned component**: the
  returned-value line (flag F3).

**GRANT**, under F3. If the line is redrawn, this row falls unconditionally
(`untrack(comp)`). The rc.3 summaries publish the returned component's
render-time operations inside `call` while closing `reads` and `creates`, so
their evidence is consistent only under the same line (Reader D § 1.6).

### 16. `clientOnly` — `creates` — archive `@solidjs/web@2.0.0-rc.9`

- At the call: the client allocates a signal and starts the caller's
  `fn()`, and its continuation writes. Nothing in signals can register a
  resource. The server invokes nothing.
- The returned component's server `registerAsset("preload" | "module" |
  "style", …)` hands asset URLs to the render context. The `preload` kind is
  new in rc.9. Its `createMemo(() => props.fallback)` can reach
  `ctx.serialize` for a thenable fallback. Both are later invocations of the
  returned value (F3).

**GRANT**, under F3.

---

### 17. `httpHeader` — `reads` — archive `@solidjs/web@2.0.0-rc.9`

**Client.** `function httpHeader(_name, _value, _options) {}` (`web.js:2136`,
and the dev and observe twins, `cd98272f…`, byte-identical to rc.3) has no row.

**Server.** `server.js:3850-3890` (all three builds, `3d0445e2…`,
byte-identical to rc.3's `:3289-3329`):

| Callee / site | Reach | Disposition | Reads? |
| --- | --- | --- | --- |
| `getRequestEvent()` (`:3465-3467`) | always | local → **host** | `globalThis[Symbol.for("solid.RequestContext")].getStore()` (an `AsyncLocalStorage` a framework installed), or `sharedConfig.context.event`, or `console.warn`. Not a reactive source. |
| `event.response`, `.committed`, `.headers` | always / cond | ambient request object (a `createResponseStub` literal when `@solidjs/web` built it) | not a reactive source, and not a proxy the export owns |
| `headerLedgers` `WeakMap`, `Map`, `ledger.live.push`; `name.toLowerCase()`; `headers.getSetCookie/get/set/append` | cond | builtin, module-private | no |
| `onCleanup(fn)` → `solid-js` server `onCleanup` (`js server.js:113-118`) | cond | archive | a `cleanups` item |
| the retraction closure, at cleanup | later | local | `indexOf`/`splice` and header restore; no read |

**GRANT.**

### 18. `httpHeader` — `creates` — archive `@solidjs/web@2.0.0-rc.9`

The body mutates the headers of a response that already exists, and registers
a cleanup that retracts the mutation. The rc.3 node-server summary
`summary-0ffbf4d4…` models exactly this:

- `declare-response`, `kind: write` over `request`/`response`;
- `retract-declaration`, `kind: cleanup`;
- `creates: []` closed.

rc.9's bytes are unchanged. The values handed over are strings. No version-1
resource kind is registered: the `response` already exists and is written.

**GRANT.**

**Owner flag.** Read literally, [Decision 2026-09-04]'s headline sentence
("handing a value to a per-request render context that writes it into the
response is a `create`") could cover this mutation. The decision's own four
terms require a version-1 resource kind to be registered, and this audit
follows the terms, as the rc.3 document did. If the headline is ruled
decisive, this row and § 20 fall on the server. The guard would be an active
request event with an uncommitted response.

---

### 19. `httpStatus` — `reads` — archive `@solidjs/web@2.0.0-rc.9`

`function httpStatus(_code, _text) {}` on the client (`web.js:2135`,
`9de7ebfe…`). On the server, `server.js:3816-3849` (all three builds,
`56233798…`, byte-identical to rc.3's `:3255-3288`) has the same shape as
`httpHeader`:

- `getRequestEvent()`;
- the `statusLedgers` `WeakMap`;
- `response.status = code` and `response.statusText = text`;
- an `onCleanup` retraction.

No reactive source is observed. **GRANT.**

### 20. `httpStatus` — `creates` — archive `@solidjs/web@2.0.0-rc.9`

§ 18's reading applies: a write to an existing response and a cleanup.
**GRANT**, with § 18's owner flag.

---

### 21. `hydrate` — `reads` — archive `@solidjs/web@2.0.0-rc.9` — **WITHHOLD**

`web.js:1323-1407` (observe identical, `e9768b7a…`; dev `web.dev.js:1422-1517`)
does the following:

1. It calls `enableHydration()` and `installHydrationRuntime()`.
2. On the `_$HY.done` fast path it returns `render(code, element,
   [...childNodes], options)`.
3. It hoists `data-dh` head nodes (DOM only) and writes `options.renderId ||=
   ""` (the caller's object).
4. It installs `sharedConfig` closures (`load`, `has`, `gather`,
   `loadModuleAssets`, `cleanupFragment`, `captureBoundaryScope`) and sets
   `sharedConfig.hydrating = true`.
5. With a root `_assets` mapping, it preloads modules. It renders on
   fulfilment. On rejection, it renders unless the root is the document, in
   which case it reports: that arm is **new in rc.9**.
6. Otherwise it gathers and returns `render(…, options)`, then clears
   `hydrating` in `finally`.

`hydrate` itself creates no signal or memo. `sharedConfig.hydrating` and
`done` are accessors over module `let`s, and `has`/`load` read the `_$HY.r`
host object. None of these is a reactive source.

It hands the caller's `options` to `render`, and that reaches § 22's path with
the hydrating precondition established by `hydrate` itself. The
rc.3 withdrawal (§ 5 there) reaches the same verdict on rc.3's bytes.

**WITHHOLD.** Its `creates` stays withheld too:

- `render` still calls `registerDelegatedRoot(element)` before `createRoot`
  (`web.js:370`).
- `hydrate` reaches `render` on every path except the new rejected-preload arm
  for a document root.

### 22. `render` — `reads` — archive `@solidjs/web@2.0.0-rc.9` — **WITHHOLD**

`web.js:368-400` (observe identical, `d86a26d9…`; dev `web.dev.js:426-464`).

**What changed from rc.3.** One line was added,
`if (options.onError) getOwner()[ROOT_ERROR_HOOK] = options.onError;`, inside
the created root (`:374`). It is a write onto the root owner. Signals' error
path later reads it to invoke the caller's hook, which is a `callbacks` path,
not a read.

**What the rest of the walk reads.**

- `code()` is the caller's.
- The document branch runs `effect(() => flatten(tree), () => {})`, which is
  transparent. Its compute runs at creation, and `flatten` invokes only
  functions inside the caller's tree.
- The element branch runs `insert(element, () => tree, …, { ...options.insertOptions, schedule: true })`.
  Its compute reads `accessor()` (`render`'s arrow over the caller's value)
  and nests an effect over a function-valued tree.
- `flush()` drains `render`'s own insert effects, whose bodies read nothing
  of their own, and computations third parties registered
  ([Decision 2026-09-10]).

**The guarded reach.** `options.insertOptions` is spread into `insert`'s
options (`:381`), and the package's `effect` (`:61-68`) sets `transparent:
!options.scope`. Once `enableHydration()` has run, `solid-js`'
`createRenderEffect` is `hydratedCreateRenderEffect` → `hydratedEffect`
(`solid.js:665-682`, byte-identical to rc.3). With `sharedConfig.hydrating`
and a non-transparent effect there are two gates:

- **G1**, `ssrSource === "client"` (rc.3 and rc.9): `withHydrationGate`
  (`:563-570`) creates a signal. A compute that `solid-js` authored reads it
  when signals computes the effect at creation (`recompute(node, true)`).
- **G2**, any other `ssrSource`, when `_$HY.r` holds the effect owner's id
  (rc.9 only): `readSerializedOrCompute` → `armLiveTakeover` creates the module
  `liveGate` signal if absent and reads it, on a second compute of this call's
  own effect or on a live-source result.

`insertOptions` is not in `render`'s declared options (`types/client.d.ts:129`).
Reader D ran `tsc` 5.9.3 against the published rc.9 typings **[M]**:

- a fresh literal is TS2353;
- `const opts = { renderId: "a", insertOptions: { scope: true, ssrSource:
  "client" } }; render(c, el, undefined, opts)` type-checks, and so does the
  same `hydrate(c, el, opts)`.

So this is a guarded reach. The runtime execution was read, not run (no DOM):
**[E]** on execution, **[M]** on the bytes.

**WITHHOLD.**

**Owner flag.** If an option absent from the declared type is ruled
unreachable, §§ 21-22 stand on the rest of the walk, which is clean. The same
fork applies to the rc.3 rows withdrawn in the rc.3 withdrawals audit §§ 4-5.

---

## 23. Tier-side admission of an rc.9 environment (no code change)

`scripts/ecosystem-benchmark/lib/consumer-environments.mjs` `environmentProblems`
checks each runtime package in this order:

1. an exact version and integrity;
2. for `solid-js` and `@solidjs/web` (`CEILINGED_RUNTIME`), **`compareVersions(pin.version,
   auditedSolid2) > 0` refuses and `continue`s** (`:114-118`);
3. only then, the audited-archive lookup and the integrity comparison
   (`:120-127`).

With this audit's archives in `rust/crates/solid-dialect/audited-archives.json`,
step 3 would admit exact rc.9 `solid-js`/`@solidjs/web` pins at these
integrities, and `@solidjs/signals` rc.9 is already listed. But step 2 still
refuses both before step 3 runs, because `AUDITED_SOLID_2` is `"2.0.0-rc.3"`
(`scripts/ecosystem-benchmark/lib/families.mjs:26`; the path is under
`ecosystem-benchmark/`, not `scripts/lib/`).

Admitting rc.9 needs one of two changes:

- **Raise `AUDITED_SOLID_2` to `"2.0.0-rc.9"`.** That constant is also the
  discovery ceiling. It feeds `select.mjs`' `solidReleaseCatalog` (`:20-30`)
  and the head tuple, which requires all three runtime packages at exactly
  that version (`:487-527`). It feeds `manifest.mjs:265`, which fails a
  checked-in manifest whose `auditedSolid2` differs. So raising it moves the
  corpus head: the manifest has to be regenerated and the census re-pinned. It
  also flips `consumer-environments.test.mjs:97-102`, which asserts that rc.9
  is refused as above the ceiling.
- **Or give consumer environments their own ceiling.** An environment-only
  ceiling, or an audited-archive membership check in place of the version
  comparison for the two `CEILINGED_RUNTIME` packages, admits rc.9
  environments without moving discovery.

---

## 24. What this changes

- **Archives.** `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` are listed
  in `AUDITED_ARCHIVES` and mirrored in `audited-archives.json`. Their tuples
  are checked against `phase0/rc9/{solid-js,solidjs-web}` (pinned manifest,
  export map, file list and tarball record), as `@solidjs/signals@2.0.0-rc.9`'s
  is.
- **Rows.** 15 new rc.9 rows: 14 flat and one scoped.
  - `solid-js`: `For`, `Repeat` and `Match` `reads` and `creates`;
    `createContext` and `useContext` `creates`; `createSignal` `creates`,
    scoped to `browser`.
  - `@solidjs/web`: `clientOnly`, `httpHeader` and `httpStatus` `reads` and
    `creates`.
- **Withheld on rc.9**, each in `IMPLEMENTATION_AUDITED`:
  - `solid-js`: `Show` `reads` and `creates`; `Loading` `creates`; flat
    `createSignal` `creates`; `affects` `reads` and `creates`; `isPending`
    `creates`; `latest` `creates`; `refresh` `reads` and `creates`.
  - `@solidjs/web`: `hydrate` and `render` `reads`.
- **Nothing binds today.**
  - A row binds only through the census's four-field identity gate on an
    authenticated snapshot, and no census row or checked-in consumer
    environment installs `solid-js@2.0.0-rc.9` or `@solidjs/web@2.0.0-rc.9`:
    § 23's ceiling refuses them.
  - The proposal side (`some_audit_denies_primitive`) is version-blind, and
    rc.3 already carries a row for every `(package, export, domain)` this
    audit grants, so no proposal answer moves.
- **Not covered.** A `solid-js` or `@solidjs/web` release other than rc.3 and
  rc.9. `@solidjs/signals` rows beyond the rc.9 signals audit's five. The
  client `Loading` hydration path's module import and fragment activation.
