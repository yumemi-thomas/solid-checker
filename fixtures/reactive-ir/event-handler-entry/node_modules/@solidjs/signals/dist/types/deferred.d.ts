import { type MemoOptions, type SourceAccessor } from "./signals.js";
/**
 * Creates an async memo that may lag, but never leads. Before its first
 * answer it is an ordinary async memo: readers suspend to the nearest
 * `<Loading>`. After it, a refetch holds nothing: readers keep the last
 * committed value until the new one lands, and the rest of the screen moves
 * on without it. `isPending` reports the refetch, on the node and on what
 * derives from it. Use it for independent widgets (dashboards, feeds,
 * search results) where the previous answer with a "refreshing" indicator is
 * better than waiting with everything else.
 *
 * - It never shows a value ahead of a held change: a refetch that read a
 *   held value lands with that hold.
 * - A rejected refetch errors like any memo; the stale value never masks it.
 * - `refresh(d)` is a quiet re-ask, and `await refresh(d)` and `until`
 *   wait for the landing rather than the stale value.
 * - Combine with `loadingValue` for a node that never suspends at all.
 *
 * @example
 * ```tsx
 * const rows = createDeferred(() => fetchRows(store.period));
 *
 * <section class={{ stale: isPending(() => rows()) }}>
 *   <For each={rows()}>{row => <Row row={row} />}</For>
 * </section>
 * ```
 */
export declare function createDeferred<T>(fn: (prev?: T) => T | PromiseLike<T> | AsyncIterable<T>, options?: MemoOptions<T>): SourceAccessor<T>;
