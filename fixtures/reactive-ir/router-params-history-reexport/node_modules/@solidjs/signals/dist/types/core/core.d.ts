import { type OptimisticLane } from "./lanes.js";
import type { Transition } from "./scheduler.js";
import type { Computed, FirewallSignal, NodeExtension, NodeOptions, Owner, Signal } from "./types.js";
export declare const PRIMITIVE_IN_FORBIDDEN_SCOPE_MESSAGE = "[PRIMITIVE_IN_FORBIDDEN_SCOPE] Cannot create reactive primitives inside createTrackedEffect or owner-backed onSettled";
export declare const REACTIVE_WRITE_IN_OWNED_SCOPE_SIGNAL_MESSAGE: string;
export declare const REACTIVE_WRITE_IN_OWNED_SCOPE_REFRESH_MESSAGE: string;
export declare const ASYNC_STORE_SETTER_MESSAGE: string;
export declare let tracking: boolean;
/** @internal verdict-module glue */
export declare function setPendingCheckActive(v: boolean): void;
/** @internal verdict-module glue */
export declare function setLatestReadActive(v: boolean): void;
/** @internal verdict-module glue */
export declare function setContextInternal(v: Owner | null): void;
export declare let stale: boolean;
export declare let pendingCheckActive: boolean;
export declare let latestReadActive: boolean;
export declare let context: Owner | null;
export declare let currentOptimisticLane: OptimisticLane | null;
/** Notify `node`'s subscribers on `lane`'s channel: they recompute as the
 * lane's passes — a memo publishes a derived override (lanes stage, #3479),
 * an effect runs from the lane's queue, at the park, ahead of the
 * transaction — the display-ahead view. The write itself is already staged
 * (setSignal) and commits with the frame; this walk shows it now. Used by a
 * boundary re-armed from a lane pass (boundaries.ts `_swap`, #3540). */
export declare function notifyOnLane(node: Signal<any>, lane: OptimisticLane): void;
export declare let snapshotCaptureActive: boolean;
export declare let snapshotSources: Set<any> | null;
export declare function setSnapshotCapture(active: boolean): void;
export declare function markSnapshotScope(owner: Owner): void;
export declare function releaseSnapshotScope(owner: Owner): void;
export declare function clearSnapshots(): void;
export declare function recompute(el: Computed<any>, create?: boolean): void;
export declare function computed<T>(fn: (prev?: T) => T | PromiseLike<T> | AsyncIterable<T>): Computed<T>;
export declare function computed<T>(fn: (prev: T) => T | PromiseLike<T> | AsyncIterable<T>, options?: NodeOptions<T>): Computed<T>;
/** Lazily allocate a node's cold extension (ONE shape for signals and
 * computeds — `_x` access stays monomorphic). Installers write through
 * this; hot paths read `el._x?._field` gated by the _config presence bits.
 * Never call ext() just to store a field's default. */
export declare function ext(el: {
    _x: NodeExtension | null;
}): NodeExtension;
/**
 * Build an Effect node with all effect-specific fields baked into a single object literal,
 * so V8 sees the full hidden class shape at construction time. Effects always run in lazy
 * mode (recompute is called explicitly by `effect()`), so we hardcode the lazy bits and skip
 * the auto-dispose CONFIG bit (effect() previously cleared it post-construction).
 */
export declare function createEffectNode<T>(fn: (prev?: T) => T, effectFn: (val: T, prev: T | undefined) => void | (() => void), errorFn: ((err: unknown, cleanup: () => void) => void | (() => void)) | undefined, type: number, options: NodeOptions<T> | undefined): any;
/**
 * The shared status notifier for effect nodes, installed once by effect.ts
 * at module evaluation (`this`-dispatched — one function serves every
 * effect, so nodes never store it). Boundary computeds keep their own
 * per-node channel on `_x._notifyStatus`, which takes precedence.
 */
export declare let effectStatusNotify: ((this: any, status?: number, error?: any) => void) | null;
export declare function setEffectStatusNotify(fn: NonNullable<typeof effectStatusNotify>): void;
/** Resolve a node's status notifier: an own `_x` channel (boundaries) wins;
 * effect nodes (`_type` — EFFECT_PURE is 0, and only effect literals carry
 * the field) fall back to the shared notifier. Presence doubles as the
 * "display consumer" membership test in the status walks, exactly as the
 * per-node field did when every effect carried one. */
