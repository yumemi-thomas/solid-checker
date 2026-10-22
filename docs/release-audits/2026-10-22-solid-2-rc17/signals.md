# C. `@solidjs/signals` premises, rc.13 → rc.17

Bytes: `/tmp/sc-rc17-audit/inst13` and `inst17` (registry installs; integrities in the integrity section). **[R]** means read on both releases' bytes. **[M]** means measured by probe: `scripts/forkC/probe*.mjs` and `until.mjs`, one Node process per probe, loading `dist/dev.js` and `dist/prod/index.js` by absolute path. The checker runs used the repo's debug binary on scratch projects (`scripts/forkC/proj13`, rc.13 install). Function-level equality comes from `scripts/forkC/slices.py`, which compares comment- and whitespace-stripped top-level declarations. "SAME" means byte-equal after that normalisation.

Summary: **3 changes need checker work** (C1-C3). **4 behaviour changes need no change but should be recorded** (C4-C7). Every other premise holds; only its line numbers moved.

## C1. CHANGED: the `createOptimistic` setter no longer meets the owned-scope write guard [M]

rc.13 `createOptimistic` returned `setSignal.bind(null, node)` (the guarded setter). rc.17 `dist/dev.js:2423-2432`:

```js
function createOptimistic(first, second) {
  if (typeof first === "function") {
    const node = computed(first, second);
    node._config &= ~CONFIG_AUTO_DISPOSE;
    return [accessor(node), optimisticWrite.bind(null, node)];
  }
  const node = signal(first, second);
  registerGraph(node, getOwner());
  return [accessor(node), optimisticWrite.bind(null, node)];
}
```

`optimisticWrite` (`dist/dev.js:294-306`) only queues a guess (`pendingGuesses.push(n, v, question || nextQuestion()); schedule();`). It has no `REACTIVE_WRITE_IN_OWNED_SCOPE` guard.

Probe (`opt.mjs`): a `createOptimistic` setter called directly in a `createRoot` body, and in a memo compute:

| | rc.13 dev | rc.17 dev |
| --- | --- | --- |
| root body | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` | no throw |
| memo compute | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` | no throw |

The checker today (debug binary, rc.13 install) reports **SC2001 violation** for `const [v, setV] = createOptimistic(0); setV(1);` in a component body, and for `setO(2)` in a memo compute. On rc.17 both would be **false positives**.

Required:
- A release-gated answer from the signals owner: "the `createOptimistic` setter is guarded", true on read releases ≤ rc.13 and false on rc.17. It needs a new vocabulary digit, or an extension of N5 (which today covers only the optimistic *store*).
- `solid_2.rs:614-619` ("A signal, optimistic-signal or action call there throws on every release") must become release-scoped.
- Update the SC2001 rule page.

The `createOptimisticStore` setter is still guarded on rc.17: `fn => storeSetter(store, fn)` (`dist/dev.js:7399`) reaches `devGuardStoreSetterWrite` (`:5849`). Measured: it throws in a memo on both releases. N5 holds.

## C2. CHANGED: `latest(fn)` now exempts a pending accessor read from the strict pending throw [M]

rc.13 `read()` (`dist/dev-shared.js:6386`) applied the throw `if (strictRead && !pendingCheckActive && owner._statusFlags & STATUS_PENDING)`. It exempted only `isPending`.

rc.17 `read()` hands every verdict-window read (both `latest` and `isPending`) to `verdictValue` *before* that check: `if (verdict !== null) return verdict(el, c);` (`dist/dev-shared.js:5258-5281`). `verdictValue` (`dist/dev.js:1349-1358`) says:

```js
  // Dev strict-read scopes (a component body, an effect callback) warn on a
  // verdict read as on any untracked read; the pending throw they add for a
  // plain read does not apply — a verdict read is a sanctioned read of a
  // pending node (an uninitialized one still throws NotReady below).
  if (strictRead)
    warnStrictReadUntracked(strictRead, {
```

Probe: a pending accessor read inside `untrack(fn, "a component body")`:

| read | rc.13 dev | rc.17 dev |
| --- | --- | --- |
| plain, initialized and refetching | throws `PENDING_ASYNC_UNTRACKED_READ` | throws `PENDING_ASYNC_UNTRACKED_READ` |
| `isPending(() => m())` | no throw; `STRICT_READ_UNTRACKED` | same |
| `latest(() => m())`, initialized and refetching | **throws `PENDING_ASYNC_UNTRACKED_READ`** | **no throw**; `STRICT_READ_UNTRACKED` |
| `latest(() => m())`, uninitialized | throws `PENDING_ASYNC_UNTRACKED_READ` | throws a plain `NotReadyError`, plus `STRICT_READ_UNTRACKED` |
| `isPending(() => m())`, uninitialized | no throw, no warning | no throw, **plus `STRICT_READ_UNTRACKED`** |

