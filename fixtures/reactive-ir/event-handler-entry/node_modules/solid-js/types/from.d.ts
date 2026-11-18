import { type FromSource } from "@solidjs/signals";
/**
 * A source that already carries the brand (a server function's `live()`):
 * `from` returns it as is. Its transport owns its lifetime, and hydration
 * stamps (`LiveResumeFrom`) must land on the object the transport reads.
 */
export declare function isLiveSource(source: unknown): boolean;
/**
 * Brands `from`'s iterable when the source is a subscription or an Observable,
 * the same precedence `from` itself uses. With latest-value semantics each
 * yield is the whole current value: a standing answer, which the server
 * answers with its first value and closes, and the hydrated client re-runs to
 * reconnect, instead of serializing a stream that might never end. An
 * AsyncIterable stays a plain stream, which the server streams as any other.
 */
export declare function brandFrom<T>(source: FromSource<T>, iterable: AsyncIterable<T>): AsyncIterable<T>;
/**
 * Converts a callback subscription, an Observable or an AsyncIterable into an
 * AsyncIterable for a computation: `createMemo(() => from(source))`. See
 * `from` in `@solidjs/signals`; this adds the hydration brand (`brandFrom`),
 * so a subscription read during hydration reconnects once its section has
 * hydrated rather than staying at the server's value.
 */
export declare function from<T>(source: FromSource<T>): AsyncIterable<T>;
