# Review: does the rc.3-audited Solid 2 vocabulary hold for `solid-js@2.0.0-rc.9`?

Date: 2026-09-26. Status: **for the repository owner's review**. This is a
review, not a dialect change: no row, no `AUDITED_SOLID_2` entry, and no
dialect answer was changed.

**Why it exists.** Dialect selection goes by major version only
(`rust/crates/solid-facts-backend/src/dialect.rs:497-515`). The 2026-09-26
sweep therefore analysed `solid-js@2.0.0-rc.9` (solid-primitives `next`) and
`solid-js@2.0.0-experimental.1` (corvu) with the vocabulary that was read on
rc.0/rc.3 bytes. AGENTS.md says "a newer prerelease must be reviewed rather
than silently substituted", and the backlog entry "Newer Solid 2 prereleases
are analysed with rc.3 vocabulary (2026-09-26)" deferred that review. This
document is that review.

Every claim is tagged **[M]** (measured: cited to a file and line, a digest,
or a command run on the named bytes) or **[E]** (estimated: inferred from
code reading or from a neighbouring version, not executed on rc.9).

## 0. Recommendation

**(b) rc.9 is vocabulary-compatible except for the four items in § 4, and
those need dialect changes.** Every timing, tracking, ownership and
write-guard premise the rules rest on is unchanged on rc.9's bytes (§ 3). But
rc.9 changes four things the vocabulary answers by name, and on each one the
checker either certifies code that rc.9 breaks or states a claim rc.9 does not
honour:

| # | rc.9 change | what the rc.3 vocabulary does with it | tag |
| --- | --- | --- | --- |
| B1 | `Store<T>` is now `T`, not `Readonly<T>`. `tsc` no longer rejects a write to a store's root property. | `store_root_properties_are_readonly() == true` makes SC2003 skip root writes, so that `tsc` reports them. Under rc.9 neither reports them, but the runtime still drops the write. | [M] |
| B2 | `dynamic(source, { static: true })` calls `source` once, untracked, during the call. | It models `dynamic`'s argument 0 as `Tracked`, `Creates` owner. That is false for the static form, both for the checker's read attribution and for any contract it emits. | [M] bytes, [E] contract effect |
| B3 | `omit(props, hidden)` takes a predicate function that omit invokes. | `splits_props(Omit)` skips every `omit` argument as a value (`interproc.rs:2246-2254`), which misses a callable argument that is invoked. | [M] bytes and code, [E] finding effect |
| B4 | New callback-taking export `until(fn, options)`. It throws inside a reactive scope and runs `fn` as a tracked compute under a fresh root. | It is not in `TABLE`. A probe project that uses it gets `"status": "certified"`. That breaks the completeness criterion of `every_callback_taking_export_is_modelled_or_excluded`. | [M] |

B2, B3 and B4 are additive. rc.3's own typings reject each rc.9 form (a
two-argument `dynamic` is TS2554, a predicate `omit` is TS2345, `until` is
TS2305, all measured in § 2.3), so modelling them cannot change an rc.3
answer. B1 cannot be fixed that way. The right answer differs between two
prereleases of the same major (rc.3: readonly, rc.9: not), so it needs the
first *prerelease-sensitive* dialect answer. The installed version is already
in `Detection::Installed { version, manifest }`, so the seam can carry it.
Answering `false` for both versions would duplicate TS2540 on rc.3, which the
absolute rule forbids.

Until B1-B4 land, the owner has to choose what rc.9 projects get. The review
does not refuse them. My advice is a visible, non-refusing notice at selection
time. The reason is B1: today `--certify` reports `certified` for an rc.9
project that writes a store's root property outside a setter, and rc.9 drops
that write (§ 4.1). Refusal, option (c), would refuse most current Solid 2
projects for four items, three of them additive. The backlog already
records that trade-off.

**Negative rows stay archive-scoped.** No rc.9 archive is listed in
`AUDITED_ARCHIVES`, so no row answers for rc.9 today. § 7 names five
`@solidjs/signals@2.0.0-rc.9` `creates` rows that could be granted cheaply,
and says what they would still need.

**`2.0.0-experimental.1` (corvu) should be refused, not treated as pre-rc.**
See § 8.

## 1. The bytes compared

| package | rc.3 (audited) | rc.9 (under review) |
| --- | --- | --- |
| `solid-js` | kobalte sweep tree, `.pnpm/solid-js@2.0.0-rc.3` | solid-primitives sweep tree, `.pnpm/solid-js@2.0.0-rc.9` |
| `@solidjs/signals` | `.pnpm/@solidjs+signals@2.0.0-rc.3` | `.pnpm/@solidjs+signals@2.0.0-rc.9` |
| `@solidjs/web` | `.pnpm/@solidjs+web@2.0.0-rc.3_solid-js@2.0.0-rc.3` | `.pnpm/@solidjs+web@2.0.0-rc.9_solid-js@2.0.0-rc.9` |

Both trees are under the session scratchpad `sweep/consumers/`. In the
citations below, `<r3js>`, `<r3sig>`, `<r3web>`, `<r9js>`, `<r9sig>` and
`<r9web>` are those package roots.

- **rc.3 identity [M].** Every file of the three rc.3 installs matches
  `benchmarks/package-contract-v2/phase0/rc3/*/files.json` by sha256: 43/43
  (`solid-js`), 107/107 (`@solidjs/signals`) and 103/103 (`@solidjs/web`),
  with no extra files. So `<r3*>` is the audited rc.3.
- **rc.9 identity [M].** `package.json` sha256 values:
  `solid-js` `c8d6224bd4bd63ed38f0cec77eb4d30d3f78debec091338dc4e466a2618e11bc`,
  `@solidjs/signals` `c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6`,
  `@solidjs/web` `5de9244eb8121c5efe10f3976a06b09ca09cb99de4caf5fcf56dc040ce9773dc`.
  File counts are 33, 119 and 67.
