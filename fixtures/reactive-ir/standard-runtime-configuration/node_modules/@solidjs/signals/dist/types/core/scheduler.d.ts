import { type Heap } from "./heap.js";
import { activeLanes, assignOrMergeLane, findLane, type OptimisticLane } from "./lanes.js";
import type { Computed, Signal } from "./types.js";
export { activeLanes, assignOrMergeLane, findLane };
export { getOrCreateLane, hasActiveOverride, mergeLanes, resolveLane } from "./lanes.js";
export declare const transitions: Set<Transition>;
export declare const dirtyQueue: Heap;
export declare const zombieQueue: Heap;
export declare let clock: number;
export declare let activeTransition: Transition | null;
export declare let projectionWriteActive: boolean;
/** > 0 while an action's generator body is on the stack (the synchronous
 * slice between yields). Maintained by action.ts around `it.next()`. */
export declare let actionStepDepth: number;
export declare function enterActionStep(): void;
export declare function exitActionStep(): void;
export declare let _hitUnhandledAsync: boolean;
/** Slot hook's deferral: release this node when its carried state resolves. */
export declare function deferSlotRelease(node: Signal<any>): void;
/**
 * Consume the unhandled-async hit. Returns whether this is the first report
 * of the current enforcement window — the caller warns only then.
 */
export declare function resetUnhandledAsync(): boolean;
/**
 * Toggles the dev-mode "must be inside a `<Loading>` boundary" enforcement
 * window. Only `render()` calls this — wrapping the initial mount so that a
 * top-level uncaught async read surfaces the diagnostic. Not part of the
 * user-facing API.
 *
 * @internal
 */
export declare function enforceLoadingBoundary(enabled: boolean): void;
export declare function setProjectionWriteActive(value: boolean): void;
export declare function setTrackedQueueCallback(value: boolean): void;
export declare function setEffectCallback(value: boolean): void;
export type QueueCallback = (type: number) => void;
type QueueStub = {
    _queues: [QueueCallback[], QueueCallback[]];
    _children: QueueStub[];
};
type OptimisticNode = Signal<any> | Computed<any>;
export interface Transition {
    _time: number;
    _asyncReporters: Map<Computed<any>, Set<Computed<any>>>;
    _pendingNodes: Signal<any>[];
    _optimisticNodes: OptimisticNode[];
    _affectsNodes: OptimisticNode[];
    _optimisticStores: Set<any>;
    _actions: Array<Generator<any, any, any> | AsyncGenerator<any, any, any>>;
    /** An action ran in this transaction (#3427, set by action()): once
     * `_actions` drains, its bodies are OVER — as opposed to a transaction that
     * never had one, whose bare optimistic writes live until it settles. */
    _acted?: boolean;
    _queueStash: QueueStub;
    _done: boolean | Transition;
    _gatedSubs: Set<Computed<any>>;
    /** Effects whose single value slot was written under this transaction AND
     * another live one (#3322). Re-dirtied at commit, ahead of the effect
     * phase, so the run publishes a value derived from the committed world
     * rather than whichever transaction's staged view wrote last. */
    _contested: Computed<any>[] | null;
}
/**
 * Flip-entanglement (#3164 follow-up): `until()` is a declaration of
 * relatedness — the predicate names the condition that confirms the awaiting
 * transaction. When the predicate settles truthy, every live foreign
 * transition whose staged write it read IS the confirming event by the
 * user's own definition, so it merges into the awaiting transaction and
 * reveals at the joint settle — the cross-primitive twin of the family fold
 * (a landing on an optimism-carrying family joins the retaining
 * transaction). Non-flipping updates never pass through here: falsy
 * evaluations don't entangle, so unrelated traffic on the watched sources
 * reveals freely on its own schedule.
 *
 * Runs inside the predicate's compute (pure phase) — the confirming
 * transition's stamps are still live and its commit decision hasn't run, so
 * the merge lands before any reveal. Only the tree-shaken graphs that call
 * `until()` retain this.
 */
