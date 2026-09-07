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
export declare function constBound(callback: (value: number) => unknown): unknown;
export declare function callInitialized(): unknown;
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
export declare function updateOnParameter(source: { value: number }): void;
export declare function setterOnModuleValue(): void;
export declare function chainCallbacks(
  callbacks: Iterable<((...args: unknown[]) => void) | undefined>,
): (...args: unknown[]) => void;
export declare function chainModuleCallbacks(): void;
export declare function awaitIterateParameter(callbacks: AsyncIterable<unknown>): Promise<void>;
export declare function spreadParameter(source: unknown): unknown;
export declare function spreadWrittenParameter(source: unknown): unknown;
export declare function destructureParameter(source: unknown): unknown;
export declare function destructureModuleValue(): unknown;
// ADR 0038: these signatures are the premise the form census classifies the
// runtime bodies under. `typedCoercion`, `returnedCallbackCoercion` and
// `declaredMemberCoercion` certify because of the types stated here;
// `untypedCoercion` refuses on `unknown`. `helperCoercion` certifies because
// the argument types at its call reach the helper as the helper's premise
// (protocol 23); `helperSpreadCoercion` (a spread carries no slot) and
// `helperUntypedArgument` (an `any` slot) refuse at the helper.
export declare function typedCoercion(min: number, max: number, v: number): number;
export declare function untypedCoercion(value: unknown): unknown;
export declare function returnedCallbackCoercion(step: number): (p: number) => number;
export declare function declaredMemberCoercion(axis: { min: number; max: number }): number;
export declare function helperCoercion(a: number, b: number): number;
export declare function helperSpreadCoercion(a: number, b: number): number;
export declare function helperUntypedArgument(a: number): number;
// An immediately-invoked function expression: this census refuses it.
export declare function iife(value: number): number;
// ADR 0043: the root set closed under the reads the census dispositions. The
// four legs and their boundaries, one export each.
export declare function defaultedFromParameter(
  axis: { min: unknown },
  sourceAxis?: { min: unknown },
): unknown;
export declare function defaultedFromModuleValue(source?: any): unknown;
export declare function defaultedFromDefaulted(
  axis: { min: unknown },
  mid?: { min: unknown },
  tail?: { min: unknown },
): unknown;
export declare function patternParameter(source: { inner: { value: unknown } }): unknown;
export declare function patternParameterDefault(source?: any): unknown;
export declare function patternElementDefault(source: any): unknown;
export declare function patternRestParameter(source: any): unknown;
export declare function localBindingFromParameter(source: {
  inner: { value: { text: unknown } };
}): unknown;
export declare function localPatternFromParameter(source: {
  inner: { value: unknown };
}): unknown;
export declare function localBindingWritten(source: { inner: any }): unknown;
export declare function localBindingFromCall(source: { text: string }): unknown;
// ADR 0044: a value this program built. `key` and `index` stay `any` so the
// element access keeps a computed key the checker resolves no symbol for —
// a literal key would bind a data property and record no form at all.
export declare function ownTableRead(key: any): unknown;
export declare function ownArrayRead(index: any): unknown;
export declare function ownTableWrite(key: any): void;
export declare function ownRestSpread(source: any): unknown;
export declare function accessorTableRead(key: any): unknown;
export declare function protoTableRead(key: any): unknown;
export declare function writtenTableRead(key: any): unknown;
export declare function ownTableMemberRead(key: any): unknown;
export declare function arrayRestRead(source: any): unknown;
// ADR 0045: a coercion over a call into this program's own runtime source.
// The declared `number`s are what makes the other operand primitive; the
// helper's own completion is what the census asks the helper about.
export declare function coerceHelperResult(base: number): number;
export declare function coerceBoundHelperResult(base: number): number;
export declare function coerceConditionalHelperResult(base: number, factor: number): number;
export declare function coerceObjectHelperResult(base: number): unknown;
export declare function coerceWrittenHelperResult(base: number): unknown;
export declare function coerceLibraryResult(base: number): unknown;
// ADR 0047: whose `Symbol.hasInstance` an `instanceof` can reach.
export declare function instanceOfParameter(value: unknown, constructor: any): boolean;
export declare function instanceOfLibrary(value: unknown): boolean;
export declare function instanceOfOwnClass(value: unknown): boolean;
export declare function instanceOfDerivedClass(value: unknown): boolean;
export declare function instanceOfComputedClass(value: unknown): boolean;
export declare function instanceOfModuleValue(value: unknown): boolean;
// ADR 0048: a read of what a caller-supplied callee handed back.
export declare function readCallerResult(transform: (p: any) => any, point: any): unknown;
export declare function readBoundCallerResult(transform: (p: any) => any, point: any): unknown;
export declare function readLocalResult(point: any): unknown;
// ADR 0050: a written binding whose every value is rooted.
export declare function writtenJoin(source: any): unknown;
export declare function writtenFromUninitialized(items: any): unknown;
export declare function joinedArms(source: any): unknown;
export declare function writtenFromModuleValue(source: any, flag: any): unknown;
export declare function writtenFromTwoSlots(first: any, second: any, flag: any): unknown;
export declare function writtenByDestructuring(source: any, other: any): unknown;
