# Audit: `solid-js`' own `createSignal` and `createMemo` in the rc.9 server builds — `node` and host-free rows

Date: 2026-09-28. Status: **for the repository owner's review**, as the rc.3,
rc.6 and rc.9 audits it follows were.

**Why it exists.** The Solid Primitives checkpoint
([`phase22/2026-09-28-solid-primitives-checkpoint.md`](../phase22/2026-09-28-solid-primitives-checkpoint.md),
order of work step 6) finds two dialect rows that answer under `browser` and
nowhere else:

| wall (run data, `measure{,-browser,-node}.json`) | domain | host free | `browser` | `node` |
| --- | --- | ---: | ---: | ---: |
| dialect-silent `solid-js:createSignal` | `creates` | 83 | 2 | 83 |
| dialect-silent `solid-js:createMemo` | `creates` | 68 | 0 | 68 |
| attribution catch-all "dialect row scoped to the browser host (solid-js)" (the same two rows, surfacing under `callbacks`) | `callbacks` | 16 | 0 | 8 |

Every other dialect-silent wall of the run (`createEffect`, `createStore`,
`mapArray`, `onCleanup`, `children`, `createOptimistic`, `createOwner`,
`createProjection`, `createReaction`) is silent under `browser` too, so it is
not a host-scope wall and is not read here. Every dialect-silent cause in the
three runs is a `creates` cause; none is `reads`, because the census consults
the negative table for `creates` only (`census_dialect_axiom` has one
production caller, the `creates` census in
`contract_certification/type_facts.rs`, and the proposal side's
`some_audit_denies_primitive` is called from `creates_walk.rs` only).

The two rows are `browser`-scoped because the flat rows were withheld for the
server builds: [RC9 core-and-web § 12](2026-09-27-solid-2-rc9-core-and-web-negative-rows.md)
for `createSignal`, [merge/omit/createMemo § 3](2026-09-28-solid-2-rc9-merge-omit-creatememo-creates.md)
for `createMemo`. Neither audit read those server bodies as the subject of a
`node` row. This one does, per claim domain (`creates`, `reads`), on the exact
rc.9 bytes, and asks two questions of each export:

1. Is a `node`-scoped row sound: does the claim hold on every file `node` can
   select (`dist/server{,.dev,.observe}.js`)?
2. Is a host-free (flat, `EveryCondition`) row sound: does it hold on those
   *and* on every file `browser` or no host can select
   (`dist/solid{,.dev,.observe}.js`)?

**Result.** No row is granted.

| # | row | `node` | host free | the reason |
| --- | --- | --- | --- | --- |
| 1 | `solid-js` `createSignal` `creates` | **WITHHOLD** | **WITHHOLD** | The derived overload builds a server `createMemo`, which hands a thenable or async-iterable compute result to `ctx.serialize` under `renderToStream` (`server.js:312` → `:364` → `:574`/`:715`/`:776`). **Probed**: in all three server builds, `createSignal(async () => 1)` inside `renderToStream` calls `ctx.serialize("1", <promise>)`, and the response carries the resolved value in its `_$HY.r` payload. A create under [Decision 2026-09-04]; a guarded reach still counts. |
| 2 | `solid-js` `createMemo` `creates` | **WITHHOLD** | **WITHHOLD** | The same reach, direct (`server.js:394` → `:364` → `processResult`). Probed for a promise and an async-iterable compute in all three server builds. |
| 3 | `solid-js` `createSignal` `reads` | clean on the bytes, **not granted** | **WITHHOLD** | Server: nothing is observed at the call but the caller's compute and properties of caller-supplied values. Browser: under hydration with `ssrSource: "client"` or `"hybrid"`, the export's own compute reads the gate signal `withHydrationGate` just created, on the call's stack (`solid.js:563-595`, measured eager). The `node` row is not added because the existing scope cannot state its one remaining premise (§ 3.3). |
| 4 | `solid-js` `createMemo` `reads` | clean on the bytes, **not granted** | **WITHHOLD** | As row 3. |

### What the owner is asked to agree with beyond the table

