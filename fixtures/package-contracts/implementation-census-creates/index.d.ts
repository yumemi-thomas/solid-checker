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
// Two overloads and no hand recipe. The runtime body is `plain`'s again; what
// differs is that the export states no single call signature, only a complete
// overload set, and synthesis samples every member of it (ADR 0036).
export declare function overloaded<T, U>(items: Iterable<T>, callback: (value: T) => U): U[];
export declare function overloaded<T, U>(items: Iterable<T>, callback: (value: T) => U, seed: U): U[];
export declare function loopCall(el: unknown): void;
export declare function switchBreak(kind: string, el: unknown): void;
export declare function whileBreak(el: unknown): void;
export declare function labelledBreak(el: unknown): void;
export declare function stdlibRefInvoker(items: Iterable<unknown>): void;
export declare function reflectApply(args: unknown[]): unknown;
export declare function reassignedHelper(el: unknown): unknown;
// ADR 0034: a read accessor rooted at an unwritten parameter certifies; the
// exports after it pin the boundary of that disposition, one premise each.
export declare function memberParameterRooted(source: { read(): unknown }): unknown;
export declare function toStringTagViaCall(value: unknown): boolean;
export declare function writtenBeforeRead(source: { value: unknown }): unknown;
export declare function writtenAfterRead(source: { value: unknown }): unknown;
export declare function moduleReceiverRead(): unknown;
export declare function nestedCallableParameterRead(items: Array<{ value: unknown }>): unknown[];
export declare function callNonLibraryReceiver(value: unknown): unknown;
export declare function callLibraryOutsideTable(value: unknown): unknown;
export declare function setterOnParameter(source: { value: unknown }): void;
// An immediately-invoked function expression: this census refuses it.
export declare function iife(value: number): number;
