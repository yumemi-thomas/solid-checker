import { hasActiveOverride, visibleOverride } from "../../core/core.js";
import type { Computed, Signal } from "../../core/types.js";
import { type StoreNextFamily, type StoreNextTarget } from "./target.js";
export declare function wrapNext<T extends Record<PropertyKey, any>>(value: T, parent?: StoreNextTarget | null, parentKey?: PropertyKey | null, fam?: StoreNextFamily | null): T;
/** Unwrap our own proxies to their current backing; leave everything else. */
export declare function unwrapValue(v: any): any;
export declare function getNode(target: StoreNextTarget, key: PropertyKey, current: any, accKnown?: -1 | 0 | 1): Signal<any>;
/** Record a store proxy's declared name for attribution labels (observe tiers). */
export declare function nameStore(proxy: any, name: string | undefined): void;
export declare function getHasNode(target: StoreNextTarget, key: PropertyKey, present: boolean): Signal<boolean>;
export declare function getKeySetNode(target: StoreNextTarget): Signal<number>;
/** Deep-witness bump: any value/shape change on a record with a live deep()
 * subscriber notifies it. One null check when unused. */
export declare function bumpDeep(t: StoreNextTarget): void;
/** Downgrade a prototype-overlay pending backing to the clone path: builds
 * the real container (committed + overlay writes − deletes) that fold will
 * SWAP in as the committed backing, exactly as if the draft had started on
 * the clone path. Consumers that need a complete container (reconcile's
 * diff walks, drafts escaping into other storage) call this, and so does the
 * commit itself when the fold changed the key set (overlayRebuilds). Returns
 * the pending backing (the clone, or the pb as-is when not an overlay). */
export declare function materializePB(target: StoreNextTarget): Record<PropertyKey, any>;
/** Resolve the held committed view (#3074): answers the masked old backing
 * while the hold is live, and lazily clears a hold whose transition has
 * committed (transitions merge — resolve through currentTransition, same as
 * foldHeld's node stamps). */
export declare function heldMaskView(t: StoreNextTarget): Record<PropertyKey, any> | null;
/**
 * Adoption (2026-08-16c): the incoming object becomes the committed backing
 * IMMEDIATELY — reconcile is eagerly visible to every reader (shipped
 * contract; only its notifications batch), unlike setter writes which stay
 * pending until flush. Ownership resets (incoming is unowned/user data). Any
 * staged draft clone folds into the diff and is discarded — next is the
 * authoritative base (R21/R32).
 */
export declare function adoptPB(target: StoreNextTarget, incoming: Record<PropertyKey, any>, eager?: boolean): void;
/** Parked truth-staged pending backings (#3164 fold): a tentative draft that
 * opens while a folded landing's backing is live moves the staged container
 * here (see ensurePB); the tentative discard in notifyOptimisticWrites
 * restores it in place of the usual null. */
export declare const stagedTruthPB: WeakMap<StoreNextTarget, Record<PropertyKey, any>>;
/** Same logical slot: both values resolve to one (re-pointed) child target —
 * adoption preserved identity, so the slot did not change (R9). */
export declare function targetsEqual(ov: any, nv: any): boolean;
export declare function arrayStructureChanged(old: any[], neu: any[]): boolean;
export declare function membershipChanged(old: Record<PropertyKey, any>, neu: Record<PropertyKey, any>): boolean;
/**
 * The fold diff walks SUBSCRIPTION KEYS ONLY (legacy parity: `for key in
 * nodes`): nodes exist exactly where something tracked, so unobserved data
 * costs nothing here regardless of object size. Accessor safety rides the
 * sticky `t.a` flag — a node's key was necessarily read, so the get trap has
 * already seen whether it is an accessor.
 */
/** One node's fold notification (shared by notifyFold's walk and the fused
 * adoption walk): accessor-aware compare + equality/identity-gated setSignal. */
export declare function notifyKeyDiff(node: Signal<any>, key: PropertyKey, old: Record<PropertyKey, any>, neu: Record<PropertyKey, any>, probe?: boolean): void;
/** Accessor-flag probe for the fused walk's early-continue (accessor keys
 * can never identity-skip: their VALUE is the descriptor's product). */
export declare function hasAccessorFlag(node: Signal<any>): boolean;
/** Fused-walk per-key notification with values already in hand: the caller
 * fetched both sides and handled the identity skip; this applies the
 * accessor branch (cached flag only — reconcile channel) or the plain
 * equality/identity-gated write. */
export declare function notifyKeyValue(node: Signal<any>, key: PropertyKey, ov: any, nv: any, old: Record<PropertyKey, any>, neu: Record<PropertyKey, any>): void;
/** Presence + membership halves of a fold notification (shared tail). */
export declare function notifyFoldTail(t: StoreNextTarget, old: Record<PropertyKey, any>, neu: Record<PropertyKey, any>): void;
export declare function notifyFold(t: StoreNextTarget, old: Record<PropertyKey, any>, neu: Record<PropertyKey, any>): void;
/** Authoritative-write wrapper exported for the optimistic module: sets the
 * scheduler's projectionWriteActive through THIS module's binding (proven to
 * share the instance core reads — cross-module live-binding writes from other
 * store modules were observed not to propagate under the test transform). */
