// The declarations a consumer compiles against: exactly the shapes `index.js`
// implements, over the `solid-js` types it re-exports.
import type { Context, Owner } from "solid-js";
export declare function trackEach(handle: () => void): void;
export declare function cleanUp(handle: () => void): void;
export declare function runUnder<T>(owner: Owner | null, fn: () => T): T;
export declare function makeContext<T>(value: T): Context<T>;
export declare function readContext<T>(context: Context<T>): T;
export declare function currentOwner(): Owner | null;
export declare function hasOwner(): boolean;