- **rc.9 integrities.** Copied from solid-primitives' `pnpm-lock.yaml`, lines
  9038, 4881 and 4909 [M]. No tarball was downloaded to re-derive them [E]:
  - `solid-js`: `sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==`
  - `@solidjs/signals`: `sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==`
  - `@solidjs/web`: `sha512-pfiWoLDnLc+QYWc7UyLqO+5QrPEf3oTiNmmRC+C+uM6AZ5VH0bZMNPtLM5rJ29LKPiTwQitKV843IQDf/oeyhQ==`
- **Dependency pins [M].** rc.9 `solid-js` depends on
  `@solidjs/signals: ^2.0.0-rc.9` and `seroval ~1.6.7`. rc.3 depended on
  `^2.0.0-rc.3` and `seroval ~1.5.4`. Both trees resolve exactly one signals
  copy at the matching version.
- **Main bundles digested [M] (sha256, bytes):**
  - `<r3sig>/dist/dev.js`: `cc68ed0f…1a79`, 420,677
  - `<r9sig>/dist/dev.js`: `f08c227c…2120`, 418,037
  - `<r9sig>/dist/dev-shared.js` (new): `70b88ba9…463e`, 295,301
  - `<r3js>/dist/dev.js`: `dfc36239…ac18`, 42,351
  - `<r9js>/dist/solid.dev.js`: `7baf8808…11de`, 45,011
  - `<r3web>/dist/dev.js`: `d848d003…7851`, 71,187
  - `<r9web>/dist/web.dev.js`: `bcbe0218…129d`, 86,356
- **Reference text [E].** The canonical RFCs are `documentation/solid-2.0` in
  solidjs/solid. No rc.9 checkout was available offline and none was fetched.
  RFC corroboration below comes from the rc.7-era text in the unpinned cargo
  checkouts of `yumemi-thomas/solid` (`16f0988`, `9d2ccb9`, both monorepo
  version `2.0.0-rc.7`). The published rc.9 bytes are this review's authority.

## 2. Method

1. **Declared surface [M].** The TypeScript 5.9.3 compiler API
   (`packages/cli/node_modules/typescript`) walked every non-wildcard
   `exports` subpath's `types` target of each package. For each export it
   compared the normalized declaration text, comments stripped. The signals
   comparison is the authority for re-exported core names. (The solid-js
   level resolved its `@solidjs/signals` re-exports without declarations in
   this harness, so those rows were read at the signals level instead.)
2. **Runtime surface [M].** Node 24.11.1 imported each ESM bundle file
   directly and listed `Object.keys` of the namespace.
3. **Function slices [M].** The TypeScript parser extracted top-level
   function, class and const slices from the bundles and compared them by
   name and sha256. A differing slice was read as a diff.
4. **Runtime probes [M].** The rule-relevant cases were executed on
   `<r3sig>` and `<r9sig>`, both `dist/dev.js` and `dist/prod/index.js`. The
   30 cases and their results are listed where they are used.
5. **`tsc` against the published typings [M].** `tsc` 5.9.3 ran with
   `strict`, bundler resolution and `jsxImportSource: "@solidjs/web"`, over
   `node_modules` links to the real installs. No stubs were involved.
6. **Checker probes [M].** Probes ran on the main checkout's
   `rust/target/debug/solid-checker-rust` (sha256 `6ad761f6…97e2`, built
   2026-09-26 14:23), with `bin/solid-typefacts` (sha256 `fc93317f…aaf`). The
   binary's source revision was not re-derived; it is estimated to be the
   `codex/phase19a-authenticated-proof-policy` tip, `53c9a41b` [E]. The same
   source file was checked twice, once over an rc.3 install and once over an
   rc.9 install. Only the installed trees differ.

### 2.1 Surface counts [M]

| subpath | added | removed | declaration changed | unchanged |
| --- | ---: | ---: | ---: | ---: |
| `solid-js` `.` | 50 | 10 | 10 | 110 |
| `solid-js/internal` (new) | 29 | — | — | — |
| `solid-js/attribution` (new) | 0 | — | — | — |
| `solid-js/refresh` | 0 | 0 | 0 | 12 |
| `@solidjs/signals` `.` | 49 | 0 | 16 | 90 |
| `@solidjs/signals/attribution` (new) | 35 | — | — | — |
| `@solidjs/web` `.` | 25 | 2 | 15 | 121 |
| `@solidjs/web/server-functions{,/client}` | 12 | 6 | 5 | 33 |

Runtime namespace changes [M]:

- **`solid-js` root** (all three client, dev and server bundles):
  - added `OBSERVE`, `ROOT_ERROR_HOOK`, `TimeoutError`, `configureClientErrors`,
    `isStatic`, `reportServerError`, `ssrSanitizeError` and `until`;
  - removed `$REFRESH`, `NoHydrateContext` and `storePath`.
- **`@solidjs/signals` root:** 27 added (the list above plus the merge/omit
  view helpers `MergeView`, `OmitView`, `SOURCE_*`, `sourceGet`, …), none
  removed.
- **`@solidjs/web` root:** added `NULL_BODY_STATUSES`,
  `RESPONSE_HEADER_VALUE_LIMIT`, `REVALIDATE_ALL`, `configureServerErrors`,
  `getTraceContext` and `readShallow`; removed `patchDriver` and `rowProof`.

**None of the 52 `TABLE` names is removed at runtime [M].**

### 2.2 Packaging [M]

- **CommonJS is gone.** rc.9 ships no `require` condition, no `.cjs` bundle
  and no `types-cjs/` in any of the three packages.
