// The declarations a consumer compiles against. `pipe` and `changed` are
// declared exactly as `@solid-primitives/utils@7.0.0-next.4` and
// `@solid-primitives/promise@2.0.0-next.2` declare them (the second with
// `Accessor` expanded). The census decides
// from `index.js`.
export declare function pipe<A, B>(a: (raw: string) => A, b: (a: A) => B): (raw: string) => B;
// `changed` with `Accessor<T>` written out as solid-js declares it.
export declare function changed(source: () => any, times?: number): () => boolean;
export declare function required(callback: () => void, message?: string): () => void;
export declare function guarded(callback?: () => void): () => void;
export declare function twice(callback: () => void): () => void;
export declare function deferred(callback: () => void): () => void;
export declare function early(callback: () => void, flag: boolean): () => void;
export declare function defaulted(callback?: () => void): () => void;
export declare function registered(callback: () => void): () => void;