export declare function entangleConfirmingTransitions(obs: Computed<any>, target: Transition): void;
export declare function schedule(): void;
/** Arm the microtask `schedule()` withheld under projectionWriteActive. The
 * projection draft calls this after a write made outside its run with no
 * flight up (proj R37): nothing else will drain, and leaving `scheduled`
 * armed with no microtask strands the whole scheduler — every later
 * `schedule()` early-returns — until something calls `flush()` by hand.
 * (`withheld` was set in the same synchronous slice as this call, so the
 * syncDepth/_running gates it passed still hold; a stale mark left by a
 * landing's own flush arms at worst one no-op drain.) */
export declare function scheduleWithheld(): void;
/**
 * Parked transactions whose reporter set changed without a write. A
 * transaction completes when nothing live reports a flight it waits on, but
 * the flush only judges the ACTIVE transaction: a parked one is re-entered by
 * a stamped node's landing or an action's resume. A reporter that stops
 * counting for another reason — its loading boundary flipped to the fallback
 * (#3375), or it was disposed by ambient work (#3372) — is neither: the
 * pruning in `reporterBlocksSource` would drop it at the next check, but no
 * check comes, and the writes held with it stay staged. Such sites record the
 * transaction here (deduped: one idle pass per transaction, however many
 * reporters changed); the flush re-enters it on an otherwise idle pass, so
 * the re-evaluation adopts no unrelated ambient work.
 */
export declare const wokenTransitions: Transition[];
/** Wake every parked transaction — for a site that knows a reporter stopped
 * counting but not whose (a boundary reset). */
export declare function wakeParked(): void;
/** A boundary that can be re-armed (boundaries.ts `CollectionQueue._rearm`). */
export interface Rearmable {
    _rearm(): void;
}
/**
 * Boundaries whose `on` dependencies notified this flush (#3540; a Set:
 * many notifications, one re-arm). The notification arrives inside a pass,
 * at the height of the `on` reads — before the readers the write put in
 * flight are registered — so the re-arm waits for the heap and runs before
 * the verdict (GlobalQueue.run → drainRearms), still under the write's
 * transaction: the release is seen by the verdict that follows, and the
 * staged fallback swap lands with the write's frame.
 */
export declare const pendingRearms: Set<Rearmable>;
export declare function queueRearm(boundary: Rearmable): void;
/** Transactions a mainline tick has PROPOSED against (A34, #3494): a write to a
 * node one of them holds — the same value or another — is a second proposal
 * on a contested node, and the tick reveals with the hold ("both are
 * suggesting a value; if one finished before the other that would be odd").
 * Entered at the next flush's start, where the ambient batch is adopted;
 * never from the write itself, which left `activeTransition` set across the
 * caller's block and made creation after the write the transaction's (A29). */
export declare const batchJoins: Transition[];
/**
 * Permanently halts the reactive system. Called when a user error escapes
 * every boundary — app state is undefined at that point, so scheduling stops
 * entirely rather than limping along with a half-applied update.
 */
/**
 * The key a root owner carries its client error hook under (`render`'s
 * `onError`) — registered, so a runtime writes it with no import of the hook
 * module (core/error-hooks.ts) and no property mangling in the way. Defined
 * HERE, not there: a runtime that only writes the key must not retain the
 * hook machinery (pay-for-use).
 */