- **A new `observe` condition** selects a third build: `dist/observe/**` for
  signals and `*.observe.js` for solid-js and web.
- **The signals dev bundle is split** into `dist/dev.js` and
  `dist/dev-shared.js`.
- **Server builds have a dev bundle:** `server.dev.js`.
- **solid-js's `./types/*` subpath is gone.** New subpaths are
  `solid-js/attribution`, `solid-js/internal` and
  `@solidjs/signals/attribution`.

None of this changes a vocabulary answer. It does change what "every bundle
the `exports` map can select" means for any rc.9 row: `dist/prod/**`,
`dist/dev.js` plus `dist/dev-shared.js`, and now `dist/observe/**` [M].

### 2.3 `tsc` over the published typings [M]

A probe file imports all 52 `TABLE` names from `solid-js` and `@solidjs/web`
and exercises the shapes the rows restate: the `Signal` tuple, the
`createEffect(compute, apply)` split, the `createStore` draft setter, the
`For`/`Show` keyed forms, and `Loading`/`Errored`.

- With `skipLibCheck: true`, it type-checks clean on both rc.3 and rc.9.
- With `skipLibCheck: false`, rc.9 alone reports five errors in its own
  declarations (§ 5, N2): TS2305 at `<r9js>/types/index.d.ts(3,10)` for
  `$DEVCOMP`, and at `(8,10)`, `(8,53)`, `(8,74)` and `(8,97)` for
  `sharedConfig`, `createErrorBoundary`, `createLoadingBoundary` and
  `createRevealOrder`.

Rule-premise cases, `tsc` result on each version:

| case | rc.3 | rc.9 |
| --- | --- | --- |
| `createEffect(() => x())`, one argument | clean (the `never` overload) | TS2554 |
| `createRenderEffect(() => x())`, one argument | TS2554 | TS2554 |
| `store.a = 2` on a `createStore` root | TS2540 | clean |
| `projected.a = 5`, `optimistic.a = 3` (root writes) | TS2540 | clean |
| `omit(props, key => …)` | TS2345 | clean |
| `dynamic(src, { static: true })` | TS2554 | clean |
| `import { until }` | TS2305 | clean |

## 3. Inventory: what the dialect assumes, rc.3 against rc.9

`solid_2.rs` line numbers are this branch's.

The classes are:

- **U**: unchanged.
- **X**: export renamed, added or removed.
- **S**: signature change.
- **R**: semantic change that affects a rule.
- **I**: irrelevant change.

### 3.1 Export vocabulary

| assumption | source | rc.3 → rc.9 evidence | class |
| --- | --- | --- | --- |
| The 52 `TABLE` names are exports of `solid-js` / `@solidjs/web`. | `solid_2.rs:36-89` | All 52 are still runtime exports in every client, dev and server bundle [M]. At the declaration level, four resolve to nothing in rc.9 (§ 5, N2) [M]. | U (runtime); declaration defect for four names |
| Namespace-import lists | `solid_2.rs:3242-3311` | Same 45 + 16 names, all exported [M]. | U |
| Where-exported tables | `exports/solid_v2_solid_js.rs`, `exports/solid_v2_solidjs_web.rs` | The tables still list `$REFRESH`, `NoHydrateContext`, `storePath`, `creationStamp`, `getProjectionTrace`, `inServerComponentScope`, `materializeContainerTrace`, `runInServerComponentScope`, `ssrHandleError` and `ssrScope` under `solid-js`. rc.9 removed them from the root; the `ssr*`/trace internals moved to `solid-js/internal` [M]. None is a vocabulary primitive, and importing one from the root is TS2305 on rc.9 [E]. rc.9 additions (`until`, `isStatic`, `TimeoutError`, `configureClientErrors`, `OBSERVE`, …) are absent from the tables [M]. | X, I except `until` (B4) |
| Every callback-taking `solid-js`/`@solidjs/web` export is modelled or excluded | `solid_2.rs:3615-3669` (test over the rc.3 bundles) | rc.9 adds callback-taking forms: `until(fn, options)` (`<r9sig>/dist/types/signals.d.ts:608`), the `omit` predicate (`store/utils.d.ts:251`), and `onError` hooks inside `render`/`hydrate` options and `configureClientErrors`/`configureServerErrors` config [M]. | X: B4, B3 |

### 3.2 Declarations the dialect restates

