// Per-row reasoning for the rc.13 negative-rows audit. Keyed `package|export|domain`.
// Each entry: { verdict: 'granted'|'withheld', reason: one line, body: markdown }.
const SIG = '@solidjs/signals', WEB = '@solidjs/web', JS = 'solid-js';
const signalsCreatesBound = 'The closure reaches only archive code, builtins, the caller\'s callables and installed hooks; its host references are the ones § 0.3 lists, none of which registers a version-1 resource into a browser document or a server runtime.';
export const T = {
[`${SIG}|action|Reads`]: { verdict: 'granted', reason: 'Wrapper frames are rc.9\'s plus module counters; no read entry point reachable in any build with rc.9\'s cuts; drains are the registrants\'.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group D § 1 (call event returns an arrow; every run of the wrapper, its resumptions and the flushes it drives).

**rc.13 bytes [M].** prod and observe differ from rc.9 only in mangled property names on the transition record (prod \`s.he\` → \`s.Te\`; observe \`s.dt\`/\`s.Tt\` → \`s.tt\`/\`s.et\`); the byte ranges are unchanged (4429..7642, 4502..8183). dev adds \`enterCallback()\` before the generator step and \`exitCallback()\` on both exits: a module counter (\`callbackDepth\`) that the new dev \`UNTRACKED_READ_AFTER_AWAIT\` check consults (\`dev-shared.js\` \`enterCallback\`/\`exitCallback\`, two statements each).

**Closure [M].** With rc.9's cuts (boundary-queue methods \`run\`, \`_evaluate\`/\`B\`, the \`wireExternalSource\` hook) and the spurious \`write\` edge (§ 0.4), the call graph from \`action\` reaches no read entry point: {{reads:action}}. Hook slots followed in dev reach none (§ 0.4). \`flush\` and \`GlobalQueue.flush\` are § 15's.

**Pairing [M].** \`solid-js@2.0.0-rc.13\` server \`action\` is byte-identical to rc.9 (\`return fn\`) in all three server builds (§ 0.5).

**Verdict: GRANTED** (prod, dev, observe). Denies that one invocation of \`action\`, at its call event or on any later run of the wrapper it returns, observes a reactive source \`action\` authored.` },
[`${SIG}|action|Creates`]: { verdict: 'granted', reason: 'Host reaches are the returned Promise, queueMicrotask, console, and dev/observe-only WeakRef/FinalizationRegistry bookkeeping; none registers a version-1 resource.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group D § 2.

**rc.13 bytes [M].** As § 1.

**Closure [M].** Host references reached per build: {{host:action}}. ${signalsCreatesBound} The dev \`Promise\`/\`resolve\` entries include the spurious executor-parameter edge rc.9 already named; \`WeakRef\`/\`watchAsyncTail\` and \`registerRoot\` are § 0.3's new dev/observe bookkeeping (owner flag O1).

**Pairing [M].** Server \`action\` identical (\`return fn\`).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createMemo|Reads`]: { verdict: 'granted', reason: 'Export slices byte-identical; changed frames (computed, setupComputedNode, recompute, handleAsync) add no read entry point; external-source hook line unchanged.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 1.

**rc.13 bytes [M].** All three slices are byte-identical to rc.9's (prod/observe \`f0f1d1a8…\`, dev \`46130d23…\`).

**What changed in the closure [M].** \`computed\` adds a \`_plumbing\` config bit and leaves plumbing nodes unnamed; \`setupComputedNode\` replaces its inline child link with \`linkChild(context, self)\` (four field writes) and keeps \`!options?.lazy && recompute(self, true)\`; \`recompute\` gains calls to \`findLane\`, \`clearDeps\`, \`underFreshLoadingBoundary\` (a queue-chain flag walk), \`el._queue.notify\` (the boundary queues' \`notify\`, which write \`_disabled\`/\`_error\` with \`setSignal\` and report errors) and \`NotReadyError\`; \`handleAsync\` attaches its continuation through dev's \`watchAsyncTail\` wrapper (§ 0.3). \`accessor\`, \`signal\`, \`wireExternalSource\` and \`externalUntrack\` are byte-identical.

**Closure [M].** With rc.9's cuts (\`flush\`, the \`wireExternalSource\` hook) and the spurious \`write\` edge: {{reads:createMemo}}. Dev hook slots reach none.

**The guarded read on the stack.** Unchanged: the \`wireExternalSource\` wrapper (\`dist/dev.js\`, byte-identical to rc.9) reads the bridge signal the hook created; dispositioned **hook**, as on rc.3, rc.6 and rc.9.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createMemo|Creates`]: { verdict: 'granted', reason: 'Signals archive has no handle on a document or server runtime (§ 0.3); closure host reaches are microtasks, console and dev bookkeeping.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 2.

**rc.13 bytes [M].** Slices byte-identical (§ 3).

**Closure [M].** {{host:createMemo}}. ${signalsCreatesBound} \`createMemo\` is \`solid-js\`' own declaration in rc.13 (\`types/index.d.ts:8\`), so no server pairing applies.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createOptimistic|Reads`]: { verdict: 'granted', reason: 'dev/observe slices identical, prod differs by one mangled field; engine install and node constructors identical; neither the call nor the setter closure reaches a read entry point.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 3 (rests on the accessor line, rc.6 owner flag 2).

**rc.13 bytes [M].** dev (\`9180aeea…\`) and observe (\`760ee92d…\`) are byte-identical to rc.9. prod differs in one mangled field name (\`n.T &= ~CONFIG_AUTO_DISPOSE\` → \`n.C &= …\`). \`installOptimisticEngine\`, \`optimisticComputed\`, \`optimisticSignal\` and \`accessor\` are byte-identical in dev.

**Closure [M].** Call: {{reads:createOptimistic}}. Returned setter (\`setSignal\`): {{reads:createOptimistic setter (setSignal)}}. \`setSignal\`'s only code change is \`captureWriteSnapshot\` while a hydration snapshot capture is active (a write-side record).

**Verdict: GRANTED** (prod, dev, observe). Not denied, as on rc.9: a later, separate call of the returned accessor.` },
[`${SIG}|createOptimistic|Creates`]: { verdict: 'granted', reason: 'Node allocation and engine slots; host reaches are microtasks and console.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 4.

**Closure [M].** {{host:createOptimistic}}. ${signalsCreatesBound}

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createOptimisticStore|Creates`]: { verdict: 'granted', reason: 'Only code change in the export is dev/observe nameStore (an attribution-only map write); closure stays inside the archive.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 6. (\`reads\` stays withheld; it is not an rc.9 row and was not re-read.)

**rc.13 bytes [M].** None of the three slices is byte-identical to rc.9's. prod differs in short (mangled) identifiers only. In dev and observe the only code line added (comments excluded) is \`nameStore(store, options?.name)\`, which writes \`storeNames.set(proxy[$TARGET], name)\` only when an attribution engine is installed (a module \`WeakMap\`; the \`[$TARGET]\` is a brand lookup).

**Closure [M].** {{host:createOptimisticStoreNext}}. ${signalsCreatesBound} New in rc.13: \`scheduleWithheld\`'s \`queueMicrotask(flush)\` (the archive's own drain).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createProjection|Creates`]: { verdict: 'granted', reason: 'Export slices byte-identical; changed projection internals stay inside the archive.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 2.

**rc.13 bytes [M].** All three slices (\`return createProjectionNextInternal(e, t, r).store;\`) are byte-identical to rc.9's. \`createProjectionNextInternal\` and \`runProjectionComputedNext\` changed (draft write withholding, \`scheduleWithheld\`).

**Closure [M].** {{host:createProjectionNext}}. ${signalsCreatesBound}

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createRoot|Creates`]: { verdict: 'granted', reason: 'dev/observe slices identical, prod locals renamed; createOwner adds a dev/observe-only weak root registry (module FinalizationRegistry), judged not a create (owner flag O1).', body: `
**rc.9 basis.** RC9_SIGNALS_AUDIT § 3.

**rc.13 bytes [M].** dev (\`9a66667d…\`) and observe (\`d266035c…\`) are byte-identical to rc.9. prod differs only in local names (\`t\`/\`n\` swapped).

**What changed in the closure [M].** \`createOwner\` now links a child through \`linkChild\` and, for an owner with **no parent**, calls \`registerRoot(owner)\` (dev \`dev-shared.js:775-783\`, observe \`observe/core/dev.js:265-273\`; prod folds it away, \`prod/core/owner.js\` has only \`if (n) linkChild(n, i)\`). \`registerRoot\` does \`new WeakRef(owner)\`, a module \`WeakMap\`/\`Set\` insert, and \`rootReaper?.register(owner, ref, ref)\` on a module-level \`FinalizationRegistry\` whose callback is \`ref => liveRoots.delete(ref)\`. Disposal calls \`unregisterRoot\` and runs disposal lists last-registered first (runtime.md item 7); \`assertInvariant\` only emits a dev diagnostic.

**Judgement [E], owner flag O1.** The registry is the archive's own module object; the host's only action is to call the archive's own deleter back after collection. Nothing is registered into a browser document or a server runtime, and no runtime outside the archive acts on the owner: semantic-model § creates' "a package's own private module variable is neither". So it is not a \`create\`, by the same reasoning rc.9 applied to the archive's \`globalThis[Symbol.for(…)]\` slots.

**Closure [M].** {{host:createRoot}}.

**Pairing.** \`createRoot\` is \`solid-js\`' own declaration in rc.13 too, so this row does not answer a \`solid-js\` import.

**Verdict: GRANTED** (prod, dev, observe), resting on O1.` },
[`${SIG}|createSignal|Reads`]: { verdict: 'granted', reason: 'dev/observe slices identical, prod one mangled field; plain path signal/accessor identical; function path is createMemo\'s clean closure; probes 16/16 per build.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 5, argument-scoped (\`callable_or_primitive_slots: [0]\`; scope unchanged).

**rc.13 bytes [M].** dev (\`d1292c1a…\`) and observe (\`0366fccd…\`) byte-identical; prod differs in one mangled field (\`n.T\` → \`n.C\`). \`signal\` and \`accessor\` byte-identical in dev.

**Closure [M].** {{reads:createSignal}}; setter \`setMemo\`: {{reads:createSignal setter (setMemo)}} (\`setMemo\` adds \`heldDerivation\`/\`rederiveHeld\`: a flag test and a heap insert).

**Probes [M]** (§ 0.6): \`createSignal(0)\`, \`createSignal(0, { equals: false })\`, \`createSignal(undefined)\` and the function form leave the probe memo untracked; \`createSignal(createdMemoAccessor)\` by reference computes the created memo (the case the scope declines). 16/16 in prod, dev and observe, as on rc.9.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe). It is the delegate of § 45.` },
[`${SIG}|createSignal|Creates`]: { verdict: 'granted', reason: 'Same node constructors; host reaches are microtasks and console.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group A § 1.

**Closure [M].** {{host:createSignal}}. ${signalsCreatesBound}

**Verdict: GRANTED** (prod, dev, observe). With it and § 18, the scoped \`solid-js\` rows' delegates are answered beside rc.13 signals.` },
[`${SIG}|createStore|Creates`]: { verdict: 'granted', reason: 'prod slice identical; dev/observe add nameStore (attribution map write); closure stays inside the archive.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 1.

**rc.13 bytes [M].** prod is byte-identical (\`5ccb9272…\`). dev and observe wrap the plain overload as \`const store = createStoreNext(…); if (second?.name) nameStore(store[0], second.name); return store;\` (\`nameStore\`: \`attrHooks !== null && name\` → \`storeNames.set(proxy[$TARGET], name)\`).

**Closure [M].** {{host:createStore}}. ${signalsCreatesBound}

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createTrackedEffect|Reads`]: { verdict: 'granted', reason: 'Export slices, trackedEffect and enqueueSub byte-identical; no read entry point reachable.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 3.

**rc.13 bytes [M].** All three slices byte-identical (\`eb5e0531…\`, \`59effb18…\`). \`trackedEffect\` and \`enqueueSub\` byte-identical in dev; \`schedule\` adds the projection-draft withholding (\`withheld = projectionWriteActive\`), a flag.

**Closure [M].** {{reads:createTrackedEffect}}.

**Pairing [M].** Server body identical (\`getOwner()\` + \`getNextChildId\`); \`nextChildIdFor\` now prefixes with \`materializeId(counter)\` for a hole-scoped owner (an id string write).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|createTrackedEffect|Creates`]: { verdict: 'granted', reason: 'Same closure; host reaches are microtasks and console; server body registers nothing.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 4.

**Closure [M].** {{host:createTrackedEffect}}. ${signalsCreatesBound}

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|flush|Reads`]: { verdict: 'granted', reason: 'flush adds only an attribution hook; GlobalQueue.flush\'s new boundary re-arm/judge calls are boundary-queue methods (drain); no read entry point with rc.9\'s cuts.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 7.

**rc.13 bytes [M].** prod differs only in a mangled slot name (\`globalQueue.Kt\` → \`.Jn\`). observe and dev add \`if (attrHooks !== null && (scheduled || activeTransition)) attrHooks.flushStart();\` (**hook**, the attribution engine). The \`FLUSH_IN_ACTION\` guard and the \`fn\` branch are unchanged.

**GlobalQueue.flush, rc.13 additions [M].** \`GlobalQueue._endOptimism\` hoisted out of the \`activeTransition\` arm; \`drainRearms()\` calls \`_rearm()\` on boundaries an \`on\` option re-armed (\`CollectionQueue\` methods of boundaries \`createLoadingBoundary\` registered); \`checkBoundaryChildren(this, true)\` calls each child boundary's \`_judgeHeld\` → \`_checkSources\` (\`setSignal(this._disabled, false)\`, the reveal controller's \`_evaluate\`); \`commitPendingNodes()\` before the zombie heap; a \`_batch._pendingNodes\` test. All are boundary-queue registrants' methods (**drain**, as rc.9's \`CollectionQueue._readOn\`) or bookkeeping.

**Closure [M].** With rc.9's cuts and the spurious \`write\` edge: {{reads:flush}}.

**Pairing [M].** Server \`flush\` is \`function flush() {}\`, identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|flush|Creates`]: { verdict: 'granted', reason: 'Drain stays inside the archive; host reaches are microtasks, console and haltReactivity\'s reportError.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 8.

**Closure [M].** {{host:flush}}. ${signalsCreatesBound} \`haltReactivity\`'s \`globalThis.reportError\` is rc.9's ruling (it reports an error and registers nothing).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|getOwner|Reads`]: { verdict: 'granted', reason: 'All three slices byte-identical (return context); no call.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 1. **Carried on byte identity [M]:** prod/observe \`67fcbebd…\`, dev \`e8cb95b7…\`; the closure is the function alone (no helper). Server \`getOwner\` identical. Probe: \`getOwner()\` leaves the probe memo untracked in all three builds (§ 0.6).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|getOwner|Creates`]: { verdict: 'granted', reason: 'All three slices byte-identical; no helper.', body: `
**rc.9 basis.** RC9_SIGNALS_AUDIT § 1. **Carried on byte identity [M]** of all three slices and of the server body; the function calls nothing.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|omit|Creates`]: { verdict: 'granted', reason: 'Closure is store/utils view code, built-ins and caller values only; the rc.13 changes (one-symbol record lookup, re-homed accessors on the copy path) call nothing new.', body: `
**rc.9 basis.** RC9_MERGE_OMIT_MEMO_AUDIT § 1.

**rc.13 bytes [M].** All three slices changed. Two changes: the proxy classification reads one brand, \`recordOf(props)\` = \`props[$RECORD]\`, instead of \`[$TARGET]\`/\`[$OMIT]\`/\`[$VIEW]\`; and the non-\`Proxy\` copy path re-homes an accessor descriptor with \`get: desc.get && desc.get.bind(props)\` instead of copying it (the getter is bound, not invoked).

**Walk [M]** (§ 0.4, per-file top-level walk): rc.13 reaches 42 definitions in each minified \`store/utils.js\` and 48 in \`dist/dev.js\` (rc.9: 45 and 50 by the same tool). New: \`recordOf\`, \`$RECORD\`; gone: \`$OMIT\`, \`$VIEW\`, \`$SOURCES\`, \`isView\`; changed: \`omitTraps\`, \`mergeLookup\`, \`sourceDescriptor\` (brand tests and a plain-source fast path). Imports used: \`SUPPORTS_PROXY\`, \`$PROXY\`, \`$RECORD\`, \`$TARGET\`, \`ownEnumerableKeys\`. Free globals: \`Map\`, \`Object\`, \`Proxy\`, \`Reflect\`, \`Set\`, \`Symbol\`, \`undefined\` (the rc.9 set). No scheduler, owner, memo, signal or host function is reachable, traps included.

**Pairing [M].** All six \`solid-js@2.0.0-rc.13\` builds re-export \`omit\` from \`@solidjs/signals\` (\`dist/solid.js:2\`, \`dist/server.js:2\`, siblings).

**Verdict: GRANTED** (prod, dev, observe, and every \`solid-js\` host).` },
[`${SIG}|onCleanup|Reads`]: { verdict: 'granted', reason: 'All three slices byte-identical; dev closure is the same 8 functions, two of them diagnostic code with no read.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 2.

**rc.13 bytes [M].** All three slices byte-identical (\`89ddda30…\`, dev \`5f8d40b1…\`). The dev closure is the same eight functions as rc.9's; \`emitDiagnostic\` (listeners now receive the live subject as a second argument) and \`isExcluded\` (a \`markedOwners\` map instead of \`excludedOwners\`) changed, \`cleanup\`, \`getOwner\`, \`reportDiagnostic\`, \`takeFooter\`, \`ownerPath\` are identical. {{reads:onCleanup}}. Server body identical. Probe: untracked in all builds.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|onCleanup|Creates`]: { verdict: 'granted', reason: 'Slices identical; dev diagnostics only.', body: `
**rc.9 basis.** RC9_SIGNALS_AUDIT § 2. Slices byte-identical [M]; dev closure as § 20. {{host:onCleanup}}. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|onSettled|Reads`]: { verdict: 'granted', reason: 'dev slice identical, prod/observe mangled fields only; no read entry point reachable.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 5.

**rc.13 bytes [M].** dev byte-identical (\`add35758…\`). prod and observe differ only in mangled field names (\`t.T\` → \`t.C\`; \`dirtyQueue.et\`/\`.Ct\` → \`.ln\`/\`.st\`).

**Closure [M].** {{reads:onSettled}}. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|onSettled|Creates`]: { verdict: 'granted', reason: 'Both arms reduce to trackedEffect or Queue.enqueue + schedule.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group B § 6. {{host:onSettled}}. ${signalsCreatesBound} Server body identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|reconcile|Reads`]: { verdict: 'withheld', reason: 'rc.13 applyAdopt composes the optimistic view through readerOverride -> nodeValue -> serve, an engine read-path call the rc.9 grant said did not exist in the closure; reachable unless every application runs under authoritativeServe(), which was not established.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 7. That grant rested on two findings: (a) "the call graph from \`reconcileNextState\` reaches no read-path call" in any build, and (b) every proxy it can touch is parameter-rooted. Group C § 0 draws the line: "A read is either an engine read-path call (\`read\`/\`readNodeFast\`), or a … trap on a store … the export created".

**rc.13 bytes [M].** The three \`reconcile\` slices are byte-identical (\`d99b5e91…\`, dev \`572cd064…\`), but the application closure changed. \`applyAdopt\` (prod \`store/next/reconcile.js:102\`, dev \`dist/dev.js\` \`applyAdopt\`) still computes \`optHooks.optimisticView(t, prev)\` for an optimistic family (\`fam?.opt === true\`), with \`draft\` defaulted to \`false\`. rc.13's \`optimisticView\` now sets \`const reader = !draft && !authoritativeServe()\` and, for each node with a visible override, takes \`readerOverride(node, …)\` instead of the raw \`_overrideValue\`. \`readerOverride\` returns \`nodeValue(node, committed)\` when the node carries \`CONFIG_OVERRIDE_SUPERSEDED\`, and \`nodeValue\` calls \`serve(node, readerContext(), …)\`: the selection core of \`read\` (it may \`enterStagedRead\`, \`throw new NotReadyError\`, and record into an \`isPending\` probe). rc.9's census already counts \`nodeValue → serve\` as a read path (parity audit § 0.4).

**Closure [M].** {{reads:reconcile}}. The path is \`reconcileNextState → applyAdopt → optimisticView → readerOverride → nodeValue → serve\` in all three builds, with no spurious edge on it (\`optHooks.optimisticView\` is \`optimisticView\`: prod \`store/next/optimistic.js:71\`, dev \`dist/dev.js:8084\`).

**Why withheld.** The reach is guarded (optimistic store, visible superseded override, \`!authoritativeServe()\`, \`!authoritativeRead()\`), and a guarded reach still counts. The dev comment on \`optimisticView\` says applyAdopt's key-matching view "keeps the override itself", which would hold only if every application runs under \`authoritativeServe()\` (\`projectionWriteActive || getWriteOverride() || authoritativeRead()\`). This reading did not establish that for a caller's optimistic-store setter applying \`reconcile(...)\` [E]. Separately, the node read belongs to the store the caller handed in. rc.9's sign-off excluded "reads of the store handed to that function", but the model's receiver carve-out (Decision 2026-09-10) covers a *property access* on a caller-supplied receiver, not the archive's own read path observing a node of that receiver.

**What would grant it.** Either (1) a reading that proves every \`applyAdopt\` call on an optimistic family runs with \`authoritativeServe()\` true, so \`readerOverride\` is unreachable, or (2) an owner ruling that an engine read of a node inside a caller-supplied store is the caller's read. Owner flag O2.

**Verdict: WITHHELD** on rc.13. No citation is carried.` },
[`${SIG}|reconcile|Creates`]: { verdict: 'granted', reason: 'Application performs adoption writes, setSignal and schedule; the new serve path can only enter a transaction (schedule -> queueMicrotask).', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group C § 8.

**rc.13 bytes [M].** Slices byte-identical. The § 24 \`serve\` path can \`enterStagedRead\` → \`globalQueue.initTransition\` → \`schedule()\`: the archive's own flush microtask, the same class rc.9 D3 recorded for \`snapshot\`.

**Closure [M].** {{host:reconcile}}. ${signalsCreatesBound}

**Pairing [M].** Server \`reconcile\` identical (\`setProperty\` on the caller's \`state\`).

**Verdict: GRANTED** (prod, dev, observe, and the \`solid-js\` server path).` },
[`${SIG}|runWithOwner|Reads`]: { verdict: 'granted', reason: 'dev/observe slices identical, prod locals renamed; the only invocation is fn; probes as rc.9.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 3, argument-scoped (\`invoked_slots: [1]\`; scope unchanged).

**rc.13 bytes [M].** dev (\`332a218c…\`) and observe (\`5c40363e…\`) byte-identical; prod differs only in local names. Dev closure: the same six functions (\`emitDiagnostic\`, \`isExcluded\` changed as § 20). {{reads:runWithOwner}}. Server body identical.

**Probes [M].** \`runWithOwner(owner, () => s())\` and \`runWithOwner(null, () => 1)\` leave the memo untracked; \`runWithOwner(owner, createdMemoAccessor)\` by reference computes the created memo once (the case the scope declines). All three builds.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe).` },
[`${SIG}|runWithOwner|Creates`]: { verdict: 'granted', reason: 'Saves/restores context and tracking around fn; dev diagnostic only.', body: `
**rc.9 basis.** RC9_SIGNALS_AUDIT § 5. Slices as § 26 [M]. {{host:runWithOwner}}. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|snapshot|Creates`]: { verdict: 'granted', reason: 'Slices identical; snapshotWalk\'s reach is still the archive\'s own flush microtask.', body: `
**rc.9 basis.** RC9_PARITY_AUDIT group D § 3.

**rc.13 bytes [M].** All three slices byte-identical (\`3bb31686…\`, dev \`eebc4206…\`). \`snapshotWalk\` and \`pendingBackingVisible\` changed. Through \`optimisticView\`'s new reader path (§ 24) a snapshot of an optimistic store can now reach \`serve\` → \`enterStagedRead\` → \`initTransition\` → \`schedule()\`, which is the rc.9 D3 class: the archive's own flush microtask.

**Closure [M].** {{host:snapshot}}. ${signalsCreatesBound}

**Pairing [M].** All three \`solid-js\` server builds re-export \`snapshot\` from \`@solidjs/signals\` (\`dist/server.js:2\`).

**Verdict: GRANTED** (prod, dev, observe).` },
[`${SIG}|untrack|Reads`]: { verdict: 'granted', reason: 'prod/observe differ only in the hook slot\'s mangled name; dev adds a counter and a fast-path condition; the only invocation is fn or the external-source hook.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 4, argument-scoped (\`invoked_slots: [0]\`; scope unchanged).

**rc.13 bytes [M].** prod and observe differ only in the mangled hook-slot name (\`GlobalQueue.Yt\`/\`.In\` → \`.jn\`/\`.Jt\`) and local names. dev adds \`asyncTailFlights === 0\` to the fast-path test and \`untrackDepth++\`/\`--\` around \`fn\` (a module counter the dev \`UNTRACKED_READ_AFTER_AWAIT\` check reads). The hook slot \`GlobalQueue._externalUntrack\` is still installed only by \`enableExternalSource\` (\`dist/dev.js:236\`; \`externalUntrack\` byte-identical). {{reads:untrack}}. Server body identical (\`return fn();\`).

**Probes [M].** \`untrack(() => s())\` and \`untrack(() => 1)\` untracked; \`untrack(createdMemoAccessor)\` by reference and the literal \`untrack(() => createdMemoAccessor())\` compute the created memo, as on rc.9. All three builds.

**Verdict: GRANTED, argument-scoped** (prod, dev, observe).` },
[`${SIG}|untrack|Creates`]: { verdict: 'granted', reason: 'Flag toggles and counters around fn; no helper call.', body: `
**rc.9 basis.** RC9_SIGNALS_AUDIT § 4. Slices as § 29 [M]; the closure is the function alone in every build. Server body identical.

**Verdict: GRANTED** (prod, dev, observe).` },
[`${WEB}|clientOnly|Reads`]: { verdict: 'granted', reason: 'All six slices identical; call-event closure (solid-js createSignal plain path, loadClientOnly) identical; returned component is F3.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 15 (returned-value line, flag F3).

**rc.13 bytes [M].** All six slices byte-identical (client \`f6412c9a…\`, server \`31fd1a14…\`). Client walk: \`clientOnly\` and \`loadClientOnly\`, both identical. At the call, \`createSignal()\` (no argument) is \`solid-js@2.0.0-rc.13\`'s wrapper (identical) → \`hydratedCreateSignal\` (identical; a non-function argument goes straight to signals) → signals \`createSignal\` plain overload (§ 10). Server body \`return props => {…}\` invokes nothing.

**Verdict: GRANTED**, under F3.` },
[`${WEB}|clientOnly|Creates`]: { verdict: 'granted', reason: 'Nothing registers at the call in any build; server preload registration is inside the returned component (F3).', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 16. Same bytes and closure as § 31 [M]; the signals plain path is bounded by § 0.3.

**Verdict: GRANTED**, under F3.` },
[`${WEB}|httpHeader|Reads`]: { verdict: 'granted', reason: 'Slices identical; getRequestEvent now also looks the request event up through a realm-global WeakMap of render roots; no reactive source.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 17.

**rc.13 bytes [M].** All six slices byte-identical (client \`cd98272f…\`, server \`3d0445e2…\`). Server walk: \`headerLedgers\`, \`RequestContext\` identical; \`getRequestEvent\` changed: after \`globalThis[RequestContext].getStore()\` it now falls back to \`renderContextOf(getOwner())\`, a lookup of \`globalThis[Symbol.for("@solidjs/web/render-roots")]\` (a \`WeakMap\` a renderer populates) along the owner chain, then \`ctx.event\`. \`getOwner\` and \`onCleanup\` are \`solid-js\` server bodies, identical. No reactive source is observed.

**Verdict: GRANTED.**` },
[`${WEB}|httpHeader|Creates`]: { verdict: 'granted', reason: 'Writes to an existing response plus a retracting cleanup; owner flag carried from rc.9.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 18 and its owner flag ([Decision 2026-09-04]'s headline sentence vs its four terms). Bytes and closure as § 33 [M]; \`renderContextOf\` only reads.

**Verdict: GRANTED**, with the rc.9 owner flag.` },
[`${WEB}|httpStatus|Reads`]: { verdict: 'granted', reason: 'As httpHeader.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 19. All six slices byte-identical (client \`9de7ebfe…\`, server \`56233798…\`) [M]; closure as § 33 (\`statusLedgers\` identical, \`getRequestEvent\` as there).

**Verdict: GRANTED.**` },
[`${WEB}|httpStatus|Creates`]: { verdict: 'granted', reason: 'As httpHeader.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 20. As § 34.

**Verdict: GRANTED**, with the rc.9 owner flag.` },
[`${JS}|For|Reads`]: { verdict: 'granted', reason: 'Six slices identical; signals mapArray identical and updateKeyedMap changes only route its row writes; server mapArray changes an id read.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 1 (flags F1–F3).

**rc.13 bytes [M].** All six slices byte-identical (client lazy-\`list\` body; server \`36dc66a0…\`). Client imports used: \`getOwner\`, \`runWithOwner\`, \`mapArray\` (the rc.9 set). Signals \`mapArray\` is byte-identical in dev; \`updateKeyedMap\` now writes its row and index signals through \`write\`, which is \`setSignal\` or, under a live optimistic lane, \`GlobalQueue._landOnOverride\` (writes; its dev closure reaches no read entry point). Closure from \`mapArray\`: {{reads:mapArray (For, solid-js client)}}. Server \`mapArray\` changes \`rowOwner.id\` to \`ownerId(rowOwner)\` (an id field; dev throws on an unmaterialized hole-scoped owner) and still calls \`createMemo(…, { sync: true })\` → \`createSyncMemo\` (identical).

**Verdict: GRANTED**, under F1–F3.` },
[`${JS}|For|Creates`]: { verdict: 'granted', reason: 'Client stays inside the signals archive; server createSyncMemo never reaches processResult.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 2. Bytes as § 37 [M]. Client callees are signals code bounded by § 0.3. Server: \`createSyncMemo\`, \`createOwner\`, \`runWithOwner\`, \`formatChildId\`, \`stampThrower\` identical; \`ownerId\`/\`materializeId\` are id arithmetic; no \`processResult\` or \`ctx.serialize\` on the path [M].

**Verdict: GRANTED.**` },
[`${JS}|Match|Reads`]: { verdict: 'granted', reason: 'function Match(props) { return props; } identical in all six builds.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 5. **Carried on byte identity [M]** (\`54f4236b…\` in all six); no call, no property access.

**Verdict: GRANTED.**` },
[`${JS}|Match|Creates`]: { verdict: 'granted', reason: 'Same body.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 6. As § 39 [M].

**Verdict: GRANTED.**` },
[`${JS}|Repeat|Reads`]: { verdict: 'granted', reason: 'Six slices identical; signals repeat/updateRepeat identical in dev; server repeat changes an id read.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 3.

**rc.13 bytes [M].** All six slices byte-identical (\`540fa67e…\`, dev/observe \`c9b26a45…\`). Signals \`repeat\` and \`updateRepeat\` byte-identical in dev; closure from \`repeat\`: {{reads:repeat (Repeat, solid-js client)}}. Server \`repeat\`: \`ownerId(rowOwner)\` as § 37.

**Verdict: GRANTED**, under F1.` },
[`${JS}|Repeat|Creates`]: { verdict: 'granted', reason: 'As For.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 4. As § 38 [M].

**Verdict: GRANTED.**` },
[`${JS}|createContext|Creates`]: { verdict: 'granted', reason: 'Six slices identical; the call\'s only callee is Symbol(); provider is defined, not invoked.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 10. **Carried on byte identity [M]**: client \`c3be410a…\`, server \`87cfe46a…\` in all six builds. The transitive call table at the call event has one row, \`Symbol(...)\`. \`provider\` (whose body reaches \`createRoot$1\`, \`setContext\`, \`children\`) runs only when a consumer renders it.

**Verdict: GRANTED.**` },
[`${JS}|createMemo|Creates`]: { verdict: 'granted', reason: 'Wrapper identical; browser closure (40 definitions, identical across the three builds) reaches only the same five signals imports, module state, a restored fetch/Promise swap and microtasks; delegates granted on rc.13 signals.', body: `
**rc.9 basis.** RC9_MERGE_OMIT_MEMO_AUDIT § 4 (§ 13 of the core-and-web audit's dispositions).

**rc.13 bytes [M].** The wrapper \`const createMemo = (...args) => (_createMemo || createMemo$1)(...args);\` is byte-identical in the three browser builds (\`1528dcd8…\`); \`_createMemo\` is still written only by \`enableHydration\` (\`dist/solid.js:910\`). \`hydratedCreateMemo\` dropped its \`options?.transparent\` test, which moved into \`hydrateSignalLike\`'s new first line \`if (options?.transparent || noHydrationId()) return coreFn(fn, options);\`.

**Walk [M].** From \`createMemo\`, \`hydratedCreateMemo\` and \`_createMemo\`: 40 definitions (rc.9: 27), every one byte-identical across \`solid.js\`, \`solid.dev.js\` and \`solid.observe.js\`. Changed: \`hydrateSignalLike\`, \`hydratedCreateMemo\`, \`readSerializedOrCompute\`, \`armLiveTakeover\` (per-owner gates in a \`WeakMap\`/\`Map\`), \`subFetch\` (iterator test), \`markTopLevelSnapshotScope\` (adds \`openLiveScope\`). New: \`noHydrationId\`, \`adoptedAnswerStream\`, \`takeOver\`, \`wrapFirstYield\`, \`liveScopeOf\`, \`openLiveScope\`, \`isClaiming\`, \`_claimOwner\`, \`nodeGate\`, \`liveGates\`, \`openScopes\`, \`TAKEN\`, \`LIVE_LOCAL\`, \`LIVE_RESUME_FROM\`. Gone: \`liveGate\`. Imports used are exactly rc.9's: \`createMemo$1\`, \`createSignal$1\`, \`getOwner\`, \`peekNextChildId\`, \`markSnapshotScope\`. Free globals: \`window\`/\`fetch\`/\`Promise\` (\`subFetch\`, the swap restored in \`finally\`), \`Promise.resolve\` (\`adoptedAnswerStream\`, \`normalizeIterator\`), \`queueMicrotask\` (\`onHydrationEnd\`, and new \`queueMicrotask(flip)\` in the hybrid branch, a write of the gate signal), \`Symbol\`, \`WeakMap\`, \`WeakSet\`, \`Map\`, \`Set\`. \`sharedConfig.has\`/\`.load\` are still \`@solidjs/web\`'s hooks over \`globalThis._$HY.r\` (\`web.js:1371-1372\`). No \`document\`, no \`_$HY\` write, no \`addEventListener\`, no \`setTimeout\` is reachable.

**No create.** Same dispositions as rc.9: delegate calls, host reads, a restored swap, microtasks, module state.

**Scope.** Condition \`browser\`; runtime \`dist/solid.js\`, \`dist/solid.dev.js\`, \`dist/solid.observe.js\` (rc.13's \`exports["."]\` is identical to rc.9's); delegates \`@solidjs/signals\` \`createMemo\`, \`createSignal\`, \`getOwner\` \`creates\`, all granted here on rc.13 (§ 4, § 11, § 18). \`solid-js@2.0.0-rc.13\` depends on \`@solidjs/signals: ^2.0.0-rc.13\`.

**Verdict: GRANTED, scoped to \`browser\`.**` },
[`${JS}|createSignal|Reads`]: { verdict: 'granted', reason: 'Wrapper, hydratedCreateSignal and all three server bodies identical; a primitive first argument reaches only the signals plain path (delegate granted).', body: `
**rc.9 basis.** RC9_READS_AUDIT § 6 (\`primitive_slots: [0]\`, delegate signals \`createSignal\` \`reads\`; scope unchanged).

**rc.13 bytes [M].** Client wrapper slices byte-identical (\`96473897…\`); \`hydratedCreateSignal\` byte-identical (\`if (typeof fn !== "function" || !sharedConfig.hydrating) return createSignal$1(fn, second);\`). Server \`createSignal\` byte-identical in all three (\`38028a6c…\`, dev \`6002fc5a…\`): a non-function first argument returns two closures and calls nothing. The function path (server \`createMemo\`, client \`hydrateSignalLike\`) is excluded by the scope.

**Probes [M].** Client builds: \`createSignal(5)\` leaves the memo untracked with hydration off and on, and reads back \`5\` under \`ssrSource: "client"\` (5/5 primitive-path checks per build). The function-path contrast (\`createSignal(fn, { ssrSource: "client" })\` under \`sharedConfig.hydrating\`) now runs \`fn\` (1, rc.9: 0) because rc.13's \`hydrateSignalLike\` returns \`coreFn(fn, options)\` when the owner has no hydration id (\`noHydrationId()\`), which is the probe's situation. That is the excluded path; it does not bear on the row. Server builds 2/2.

**Delegate.** \`(@solidjs/signals@2.0.0-rc.13, createSignal, Reads)\`, § 10.

**Verdict: GRANTED, argument-scoped.**` },
[`${JS}|createSignal|Creates`]: { verdict: 'granted', reason: 'As createMemo browser: wrapper identical, closure 40 definitions identical across builds, delegates granted on rc.13 signals.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 13.

**rc.13 bytes [M].** Wrapper slices byte-identical (\`96473897…\`); \`hydratedCreateSignal\` and \`withHydrationGate\` byte-identical; \`_createSignal\` still written only by \`enableHydration\` (\`dist/solid.js:911\`).

**Walk [M].** From \`createSignal\`, \`hydratedCreateSignal\`, \`_createSignal\`: 40 definitions (rc.9: 27), identical across the three browser builds; changed/new/gone exactly as § 44 minus \`hydratedCreateMemo\`; imports used \`createSignal$1\`, \`getOwner\`, \`peekNextChildId\`, \`markSnapshotScope\` (the rc.9 set); the same free globals. The hybrid branch now builds its own gate (\`createSignal$1(false, { ownedWrite: true })\`, a delegate call) instead of \`withHydrationGate\`, and may \`queueMicrotask(flip)\` (a reactive write).

**No create.** § 44's dispositions.

**Scope.** Condition \`browser\`; runtime the three browser files; delegates \`@solidjs/signals\` \`createSignal\` and \`getOwner\` \`creates\`, granted on rc.13 (§ 11, § 18).

**Verdict: GRANTED, scoped to \`browser\`.**` },
[`${JS}|useContext|Reads`]: { verdict: 'granted', reason: 'Six slices identical; signals getContext identical in dev and differs by one mangled field in prod/observe; server getContext identical.', body: `
**rc.9 basis.** RC9_READS_AUDIT § 7.

**rc.13 bytes [M].** All six slices byte-identical (client \`03f0fc70…\`, server \`3cbc46d0…\`). Signals \`getContext\`: dev byte-identical; prod and observe differ in one mangled owner field (\`t.ze\`/\`t.wt\` → \`t.Je\`/\`t.rt\`). \`NoOwnerError\`/\`ContextNotFoundError\` identical. Server walk (5 definitions: \`getContext\`, \`serverComponentContextError\`, …) all identical.

**Probes [M].** \`useContext(ctx)\` leaves the memo untracked and returns the default in every client build; the server builds return the default.

**Verdict: GRANTED.**` },
[`${JS}|useContext|Creates`]: { verdict: 'granted', reason: 'Same bytes; a context-map read that may construct and throw an Error.', body: `
**rc.9 basis.** RC9_CORE_WEB_AUDIT § 11. As § 47 [M]; no path touches a host.

**Verdict: GRANTED.**` },
};
