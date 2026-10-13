// The declarations a consumer compiles against. The census decides from
// `index.js`; a declared result that is not callable would refuse the claim.
export declare const createIdGenerator: () => () => string;
export declare function makeNoop(): () => void;
export declare const makeSilent: () => () => void;
export declare function makeTicker(): () => number;
export declare function makeCounter(): () => number;
export declare function choose(flag: boolean): () => number | string;
export declare function invokesCaptured(fn: () => void): () => void;
export declare function invokesOwnArgument(): (callback: () => void) => void;
export declare function returnsObject(): () => object;
export declare function readsCapturedMember(options: { value: number }): () => boolean;
export declare function throughMutableBinding(flag: boolean): () => unknown;