The checker today reports **SC5001 violation** ("Solid throws PENDING_ASYNC_UNTRACKED_READ in dev") for `latest(() => user())` in a component body. That is correct on rc.13 and **wrong on rc.17**.

Required:
- `callback_handles_pending_accessor_read(Latest, 0)` becomes release-gated (true on signals rc.17).
- Update the SC5001 page ("Native `isPending` …" must name `latest` on rc.17).
- `isPending` keeps its exemption: the `solid_2.rs:265-270` premise holds in effect, but its mechanism changed (the `pendingCheckActive` exclusion became `verdictValue`). Update the comment.

## C3. NEW dev throw `PRIMITIVE_IN_EFFECT_CALLBACK` [M]

rc.17 `dist/dev-shared.js:3947-3957`:

```js
function assertNotInEffectCallback() {
  if (inEffectCallback && context === null) {
    emitDiagnostic({ code: "PRIMITIVE_IN_EFFECT_CALLBACK", kind: "lifecycle", severity: "error", message: PRIMITIVE_IN_EFFECT_CALLBACK_MESSAGE });
    throw new Error(PRIMITIVE_IN_EFFECT_CALLBACK_MESSAGE);
  }
}
```

It is called from `createOwner` (`:1496`) and `setupComputedNode` (`:4927`), so every root, memo and effect is covered. rc.13 has no such code (grep: 0 hits).

Probe (`probe3.mjs`): each primitive created directly in a `createEffect` apply:

| created there | rc.13 dev | rc.17 dev |
| --- | --- | --- |
| `createRoot` | runs | **throws** |
| `createMemo` | runs | **throws** |
| `createEffect` | `NO_OWNER_EFFECT` warning only | **throws** |
| `createSignal` | runs | runs |
| `onCleanup` | `NO_OWNER_CLEANUP` | `NO_OWNER_CLEANUP` |
| `runWithOwner(captured, () => createMemo(…))` | runs | runs |
| after an `await` in the apply | runs | runs |

The same throw happens in a `createRenderEffect` apply on a *later* run, but not on the first, synchronous run. Prod builds never throw.

Consequences:
- `missing-owner.md:44-49` ("A root created inside such a callback … answers for what it contains") is false on rc.17 dev: that root throws before its body runs. An effect in the apply is still a defect, but it now throws instead of warning `NO_OWNER_EFFECT`.
- SC4001's message wording for that case, and the `primitive-in-directive-application.md:37-41` premise ("nothing throws"), need re-measuring. The second one is for the web fork: does `ref` apply run with `inEffectCallback` up? Unverified here.
- A new rule opportunity (a computation created ownerless in an effect apply). Until one exists, record it in open-work as a known miss for rc.17.

## C4. CHANGED, no claim affected: owned-scope write guards stand down during disposal [M]

rc.17 adds `!disposalDepth` to all three write guards:

| guard | rc.17 location |
| --- | --- |
| `setSignal` | `dist/dev-shared.js:5621-5626` |
| `markRefresh` (refresh) | `:5801-5806` |
| `devGuardStoreSetterWrite` | `dist/dev.js:5812` (`if (context && !(context._config & CONFIG_CHILDREN_FORBIDDEN) && !disposalDepth)`) |

`setSignal` also drops rc.13's `el._firewall !== context` clause: `_firewall` no longer exists in rc.17 (one comment hit, `:5272`). `disposalDepth` is raised by `runDisposal` and by effect-cleanup teardown (`:1290`, `:1314`), citing #4165.

Probe (`probe2.mjs`): a cleanup whose child root is disposed synchronously inside a root body or a memo compute:

