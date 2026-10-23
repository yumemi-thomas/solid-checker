import type { Computed, Signal } from "./core/types.js";
export type Marked = Signal<any> | Computed<any>;
/** The verdict readers downstream of `node` — through derivations, stopping
 * at effects — re-derive: the mark's one notification (a probe's answer
 * changed with no value notifying anyone). Truth readers (`until`, the
 * `refresh` waiter) too, which a lagging flight parked (deferred.ts) — but
 * not through a derived store, whose leaves change only once its derive
 * re-runs: they re-derive at that write. */
export declare function repoll(node: Marked, seen?: Set<Computed<any>>, readers?: number): void;
/** One mark on `node`. The declarer repolls once its bookkeeping is done. */
export declare function mark(node: Marked): void;
/** Drops one mark from `node`: true when it was the last — the node's
 * verdict readers have re-derived. */
export declare function unmark(node: Marked): boolean;