1. **`ctx.serialize` under `renderToStream` is a create, and `renderToString`
   does not change that.** Probed (§ 1.4): with no SSR context, inside a root
   with an id, and under `renderToString`, neither export calls any context
   method. `renderToString`'s context has no `async` flag
   (`@solidjs/web/dist/server.js:1326-1345`), so `processResult`'s
   `serializes` is false and the sync `serialize` there (which *throws* for an
   async value) is never reached from these two bodies. Only `renderToStream`
   (`async: true`, `:1697-1698`) makes the reach live, and a `node` consumer can
   call it. The row is a claim about every invocation under `node`, so the
   guarded reach withholds it: the rc.3 § 7.4 ruling and semantic-model
   § creates' "A guarded reach still counts". It is not the `haltReactivity` /
   `reportError` case the 2026-09-26 five-row audit ruled on (its "What the
   owner is asked to agree with" item 2): `reportError` hands a cause to the host's error
   channel and registers nothing that stays live, while `ctx.serialize` puts an
   entry keyed by the memo's owner id into the hydration payload the response
   carries, which the client's `sharedConfig.load` later reads as that memo's
   value. That is [Decision 2026-09-04]'s shape exactly: a version-1 resource
   (the memo's pending result) handed to a per-request runtime that acts on it
   after the call returns.
2. **The `node` `reads` verdict is clean, and the row is still not added.** The
   server closure calls no canonical `@solidjs/signals` primitive, so a
   `HostTargetScope` for it would carry no delegate — which
   `host_target_rows_state_checkable_premises` refuses — and it does construct
   signals' `NotReadyError`, `NoOwnerError` and `ContextNotFoundError` and key
   the accessor with signals' `$REFRESH`. The delegate premise is what pins, in
   the browser rows, that the `@solidjs/signals` beside the import is an
   audited archive; a delegate-less `node` row would answer beside any signals
   `^2.0.0-rc.9` admits, i.e. on unaudited bytes. The sound form needs a new
   premise kind ("exactly one authenticated, audited snapshot of this package
   is in the closure", with no delegated claim). And no count would move: the
   census does not consult `reads` rows. So the verdict is recorded and the row
   is left for when a `reads` consumer exists and that premise is built.
3. **The host-free `reads` withholding rests on the rc.6 line.** "It includes
   a read of a source the export itself created" (core-and-web § Method). The
   gate compute is the export's own closure (not the caller's `fn`), it runs on
   the call's stack because `@solidjs/signals@2.0.0-rc.9`'s `createMemo`
   computes at creation (measured, § 3.2), and it reads `hydrated()`, the
   accessor of the signal `withHydrationGate` created one line earlier. This is
   the same gate pattern [core-and-web § 22](2026-09-27-solid-2-rc9-core-and-web-negative-rows.md)
   withheld `render` `reads` for, reached here directly.

### What this audit does not do

- It reads `solid-js@2.0.0-rc.9` only, and only `createSignal` and
  `createMemo`. It does not read `deno` or `worker` (they select the same three
  server files, and no certification requests them), `merge` (withheld by the
  merge/omit/createMemo audit for this same server `createMemo`), or any other
  dialect-silent callee of the checkpoint.
- It does not change the `browser`-scoped `creates` rows, which stand on their
  own readings (core-and-web § 13, merge/omit/createMemo § 4).
- It does not decide the two `browser` `createSignal` exports the checkpoint
  still reports dialect-silent (`transition-group` `createSwitchTransition`,
  `trigger` `createTrigger`); those are `browser`-lane answers, not host scope.
- It builds no scope premise and grants no row.

---

## 0. Shared inputs

### 0.1 Identity

The three rc.9 archives were installed with `npm install --ignore-scripts
solid-js@2.0.0-rc.9 @solidjs/web@2.0.0-rc.9 @solidjs/signals@2.0.0-rc.9` into
an isolated `$TMPDIR` directory on 2026-09-28. The lockfile's integrities are
the audited tuples in `rust/crates/solid-dialect/audited-archives.json`:

| archive | integrity |
| --- | --- |
| `solid-js@2.0.0-rc.9` | `sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==` |
| `@solidjs/signals@2.0.0-rc.9` | `sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==` |
| `@solidjs/web@2.0.0-rc.9` | `sha512-pfiWoLDnLc+QYWc7UyLqO+5QrPEf3oTiNmmRC+C+uM6AZ5VH0bZMNPtLM5rJ29LKPiTwQitKV843IQDf/oeyhQ==` |

`diff -r` of each installed package against the `make audited-archives-provision`
tree (`rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules`) reports
no difference apart from the provisioner's own `.solid-checker-archive.json`
stamp. The runtime probes (§ 1.4, § 3.2) also need `@solidjs/web`'s
dependencies `seroval`/`seroval-plugins@1.6.7` and `csstype@3.2.3`, which the
same install fetched; they are the serializer, not a subject of any row. Every
file cited below has the sha256 and length that
`benchmarks/package-contract-v2/phase0/rc9/solid-js/files.json` pins:

| file | sha256 | bytes |
| --- | --- | ---: |
| `dist/server.js` | `9c25fe06f9aab7657bcd87c7ad4b12ac413ebab3f02bae651da253a7478db7f9` | 74,707 |
| `dist/server.dev.js` | `129cbe735cceed1ff627aaab146e3c7c5e2032ed0a1f980134334226651ab7e7` | 84,772 |
| `dist/server.observe.js` | `0d1519f0613584c577288aa2519c29fb42c5aa1446742f84f3559841ec22aede` | 79,326 |

### 0.2 Which file `node` selects

`solid-js@2.0.0-rc.9` `exports["."]` orders `worker`, `browser`, `deno`,
`node`, then `development`, `observe`, `default` (core-and-web § 0.2). A
certification requesting `node` without `browser` or `worker` selects
`dist/server.js`, `dist/server.dev.js` under `development`, and
`dist/server.observe.js` under `observe`. **Measured**: `node` resolves
`solid-js` to `dist/server.js` (plain), `dist/server.dev.js`
(`--conditions=development`) and `dist/server.observe.js`
(`--conditions=observe`), and `@solidjs/web` to its matching server file. The
declaration is `types/index.d.ts:8` (from `./client/hydration.js`) under every
condition, so a `solid-js` import of either name binds the `solid-js` archive.

### 0.3 How the walks were done

The § 0.4 method of the merge/omit/createMemo audit: a breadth-first walk over
top-level definitions (acorn, from `packages/cli/node_modules`), from the
export's definition, following every free identifier that names another
top-level definition of the same file, listing the imported bindings and free
globals reached, and hashing each reached definition's bytes. It
over-approximates (it links any same-named identifier; `resolve` below is a
local parameter name in `createDeferredPromise` that links the unrelated
top-level `resolve`). Every frame a verdict leans on was read by hand in
`server.js` and checked against the other two builds.

### 0.4 The probes

`probe-serialize.mjs` imports `solid-js` and `@solidjs/web` under the host's
conditions, wraps `serialize`, `registerFragment`, `hold`, `commit` and `block`
on `sharedConfig.context` whenever one is installed, and calls nine shapes
(`createSignal(value)`, `createSignal(fn)` sync and async, async with
`{deferStream}`, `createMemo` sync, async, async-iterable, async `{lazy}`, and
sync `{loadingValue}`) in four settings: no context and no owner; no context
inside `createRoot(…, {id})`; inside `renderToString`; inside
`renderToStream`, piping the stream to completion. `probe-browser-gate.mjs`
runs under `--conditions=browser`, calls `enableHydration()`, sets
`sharedConfig.hydrating` and hook `has`/`load` functions that log, and calls
`createMemo(() => 1, {ssrSource: "hybrid"})` and the `createSignal` equivalent
inside a root. Node v24.11.1. Type reachability was checked with TypeScript
5.9.3 (`--strict --module nodenext --skipLibCheck`) against the published
`types/`: every shape probed below type-checks. `--skipLibCheck` only silences
the known rc.9 declaration defect (`types/index.d.ts:3,8` re-export
`$DEVCOMP`, `sharedConfig`, `createErrorBoundary`, `createLoadingBoundary`,
`createRevealOrder`, which `./client/*.js` do not declare), which the
core-and-web audit records.

---

## 1. `createSignal` — `creates`, `node` host target — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

### 1.1 Definition

`dist/server.js:302-320` (bytes `8763..9387`, `d23963d5…`),
`server.observe.js:346-364` (the same slice digest) and
`server.dev.js:371-391` (`fc835fed…`; it adds `warnServerWrite("signal")`
inside both returned setters, which run only when the caller invokes them).

```js
function createSignal(first, second) {
  if (typeof first === "function") {
    // opts carries deferStream, ssrSource and loadingValue from `second`
    const memo = createMemo(prev => first(prev), opts);
    return [memo, () => undefined];
  }
  return [() => first, v => first = typeof v === "function" ? v(first) : v];
}
```

The value overload allocates two closures and nothing else. The derived
overload is the server `createMemo` of § 2, with an export-authored compute
that calls the caller's `first`.

### 1.2 The walk [M]

From `createSignal` and `createMemo`, the walk reaches 40 definitions in
`server.js`, 41 in `server.observe.js` and 45 in `server.dev.js`. Every
definition common to the three is byte-identical across them except
`allocateOwner` (dev and observe add `owner._name = undefined`),
`clientHoleRead` (dev and observe add `recordFinding(…)`, reached only from the
returned accessor), and dev's `createSignal`. The dev/observe additions are
`recordFinding`, `emitFinding`, `devCheck`, `warnServerWrite` and
`warnedServerWrites`, all reached only through a returned setter or accessor.

The definitions reached from the call event: `createOwner`, `allocateOwner`,
`nextChildIdFor`, `formatChildId`, `runWithOwner`, `stampThrower`,
`runWithObserver`, `resetOwnerForRerun`, `disposeOwner`, `unlinkOwner`,
`getContext`, `processResult`, `settleServerAsync`, `createDeferredPromise`,
`isThenable`, `subscribePendingRetry`, `closeAsyncIterator`,
`inServerComponentScope`, `createSyncMemo`, and the module state
`sharedConfig`, `currentOwner`, `ownerPool`, `Observer`, `SLOTS`,
`CLIENT_HOLE`, `LIVE_SOURCE`, `NoHydrateContext`. Imported bindings, all from
`@solidjs/signals`: `NotReadyError`, `$REFRESH`, `NoOwnerError`,
`ContextNotFoundError`; dev adds `DEV` and `OBSERVE` (through the finding
helpers only). Free globals: `Array`, `Error`, `Object`, `Promise`, `String`,
`Symbol`, `WeakMap`, and dev's `Set`.

### 1.3 The reach

`processResult` (`server.js:516-819`, `fb9c55af…` in all three builds) is
handed the compute's result and `sharedConfig.context` as `ctx`. It calls
`ctx.serialize(id, …, deferStream)` at four sites, each under
`ctx.async ∧ ctx.serialize ∧ id ∧ ¬noHydrate`:

| site | `server.js` | `server.dev.js` | `server.observe.js` | reached when the result is |
| --- | ---: | ---: | ---: | --- |
| the pending thenable | 574 | 645 | 618 | a thenable with no settled slot |
| the hybrid async iterable | 715 | 786 | 759 | an async iterable, `ssrSource: "hybrid"` or a live source |
| the plain async iterable | 776 | 847 | 820 | an async iterable otherwise |
| a served `loadingValue` | 813 | 884 | 857 | synchronous, after a `loadingValue` was served |

The guard, for the derived overload:

- `node` (or `worker`/`deno`) selected a server build;
- `sharedConfig.context` is a `renderToStream` context (`async: true`,
  `@solidjs/web/dist/server.js:1697-1698`, whose `serialize` at `:1760-1779`
  writes into the response's serializer);
- `ssrSource !== "client"` and not `lazy` (otherwise `update()` does not run at
  creation);
- the owner has an id (any owner under a render root);
- no `NoHydrate` ancestor and `options.serialize !== false`;
- a thenable or async-iterable result, or a served `loadingValue`.

`createSignal(async () => …)` is type-correct (§ 0.4).

### 1.4 Probed [M]

In each of `server.js`, `server.dev.js` and `server.observe.js`:

| setting | `createSignal(value)` | `createSignal(fn)` sync | `createSignal(async fn)` | `… {deferStream: true}` |
| --- | --- | --- | --- | --- |
| no context, no owner | none | none | none | none |
| no context, root with an id | none | none | none | none |
| `renderToString` | none | none | none | none |
| `renderToStream` | none | none | `ctx.serialize("1", <thenable>)` | `ctx.serialize("2", <thenable>)` |

"none" means no context method was called. The `renderToStream` response of
the plain build ends with `_$HY.r["2"]=1` (the `deferStream` memo's value,
written after the blocking promise settled) and a batch that resolves ids `1`
and `4` with `1` and `2`: the serialized memos' values, delivered to the
client's hydration payload.

### 1.5 Verdict

**WITHHOLD** for `node`, and so for host free. The reach is the export's own
act (`createSignal` builds the memo; its compute calls the caller's `first`,
but the hand-off to `ctx.serialize` is `processResult`'s), and it is the
registration [Decision 2026-09-04] names. The value overload is clean, but a
row is keyed by export, not by overload; a row that could say "the first
argument is not a function" would need an argument-shape premise that
`HostTargetScope` does not have.

---

## 2. `createMemo` — `creates`, `node` host target — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

### 2.1 Definition

`dist/server.js:321-410` (`9388..12085`), `server.dev.js:392-481`
(`13006..15703`), `server.observe.js:365-454` (`10629..13326`), one slice
digest `e72123d4…` (the same bytes the merge/omit/createMemo audit § 3 read for
the flat row). `options.sync` routes to `createSyncMemo` (`:411-455`,
`701710a3…` in all three), which calls the caller's compute and
`ctx?.commitEpoch?.()` and nothing else.

### 2.2 The reach

The same `processResult` sites as § 1.3, directly: unless `ssrSource ===
"client"` or `lazy`, `update()` runs at creation (`:393-395`), and its result
goes to `processResult` (`:364`). § 1.3's guard applies with the caller's
compute in place of the derived wrapper.

### 2.3 Probed [M]

In each of the three server builds:

| setting | sync | async | async iterable | async `{lazy}` | sync `{loadingValue}` |
| --- | --- | --- | --- | --- | --- |
| no context (with or without a root) | none | none | none | none | none |
| `renderToString` | none | none | none | none | none |
| `renderToStream` | none | `ctx.serialize("4", <thenable>)` | `ctx.serialize("5", <async iterable>)` | none | none |

### 2.4 Verdict

**WITHHOLD** for `node`, and so for host free, for § 1.5's reason.
`createMemo(async () => …)` and an async-iterable compute are type-correct
(§ 0.4).

---

## 3. `createSignal` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

### 3.1 The server builds: clean on the bytes

At the call event the server closure (§ 1.2) observes:

| what is read | where | disposition |
| --- | --- | --- |
| `first(prev)` / `compute(comp.value)` | `:312`, `:354` | **caller** (a caller-supplied callable; `comp.value` is handed to it, not observed) |
| `options.*`, `second.*` (`deferStream`, `ssrSource`, `loadingValue`, `lazy`, `sync`, `id`, `transparent`, `serialize`) | `:304-311`, `:322-330`, `:48-49` | **caller** (properties of a caller-supplied receiver, [Decision 2026-09-10]) |
| `result.s`, `.v`, `.d`, `.then`, `[Symbol.asyncIterator]`, `iter.next()` | `processResult`, `settleServerAsync` | **caller** (the value the caller's compute returned) |
| `err.source` of a thrown `NotReadyError` | `subscribePendingRetry`, `:268-272` | **caller** (thrown by the caller's compute) |
| owner fields, `getContext(NoHydrateContext, owner)`, `sharedConfig.context`, `ctx[SLOTS]` | `:51-86`, `:119-126`, `:325`, `:544` | **local** bookkeeping: owner and render-context records, no reactive source |
| `ctx.commit?.()`, `ctx.commitEpoch?.()`, `ctx.hold?.()` | `processResult`, `createSyncMemo` | **hook**: `@solidjs/web@2.0.0-rc.9` installs no `commit` or `commitEpoch`; `hold` (`:1742-1750`) counts pending holds. An installed third-party context is installed-hook behaviour (core-and-web flag F1). |
| resets of the memo's own owner on a retry (`resetOwnerForRerun` → `disposeOwner` → disposal callbacks) | `:186-190`, `:146-185` | **caller** (cleanups the caller's compute registered) |

Scheduled work (the retry `update` that `subscribePendingRetry` and
`settleServerAsync` schedule) re-runs the caller's compute and writes
`comp.value`. The server build has no reactive graph: the memo is `read`, a
closure over `comp` that the call returns and does not invoke. So nothing the
export itself does observes a reactive source's current value. The dev and
observe differences (§ 1.2) are in returned values only.

### 3.2 The browser builds: the gate read [M]

`dist/solid.js`'s wrapper (core-and-web § 13) reaches `hydratedCreateSignal`
once `@solidjs/web`'s `hydrate` has called `enableHydration()`. For the
derived overload while `sharedConfig.hydrating`, it runs
`hydrateSignalLike(createSignal$1, fn, second)` (`:602-605`), whose two gated
branches are (`:571-590`):

```js
if (ssrSource === "client") {
  return withHydrationGate(hydrated => coreFn(prev => {
    if (!hydrated()) return UNASKED;
    return fn(prev);
  }, options));
}
if (ssrSource === "hybrid" && sharedConfig.has(peekNextChildId(getOwner()))) {
  …
  return withHydrationGate(hydrated => coreFn(prev => {
    if (hydrated() && takeover) return fn(prev);
    return readSerializedOrCompute(detect, prev, options);
  }, options));
}
```

`withHydrationGate` (`:563-570`) creates `createSignal$1(false, {ownedWrite:
true})`, passes its accessor as `hydrated`, and writes it only after `create`
returns. The compute handed to `coreFn` is the export's own closure, and its
first act is `hydrated()`. `probe-browser-gate.mjs` measured that the compute
runs on the call's stack: for `createMemo(() => 1, {ssrSource: "hybrid"})`
(owner id `t0`) and `createSignal(() => 1, {ssrSource: "hybrid"})` (`t1`)
under a root with id `t`, the logging `has` hook is called twice before each
call returns, both times with that call's id: once by `hydrateSignalLike`'s
own `sharedConfig.has(peekNextChildId(…))`, and once by
`readSerializedOrCompute` *inside the compute*, which is reached only after
`hydrated() && takeover` has evaluated `hydrated()`. So the export reads a
signal it created, during the call. `armLiveTakeover` (`:188-198`) adds a read
of `liveGate()` on the same stack on a second compute. `solid.dev.js`
(`withHydrationGate` and `hydrateSignalLike` at `:588-620`) and
`solid.observe.js` (`:575-607`) carry the byte-identical closure (core-and-web
§ 13's measured 27 definitions). The guard is
`sharedConfig.hydrating` ∧ the derived overload ∧ `ssrSource ∈ {client,
hybrid}` (hybrid also ∧ a serialized slot), and `{ssrSource: "client"}` and
`"hybrid"` are type-correct options (§ 0.4).

### 3.3 Verdict

- **Host free: WITHHOLD**, for § 3.2's read of a source the export created. A
  guarded reach still counts.
- **`node`: clean on the bytes, not granted.** § 3.1 supports the claim on all
  three server builds. The row is not added because the scope has no premise
  for the one remaining input (owner item 2): the server closure reaches
  `@solidjs/signals` only through the non-primitive `NotReadyError`,
  `NoOwnerError`, `ContextNotFoundError` and `$REFRESH`, so there is no
  canonical delegate through which to require an audited signals archive, and
  `HostTargetScope` requires at least one delegate. Nothing consults a `reads`
  row today, so the withholding moves no count.

---

## 4. `createMemo` — `reads` — archive `solid-js@2.0.0-rc.9` — **WITHHOLD**

Server: § 3.1's table without the `first(prev)` wrapper; the compute is the
caller's `compute`, and `createSyncMemo` reads nothing further. Browser:
`hydratedCreateMemo` (`solid.js:596-601`) enters the same `hydrateSignalLike`
(`coreFn` = `createMemo$1`) whenever `sharedConfig.hydrating ∧
¬options.transparent`, so § 3.2's gate read applies, measured for the
`hybrid` branch. **Host free: WITHHOLD. `node`: clean on the bytes, not
granted,** for § 3.3's reasons.

---

## 5. Citations

Byte offsets are of the whole file, read in binary. Each slice runs from the
`function <name>(` token through its closing `}`, the 2026-09-23 convention.
Each token is the only definition of its name in its file (checked), and each
file's sha256 and length equal the `files.json` pin (§ 0.1). No row cites
these, because no row is granted; they are the withholdings' subjects.

| subject | `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` |
| --- | --- | --- | ---: | ---: | --- |
| `createSignal` | `dist/server.js` | 302-320 | 8763 | 9387 | `d23963d5bc2a7192270430bb5e9b7acbf344ccc962c6848f5738eb64f7cbe84b` |
| `createSignal` | `dist/server.dev.js` | 371-391 | 12317 | 13005 | `fc835fed35274a0c89521c31d62a2e3e0495f1c7d3c549f4c862aad1ffee9006` |
| `createSignal` | `dist/server.observe.js` | 346-364 | 10004 | 10628 | `d23963d5bc2a7192270430bb5e9b7acbf344ccc962c6848f5738eb64f7cbe84b` |
| `createMemo` | `dist/server.js` | 321-410 | 9388 | 12085 | `e72123d422b4e95270be8a08ec711bba70708e167623b18ac2f72453bc9ef03f` |
| `createMemo` | `dist/server.dev.js` | 392-481 | 13006 | 15703 | `e72123d422b4e95270be8a08ec711bba70708e167623b18ac2f72453bc9ef03f` |
| `createMemo` | `dist/server.observe.js` | 365-454 | 10629 | 13326 | `e72123d422b4e95270be8a08ec711bba70708e167623b18ac2f72453bc9ef03f` |
| `processResult` | `dist/server.js` | 516-819 | 15322 | 24796 | `fb9c55af7e348d868b783101b6c6a4574bf076ddb3aa14099ddb7b53722aeb94` |
| `processResult` | `dist/server.dev.js` | 587-890 | 18940 | 28414 | `fb9c55af7e348d868b783101b6c6a4574bf076ddb3aa14099ddb7b53722aeb94` |
| `processResult` | `dist/server.observe.js` | 560-863 | 16563 | 26037 | `fb9c55af7e348d868b783101b6c6a4574bf076ddb3aa14099ddb7b53722aeb94` |
| `createSyncMemo` | `dist/server.js` | 411-455 | 12086 | 13076 | `701710a38c71d2fff9d61c8e86c905b6756d665c481a23ba8ed263655c45a952` |

## 6. What this changes

- **Rows.** None. The `browser`-scoped `creates` rows for `createSignal` and
  `createMemo` stay the only rows for either export.
- **Withholdings recorded.** Two new ones, both host free:
  `(solid-js, 2.0.0-rc.9, createSignal, Reads)` and
  `(solid-js, 2.0.0-rc.9, createMemo, Reads)`, in `solid_2.rs`'s
  `IMPLEMENTATION_AUDITED` and `WITHHELD`, so a later flat `reads` row has to
  come through this reading. The flat `creates` tuples were already withheld;
  this audit adds the `node` reading beside them and pins, in a test, that
  neither export carries a `node` row in either domain.
- **What moves.** Nothing: the checkpoint's dialect-silent counts stay 180 /
  67 / 182 exports (host free / `browser` / `node`), with `createSignal` 83 / 2
  / 83 and `createMemo` 68 / 0 / 68, and clean exports 96 / 96 / 93.

## 7. What would move the wall

- **`creates`, `node`.** Nothing on these bytes: the reach is real under
  `renderToStream`. A row could exist only with a premise that excludes it:
  an argument-shape premise for `createSignal`'s value overload (the one
  overload that is clean everywhere, § 1.1), or a premise that the certified
  consumer never runs under a streaming render context, which no certification
  can establish. Either is a new `HostTargetScope` premise kind, recorded here
  as design rather than built.
- **`reads`, `node`.** A companion-archive premise (§ 3.3) and a `reads`
  consumer in the census.

## 8. Residual approximations

- The walks are name-based over-approximations (§ 0.3); dispatch through
  `ctx` and caller values is dispositioned by hand.
- The server `reads` reading treats `sharedConfig.context`, owner records and
  `ctx[SLOTS]` as bookkeeping rather than reactive sources, because the server
  build defines no reactive graph; a third-party context's `commit` /
  `commitEpoch` / `hold` hooks are installed-hook behaviour.
- The probes exercise `@solidjs/web@2.0.0-rc.9`'s two render contexts. A
  different renderer installing `sharedConfig.context` with `async` and
  `serialize` reaches the same sites; that is why the verdict rests on the
  `solid-js` bytes, not on the probe.
