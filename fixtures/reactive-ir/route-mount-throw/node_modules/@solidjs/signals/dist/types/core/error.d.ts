/**
 * Thrown by a tracked read whose value is currently pending (an async memo /
 * `createSignal(asyncFn)` / projection / store derivation that hasn't settled
 * yet). Surfacing through the reactive graph is what suspends the consumer
 * scope — the nearest enclosing `<Loading>` boundary catches the throw and
 * renders its fallback until the source resolves.
 *
 * App code rarely catches this directly; `<Loading>` is the canonical
 * handler. The error type is exposed for advanced cases — e.g. interop layers
 * that bridge Solid's pending-throw protocol to a different async strategy,
 * or tests that want to assert on the suspension shape.
 *
 * @example
 * ```ts
 * // Advanced: distinguish "not ready yet" from a real error in custom
 * // boundary plumbing. App code should rely on `<Loading>` / `<Errored>`.
 * try {
 *   const value = readReactiveSource();
 * } catch (err) {
 *   if (err instanceof NotReadyError) throw err; // re-throw to suspend
 *   reportError(err);
 * }
 * ```
 */
export declare class NotReadyError extends Error {
    /**
     * Tags a visibility-only notification on the affects() boundary channel:
     * boundaries update display state from it, but the root queue never
     * registers a reporter — marks are invisible to completion accounting by
     * construction.
     */
    _markVisual?: boolean;
    source: any;
    constructor(source: any);
}
export declare class StatusError extends Error {
    source: any;
    constructor(source: any, original: any);
}
/** Return the user's error from an internal status wrapper. */
export declare function unwrapStatusError(error: unknown): unknown;
/**
 * Rejection value of `until(fn, { timeout })` when the predicate does not turn
 * truthy within the window. Inside an `action()`, the rejection is thrown back
 * in at the `yield` point — catchable there, or the action fails and its
 * optimistic state reverts.
 */
export declare class TimeoutError extends Error {
    constructor(message?: string);
}
export declare class NoOwnerError extends Error {
    constructor();
}
export declare class ContextNotFoundError extends Error {
    constructor();
}