export declare const ROOT_ERROR_HOOK: unique symbol;
export declare function haltReactivity(cause?: unknown): void;
/** @internal Test/dev-reload hook. Revives scheduling after a halt. */
export declare function resetErrorHalt(): void;
export interface IQueue {
    enqueue(type: number, fn: QueueCallback): void;
    run(type: number): boolean | void;
    addChild(child: IQueue): void;
    removeChild(child: IQueue): void;
    created: number;
    notify(node: Computed<any>, mask: number, flags: number, error?: any): boolean;
    stashQueues(stub: QueueStub): void;
    restoreQueues(stub: QueueStub): void;
    _parent: IQueue | null;
    /** Loading/error boundary queues (boundaries.ts): the status dimension the
     * queue consumes, and whether it currently shows content (initialized) or
     * its fallback (collecting). Read by `reporterBlocksSource`. */
    _collectionType?: number;
    _initialized?: boolean;
}
export declare class Queue implements IQueue {
    _parent: IQueue | null;
    _queues: [QueueCallback[], QueueCallback[]];
    _children: IQueue[];
    _ranAt: number;
    created: number;
    addChild(child: IQueue): void;
    removeChild(child: IQueue): void;
    notify(node: Computed<any>, mask: number, flags: number, error?: any): boolean;
    run(type: number): void;
    enqueue(type: number, fn: QueueCallback): void;
    stashQueues(stub: QueueStub): void;
    restoreQueues(stub: QueueStub): void;
}
export declare class GlobalQueue extends Queue {
    _running: boolean;
    _batch: Transition;
    static _update: (el: Computed<unknown>) => void;
    static _dispose: (el: Computed<unknown>, self: boolean, zombie: boolean) => void;
    static _runEffect: (el: Computed<unknown>) => void;
    static _clearOptimisticStores: ((stores: Set<any>, completing: Transition | null) => void) | null;
    static _releaseAffectsScope: ((node: OptimisticNode) => void) | null;
    static _releaseAffectsMarks: ((nodes: OptimisticNode[]) => void) | null;
    static _markAffects: ((node: OptimisticNode) => void) | null;
    static _releaseAffectsMark: ((node: OptimisticNode) => void) | null;
    static _wireExternalSource: ((self: Computed<any>) => void) | null;
    static _externalUntrack: (<T>(fn: () => T) => T) | null;
    static _syncCompanions: (<T>(el: Signal<T> | Computed<T>, value: T) => void) | null;
    static _updatePendingSignal: ((el: OptimisticNode) => void) | null;
    static _updateChildCompanions: ((el: Computed<any>) => void) | null;
    static _snapCompanions: ((el: OptimisticNode) => void) | null;
    static _latestRead: (<T>(el: Signal<T> | Computed<T>) => T) | null;
    static _pendingCheck: ((el: OptimisticNode, c: Computed<any> | null, owner: OptimisticNode, firewall: Computed<any> | null) => void) | null;
    static _recordFresh: ((el: OptimisticNode, value: any) => void) | null;
    static _applyReask: ((el: Computed<any>, hadReask: boolean) => boolean) | null;
    static _repollVerdicts: ((el: Computed<any>, snap?: boolean) => void) | null;
    static _witnessAffects: ((node: OptimisticNode) => void) | null;
    static _wakeSuppressedProbes: ((transition: Transition) => void) | null;
    static _optimisticWrite: (<T>(el: Signal<T> | Computed<T>, v: T | ((prev: T) => T)) => T) | null;
    static _resolveOptimistic: ((nodes: OptimisticNode[]) => void) | null;
    static _transitionBlocked: ((transition: Transition) => boolean) | null;
    static _cleanupLanes: ((completingTransition: Transition | null) => void) | null;
    static _runLaneEffects: ((type: number) => void) | null;
    /** Patch-channel optimistic drain (next/patch.ts): optimistic emissions
     * apply at lane-effect timing — visible in flight, unlike the regular
     * effect queues an action stashes. Injected; null when unused. */
    static _drainPatchOptimistic: (() => void) | null;
    static _gatedRead: ((el: Signal<any>, owner: OptimisticNode, c: Computed<any>) => boolean) | null;
    static _laneSuspends: ((owner: OptimisticNode) => boolean) | null;
    /** Is the node routed through a LIVE lane (`resolveLane`)? read()'s reveal
     * carve-out asks before showing a foreign-held pending node's committed
     * value: a lane-derived flight's inputs are already revealed through the
     * lane (#3334). Gated on CONFIG_HAS_LANE, which only the engine sets. */
    static _laneLive: ((el: Computed<any>) => boolean) | null;
    static _laneReadsCommitted: ((el: OptimisticNode, owner: OptimisticNode, c: Computed<any>) => boolean) | null;
    static _recomputeLane: ((el: Computed<any>, own: boolean) => OptimisticLane | null | false) | null;
    static _laneAsyncPending: ((el: Computed<any>) => void) | null;
    /** Authoritative-view reader wakeup: installed by until() and refresh() before
     * their first read. Call sites are gated by CONFIG_AUTHORITATIVE_OBSERVED, which
     * only such a reader's carve-out read can set, so `!` invocations are safe once
     * the gate holds (#3303). */
    static _notifyAuthoritativeObservers: ((el: Signal<any> | Computed<any>) => void) | null;
    static _laneAsyncSettled: ((el: Computed<any>) => void) | null;
    /** A18 supersession (#3331): own-source truth `value` landed under an active
     * override. The engine decides whether the graph re-derives — the value
     * differs from the override and is not a stale (older-action) answer (mark
     * the node, demote its lane cascade, notify), or returns to it after an
     * earlier differing arrival (clear the mark, notify) — and owns the
     * authoritative-observer wake for a silent confirm. Installed with the
     * optimistic engine; only reachable on a node that has an override. */
    static _supersedeOverride: ((el: Signal<any> | Computed<any>, value: unknown) => void) | null;
    /** The flush's pre-verdict step (#3427): once the transaction's action
     * bodies have all ended and nothing authoritative is left in flight, the
     * engine supersedes every override still in force with the truth it
     * reverts to, so the graph re-derives from it now, as the transaction's
     * held work, instead of after the flights the overrides fed have landed.
     * True when it superseded something: the caller re-runs the heap ahead of
     * the verdict. The engine owns every gate (acted, actions drained, has
     * overrides, no store edits, no authoritative flight); null without it. */
    static _endOptimism: ((transition: Transition) => boolean) | null;
    /** read()'s value for a TRACKED reader of a superseded node (#3331): the
     * staged truth, unless the reader is a stale (render) reader of another
     * transaction — then the displayed override, as it keeps a foreign
     * transaction's committed value over its staged write. */
    /** A tracked read of an active override: the lane outside-view rule
     * (#3460) and the A18 supersession selection (#3331) — see optimistic.ts. */
    static _overrideRead: ((el: Computed<any>, c: Computed<any>) => unknown) | null;
    /** A lane pass's publish for a memo (#3479, lanes stage): the speculative
     * result becomes a DERIVED override, `_value` stays committed — see
     * optimistic.ts laneOverride. Set with the engine, which a lane implies. */
    static _laneOverride: ((el: Computed<any>, value: unknown, lane: OptimisticLane) => void) | null;
    /** Verdict-layer recompute in progress (companion creation, latest()/
     * isPending() pulls): never born held — see core.ts enterStagedRead. */
    static _verdictPull: boolean;
    /** setSignal's authoritative (projection-write) landing on an override-
     * covered node (#3331 store twin): stage the truth for its transaction's
     * commit whatever its relation to the committed value — a landing equal to
     * committed still differs from the override — then _supersedeOverride
     * decides. Installed with the optimistic engine; only reachable on a node
     * that has an override — or, from `mapArray` once a lane pass has run over
     * the map, on a per-slot signal (never CONFIG_OPTIMISTIC): the slot arm
     * publishes a lane pass's write as the slot's derived override, lands a
     * plain pass's write over one, and is the plain `setSignal` otherwise (F1,
     * see optimistic.ts landOnOverride). */
    static _landOnOverride: (<T>(el: Signal<T> | Computed<T>, v: T | ((prev: T) => T)) => T) | null;
    static _trackOptimisticStore: ((store: any) => void) | null;
    flush(): void;
    notify(node: Computed<any>, mask: number, flags: number, error?: any): boolean;
    initTransition(transition?: Transition | null): void;
}
export declare function queuePendingNode(node: Signal<any>): void;
export declare let reaskArmed: boolean;
/** §12d: bumped by every recompute and every new subscriber edge. A node's
 * staged-rewrite skip is sound only while NOTHING recomputed or linked since
 * its last notify — a mid-batch pull can clean a marked subscriber, and a
 * skipped re-write would leave it stale. */
