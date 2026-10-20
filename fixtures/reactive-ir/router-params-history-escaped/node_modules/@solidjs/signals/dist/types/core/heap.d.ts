import type { Computed } from "./types.js";
/** The queue a node belongs to, picked from its own zombie flag. */
export declare function queueFor(n: Computed<any>): Heap;
/**
 * Schedule one subscriber to re-run on the next flush: inserted into its own
 * (zombie-flag-routed) heap with the `_min` cursor pulled down. Tracked
 * effects ride the heap too — the heap visit is their (empty) compute phase,
 * which hands the callback to the user queue once the pass has committed
 * (see GlobalQueue._update, #3291).
 */
export declare function enqueueSub(node: Computed<any>): void;
export interface Heap {
    _heap: (Computed<unknown> | undefined)[];
    _marked: boolean;
    _min: number;
    _max: number;
}
export declare function increaseHeapSize(n: number, heap: Heap): void;
export declare function insertIntoHeap(n: Computed<any>, heap: Heap): void;
export declare function insertIntoHeapHeight(n: Computed<unknown>, heap: Heap): void;
export declare function deleteFromHeap(n: Computed<unknown>, heap: Heap): void;
export declare function markHeap(heap: Heap): void;
export declare function markNode(el: Computed<unknown>, newState?: number): void;
export declare function runHeap(heap: Heap, recompute: (el: Computed<unknown>) => void): void;
