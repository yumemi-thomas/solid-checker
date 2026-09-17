// The declarations for the hazarded entrypoint. Byte-for-byte the same *kind*
// of declaration as `./index.d.ts` carries — nothing in a signature marks the
// receiver as a proxy, which is the whole reason the refusal has to come from
// the closure rather than from here.
export declare function readsOwnProxy(): number;
export declare function readsOwnProxyElement(key: string): number;
export declare function observedReads(): number;