export declare let notifyEpoch: number;
export declare function bumpNotifyEpoch(): void;
export declare function armReaskClear(): void;
/** Provenance of the work currently running (A18 supersession, #3331): the
 * invocation sequence of the action whose ambient window this is — set by
 * action() for each slice; the flush that ends the window clears it — or,
 * inside an async landing, the sequence captured when that flight was
 * registered (asyncWrite sets it for the landing's synchronous propagation,
 * so a sync recompute downstream of the landing — an optimistic wrapper over
 * the async source — derives under the flight's provenance, and flights it
 * registers inherit it). 0 is mainline: no action, always the current
 * question. An override stamps this at its write (`_overrideStamp`); an
 * answer whose flight an OLDER action issued is a stale question the user
 * has since changed — it holds silently to commit instead of superseding. A
 * slow source must not leak back in over a newer intent. Transactions merge,
 * so the transition object cannot say WHICH action asked; this can. */
export declare let origin: number;
export declare function setOrigin(seq: number): number;
export declare function insertSubs(node: Signal<any> | Computed<any>, optimistic?: boolean): void;
export declare let storeCommitHook: (() => void) | null;
export declare function setStoreCommitHook(fn: () => void): void;
/** Patch-channel release hook (next/patch.ts): transition-stamped patch
 * emissions are released when THEIR batch commits. Transitions never
 * abort: failed actions still commit (only optimistic overrides revert),
 * and merged-away transitions hand their stash to the survivor
 * (mergeTransitionState) — every stash drains exactly once. Injected like
 * storeCommitHook to stay tree-shakeable. */
