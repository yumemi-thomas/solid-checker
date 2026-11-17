import { type Signal } from "../core/index.js";
import type { Refreshable } from "../core/index.js";
/** A reactive view of a store's value. Update it through the paired `StoreSetter`. */
export type Store<T> = T;
/**
 * A store setter. The callback receives a writable **draft** of the store.
 *
 * - **Mutate in place (canonical):** `s.foo = 1`, `s.list.push(x)`,
 *   `s.list.splice(i, 1)`. This is the default form for most updates.
 * - **Return a new value:** for shapes where mutation is awkward, most
 *   commonly removing items (`s => s.list.filter(...)`). Arrays are replaced
 *   by index (length adjusted); objects are shallow-diffed at the top level
 *   (keys present in the returned value are written, missing keys deleted).
 *
 * The setter does **not** perform keyed reconciliation. If you need surviving
 * items to keep their store identity across full-array replacement, use the
 * projection form — `createStore(fn, seed, { key })` or
 * `createProjection(fn, seed, { key })` — whose derive function reconciles
 * its return by `options.key`.
 */
export type StoreSetter<T> = (fn: (state: T) => T | void) => void;
/** Tuple returned by the plain `createStore(initialValue, options?)` form. */
export type StoreReturn<T> = [get: Store<T>, set: StoreSetter<T>];
/** Tuple returned by the derived `createStore(fn, seed, options?)` form. */
export type ProjectionStoreReturn<T> = [get: Refreshable<Store<T>>, set: StoreSetter<T>];
/** Options shared by all store primitives. */
export interface StoreOptions {
    /**
     * Debug name (dev and observe builds). Property nodes are labelled
     * `<name>.<key>` in attribution output (`todos.title`); a derived store's
     * projection node carries the name itself.
     */
    name?: string;
    /** Single-layer store: root keys reactive, values raw records replaced by reference */
    shallow?: boolean;
}
/**
 * Options for derived/projected stores created with
 * `createStore(fn, seed, options?)`, `createProjection(fn, seed, options?)`,
 * or `createOptimisticStore(fn, seed, options?)`.
 */
export interface ProjectionOptions extends StoreOptions {
    /** Key property name or function for reconciliation identity; `null` merges positionally */
    key?: string | ((item: NonNullable<any>) => any) | null;
    /**
     * Treat the seed as commit #0: the store is born committed with the seed's
     * contents, shown until the derive's first real answer lands. While that
     * first answer is in flight, reads serve the seed everywhere — nothing
     * suspends to a `<Loading>` boundary, no transition is held, and
     * `isPending` stays false (the seed answers by declaration; first-load
     * affordances belong to the data, e.g. a `skeleton: true` field in the
     * seed). Once the first answer lands (reconciled into the seed), refetches
     * use normal pending semantics with `isPending` true.
     *
     * The store equivalent of `MemoOptions.loadingValue`; the seed already
     * carries the placeholder shape, so this is just the opt-in.
     */
    seedLoadingValue?: boolean;
}
export type NoFn<T> = T extends Function ? never : T;
type DataNode = Signal<any>;
type DataNodes = Record<PropertyKey, DataNode>;
/**
 * Brand symbols used internally by the store proxy / projection plumbing.
 * Cross-package wiring; not part of the user-facing API.
 *
 * @internal
 */
export declare const $TRACK: unique symbol, $TARGET: unique symbol, $PROXY: unique symbol, $RECORD: unique symbol, $DELETED: unique symbol, $AFFECTS: unique symbol;
export declare const STORE_VALUE = "v", STORE_NODE = "n", STORE_HAS = "h", STORE_PARENT = "u", STORE_DESC = "d", STORE_SHALLOW = "s";
/** Structural view of a store target as shared machinery sees it (the real
 * shape is `StoreNextTarget` in ./next/target.ts). */
export type StoreNode = {
    [$PROXY]: any;
    [STORE_VALUE]: Record<PropertyKey, any>;
    [STORE_NODE]?: DataNodes;
    [STORE_HAS]?: DataNodes;
    [STORE_PARENT]?: StoreNode;
    [STORE_SHALLOW]?: boolean;
    [STORE_DESC]?: boolean;
};
export declare namespace SolidStore {
    interface Unwrappable {
    }
}
export type NotWrappable = string | number | bigint | symbol | boolean | Function | null | undefined | SolidStore.Unwrappable[keyof SolidStore.Unwrappable];
/**
 * Marks a value as raw: no store will ever wrap it — every store presents it
 * as-is, tracked by reference at whatever slot holds it and updated by
 * replacement. Useful for class instances and external objects (editors,
 * scene graphs, Maps) and for record-shaped data updated wholesale. Sticky
 * for the value's lifetime.
 */
export declare let rawValuesUsed: boolean;
export declare function isRawValue(value: any): boolean;
export declare function markRaw<T>(value: T): T;
export declare function markRawOne(v: any): void;
export declare function markRawIngest(container: any): void;
export declare function isWrappable<T>(obj: T | NotWrappable): obj is T;
export declare function setWriteOverride(value: boolean): void;
export declare function getWriteOverride(): boolean;
export declare function ownEnumerableKeys(o: object): (string | symbol)[];
/**
 * Scope inheritance for late-created nodes: every live mark whose identity
 * scope contains the owning record's raw — and, for keyed marks, whose key
 * is this property — gets counted on the new node. Inherited marks live
 * exactly as long as the scope's carrier — the release hook below drops
 * them with the entry.
 */
export declare function inheritAffectsMarks(node: DataNode, raw: object, property: PropertyKey): void;
/** Next-store node factory for affects carriers/slots: injected by the
 * rewrite module (next targets alias the legacy field names, so everything
 * here EXCEPT node creation works on them structurally). */
export declare let nextAffectsNodeResolver: ((target: any, key: PropertyKey) => DataNode) | null;
export declare function setNextAffectsNodeResolver(fn: (target: any, key: PropertyKey) => DataNode): void;
/** Next-store optimistic view for the declaration walk (optimistic rows
 * pushed before the declaration are in motion too — legacy reads its write
 * overlays; next composes armed-node overrides). */
export declare let nextOptimisticViewResolver: ((target: any, raw: any) => any) | null;
export declare function setNextOptimisticViewResolver(fn: (target: any, raw: any) => any): void;
/** @internal birth inheritance for nodes created inside a live mark window —
 * exported for the rewrite's node factories. */
export declare function affectsScopesLive(): boolean;
/**
 * Witness live mark coverage of a record into the active isPending() probe.
 * Tracked reads don't need this — they go through real signal nodes, which
 * carry marks directly (declaration walk or birth inheritance). This covers
 * UNTRACKED probes reading through records whose nodes never materialized
 * (no observer ever subscribed, so no node exists to carry the mark).
 * Callers guard on `pendingCheckActive`, so plain reads never pay for this.
 *
 * @internal
 */
export declare function witnessAffectsMark(target: StoreNode, property?: PropertyKey): void;
/**
 * Resolves the store nodes an `affects()` declaration marks: with a `key`,
 * the named slot's leaf node (upserted so the mark has an addressable
 * carrier); without, the record's $AFFECTS carrier plus every LIVE node in
 * its subtree (the edges existing readers subscribed through), with the
 * subtree's identities snapshotted into the mark's scope so nodes created
 * during the window — and untracked probes over captured proxies — resolve
 * against it (#2882).
 *
 * @internal
 */
export declare function getStoreAffectsNodes(target: StoreNode, key?: PropertyKey): DataNode[];
export {};
