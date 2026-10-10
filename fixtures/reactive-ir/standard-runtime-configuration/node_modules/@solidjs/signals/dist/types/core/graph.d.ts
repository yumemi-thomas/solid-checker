import type { Computed, Link, Signal } from "./types.js";
export declare function unlinkSubs(link: Link): Link | null;
export declare function trimStaleDeps(el: Computed<any>): void;
export declare function clearDeps(el: Computed<unknown>): void;
export declare function unobserved(el: Computed<unknown>): void;
/**
 * Deferred dormancy for never-observed auto-dispose computeds (#3078).
 *
 * An untracked top-level read of a subscriber-less observation-lifecycle memo
 * used to call unobserved() inline at the end of read(). That kept the leak
 * closed (the compute links the memo into its deps' sub lists — without a
 * teardown point a never-observed memo is retained by its sources forever;
 * upstream alien-signals has exactly this retention), but it made reads
 * destructive: each read disposed the node, the next read revived it with a
 * full recompute in whatever ambient transition/lane context happened to be
 * current, so consecutive reads could return different answers with no write
 * in between.
 *
 * Instead, reads queue the node here and the scheduler sweeps at the top of
 * the next flush (before runHeap, so a same-tick dirtying is reclaimed
 * instead of recomputed). Reads become idempotent within a tick (the node
 * stays alive and serves its cache, uniform with observed memos) while
 * reclamation still happens within one microtask — the enqueue site arms
 * schedule(), so a flush is guaranteed even when no other work is queued.
 */
export declare const dormantNodes: Set<Computed<unknown>>;
export declare function sweepDormant(): void;
export declare function link(dep: Signal<any> | Computed<any>, sub: Computed<any>, pendingObserver?: boolean): void;
