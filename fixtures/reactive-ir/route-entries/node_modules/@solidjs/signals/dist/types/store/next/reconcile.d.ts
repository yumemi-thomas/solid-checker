type KeyFn = (item: any) => any;
export declare function reconcileNextState(value: any, state: any, key: string | KeyFn | null | undefined, replace?: boolean): void;
/** Setter-channel row ops (the fold site calls this for array targets with
 * ops consumers): structural mutation through the setter — push/splice/index
/** Key equality for EVERY key comparison in this module (re-audit 2, P1-5):
 * SameValueZero, matching the adoption window's Map-based matcher — NaN keys
 * are equal to themselves, so aligned NaN rows stay aligned in the prefix
 * walk instead of forever misaligning. */
export declare function sameKey(a: any, b: any): boolean;
export {};
