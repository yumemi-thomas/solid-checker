import type { OptimisticLane } from "./core/lanes.js";
import { Queue, type Computed, type Effect, type Owner } from "./core/index.js";
import type { Signal } from "./core/index.js";
import { type Accessor } from "./signals.js";
export interface BoundaryComputed<T> extends Computed<T> {
    _propagationMask: number;
}
type RevealSlot = CollectionQueue | RevealController;
type BoolAccessor = () => boolean;
export type RevealOrder = "sequential" | "together" | "natural";
type OrderAccessor = () => RevealOrder;
export declare class RevealController {
    _orderAccessor: OrderAccessor;
    _collapsedAccessor: BoolAccessor;
    _slots: RevealSlot[];
    _parentController?: RevealController;
    _disabled: Signal<boolean>;
    _collapsed: Signal<boolean>;
    _ready: boolean;
    _minimallyReady: boolean;
    _evaluating: boolean;
    constructor(order: OrderAccessor, collapsed: BoolAccessor);
    _forEachOwnedSlot(fn: (slot: RevealSlot) => boolean | void): boolean;
    _isReady(): boolean;
    /**
     * "Minimally ready" = this group has something visible to show under its own policy.
     * Used by an enclosing `together` group to decide when it can release.
     * - `together`: every direct slot is minimally ready.
     * - `sequential`: the first owned slot is minimally ready (frontier can advance).
     * - `natural`: any owned slot is minimally ready.
     */
    _isMinimallyReady(): boolean;
    _register(slot: RevealSlot): void;
    _unregister(slot: RevealSlot): void;
    _evaluate(disabledOverride?: boolean, collapsedOverride?: boolean): void;
}
export declare class CollectionQueue extends Queue {
    _collectionType: number;
    _sources: Set<Computed<any>>;
    _tree?: BoundaryComputed<any>;
    /** The output pass — fallback or content (createCollectionBoundary). */
    _output?: Computed<any>;
    _pending: boolean;
    _disabled: Signal<boolean>;
    _error?: Signal<unknown>;
    _collapsed: Signal<boolean>;
    _revealController?: RevealController;
    _initialized: boolean;
    /** The boundary's owner — where a `caught` report locates itself, set before the children are built (a creation-time throw arrives before `_tree`). */
    _owner?: Owner;
    /** The lane the `on` pass that queued this re-arm ran under (onNode), if
     * any: the fallback swap is display-ahead — shown through the lane. */
    _rearmLane: OptimisticLane | null;
    constructor(type: number);
    run(type: number): void;
    /** An `on` dependency notified (onNode → scheduler `pendingRearms`);
     * drained after the heap, before the verdict, under the notifying write's
     * transaction (#3540). A boundary showing content is fresh again: it
     * releases its hold now and, if anything under it is still pending, swaps
     * to its fallback. The swap is staged, so it lands with the write's frame
     * — at once when nothing else holds it, with the rest of the new page
     * when something outside the boundary does; if the pending lands first,
     * `_checkSources` clears it and no fallback is shown. An `on` that read a
     * lane (`latest()`, an optimistic write) asked for the change now:
     * `_rearmLane` shows the swap through the lane, beside the held frame.
     * Children stay alive behind the fallback. */
    _rearm(): void;
    /** Show the fallback: the swap the output pass selects on. Staged, it is
     * the frame's and lands with its commit. Re-armed from a lane pass
     * (`lane`), it is the current frame's — committed outright, as the lane's
     * view already is on screen — and shown through the lane: the output pass
     * publishes a derived override and its readers run from the lane's queue,
     * at the park, ahead of the transaction (a lane pass reads staged plain
     * writes committed, so a staged swap would be invisible to it). */
    _swap(lane: OptimisticLane | null): void;
    /** Retry the collected failures of an error boundary: recompute each
     * source that threw, so the boundary can recover. */
    _retry(): void;
    notify(node: Effect<any>, type: number, flags: number, error?: any): boolean;
    /** Is `reporter` live and routed to this boundary — under it, with no
     * collecting pending-type boundary in between (`reporterBlocksSource`'s test)? */
    _holds(reporter: Computed<any>): boolean;
    /** Has a collected source stopped counting for this boundary? A source
     * with a live affects() mark holds display state for the mark's lifetime
     * (the visual channel): the marked node carries no status of its own, so
     * the count is the liveness test. The release sweep (finalizePureQueue
     * after mark release) re-runs this check. A source born held under this
     * boundary (recompute, #3540) carries no status either: it is collected
     * while it has a staged value and no committed one, and released by the
     * commit that initializes it. */
    _settled(source: Computed<any>): boolean;
    /** The pre-verdict sweep (scheduler run(), #3540): a collecting boundary
     * whose OUTPUT is pending — its fallback read something not ready — is
     * judged here, under the transaction. That output is what an initialized
     * parent holds the frame on, and it derives from `_disabled`, not the
     * tree: the tree settling never re-runs it, only a sweep does, and the
     * commit sweep runs after the verdict the output's own read keeps parking
     * — the content waited for the fallback's flight. Judged ready here, the
     * boundary stages `_disabled` false with the frame and the output re-runs
     * in this heap: it reads the tree and drops the fallback's read (recompute
     * settles a pass's outgoing pending sources), so the verdict sees the
     * release. A boundary showing a ready fallback parks nothing and keeps the
     * commit sweep's reveal. */
    _judgeHeld(): void;
    _checkSources(): void;
}
/**
 * Lower-level primitive that backs the `<Loading>` flow control. Catches
 * pending async reads inside `fn` and renders `fallback` until they settle.
 *
 * App code should use `<Loading fallback={...}>` instead — reach for this only
 * when authoring custom boundary components.
 *
 * @param fn the tracked subtree
 * @param fallback the fallback shown while async reads in `fn` are unresolved
 * @param options `on` — a dependency list: a tracked function whose reads
 *   re-arm the boundary. Its return value is irrelevant (never compared);
 *   what matters is what it reads. Without `on`, a boundary that has shown
 *   content keeps it through a refetch (the pending holds with the
 *   transaction). With `on`, a write to anything it reads makes the boundary
 *   fresh again: it stops waiting on its current content, and if something
 *   under it is pending it shows `fallback` until the new content is ready;
 *   if nothing is pending, the notification is a no-op. The fallback lands
 *   with the same frame as the change that caused it — now, when nothing
 *   else holds that frame; together with the rest of the new page during a
 *   held navigation, not before it. If the same data is also read outside
 *   the boundary, the frame waits on it and the fallback can never be seen
 *   (DEV warns `LOADING_ON_OUTSIDE_HOLD`); the fix is structural — move the
 *   outside read under the boundary so one hold owns the data. A frame held
 *   past the content's landing by something else (the write's action, other
 *   pending data) also shows no fallback; that is a race the fallback may
 *   lose, a legitimate outcome, and not reported. A display-ahead read in
 *   `on` (`latest()`) shows the fallback now, beside the held frame; that
 *   is a capability, not the recommended shape. Optimistic writes and a
 *   source going pending notify like any other. The children are not
 *   re-created — they stay alive behind the fallback.
 *
 * @example
 * ```tsx
 * // Custom boundary component built on top of the primitive.
 * function MyLoading(props: { fallback: JSX.Element; children: JSX.Element }) {
 *   return createLoadingBoundary(
 *     () => props.children,
 *     () => props.fallback
 *   ) as unknown as JSX.Element;
 * }
 * ```
 */
