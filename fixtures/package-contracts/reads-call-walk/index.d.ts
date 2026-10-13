export declare function readsCreatedAccessor(value: number): number;
export declare function callsReadingHelper(value: number): number;
export declare function callsPureHelper(value: number): number;
export declare function invokesArgument(read: () => number): number;
export declare function callsStandardLibrary(value: number): number;
export declare function returnsReader(value: number): () => number;
export declare function mutualRecursion(n: number): boolean;
export declare function readingCycle(n: number, value: number): number;
export declare function callsNothing(value: number): number;