export declare function statusNotifierOf(el: any): ((this: any, status?: number, error?: any) => void) | undefined;
export declare function signal<T>(v: T, options?: NodeOptions<T>): Signal<T>;
export declare function signal<T>(v: T, options?: NodeOptions<T>, firewall?: Computed<any>): FirewallSignal<T>;
/** The shared slot-node unobserved handler — a live binding read directly by
 * the sweep sites (no wrapper frame, no null check: a CONFIG_SLOT_NODE node
 * existing implies the store module loaded and registered the hook). */
export declare let slotUnobservedHook: (node: Signal<any>) => void;
/** Install the shared slot-node unobserved handler (store module, once). */
export declare function setSlotUnobserved(fn: (node: Signal<any>) => void): void;
/** Release a firewall child the store no longer addresses (unobserved sweep
 * dropped it from its target's cache): unlink it from the chain so the
 * projection stops retaining it and its last value. The node keeps its own
 * `_nextChild` so a walk that is mid-chain on it still terminates. It also
 * leaves `_companionChildren` (#3503): the companions themselves are
 * permanent on the node, but the node is unreachable through the store, so
 * the set would only retain it and its last value. */
export declare function unlinkFirewallChild(node: Signal<any>): void;
export declare function slotSignal<T>(v: T, equals: (a: T, b: T) => boolean, host: object, key: PropertyKey, acc: boolean, firewall?: Computed<unknown> | null): Signal<T>;
export declare function optimisticSignal<T>(v: T, options?: NodeOptions<T>): Signal<T>;
export declare function optimisticComputed<T>(fn: (prev?: T) => T | PromiseLike<T> | AsyncIterable<T>, options?: NodeOptions<T>): Computed<T>;
export declare function isEqual<T>(a: T, b: T): boolean;
/**
 * When set to a component name string, any reactive read that is not inside a nested tracking
 * scope will log a dev-mode warning. Managed automatically by `untrack(fn, strictReadLabel)`.
 */
export declare let strictRead: string | false;
/** Dev-only: explicit untrack() nesting, so deliberate post-await reads stay quiet. */
export declare let untrackDepth: number;
/**
 * Dev-only: > 0 while Solid runs user code that is imperative by construction
 * — an effect callback (effect.ts) or an action body's synchronous slice
 * (action.ts). Both run with no owner, exactly like an async continuation, so
 * the post-await read check (dev.ts) consults this rather than blame the
 * continuation that called flush() or invoked the action. Both sites bracket
 * with try/finally, so a throw cannot leave it raised.
 */
export declare let callbackDepth: number;
export declare function enterCallback(): void;
export declare function exitCallback(): void;
/**
 * Dev-only: > 0 while owner teardown runs cleanups (`_disposal` entries and
 * effect-returned cleanups — owner.ts), for the same reason. Kept apart from
 * `callbackDepth` because its sites cannot use try/finally (the frame would
 * survive into prod): a throwing cleanup leaves it raised, and dev.ts resets
 * it on the next microtask, which teardown — synchronous — never spans.
 */
export declare let disposalDepth: number;
export declare function enterDisposal(): void;
export declare function exitDisposal(): void;
export declare function resetDisposalDepth(): void;
export declare function setStrictRead(v: string | false): string | false;
/**
 * Runs `fn` outside of any reactive tracking — reads inside `fn` will not
 * subscribe the current scope. Returns whatever `fn` returns.
 *
 * Use `untrack` inside a memo or effect when you need to read a signal once
 * without making the surrounding computation depend on its future changes.
 *
 * Pass a `strictReadLabel` string to enable a dev-mode warning: any reactive
 * read inside `fn` that isn't inside a nested tracking scope will log a
 * warning naming the label.
 *
 * @example
 * ```ts
 * createEffect(
 *   () => trigger(),                 // tracks `trigger` only
 *   () => {
 *     const snapshot = untrack(() => state); // read once, untracked
 *     log(snapshot);
 *   }
 * );
 * ```
 */