export declare function createLoadingBoundary<T, U>(fn: () => T, fallback: () => U, options?: {
    on?: () => any;
}): Accessor<T | U>;
/**
 * Lower-level primitive that backs the `<Errored>` flow control. Catches
 * thrown errors inside `fn` and invokes `fallback(error, reset)` instead.
 * `error` is an accessor for the latest captured error; `reset()` recomputes
 * the failing sources so the boundary can attempt to recover.
 *
 * App code should use `<Errored fallback={...}>` instead — reach for this only
 * when authoring custom boundary components.
 *
 * @example
 * ```tsx
 * // Custom boundary that wraps the primitive and adds telemetry.
 * function TracedErrored(props: { fallback: (e: () => unknown) => JSX.Element; children: JSX.Element }) {
 *   return createErrorBoundary(
 *     () => props.children,
 *     (err, reset) => {
 *       reportError(err());
 *       return props.fallback(err);
 *     }
 *   ) as unknown as JSX.Element;
 * }
 * ```
 */
export declare function createErrorBoundary<T, U>(fn: () => T, fallback: (error: Accessor<unknown>, reset: () => void) => U): Accessor<T | U>;
/**
 * Coordinate the reveal timing of sibling loading boundaries.
 *
 * Accepts reactive accessors:
 * - `order`: `"sequential"` (default) | `"together"` | `"natural"`.
 *   - `"sequential"` — classic frontier reveal: siblings reveal in registration order
 *     as each resolves; later siblings stay hidden until earlier ones complete.
 *   - `"together"` — every direct slot stays on its fallback until the whole group
 *     is "minimally ready" (each direct slot has produced its own first visible
 *     content under its own order), then the whole group releases at once.
 *   - `"natural"` — children reveal independently (as each resolves). At the top
 *     level this is a no-op compared to not using `createRevealOrder`; the mode
 *     exists for nesting, where the group registers as a single composite slot to
 *     any enclosing `createRevealOrder`.
 * - `collapsed`: only meaningful when `order === "sequential"`. When set, tail siblings
 *   past the frontier suppress their own fallback output. Ignored under `"together"`
 *   and `"natural"` — those orders have no frontier.
 *
 * Nested `createRevealOrder` groups compose: the inner controller registers as a
 * single slot in the outer controller and is held on its fallbacks until the outer
 * releases that slot. Once released, the inner controller runs its own order locally
 * over anything still pending. There is no opt-out from an outer hold.
 *
 * "Minimally ready" is what an order considers its first visible content:
 * - `sequential` — frontier-0 is minimally ready (leaf: on resolve; nested: via its
 *   own minimal signal).
 * - `together` — every direct slot is minimally ready.
 * - `natural` — any direct slot has visible content (leaves on resolve; nested
 *   composites via their own minimal signal).
 *
 * @example
 * ```ts
 * // Primitive form of `<Reveal>` — coordinate sibling loading boundaries
 * // programmatically. App code uses the JSX `<Reveal>` component instead.
 * // Both options are accessors so they can react to state changes.
 * createRevealOrder(
 *   () => renderSiblings(),
 *   { order: () => mode(), collapsed: () => true }
 * );
 * ```
 */
export declare function createRevealOrder<T>(fn: () => T, options?: {
    order?: OrderAccessor;
    collapsed?: BoolAccessor;
}): T;
/**
 * Resolves a children value to its renderable form: unwraps zero-arg functions
 * (accessors), recursively flattens arrays, and optionally skips
 * non-rendering values (`null`, `undefined`, `true`, `false`, `""`).
 *
 * Used internally by flow components and by the renderer to walk a children
 * tree. App code rarely needs this directly — see `children()` in `solid-js`
 * for the user-facing helper that memoizes the result.
 *
 * @param children value or array of values to flatten
 * @param options
 *   - `skipNonRendered` — drop values that won't render
 *   - `doNotUnwrap` — leave function children as-is (caller will resolve)
 *
 * @example
 * ```ts
 * // Custom renderer walking a children tree manually. Most authors should
 * // use `children()` from solid-js, which memoizes the resolved value.
 * function renderChildren(value: unknown): unknown {
 *   return flatten(value, { skipNonRendered: true });
 * }
 * ```
 */
export declare function flatten(children: any, options?: {
    skipNonRendered?: boolean;
    doNotUnwrap?: boolean;
}): any;
export {};
