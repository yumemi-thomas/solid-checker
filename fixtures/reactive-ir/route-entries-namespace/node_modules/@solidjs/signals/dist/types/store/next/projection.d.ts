/**
 * Store rewrite — projections (§7/§7b): a projection is a computed store.
 * The derive runs inside a computed whose recompute merges its output into
 * the projection's backing through the adoption channel (replace-mode root:
 * entity changes merge in place, the root proxy is stable for life). Children
 * wrap into the projection's own FAMILY (writes land here, never in a source
 * family), and every family node carries the projection computed as its
 * firewall — reads link the derive's status and lifecycle natively. The §6c
 * status gate in the traps makes an uninitialized async derive's seed
 * unobservable through every read surface.
 *
 * Mirrors the legacy runProjectionComputed shape (shadow runs for open
 * loading windows, handleAsync landings, commit-through-setter) on next
 * primitives; the generic draft write-traps are reused from the legacy
 * module unchanged.
 */
import { type Computed, type Refreshable } from "../../core/index.js";
import { type NoFn, type ProjectionOptions, type Store } from "../store.js";
export declare function createProjectionNext<T extends object = {}>(fn: (draft: T) => void | T | Promise<void | T> | AsyncIterable<void | T>, seed: Partial<T> | Store<NoFn<T>>, options?: ProjectionOptions): Refreshable<Store<T>>;
/** Derived writable store (legacy parity): a projection whose public setter
 * masks the recompute for the tick (core R31 — the manual write wins over a
 * same-flush dependency change). Across a hold the write is not a proposal:
 * a leaf another transaction holds as the fold's result re-runs the fold
 * under it, the write being the draft's prior state (A34 amendment, #3612;
 * core derivedWrite). */
export declare function createStoreDerivedNext<T extends object = {}>(fn: (draft: T) => void | T | Promise<void | T> | AsyncIterable<void | T>, seed: Partial<T> | Store<NoFn<T>>, options?: ProjectionOptions): [Refreshable<Store<T>>, (f: (draft: T) => T | void) => void];
export declare function runProjectionComputedNext<T extends object>(wrappedStore: Store<T>, fn: (draft: T) => void | T | Promise<void | T> | AsyncIterable<void | T>, key: string | ((item: NonNullable<any>) => any) | null, wrapCommit?: (write: () => void, value: T) => void, aroundDraftWrite?: (op: () => void) => void): Computed<void | T>;
