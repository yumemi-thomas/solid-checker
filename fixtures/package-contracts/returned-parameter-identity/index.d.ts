export declare function identity<T extends () => any>(value: T): T;
export declare function second<T extends () => any, U extends () => any>(first: T, second: U): U;
export declare function mutated(value: number): number;