| assumption | source | evidence | class |
| --- | --- | --- | --- |
| `createSignal` → `Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`; `createMemo` → `SourceAccessor<T>` | `reactive_result_slot`, `solid_2.rs:2726` | `createSignal` (`signals.d.ts:262-264`), `createMemo` (`:295,298`), `Signal`, `SourceAccessor`, `Accessor`, `Setter` and `Refreshable` declarations are textually unchanged [M]. | U |
| `For`/`Show`/`Match`/`Repeat` callback shapes per `keyed` form | `children_accessor_parameters`, `:2665` | `<r3js>/types/client/flow.d.ts` = `<r9js>/types/client/flow.d.ts`, sha256 `81af6e73…148a`, byte-identical [M]. | U |
| `createStore` root is `Readonly` (root writes belong to `tsc`) | `store_root_properties_are_readonly`, `:2513`; used at `shared_reactivity.rs:616-640` | `Store<T> = Readonly<T>` (`<r3sig>/dist/types/store/store.d.ts:4`) → `Store<T> = T` (`<r9sig>/…/store.d.ts:4`) [M]. Also in the rc.7-era source, `packages/signals/src/store/store.ts:8` [M]. | **R: B1** |
| `options_argument`: memo/signal/optimistic/trackedEffect → 1; store/projection/optimisticStore/effect/renderEffect → 2 | `:2743` | `createStore` plain form keeps options at 1 and derived form at 2, with `shallow` moved into `StoreOptions` (`store/index.d.ts:13-14`). `createOptimisticStore`'s plain form *gains* `options?: StoreOptions` at 1 (`store/next/optimistic.d.ts:12`). Derived-form positions are unchanged [M]. | S, I (rule only asks about computes, derived form) |
| `supports_sync_option` for the signal family only | `:2768` | `computed` still maps `options?.sync` → `CONFIG_SYNC` (`<r3sig>/dist/dev.js:3597`, `<r9sig>/dist/dev-shared.js:4824`) [M]. `ProjectionOptions` still has no `sync` [M]. | U |
| `createEffect(compute, apply)`; one-argument form is the type-correct `never` overload | `effect_api.rs:176-181`, `docs/rules/missing-effect-function.md` | rc.3 `signals.d.ts:367,378` (two overloads) → rc.9 `:367` only [M]. | S (see N1) |
| `omit(props, ...keys)`, a props-plus-key-lists shape | `splits_props`, `:3145`; `interproc.rs:2246-2254` | rc.9 adds `omit(props, hidden: (key) => boolean)` (`store/utils.d.ts:251`), and the runtime selects it by `typeof keys[0] === "function"` (`<r9sig>/dist/dev.js:4380`) [M]. | **R: B3** |
| `dynamic(source)` | `callback_positions`/`callback_executions`/`callback_owners` for `Dynamic` | `dynamic(source, options?: DynamicOptions)` with `static?: boolean` (`<r9web>/types/index.d.ts:82-97`) [M]. | **R: B2** |
| `refresh(target): void` | SC2001 refresh arm | `refresh` now returns `Promise<…>` (`<r9sig>/dist/types/signals.d.ts:534`, was `core/core.d.ts:163`) [M]. The rule keys on the call, not the result [E]. | S, I |
| `render`/`hydrate(fn, el, init?, options?)` | `callback_positions`/`callback_owners` for mount | Only an `onError?: ClientErrorHook` option was added (`<r9web>/types/client.d.ts:129,223`) [M]. | S, I |

### 3.3 Timing, tracking and ownership

| assumption | source | rc.9 evidence | class |
| --- | --- | --- | --- |
| Non-lazy `computed` runs its compute during the creating call (`createMemo`, `createSignal(fn)`, `createOptimistic(fn)`, `createProjection`, and `createStore(fn)`, measured here for the first time) | `tracked_callback_timing`, `:2938`; `callback_executions`, `:2811` | `!options?.lazy && recompute(self, true)`: `<r3sig>/dist/dev.js:3749` → `<r9sig>/dist/dev-shared.js:5005`. The `setupComputedNode` diff is a `DEV$1`→`DEV` rename only [M]. Probes J, Y, AH, AI and X give `ran=true` on both versions, dev and prod [M]. `createSignal`, `createMemo`, `createOptimistic` and `createStore` top slices are byte-identical [M]. | U |
| `createEffect`/`createRenderEffect` compute during call, apply `Deferred` | `:2811`, `:2938` | `effect()` still does `recompute(node, true)` (`<r9sig>/dist/dev.js:1612`). The added `node._pendingValue === NOT_PENDING` gate only skips a first apply staged into a live transaction [M]. Probe K gives `compute,returned,apply` on both versions [M]. | U (render effect: see § 9) |
| `createTrackedEffect` runs after the call | `:2938` | First run moved from `queue.enqueue(EFFECT_USER, run)` (`<r3sig>/dist/dev.js:5467`) to `enqueueSub(node); schedule()` (`<r9sig>/dist/dev.js:1845`). `enqueueSub` is a heap insert and `schedule` queues a microtask (`dev-shared.js:1297-1305, 2508-2512`) [M]. Probe M gives `returned,run` on both versions [M]. | I |
| `untrack` clears tracking, not `context` | `callback_preserves_owner_write_context`, `:2493`; `runs_callback_deferred`, `:2454` | `untrack` slice byte-identical (`<r9sig>/dist/dev-shared.js:5183-5197`, sha256-16 `ee8333a5d1128c34`) [M]. Probes AD (no re-run) and N (write still throws inside) agree on both versions [M]. | U |
| `runWithOwner` swaps owner and sets `tracking = false` | `:2454`, `reports_untracked_reads_at`, `:2982` | `<r9sig>/dist/dev-shared.js:6083-6111`. The only change is disposed-owner diagnostic reporting [M]. Probe AE: no re-run on either version [M]. | U |
| `createRoot` = `createOwner` + `runWithOwner` (Creates, Inline, untracked) | `callback_owners`, `:2584` | Signals `createRoot` byte-identical (`dev-shared.js:2964-2967`) [M]. **`solid-js`'s `createRoot` is now its own export**, `(_createRoot \|\| createRoot$1)(...args)` (`<r9js>/dist/solid.dev.js:818`), and during hydration it is `hydratedCreateRoot` (`:684-689`), which only marks a snapshot scope before calling the core `createRoot` [M]. rc.3 re-exported signals' directly (`<r3js>/types/index.d.ts`) [M]. | X (moved), U (semantics) |
| `resolve(fn)` wraps `fn` in `createRoot` and throws in a reactive scope | `:2584`, SC2004 | Only `_extraConfig: CONFIG_DIRECT_COMMIT` was added [M]. Probe AA: thunk owned on both versions. Probe Q: throws in a memo on both [M]. | U |
| `flush(fn)` runs `fn` inline | `:2811` | Probe AG gives `ran=true ret=3` on both versions [M]. rc.9 adds a dev throw for `flush()` inside an action body (`dev-shared.js:2210-2217`) [M]. | U; new throw is N4 |
| `onSettled` under a children-capable owner becomes a tracked-effect leaf; otherwise out-of-band | `leaf_owner_requires_owned_call_site`, `:2505`; `callback_owners` Leaf | `createTrackedEffect(() => untrack(cb))` → `trackedEffect(() => untrack(cb))`, the same node (`<r9sig>/dist/dev.js:2894-2924`). The unowned arm re-enqueues while the heap is dirty [M]. Probe W: `onCleanup` inside throws `CLEANUP_IN_FORBIDDEN_SCOPE` on both versions. Probe P: unowned cleanup throws `SETTLED_CLEANUP_UNOWNED` on both [M]. | U |
| `createReaction`'s invalidation callback has no owner | `callback_owners` None | Probe U: `owner=null` on both versions [M]. The slice diff is option spreading only [M]. | U |
| `mapArray`/`repeat` rows are owned | `callback_owners` Creates | Probe V: `owner,owner` on both versions [M]. `repeat` is byte-identical. `mapArray` only forwards `options.name` [M]. `<For>` now creates its `mapArray` lazily, on first read, under `runWithOwner(owner)` of the `For` call (`<r9js>/dist/solid.dev.js:1204`) [M]. | I |
| `latest`/`isPending` do not clear tracking | `:2454` | `latest` byte-identical. The `isPending` diff suspends latest-mode only for its internal companion reads [M]. | U |
| Component bodies run in `untrack(() => Comp(props), label)` (strict-read window) | `props_require_caller_proof`, `:2534` | `devComponent` → `observedComponent`, with the same `createRoot(…, { transparent: true })` + `untrack(() => Comp(props), label)` (`<r9js>/dist/solid.dev.js:35-58, 53`; rc.3 `dev.js:35-53, 49`) [M]. | U |
| Store setter draft write-enables the original proxy only | `store_setter_callback_enables_proxy_writes`, `:2523` | Probe T commits `store.a=7`. Probe T2 drops the other store's write, `other.b=1`. Both versions agree, in dev and prod [M]. | U |
| Store writes outside a setter are dropped | SC2003 premise | Probe H: the direct `store.a = 99` is dropped on both versions, dev and prod [M]. | U (the *typing* changed: B1) |
| `merge` keeps prop reactivity | `merges_props_reactivity`, `:3139` | `merge` now always returns a view proxy when `Proxy` is available. rc.3 copied plain-object sources eagerly (`<r9sig>/dist/dev.js:4277-4338`) [M]. Proxy/function sources, the reactive case, were a view on rc.3 too [M]. Rule effect [E]: none. | I |
| `Loading` → async boundary, `Errored` → error boundary | `boundary_kind`, `:2557` | Slices for `Loading`, `Errored`, `Reveal`, `Show`, `Switch`, `Match` and `Repeat` are byte-identical in `solid.dev.js` [M]. | U |
| `cleanup_rule`, `accepts_cleanup_return`, `creates_directive_owner`, `returns_store`, `returns_reactive_tuple`, `creates_reactive_source` | `:3057-3190` | Return shapes are unchanged in the declarations (§ 3.2) [M]. `onCleanup`/`cleanup` bodies are unchanged except dev reporting (`cleanup` byte-identical; `onCleanup` wraps `reportDiagnostic`) [M]. | U |

