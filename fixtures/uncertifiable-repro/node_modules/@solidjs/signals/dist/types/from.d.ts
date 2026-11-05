/** The observer a `subscribe` call receives. */
type FromObserver<T> = {
    next(value: T): void;
    error(err: unknown): void;
    complete(): void;
};
type Teardown = (() => void) | {
    unsubscribe(): void;
} | void;
/**
 * Anything `from` accepts: a subscribe function
 * (`(next, error, complete) => unsubscribe`), an object with
 * `subscribe(observer)` (RxJS and TC39 Observables, including
 * `Symbol.observable` / `"@@observable"` interop), or an AsyncIterable.
 */
export type FromSource<T> = ((next: (value: T) => void, error: (err: unknown) => void, complete: () => void) => Teardown) | {
    subscribe(observer: FromObserver<T>): Teardown;
} | AsyncIterable<T>;
/**
 * Converts a callback subscription, an Observable or an AsyncIterable into an
 * AsyncIterable for a Solid computation. A subscription keeps only the latest
 * value, and a value the source emits during `subscribe` is served without a
 * loading frame; an AsyncIterable streams every value through.
 *
 * The source closes on supersede — the owning computation re-running — and
 * on owner disposal. Inside an async generator memo that holds for a
 * `for await` that starts before the first `await`; one that starts after it
 * has no owner to bind to, so its source closes only when the loop ends.
 * Prefer returning `from(...)` from its own memo, with awaited inputs read
 * from upstream memos and transforms done in a downstream memo.
 *
 * Iterate the result with `for await` or return it from a computation: its
 * `next()` returns an already-buffered value as a bare result, not a promise.
 *
 * @example
 * ```ts
 * const position = createMemo(() =>
 *   from<Position>((next, error) => {
 *     const id = navigator.geolocation.watchPosition(next, error);
 *     return () => navigator.geolocation.clearWatch(id);
 *   })
 * );
 * const ticks = createMemo(() => from(interval$(rate())));
 * ```
 */
export declare function from<T>(source: FromSource<T>): AsyncIterable<T>;
export {};