export declare function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
/**
 * Set while runtime bookkeeping reads a node inside another node's pass (a
 * loading boundary priming its tree at creation, from whatever pass is
 * mounting it). `context` is that node, but the read is nobody's: the value
 * is probed, never derived from, so nothing the read would normally record
 * on `context` may be recorded — not the untracked-pending re-run link
 * (`read`, `!tracking`; #3528: a boundary's `on` key read this way from
 * `notify` linked the key's source into an unrelated async memo, a cycle that
 * never converged), and not a transaction entry (`enterStagedRead`; #3540: a
 * born-held tree would otherwise pull the mounting pass into the hold).
 */
export declare let spectating: boolean;
/**
 * Evaluates `fn` untracked, recording nothing on the current `context`: no
 * untracked-pending re-run link, no transaction entry. For bookkeeping reads
 * made on behalf of no node — see `spectating`.
 */
export declare function spectate<T>(fn: () => T): T;
/**
 * Bring a computed to a readable state: lazy/disposed nodes are (re)computed;
 * an isPending() probe (`refresh`) additionally pulls the node fully up to
 * date so its status flags reflect the current graph.
 */
export declare function prepareComputed(comp: Computed<unknown>, refresh: boolean): void;
/**
 * Sentinel returned by readNodeFast when the plain-signal fast path does not
 * apply and the caller must fall back to the full read().
 */
export declare const READ_SLOW: unique symbol;
/**
 * read()'s plain-signal fast path as a standalone entry for hot callers
 * (store traps). Safe to substitute for read() only because the bail
 * conditions mirror read()'s prelude and fast-path guard exactly: the
 * latestRead and pendingCheck windows run side-effectful hooks before the
 * fast path, `_fn` nodes need prepareComputed, and firewall / override /
 * snapshot / transition / lane / dev-strictRead state all take the full
 * resolution. Anything slow returns READ_SLOW; the caller then calls read().
 */
/**
 * Wake only authoritative-view readers (until() predicates) subscribed to `el`.
 * The A17-silent ack paths — an authoritative arrival equal to the active
 * override — use this so the predicate re-evaluates without re-firing
 * ordinary subscribers whose visible (override) value did not change.
 * Pay-for-use: reached through GlobalQueue._notifyAuthoritativeObservers,
 * installed at first until() call — apps that never use until() shake it.
 */
export declare function notifyAuthoritativeObservers(el: Signal<any> | Computed<any>): void;
/** Installs the authoritative-reader wakeup hook. Idempotent; called by every
 * creator of a CONFIG_AUTHORITATIVE_READ computation — until() and refresh() —
 * before its first read (same late-binding contract as the optimistic engine;
 * the gating bit is only ever set by such a read, so the `!` call sites are
 * safe once every setter installs, #3303). */
export declare function installAuthoritativeRead(): void;
/**
 * Stale-reader term of the value selections below: a render effect reading a
 * node some OTHER live transaction has staged sees the committed value. The
 * commit is silent — the staging walk was the notification — so a reader
 * that linked AFTER that walk (an effect created during the hold, a store
 * key first read under it) would show the old value past the reveal: record
 * it for the transaction's commit replay (the `_gatedSubs` contract lanes
 * already use). An effect the transaction itself computed re-derives at its
 * commit on its own (parked run, or the contested re-derive, #3322) and is
 * not recorded — replaying it too would publish the frame twice.
 *
 * Flight twin (the pending-branch carve-out): the reader is served the
 * node's committed, pre-flight value and now observes that flight — A15:
 * async work observed by a reader settles as one unit with the writes that
 * asked it — so it joins the transaction's reporters for the node. The
 * reporter the transaction recorded when the flight started may be gone (a
 * keyed remount disposed it, #3374); a completion check that found no live
 * reporter committed the writes ahead of the answer, tearing the new
 * reader's frame (`Count: 1` beside `Details: 0`). Joins an entry the
 * transaction already holds; a flight nobody had observed yet has none (the
 * reader is its first observer — a conditional that just revealed it, #3458)
 * and is notified up the reader's own queue chain under that transaction,
 * the one sanctioned registration site (INV-3): a collecting boundary above
 * the reader consumes it as it would any pending, an unboundaried reader
 * opens the entry — and the transaction, judged complete on its other
 * flights, revealed the inputs beside the reader's pre-flight value
 * otherwise (`Count: 1 | A: 1` beside `B: 0`). A staged signal or a settled
 * node registers nothing. Every reporter dies with its reader
 * (reporterBlocksSource: the read linked it as a dep). The node's own entry
 * is the only one that can matter: a chain's intermediate memo is re-pulled
 * by the read (updateIfNecessary's retry) and enters the transaction, so the
 * reader holds through the normal path; a node with its own flight that is
 * also pending on an upstream re-ask blocks through that flight until it
 * lands, and its landing re-runs the reader into the normal path.
 */
