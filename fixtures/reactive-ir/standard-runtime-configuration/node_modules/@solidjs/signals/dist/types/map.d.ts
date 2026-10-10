import { type Accessor } from "./signals.js";
export type Maybe<T> = T | void | null | undefined | false;
/**
 * Reactively maps an array, reusing the previously-mapped value for unchanged
 * items.
 *
 * The callback shape follows the keying mode:
 * - default / `keyed: true` receives `(item, index)` where `item` is the raw
 *   row value and `index` is an accessor.
 * - `keyed: false` receives `(item, index)` where `item` is an accessor and
 *   `index` is a stable number.
 * - `keyed: item => key` receives accessors for both arguments.
 *
 * This is the underlying helper that powers `<For>`. App code should use
 * `<For>` directly; reach for `mapArray` when implementing custom list
 * components.
 *
 * - `options.keyed` — `true` (default for primitives) compares by identity;
 *   `false` falls back to index-only mapping; pass a function `(item) => key`
 *   for stable identity by extracted key.
 * - `options.fallback` — accessor returning a value to show when the input is
 *   empty.
 *
 * @example
 * ```ts
 * const view = mapArray(
 *   items,
 *   (item, index) => `${index()}: ${item.label}`,
 *   { fallback: () => "no items" }
 * );
 * ```
 *
 * @description https://docs.solidjs.com/reference/reactive-utilities/map-array
 */
export declare function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Item, index: Accessor<number>) => MappedItem, options?: {
    keyed?: true;
    fallback?: Accessor<any>;
    name?: string;
}): Accessor<MappedItem[]>;
export declare function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Accessor<Item>, index: number) => MappedItem, options: {
    keyed: false;
    fallback?: Accessor<any>;
    name?: string;
}): Accessor<MappedItem[]>;
export declare function mapArray<Item, MappedItem>(list: Accessor<Maybe<readonly Item[]>>, map: (value: Accessor<Item>, index: Accessor<number>) => MappedItem, options: {
    keyed: (item: Item) => any;
    fallback?: Accessor<any>;
    name?: string;
}): Accessor<MappedItem[]>;
/** @internal */
export declare function __smallMoveHits(): number;
/**
 * Reactively renders a callback `count` times, reusing previously-rendered
 * entries when only the count changes. Underlying helper for `<Repeat>`.
 *
 * - `options.from` — start index (default `0`); useful for offset/windowed
 *   rendering.
 * - `options.fallback` — accessor returning a value to show when count is `0`.
 *
 * @example
 * ```ts
 * const view = repeat(count, i => `Item ${i}`, { fallback: () => "empty" });
 * ```
 *
 * @description https://docs.solidjs.com/reference/reactive-utilities/repeat
 */
export declare function repeat(count: Accessor<number>, map: (index: number) => any, options?: {
    from?: Accessor<number | undefined>;
    fallback?: Accessor<any>;
    name?: string;
}): Accessor<any[]>;
