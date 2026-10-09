import { NotReadyError } from "./error.js";
import { type OptimisticLane } from "./lanes.js";
import type { Computed, Link } from "./types.js";
export declare function addPendingSource(el: Computed<any>, source: Computed<any>): boolean;
/**
 * A loading-window node hit an unready source (sync throw in recompute, or a
 * NotReadyError-rejected flight): register for the source's settle — the
 * settlePendingSource walk runs off `_pendingSources` + `_blocked` alone —
 * with NO read-visible pending status, no downstream propagation, no
 * transition, no lane registration. Commit #0 keeps serving.
 */
export declare function parkLoadingWindow(el: Computed<any>, e: NotReadyError): void;
export declare function setPendingError(el: Computed<any>, source?: Computed<any>, error?: any): void;
export declare function forEachDependent(el: Computed<any>, fn: (node: Computed<any>, link: Link) => void): void;
export declare function releaseSettledDependents(el: Computed<any>): void;
export declare function settleErroredDependents(el: Computed<any>, error: any): void;
export declare function settlePendingSource(el: Computed<any>, source?: Computed<any>): void;
export declare function isThenable<T>(value: T | PromiseLike<T>): value is PromiseLike<T>;
/** Fire and clear a node's iterator-flight cancellation hook (#3122). */
export declare function releaseFlightTeardown(el: Computed<any>): void;
export declare function handleAsync<T>(el: Computed<T>, result: T | PromiseLike<T> | AsyncIterable<T>, setter?: (value: T) => void): T;
export declare function clearStatus(el: Computed<any>, clearUninitialized?: boolean): void;
export declare function notifyStatus(el: Computed<any>, status: number, error: any, blockStatus?: boolean, lane?: OptimisticLane): void;