/** The replay half of the stale-of-foreign clause (A15 / A26): a stale reader
 * served the committed value because `txn` holds what it read re-runs at
 * txn's commit, when the value it was denied becomes the frame — unless its
 * own last value already came from that transaction. One registration for
 * the node path (heldFromStale) and the store's backing paths, which have
 * no node to carry the hold (heldFromReader, the adoption hold view). */
export declare function recordStaleReplay(txn: Transition, c: Computed<any>): void;
/**
 * The ownership relation (DESIGN-CONSOLIDATION §6, ruled 2026-09-17): is
 * `hold` part of the running pass's world? A plain reader's world is the
 * transaction it runs under, through merges. A lane reader's world is its
 * lane AND the transition that owns the lane — the one asymmetry between a
 * lane and a separate transaction (a lane sees what lands from its parent as
 * its own; a separate transaction would wait for the parent to settle) —
 * see `ownsLane` in lanes.ts, built on this. One relation for the
 * stale-of-foreign clause (heldFromStale), the lane arm (readsHeldCommitted)
 * and the store's backing holds (foreignHold); `serve` has no lane arm of
 * its own, the lane's extra visibility lives here.
 */
export declare function ownsHold(hold: Transition): boolean;
export declare function enterStagedRead(el: Signal<any> | Computed<any> | null, t?: Transition | null | undefined): void;
/**
 * Rule 1 (value selection), the full arm: does this reader see a STAGED
 * node's COMMITTED value? One implementation of the rule the fast paths
 * (readNodeFast, read's fast block) carry as their trivial ternary and that
 * every slow site — read's tail, the store's backing selection, the lane and
 * verdict arms — used to restate by hand (docs/DESIGN-CONSOLIDATION.md, move 3b). In order:
 * - no reader at all (an untracked read) — the committed frame;
 * - a reader under an optimistic lane the engine says reads committed
 *   (laneReadsCommitted: another lane's hold, #3460);
 * - nothing staged;
 * - a children-forbidden reader (createTrackedEffect / onSettled: the frame,
 *   never the graph — A32);
 * - a stale reader (render effect) of a FOREIGN transaction's staged write —
 *   committed, no entanglement (heldFromStale registers the replay; a node
 *   born held has no committed frame to fall back to, `noCommitted`);
 * - HELD truth (#3164, CONFIG_HELD_TRUTH) read by a LANE pass: staged
 *   confirming truth — fold-staged onto an armed family, or entangle-stolen
 *   by an awaited until() — is masked from lane passes only, owning
 *   transaction or not. A lane applies its frame display-ahead at the park,
 *   so a lane pass served the truth would paint the confirmation beside the
 *   optimism it confirms (`saving=true` beside the saved row — the #3164
 *   tear); it keeps committed and is re-run by the reveal's post-revert
 *   wake. Every other deriving reader falls through to A29 below: the truth
 *   is a staged value like any other, and the pass that derives from it is
 *   held with it — including the retaining transaction's own passes, which
 *   a superseded override already hands the truth (#3568: masking them
 *   composed the landed `length` with rows still masked to committed).
 *   latest() and authoritative readers (until()'s predicate) tunnel
 *   through — the tunnel that keeps the hold deadlock-free.
 * False means the reader derives from the staged value and enters its
 * transaction (enterStagedRead, A29).
 */
export declare function readerSeesCommitted(el: Signal<any> | Computed<any>, c: Computed<any> | null, owner: Signal<any> | Computed<any>, noCommitted: boolean): boolean;
/** A28 — set when a node is staged (queuePendingNode) or a held node rewritten
 * (stashHeldRewrite) OUTSIDE a flush; cleared when the next flush begins. The
 * read sites test this one module boolean instead of `globalQueue._running`:
 * inside a flush it is false and the A28 arm costs nothing; outside, only a
 * tick with unflushed writes pays the staged-node check. */
