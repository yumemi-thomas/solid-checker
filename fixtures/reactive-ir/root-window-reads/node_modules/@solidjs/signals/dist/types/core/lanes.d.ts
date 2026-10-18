import { hasActiveOverride } from "./core.js";
export { hasActiveOverride };
import { type QueueCallback, type Transition } from "./scheduler.js";
import type { Computed, Signal } from "./types.js";
/**
 * OptimisticLane represents the context for a single optimistic write.
 * Each optimistic signal creates its own lane. Lanes merge when their
 * dependency graphs overlap.
 */
export interface OptimisticLane {
    _source: Signal<any>;
    _pendingAsync: Set<Computed<any>>;
    _effectQueues: [QueueCallback[], QueueCallback[]];
    _mergedInto: OptimisticLane | null;
    _transition: Transition | null;
    _parentLane: OptimisticLane | null;
}
export declare const signalLanes: WeakMap<Signal<any>, OptimisticLane>;
export declare const activeLanes: Set<OptimisticLane>;
/**
 * Get an existing lane for a signal or create a new one.
 * Reuses lane for multiple writes to the same signal.
 */
export declare function getOrCreateLane(signal: Signal<any>): OptimisticLane;
/**
 * Union-find: find the root lane.
 */
export declare function findLane(lane: OptimisticLane): OptimisticLane;
/**
 * Is the lane held? `_pendingAsync` records the async the lane OWNS (derived
 * under it); a transaction's reporter map records the async a render effect
 * OBSERVED pending with no boundary taking it (INV-3, the one registration
 * site). A hold needs both — the same rule the transaction itself uses, so a
 * memo nobody renders, or one a fallback-showing boundary caught, cannot tear
 * a frame and holds nothing (#3289). An orphan lane has no observation record
 * and never holds.
 *
 * The observation is looked up per NODE, in whichever live transaction
 * recorded it — not in this lane's transaction. Lanes merge across
 * transactions (#2912: ownership never travels through lanes), so after a
 * merge the root's transaction holds the observations of only one member;
 * the async the other member's transaction observed must hold the merged
 * reveal just the same (A15 for lanes, #3335).
 */
export declare function laneHeld(lane: OptimisticLane): boolean;
/**
 * Lanes mirror transitions (#3460): a render effect OFF a HELD lane that reads
 * a value the lane is revealing — an override, a `latest()` shadow — sees the
 * committed value, exactly as a stale reader of a held transaction does
 * (A15 reveal corollary): it publishes now, with the frame that is on screen
 * (the lane defers its own readers' runs, so the committed value is what is
 * visible), entangles nothing — a sync write is never held by a lane — and
 * re-derives at the release. The release re-run rides the lane's own render
 * queue, which runs when the lane reveals (runLaneEffects) or its transaction
 * commits (cleanupCompletedLanes). A reader ON the lane computes the lane's
 * reveal and takes the value as before.
 *
 * OFF the lane is provenance, not membership — the transaction mirror
 * exactly: a stale reader of a transaction is a pass that runs outside it. A
 * pass under the lane's own transaction is the lane's work — its write (lane
 * posture), or the landing of its async, which re-enters the transaction
 * (#3334) and runs a member with no ambient lane. Read as an outsider, that
 * pass published the committed view and queued a replay that revealed the
 * override beside its unready derivation at the release (`1:0` for an
 * optimistic frame that never became ready; #3479 review). Membership is the
 * wrong test the other way: a member re-run by a sibling's sync write is a
 * mainline pass and shows the committed view.
 */
export declare function readsHeldCommitted(owner: Computed<any>, c: Computed<any>): boolean;
/** The ownership relation for a lane hold (core `ownsHold`, §6 ruling 2): the
 * running pass owns `lane`'s hold if it runs under the transition that owns
 * the lane (the node's, resolved through override ownership and merges) or
 * inside the lane itself. */
export declare function ownsLane(lane: OptimisticLane, owner: Computed<any>): boolean;
/**
 * Merge two lanes when their dependency graphs overlap.
 */
export declare function mergeLanes(lane1: OptimisticLane, lane2: OptimisticLane): OptimisticLane;
/**
 * Resolve a node's lane: follow union-find chain, verify active, clear if stale.
 */
export declare function resolveLane(el: Signal<any> | Computed<any>): OptimisticLane | undefined;
export declare function resolveTransition(el: Signal<any> | Computed<any>): Transition | null | undefined;
/**
 * Assign or merge a lane onto a node. At convergence points (node already has
 * a different active lane), merge unless the node has an active override.
 */
export declare function assignOrMergeLane(el: Signal<any> | Computed<any>, sourceLane: OptimisticLane): void;
