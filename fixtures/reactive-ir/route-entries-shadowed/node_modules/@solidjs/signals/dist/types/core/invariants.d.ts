import type { OptimisticLane } from "./lanes.js";
import type { Computed, Signal } from "./types.js";
/**
 * Test-mode invariant checks for the async/transition/lane machinery.
 * Catalog and rationale: packages/signals/docs/INTERNALS-ASYNC-STATE.md.
 *
 * These are implementation self-consistency checks, not semantic rules: a
 * violation means the reactive system contradicted itself.
 *
 * They are gated on `__TEST__` (not just `__DEV__`): the per-write Set
 * tracking and per-flush quiescence sweep are too expensive for shipped dev
 * builds and for benchmarks (they showed up as a 5-21% hit across the
 * CodSpeed suite when they ran under `__DEV__`). Call sites stay `__DEV__`
 * guarded so production tree-shakes the calls; each entry point here
 * early-returns unless `__TEST__` is set, so dev builds pay only a no-op
 * call. The test suite (vitest run) defines `__TEST__: true`; benchmark mode
 * defines `__TEST__: false`.
 */
type AnyNode = Signal<any> | Computed<any>;
/** Wired by core.ts at module init to avoid import cycles. */
export declare const InvariantHooks: {
    pendingProbeActive: (() => boolean) | null;
    /** Fresh oracle for what an isPending companion SHOULD read right now. */
    computePendingState: ((node: AnyNode) => boolean) | null;
};
export declare function devTrackHeldPending(node: AnyNode): void;
export declare function devTrackCompanionOwner(node: AnyNode): void;
/** #3503: a firewall child the store released (unobserved sweep) leaves the
 * registry with it — the runtime drops it from `_companionChildren` so the
 * projection stops retaining it; the test-only set must not either. */
export declare function devUntrackCompanionOwner(node: AnyNode): void;
export declare function devTrackOptimistic(node: AnyNode): void;
export declare function devTrackAffects(node: AnyNode): void;
/**
 * Open/close the sanctioned registration window. Call sites are `__DEV__`
 * guarded (no prod cost); the code inside the window must not throw.
 */
export declare function beginAsyncReporterWrites(): void;
export declare function endAsyncReporterWrites(): void;
export declare function createAsyncReporters(): Map<Computed<any>, Set<Computed<any>>>;
/**
 * INV-2: a node with an *active* override must be registered for reversion in
 * the queue's or a transition's `_optimisticNodes`. An unregistered active
 * override would survive transition completion forever. Runs at the end of
 * every flush (not just quiescence — the invariant holds mid-transition).
 * (There is no revert-target requirement: authoritative values commit
 * silently into `_value` under the override mask — A17 — so reverting is
 * just dropping the override.)
 */
export declare function devCheckActiveOverrides(isRegisteredForRevert: (node: AnyNode) => boolean): void;
/** INV-1: an isPending() probe must never leak past its own call. */
export declare function devCheckFlushStart(): void;
/** INV-5: a merged lane's work moved to its root on merge and must stay empty. */
export declare function devCheckMergedLaneEmpty(lane: OptimisticLane): void;
export declare function devCensusCompanions(isQueuedForCommit?: (node: AnyNode) => boolean): void;
/**
 * Quiescence checks. Run only when the system is fully drained: nothing
 * scheduled, no active/stashed transitions, no live lanes. At that point no
 * transition-scoped state may survive, and the lazily-created companions must
 * agree with a fresh computation of their owner's state.
 */
export declare function devCheckQuiescent(isQueuedForCommit: (node: AnyNode) => boolean): void;
export {};