export declare let unflushedStaged: boolean;
export declare function markUnflushedStaged(): void;
/** A28 — a write becomes visible at flush. Outside a flush, a node holding an
 * AMBIENT staged value (no transaction stamp) was written since the last
 * flush: ambient staging commits at flush end, so nothing else leaves a node
 * in this state; a stamped value is the flushed held world, which A28 says
 * latest() serves. Inside a flush the rule does not apply (A28 (4): promoted
 * within the round). Structural — no marker on the write path. */
export declare function unflushed(el: Signal<any> | Computed<any>): boolean;
/** The value an unflushed node serves — the committed value for an ambient
 * write, the flushed staged value for a rewrite of a held node — or
 * NOT_PENDING when nothing is unflushed. Exempt: owned-write nodes (A28 (4):
 * a write issued inside a recompute is promoted at that recompute's end —
 * boundary and loading machinery, until()'s internals, signals declared for
 * in-computation writes) and engine companions (the isPending() verdict
 * signal, the latest() shadow: the system's own writes, made at the source's
 * write to mirror it, installing eagerly — A28, A8). */
export declare function unflushedValue(el: Signal<any> | Computed<any>, committed?: any): unknown;
/** A28 (5): an optimistic write becomes the ACTIVE override at the flush that
 * carries it. `_overrideTime` is stamped with `clock` at the write and `clock`
 * advances after every flush, so "this tick, outside a flush" is unflushed. */
export declare function unflushedOverride(el: Signal<any> | Computed<any>): boolean;
/** Active optimistic override on an armed node (an armed slot idles at
 * NOT_PENDING; undefined = unarmed plain node). The writer's own channels —
 * the draft, `in`/keys inside the setter — compose on this regardless of
 * flush state. */
export declare function hasActiveOverride(el: Signal<any> | Computed<any>): boolean;
/** The override a READER sees: installed, and carried by a flush (A28 (5) —
 * an optimistic write is a write; until its flush no reader sees it). One
 * implementation for read()'s override arm, the verdict channels
 * (latestRead, computePendingState) and the store's selection
 * (docs/DESIGN-CONSOLIDATION.md, move 3b). */
export declare function visibleOverride(el: Signal<any> | Computed<any>): boolean;
/** A derivation served the committed value because of an unflushed write
 * (A28) must run again in the flush that carries it — the late-linker case
 * (#3337's reason to defer the walk): it linked after the write walked. */
export declare function markLateLinker(c: Computed<any>): true;
/** Companion-bearing nodes written outside a flush (setSignal); the flush
 * that carries their writes re-syncs their companions (A28). */
export declare const unflushedCompanions: Array<Signal<any> | Computed<any>>;
export declare function resyncUnflushedCompanions(): void;
export declare function readNodeFast<T>(el: Signal<T>): T | typeof READ_SLOW;
export declare function read<T>(el: Signal<T> | Computed<T>): T;
/**
 * Rule 1, the one slow implementation (DESIGN-CONSOLIDATION move 3b, step
 * 6c): the value a reader `c` (null = untracked, no pass) is served from
 * `el`, whose committed value is `committed` — the node's own `_value` for
 * a signal or memo, the BACKING for a store property node (single-home rule,
 * O6: committed truth lives in the backing and a node's `_value` is never
 * served for one). Called by read()'s slow tail and by the store's untracked
 * node path (nodeValue); the fast paths (readNodeFast, read's fast block)
 * keep their trivial ternary by design (perf, see the doc). Arms, in order:
 * - the override (A17), routed through the engine for a tracked reader under
 *   a lane or a supersession (A18), an authoritative reader marked instead;
 * - the lane entanglement gate (committed, recorded for replay);
 * - a node born held has nothing for an untracked reader (A19 exception 1);
 * - an unflushed write serves committed and re-runs the reader in the
 *   carrying flush (A28);
 * - readerSeesCommitted, else the staged value and the transaction (A29).
 */
export declare function serve(el: Signal<any> | Computed<any>, c: Computed<any> | null, owner: Signal<any> | Computed<any>, committed: unknown): unknown;
/**
 * Store-rewrite setter guard: the rewrite parks writes in a pending backing
 * (no setSignal at write time), so the owned-scope write protection must
 * fire at the setter entry instead. Exactly setSignal's guard condition
 * minus the node-specific exemptions (ownedWrite/firewall), which don't
 * apply to plain store setters. Roots are NOT exempt (#3500): a root body is
 * tree construction — every dev component body, every context Provider, the
 * top of `render()`, and the whole SSR pass run directly under one — and a
 * write there re-runs what already read the old value (or on the server,
 * can't). Same rule as setSignal, which never exempted roots.
 */
