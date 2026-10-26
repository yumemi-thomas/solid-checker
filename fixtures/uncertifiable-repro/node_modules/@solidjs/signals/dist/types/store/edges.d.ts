/**
 * @internal The name of the store `x` belongs to — `x` a store proxy (a
 * setter's entry) or one of its property nodes (the setter notifying it) —
 * or undefined when it has none or it cannot be placed: a projection or
 * optimistic family, a store chained over another store's proxy, a root
 * named twice, or a proxy reached through a store other than the one it
 * was named as the root of.
 */
export declare function storeName(x: object | undefined): string | undefined;
