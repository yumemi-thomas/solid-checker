export type { Store, StoreReturn, ProjectionStoreReturn, StoreSetter, StoreNode, StoreOptions, ProjectionOptions, NotWrappable, SolidStore } from "./store.js";
export type { Merge, Omit } from "./utils.js";
export { isWrappable, $TRACK, $PROXY, $TARGET, $RECORD } from "./store.js";
export { mergeSources, mergeView, viewOf, omitView, sourceKeys, sourceHas, sourceGet, hasStaticKeys, isStatic, resolvedTable, OmitView, MergeView, SOURCE_PLAIN, SOURCE_OMIT, SOURCE_PROXY, SOURCE_MEMO, SOURCE_MERGE, sourceOwners } from "./utils.js";
export type { SourceKind } from "./utils.js";
import type { NoFn, ProjectionOptions, Store, StoreOptions, StoreSetter } from "./store.js";
import type { Refreshable } from "../core/index.js";
export { createProjectionNext as createProjection } from "./next/projection.js";
export { storeIsShallow, storeHasFamily, storeHasOptimisticFamily } from "./next/store.js";
export { createOptimisticStoreNext as createOptimisticStore } from "./next/optimistic.js";
/** Public createStore: plain form `(initialValue, options?)` and derived writable
 * form `(fn, seed, options?)`. */
export declare function createStore<T extends object = {}>(initialValue: NoFn<T> | Store<NoFn<T>>, options?: StoreOptions): [get: Store<T>, set: StoreSetter<T>];
export declare function createStore<T extends object = {}>(fn: (draft: T) => void | T | Promise<void | T> | AsyncIterable<void | T>, seed: Partial<T> | Store<NoFn<T>>, options?: ProjectionOptions): [get: Refreshable<Store<T>>, set: StoreSetter<T>];
export declare function reconcile<T extends U, U>(value: T, key?: string | ((item: NonNullable<any>) => any) | null): (state: U) => T;
export declare function snapshot<T>(value: T): T;
export declare function deep<T>(value: T): T;
export { storePath } from "./storePath.js";
export type { PathSetter, Part, StorePathRange, ArrayFilterFn, CustomPartial } from "./storePath.js";
export { merge, omit } from "./utils.js";
