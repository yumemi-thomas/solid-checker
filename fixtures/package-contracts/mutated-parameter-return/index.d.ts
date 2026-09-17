export declare function Direct<T>(value: T): T;
export declare function Mutated(value: number): number;
export declare function Updated(value: number): number;
export declare function Nested(value: number): number;
export declare function Alias<T>(value: T): T;
export declare function Defaulted(value?: number): number;
export declare function Destructured<T>(input: { value: T }): T;
export declare function Async<T>(value: T): Promise<Awaited<T>>;
