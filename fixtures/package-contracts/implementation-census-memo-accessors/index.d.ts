export declare function createDoubled(count: () => number): () => number;
export declare const createLabel: (source: () => number) => () => string;
export declare function createLive<T>(compute: () => T): () => T;
export declare function createGuarded<T>(flag: boolean, a: () => T, b: () => T): () => T;
export declare function createEither<T>(flag: boolean, a: () => T, b: () => T): () => T;
export declare function createBound(source: () => number): (() => number) & { extra: number };
export declare function readOnce(source: () => number): number;
export declare function createShadowed<T>(fn: () => T): () => T;