### 3.4 Guards and diagnostics the rules mirror

Every runtime diagnostic the catalog cites is still emitted by rc.9 [M]:

- `REACTIVE_WRITE_IN_OWNED_SCOPE`: setter at `dev-shared.js:5946-5964`,
  refresh at `:6144`, store setter at `:5877-5892`.
- `ACTION_CALLED_IN_OWNED_SCOPE`: `dev.js:1982`.
- `MISSING_EFFECT_FN`: `dev.js:2293`.
- `Cannot call resolve inside a reactive scope`: `dev.js:2486`.
- `SETTLED_CLEANUP_UNOWNED`: `dev.js:2914`.
- `CLEANUP_IN_FORBIDDEN_SCOPE`: `dev.js:2190`.
- `STRICT_READ_UNTRACKED`: `dev-shared.js:631`.
- `SYNC_NODE_RECEIVED_ASYNC`: `dev-shared.js:3397`.
- `NO_OWNER_BOUNDARY`: `dev.js:9191`.
- `ASYNC_OUTSIDE_LOADING_BOUNDARY`: `dev.js:1677`.
- `NO_OWNER_EFFECT` and `NO_OWNER_CLEANUP`.
- The default-transport rich-argument throw: rc.3
  `<r3web>/server-functions/dist/client.js:423`, rc.9 `:570`.

Probe results on both versions [M]:

- **Identical throw or allow outcomes** for: the setter in a memo or a
  component, the store setter in a memo, a write inside a tracked effect
  (allowed), `untrack` inside a memo (still guarded), an action inside a memo
  (throws) and inside a tracked effect (allowed), `refresh` inside a memo
  (throws), `resolve` inside a memo (throws), and one-argument `createEffect`
  (throws `MISSING_EFFECT_FN`).
- **In prod** none of the dev guards fire on either version. One-argument
  `createEffect` crashes on `.effect` on both.

What changed in the guards [M]:

