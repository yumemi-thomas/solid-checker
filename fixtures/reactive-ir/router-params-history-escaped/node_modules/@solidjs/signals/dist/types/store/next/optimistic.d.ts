import { type Refreshable } from "../../core/index.js";
import { type Transition } from "../../core/scheduler.js";
import { type NoFn, type ProjectionOptions, type Store, type StoreOptions, type StoreSetter } from "../store.js";
import type { StoreNextTarget } from "./target.js";
/** #3164 fold: a stamped truth is HELD (masked from ordinary readers until
 * the reveal) only while its transition is live AND retaining optimism —
 * overrides are what make partial-coverage composition a tear. A plain
 * async transition carries no overrides, so downstream computes must see
 * staged values to converge (normal speculation). Resolves merges first:
 * merge unions optimistic nodes/stores into the target. */
export declare function transitionHoldsOptimism(transition: Transition): boolean;
export declare function createOptimisticStoreNext<T extends object = {}>(initialValue: NoFn<T> | Store<NoFn<T>>, options?: StoreOptions): [get: Store<T>, set: StoreSetter<T>];
export declare function createOptimisticStoreNext<T extends object = {}>(fn: (draft: T) => void | T | Promise<void | T> | AsyncIterable<void | T>, seed: Partial<T> | Store<NoFn<T>>, options?: ProjectionOptions): [get: Refreshable<Store<T>>, set: StoreSetter<T>];
/** Diff the draft against the current OPTIMISTIC VIEW (committed + active
 * overrides — the same view the draft was seeded from) and emit engine writes
 * for exactly the changed keys. Visible-view diffing keeps no-op writes from
 * entangling lanes (RUL-10 / opt R38). */
export declare function notifyOptimisticWrites(t: StoreNextTarget, pb: Record<PropertyKey, any>): void;
/** Optimistic-view composition for snapshot/deep (O1: snapshot is the CURRENT
 * view, lane values included; a fresh copy per call during pending windows —
 * RUL-12). Returns `src` untouched when no override is active on `t`.
 * Authoritative-view reads (until()'s predicate) skip composition entirely:
 * the predicate observes authoritative truth, never the caller's tentative
 * overlay. (Write-side emission callers never run under such a compute.) */
export declare function optimisticView(t: StoreNextTarget, src: Record<PropertyKey, any>, draft?: boolean): Record<PropertyKey, any>;
