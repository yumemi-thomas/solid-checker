// The declarations a consumer compiles against; the census decides from
// `index.js`.
export declare function always(callback: () => void): void;
export declare function afterThrow(callback: () => void, message?: string): void;
export declare function guarded(callback: () => void, flag: boolean): void;
export declare function shortCircuit(callback: () => void, flag: boolean): void;
export declare function chosen(callback: () => void, flag: boolean): void;
export declare function optional(callback?: () => void): void;
export declare function early(callback: () => void, flag: boolean): void;
export declare function looped(callback: () => void, count: number): void;
