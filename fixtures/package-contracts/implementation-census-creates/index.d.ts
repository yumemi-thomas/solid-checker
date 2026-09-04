// The declarations a consumer compiles against. Every export is a plain
// function; nothing here says anything about `creates`, which is exactly why
// the domain is decided by a census of `index.js` and not of this file.
export declare function plain<T, U>(items: Iterable<T>, callback: (value: T) => U): U[];
export declare function viaHelperChain(callback: () => void): void;
export declare function cycle(): void;
export declare function deep(): void;
export declare function unresolved(value: number): number;
export declare function taggedTemplate(): string;
export declare function spreadArgs(first: number, second: number): number[];
export declare function spreadUntyped(items: Iterable<number>): number[];
export declare function noRecipe<T, U>(items: Iterable<T>, callback: (value: T) => U): U[];
export declare function loopCall(el: unknown): void;
export declare function switchBreak(kind: string, el: unknown): void;
export declare function whileBreak(el: unknown): void;
export declare function labelledBreak(el: unknown): void;
export declare function stdlibRefInvoker(items: Iterable<unknown>): void;
export declare function reflectApply(args: unknown[]): unknown;
export declare function reassignedHelper(el: unknown): unknown;
// The two exports that pin why the generator's walk keeps declining an
// unresolved member callee and an immediately-invoked function: this census
// refuses both.
export declare function memberParameterRooted(source: { read(): unknown }): unknown;
export declare function iife(value: number): number;