export declare function devGuardStoreSetterWrite(): void;
/**
 * Store setter result guard: the callback's return has one meaning — a
 * replacement root to adopt — and a thenable can never be that. It is the
 * signature of `setStore(async d => …)` (or a sync arrow whose helper is
 * async): only the writes before the first `await` were in the transaction;
 * the rest land on a closed draft and vanish. Setters are synchronous
 * transactions; async orchestration is `action()`'s job. Store-specific —
 * a signal may legitimately hold a promise, so its setter has no such rule.
 */
export declare function devGuardStoreSetterResult(result: unknown): void;
export declare function setSignal<T>(el: Signal<T> | Computed<T>, v: T | ((prev: T) => T)): T;
/**
 * Suppresses automatic recomputation of `el` until the scheduler drains. Used
 * when a manual write should win over dependency changes queued in the same
 * tick. The MANUAL_WRITE flag is cleared by the pending-node drain; projection
 * computeds don't commit values, but they still need the same end-of-tick
 * cleanup point.
 */
export declare function suppressComputedRecompute(el: Computed<unknown>): void;
/** A34 amendment (#3612) — is `el`'s staging a DERIVATION another transaction
 * holds: stamped by a transaction that is not the writer's, and not a manual
 * proposal (the mask, on the node or — a store leaf — its firewall)? #2692's
 * "manual write wins" is a rule for one synchronous frame; across a hold the
 * held pass result is nobody's proposal, and a user setter reaching it
 * composes on the committed frame it was written against and becomes `prev`
 * for the transaction's re-derivation (rederiveHeld) instead of replacing it.
 * Writes made under the holding transaction masked the node and keep
 * A34(1)'s last-write-wins. Only the user setters ask; an async landing or a
 * companion writing a stamped node is the transaction's own work. */
export declare function heldDerivation(el: Signal<any> | Computed<any>): boolean;
/** The held-derivation write's second half: the node re-derives under its
 * hold with the written staging as the pass's `prev`. Nothing is masked. The
 * write took the A34 join (setSignal), which scheduled the flush that drains
 * this; recompute re-enters the stamp. */
export declare function rederiveHeld(el: Computed<unknown>): void;
/**
 * User-facing setter for the memo form of `createSignal(fn)`. Behaves like
 * `setSignal`, but also cancels any pending recompute of the memo so the
 * manual value wins over a value that would otherwise be produced by an
 * upstream change in the same tick. Across a hold the write is not a
 * proposal: a memo another transaction holds as a pass result composes on the
 * committed value and re-derives under the hold with the write as `prev`
 * (A34 amendment, #3612; heldDerivation).
 */
export declare function setMemo<T>(el: Computed<T>, v: T | ((prev: T) => T)): T;
/**
 * Executes `fn` with the given `owner` set as the current owner. Any reactive
 * primitives (`createSignal`, `createMemo`, `createEffect`, `onCleanup`,
 * `cleanup`, etc.) created inside `fn` are attached to that owner, so they
 * are disposed when the owner is disposed.
 *
 * The classic pattern: capture the current owner with `getOwner()` inside a
 * component, then re-enter it from a callback (event handler, async resolve,
 * setTimeout) so disposables created in the callback get cleaned up with the
 * component.
 *
 * @example
 * ```ts
 * function delayed<T>(ms: number, fn: () => T) {
 *   const owner = getOwner();
 *   setTimeout(() => runWithOwner(owner, fn), ms);
 * }
 * ```
 */
export declare function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
export declare function staleValues<T>(fn: () => T, set?: boolean): T;
/**
 * Core marking half of `refresh()` (the public wrapper lives in signals.ts —
 * it validates the target, marks through here, then builds the quiescence
 * promise on the resolve()/until() effect machinery). Flags the node's next
 * recompute as a quiet re-ask and schedules it; no-ops for non-derived or
 * disposed targets and for same-tick manual writes.
 */
export declare function markRefresh(node: Computed<any>): void;