export declare function runAuthoritative<T>(fn: () => T): T;
export { hasActiveOverride, visibleOverride };
/** The reading computation is until()'s authoritative-view predicate — same
 * source of truth as core read()'s A17 carve-out (`context`, which persists
 * under untrack). optimisticView()'s composition gate consults exactly this:
 * write-side machinery (patch emission, tentative re-application) must keep
 * composing even when it runs inside an authoritative-write bracket. */
export declare function authoritativeRead(): boolean;
/** Serve-side authoritative gate: until()'s predicate PLUS truth authors —
 * the projection derive's draft (wrapDraft trap brackets, runAuthoritative;
 * the same posture pair ensurePB classifies drafts by). A source computing
 * the next truth must never read its callers' tentative overlays: a derive
 * continuation's `store.push` computing its index from an action's
 * optimistic row landed truth in the wrong slot and corrupted committed
 * state (#3108). Trap-level overlay serves gate on this so values, length,
 * membership, and keys leave the authoritative view together. */
export declare function authoritativeServe(): boolean;
/** The override a composed READER view (keys, descriptors, snapshot/deep,
 * optimisticView) takes from an armed node whose override is active — the
 * override itself, unless the node's own source superseded it (#3331,
 * CONFIG_OVERRIDE_SUPERSEDED): then the reader-aware selection `serve`
 * makes through nodeValue, so the composed view agrees with what `get`,
 * `in` and `length` serve the same reader (a deriving pass: the staged
 * truth; a lane pass or a context-free read: the override, A18). Composing
 * the raw override left Object.keys / snapshot() / deep() one row behind the
 * traps at a landing whose shape differed from the optimistic frame (F5
 * parity cases). Draft and authoritative callers never reach here — the
 * writer composes on hasActiveOverride, truth authors on the backing. */
export declare function readerOverride(node: Signal<any>, committed: any): any;
export type SetStoreNextFunction<T> = (fn: (draft: T) => T | void) => void;
/** The derived store's setter (CS-R31): within a synchronous frame the manual
 * write wins — the projection's recompute is masked for the tick. Across a
 * hold the write is not a proposal: a leaf the fold staged under another
 * transaction re-runs the fold under it, the write being the draft's prior
 * state (#3612). Decided from the leaf notifications, so the mask lands
 * after them (in the `finally`: a throwing setter's writes before the throw
 * were notified, and are masked as before). */
export declare function derivedStoreWrite<T>(node: Computed<unknown>, proxy: T, fn: (draft: T) => T | void): void;
export declare function storeSetterNext<T>(proxy: T, fn: (draft: T) => T | void, guard?: boolean): void;
export declare function createStoreNext<T extends Record<PropertyKey, any>>(initialValue: T, shallow?: boolean): [T, SetStoreNextFunction<T>];
/** True when `proxy` is a SHALLOW store (children served verbatim, slots
 * replaced by reference — #2932). The list driver uses this to choose the
 * slot-patch channel (collected row bodies) over per-record registration. */
export declare function storeIsShallow(proxy: any): boolean;
/** True when `proxy` belongs to a projection/optimistic FAMILY. The list
 * driver must DECLINE family arrays (external audit finding): family
 * structural changes never emit row/slot ops (the setter channel is
 * fam-gated; optimistic writes ride node overrides), and the proxy identity
 * is stable so the each-watch cannot catch the change either — an engaged
 * list would freeze on optimistic/projection structural updates. Record-
 * level family patches are unaffected (they have their own emission). */
export declare function storeHasFamily(proxy: any): boolean;
/** True when `proxy` belongs to an OPTIMISTIC family specifically. The list
 * driver declines these (audit finding, narrowed): optimistic user writes
 * ride node-level overrides — they never enter the reconcile walk, so no
 * row/slot ops are emitted and an engaged list would freeze on optimistic
 * structural changes. PROJECTION (non-optimistic) families are drivable:
 * their recomputes go through the reconcile walk, whose emissions are
 * transition-stamped in the apply queue like any other (equivalence-matrix
 * gated). Re-admitting optimistic families requires a lane-timed structural
 * emission mirroring emitPatchOptimistic, plus revert resync. */
export declare function storeHasOptimisticFamily(proxy: any): boolean;
/** Tracking deep snapshot (`deep()` for next targets): subscribes to the
 * key-set and deep-witness node at every reachable level, then returns the
 * plain view. Shared references and cycles handled via the visited set. */
export declare function deepNext<T>(value: T): T;
/**
 * Snapshot with per-object registration resolution (RUL-12 DAG ruling): every
 * reachable wrappable resolves through its target's CURRENT backing, so
 * privatized subtrees are seen through any parent path. Identity-preserving:
 * a subtree with no substitutions below returns its own object (zero copy for
 * settled, never-diverged graphs).
 */
export declare function snapshotNext<T>(value: T): T;
