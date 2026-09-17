// The declarations a consumer compiles against. Nothing here decides
// `returns`: a `void` annotation is a type, and the domain is decided by a
// census of `index.js`'s completions, not of this file.
export declare function bareCompletion(count: number): void;
export declare function earlyBareReturn(flag: boolean): void;
export declare function bareReturnInLoop(count: number): void;
export declare function nestedReturnsValue(count: number): void;
export declare function returnsValue<T>(value: T): T;
export declare const expressionArrow: <T>(value: T) => T;
export declare function asyncVoid(): Promise<void>;
export declare function generatorVoid(): Generator<never, void, unknown>;
export declare function valueReturnInLoop(count: number): number | undefined;