export declare let patchCommitHook: ((batch: Transition) => void) | null;
export declare function setPatchCommitHook(fn: (batch: Transition) => void): void;
/** Unchanged passes with a stale dependency tail, waiting on this flush's
 * verdict (A30, #3469). A pass that changed nothing replaced nothing either —
 * and cannot know at its own tail whether the flush that ran it will park:
 * parked, its inputs are held and the committed frame still derives from the
 * tail (`b() ? b() : a()` computed `1` from the held `b`, equal to the `1` it
 * had from `a` — with `a` trimmed, the mainline `a = 2` never reached it).
 * Trimmed when the flush commits; dropped with a park, the tail stays linked
 * until a committing pass trims it (one spurious recompute at most). */
export declare const heldTrims: Computed<any>[];
export declare function finalizePureQueue(completingTransition?: Transition | null, incomplete?: boolean): void;
/**
 * Count of live `affects()` registrations across the system (including
 * store-scope inherited marks). Gates the read-path mark check in `read()` so
 * graphs that never use the feature pay one integer compare.
 */
export declare let activeAffectsMarks: number;
/**
 * Counter mutation seam for the mark engine in affects.ts: an imported `let`
 * binding is read-only, and the read-path gate above must stay a plain module
 * variable so `read()` pays one integer compare, not a function call.
 *
 * @internal
 */
export declare function shiftAffectsMarks(delta: 1 | -1): void;
export declare const globalQueue: GlobalQueue;
/**
 * Synchronously processes the pending reactive queue, or runs `fn` in a synchronous
 * flush scope before draining the queue.
 *
 * Reactive updates are normally batched onto the microtask queue, so multiple
 * writes in a row collapse into a single update pass. Call `flush()` when you
 * need to *observe* the result of those writes synchronously — most commonly
 * in tests, but also at the boundary of imperative integration code. Pass a
 * callback when the writes themselves should bypass microtask scheduling and
 * drain synchronously when the callback returns.
 *
 * @example
 * ```ts
 * const [count, setCount] = createSignal(0);
 * const doubled = createMemo(() => count() * 2);
 *
 * setCount(5);
 * flush();
 * expect(doubled()).toBe(10);
 *
 * flush(() => setCount(6));
 * expect(doubled()).toBe(12);
 *
 * // Nested flushes drain at each level:
 * flush(() => {
 *   setCount(7);
 *   flush(() => setCount(8)); // inner drain — effects fire here
 *   // outer continues with up-to-date state
 * });
 * ```
 */