- **The store-setter guard no longer exempts roots.** rc.3:
  `if (context && !context._root && …)` (`<r3sig>/dist/dev.js:4111`). rc.9:
  `if (context && …)` (`dev-shared.js:5878`, citing #3500). Probe E: a store
  setter directly in a `createRoot` body is legal on rc.3 and throws on rc.9.
  See N3.
- **`flush()` inside an action body now throws `FLUSH_IN_ACTION` in dev**
  (`dev-shared.js:2210-2217`). Probe R: resolved on rc.3, rejected on rc.9.
  See N4.
- **`until` throws** "Cannot call until inside a reactive scope"
  (`<r9sig>/dist/dev.js:2718-2722`). It is new; see B4.

### 3.5 `@solidjs/web` entry points

| assumption | evidence | class |
| --- | --- | --- |
| Ref application runs with no owner: `ref` = `untrack(fn)` then `runWithOwner(null, applyRef)` (SC6001) | `ref` and `applyRef` slices byte-identical (`<r9web>/dist/web.dev.js:850-856`) [M] | U |
| `render`/`hydrate` wrap `code` in a created root, Inline | `render` adds only `getOwner()[ROOT_ERROR_HOOK] = options.onError` inside the same `createRoot` [M] | U |
| `clientOnly`, `useHead`, `httpHeader`, `httpStatus` | Byte-identical in the web dev bundle and in `dist/server.js` [M] | U |
| `dynamic(() => Comp)` tracks its source and renders under a component owner | The default path is unchanged: a lazy `createMemo` over `source`. The new `if (options?.static) return staticDynamic(untrack(source))` is at `web.dev.js:2199`, `web.js:2034` and `server.js:3729` [M] | **R: B2** |
| Compiler helper runtime (`insert`, `memo`, `template`, `delegateEvents`) | Byte-identical [M]. `effect` wraps `effectFn` to record the binding node, same `createRenderEffect` with `sync: true`, `transparent: !scope` (`web.dev.js:65-87`) [M]. `spread` gains a multi-source overload and `addEvent` returns the listener [M] | U / I |

### 3.6 Server bodies (`solid-js` `dist/server.js`)

`serverEffect`, `createEffect`, `createRenderEffect`, `createMemo`,
`createRoot`, `createTrackedEffect`, `onSettled` and `untrack` are
byte-identical [M]. Three bodies changed [M]:

- `createSignal` drops a server-write warning.
- `runWithOwner` gains `catch { stampThrower }`.
- `createComponent` gains an ignored `_name`.

Rule effect [E]: none, because the rules cite the server entry only for
"`serverEffect` ignores the apply argument" (SC7001), which is unchanged.

## 4. Items that need a dialect change

### 4.1 B1: store root writes are no longer TypeScript's

**The change.** rc.9 declares `Store<T> = T`
(`<r9sig>/dist/types/store/store.d.ts:4`). Writing a root property is legal
TypeScript on rc.9, but the runtime still drops it outside a setter. Probe H
drops the write on both versions, in dev and prod [M].

**What the checker does.** The measured probe was
`store.a = 2; store.nested.b = 3;` outside any setter:

- rc.3: `tsc` reports TS2540 on the root write. The checker reports SC2003 on
  the nested write only (line 18), which is correct.
- rc.9: `tsc` is silent. The checker still reports only the nested write
  (line 18), so the root write is reported by nobody.

The suppression that causes this is `shared_reactivity.rs:616-640`, gated on
`store_root_properties_are_readonly()`.

**Why the fix needs the seam.** On rc.3, `true` is correct and required: the
absolute rule forbids duplicating TS2540. On rc.9, `false` is correct. The
dialect has no prerelease input today, so this needs one.
`Detection::Installed` already carries `version` [M]. This is the first place
where two prereleases of one major disagree on a vocabulary answer.

### 4.2 B2: `dynamic(source, { static: true })`

**The change.** rc.9 `dynamic` calls `source` exactly once, untracked, before
returning (`staticDynamic(untrack(source))`, `web.dev.js:2199`). No memo and
no per-instance computation is created, and the resolved component renders in
`untrack` (`:2248-2262`) [M].

**What the dialect claims.** The dialect answers for `Dynamic` without looking
at options: `callback_executions` `(0, Tracked)`, `callback_owners`
`(0, Creates)`, and `contract_callback_execution_at` `Tracked`
(`solid_2.rs:2811, 2584, 2904`) [M]. Three consequences:

- **Missed read.** A reactive read in a static source is a one-shot read the
  checker treats as tracked. Measured: `dynamic(() => props.kind === "a" ? …,
  { static: true })` in a component produces no finding on rc.9.
- **False contract claim.** A package contract emitted over such a call would
  publish `tracked` for a callback the runtime runs untracked [E].
- **Suggested fix.** Refuse or specially model the `static` key. The
  options-object shape is readable from the call's second argument. When
  `static` is not a proven literal, the answer should be uncertain rather
  than `Tracked` [E].

### 4.3 B3: the `omit` predicate

**The change.** rc.9 `omit(props, hidden)` treats a single function argument
as a key predicate (`<r9sig>/dist/dev.js:4380`) and invokes it per key: in
`omitTraps` under `Proxy`, and eagerly per property name otherwise
(`:4410-4420`) [M].

**What the engine does.** It treats every `omit` argument as a value: "a
props split only creates property views. Its source and key lists are values
even when erased JavaScript types leave their callability unknown", then
`continue` (`interproc.rs:2246-2254`) [M]. A callable second argument is
therefore dropped from callback analysis, including contract-generation
obligations for a package parameter forwarded into it [E]. Measured: an rc.9
probe with `omit(props, (key) => key === "a")` and a forwarding
`omit(props, hidden)` reports nothing and certifies.

**Suggested fix.** Answer `splits_props` only for non-callable key
arguments, and model the predicate. It runs inline, during property access
of the result, untracked by `omit` itself [E: the trap body was not read in
full]. Until then, the predicate form should be an unknown callback.

### 4.4 B4: `until(fn, options)`

**The change.** rc.9 adds `until<T>(fn: () => T, options?): Promise<Truthy<T>>`
(`<r9sig>/dist/types/signals.d.ts:608`). Its runtime [M]
(`<r9sig>/dist/dev.js:2717-2785`):

- throws when an observer is active;
- otherwise creates a root with a private microtask queue;
- runs `fn` as the tracked compute of a user effect during the call (`effect`
  → `recompute(node, true)`); probe "until fn runs during call" gives
  `ran=true`;
- resolves on the first truthy value, and uses timers and an abort listener.

**What the checker does.** It has no `TABLE` entry. Measured: an rc.9 probe
with `await until(() => ready()); setReady(false)` and the `omit` predicate
yields `{"status":"certified","findings":[]}`. That is the silent-substitution
failure AGENTS.md describes. A callback-taking export of the modelled package
that the builtin model does not describe is treated as having no obligation.

**Suggested fix.** Model `until` like `resolve` (Deferred for the caller,
Creates owner, throws in a tracked scope, which is an SC2004 analogue), or
make an unmodelled callback-taking export of a primitive-defining package
uncertifiable. The second fix also covers the next prerelease.

## 5. Changes that need no dialect change but should be recorded

- **N1. SC7001's headline case becomes TypeScript's on rc.9 [M].** rc.9
  removed the `createEffect(compute): never` overload (rc.3
  `signals.d.ts:378`). One-argument `createEffect` is now TS2554. The rule
  already stays silent: `effect_api.rs:176-181` classifies only calls whose
  resolved validity is `Valid`. Measured: SC7001 fires on rc.3 (line 7) and
  not on rc.9, where `tsc` reports TS2554 at the same line. No duplicate, no
  change needed. The rule page's first sentence ("Flags the published,
  deprecated one-argument overload") is rc.3-specific, and the cast-hidden
  cases remain the checker's on both versions.
- **N2. Broken declarations in `solid-js@2.0.0-rc.9` [M].**
  `<r9js>/types/index.d.ts:8` re-exports `sharedConfig`,
  `createErrorBoundary`, `createLoadingBoundary` and `createRevealOrder` from
  `./client/hydration.js`, which no longer declares them. `:3` re-exports
  `$DEVCOMP` from `./client/core.js`, which no longer declares it either. All
  five are still runtime exports.
  - Under `skipLibCheck: false` this is five TS2305 errors inside the
    package.
  - Under `skipLibCheck: true` the imports are untyped. A probe
    `createErrorBoundary((err) => …)` gets TS7006 for `err`.
  - The checker's reaction is fail-closed but it loses findings. Measured: an
    SC2001 write inside `createErrorBoundary(() => { setCount(1); … })` in a
    component body is reported on rc.3 and **not** on rc.9. The call no
    longer resolves to the primitive.
  - This is an upstream typing defect. Report it to solidjs/solid and do not
    work around it in the dialect.
- **N3. Store setters in a `createRoot` body now throw [M].** rc.9 removed the
  root exemption from `devGuardStoreSetterWrite` (§ 3.4, probe E). The
  checker reports neither a signal setter (which throws on *both* versions,
  probe D) nor a store setter directly in a module-level `createRoot` body.
  Measured on both versions: probe4 cases D and E produce no finding. So no
  finding moves today. A future SC2001 root-body arm must be version-aware
  for store setters, because they are legal on rc.3.
- **N4. `flush()` inside an action body is a dev throw on rc.9 [M]**
  (`FLUSH_IN_ACTION`, `dev-shared.js:2210-2217`). This is a new runtime defect
  class with no rule. Nothing the checker claims is wrong. It is a rule
  opportunity, not a compatibility item.
- **N5. `refresh` returns a promise; `createOptimisticStore` gains plain-form
  options; `render`/`hydrate` gain `onError` [M].** No rule reads these
  results or positions [E].
- **N6. `For` builds its `mapArray` lazily and `merge` is always a view [M]**
  (§ 3.3). The ownership and tracking the rules use are unchanged [M for the
  owner probe, E for rule effect].

## 6. Compiler

- **What the checker compiles with [M].** `solidjs-compiler` from
  `yumemi-thomas/solid` at distribution `9f9a84b2`, implementation
  `7f4e1135` and upstream `a10cf1a1` (`rust/Cargo.toml:37`,
  `rust/dialects/solid-v2/compiler/src/lib.rs:29-31`,
  `docs/package-contract-v2/phase4/compiler-identity.json`). Semantic trace
  version is 3. The monorepo at `9f9a84b` is version `2.0.0-rc.3`: its
  `packages/compiler/package.json` is `@solidjs/compiler@2.0.0-rc.3`.
- **What rc.9 "expects" [M].** `solid-js@2.0.0-rc.9` declares no compiler
  dependency. The rc.9 consumer compiles with
  `@dom-expressions/babel-plugin-jsx@0.50.0-next.44`, through
  `babel-preset-solid@2.0.0-rc.2`, and `@solidjs/vite-plugin@3.0.0-next.31`
  pulls `@dom-expressions/compiler@0.50.0-next.44`. The rc.3 kobalte consumer
  compiles with **the same** `@dom-expressions/babel-plugin-jsx@0.50.0-next.44`
  through the same preset. So rc.9 does not move the compiler a project
  actually runs, at least for these two trees. Whether `dom-expressions
  next.44` lowers like the pinned fork is a pre-existing parity question,
  not an rc.9 one [E].
- **Runtime interface [M].** The pinned fork imports 40 helpers by name
  (`import_named`). All of them are exported by `@solidjs/web@2.0.0-rc.9`
  wherever rc.3 exported them, except `rowProof`. The fork emits `rowProof`
  only when the opt-in `patch_driver` is set; the default resolves to
  disabled (`packages/compiler/src/compiler.rs:380-382` at `9f9a84b`), and
  the adapter never sets it
  (`compile_options`). The helpers whose runtime semantics the execution
  facts model (`insert`, `memo`, `template`, `ref`/`applyRef`,
  `delegateEvents`) are byte-identical, and `effect` keeps its
  `createRenderEffect` shape (§ 3.5).
- **Upstream drift [E].** The unpinned rc.7-era fork checkouts `16f0988` and
  `9d2ccb9` differ from `9f9a84b` in 26 files under `packages/compiler/src`
  (≈2,100 changed lines, a new `tsrx/` module, `shared/patch.rs` removed). Of
  the helpers they import, only `rowProof` is gone [M for the counts]. A
  compiler rebase is a separate review. rc.9 compatibility does not depend on
  it, given the helper interface above.

## 7. Negative rows: what rc.9 could get cheaply

Rows stay archive-scoped. No rc.9 archive is in `AUDITED_ARCHIVES`
(`solid_2.rs:131-156`), so no row answers for rc.9 and certification of an
rc.9 tree loses closures rather than gaining wrong ones [M].

Cheap candidates are the rc.6 `@solidjs/signals` rows
(`2026-09-25-solid-2-rc6-signals-negative-rows.md`) whose top-level dev slice
is byte-identical from rc.6 to rc.9, or differs only in dev console
reporting [M, slice comparison of `signals-rc6/dist/dev.js` against
`<r9sig>/dist/dev{,-shared}.js`]:

| rc.6 row | rc.9 dev slice vs rc.6 |
| --- | --- |
| `getOwner` `creates` | `getOwner` byte-identical (`return context`) |
| `onCleanup` `creates` | `cleanup` byte-identical. `onCleanup` swaps `console.warn` for `reportDiagnostic`, which is console output plus the caller-installed footer hook (`dev-shared.js:600-609`) |
| `untrack` `creates` | byte-identical |
| `runWithOwner` `creates` | differs only in the disposed-owner `reportDiagnostic` |
| `createRoot` `creates` | byte-identical. Its `createOwner` adds a `_name: undefined` literal slot and a `DEV` rename |

Each still needs a reading of rc.9's `dist/prod/**` split files and of the
new `dist/observe/**` tier, which the rc.6 audit never had to cover. That
cost is estimated as small because the slices are tiny [E].

Not cheap: `createSignal`, `createMemo`, `createTrackedEffect`, `onSettled`,
`flush`, `createStore`, `createProjection`, `createOptimistic*`,
`reconcile`, `snapshot` and `action`. Their top slices are sometimes
identical, but their closures changed: `computed`, `recompute` (4188-4775,
rewritten), `trackedEffect`, `flush`, and the store rewrite [M].

The `solid-js@2.0.0-rc.9` rows are not cheap either. `solid-js`'s
`createRoot` is its own export in rc.9 (§ 3.3). The rc.3 rows that paired
signals bodies with `solid-js@rc.3` server bodies need `solid-js@rc.9`'s:
`runWithOwner` changed there (§ 3.6), and `getOwner` and `untrack` were not
re-read in this review.

## 8. `2.0.0-experimental.1` (corvu)

**What it is [M].** corvu's workspace pins `solid-js@2.0.0-experimental.1`
(lock integrity `sha512-IV7xHaJ3…l9Rg==`, `package.json` sha256
`9be1e2dc…90b4`), which depends on `@solidjs/signals@^0.1.0` (installed
0.1.0) and runs with `@solidjs/web@2.0.0-experimental.0`. It compiles with
`babel-preset-solid@1.9.3` and `babel-plugin-jsx-dom-expressions@0.39.6`,
the 1.x-line compiler. This is the pre-beta 2.0 experiment, not an older rc:

- **1.x-era names.** Its root exports include `createAsync`, `catchError`,
  `flushSync`, `onMount`, `isStale`, `unwrap`, `Suspense` and
  `ErrorBoundary`.
- **Missing 2.0 names.** It has no `Loading`, `Errored`, `action`,
  `createOptimistic`, `onSettled` or `createTrackedEffect`.
- **Different arities.**
  - `createEffect(compute, effect, error?, value?, options?)`: argument 2 is
    an error handler, where the dialect's `options_argument` says 2 is
    options.
  - `createMemo(compute, value?, options?)`: argument 1 is a seed value,
    where the dialect says 1 is options.
  - `createSignal(fn, initialValue?, options?)`.

  (`@solidjs/signals@0.1.0` `dist/types/signals.d.ts:43-45, 61-62, 97-98,
  116-117`.)

**Recommendation [E].** Treat it as unsupported and refuse it with SC9013's
reasoning ("the checker cannot model the runtime it installs"). Do not treat
it as pre-rc. Analysing it under the rc vocabulary misreads argument
positions the rules use, and its boundaries (`Suspense`/`ErrorBoundary`) are
invisible to `boundary_kind`. Doing this needs a prerelease-aware
classification: any 2.0 prerelease below the floor the vocabulary was read
on is `Unsupported`. The floor is the owner's choice; `rc.0` is the lowest
the vocabulary cites. It is the same seam extension B1 needs.

## 9. Not done, and incidental observations

- **Not done.** No rc.9 RFC text was read (§ 1). `@solidjs/web` server
  functions beyond the rich-argument throw were not compared. The `observe`
  bundles were not read, only enumerated. The `omit` proxy traps were not
  read in full (B3). No tarball integrity was re-derived. No contract or
  census was run on rc.9.
- **Incidental, version-independent [M].** Probe L shows `createRenderEffect`
  runs its apply function *during* the creating call on both rc.3 and rc.9,
  dev and prod (`compute,apply,returned`). `effect()` calls
  `runEffect(node, LANE_RUN)` synchronously for non-user, non-`schedule`
  effects (`<r9sig>/dist/dev.js:1612-1620`). The dialect answers `(1, Deferred)` for
  `createRenderEffect` (`solid_2.rs:2811`). This is not an rc.9 difference.
  It is recorded for the owner because it contradicts a current row on rc.3's
  own bytes.
- **Incidental, version-independent [M].** A signal setter directly in a
  module-level `createRoot` body throws on both versions (probe D) and the
  checker is silent on both (probe4 case D).