| write in that cleanup | rc.13 dev | rc.17 dev |
| --- | --- | --- |
| signal setter | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` | legal |
| store setter | throws | legal |
| `refresh` | throws | legal |

A cleanup run by a memo re-run is legal on both releases.

The checker claims no write in a cleanup: SC2001's page has no cleanup case, and `fixtures/engine/eslint-reactivity-v2/cleanup-thunk-write.tsx` snapshots only SC4001 uncertifiable. So no false positive is introduced. The runtime only became more permissive.

The store-setter `guard = false` path (`storeSetter(wrappedStore, st => reconcileState(…), false)`, `dist/dev.js:6689`) is the projection's internal commit inside `createProjectionInternal`. The user setters (`:5938`, `:6795`, `:7399`) keep the default `guard = true`. **N3 holds** [M]: a store setter in a `createRoot` body throws on both releases.

## C5. CHANGED, no checker claim: the derived store's pending throw on a store Get is gone [R]

rc.13's store Get trap escalated a strict-window read of a *refetching derived store* (`dist/dev.js:6662-6668`, `if ((target.fam?.node?._statusFlags ?? 0) & STATUS_PENDING) throwPendingUntrackedRead(…)`). rc.17 removes it and only warns (`dist/dev.js:5536-5548`).

The SC5001 page says store reads do not inherit the accessor rules. I found no store-pending SC5001 claim, but did not exhaustively verify that.

The `then`/string-key gate (ADR 0293, `store_strict_keys_audited`) **holds** [M]:
- rc.17 `dist/dev.js:5536-5542`: `strictRead && !inDraft(target) && typeof key === "string" && key !== "then" && getObserver() === null`.
- Probe: `s.a` warns `STRICT_READ_UNTRACKED`; `s.then` and a symbol key are silent, on both releases.

## C6. `createStore(fn)` / `createProjection(fn)` with no seed [M]

| | rc.13 | rc.17 |
| --- | --- | --- |
| runtime | crashes (`Cannot read properties of undefined (reading 'Symbol(STORE_OWNER)')`, dev and prod) | works: `seed ||= {}` in `createProjectionInternal`, `dist/dev.js:6744-6747` |
| typings | `seed: Partial<T> \| NoFn<T>` required | `fn: (() => T \| Promise<T> \| AsyncIterable<T>) & NoArray<T>, seed?: null`; non-null seeds now `NoFn<T>` (`store/index.d.ts:33-35`, `solid-js/types/client/hydration.d.ts:401`) |

The dispatch is still `typeof first === "function"` (`dist/dev.js:8438-8440`, `if (typeof first === "function") return createStoreDerived(first, second, third);`). So `options_argument` = 2 (options at argument 2 of the derived form) and `WhenFirstArgumentIsFunction` hold.

The zero-parameter compute is still called with the draft (`fn(shadow ?? s)`, `:6677`), so a compute that ignores it is fine. The `callback_chain.rs:642` "no arity premise" reasoning still holds.

`createEffect<const T>` / `createRenderEffect<const T>` is a typing-inference change only; no runtime change (`createEffect` and `createRenderEffect` are SAME).

## C7. New exports (callback-taking, unmodelled)

| export | where | what it does | gap |
| --- | --- | --- | --- |
| `createDeferred(fn, options?)` | `dist/dev.js:8852`; root of signals and solid-js | wraps `computed(prev => …)`, so `fn` is an eager tracked compute like `createMemo` | neither modelled nor excluded, like rc.4's `registerPatch` gap: add a KNOWN_GAP or model it |
| `setWriteHook(hook)` | `dev-shared.js:5596-5598` | runs `hook` on every later `setSignal` / store setter | internal (`solid-js/internal`); classify as excluded |
| `setReadWatch(watch)` | `dev.js:1782-1784`, sets `DEV.hooks.onRead` | runs `watch` on every `read` | internal (`solid-js/internal`); classify as excluded |
| `markRaw(value)` | `dev.js:2241` | no callback | none |
| `setSignalIdentities(bool)` | | no callback | none |
| `signalName` | | no callback | none |
| `storeName` | | no callback | none |

## Premises that hold (with rc.17 citations)

| premise (checker site) | rc.13 citation verified | rc.17 citation | verdict |
| --- | --- | --- | --- |
| B1 `Store<T> = T` | `types/store/store.d.ts:4` | `types/store/types.d.ts:7` | holds [R] |
| B3 omit predicate `keys.length === 1 && typeof keys[0] === "function"` | dev.js:4550; prod/store/utils.js | dev.js:8381; `prod/store/utils.js:1010`; `observe/store/utils.js:1010` | holds; `omit` SAME [M: predicate called on reads, 5 calls on both] |
| `SUPPORTS_PROXY = typeof Proxy === "function"` | dev-shared.js:317 | dev-shared.js:298 | holds (SAME) |
| `omitTraps` `set: trueFn, deleteProperty: trueFn`; OmitView; isHidden | dev.js:4344-4404, 3611, 3633 | dev.js:8175-8235 (set/delete :8208-8209), 7442, 7464 | holds (SAME) [M] |
| `merge` returns its one non-falsy source; function sources → `createMemo(s)` | dev.js:4443-4548 | dev.js:8274-8379 (byte-identical span; `createMemo(s)` :8292) | holds [M] |
| `viewSource(e)[key]` (discover_sources.rs:872) | prod/store/utils.js | prod/store/utils.js:102, 164, 826 | holds (`viewSource` SAME) |
| N3 store setter guard, no root exemption | dev-shared.js:6650-6666 | dev.js:5811-5827 (+`!disposalDepth`, C4) | holds for roots [M] |
| N4 `FLUSH_IN_ACTION` | `actionStepDepth > 0`, dev-shared.js:2656; enterActionStep/exitActionStep dev.js:2142-2154 | `if (actionDepth > 0) { throw new Error("[FLUSH_IN_ACTION] …` dev-shared.js:2647-2656; `enterAction()` / `exitAction()` around `it.next(v)` / `it.throw(v)` dev.js:1579-1600 | holds, renamed [M: rejects in dev, resolves in prod]; prod `if (actionDepth > 0) { return e ? e() : undefined; }` prod/core/scheduler.js:1010-1012 |
| N6 `FLUSH_IN_EFFECT_CALLBACK` in the running-flush branch | dev-shared.js:2680-2698 | dev-shared.js:2670-2693 (report :2678-2690) | holds [M] |
| `setEffectCallback(true/false)` around the apply in runEffect | dev.js:1876, 1906 | dev.js:1994, 2024 (finally) | holds; also raises a `"an effect callback"` strict window (:1993) |
| user effects enqueue; render effect first apply synchronous unless defer/schedule | dev.js:1737-1740 | dev.js:1866-1870 (`node._type === EFFECT_USER \|\| options?.schedule ? globalQueue.enqueue(…) : runEffect(node, node._type)`) | holds [M: `c,ret,a` vs `c,a,ret`] |
| `flush(fn)` inline (syncDepth), owner unchanged | dev-shared.js:2665-2677 | dev-shared.js:2657-2669 | holds [M] |
| `createRoot` = `runWithOwner(createOwner(), () => init(dispose))` | dev-shared.js:3478-3483 | dev-shared.js:1537-1543 (`init(disposeRootSelf.bind(owner, true))`) | holds (dispose is now a bound function) |
| `runWithOwner` swaps context, `tracking = false`, finally restore; `RUN_WITH_DISPOSED_OWNER` | dev-shared.js:6919-6947 | dev-shared.js:5758-5786 | holds (SAME) [M] |
| strictRead warning in `read()` | dev-shared.js:6511-6516 | dev-shared.js:5441-5445 | holds |
| `untrack(fn, label)` sets `strictRead = label \|\| false`, finally restore | dev-shared.js:5863-5900 | dev-shared.js:5099-5117 (fast path dropped; `externalUntrackHook` added) | holds |
| `recompute` clears strictRead | dev-shared.js:4890-4896 | dev-shared.js:4196-4200 | holds |
| body_roles.rs:763 window restored in finally | dev-shared.js:5801-5885 | dev-shared.js:5059-5117 | holds |
| `createRevealOrder`: createOwner + runWithOwner | dev.js:10170-10179 | dev.js:9902-9925 | holds (context plumbing changed) |
| `onSettled`: tracked effect under a children-capable owner, else queued; `SETTLED_CLEANUP_UNOWNED` | dev.js:3031-3076 | dev.js:3058-3088 | holds (SAME) [M] |
| `onCleanup` registers or returns `fn` | dev.js:2311-2340; dev-shared.js `cleanup` | dev.js:2377-2406; dev-shared.js:1415-1437; prod/signals.js:65 | holds [M: `NO_OWNER_CLEANUP`] |
| createMemo eager unless `lazy` | setupComputedNode `!options?.lazy && recompute` | dev-shared.js:4932 (same line) | holds [M] |
| `until`: aborted signal rejects before `createRoot`; predicate runs during the call under a fresh root, re-runs | dev.js:2854-2922 (effect) | dev.js:2909-3057 (`watch()` = `computed` under `createRoot`, dev.js:2692-2734) | holds behaviourally [M]; implementation changed, update the comment (effect → computed) |
| `resolve`: createRoot + compute | dev.js:2620-2704 | dev.js:2756-2803 (`watch`) | holds [M] |
| `until` / `resolve` dev throw under an observer; prod none | dev.js:2718-2722 | dev.js:2910-2914, 2757-2761; prod/signals.js has no guard (512, 390) | holds [M]. `getObserver` is now `return tracking ? context : null;` (dev-shared.js:1385-1387); rc.13 also returned `PENDING_OWNER` inside isPending/latest |
| `mapArray` / `repeat` `setStrictRead(options.name)` | dev.js:8889, 9332 | dev.js:3095, 3626 | holds [M] |
| getter `read.bind(null, node)`, `function read(el)`, length 0 | dev.js:2341; dev-shared.js:6333; prod/signals.js:68; prod/core/core.js:1494 | dev.js:2407-2411 (callers 2416, 2421, 2427, 2431, 2434); dev-shared.js:5258; prod/signals.js:69-73; prod/core/core.js:1127 | holds [M: 0,0,0,0] |
| `checkPostAwaitRead` / `attributedFlight` | dev-shared.js:968, 1061 | dev-shared.js:1007-1078, 1101-1137 | holds (SAME) [M: async memo warns; untrack / tracked-before / plain async / action / effect-apply IIFE / tracked-effect IIFE silent on both] |
| owner `null` after `await` | — | — | holds [M] |
| ACTION_CALLED_IN_OWNED_SCOPE | — | dev.js:1545-1556 | holds [M] |
| createContext / getContext: `NoOwnerError` first, then `ContextNotFoundError` | dev.js:270-303 | dev.js:1794-1833 | holds (messages gain detail; explicit-`undefined` default flagged `_undefinedDefault`) |
| `isPending` pending-read exemption (`solid_2.rs:265-270`) | dev-shared.js:6386 `!pendingCheckActive` | `verdictValue` dev.js:1349-1358 | holds in effect, mechanism changed (C2) |

A side effect of the C2 restructure, read but not probed: rc.17's `read()` calls `checkPostAwaitRead` before the verdict dispatch, without rc.13's `!pendingCheckActive` clause. So a read inside `isPending` after an `await` may now warn `UNTRACKED_READ_AFTER_AWAIT`. This is unverified, and it is not an SC1002 false-positive risk.

## Diagnostic codes

The `code: "…"` sets of signals' dev builds: 45 on rc.13, 47 on rc.17. **Added:** `PRIMITIVE_IN_EFFECT_CALLBACK` (C3) and `STORE_WRITE_OUTSIDE_SETTER`. The second is a new dev warning on a store write outside its setter (`dist/dev.js:5440`, measured); it is the runtime counterpart of SC2003, so cite it on `no-direct-mutation.md`. **Removed:** none.

Every all-caps code cited in `docs/rules/*.md`, `packages/cli/lib/rules-solid-v2.json` and `rust/dialects/solid-v2/rules/src/rules.rs` is still present in rc.17's bytes, across signals, solid-js and web. The only non-hits are not diagnostic codes (`CARGO_MANIFEST_DIR`, `DOCS_BASE_URL`, `SOLID_RULES_UPDATE`, and the prefix `NO_OWNER_`).

## `packages/cli/scripts/feedback-native-hook.mjs` (`readerProfiles`)

The rc.13 row was re-derived: sha256 `c1901652…d074`, observer [156682,156812], reader [301718,311816], untrack [277947,278538]. It matches the repo.

The rc.17 row (TypeScript `getStart` / `end` on `dist/dev-shared.js`):

```
"2.0.0-rc.17": { sha256: "sha256:93e01d4ef6ba1ec02daeeb6c378ff8f7baa2ca6ccfc1eff8a2f8f3c767ed0572",
  observer: [60078, 60140], reader: [238087, 248114], untrack: [230810, 231214] }
```

The instrumentation contract still fits mechanically:

| function | rc.17 shape | fit |
| --- | --- | --- |
| `read` | still `function read(el)`, one parameter named `el` (the hook passes `el` to `begin`); 10 own return statements, all rewritten | fits |
| `untrack` | `(fn, strictReadLabel)`; its whole body is wrapped in try/finally | fits |
| `getObserver` | one parameterless return | fits |

The semantics changed, though. rc.13's `getObserver` returned `PENDING_OWNER` inside `isPending`/`latest`; rc.17's never does, and verdict-window reads leave `read()` early through `return verdict(el, c)`. Any hook consumer that inferred an isPending/latest window from `observerQuery` returning `PENDING_OWNER` would lose it on rc.17. I did not read the consumer side, so this is unverified. Also, rc.17's `untrack` gains `if (externalUntrackHook) return externalUntrackHook(fn);`, which still sits inside the wrapped body.

## Not verified

- Component bodies under `solid-js` (C1 and C2 were measured with `createRoot` and `untrack(fn, "a component body")` on signals directly). This is the solid-js fork's scope.
- Observe builds were not probed.
- Whether the web package's `ref` apply runs inside `inEffectCallback` (C3, directive rule).
- Exhaustive confirmation that no SC5001 store-pending claim exists (C5).
