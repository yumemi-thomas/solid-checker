export declare function derive(fn: () => unknown): void;
export declare function deriveMaybe(flag: boolean, fn: () => unknown): void;
export declare function watch(compute: () => unknown, effect: (value: unknown) => void): void;
export declare function deriveWrapped(fn: () => unknown): void;
export declare const deriveArrow: (fn: () => unknown) => () => unknown;
export declare function deriveWrappedMaybe(flag: boolean, fn: () => unknown): void;
export declare function deriveWrappedAsync(fn: () => unknown): void;
