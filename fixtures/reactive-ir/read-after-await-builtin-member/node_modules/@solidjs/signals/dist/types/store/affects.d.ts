import type { Computed, Signal } from "../core/types.js";
import { type StoreTarget } from "./target.js";
type Marked = Signal<any> | Computed<any>;
/** The nodes an `affects(store[, key])` declaration marks (affects.ts
 * registers each). */
export declare function storeMarks(t: StoreTarget, key: PropertyKey | undefined): Marked[];
/** `affects()` calls it before a store-targeted declaration (idempotent). */
export declare function installStoreAffects(): void;
/** The carrier's last mark released (affects.ts): its scope dies, and the
 * nodes that inherited the mark release theirs. */
export declare function releaseMarkScope(carrier: Marked): void;
export {};
