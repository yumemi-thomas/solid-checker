import { type Transition } from "./scheduler.js";
import type { Computed, NodeOptions, Owner } from "./types.js";
export interface Effect<T> extends Computed<T>, Owner {
    _effectFn: (val: T, prev: T | undefined) => void | (() => void);
    _errorFn?: (err: unknown, cleanup: () => void) => void;
    _modified: boolean;
    _prevValue: T | undefined;
    _type: number;
    _boundRunEffect?: (type: number) => void;
    /** The transaction whose staged view produced `_value` (null = committed
     * view). Effects have one value slot and do not entangle transactions, so
     * a second transaction recomputing the same effect overwrites a value the
     * first one still owes a run for; see the contested-effect arm of recompute
     * (#3322). */
    _valueTransition: Transition | null;
}
/**
 * Effects are the leaf nodes of our reactive graph. When their sources change, they are
 * automatically added to the queue of effects to re-execute, which will cause them to fetch their
 * sources and recompute
 */
export declare function effect<T>(compute: (prev: T | undefined) => T, effect: (val: T, prev: T | undefined) => void | (() => void), error?: (err: unknown, cleanup: () => void) => void | (() => void), options?: NodeOptions<any> & {
    user?: boolean;
    defer?: boolean;
    schedule?: boolean;
}): void;
export interface TrackedEffect extends Computed<void> {
    _modified: boolean;
    _type: number;
    _run: () => void;
}
/**
 * Internal tracked effect - bypasses heap, goes directly to effect queue.
 * Runs as a leaf owner: child primitives and onCleanup are forbidden (__DEV__ throws).
 * Uses stale reads.
 */
export declare function trackedEffect(fn: () => void | (() => void), options?: NodeOptions<any>): void;