export declare function flush(): void;
export declare function flush<T>(fn: () => T): T;
/** Does `reporter` still hold the transaction waiting on `source` — live,
 * routed to no collecting loading boundary, and deriving from the source in
 * its current pass? The verdict's per-reporter test (sourceObserved), also
 * a boundary re-arm's (boundaries.ts `_rearm` runs before the verdict prunes
 * registrations that stopped counting). */
export declare function reporterBlocksSource(reporter: Computed<any>, source: Computed<any>, verdict?: Transition): boolean;
/**
 * Does a live reporter of `transition` still observe `source` pending? Dead
 * reporters (disposed, behind a fallback, no longer reading the source) are
 * pruned as they are found, and the source's entry with them. Shared by the
 * settle verdict and the lane's hold check (`waitingTransition`): a live
 * action parks its transaction without a verdict, so this prune is the only
 * one an optimistic lane whose last async reader unmounted mid-action ever
 * gets — without it the lane held on the dead reporter's registration until
 * the flight it no longer observed landed (#3426).
 */
export declare function sourceObserved(transition: Transition, source: Computed<any>, verdict?: Transition): boolean;
/** A fresh, unentered transaction (#3146): the optimistic store's truth
 * flight DECLARES an owned transaction instead of relying on whatever the
 * ambient adoption machinery stamped on its firewall. Activate it with
 * initTransition; it is a plain batch until then. */
export declare function createTransition(): Transition;
export declare function currentTransition(transition: Transition): Transition;
/**
 * The live transition blocked on `source` — the one whose render reader
 * observed it pending (INV-3 records the observation in whichever transaction
 * was active when the reader was notified). The observation is a fact about
 * the node, so a hold check must not assume it was recorded in the transaction
 * it happens to hold — lanes merge across transactions (#2912), and a merged
 * root's transaction knows nothing of the async its members' transactions
 * observed (#3335). Null when nobody is waiting — a registration whose every
 * reporter has since died is nobody (#3426).
 */
export declare function waitingTransition(source: Computed<any>): Transition | null;
/** A landing enters EVERY parked transaction still waiting on `source`, folding
 * them into the active one (A15: each reveal that discovered the flight
 * completes at its landing). The fold used to happen as the waiters' stamped
 * readers recomputed under the landing — recompute re-entering an effect's
 * stamp — which also folded in writes those readers merely shared a hole
 * with (#3407); effects no longer re-enter, so the landing folds explicitly.
 * Live iteration is safe: a merge deletes the outgoing (active) entry and
 * re-adds the visited one. */
export declare function enterWaiting(source: Computed<any>): void;
export declare function setActiveTransition(transition: Transition | null): void;
export declare function runInTransition<T>(transition: Transition, fn: () => T): T;
/** Run `fn` with `transition` as BOTH the ambient transaction and the
 * registration batch, restoring both after. runInTransition alone is not
 * enough for code that WRITES on behalf of a transaction from inside someone
 * else's window (optimistic replay re-arming a still-open action's edits
 * during a landing commit, #3123): registrations route through the queue's
 * batch pointer, and a bare activeTransition swap leaves them in the ambient
 * batch — a plain batch "completes" at the next flush and reverts optimistic
 * registrations that were supposed to live with the transaction.
 * initTransition is the wrong tool here: it MERGES the currently ambient
 * transaction into the target, entangling whatever the interrupted window
 * belonged to. */
export declare function runAsTransitionBatch<T>(transition: Transition, fn: () => T): T;
